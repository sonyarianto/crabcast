//! Song history rows, pushed from the engine's track webhook.

use serde::Serialize;
use sqlx::{FromRow, SqlitePool};

use crate::api::error::ApiError;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SongHistory {
    pub id: i64,
    pub station_id: String,
    pub title: String,
    pub started_at: String,
    pub ended_at: Option<String>,
}

/// Record a new track: close the previous open row (if any) and insert a
/// new one. Returns the new row.
pub async fn push(
    pool: &SqlitePool,
    station_id: &str,
    title: &str,
) -> Result<SongHistory, ApiError> {
    sqlx::query(
        "UPDATE song_history SET ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
WHERE station_id = ? AND ended_at IS NULL",
    )
    .bind(station_id)
    .execute(pool)
    .await?;

    let row = sqlx::query_as::<_, SongHistory>(
        "INSERT INTO song_history (station_id, title) VALUES (?, ?) \
RETURNING id, station_id, title, started_at, ended_at",
    )
    .bind(station_id)
    .bind(title)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// The currently playing track (latest row without `ended_at`), if any.
pub async fn now_playing(
    pool: &SqlitePool,
    station_id: &str,
) -> Result<Option<SongHistory>, ApiError> {
    Ok(sqlx::query_as::<_, SongHistory>(
        "SELECT id, station_id, title, started_at, ended_at FROM song_history \
WHERE station_id = ? AND ended_at IS NULL ORDER BY started_at DESC LIMIT 1",
    )
    .bind(station_id)
    .fetch_optional(pool)
    .await?)
}

/// Recent history for a station (newest first).
pub async fn recent(
    pool: &SqlitePool,
    station_id: &str,
    limit: i64,
) -> Result<Vec<SongHistory>, ApiError> {
    Ok(sqlx::query_as::<_, SongHistory>(
        "SELECT id, station_id, title, started_at, ended_at FROM song_history \
WHERE station_id = ? ORDER BY started_at DESC LIMIT ?",
    )
    .bind(station_id)
    .bind(limit)
    .fetch_all(pool)
    .await?)
}
