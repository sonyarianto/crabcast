//! Analytics & monitoring routes (Phase 8): listener series, summary,
//! top songs, request stats, song-history CSV export, the alert feed, and
//! the engine's dead-air webhook receiver.
//!
//! Reads follow the station convention (any authenticated user); mutating
//! actions (resolving alerts) require `station_manager` (or super admin for
//! global alerts). The blank webhook is unauthenticated like the track
//! webhook — the engine's `http_post` sends no headers.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use time::OffsetDateTime;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::auth::{Csrf, CurrentUser};
use crate::db::analytics;
use crate::db::stations;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/stations/{id}/analytics/listeners",
            get(listener_series),
        )
        .route("/api/stations/{id}/analytics/summary", get(summary))
        .route("/api/stations/{id}/analytics/top-songs", get(top_songs))
        .route("/api/stations/{id}/analytics/requests", get(request_stats))
        .route("/api/stations/{id}/analytics/history.csv", get(history_csv))
        .route("/api/alerts", get(list_alerts))
        .route("/api/alerts/{id}/resolve", post(resolve_alert))
        .route("/api/webhooks/blank", post(blank_webhook))
}

fn forbidden(msg: &str) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        message: msg.into(),
    }
}

/// RFC3339 string N hours in the past (default series window start).
fn hours_ago(hours: i64) -> String {
    (OffsetDateTime::now_utc() - time::Duration::hours(hours))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

#[derive(Deserialize)]
struct SeriesQuery {
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    bucket: Option<i64>,
}

#[derive(Serialize)]
struct SeriesBody {
    points: Vec<analytics::ListenerPoint>,
    bucket_minutes: i64,
}

async fn listener_series(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
    Query(q): Query<SeriesQuery>,
) -> ApiResult<Json<SeriesBody>> {
    stations::get(&state.pool, &id).await?;
    let from = q.from.unwrap_or_else(|| hours_ago(24));
    let to = q.to.unwrap_or_else(|| hours_ago(0));
    let bucket = q.bucket.unwrap_or(60).clamp(1, 1440);
    let points = analytics::listener_series(&state.pool, &id, &from, &to, bucket).await?;
    Ok(Json(SeriesBody {
        points,
        bucket_minutes: bucket,
    }))
}

async fn summary(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<analytics::AnalyticsSummary>> {
    stations::get(&state.pool, &id).await?;
    Ok(Json(analytics::summary(&state.pool, &id).await?))
}

#[derive(Deserialize)]
struct DaysQuery {
    #[serde(default)]
    days: Option<i64>,
}

async fn top_songs(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
    Query(q): Query<DaysQuery>,
) -> ApiResult<Json<Vec<analytics::TopSong>>> {
    stations::get(&state.pool, &id).await?;
    let days = q.days.unwrap_or(7).clamp(1, 365);
    Ok(Json(
        analytics::top_songs(&state.pool, &id, days, 25).await?,
    ))
}

async fn request_stats(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
    Query(q): Query<DaysQuery>,
) -> ApiResult<Json<Vec<analytics::RequestDay>>> {
    stations::get(&state.pool, &id).await?;
    let days = q.days.unwrap_or(7).clamp(1, 365);
    Ok(Json(
        analytics::request_stats(&state.pool, &id, days).await?,
    ))
}

#[derive(Deserialize)]
struct HistoryCsvQuery {
    #[serde(default)]
    days: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
struct HistoryCsvRow {
    title: String,
    started_at: String,
    ended_at: Option<String>,
    duration_seconds: Option<f64>,
}

/// Song history as a downloadable CSV (newest first).
async fn history_csv(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
    Query(q): Query<HistoryCsvQuery>,
) -> ApiResult<axum::response::Response> {
    let station = stations::get(&state.pool, &id).await?;
    let days = q.days.unwrap_or(30).clamp(1, 3650);
    let since = (OffsetDateTime::now_utc() - time::Duration::days(days))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into());
    let rows = sqlx::query_as::<_, HistoryCsvRow>(
        "SELECT title, started_at, ended_at, \
CASE WHEN ended_at IS NOT NULL THEN (julianday(ended_at) - julianday(started_at)) * 86400.0 END AS duration_seconds \
FROM song_history WHERE station_id = ? AND started_at >= ? ORDER BY started_at DESC LIMIT 100000",
    )
    .bind(&id)
    .bind(since)
    .fetch_all(&state.pool)
    .await?;

    let mut csv = String::from("title,started_at,ended_at,duration_seconds\n");
    for row in &rows {
        csv.push_str(&format!(
            "{},{},{},{}\n",
            csv_field(&row.title),
            csv_field(&row.started_at),
            csv_field(row.ended_at.as_deref().unwrap_or("")),
            row.duration_seconds
                .map(|d| format!("{d:.2}"))
                .unwrap_or_default(),
        ));
    }

    let filename = format!("{}-history.csv", station.name.trim().replace(' ', "-"));
    let body = axum::body::Body::from(csv);
    Ok((
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        body,
    )
        .into_response())
}

/// Quote a CSV field per RFC 4180 (wrap in quotes when it contains a
/// comma, quote, or newline; double embedded quotes).
fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[derive(Deserialize)]
struct AlertsQuery {
    #[serde(default)]
    station_id: Option<String>,
    #[serde(default)]
    open: Option<bool>,
}

async fn list_alerts(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<AlertsQuery>,
) -> ApiResult<Json<Vec<analytics::Alert>>> {
    let open_only = q.open.unwrap_or(false);
    match q.station_id {
        Some(station_id) => {
            stations::get(&state.pool, &station_id).await?;
            Ok(Json(
                analytics::list_alerts(&state.pool, Some(&station_id), open_only, 100).await?,
            ))
        }
        None => {
            // Cross-station/global view is ops-only.
            if !user.user.is_super_admin {
                return Err(forbidden("super admin permission required"));
            }
            Ok(Json(
                analytics::list_alerts(&state.pool, None, open_only, 100).await?,
            ))
        }
    }
}

async fn resolve_alert(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let alert = analytics::get_alert(&state.pool, &id).await?;
    match &alert.station_id {
        Some(station_id) => {
            if !user.can_manage_stations(station_id) {
                return Err(forbidden(
                    "station_manager permission required for this station",
                ));
            }
        }
        None => {
            if !user.user.is_super_admin {
                return Err(forbidden("super admin permission required"));
            }
        }
    }
    let resolved = analytics::resolve_alert(&state.pool, &id).await?;
    Ok(Json(json!({ "ok": true, "resolved": resolved })))
}

/// Engine dead-air webhook: `POST /api/webhooks/blank?station=<id>`. Fired
/// by the generated `blank.detect(...).on_blank` hook after the configured
/// silence window. Raises a deduplicated `dead_air` alert; the track
/// webhook clears it once real audio returns.
async fn blank_webhook(
    State(state): State<AppState>,
    Query(query): Query<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let station_id = query
        .get("station")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "missing ?station=<id> query parameter".into(),
        })?;
    let _ = stations::get(&state.pool, station_id).await?;

    if let Some(alert) = analytics::raise_alert(
        &state.pool,
        Some(station_id),
        "dead_air",
        "error",
        "Dead air detected",
        "The engine reported silence for 5s; the stream is still up but silent.",
    )
    .await?
    {
        tracing::warn!("station {station_id}: dead air detected");
        crate::analytics::notify(
            std::env::var("CRABCAST_ALERT_WEBHOOK_URL").ok().as_deref(),
            "raised",
            &alert,
        )
        .await;
    }
    Ok(Json(json!({ "ok": true })))
}

