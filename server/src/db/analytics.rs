//! Analytics & monitoring repository (Phase 8): listener samples polled
//! from the Icecast admin API, the alert feed, top-song/request stats, and
//! retention cleanup.

use serde::Serialize;
use sqlx::FromRow;
use sqlx::SqlitePool;
use time::Duration;
use time::OffsetDateTime;

use crate::api::error::ApiError;
use crate::db::now;

// ---------------------------------------------------------------------------
// Listener samples
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ListenerSample {
    pub id: i64,
    pub station_id: String,
    pub ts: String,
    pub listeners: i64,
    pub listener_connections: i64,
    pub reachable: bool,
}

/// One bucketed point of a listener series (AVG per bucket).
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ListenerPoint {
    pub ts: String,
    pub listeners: i64,
    pub connections: i64,
    /// Samples in the bucket (used for uptime %).
    pub samples: i64,
    /// Bucket samples where the admin API responded.
    pub reachable: i64,
}

pub async fn insert_sample(
    pool: &SqlitePool,
    station_id: &str,
    listeners: i64,
    listener_connections: i64,
    reachable: bool,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO listener_samples (station_id, ts, listeners, listener_connections, reachable) \
VALUES (?, ?, ?, ?, ?)",
    )
    .bind(station_id)
    .bind(now())
    .bind(listeners)
    .bind(listener_connections)
    .bind(reachable)
    .execute(pool)
    .await?;
    Ok(())
}

/// Bucketed listener series between `from` and `to` (RFC3339, inclusive),
/// averaged per `bucket_minutes`. Returns points ordered by time.
pub async fn listener_series(
    pool: &SqlitePool,
    station_id: &str,
    from: &str,
    to: &str,
    bucket_minutes: i64,
) -> Result<Vec<ListenerPoint>, ApiError> {
    let bucket_seconds = bucket_minutes.max(1) * 60;
    let rows = sqlx::query_as::<_, ListenerPoint>(
        "SELECT \
datetime((strftime('%s', ts) / ?) * ?, 'unixepoch') AS ts, \
CAST(ROUND(AVG(listeners)) AS INTEGER) AS listeners, \
CAST(ROUND(AVG(listener_connections)) AS INTEGER) AS connections, \
COUNT(*) AS samples, \
SUM(CASE WHEN reachable THEN 1 ELSE 0 END) AS reachable \
FROM listener_samples \
WHERE station_id = ? AND ts >= ? AND ts <= ? \
GROUP BY (strftime('%s', ts) / ?) \
ORDER BY ts",
    )
    .bind(bucket_seconds)
    .bind(bucket_seconds)
    .bind(station_id)
    .bind(from)
    .bind(to)
    .bind(bucket_seconds)
    .fetch_all(pool)
    .await?;
    // SQLite's datetime() renders 'YYYY-MM-DD HH:MM:SS'; normalize to the
    // RFC3339 shape the rest of the API uses.
    Ok(rows
        .into_iter()
        .map(|mut p| {
            p.ts = p.ts.replace(' ', "T") + "Z";
            p
        })
        .collect())
}

