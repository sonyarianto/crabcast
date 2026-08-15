//! Request routes (Phase 6): listener requests mapped to the engine's
//! `queue.push`, per-station rules (rate limit, dedupe, moderation), the
//! moderation inbox, and remote control of the engine queue.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::auth::{Csrf, CurrentUser};
use crate::control::ControlClient;
use crate::db::media;
use crate::db::requests::{self, RequestDetail, RequestRules, RequestRulesInput};
use crate::db::stations;
use crate::db::users;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/stations/{station_id}/request-rules",
            axum::routing::get(get_rules).put(put_rules),
        )
        .route(
            "/api/stations/{station_id}/requests",
            axum::routing::get(list_requests).post(create_request),
        )
        .route(
            "/api/stations/{station_id}/requests/{request_id}/approve",
            axum::routing::post(approve_request),
        )
        .route(
            "/api/stations/{station_id}/requests/{request_id}/reject",
            axum::routing::post(reject_request),
        )
        .route(
            "/api/stations/{station_id}/queue",
            axum::routing::get(get_queue).post(clear_queue),
        )
        .route(
            "/api/stations/{station_id}/queue/skip",
            axum::routing::post(skip_queue),
        )
}

fn forbidden(msg: &str) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        message: msg.into(),
    }
}

async fn engine_client(state: &AppState, station_id: &str) -> ApiResult<ControlClient> {
    let station = stations::get(&state.pool, station_id).await?;
    Ok(ControlClient::new(format!(
        "http://127.0.0.1:{}",
        station.control_http_port
    )))
}

async fn get_rules(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(station_id): Path<String>,
) -> ApiResult<Json<RequestRules>> {
    Ok(Json(
        requests::ensure_rules(&state.pool, &station_id).await?,
    ))
}

async fn put_rules(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(station_id): Path<String>,
    Json(input): Json<RequestRulesInput>,
) -> ApiResult<Json<RequestRules>> {
    if !user.can_manage_stations(&station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    let rules = requests::update_rules(&state.pool, &station_id, &input).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "request_rules.update",
        "stations",
        &station_id,
    )
    .await?;
    Ok(Json(rules))
}

#[derive(Deserialize)]
struct RequestBody {
    media_id: String,
}

/// Listener-facing and public (no auth — the Phase 7 request form works
/// without an account; the rules limit abuse).
async fn create_request(
    State(state): State<AppState>,
    Path(station_id): Path<String>,
    Json(body): Json<RequestBody>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let rules = requests::ensure_rules(&state.pool, &station_id).await?;
    if !rules.enabled {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "requests are disabled for this station".into(),
        });
    }

    // The track must exist in the library.
    let row = media::row_by_id(&state.pool, &body.media_id)
        .await?
        .ok_or_else(|| ApiError::not_found("media", &body.media_id))?;

    // Rate limit: reject when the rolling-hour cap is reached.
    let in_hour = requests::count_in_hour(&state.pool, &station_id).await?;
    if in_hour >= rules.max_per_hour {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: format!(
                "rate limit reached ({in_hour}/{}) this hour",
                rules.max_per_hour
            ),
        });
    }

    // Dedupe: a track that is already pending/queued (or in the engine
    // queue) is rejected.
    if *rules.dedupe {
        if requests::already_requested(&state.pool, &station_id, &body.media_id).await? {
            return Err(ApiError {
                status: StatusCode::BAD_REQUEST,
                message: "this track was already requested".into(),
            });
        }
        let client = engine_client(&state, &station_id).await?;
        if let Ok(queue) = client.queue().await {
            let path = format!("{}/{}", state.storage.root().display(), row.storage_path);
            if queue.contains(&path) {
                return Err(ApiError {
                    status: StatusCode::BAD_REQUEST,
                    message: "this track is already in the request queue".into(),
                });
            }
        }
    }

    let moderated = bool::from(rules.moderation);
    let req =
        requests::insert_request(&state.pool, &station_id, &body.media_id, None, moderated).await?;

    if !moderated {
        // Push straight to the engine; the request plays within seconds.
        let client = engine_client(&state, &station_id).await?;
        let path = format!("{}/{}", state.storage.root().display(), row.storage_path);
        client
            .cmd(&format!("queue.push {path}"))
            .await
            .map_err(|e| ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: format!("engine queue push failed: {e}"),
            })?;
    }

    users::log_audit(
        &state.pool,
        None,
        "request.create",
        "requests",
        &format!("{} ({})", req.id, body.media_id),
    )
    .await?;

    let status = if moderated { "pending" } else { "queued" };
    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": req.id, "status": status, "moderated": moderated })),
    ))
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    pending: bool,
}

async fn list_requests(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(station_id): Path<String>,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Vec<RequestDetail>>> {
    Ok(Json(
        requests::list_requests(&state.pool, &station_id, q.pending).await?,
    ))
}

async fn approve_request(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path((station_id, request_id)): Path<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    if !user.can_manage_stations(&station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    let req = requests::get_request(&state.pool, &request_id).await?;
    if req.status != "pending" {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "request is not pending".into(),
        });
    }
    let row = media::row_by_id(&state.pool, &req.media_id)
        .await?
        .ok_or_else(|| ApiError::not_found("media", &req.media_id))?;
    let client = engine_client(&state, &station_id).await?;
    let path = format!("{}/{}", state.storage.root().display(), row.storage_path);
    client
        .cmd(&format!("queue.push {path}"))
        .await
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("engine queue push failed: {e}"),
        })?;
    requests::set_status(&state.pool, &request_id, "queued").await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "request.approve",
        "requests",
        &request_id,
    )
    .await?;
    Ok(Json(
        json!({ "ok": true, "id": request_id, "status": "queued" }),
    ))
}

async fn reject_request(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path((station_id, request_id)): Path<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    if !user.can_manage_stations(&station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    requests::set_status(&state.pool, &request_id, "rejected").await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "request.reject",
        "requests",
        &request_id,
    )
    .await?;
    Ok(Json(
        json!({ "ok": true, "id": request_id, "status": "rejected" }),
    ))
}

async fn get_queue(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(station_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let client = engine_client(&state, &station_id).await?;
    let queue = client.queue().await.map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("engine unreachable: {e}"),
    })?;
    Ok(Json(json!({ "queue": queue })))
}

async fn clear_queue(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(station_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    if !user.can_control_station(&station_id) {
        return Err(forbidden(
            "station_manager or dj permission required for this station",
        ));
    }
    let client = engine_client(&state, &station_id).await?;
    client.cmd("queue.clear").await.map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("engine unreachable: {e}"),
    })?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "queue.clear",
        "stations",
        &station_id,
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn skip_queue(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(station_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    if !user.can_control_station(&station_id) {
        return Err(forbidden(
            "station_manager or dj permission required for this station",
        ));
    }
    let client = engine_client(&state, &station_id).await?;
    client.cmd("queue.skip").await.map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("engine unreachable: {e}"),
    })?;
    Ok(Json(json!({ "ok": true })))
}
