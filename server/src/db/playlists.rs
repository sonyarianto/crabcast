//! Playlists, playlist tracks (with fade/cue overrides) and daypart
//! schedules (Phase 4).

use serde::{Deserialize, Serialize};
use sqlx::AnyPool;
use sqlx::FromRow;

use crate::api::error::ApiError;

pub const KIND_STANDARD: &str = "standard";
pub const KIND_LOOPING: &str = "looping";
pub const KIND_SCHEDULED: &str = "scheduled";
pub const KIND_ONCE_PER_HOUR: &str = "once_per_hour";

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Playlist {
    pub id: String,
    pub station_id: String,
    pub name: String,
    pub kind: String,
    pub weight: i64,
    pub shuffle: crate::db::DbBool,
    pub enabled: crate::db::DbBool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PlaylistTrack {
    pub id: String,
    pub media_id: String,
    pub position: i64,
    pub fade_in: Option<f64>,
    pub fade_out: Option<f64>,
    pub cue_in: Option<f64>,
    pub cue_out: Option<f64>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PlaylistSchedule {
    pub id: String,
    pub days: String,
    pub start_time: String,
    pub end_time: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaylistDetail {
    #[serde(flatten)]
    pub playlist: Playlist,
    pub tracks: Vec<PlaylistTrack>,
    pub schedules: Vec<PlaylistSchedule>,
}

/// An absolute media path plus per-track fade/cue overrides.
pub type TrackSource = (String, Option<f64>, Option<f64>, Option<f64>, Option<f64>);

/// Row shape of the playlist_tracks × media_files join (position + path +
/// overrides), used by [`sources`].
type TrackRow = (
    i64,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    String,
);

/// Everything the Lua generator needs about one playlist: absolute file
/// paths (joined against the media storage root) plus per-track overrides.
#[derive(Debug, Clone)]
pub struct PlaylistSource {
    pub kind: String,
    pub shuffle: crate::db::DbBool,
    pub weight: i64,
    pub files: Vec<TrackSource>,
    pub schedules: Vec<PlaylistSchedule>,
}

const COLUMNS: &str =
    "id, station_id, name, kind, weight, shuffle, enabled, created_at, updated_at";

pub async fn list(pool: &AnyPool, station_id: &str) -> Result<Vec<Playlist>, ApiError> {
    Ok(sqlx::query_as::<_, Playlist>(&format!(
        "SELECT {COLUMNS} FROM playlists WHERE station_id = $1 ORDER BY created_at"
    ))
    .bind(station_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get(pool: &AnyPool, id: &str) -> Result<Playlist, ApiError> {
    sqlx::query_as::<_, Playlist>(&format!("SELECT {COLUMNS} FROM playlists WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("playlist", id))
}

/// Full detail: playlist + ordered tracks + schedules.
pub async fn detail(pool: &AnyPool, id: &str) -> Result<PlaylistDetail, ApiError> {
    let playlist = get(pool, id).await?;
    Ok(PlaylistDetail {
        tracks: tracks(pool, id).await?,
        schedules: schedules(pool, id).await?,
        playlist,
    })
}

pub async fn detail_for_station(
    pool: &AnyPool,
    station_id: &str,
) -> Result<Vec<PlaylistDetail>, ApiError> {
    let playlists = list(pool, station_id).await?;
    let mut out = Vec::with_capacity(playlists.len());
    for p in playlists {
        out.push(PlaylistDetail {
            tracks: tracks(pool, &p.id).await?,
            schedules: schedules(pool, &p.id).await?,
            playlist: p,
        });
    }
    Ok(out)
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaylistInput {
    pub name: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default = "default_weight")]
    pub weight: i64,
    #[serde(default)]
    pub shuffle: crate::db::DbBool,
    #[serde(default = "default_true")]
    pub enabled: crate::db::DbBool,
}

fn default_kind() -> String {
    KIND_STANDARD.into()
}
fn default_weight() -> i64 {
    1
}
fn default_true() -> crate::db::DbBool {
    crate::db::DbBool(true)
}

pub async fn create(
    pool: &AnyPool,
    station_id: &str,
    input: &PlaylistInput,
) -> Result<Playlist, ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError {
            status: axum::http::StatusCode::BAD_REQUEST,
            message: "playlist name must not be empty".into(),
        });
    }
    if !matches!(
        input.kind.as_str(),
        KIND_STANDARD | KIND_LOOPING | KIND_SCHEDULED | KIND_ONCE_PER_HOUR
    ) {
        return Err(ApiError {
            status: axum::http::StatusCode::BAD_REQUEST,
            message: format!("unknown playlist kind {:?}", input.kind),
        });
    }
    let id = uuid::Uuid::new_v4().to_string();
    let now = crate::db::now();
    sqlx::query(
        "INSERT INTO playlists (id, station_id, name, kind, weight, shuffle, enabled, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(&id)
    .bind(station_id)
    .bind(input.name.trim())
    .bind(&input.kind)
    .bind(input.weight.max(1))
    .bind(input.shuffle)
    .bind(input.enabled)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn update(pool: &AnyPool, id: &str, input: &PlaylistInput) -> Result<Playlist, ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError {
            status: axum::http::StatusCode::BAD_REQUEST,
            message: "playlist name must not be empty".into(),
        });
    }
    let affected = sqlx::query(&format!(
        "UPDATE playlists SET name = $1, kind = $2, weight = $3, shuffle = $4, enabled = $5, \
         updated_at = {} WHERE id = $6",
        crate::db::now_sql(),
    ))
    .bind(input.name.trim())
    .bind(&input.kind)
    .bind(input.weight.max(1))
    .bind(input.shuffle)
    .bind(input.enabled)
    .bind(id)
    .execute(pool)
    .await?;
    if affected.rows_affected() == 0 {
        return Err(ApiError::not_found("playlist", id));
    }
    get(pool, id).await
}

pub async fn delete(pool: &AnyPool, id: &str) -> Result<(), ApiError> {
    let affected = sqlx::query("DELETE FROM playlists WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if affected.rows_affected() == 0 {
        return Err(ApiError::not_found("playlist", id));
    }
    Ok(())
}

async fn tracks(pool: &AnyPool, playlist_id: &str) -> Result<Vec<PlaylistTrack>, ApiError> {
    Ok(sqlx::query_as::<_, PlaylistTrack>(
        "SELECT id, media_id, position, fade_in, fade_out, cue_in, cue_out \
             FROM playlist_tracks WHERE playlist_id = $1 ORDER BY position",
    )
    .bind(playlist_id)
    .fetch_all(pool)
    .await?)
}

async fn schedules(pool: &AnyPool, playlist_id: &str) -> Result<Vec<PlaylistSchedule>, ApiError> {
    Ok(sqlx::query_as::<_, PlaylistSchedule>(
        "SELECT id, days, start_time, end_time FROM playlist_schedules \
             WHERE playlist_id = $1 ORDER BY start_time",
    )
    .bind(playlist_id)
    .fetch_all(pool)
    .await?)
}

/// Append media files to a playlist, assigning positions after the current
/// tail. Returns the number added (media already in the playlist are
/// skipped — UNIQUE(playlist_id, media_id)).
pub async fn add_tracks(
    pool: &AnyPool,
    playlist_id: &str,
    media_ids: &[String],
) -> Result<usize, ApiError> {
    let tail: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), -1) FROM playlist_tracks WHERE playlist_id = $1",
    )
    .bind(playlist_id)
    .fetch_one(pool)
    .await?;
    let mut added = 0usize;
    let mut pos = tail + 1;
    for media_id in media_ids {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = $1 AND media_id = $2",
        )
        .bind(playlist_id)
        .bind(media_id)
        .fetch_one(pool)
        .await?;
        if exists > 0 {
            continue;
        }
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO playlist_tracks (id, playlist_id, media_id, position) VALUES ($1, $2, $3, $4)",
        )
        .bind(&id)
        .bind(playlist_id)
        .bind(media_id)
        .bind(pos)
        .execute(pool)
        .await?;
        pos += 1;
        added += 1;
    }
    Ok(added)
}

