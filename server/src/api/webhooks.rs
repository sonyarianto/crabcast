//! On-air notification webhooks (Phase 11): per-station Slack/Discord
//! webhook CRUD for station managers. The server posts station events
//! (started/stopped/crashed/blank) to subscribed webhooks via
//! `crate::notify`.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::auth::{Csrf, CurrentUser};
use crate::db::notification_webhooks::{self, NotificationWebhook, WebhookInput};
use crate::db::users;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/stations/{station_id}/webhooks",
            get(list).post(create),
        )
        .route("/api/webhooks/{webhook_id}", axum::routing::delete(delete))
}

async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(station_id): Path<String>,
) -> ApiResult<Json<Vec<NotificationWebhook>>> {
    if !user.can_manage_stations(&station_id) {
        return Err(forbidden());
    }
    Ok(Json(
        notification_webhooks::list(&state.pool, &station_id).await?,
    ))
}

#[derive(Deserialize)]
struct WebhookCreate {
    url: String,
    #[serde(default = "default_events")]
    events: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_events() -> String {
    "*".into()
}
fn default_enabled() -> bool {
    true
}

async fn create(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(station_id): Path<String>,
    Json(input): Json<WebhookCreate>,
) -> ApiResult<(StatusCode, Json<NotificationWebhook>)> {
    if !user.can_manage_stations(&station_id) {
        return Err(forbidden());
    }
    let wh = notification_webhooks::create(
        &state.pool,
        &station_id,
        &WebhookInput {
            url: input.url,
            events: input.events,
            enabled: input.enabled.into(),
        },
    )
    .await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "webhook.create",
        &wh.id,
        &format!("station {station_id}: {}", wh.url),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(wh)))
}

async fn delete(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(webhook_id): Path<String>,
) -> ApiResult<StatusCode> {
    // The webhook's station is the scope for the permission check.
    let station_id = sqlx::query_scalar::<_, String>(
        "SELECT station_id FROM notification_webhooks WHERE id = $1",
    )
    .bind(&webhook_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("webhook", &webhook_id))?;
    if !user.can_manage_stations(&station_id) {
        return Err(forbidden());
    }
    notification_webhooks::delete(&state.pool, &webhook_id).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "webhook.delete",
        &webhook_id,
        &format!("station {station_id}"),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn forbidden() -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        message: "station_manager permission required".into(),
    }
}
