//! Jingle routes (Phase 6): manage the audio files in a station's
//! `jingles_dir`. The engine re-scans the directory when the config is
//! re-rendered, so every mutation re-applies the station.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;
use tower::util::ServiceExt;
use tower_http::services::ServeFile;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::auth::{Csrf, CurrentUser};
use crate::db::stations;
use crate::db::users;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/stations/{station_id}/jingles",
            axum::routing::get(list_jingles).post(upload_jingle),
        )
        .route(
            "/api/stations/{station_id}/jingles/{filename}",
            axum::routing::get(serve_jingle).delete(delete_jingle),
        )
}

fn forbidden(msg: &str) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        message: msg.into(),
    }
}

#[derive(Serialize)]
struct JingleEntry {
    filename: String,
    size_bytes: u64,
}

async fn jingles_dir(state: &AppState, station_id: &str) -> ApiResult<std::path::PathBuf> {
    let station = stations::get(&state.pool, station_id).await?;
    if station.jingles_dir.trim().is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "station has no jingles directory configured".into(),
        });
    }
    Ok(std::path::PathBuf::from(station.jingles_dir))
}

/// Re-render + restart the station so the engine re-scans the jingle dir.
async fn reapply(state: &AppState, station_id: &str) -> ApiResult<()> {
    let station = stations::get(&state.pool, station_id).await?;
    state
        .supervisor
        .apply(&station)
        .await
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        })?;
    Ok(())
}

async fn list_jingles(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(station_id): Path<String>,
) -> ApiResult<Json<Vec<JingleEntry>>> {
    let dir = jingles_dir(&state, &station_id).await?;
    let mut entries = Vec::new();
    if let Ok(read) = std::fs::read_dir(&dir) {
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_file()
                && let (Some(name), Ok(meta)) = (path.file_name(), path.metadata())
            {
                entries.push(JingleEntry {
                    filename: name.to_string_lossy().into_owned(),
                    size_bytes: meta.len(),
                });
            }
        }
    }
    entries.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(Json(entries))
}

async fn upload_jingle(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(station_id): Path<String>,
    mut multipart: axum::extract::Multipart,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    if !user.can_manage_stations(&station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    let dir = jingles_dir(&state, &station_id).await?;
    let mut uploaded = Vec::new();
    while let Some(field) = multipart.next_field().await.map_err(|e| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("multipart error: {e}"),
    })? {
        let filename = field.file_name().unwrap_or("unnamed").to_string();
        if !safe_filename(&filename) {
            return Err(ApiError::bad_request(format!(
                "invalid filename {filename:?}"
            )));
        }
        let data = field.bytes().await.map_err(|e| ApiError {
            status: StatusCode::BAD_REQUEST,
            message: format!("upload read error: {e}"),
        })?;
        if data.is_empty() {
            return Err(ApiError::bad_request(format!("{filename} is empty")));
        }
        std::fs::create_dir_all(&dir).map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("cannot create jingles dir: {e}"),
        })?;
        std::fs::write(dir.join(&filename), &data).map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("cannot write jingle: {e}"),
        })?;
        uploaded.push(filename);
    }
    if uploaded.is_empty() {
        return Err(ApiError::bad_request("no files uploaded"));
    }
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "jingle.upload",
        "stations",
        &format!("{}: {}", station_id, uploaded.join(", ")),
    )
    .await?;
    reapply(&state, &station_id).await?;
    Ok((StatusCode::CREATED, Json(json!({ "uploaded": uploaded }))))
}

async fn serve_jingle(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path((station_id, filename)): Path<(String, String)>,
    req: axum::http::Request<axum::body::Body>,
) -> ApiResult<Response> {
    let dir = jingles_dir(&state, &station_id).await?;
    if !safe_filename(&filename) {
        return Err(ApiError::not_found("jingle", &filename));
    }
    let path = dir.join(&filename);
    if !path.is_file() {
        return Err(ApiError::not_found("jingle", &filename));
    }
    let serve = ServeFile::new(&path);
    let res = serve.oneshot(req).await.map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("serve error: {e}"),
    })?;
    Ok(res.map(axum::body::Body::new))
}

async fn delete_jingle(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path((station_id, filename)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    if !user.can_manage_stations(&station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    let dir = jingles_dir(&state, &station_id).await?;
    if !safe_filename(&filename) {
        return Err(ApiError::not_found("jingle", &filename));
    }
    let path = dir.join(&filename);
    if !path.is_file() {
        return Err(ApiError::not_found("jingle", &filename));
    }
    std::fs::remove_file(&path).map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("cannot delete jingle: {e}"),
    })?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "jingle.delete",
        "stations",
        &format!("{}: {}", station_id, filename),
    )
    .await?;
    reapply(&state, &station_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Only plain file names (no separators, no dotfiles, no path tricks).
fn safe_filename(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\'])
        && !name.starts_with('.')
}