/// Latest sample (for the "current listeners" stat), if any.
pub async fn latest_sample(
    pool: &SqlitePool,
    station_id: &str,
) -> Result<Option<ListenerSample>, ApiError> {
    Ok(sqlx::query_as::<_, ListenerSample>(
        "SELECT id, station_id, ts, listeners, listener_connections, reachable \
FROM listener_samples WHERE station_id = ? ORDER BY ts DESC LIMIT 1",
    )
    .bind(station_id)
    .fetch_optional(pool)
    .await?)
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsSummary {
    pub current_listeners: i64,
    pub last_sample_at: Option<String>,
    /// Approximate unique listeners in the last 24h: the delta of Icecast's
    /// cumulative connection counter over the window.
    pub unique_listeners_24h: i64,
    /// Fraction of listener samples in the last 24h where the admin API
    /// responded (0-100). NULL when no samples exist.
    pub uptime_percent_24h: Option<f64>,
    pub plays_today: i64,
    pub requests_today: i64,
}

pub async fn summary(pool: &SqlitePool, station_id: &str) -> Result<AnalyticsSummary, ApiError> {
    let latest = latest_sample(pool, station_id).await?;
    let day_ago = rfc3339_days_ago(1);
    let today = today_start();

    let unique: Option<i64> = sqlx::query_scalar(
        "SELECT MAX(listener_connections) - MIN(listener_connections) \
FROM listener_samples WHERE station_id = ? AND ts >= ?",
    )
    .bind(station_id)
    .bind(&day_ago)
    .fetch_optional(pool)
    .await?;

    let uptime: Option<f64> = sqlx::query_scalar(
        "SELECT CAST(100.0 * SUM(CASE WHEN reachable THEN 1 ELSE 0 END) / COUNT(*) AS REAL) \
FROM listener_samples WHERE station_id = ? AND ts >= ?",
    )
    .bind(station_id)
    .bind(&day_ago)
    .fetch_optional(pool)
    .await?;

    let plays_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM song_history WHERE station_id = ? AND started_at >= ?",
    )
    .bind(station_id)
    .bind(&today)
    .fetch_one(pool)
    .await?;

    let requests_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM requests WHERE station_id = ? AND created_at >= ?",
    )
    .bind(station_id)
    .bind(&today)
    .fetch_one(pool)
    .await?;

    Ok(AnalyticsSummary {
        current_listeners: latest.as_ref().map(|s| s.listeners).unwrap_or(0),
        last_sample_at: latest.map(|s| s.ts),
        unique_listeners_24h: unique.unwrap_or(0),
        uptime_percent_24h: uptime,
        plays_today,
        requests_today,
    })
}

// ---------------------------------------------------------------------------
// Top songs & request stats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TopSong {
    pub title: String,
    pub plays: i64,
    pub total_seconds: f64,
    pub last_played_at: String,
}

