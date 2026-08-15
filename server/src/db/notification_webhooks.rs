//! On-air notification webhooks (Phase 11): per-station Slack/Discord
//! webhook URLs that the server posts to on station events
//! (started/stopped/crashed/blank).

use serde::Serialize;
use sqlx::FromRow;
use sqlx::SqlitePool;

use crate::api::error::ApiError;

/// The events a webhook can subscribe to; a `*` subscription means all.
pub const EVENTS: [&str; 4] = ["started", "stopped", "crashed", "blank"];

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct NotificationWebhook {
    pub id: String,
    pub station_id: String,
    pub url: String,
    pub events: String,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct WebhookInput {
    pub url: String,
    pub events: String,
    pub enabled: bool,
}

/// Validate a comma-separated events string: '*' or a subset of EVENTS.
pub fn validate_events(events: &str) -> bool {
    let trimmed = events.trim();
    if trimmed == "*" {
        return true;
    }
    trimmed
        .split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .all(|e| EVENTS.contains(&e))
}

pub async fn create(
    pool: &SqlitePool,
    station_id: &str,
    input: &WebhookInput,
) -> Result<NotificationWebhook, ApiError> {
    if !validate_events(&input.events) {
        return Err(ApiError::bad_request(format!(
            "events must be '*' or a comma-separated subset of {}",
            EVENTS.join(", ")
        )));
    }
    if input.url.trim().is_empty() {
        return Err(ApiError::bad_request("url is required"));
    }
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO notification_webhooks (id, station_id, url, events, enabled) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(station_id)
    .bind(input.url.trim())
    .bind(input.events.trim())
    .bind(input.enabled)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn list(
    pool: &SqlitePool,
    station_id: &str,
) -> Result<Vec<NotificationWebhook>, ApiError> {
    Ok(sqlx::query_as::<_, NotificationWebhook>(
        "SELECT id, station_id, url, events, enabled, created_at \
             FROM notification_webhooks WHERE station_id = ? ORDER BY created_at",
    )
    .bind(station_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<NotificationWebhook, ApiError> {
    sqlx::query_as::<_, NotificationWebhook>(
        "SELECT id, station_id, url, events, enabled, created_at \
         FROM notification_webhooks WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("webhook", id))
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), ApiError> {
    let affected = sqlx::query("DELETE FROM notification_webhooks WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if affected.rows_affected() == 0 {
        return Err(ApiError::not_found("webhook", id));
    }
    Ok(())
}

/// Enabled webhooks for a station subscribed to `event`.
pub async fn for_event(
    pool: &SqlitePool,
    station_id: &str,
    event: &str,
) -> Result<Vec<NotificationWebhook>, ApiError> {
    Ok(sqlx::query_as::<_, NotificationWebhook>(
        "SELECT id, station_id, url, events, enabled, created_at \
             FROM notification_webhooks \
             WHERE station_id = ? AND enabled = 1",
    )
    .bind(station_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .filter(|w| {
        let events = w.events.trim();
        events == "*" || events.split(',').any(|e| e.trim() == event)
    })
    .collect())
}
