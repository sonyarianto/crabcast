//! Backup & restore (Phase 10): one-click snapshot of the SQLite DB, media
//! library and station configs as a zip, downloadable by a super admin; and
//! restore of such an archive.
//!
//! Restore is staged (nothing is touched until the archive validates), the
//! station engines are stopped, the live files are swapped aside (kept as
//! `<name>.pre-restore-<ts>` safety copies), then the server exits with
//! code 3 so the process supervisor (systemd `Restart=on-failure`, docker
//! `restart: unless-stopped`) brings it back and the boot migrations run on
//! the restored DB.

use std::io::{Read, Seek, Write};
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::Executor;
use tokio_stream::Stream;
use tokio_util::io::ReaderStream;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::auth::{CurrentUser, require_super_admin};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/backup/download", get(download_backup))
        .route("/api/backup/restore", post(restore_backup))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024 * 1024))
}

/// Manifest written into every backup zip and validated on restore.
#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    app: String,
    schema_version: i64,
    created_at: String,
    media_files: usize,
    station_configs: usize,
}

/// Highest migration version this server knows (`0001..0011` → 11).
fn current_schema_version() -> i64 {
    sqlx::migrate!("./migrations")
        .iter()
        .map(|m| m.version)
        .max()
        .unwrap_or(0)
}

/// The SQLite file behind `DATABASE_URL` (strips `sqlite:` prefix and any
/// `?option` query string).
fn db_file_path() -> PathBuf {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "crabcast.db".into());
    db_file_path_from(&url)
}

fn db_file_path_from(url: &str) -> PathBuf {
    let path = url.strip_prefix("sqlite:").unwrap_or(url);
    PathBuf::from(path.split('?').next().unwrap_or(path))
}

fn io_err(e: std::io::Error) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("I/O error: {e}"),
    }
}

fn multipart_err(e: axum::extract::multipart::MultipartError) -> ApiError {
    ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("multipart error: {e}"),
    }
}

// --- download ---------------------------------------------------------------

async fn download_backup(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Response> {
    require_super_admin(&user)?;

    let media_dir = state.supervisor.media_root().clone();
    let configs_dir = state.supervisor.base_dir().join("configs");

    let work_dir = std::env::temp_dir().join(format!("crabcast-backup-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&work_dir).map_err(io_err)?;
    let db_copy = work_dir.join("crabcast.db");
    let zip_path = work_dir.join("backup.zip");

    // Snapshot the live DB safely (works in WAL mode) via VACUUM INTO.
    {
        let mut conn = state.pool.acquire().await?;
        let target = db_copy.display().to_string().replace('\'', "''");
        let sql = format!("VACUUM INTO '{target}'");
        Executor::execute(&mut *conn, sqlx::raw_sql(&sql))
            .await
            .map_err(|e| ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: format!("database snapshot failed: {e}"),
            })?;
    }

    build_backup(&zip_path, &db_copy, &media_dir, &configs_dir).await?;

    let file = tokio::fs::File::open(&zip_path).await.map_err(io_err)?;
    let stream = Box::pin(CleanupDir {
        inner: ReaderStream::new(file),
        dir: Some(work_dir),
    });
    let stamp = crate::db::now().replace([':', '.'], "-");
    let filename = format!("crabcast-backup-{stamp}.zip");
    let headers = [
        (CONTENT_TYPE, HeaderValue::from_static("application/zip")),
        (
            CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\"")
                .parse::<HeaderValue>()
                .expect("static disposition is valid"),
        ),
    ];
    Ok((StatusCode::OK, headers, Body::from_stream(stream)).into_response())
}

/// Zip the DB snapshot, media library and station configs (blocking; runs on
/// the blocking pool). Returns the manifest that was embedded.
async fn build_backup(
    zip_path: &Path,
    db_copy: &Path,
    media_dir: &Path,
    configs_dir: &Path,
) -> ApiResult<Manifest> {
    let zip_path = zip_path.to_path_buf();
    let db_copy = db_copy.to_path_buf();
    let media_dir = media_dir.to_path_buf();
    let configs_dir = configs_dir.to_path_buf();
    let created_at = crate::db::now();
    let schema_version = current_schema_version();

    tokio::task::spawn_blocking(move || -> Result<Manifest, String> {
        let file = std::fs::File::create(&zip_path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
        let media_files = collect_files(&media_dir);
        let config_files = collect_files(&configs_dir);
        let manifest = Manifest {
            app: "crabcast".into(),
            schema_version,
            created_at,
            media_files: media_files.len(),
            station_configs: config_files.len(),
        };

        let method = zip::CompressionMethod::Deflated;
        let opts: zip::write::FileOptions<'static, ()> =
            zip::write::FileOptions::default().compression_method(method);
        zip.start_file("manifest.json", opts)
            .map_err(|e| e.to_string())?;
        zip.write_all(&serde_json::to_vec(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        add_file(&mut zip, method, "crabcast.db", &db_copy)?;
        for f in &media_files {
            let rel = f.strip_prefix(&media_dir).map_err(|e| e.to_string())?;
            add_file(&mut zip, method, &format!("media/{}", rel.display()), f)?;
        }
        for f in &config_files {
            let rel = f.strip_prefix(&configs_dir).map_err(|e| e.to_string())?;
            add_file(
                &mut zip,
                method,
                &format!("stations/configs/{}", rel.display()),
                f,
            )?;
        }
        zip.finish().map_err(|e| e.to_string())?;
        Ok(manifest)
    })
    .await
    .map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("backup task panicked: {e}"),
    })?
    .map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: e,
    })
}

