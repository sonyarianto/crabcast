//! Media library routes: upload (multipart, dedupe by content hash), list
//! with search/filters/sort/pagination, streaming preview, cover art, tag
//! editing (writes back to the file), and delete.

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::{CONTENT_TYPE, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tower::util::ServiceExt;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::auth::{Csrf, CurrentUser};
use crate::db::media as media_db;
use crate::db::users;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/media", get(list_media).post(upload_media))
        .route("/api/media/facets", get(media_facets))
        .route("/api/media/config", get(media_config))
        .route(
            "/api/media/{id}",
            get(get_media).put(update_media).delete(delete_media),
        )
        .route("/api/media/{id}/stream", get(stream_media))
        .route("/api/media/{id}/cover", get(cover_media))
        // Uploads are music files, not JSON; lift axum's 2 MB default.
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024 * 1024))
}

fn forbidden() -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        message: "media_editor permission required".into(),
    }
}

/// Content hash → relative storage path `{aa}/{sha}.{ext}` (shard by first
/// two hex chars so a directory never grows unbounded).
fn storage_rel(sha: &str, ext: &str) -> String {
    format!("{}/{sha}.{ext}", &sha[..2])
}

fn cover_rel(sha: &str, mime: &str) -> String {
    let ext = match mime {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "jpg",
    };
    format!("{}/{sha}.cover.{ext}", &sha[..2])
}

#[derive(Deserialize)]
struct ListQuery {
    q: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,
    sort: Option<String>,
    order: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Serialize)]
struct ListBody {
    items: Vec<media_db::MediaFile>,
    total: i64,
}