pub async fn remove_track(
    pool: &AnyPool,
    playlist_id: &str,
    media_id: &str,
) -> Result<(), ApiError> {
    let affected =
        sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = $1 AND media_id = $2")
            .bind(playlist_id)
            .bind(media_id)
            .execute(pool)
            .await?;
    if affected.rows_affected() == 0 {
        return Err(ApiError::not_found("playlist track", media_id));
    }
    // Renumber so positions stay dense (0, 1, 2, ...).
    renumber(pool, playlist_id).await?;
    Ok(())
}

/// Reorder tracks to match `media_ids` order (the full ordered list).
pub async fn reorder(
    pool: &AnyPool,
    playlist_id: &str,
    media_ids: &[String],
) -> Result<(), ApiError> {
    for (i, media_id) in media_ids.iter().enumerate() {
        sqlx::query(
            "UPDATE playlist_tracks SET position = $1 WHERE playlist_id = $2 AND media_id = $3",
        )
        .bind(i as i64)
        .bind(playlist_id)
        .bind(media_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn renumber(pool: &AnyPool, playlist_id: &str) -> Result<(), ApiError> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT media_id, position FROM playlist_tracks WHERE playlist_id = $1 ORDER BY position",
    )
    .bind(playlist_id)
    .fetch_all(pool)
    .await?;
    for (i, (media_id, _)) in rows.into_iter().enumerate() {
        sqlx::query(
            "UPDATE playlist_tracks SET position = $1 WHERE playlist_id = $2 AND media_id = $3",
        )
        .bind(i as i64)
        .bind(playlist_id)
        .bind(media_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TrackOverrides {
    pub fade_in: Option<f64>,
    pub fade_out: Option<f64>,
    pub cue_in: Option<f64>,
    pub cue_out: Option<f64>,
}

pub async fn update_track_overrides(
    pool: &AnyPool,
    playlist_id: &str,
    media_id: &str,
    o: &TrackOverrides,
) -> Result<(), ApiError> {
    let affected = sqlx::query(
        "UPDATE playlist_tracks SET fade_in = $1, fade_out = $2, cue_in = $3, cue_out = $4 \
         WHERE playlist_id = $5 AND media_id = $6",
    )
    .bind(o.fade_in)
    .bind(o.fade_out)
    .bind(o.cue_in)
    .bind(o.cue_out)
    .bind(playlist_id)
    .bind(media_id)
    .execute(pool)
    .await?;
    if affected.rows_affected() == 0 {
        return Err(ApiError::not_found("playlist track", media_id));
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScheduleInput {
    #[serde(default)]
    pub days: String,
    pub start_time: String,
    pub end_time: String,
}

pub async fn add_schedule(
    pool: &AnyPool,
    playlist_id: &str,
    input: &ScheduleInput,
) -> Result<PlaylistSchedule, ApiError> {
    validate_time(&input.start_time)?;
    validate_time(&input.end_time)?;
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO playlist_schedules (id, playlist_id, days, start_time, end_time)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&id)
    .bind(playlist_id)
    .bind(&input.days)
    .bind(&input.start_time)
    .bind(&input.end_time)
    .execute(pool)
    .await?;
    let row = sqlx::query_as::<_, PlaylistSchedule>(
        "SELECT id, days, start_time, end_time FROM playlist_schedules WHERE id = $1",
    )
    .bind(&id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn delete_schedule(pool: &AnyPool, id: &str) -> Result<(), ApiError> {
    let affected = sqlx::query("DELETE FROM playlist_schedules WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if affected.rows_affected() == 0 {
        return Err(ApiError::not_found("schedule", id));
    }
    Ok(())
}

fn validate_time(t: &str) -> Result<(), ApiError> {
    let ok = t.len() == 5
        && t.as_bytes()[2] == b':'
        && t[..2].parse::<u32>().map(|h| h < 24).unwrap_or(false)
        && t[3..].parse::<u32>().map(|m| m < 60).unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err(ApiError {
            status: axum::http::StatusCode::BAD_REQUEST,
            message: format!("invalid time {t:?}, use \"HH:MM\""),
        })
    }
}

/// Enabled playlists with their tracks as absolute file paths and their
/// schedules — the input to the Lua generator. `media_root` is the storage
/// root (files live at `{media_root}/{storage_path}`).
pub async fn sources(
    pool: &AnyPool,
    station_id: &str,
    media_root: &std::path::Path,
) -> Result<Vec<PlaylistSource>, ApiError> {
    let playlists = list(pool, station_id).await?;
    let mut out = Vec::new();
    for p in playlists {
        if !p.enabled {
            continue;
        }
        let rows: Vec<TrackRow> = sqlx::query_as(
            "SELECT pt.position, pt.fade_in, pt.fade_out, pt.cue_in, pt.cue_out, mf.storage_path
                 FROM playlist_tracks pt
                 JOIN media_files mf ON mf.id = pt.media_id
                 WHERE pt.playlist_id = $1
                 ORDER BY pt.position",
        )
        .bind(&p.id)
        .fetch_all(pool)
        .await?;
        if rows.is_empty() {
            continue;
        }
        let files = rows
            .into_iter()
            .map(|(_, fade_in, fade_out, cue_in, cue_out, storage_path)| {
                (
                    media_root.join(&storage_path).display().to_string(),
                    fade_in,
                    fade_out,
                    cue_in,
                    cue_out,
                )
            })
            .collect();
        out.push(PlaylistSource {
            kind: p.kind.clone(),
            shuffle: p.shuffle,
            weight: p.weight.max(1),
            files,
            schedules: schedules(pool, &p.id).await?,
        });
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> AnyPool {
        sqlx::any::install_default_drivers();
        // Single connection: in-memory SQLite is per-connection.
        let pool = sqlx::any::AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn seed_station(pool: &AnyPool) -> String {
        let id = "st1";
        sqlx::query(
            "INSERT INTO stations (id, name, description, sample_rate, channels, \
             frames_per_buffer, crossfade_seconds, fade_curve, duck_seconds, playlist_dir, \
             jingles_dir, harbor_port, harbor_mount, harbor_password, control_port, \
             control_http_port, icecast_host, icecast_port, icecast_mount, icecast_format, \
             icecast_bitrate, icecast_source_user, icecast_source_password) \
             VALUES ($1, 'Test', '', 44100, 2, 4096, 3.0, 1.0, 1.5, '', '', 8005, '/live', 'dj', \
             1234, 9234, 'localhost', 8000, '/radio', 'mp3', 128000, 'source', 'hackme')",
        )
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
        id.into()
    }

    #[tokio::test]
    async fn playlist_crud_and_detail() {
        let pool = test_pool().await;
        let station_id = seed_station(&pool).await;

        let pl = create(
            &pool,
            &station_id,
            &PlaylistInput {
                name: "Morning Mix".into(),
                kind: KIND_STANDARD.into(),
                weight: 2,
                shuffle: true.into(),
                enabled: true.into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(pl.name, "Morning Mix");
        assert_eq!(pl.kind, KIND_STANDARD);
        assert_eq!(pl.weight, 2);

        let listed = list(&pool, &station_id).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(
            detail(&pool, &pl.id).await.unwrap().playlist.name,
            "Morning Mix"
        );

        let updated = update(
            &pool,
            &pl.id,
            &PlaylistInput {
                name: "Evening Mix".into(),
                kind: KIND_LOOPING.into(),
                weight: 1,
                shuffle: false.into(),
                enabled: false.into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(updated.name, "Evening Mix");
        assert!(!updated.enabled);

        delete(&pool, &pl.id).await.unwrap();
        assert!(list(&pool, &station_id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn tracks_add_remove_reorder_and_overrides() {
        let pool = test_pool().await;
        let station_id = seed_station(&pool).await;
        let pl = create(
            &pool,
            &station_id,
            &PlaylistInput {
                name: "P".into(),
                kind: KIND_STANDARD.into(),
                weight: 1,
                shuffle: false.into(),
                enabled: true.into(),
            },
        )
        .await
        .unwrap();

        // Seed media rows.
        for (i, mid) in ["m1", "m2", "m3"].iter().enumerate() {
            sqlx::query(
                "INSERT INTO media_files (id, sha256, filename, mime, size_bytes, \
                 storage_path, title, waveform, created_at, updated_at) \
                 VALUES ($1, $2, $3, 'audio/mpeg', 1, $4, $5, '[]', '2026-01-01', '2026-01-01')",
            )
            .bind(mid)
            .bind(format!("sha{i}"))
            .bind(format!("{mid}.mp3"))
            .bind(format!("ab/sha{i}.mp3"))
            .bind(format!("Track {i}"))
            .execute(&pool)
            .await
            .unwrap();
        }

        let added = add_tracks(&pool, &pl.id, &["m1".into(), "m2".into(), "m1".into()])
            .await
            .unwrap();
        assert_eq!(added, 2, "duplicate media id skipped");

        // Append more and check dense positions.
        let added = add_tracks(&pool, &pl.id, &["m3".into()]).await.unwrap();
        assert_eq!(added, 1);
        let tracks = detail(&pool, &pl.id).await.unwrap().tracks;
        assert_eq!(tracks.len(), 3);
        assert_eq!(
            tracks
                .iter()
                .map(|t| t.media_id.as_str())
                .collect::<Vec<_>>(),
            vec!["m1", "m2", "m3"]
        );

        // Reorder m3 first.
        reorder(&pool, &pl.id, &["m3".into(), "m1".into(), "m2".into()])
            .await
            .unwrap();
        let tracks = detail(&pool, &pl.id).await.unwrap().tracks;
        assert_eq!(
            tracks
                .iter()
                .map(|t| t.media_id.as_str())
                .collect::<Vec<_>>(),
            vec!["m3", "m1", "m2"]
        );

        // Per-track overrides.
        update_track_overrides(
            &pool,
            &pl.id,
            "m1",
            &TrackOverrides {
                fade_in: Some(2.0),
                fade_out: None,
                cue_in: Some(0.5),
                cue_out: Some(180.0),
            },
        )
        .await
        .unwrap();
        let tracks = detail(&pool, &pl.id).await.unwrap().tracks;
        let m1 = tracks.iter().find(|t| t.media_id == "m1").unwrap();
        assert_eq!(m1.fade_in, Some(2.0));
        assert_eq!(m1.cue_out, Some(180.0));

        remove_track(&pool, &pl.id, "m1").await.unwrap();
        let tracks = detail(&pool, &pl.id).await.unwrap().tracks;
        assert_eq!(tracks.len(), 2);
        // Positions stay dense after removal.
        assert_eq!(
            tracks.iter().map(|t| t.position).collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[tokio::test]
    async fn schedules_validate_and_crud() {
        let pool = test_pool().await;
        let station_id = seed_station(&pool).await;
        let pl = create(
            &pool,
            &station_id,
            &PlaylistInput {
                name: "Daypart".into(),
                kind: KIND_SCHEDULED.into(),
                weight: 1,
                shuffle: false.into(),
                enabled: true.into(),
            },
        )
        .await
        .unwrap();

        let sch = add_schedule(
            &pool,
            &pl.id,
            &ScheduleInput {
                days: "mon,tue".into(),
                start_time: "09:00".into(),
                end_time: "17:00".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(sch.start_time, "09:00");

        // Bad time rejected.
        let err = add_schedule(
            &pool,
            &pl.id,
            &ScheduleInput {
                days: String::new(),
                start_time: "25:00".into(),
                end_time: "17:00".into(),
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::BAD_REQUEST);

        delete_schedule(&pool, &sch.id).await.unwrap();
        assert!(detail(&pool, &pl.id).await.unwrap().schedules.is_empty());
    }

    #[tokio::test]
    async fn sources_joins_media_paths_and_skips_disabled_or_empty() {
        let pool = test_pool().await;
        let station_id = seed_station(&pool).await;
        let pl = create(
            &pool,
            &station_id,
            &PlaylistInput {
                name: "P".into(),
                kind: KIND_STANDARD.into(),
                weight: 1,
                shuffle: false.into(),
                enabled: true.into(),
            },
        )
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO media_files (id, sha256, filename, mime, size_bytes, \
             storage_path, title, waveform, created_at, updated_at) \
             VALUES ('m1', 'sha1', 'm1.mp3', 'audio/mpeg', 1, 'ab/sha1.mp3', 'T', \
             '[]', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .unwrap();
        add_tracks(&pool, &pl.id, &["m1".into()]).await.unwrap();

        let srcs = sources(&pool, &station_id, std::path::Path::new("/media-root"))
            .await
            .unwrap();
        assert_eq!(srcs.len(), 1);
        assert_eq!(srcs[0].files.len(), 1);
        assert_eq!(srcs[0].files[0].0, "/media-root/ab/sha1.mp3");

        // Empty playlist → omitted.
        let empty = create(
            &pool,
            &station_id,
            &PlaylistInput {
                name: "Empty".into(),
                kind: KIND_STANDARD.into(),
                weight: 1,
                shuffle: false.into(),
                enabled: true.into(),
            },
        )
        .await
        .unwrap();
        let _ = empty;
        let srcs = sources(&pool, &station_id, std::path::Path::new("/media-root"))
            .await
            .unwrap();
        assert_eq!(srcs.len(), 1, "empty playlists are skipped");

        // Disabled playlist → omitted.
        update(
            &pool,
            &pl.id,
            &PlaylistInput {
                name: "P".into(),
                kind: KIND_STANDARD.into(),
                weight: 1,
                shuffle: false.into(),
                enabled: false.into(),
            },
        )
        .await
        .unwrap();
        let srcs = sources(&pool, &station_id, std::path::Path::new("/media-root"))
            .await
            .unwrap();
        assert!(srcs.is_empty(), "disabled playlists are skipped");
    }
}