fn add_file<W: Write + Seek>(
    zip: &mut zip::ZipWriter<W>,
    method: zip::CompressionMethod,
    name: &str,
    path: &Path,
) -> Result<(), String> {
    let opts: zip::write::FileOptions<'static, ()> =
        zip::write::FileOptions::default().compression_method(method);
    zip.start_file(name, opts).map_err(|e| e.to_string())?;
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    std::io::copy(&mut f, zip).map_err(|e| e.to_string())?;
    Ok(())
}

/// All regular files under `root`, sorted by path (skips symlinks).
fn collect_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let mut entries: Vec<_> = match std::fs::read_dir(dir) {
            Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
            Err(_) => return,
        };
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    walk(root, &mut out);
    out
}

/// Body stream that removes its work directory once the download finishes
/// or is aborted (the open zip stays readable on Unix).
struct CleanupDir {
    inner: ReaderStream<tokio::fs::File>,
    dir: Option<PathBuf>,
}

impl Stream for CleanupDir {
    type Item = Result<axum::body::Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for CleanupDir {
    fn drop(&mut self) {
        if let Some(dir) = self.dir.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

// --- restore -----------------------------------------------------------------

async fn restore_backup(
    State(state): State<AppState>,
    user: CurrentUser,
    mut multipart: Multipart,
) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;

    let mut data: Option<Vec<u8>> = None;
    while let Some(field) = multipart.next_field().await.map_err(multipart_err)? {
        if field.name() == Some("file") {
            data = Some(field.bytes().await.map_err(multipart_err)?.to_vec());
            break;
        }
    }
    let data = data.ok_or_else(|| ApiError::bad_request("missing \"file\" part"))?;

    let media_dir = state.supervisor.media_root().clone();
    let base_dir = state.supervisor.base_dir().clone();
    let db_path = db_file_path();
    let current = current_schema_version();

    // Validate + extract into a staging dir; the live files are untouched if
    // anything here fails.
    let restore_id = uuid::Uuid::new_v4().to_string();
    let staging = std::env::temp_dir().join(format!("crabcast-restore-{restore_id}"));
    let stage_for_validation = staging.clone();
    tokio::task::spawn_blocking(move || stage_archive(&data, &stage_for_validation, current))
        .await
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("restore task panicked: {e}"),
        })?
        .map_err(ApiError::bad_request)?;

    // Stop every engine so no process holds files we are about to move.
    state.supervisor.shutdown().await;
    tracing::warn!("backup restore: all station engines stopped");

    let ts = crate::db::now().replace([':', '.'], "-");
    let (db_path, media_dir, base_dir, staging) = (db_path, media_dir, base_dir, staging);
    tokio::task::spawn_blocking(move || {
        swap_into_place(&staging, &db_path, &media_dir, &base_dir, &ts)
    })
    .await
    .map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("restore task panicked: {e}"),
    })??;

    // Let the response flush, then close the pool and exit so the process
    // supervisor brings the server back; boot then runs migrations on the
    // restored DB.
    let pool = state.pool.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(600)).await;
        pool.close().await;
        tracing::info!("backup restore applied; exiting for restart (code 3)");
        std::process::exit(3);
    });

    Ok(Json(serde_json::json!({
        "status": "restored",
        "restarting": true,
        "message": "Restore complete — the service is restarting with the restored data.",
    })))
}