async fn list_media(
    State(state): State<AppState>,
    _user: CurrentUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<ListBody>> {
    let (items, total) = media_db::list(
        &state.pool,
        &media_db::ListQuery {
            q: q.q.as_deref(),
            artist: q.artist.as_deref(),
            album: q.album.as_deref(),
            genre: q.genre.as_deref(),
            sort: q.sort.as_deref(),
            order: q.order.as_deref(),
            limit: q.limit.clamp(1, 200),
            offset: q.offset.max(0),
        },
    )
    .await?;
    Ok(Json(ListBody { items, total }))
}

/// The storage root, so the UI can tell users which directory to point a
/// station's playlist at (uploaded files land directly under it).
#[derive(Serialize)]
struct ConfigBody {
    storage_dir: String,
}

async fn media_config(
    State(state): State<AppState>,
    _user: CurrentUser,
) -> ApiResult<Json<ConfigBody>> {
    Ok(Json(ConfigBody {
        storage_dir: state.storage.root().display().to_string(),
    }))
}

#[derive(Serialize)]
struct FacetsBody {
    artists: Vec<String>,
    albums: Vec<String>,
    genres: Vec<String>,
}

async fn media_facets(
    State(state): State<AppState>,
    _user: CurrentUser,
) -> ApiResult<Json<FacetsBody>> {
    let (artists, albums, genres) = media_db::facets(&state.pool).await?;
    Ok(Json(FacetsBody {
        artists,
        albums,
        genres,
    }))
}

async fn get_media(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<media_db::MediaFile>> {
    Ok(Json(media_db::get(&state.pool, &id).await?))
}

/// Per-file outcome of a multipart upload.
#[derive(Serialize)]
struct UploadResult {
    filename: String,
    status: &'static str, // "created" | "duplicate" | "error"
    id: Option<String>,
    message: Option<String>,
}

async fn upload_media(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    mut multipart: Multipart,
) -> ApiResult<Json<Vec<UploadResult>>> {
    if !user.can_manage_media() {
        return Err(forbidden());
    }
    let mut results = Vec::new();
    while let Some(field) = multipart.next_field().await.map_err(|e| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("multipart error: {e}"),
    })? {
        let filename = field.file_name().unwrap_or("unnamed").to_string();
        let Some(ext) = extension_of(&filename) else {
            results.push(UploadResult {
                filename,
                status: "error",
                id: None,
                message: Some("no file extension".into()),
            });
            continue;
        };
        let data = field.bytes().await.map_err(|e| ApiError {
            status: StatusCode::BAD_REQUEST,
            message: format!("upload read error: {e}"),
        })?;
        results.push(handle_upload(&state, &user, &filename, &ext, &data).await);
    }
    Ok(Json(results))
}

async fn handle_upload(
    state: &AppState,
    user: &CurrentUser,
    filename: &str,
    ext: &str,
    data: &Bytes,
) -> UploadResult {
    let sha = format!("{:x}", Sha256::digest(data));

    // Dedupe by content hash: the same bytes upload once, ever.
    match media_db::find_by_sha256(&state.pool, &sha).await {
        Ok(Some(id)) => {
            return UploadResult {
                filename: filename.to_string(),
                status: "duplicate",
                id: Some(id),
                message: None,
            };
        }
        Ok(None) => {}
        Err(e) => {
            return UploadResult {
                filename: filename.to_string(),
                status: "error",
                id: None,
                message: Some(e.to_string()),
            };
        }
    }

    let rel = storage_rel(&sha, ext);
    if let Err(e) = state.storage.write(&rel, data) {
        return UploadResult {
            filename: filename.to_string(),
            status: "error",
            id: None,
            message: Some(format!("storage write failed: {e}")),
        };
    }

    // Tag scan + waveform are CPU-heavy; keep them off the async runtime.
    let path = state.storage.abs_path(&rel);
    let name = filename.to_string();
    let scanned = tokio::task::spawn_blocking(move || crate::media::scan::scan(&path, &name))
        .await
        .ok()
        .and_then(|r| r.ok())
        .flatten();

    let Some(scan) = scanned else {
        let _ = state.storage.delete(&rel);
        return UploadResult {
            filename: filename.to_string(),
            status: "error",
            id: None,
            message: Some("not a recognized audio file".into()),
        };
    };

    let id = uuid::Uuid::new_v4().to_string();

    // Persist cover art alongside the audio so /cover can serve it.
    let (cover_path, cover_mime) = match &scan.cover {
        Some((mime, bytes)) => {
            let rel = cover_rel(&sha, mime);
            match state.storage.write(&rel, bytes) {
                Ok(_) => (Some(rel), Some(mime.clone())),
                Err(_) => (None, None),
            }
        }
        None => (None, None),
    };

    let mime = mime_for_ext(ext);
    let row = media_db::insert(
        &state.pool,
        &media_db::InsertMedia {
            id: &id,
            sha256: &sha,
            filename,
            mime,
            size_bytes: data.len() as i64,
            storage_path: &rel,
            scan: &scan,
            cover_path: cover_path.as_deref(),
            cover_mime: cover_mime.as_deref(),
        },
    )
    .await;

    match row {
        Ok(row) => {
            let _ = users::log_audit(
                &state.pool,
                Some(&user.user.id),
                "media.upload",
                "media",
                &format!("{} ({id})", row.filename),
            )
            .await;
            UploadResult {
                filename: filename.to_string(),
                status: "created",
                id: Some(id),
                message: None,
            }
        }
        Err(e) => {
            let _ = state.storage.delete(&rel);
            UploadResult {
                filename: filename.to_string(),
                status: "error",
                id: None,
                message: Some(e.to_string()),
            }
        }
    }
}

#[derive(Deserialize)]
struct TagInput {
    #[serde(default)]
    title: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    album: String,
    #[serde(default)]
    genre: String,
}

async fn update_media(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(id): Path<String>,
    Json(input): Json<TagInput>,
) -> ApiResult<Json<media_db::MediaFile>> {
    if !user.can_manage_media() {
        return Err(forbidden());
    }
    let row = media_db::row_by_id(&state.pool, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("media file", &id))?;

    // Write tags back into the file itself, then update the DB.
    let path = state.storage.abs_path(&row.storage_path);
    write_tags(
        &path,
        input.title.trim(),
        input.artist.trim(),
        input.album.trim(),
        input.genre.trim(),
    )
    .map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("tag write-back failed: {e}"),
    })?;

    let file = media_db::update_tags(
        &state.pool,
        &id,
        input.title.trim(),
        input.artist.trim(),
        input.album.trim(),
        input.genre.trim(),
    )
    .await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "media.update",
        "media",
        &format!("{} ({id})", row.filename),
    )
    .await?;
    Ok(Json(file))
}

