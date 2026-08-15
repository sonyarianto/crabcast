//! Podcast episodes (Phase 11 stretch): each episode references an audio
//! file in the media library; the public RSS feed is rendered from this
//! join at request time.

use serde::Serialize;
use sqlx::FromRow;
use sqlx::SqlitePool;

use crate::api::error::{ApiError, ApiResult};
use crate::db::now;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Episode {
    pub id: String,
    pub station_id: String,
    pub media_id: String,
    pub title: String,
    pub description: String,
    pub created_at: String,
}

/// Flat join row: episode + the media file it points at (for the feed).
#[derive(Debug, Clone, FromRow)]
pub struct EpisodeFeedRow {
    pub id: String,
    pub station_id: String,
    pub media_id: String,
    pub title: String,
    pub description: String,
    pub created_at: String,
    pub filename: String,
    pub mime: String,
    pub size_bytes: i64,
    pub artist: String,
    pub album: String,
}

impl EpisodeFeedRow {
    pub fn into_episode(self) -> Episode {
        Episode {
            id: self.id,
            station_id: self.station_id,
            media_id: self.media_id,
            title: self.title,
            description: self.description,
            created_at: self.created_at,
        }
    }
}

pub async fn create(
    pool: &SqlitePool,
    station_id: &str,
    media_id: &str,
    title: &str,
    description: &str,
) -> ApiResult<Episode> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO podcast_episodes (id, station_id, media_id, title, description, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(station_id)
    .bind(media_id)
    .bind(title)
    .bind(description)
    .bind(now())
    .execute(pool)
    .await
    .map_err(|e| match e {
        // Unknown station/media id → FK violation (code 787 is SQLite's
        // foreign key constraint failure).
        sqlx::Error::Database(db) if db.is_foreign_key_violation() => {
            ApiError::bad_request("station or media file not found")
        }
        other => other.into(),
    })?;
    Ok(Episode {
        id,
        station_id: station_id.to_string(),
        media_id: media_id.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        created_at: now(),
    })
}

pub async fn list(pool: &SqlitePool, station_id: &str) -> sqlx::Result<Vec<EpisodeFeedRow>> {
    sqlx::query_as::<_, EpisodeFeedRow>(
        "SELECT e.id, e.station_id, e.media_id, e.title, e.description, e.created_at,
                m.filename, m.mime, m.size_bytes, COALESCE(m.artist, '') AS artist,
                COALESCE(m.album, '') AS album
         FROM podcast_episodes e
         JOIN media_files m ON m.id = e.media_id
         WHERE e.station_id = ?
         ORDER BY e.created_at DESC",
    )
    .bind(station_id)
    .fetch_all(pool)
    .await
}

pub async fn delete(pool: &SqlitePool, episode_id: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM podcast_episodes WHERE id = ?")
        .bind(episode_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}
