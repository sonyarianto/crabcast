//! Media library repository (Phase 3).

use serde::Serialize;
use sqlx::FromRow;
use sqlx::SqlitePool;

use crate::api::error::ApiError;

/// Full row including server-side fields.
#[derive(Debug, Clone, FromRow)]
pub struct MediaRow {
    pub id: String,
    pub sha256: String,
    pub filename: String,
    pub mime: String,
    pub size_bytes: i64,
    pub storage_path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub duration_seconds: Option<f64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub bitrate: Option<i64>,
    pub replaygain_track_gain: Option<f64>,
    pub replaygain_album_gain: Option<f64>,
    pub cover_path: Option<String>,
    pub cover_mime: Option<String>,
    pub waveform: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Public shape returned by the API. Storage paths stay server-side;
/// waveform is heavy, so lists omit it (see [`MediaFile::for_list`]).
#[derive(Debug, Clone, Serialize)]
pub struct MediaFile {
    pub id: String,
    pub sha256: String,
    pub filename: String,
    pub mime: String,
    pub size_bytes: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub duration_seconds: Option<f64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub bitrate: Option<i64>,
    pub replaygain_track_gain: Option<f64>,
    pub replaygain_album_gain: Option<f64>,
    pub has_cover: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waveform: Option<Vec<f64>>,
    pub created_at: String,
    pub updated_at: String,
}

impl MediaRow {
    pub fn into_file(self, include_waveform: bool) -> MediaFile {
        MediaFile {
            id: self.id,
            sha256: self.sha256,
            filename: self.filename,
            mime: self.mime,
            size_bytes: self.size_bytes,
            title: self.title,
            artist: self.artist,
            album: self.album,
            genre: self.genre,
            duration_seconds: self.duration_seconds,
            sample_rate: self.sample_rate,
            channels: self.channels,
            bitrate: self.bitrate,
            replaygain_track_gain: self.replaygain_track_gain,
            replaygain_album_gain: self.replaygain_album_gain,
            has_cover: self.cover_path.is_some(),
            waveform: if include_waveform {
                serde_json::from_str(&self.waveform).ok()
            } else {
                None
            },
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

const COLUMNS: &str = "id, sha256, filename, mime, size_bytes, storage_path, title, artist, \
album, genre, duration_seconds, sample_rate, channels, bitrate, replaygain_track_gain, \
replaygain_album_gain, cover_path, cover_mime, waveform, created_at, updated_at";

/// Everything `insert` needs, bundled to keep the signature at 7 args.
pub struct InsertMedia<'a> {
    pub id: &'a str,
    pub sha256: &'a str,
    pub filename: &'a str,
    pub mime: &'a str,
    pub size_bytes: i64,
    pub storage_path: &'a str,
    pub scan: &'a crate::media::ScanResult,
    pub cover_path: Option<&'a str>,
    pub cover_mime: Option<&'a str>,
}

pub async fn insert(pool: &SqlitePool, m: &InsertMedia<'_>) -> Result<MediaRow, ApiError> {
    let now = crate::db::now();
    sqlx::query(
        "INSERT INTO media_files (id, sha256, filename, mime, size_bytes, storage_path, \
title, artist, album, genre, duration_seconds, sample_rate, channels, bitrate, \
replaygain_track_gain, replaygain_album_gain, cover_path, cover_mime, waveform, created_at, updated_at) \
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(m.id)
    .bind(m.sha256)
    .bind(m.filename)
    .bind(m.mime)
    .bind(m.size_bytes)
    .bind(m.storage_path)
    .bind(&m.scan.title)
    .bind(&m.scan.artist)
    .bind(&m.scan.album)
    .bind(&m.scan.genre)
    .bind(m.scan.duration_seconds)
    .bind(m.scan.sample_rate)
    .bind(m.scan.channels)
    .bind(m.scan.bitrate)
    .bind(m.scan.replaygain_track_gain)
    .bind(m.scan.replaygain_album_gain)
    .bind(m.cover_path)
    .bind(m.cover_mime)
    .bind(serde_json::to_string(&m.scan.waveform).unwrap_or_else(|_| "[]".into()))
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    row(pool, "id", m.id)
        .await?
        .ok_or_else(|| ApiError::not_found("media file", m.id))
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<MediaFile, ApiError> {
    row(pool, "id", id)
        .await?
        .map(|r| r.into_file(true))
        .ok_or_else(|| ApiError::not_found("media file", id))
}

/// The id of the file already stored under `sha256`, if any (dedupe).
pub async fn find_by_sha256(pool: &SqlitePool, sha256: &str) -> Result<Option<String>, ApiError> {
    let id: Option<String> = sqlx::query_scalar("SELECT id FROM media_files WHERE sha256 = ?")
        .bind(sha256)
        .fetch_optional(pool)
        .await?;
    Ok(id)
}

async fn row(pool: &SqlitePool, key: &str, value: &str) -> Result<Option<MediaRow>, ApiError> {
    sqlx::query_as::<_, MediaRow>(&format!(
        "SELECT {COLUMNS} FROM media_files WHERE {key} = ?"
    ))
    .bind(value)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

/// Fetch a row including its storage paths (for file deletion).
pub async fn row_by_id(pool: &SqlitePool, id: &str) -> Result<Option<MediaRow>, ApiError> {
    row(pool, "id", id).await
}

/// List files with search + filters + sort + pagination. Returns the items
/// (waveform omitted) and the total match count.
/// List/filter/sort/pagination parameters for [`list`].
pub struct ListQuery<'a> {
    pub q: Option<&'a str>,
    pub artist: Option<&'a str>,
    pub album: Option<&'a str>,
    pub genre: Option<&'a str>,
    pub sort: Option<&'a str>,
    pub order: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

pub async fn list(pool: &SqlitePool, q: &ListQuery<'_>) -> Result<(Vec<MediaFile>, i64), ApiError> {
    let mut where_sql = String::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(q) = q.q.filter(|q| !q.trim().is_empty()) {
        let like = format!("%{}%", q.trim().replace('%', "\\%").replace('_', "\\_"));
        where_sql.push_str(
            " WHERE (title LIKE ? ESCAPE '\\' OR artist LIKE ? ESCAPE '\\' OR \
album LIKE ? ESCAPE '\\' OR filename LIKE ? ESCAPE '\\')",
        );
        for _ in 0..4 {
            binds.push(like.clone());
        }
    }
    if let Some(artist) = q.artist.filter(|a| !a.is_empty()) {
        where_sql.push_str(if where_sql.is_empty() {
            " WHERE"
        } else {
            " AND"
        });
        where_sql.push_str(" artist = ?");
        binds.push(artist.to_string());
    }
    if let Some(album) = q.album.filter(|a| !a.is_empty()) {
        where_sql.push_str(if where_sql.is_empty() {
            " WHERE"
        } else {
            " AND"
        });
        where_sql.push_str(" album = ?");
        binds.push(album.to_string());
    }
    if let Some(genre) = q.genre.filter(|g| !g.is_empty()) {
        where_sql.push_str(if where_sql.is_empty() {
            " WHERE"
        } else {
            " AND"
        });
        where_sql.push_str(" genre = ?");
        binds.push(genre.to_string());
    }

    let sort_col = match q.sort.unwrap_or("created_at") {
        "title" => "title",
        "artist" => "artist",
        "album" => "album",
        "genre" => "genre",
        "duration" => "duration_seconds",
        "size" => "size_bytes",
        _ => "created_at",
    };
    let order = if q.order == Some("asc") {
        "ASC"
    } else {
        "DESC"
    };

    let total: i64 = {
        let sql = format!("SELECT COUNT(*) FROM media_files{where_sql}");
        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        for b in &binds {
            q = q.bind(b);
        }
        q.fetch_one(pool).await?
    };

    let sql = format!(
        "SELECT {COLUMNS} FROM media_files{where_sql} ORDER BY {sort_col} {order}, id DESC LIMIT ? OFFSET ?"
    );
    let mut query = sqlx::query_as::<_, MediaRow>(&sql);
    for b in &binds {
        query = query.bind(b);
    }
    let rows = query.bind(q.limit).bind(q.offset).fetch_all(pool).await?;
    let items = rows.into_iter().map(|r| r.into_file(false)).collect();
    Ok((items, total))
}

/// Distinct artists/albums/genres for filter dropdowns.
pub async fn facets(
    pool: &SqlitePool,
) -> Result<(Vec<String>, Vec<String>, Vec<String>), ApiError> {
    let artists: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT artist FROM media_files WHERE artist != '' ORDER BY artist",
    )
    .fetch_all(pool)
    .await?;
    let albums: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT album FROM media_files WHERE album != '' ORDER BY album",
    )
    .fetch_all(pool)
    .await?;
    let genres: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT genre FROM media_files WHERE genre != '' ORDER BY genre",
    )
    .fetch_all(pool)
    .await?;
    Ok((artists, albums, genres))
}

pub async fn update_tags(
    pool: &SqlitePool,
    id: &str,
    title: &str,
    artist: &str,
    album: &str,
    genre: &str,
) -> Result<MediaFile, ApiError> {
    let affected = sqlx::query(
        "UPDATE media_files SET title = ?, artist = ?, album = ?, genre = ?, \
updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(title)
    .bind(artist)
    .bind(album)
    .bind(genre)
    .bind(id)
    .execute(pool)
    .await?;
    if affected.rows_affected() == 0 {
        return Err(ApiError::not_found("media file", id));
    }
    get(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<Option<MediaRow>, ApiError> {
    let row = row_by_id(pool, id).await?;
    if row.is_none() {
        return Ok(None);
    }
    sqlx::query("DELETE FROM media_files WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In-memory pool with migrations applied, so repository behavior is
    /// tested against the real schema.
    async fn test_pool() -> SqlitePool {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("crabcast_server=debug")
            .try_init();
        // A single connection: in-memory SQLite is per-connection, so a
        // multi-connection pool would scatter tables across databases.
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    fn scan(title: &str, artist: &str, genre: &str, dur: f64) -> crate::media::ScanResult {
        crate::media::ScanResult {
            title: title.into(),
            artist: artist.into(),
            album: format!("Album of {}", artist),
            genre: genre.into(),
            duration_seconds: Some(dur),
            sample_rate: Some(44100),
            channels: Some(2),
            bitrate: Some(128000),
            replaygain_track_gain: None,
            replaygain_album_gain: None,
            cover: None,
            waveform: vec![0.0; 8],
        }
    }

    async fn insert_sample(
        pool: &SqlitePool,
        id: &str,
        sha: &str,
        scan: &crate::media::ScanResult,
    ) {
        insert(
            pool,
            &InsertMedia {
                id,
                sha256: sha,
                filename: &format!("{}.mp3", id),
                mime: "audio/mpeg",
                size_bytes: 1234,
                storage_path: &format!("ab/{sha}.mp3"),
                scan,
                cover_path: None,
                cover_mime: None,
            },
        )
        .await
        .unwrap_or_else(|e| panic!("insert failed: {e:?}"));
    }

    #[tokio::test]
    async fn insert_and_dedupe_by_sha() {
        let pool = test_pool().await;
        let scan = scan("Song A", "Artist A", "Rock", 3.5);
        insert_sample(&pool, "m1", "sha1", &scan).await;

        // Same content hash → same id, never duplicated.
        assert_eq!(
            find_by_sha256(&pool, "sha1").await.unwrap(),
            Some("m1".into())
        );
        assert_eq!(find_by_sha256(&pool, "nope").await.unwrap(), None);

        let file = get(&pool, "m1").await.unwrap();
        assert_eq!(file.title, "Song A");
        assert_eq!(file.artist, "Artist A");
        assert_eq!(file.waveform, Some(vec![0.0; 8]));
    }

    #[tokio::test]
    async fn list_searches_filters_sorts_and_paginates() {
        let pool = test_pool().await;
        insert_sample(
            &pool,
            "m1",
            "sha1",
            &scan("Midnight Run", "Delta", "Rock", 3.0),
        )
        .await;
        insert_sample(
            &pool,
            "m2",
            "sha2",
            &scan("Morning Light", "Echo", "Jazz", 4.0),
        )
        .await;
        insert_sample(
            &pool,
            "m3",
            "sha3",
            &scan("Midnight City", "Delta", "Synth", 2.0),
        )
        .await;

        let q = |s: &'static str| ListQuery {
            q: Some(s),
            artist: None,
            album: None,
            genre: None,
            sort: None,
            order: None,
            limit: 10,
            offset: 0,
        };

        // Text search matches title (case-insensitive) across all rows.
        let (items, total) = list(&pool, &q("midnight")).await.unwrap();
        assert_eq!(total, 2);
        let mut titles: Vec<_> = items.iter().map(|f| f.title.as_str()).collect();
        titles.sort();
        assert_eq!(titles, vec!["Midnight City", "Midnight Run"]);

        // Artist filter narrows to that artist only.
        let q_artist = ListQuery {
            q: None,
            artist: Some("Delta"),
            album: None,
            genre: None,
            sort: None,
            order: None,
            limit: 10,
            offset: 0,
        };
        let (items, total) = list(&pool, &q_artist).await.unwrap();
        assert_eq!(total, 2);
        let _ = items;

        // Combined search + genre filter.
        let q_both = ListQuery {
            q: Some("midnight"),
            artist: None,
            album: None,
            genre: Some("Rock"),
            sort: None,
            order: None,
            limit: 10,
            offset: 0,
        };
        let (items, total) = list(&pool, &q_both).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(items[0].title, "Midnight Run");

        // Sort by duration, ascending.
        let q_dur = ListQuery {
            q: None,
            artist: None,
            album: None,
            genre: None,
            sort: Some("duration"),
            order: Some("asc"),
            limit: 10,
            offset: 0,
        };
        let (items, _) = list(&pool, &q_dur).await.unwrap();
        assert_eq!(
            items.iter().map(|f| f.title.as_str()).collect::<Vec<_>>(),
            vec!["Midnight City", "Midnight Run", "Morning Light"]
        );

        // Pagination slices the result set.
        let q_page = ListQuery {
            q: None,
            artist: None,
            album: None,
            genre: None,
            sort: None,
            order: None,
            limit: 1,
            offset: 1,
        };
        let (items, total) = list(&pool, &q_page).await.unwrap();
        assert_eq!(total, 3);
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn update_tags_and_delete() {
        let pool = test_pool().await;
        insert_sample(
            &pool,
            "m1",
            "sha1",
            &scan("Old Title", "Old Artist", "Rock", 3.0),
        )
        .await;

        let updated = update_tags(&pool, "m1", "New Title", "New Artist", "Album X", "Pop")
            .await
            .unwrap();
        assert_eq!(updated.title, "New Title");
        assert_eq!(updated.artist, "New Artist");

        let row = delete(&pool, "m1").await.unwrap().expect("row exists");
        assert_eq!(row.storage_path, "ab/sha1.mp3");
        assert!(delete(&pool, "m1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn facets_list_distinct_values() {
        let pool = test_pool().await;
        insert_sample(&pool, "m1", "sha1", &scan("A", "Delta", "Rock", 3.0)).await;
        insert_sample(&pool, "m2", "sha2", &scan("B", "Echo", "Jazz", 3.0)).await;
        insert_sample(&pool, "m3", "sha3", &scan("C", "Delta", "Rock", 3.0)).await;

        let (artists, albums, genres) = facets(&pool).await.unwrap();
        assert_eq!(artists, vec!["Delta", "Echo"]);
        assert_eq!(genres, vec!["Jazz", "Rock"]);
        assert_eq!(albums.len(), 2);
    }
}