async fn delete_media(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    if !user.can_manage_media() {
        return Err(forbidden());
    }
    let row = media_db::delete(&state.pool, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("media file", &id))?;
    let _ = state.storage.delete(&row.storage_path);
    if let Some(cover) = &row.cover_path {
        let _ = state.storage.delete(cover);
    }
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "media.delete",
        "media",
        &format!("{} ({id})", row.filename),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Stream the stored audio with Range support (seeking in the browser
/// player) via tower-http's ServeFile.
async fn stream_media(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
    req: axum::http::Request<axum::body::Body>,
) -> ApiResult<Response> {
    let row = media_db::row_by_id(&state.pool, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("media file", &id))?;
    if !state.storage.exists(&row.storage_path) {
        return Err(ApiError::not_found("media file", &id));
    }
    let path = state.storage.abs_path(&row.storage_path);
    let service = tower_http::services::ServeFile::new(path);
    let res = service.oneshot(req).await.map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("stream error: {e}"),
    })?;
    // ServeFile's response body is its own stream type; axum requires
    // `Response<Body>`, so wrap it.
    Ok(res.map(axum::body::Body::new))
}

async fn cover_media(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let row = media_db::row_by_id(&state.pool, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("media file", &id))?;
    let (Some(cover_path), Some(cover_mime)) = (row.cover_path, row.cover_mime) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: "no cover art".into(),
        });
    };
    let bytes = state.storage.read(&cover_path).map_err(|_| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "cover art missing".into(),
    })?;
    Ok((
        StatusCode::OK,
        [(
            CONTENT_TYPE,
            HeaderValue::from_str(&cover_mime)
                .unwrap_or_else(|_| HeaderValue::from_static("image/jpeg")),
        )],
        bytes,
    )
        .into_response())
}

fn extension_of(filename: &str) -> Option<String> {
    let ext = filename.rsplit_once('.')?.1.to_ascii_lowercase();
    let allowed = [
        "mp3", "flac", "ogg", "opus", "m4a", "aac", "wav", "wma", "aiff", "mp4", "m4b",
    ];
    allowed.contains(&ext.as_str()).then_some(ext)
}

fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "ogg" => "audio/ogg",
        "opus" => "audio/opus",
        "m4a" | "mp4" | "m4b" => "audio/mp4",
        "aac" => "audio/aac",
        "wav" => "audio/wav",
        "wma" => "audio/x-ms-wma",
        "aiff" => "audio/aiff",
        _ => "application/octet-stream",
    }
}

/// Set title/artist/album/genre on the file's tags (creating a sensible
/// default tag type when the file has none), then write back in place.
fn write_tags(
    path: &std::path::Path,
    title: &str,
    artist: &str,
    album: &str,
    genre: &str,
) -> anyhow::Result<()> {
    use lofty::config::WriteOptions;
    use lofty::file::{AudioFile, TaggedFileExt};
    use lofty::tag::Accessor;

    let mut tagged = lofty::read_from_path(path)?;
    if tagged.first_tag_mut().is_none() {
        tagged.insert_tag(lofty::tag::Tag::new(default_tag_type(path)));
    }
    let tag = tagged.first_tag_mut().expect("tag inserted above");
    tag.set_title(title.to_string());
    tag.set_artist(artist.to_string());
    tag.set_album(album.to_string());
    tag.set_genre(genre.to_string());
    tagged.save_to_path(path, WriteOptions::default())?;
    Ok(())
}

fn default_tag_type(path: &std::path::Path) -> lofty::tag::TagType {
    match path.extension().and_then(|e| e.to_str()) {
        Some("flac") | Some("ogg") | Some("opus") => lofty::tag::TagType::VorbisComments,
        Some("m4a") | Some("mp4") | Some("m4b") | Some("aac") => lofty::tag::TagType::Mp4Ilst,
        _ => lofty::tag::TagType::Id3v2,
    }
}