/// Validate the archive and extract it into `staging`. Nothing outside the
/// staging dir is ever written. Errors state why the archive was rejected.
fn stage_archive(data: &[u8], staging: &Path, current_version: i64) -> Result<Manifest, String> {
    let cursor = std::io::Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("not a valid zip archive: {e}"))?;

    let manifest = {
        let mut mf = archive
            .by_name("manifest.json")
            .map_err(|_| "missing manifest.json".to_string())?;
        let mut buf = Vec::new();
        mf.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        serde_json::from_slice::<Manifest>(&buf).map_err(|e| format!("bad manifest.json: {e}"))?
    };
    if manifest.app != "crabcast" {
        return Err(format!("not a Crabcast backup (app = {:?})", manifest.app));
    }
    if manifest.schema_version > current_version {
        return Err(format!(
            "backup is schema v{} but this server only supports up to v{current_version}; upgrade the server first",
            manifest.schema_version
        ));
    }

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if name == "manifest.json" {
            continue;
        }
        let rel = archive_rel_path(&name)
            .ok_or_else(|| format!("unsafe or unexpected path in archive: {name:?}"))?;
        let dest = staging.join(&rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if entry.is_dir() {
            continue;
        }
        if name == "crabcast.db" {
            // Validate the SQLite header from the buffer (ZipFile is not
            // seekable, so read it whole) and write the complete file.
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            if buf.len() < 16 || &buf[..16] != b"SQLite format 3\0" {
                return Err("crabcast.db is not a SQLite database".into());
            }
            std::fs::write(&dest, &buf).map_err(|e| e.to_string())?;
            continue;
        }
        let mut out = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }
    Ok(manifest)
}

/// Map an archive entry name to a safe relative path under the staging dir.
/// Only `crabcast.db`, `media/*` and `stations/configs/*` are accepted; any
/// `..`, absolute or otherwise suspicious component is rejected.
fn archive_rel_path(name: &str) -> Option<PathBuf> {
    if Path::new(name)
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    if name == "crabcast.db" {
        return Some(PathBuf::from("crabcast.db"));
    }
    if let Some(rest) = name.strip_prefix("media/") {
        return Some(PathBuf::from("media").join(rest));
    }
    if let Some(rest) = name.strip_prefix("stations/configs/") {
        return Some(PathBuf::from("configs").join(rest));
    }
    None
}

/// Move the staged files into place, renaming whatever was there to
/// `<name>.pre-restore-<ts>` safety copies. Never deletes user data.
fn swap_into_place(
    staging: &Path,
    db_path: &Path,
    media_dir: &Path,
    base_dir: &Path,
    ts: &str,
) -> Result<(), ApiError> {
    let move_aside = |src: &Path| -> Result<(), ApiError> {
        if !src.exists() {
            return Ok(());
        }
        let file_name = src.file_name().and_then(|n| n.to_str()).unwrap_or("data");
        let aside = src.with_file_name(format!("{file_name}.pre-restore-{ts}"));
        if aside.exists() {
            let _ = std::fs::remove_dir_all(&aside).or_else(|_| std::fs::remove_file(&aside));
        }
        move_path(src, &aside).map_err(io_err)
    };

    // SQLite DB.
    move_aside(db_path)?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(io_err)?;
    }
    move_path(&staging.join("crabcast.db"), db_path).map_err(io_err)?;

    // Media root.
    move_aside(media_dir)?;
    let staged_media = staging.join("media");
    if staged_media.exists() {
        move_path(&staged_media, media_dir).map_err(io_err)?;
    } else {
        std::fs::create_dir_all(media_dir).map_err(io_err)?;
    }

    // Station configs (logs are regenerated by the supervisor).
    let configs_dir = base_dir.join("configs");
    move_aside(&configs_dir)?;
    let staged_configs = staging.join("configs");
    if staged_configs.exists() {
        move_path(&staged_configs, &configs_dir).map_err(io_err)?;
    } else {
        std::fs::create_dir_all(&configs_dir).map_err(io_err)?;
    }

    let _ = std::fs::remove_dir_all(staging);
    Ok(())
}