pub async fn top_songs(
    pool: &SqlitePool,
    station_id: &str,
    days: i64,
    limit: i64,
) -> Result<Vec<TopSong>, ApiError> {
    Ok(sqlx::query_as::<_, TopSong>(
        "SELECT title, COUNT(*) AS plays, \
COALESCE(SUM(CASE WHEN ended_at IS NOT NULL \
  THEN (julianday(ended_at) - julianday(started_at)) * 86400.0 ELSE 0.0 END), 0.0) AS total_seconds, \
MAX(started_at) AS last_played_at \
FROM song_history \
WHERE station_id = ? AND started_at >= ? AND title != '' \
GROUP BY title ORDER BY plays DESC, last_played_at DESC LIMIT ?",
    )
    .bind(station_id)
    .bind(rfc3339_days_ago(days))
    .bind(limit)
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RequestDay {
    pub day: String,
    pub total: i64,
    pub accepted: i64,
    pub rejected: i64,
    pub pending: i64,
}

/// Requests per calendar day (oldest first) for the last `days` days.
pub async fn request_stats(
    pool: &SqlitePool,
    station_id: &str,
    days: i64,
) -> Result<Vec<RequestDay>, ApiError> {
    Ok(sqlx::query_as::<_, RequestDay>(
        "SELECT substr(created_at, 1, 10) AS day, COUNT(*) AS total, \
SUM(CASE WHEN status = 'queued' THEN 1 ELSE 0 END) AS accepted, \
SUM(CASE WHEN status = 'rejected' THEN 1 ELSE 0 END) AS rejected, \
SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END) AS pending \
FROM requests WHERE station_id = ? AND created_at >= ? \
GROUP BY day ORDER BY day",
    )
    .bind(station_id)
    .bind(rfc3339_days_ago(days))
    .fetch_all(pool)
    .await?)
}

// ---------------------------------------------------------------------------
// Alerts
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Alert {
    pub id: String,
    pub station_id: Option<String>,
    pub kind: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

/// Raise an alert unless one of the same (station, kind) is already open.
/// Returns the new alert (caller decides whether to notify) or `None` when
/// deduplicated.
pub async fn raise_alert(
    pool: &SqlitePool,
    station_id: Option<&str>,
    kind: &str,
    severity: &str,
    title: &str,
    detail: &str,
) -> Result<Option<Alert>, ApiError> {
    let open: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM alerts WHERE station_id IS ? AND kind = ? AND resolved_at IS NULL LIMIT 1",
    )
    .bind(station_id)
    .bind(kind)
    .fetch_optional(pool)
    .await?;
    if open.is_some() {
        return Ok(None);
    }
    let id = uuid::Uuid::new_v4().to_string();
    let ts = now();
    sqlx::query(
        "INSERT INTO alerts (id, station_id, kind, severity, title, detail, created_at) \
VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(station_id)
    .bind(kind)
    .bind(severity)
    .bind(title)
    .bind(detail)
    .bind(&ts)
    .execute(pool)
    .await?;
    Ok(Some(Alert {
        id,
        station_id: station_id.map(str::to_string),
        kind: kind.into(),
        severity: severity.into(),
        title: title.into(),
        detail: detail.into(),
        created_at: ts,
        resolved_at: None,
    }))
}

/// Resolve a single alert by id; returns false when it does not exist or is
/// already resolved.
pub async fn resolve_alert(pool: &SqlitePool, id: &str) -> Result<bool, ApiError> {
    let affected =
        sqlx::query("UPDATE alerts SET resolved_at = ? WHERE id = ? AND resolved_at IS NULL")
            .bind(now())
            .bind(id)
            .execute(pool)
            .await?;
    Ok(affected.rows_affected() > 0)
}

/// Resolve every open alert for a (station, kind) — used when a condition
/// clears. Returns the number resolved (for notification).
pub async fn resolve_open(
    pool: &SqlitePool,
    station_id: Option<&str>,
    kind: &str,
) -> Result<usize, ApiError> {
    let affected = sqlx::query(
        "UPDATE alerts SET resolved_at = ? WHERE station_id IS ? AND kind = ? AND resolved_at IS NULL",
    )
    .bind(now())
    .bind(station_id)
    .bind(kind)
    .execute(pool)
    .await?;
    Ok(affected.rows_affected() as usize)
}

/// List alerts, newest first. `station_id` filters to one station (or to
/// global alerts when `Some` matches none — pass `None` to see every
/// alert regardless of scope).
pub async fn list_alerts(
    pool: &SqlitePool,
    station_id: Option<&str>,
    open_only: bool,
    limit: i64,
) -> Result<Vec<Alert>, ApiError> {
    let (sql, bound) = match (station_id, open_only) {
        (Some(s), true) => (
            "SELECT id, station_id, kind, severity, title, detail, created_at, resolved_at \
FROM alerts WHERE station_id = ? AND resolved_at IS NULL \
ORDER BY created_at DESC LIMIT ?",
            Some(s.to_string()),
        ),
        (Some(s), false) => (
            "SELECT id, station_id, kind, severity, title, detail, created_at, resolved_at \
FROM alerts WHERE station_id = ? \
ORDER BY created_at DESC LIMIT ?",
            Some(s.to_string()),
        ),
        (None, true) => (
            "SELECT id, station_id, kind, severity, title, detail, created_at, resolved_at \
FROM alerts WHERE resolved_at IS NULL \
ORDER BY created_at DESC LIMIT ?",
            None,
        ),
        (None, false) => (
            "SELECT id, station_id, kind, severity, title, detail, created_at, resolved_at \
FROM alerts \
ORDER BY created_at DESC LIMIT ?",
            None,
        ),
    };
    let mut q = sqlx::query_as::<_, Alert>(sql);
    if let Some(s) = &bound {
        q = q.bind(s);
    }
    Ok(q.bind(limit).fetch_all(pool).await?)
}

pub async fn get_alert(pool: &SqlitePool, id: &str) -> Result<Alert, ApiError> {
    sqlx::query_as::<_, Alert>(
        "SELECT id, station_id, kind, severity, title, detail, created_at, resolved_at \
FROM alerts WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("alert", id))
}

// ---------------------------------------------------------------------------
// Retention
// ---------------------------------------------------------------------------

/// Delete analytics rows older than `retention_days`: listener samples,
/// song history, and resolved alerts (open alerts are kept so a condition
/// is not silently forgotten). Returns rows deleted for logging.
pub async fn purge(pool: &SqlitePool, retention_days: i64) -> Result<i64, ApiError> {
    let cutoff = rfc3339_days_ago(retention_days);
    let mut total = 0i64;
    for sql in [
        "DELETE FROM listener_samples WHERE ts < ?",
        "DELETE FROM song_history WHERE started_at < ?",
        "DELETE FROM alerts WHERE resolved_at IS NOT NULL AND created_at < ?",
    ] {
        let affected = sqlx::query(sql).bind(&cutoff).execute(pool).await?;
        total += affected.rows_affected() as i64;
    }
    Ok(total)
}

// ---------------------------------------------------------------------------
// Time helpers
// ---------------------------------------------------------------------------

fn rfc3339_days_ago(days: i64) -> String {
    (OffsetDateTime::now_utc() - Duration::days(days))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

fn today_start() -> String {
    let now = OffsetDateTime::now_utc();
    let start = now.replace_time(time::Time::MIDNIGHT);
    start
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
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

    async fn seed(pool: &SqlitePool) {
        sqlx::query("CREATE TABLE stations (id TEXT PRIMARY KEY, name TEXT NOT NULL)")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO stations (id, name) VALUES ('st1', 'Test FM')")
            .execute(pool)
            .await
            .unwrap();
        let mig = std::fs::read_to_string("migrations/0009_analytics.sql").unwrap();
        sqlx::raw_sql(&mig).execute(pool).await.unwrap();
        // song_history / requests shape (full tables live in other
        // migrations; these minimal copies satisfy the queries).
        sqlx::raw_sql(
            "CREATE TABLE song_history (id INTEGER PRIMARY KEY, station_id TEXT NOT NULL, \
title TEXT NOT NULL, started_at TEXT NOT NULL, ended_at TEXT)",
        )
        .execute(pool)
        .await
        .unwrap();
        // Real requests/request_rules tables (summary() counts requests).
        let mig_requests = std::fs::read_to_string("migrations/0007_requests.sql").unwrap();
        sqlx::raw_sql(&mig_requests).execute(pool).await.unwrap();
        // Real media_files table (requests reference it).
        let mig_media = std::fs::read_to_string("migrations/0004_media.sql").unwrap();
        sqlx::raw_sql(&mig_media).execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn samples_series_and_summary() {
        let pool = test_pool().await;
        seed(&pool).await;
        let station = "st1";

        for (listeners, conn, reachable) in [(3i64, 10i64, true), (5, 12, true), (0, 12, false)] {
            insert_sample(&pool, station, listeners, conn, reachable)
                .await
                .unwrap();
        }

        let series = listener_series(
            &pool,
            station,
            "2000-01-01T00:00:00Z",
            "2999-01-01T00:00:00Z",
            60,
        )
        .await
        .unwrap();
        assert_eq!(series.len(), 1);
        // AVG(3,5,0) rounds to 3; 2 of 3 samples reachable.
        assert_eq!(series[0].listeners, 3);
        assert_eq!(series[0].samples, 3);
        assert_eq!(series[0].reachable, 2);
        assert!(series[0].ts.contains('T') && series[0].ts.ends_with('Z'));

        let summary = summary(&pool, station).await.unwrap();
        assert_eq!(summary.current_listeners, 0);
        assert_eq!(summary.unique_listeners_24h, 2);
        assert!(summary.uptime_percent_24h.unwrap() > 66.0);
    }

    #[tokio::test]
    async fn top_songs_and_request_stats() {
        let pool = test_pool().await;
        seed(&pool).await;
        let station = "st1";

        for (title, started, ended) in [
            (
                "Song A",
                "2026-08-01T10:00:00.000Z",
                Some("2026-08-01T10:03:00.000Z"),
            ),
            (
                "Song A",
                "2026-08-01T11:00:00.000Z",
                Some("2026-08-01T11:03:00.000Z"),
            ),
            ("Song B", "2026-08-01T12:00:00.000Z", None),
        ] {
            sqlx::query("INSERT INTO song_history (station_id, title, started_at, ended_at) VALUES (?, ?, ?, ?)")
                .bind(station)
                .bind(title)
                .bind(started)
                .bind(ended)
                .execute(&pool)
                .await
                .unwrap_or_else(|e| panic!("song_history insert failed: {e}"));
        }
        let top = top_songs(&pool, station, 30, 10).await.unwrap();
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].title, "Song A");
        assert_eq!(top[0].plays, 2);
        assert!(top[0].total_seconds > 359.0);

        // Requests: seed two media rows so the requests FK holds.
        sqlx::query(
            "INSERT INTO media_files (id, sha256, filename, size_bytes, storage_path, created_at, updated_at) \
VALUES ('m1', 'h1', 'a.mp3', 10, 'p1', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z'), \
('m2', 'h2', 'b.mp3', 10, 'p2', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        for (status, day) in [
            ("queued", "2026-08-01T10:00:00.000Z"),
            ("rejected", "2026-08-01T11:00:00.000Z"),
            ("pending", "2026-08-02T09:00:00.000Z"),
        ] {
            sqlx::query(
                "INSERT INTO requests (id, station_id, media_id, status, created_at, updated_at) \
VALUES (?, ?, 'm1', ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(station)
            .bind(status)
            .bind(day)
            .bind(day)
            .execute(&pool)
            .await
            .unwrap();
        }
        let stats = request_stats(&pool, station, 30).await.unwrap();
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].accepted, 1);
        assert_eq!(stats[0].rejected, 1);
        assert_eq!(stats[1].pending, 1);
    }

    #[tokio::test]
    async fn alerts_dedupe_resolve_and_purge() {
        let pool = test_pool().await;
        seed(&pool).await;
        let station = "st1";

        let first = raise_alert(
            &pool,
            Some(station),
            "dead_air",
            "error",
            "Dead air",
            "5s silence",
        )
        .await
        .unwrap();
        assert!(first.is_some());
        // Same kind+station while open is deduped.
        assert!(
            raise_alert(
                &pool,
                Some(station),
                "dead_air",
                "error",
                "Dead air",
                "again"
            )
            .await
            .unwrap()
            .is_none()
        );
        // A different kind raises independently.
        assert!(
            raise_alert(
                &pool,
                Some(station),
                "icecast_unreachable",
                "warning",
                "Icecast down",
                ""
            )
            .await
            .unwrap()
            .is_some()
        );

        let open = list_alerts(&pool, Some(station), true, 10).await.unwrap();
        assert_eq!(open.len(), 2);

        resolve_open(&pool, Some(station), "dead_air")
            .await
            .unwrap();
        assert!(
            list_alerts(&pool, Some(station), true, 10)
                .await
                .unwrap()
                .len()
                == 1
        );

        // After resolution a new dead_air episode raises again.
        assert!(
            raise_alert(&pool, Some(station), "dead_air", "error", "Dead air", "")
                .await
                .unwrap()
                .is_some()
        );

        // Purge removes old resolved alerts but keeps open ones.
        let mut old = list_alerts(&pool, None, false, 100).await.unwrap();
        old[0].created_at = "2020-01-01T00:00:00.000Z".into();
        sqlx::query("UPDATE alerts SET created_at = ?, resolved_at = ? WHERE id = ?")
            .bind(&old[0].created_at)
            .bind(now())
            .bind(&old[0].id)
            .execute(&pool)
            .await
            .unwrap();
        let _ = purge(&pool, 30).await.unwrap();
        let all = list_alerts(&pool, None, false, 100).await.unwrap();
        // The old resolved alert is purged; the recent resolved one and the
        // open one survive.
        assert_eq!(all.len(), 2);
        assert!(
            all.iter().any(|a| a.resolved_at.is_none()),
            "open alerts survive purge"
        );
    }
}
