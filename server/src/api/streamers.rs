//! Streamer routes (Phase 5): per-station live-DJ accounts with individual
//! source passwords. Mutations re-render the station config so a new or
//! revoked password takes effect immediately (the engine accepts any
//! enabled streamer password on the harbor).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use serde_json::json;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::auth::{Csrf, CurrentUser};
use crate::db::stations;
use crate::db::streamers::{self, Streamer, StreamerInput};
use crate::db::users;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/stations/{station_id}/streamers",
            axum::routing::get(list_streamers).post(create_streamer),
        )
        .route(
            "/api/streamers/{id}",
            axum::routing::get(get_streamer)
                .put(update_streamer)
                .delete(delete_streamer),
        )
        .route(
            "/api/streamers/{id}/connect",
            axum::routing::get(connect_info),
        )
}

fn forbidden(msg: &str) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        message: msg.into(),
    }
}

/// Re-render + restart the station's engine so password changes go live.
async fn reapply(state: &AppState, station_id: &str) -> ApiResult<()> {
    let station = stations::get(&state.pool, station_id).await?;
    state
        .supervisor
        .apply(&station)
        .await
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        })?;
    Ok(())
}

async fn list_streamers(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(station_id): Path<String>,
) -> ApiResult<Json<Vec<Streamer>>> {
    let rows = streamers::list_for_station(&state.pool, &station_id).await?;
    Ok(Json(rows))
}

async fn get_streamer(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Streamer>> {
    Ok(Json(streamers::get(&state.pool, &id).await?))
}

async fn create_streamer(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(station_id): Path<String>,
    Json(input): Json<StreamerInput>,
) -> ApiResult<(StatusCode, Json<Streamer>)> {
    if !user.can_manage_stations(&station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    let streamer = streamers::create(&state.pool, &station_id, &input).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "streamer.create",
        "streamers",
        &format!("{} ({})", streamer.name, streamer.id),
    )
    .await?;
    reapply(&state, &station_id).await?;
    Ok((StatusCode::CREATED, Json(streamer)))
}

async fn update_streamer(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(id): Path<String>,
    Json(input): Json<StreamerInput>,
) -> ApiResult<Json<Streamer>> {
    let existing = streamers::get(&state.pool, &id).await?;
    if !user.can_manage_stations(&existing.station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    let streamer = streamers::update(&state.pool, &id, &input).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "streamer.update",
        "streamers",
        &format!("{} ({})", streamer.name, streamer.id),
    )
    .await?;
    reapply(&state, &existing.station_id).await?;
    Ok(Json(streamer))
}

async fn delete_streamer(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let existing = streamers::get(&state.pool, &id).await?;
    if !user.can_manage_stations(&existing.station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    streamers::delete(&state.pool, &id).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "streamer.delete",
        "streamers",
        &format!("{} ({})", existing.name, existing.id),
    )
    .await?;
    reapply(&state, &existing.station_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The streamer-facing connect cheat sheet: mount URL + per-account
/// credentials + a copy-paste `curl` mic test.
pub async fn connect_info(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let streamer = streamers::get(&state.pool, &id).await?;
    let station = stations::get(&state.pool, &streamer.station_id).await?;
    Ok(Json(json!({
        "streamer": streamer,
        "mount_url": format!("http://{}/{}", station.icecast_host, station.harbor_mount.trim_start_matches('/')),
        "harbor_port": station.harbor_port,
        "mount": station.harbor_mount,
        "curl_mic_test": format!(
            "ffmpeg -f lavfi -i sine=frequency=440:duration=5 -f mp3 - | curl -u source:{} -T - http://localhost:{}{}",
            streamer.source_password, station.harbor_port, station.harbor_mount
        ),
    })))
}