/// Rename, falling back to copy+remove when the source and destination are
/// on different filesystems (e.g. /tmp vs a media volume).
fn move_path(src: &Path, dst: &Path) -> std::io::Result<()> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
            copy_recursive(src, dst)?;
            if src.is_dir() {
                std::fs::remove_dir_all(src)
            } else {
                std::fs::remove_file(src)
            }
        }
        Err(e) => Err(e),
    }
}

fn copy_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            copy_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(src, dst).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_path_parsing() {
        assert_eq!(
            db_file_path_from("sqlite:/data/crabcast.db"),
            PathBuf::from("/data/crabcast.db")
        );
        assert_eq!(
            db_file_path_from("crabcast.db"),
            PathBuf::from("crabcast.db")
        );
        assert_eq!(
            db_file_path_from("sqlite:crabcast.db?mode=ro"),
            PathBuf::from("crabcast.db")
        );
    }

    #[test]
    fn archive_paths_are_sandboxed() {
        assert_eq!(
            archive_rel_path("crabcast.db"),
            Some(PathBuf::from("crabcast.db"))
        );
        assert_eq!(
            archive_rel_path("media/ab/abc.mp3"),
            Some(PathBuf::from("media/ab/abc.mp3"))
        );
        assert_eq!(
            archive_rel_path("stations/configs/s1/crabsoup.lua"),
            Some(PathBuf::from("configs/s1/crabsoup.lua"))
        );
        assert_eq!(archive_rel_path("manifest.json"), None);
        assert_eq!(archive_rel_path("media/../etc/passwd"), None);
        assert_eq!(archive_rel_path("../../etc/passwd"), None);
        assert_eq!(archive_rel_path("/etc/passwd"), None);
        assert_eq!(archive_rel_path("etc/passwd"), None);
    }
    fn make_zip(manifest: &Manifest, db_bytes: &[u8], media: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opts: zip::write::FileOptions<'static, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zw.start_file("manifest.json", opts).unwrap();
        zw.write_all(&serde_json::to_vec(manifest).unwrap())
            .unwrap();
        zw.start_file("crabcast.db", opts).unwrap();
        zw.write_all(db_bytes).unwrap();
        for (name, bytes) in media {
            zw.start_file(*name, opts).unwrap();
            zw.write_all(bytes).unwrap();
        }
        zw.finish().unwrap();
        buf
    }

    fn manifest(schema_version: i64) -> Manifest {
        Manifest {
            app: "crabcast".into(),
            schema_version,
            created_at: "now".into(),
            media_files: 1,
            station_configs: 0,
        }
    }

    #[test]
    fn stage_archive_extracts_and_rejects_bad_archives() {
        let staging =
            std::env::temp_dir().join(format!("cb-restore-test-{}", uuid::Uuid::new_v4()));

        // Valid roundtrip: real SQLite header + a media file.
        let zip = make_zip(
            &manifest(3),
            b"SQLite format 3\0rest of db",
            &[("media/ab/abc.mp3", b"fake audio")],
        );
        let parsed = stage_archive(&zip, &staging, 5).unwrap();
        assert_eq!(parsed.schema_version, 3);
        assert!(staging.join("crabcast.db").exists());
        assert!(staging.join("media/ab/abc.mp3").exists());
        std::fs::remove_dir_all(&staging).unwrap();

        // crabcast.db with a non-SQLite header is rejected.
        let bad_db = make_zip(&manifest(1), b"not sqlite at all", &[]);
        assert!(stage_archive(&bad_db, &staging, 5).is_err());

        // A backup from a newer server is rejected, not silently applied.
        let too_new = make_zip(&manifest(9), b"SQLite format 3\0x", &[]);
        assert!(stage_archive(&too_new, &staging, 5).is_err());

        // A path traversal entry is rejected.
        let mut buf = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'static, ()> = zip::write::FileOptions::default();
            zw.start_file("manifest.json", opts).unwrap();
            zw.write_all(&serde_json::to_vec(&manifest(1)).unwrap())
                .unwrap();
            zw.start_file("../evil", opts).unwrap();
            zw.write_all(b"x").unwrap();
            zw.finish().unwrap();
        }
        assert!(stage_archive(&buf, &staging, 5).is_err());
    }
}
