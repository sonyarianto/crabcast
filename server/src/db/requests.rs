//! Listener requests (Phase 6): per-station rules (rate limit, dedupe,
//! moderation), the request log, and moderation actions.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::SqlitePool;

use crate::api::error::ApiError;
use crate::db::now;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RequestRules {
    pub station_id: String,
    pub enabled: bool,
    pub max_per_hour: i64,
    pub dedupe: bool,
    pub moderation: bool,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Request {
    pub id: String,
    pub station_id: String,
    pub media_id: String,
    pub requested_by: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Flat row shape of the requests × media_files join; converted to
/// [`RequestDetail`] (which embeds a [`Request`]) by the caller.
#[derive(Debug, Clone, FromRow)]
pub struct RequestDetailRow {
    pub id: String,
    pub station_id: String,
    pub media_id: String,
    pub requested_by: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub title: String,
    pub artist: String,
    pub filename: String,
}

impl RequestDetailRow {
    pub fn into_detail(self) -> RequestDetail {
        RequestDetail {
            request: Request {
                id: self.id,
                station_id: self.station_id,
                media_id: self.media_id,
                requested_by: self.requested_by,
                status: self.status,
                created_at: self.created_at,
                updated_at: self.updated_at,
            },
            title: self.title,
            artist: self.artist,
            filename: self.filename,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestDetail {
    #[serde(flatten)]
    pub request: Request,
    pub title: String,
    pub artist: String,
    pub filename: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestRulesInput {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_max")]
    pub max_per_hour: i64,
    #[serde(default = "default_true")]
    pub dedupe: bool,
    #[serde(default)]
    pub moderation: bool,
}

fn default_max() -> i64 {
    5
}
fn default_true() -> bool {
    true
}

/// Default (disabled) rules row for a station; insert-or-get.
pub async fn ensure_rules(pool: &SqlitePool, station_id: &str) -> Result<RequestRules, ApiError> {
    let existing = sqlx::query_as::<_, RequestRules>(
        "SELECT station_id, enabled, max_per_hour, dedupe, moderation FROM request_rules WHERE station_id = ?",
    )
    .bind(station_id)
    .fetch_optional(pool)
    .await?;
    if let Some(r) = existing {
        return Ok(r);
    }
    sqlx::query("INSERT INTO request_rules (station_id, enabled, max_per_hour, dedupe, moderation) VALUES (?, 0, 5, 1, 0)")
        .bind(station_id)
        .execute(pool)
        .await?;
    get_rules(pool, station_id).await
}

pub async fn get_rules(pool: &SqlitePool, station_id: &str) -> Result<RequestRules, ApiError> {
    let row = sqlx::query_as::<_, RequestRules>(
        "SELECT station_id, enabled, max_per_hour, dedupe, moderation FROM request_rules WHERE station_id = ?",
    )
    .bind(station_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("request_rules", station_id))?;
    Ok(row)
}

pub async fn update_rules(
    pool: &SqlitePool,
    station_id: &str,
    input: &RequestRulesInput,
) -> Result<RequestRules, ApiError> {
    if input.max_per_hour < 0 {
        return Err(ApiError::bad_request("max_per_hour must be >= 0"));
    }
    ensure_rules(pool, station_id).await?;
    sqlx::query(
        "UPDATE request_rules SET enabled = ?, max_per_hour = ?, dedupe = ?, moderation = ? WHERE station_id = ?",
    )
    .bind(input.enabled)
    .bind(input.max_per_hour)
    .bind(input.dedupe)
    .bind(input.moderation)
    .bind(station_id)
    .execute(pool)
    .await?;
    get_rules(pool, station_id).await
}

/// How many requests were accepted in the rolling hour — the rate-limit
/// check (pending requests count too, they are still "requested").
pub async fn count_in_hour(pool: &SqlitePool, station_id: &str) -> Result<i64, ApiError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM requests
         WHERE station_id = ? AND status != 'rejected' AND created_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 hour')",
    )
    .bind(station_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// True when this media already has a live request (pending or queued).
pub async fn already_requested(
    pool: &SqlitePool,
    station_id: &str,
    media_id: &str,
) -> Result<bool, ApiError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM requests WHERE station_id = ? AND media_id = ? AND status IN ('pending', 'queued')",
    )
    .bind(station_id)
    .bind(media_id)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

/// Insert a new request; `moderated` controls the initial status.
pub async fn insert_request(
    pool: &SqlitePool,
    station_id: &str,
    media_id: &str,
    requested_by: Option<&str>,
    moderated: bool,
) -> Result<Request, ApiError> {
    let id = uuid::Uuid::new_v4().to_string();
    let ts = now();
    let status = if moderated { "pending" } else { "queued" };
    sqlx::query(
        "INSERT INTO requests (id, station_id, media_id, requested_by, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(station_id)
    .bind(media_id)
    .bind(requested_by)
    .bind(status)
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await?;
    get_request(pool, &id).await
}

pub async fn get_request(pool: &SqlitePool, id: &str) -> Result<Request, ApiError> {
    let row = sqlx::query_as::<_, Request>(
        "SELECT id, station_id, media_id, requested_by, status, created_at, updated_at FROM requests WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("request", id))?;
    Ok(row)
}

/// Pending (moderation inbox) or recent requests joined with media titles.
pub async fn list_requests(
    pool: &SqlitePool,
    station_id: &str,
    only_pending: bool,
) -> Result<Vec<RequestDetail>, ApiError> {
    let sql = if only_pending {
        "SELECT r.id, r.station_id, r.media_id, r.requested_by, r.status, r.created_at, r.updated_at,
                COALESCE(m.title, m.filename) AS title, COALESCE(m.artist, '') AS artist, m.filename AS filename
         FROM requests r JOIN media_files m ON m.id = r.media_id
         WHERE r.station_id = ? AND r.status = 'pending'
         ORDER BY r.created_at"
    } else {
        "SELECT r.id, r.station_id, r.media_id, r.requested_by, r.status, r.created_at, r.updated_at,
                COALESCE(m.title, m.filename) AS title, COALESCE(m.artist, '') AS artist, m.filename AS filename
         FROM requests r JOIN media_files m ON m.id = r.media_id
         WHERE r.station_id = ?
         ORDER BY r.created_at DESC LIMIT 50"
    };
    let rows = sqlx::query_as::<_, RequestDetailRow>(sql)
        .bind(station_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(RequestDetailRow::into_detail)
        .collect();
    Ok(rows)
}

pub async fn set_status(pool: &SqlitePool, id: &str, status: &str) -> Result<Request, ApiError> {
    let affected = sqlx::query("UPDATE requests SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(now())
        .bind(id)
        .execute(pool)
        .await?;
    if affected.rows_affected() == 0 {
        return Err(ApiError::not_found("request", id));
    }
    get_request(pool, id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool")
    }

    async fn seed(pool: &SqlitePool) -> String {
        sqlx::query("CREATE TABLE stations (id TEXT PRIMARY KEY, name TEXT NOT NULL)")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE media_files (id TEXT PRIMARY KEY, filename TEXT NOT NULL, title TEXT, artist TEXT)")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO stations (id, name) VALUES ('st1', 'Test FM')")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO media_files (id, filename) VALUES ('m1', 'a.mp3'), ('m2', 'b.mp3')",
        )
        .execute(pool)
        .await
        .unwrap();
        let mig = std::fs::read_to_string("migrations/0007_requests.sql").unwrap();
        sqlx::raw_sql(&mig).execute(pool).await.unwrap();
        "st1".to_string()
    }

    #[tokio::test]
    async fn rules_default_disabled_and_rate_limit_counts() {
        let pool = test_pool().await;
        let station = seed(&pool).await;

        let rules = ensure_rules(&pool, &station).await.unwrap();
        assert!(!rules.enabled);
        assert_eq!(rules.max_per_hour, 5);

        insert_request(&pool, &station, "m1", None, false)
            .await
            .unwrap();
        insert_request(&pool, &station, "m2", None, true)
            .await
            .unwrap();
        assert_eq!(count_in_hour(&pool, &station).await.unwrap(), 2);
        assert!(already_requested(&pool, &station, "m1").await.unwrap());
        assert!(already_requested(&pool, &station, "m2").await.unwrap());
    }

    #[tokio::test]
    async fn moderation_flow_and_status_transitions() {
        let pool = test_pool().await;
        let station = seed(&pool).await;

        // Moderated station: request lands as pending.
        update_rules(
            &pool,
            &station,
            &RequestRulesInput {
                enabled: true,
                max_per_hour: 5,
                dedupe: true,
                moderation: true,
            },
        )
        .await
        .unwrap();

        let req = insert_request(&pool, &station, "m1", Some("alice"), true)
            .await
            .unwrap();
        assert_eq!(req.status, "pending");

        let pending = list_requests(&pool, &station, true).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].title, "a.mp3");

        // Approve → queued; pending inbox empties.
        set_status(&pool, &req.id, "queued").await.unwrap();
        assert!(
            list_requests(&pool, &station, true)
                .await
                .unwrap()
                .is_empty()
        );

        // Reject a second request.
        let req2 = insert_request(&pool, &station, "m2", None, true)
            .await
            .unwrap();
        set_status(&pool, &req2.id, "rejected").await.unwrap();
        let recent = list_requests(&pool, &station, false).await.unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].request.status, "rejected");
        // Rejected requests do not count toward the hourly cap.
        assert_eq!(count_in_hour(&pool, &station).await.unwrap(), 1);
    }
}