/// Resolve an open `dead_air` alert once the engine reports a real track
/// again (called from the track webhook receiver).
pub async fn clear_dead_air(pool: &sqlx::SqlitePool, station_id: &str) {
    match analytics::resolve_open(pool, Some(station_id), "dead_air").await {
        Ok(0) => {}
        Ok(_) => tracing::info!("station {station_id}: audio recovered, dead-air alert cleared"),
        Err(e) => tracing::error!("analytics: clear dead-air alert failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_fields_are_quoted_when_needed() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn csv_export_shape() {
        // The rows → CSV mapping used by history_csv; title always escaped.
        let rows = vec![
            HistoryCsvRow {
                title: "Artist, The - Song".into(),
                started_at: "2026-08-01T10:00:00.000Z".into(),
                ended_at: Some("2026-08-01T10:03:30.000Z".into()),
                duration_seconds: Some(210.0),
            },
            HistoryCsvRow {
                title: "Plain Song".into(),
                started_at: "2026-08-01T11:00:00.000Z".into(),
                ended_at: None,
                duration_seconds: None,
            },
        ];
        let mut csv = String::from("title,started_at,ended_at,duration_seconds\n");
        for row in &rows {
            csv.push_str(&format!(
                "{},{},{},{}\n",
                csv_field(&row.title),
                csv_field(&row.started_at),
                csv_field(row.ended_at.as_deref().unwrap_or("")),
                row.duration_seconds
                    .map(|d| format!("{d:.2}"))
                    .unwrap_or_default(),
            ));
        }
        assert_eq!(
            csv,
            "title,started_at,ended_at,duration_seconds\n\
\"Artist, The - Song\",2026-08-01T10:00:00.000Z,2026-08-01T10:03:30.000Z,210.00\n\
Plain Song,2026-08-01T11:00:00.000Z,,\n"
        );
    }
}
