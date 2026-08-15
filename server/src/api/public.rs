//! Public, no-auth endpoints (Phase 7): station brand + now playing +
//! history for the public page / embeddable widget, and a lightweight
//! library search powering the listener request form.

use std::path::{Component, Path as FsPath};

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::Response;
use axum::{Json, Router};
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::db::media;
use crate::db::requests;
use crate::db::song_history;
use crate::db::stations;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/public/stations/{station_id}",
            axum::routing::get(station_public),
        )
        .route(
            "/api/public/stations/{station_id}/library",
            axum::routing::get(library_search),
        )
        .route(
            "/api/public/stations/{station_id}/hls/{*file}",
            axum::routing::get(hls_file),
        )
        // AzuraCast-style public REST surface (Phase 9): the same data as
        // the per-station public endpoint, in shapes third-party clients
        // expect.
        .route("/api/now-playing", axum::routing::get(now_playing))
        .route(
            "/api/station/{station_id}/now-playing",
            axum::routing::get(station_now_playing),
        )
}

/// Every station's now-playing (AzuraCast `/api/now-playing` parity).
/// Public: used by directory sites and third-party clients.
async fn now_playing(State(state): State<AppState>) -> ApiResult<Json<Vec<PublicStation>>> {
    let stations = stations::list(&state.pool).await?;
    let mut out = Vec::with_capacity(stations.len());
    for station in stations {
        let rules = requests::ensure_rules(&state.pool, &station.id).await?;
        let now = song_history::now_playing(&state.pool, &station.id).await?;
        let history = song_history::recent(&state.pool, &station.id, 5).await?;
        let hls = hls_playlist_url(&station);
        out.push(PublicStation {
            id: station.id.clone(),
            name: station.name,
            description: station.description,
            website: station.website,
            facebook: station.facebook,
            twitter: station.twitter,
            instagram: station.instagram,
            requests_enabled: rules.enabled,
            stream_url: format!("/api/stations/{}/stream", station.id),
            hls_playlist_url: hls,
            now,
            history,
        });
    }
    Ok(Json(out))
}

/// Alias of the per-station public payload under the AzuraCast path.
async fn station_now_playing(
    State(state): State<AppState>,
    Path(station_id): Path<String>,
) -> ApiResult<Json<PublicStation>> {
    station_public_inner(&state, &station_id).await
}

async fn station_public(
    State(state): State<AppState>,
    Path(station_id): Path<String>,
) -> ApiResult<Json<PublicStation>> {
    station_public_inner(&state, &station_id).await
}

async fn station_public_inner(
    state: &AppState,
    station_id: &str,
) -> ApiResult<Json<PublicStation>> {
    let station = stations::get(&state.pool, station_id).await?;
    let rules = requests::ensure_rules(&state.pool, station_id).await?;
    let now = song_history::now_playing(&state.pool, station_id).await?;
    let history = song_history::recent(&state.pool, station_id, 15).await?;
    let hls = hls_playlist_url(&station);

    Ok(Json(PublicStation {
        id: station.id.clone(),
        name: station.name,
        description: station.description,
        website: station.website,
        facebook: station.facebook,
        twitter: station.twitter,
        instagram: station.instagram,
        requests_enabled: rules.enabled,
        stream_url: format!("/api/stations/{station_id}/stream"),
        hls_playlist_url: hls,
        now,
        history,
    }))
}

#[derive(Serialize)]
struct PublicStation {
    id: String,
    name: String,
    description: String,
    website: String,
    facebook: String,
    twitter: String,
    instagram: String,
    requests_enabled: bool,
    stream_url: String,
    /// `Some(playlist.m3u8 URL)` when the station has HLS enabled — the
    /// player prefers this over the raw Icecast mount.
    hls_playlist_url: Option<String>,
    now: Option<song_history::SongHistory>,
    history: Vec<song_history::SongHistory>,
}

fn hls_playlist_url(station: &stations::Station) -> Option<String> {
    if station.hls_enabled && !station.hls_dir.trim().is_empty() {
        Some(format!(
            "/api/public/stations/{}/hls/playlist.m3u8",
            station.id
        ))
    } else {
        None
    }
}

/// Serve an HLS playlist/segment from the station's HLS directory. The
/// engine writes `playlist.m3u8` + `seg-*.ts` there; the browser player
/// fetches them same-origin through this route. Paths are sandboxed to the
/// HLS dir (`..`/absolute/empty are rejected).
async fn hls_file(
    State(state): State<AppState>,
    Path((station_id, file)): Path<(String, String)>,
) -> ApiResult<Response<Body>> {
    let station = stations::get(&state.pool, &station_id).await?;
    if !station.hls_enabled || station.hls_dir.trim().is_empty() {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: "HLS is not enabled for this station".into(),
        });
    }
    let rel = FsPath::new(&file);
    if file.is_empty()
        || rel.is_absolute()
        || rel.components().any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "invalid HLS path".into(),
        });
    }
    let bytes = tokio::fs::read(FsPath::new(&station.hls_dir).join(rel))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "HLS file missing".into(),
        })?;
    let content_type = match FsPath::new(&file).extension().and_then(|e| e.to_str()) {
        Some("m3u8") => "application/vnd.apple.mpegurl",
        Some("ts") => "video/mp2t",
        _ => "application/octet-stream",
    };
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .header("Cache-Control", "no-cache")
        .body(Body::from(bytes))
        .expect("static response builds"))
}

#[derive(Deserialize)]
struct LibraryQuery {
    #[serde(default)]
    q: String,
}

/// Lightweight public search over the library for the request form: id +
/// display fields only, capped at 25 results.
async fn library_search(
    State(state): State<AppState>,
    Path(station_id): Path<String>,
    Query(query): Query<LibraryQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    // The station must exist (404 otherwise) even though search is global.
    let _ = stations::get(&state.pool, &station_id).await?;
    let q = query.q.trim();
    let items = if q.is_empty() {
        Vec::new()
    } else {
        let (rows, _) = media::list(
            &state.pool,
            &media::ListQuery {
                q: Some(q),
                artist: None,
                album: None,
                genre: None,
                sort: None,
                order: None,
                limit: 25,
                offset: 0,
            },
        )
        .await?;
        rows.into_iter()
            .map(|m| {
                json!({
                    "id": m.id,
                    "title": m.title,
                    "artist": m.artist,
                    "filename": m.filename,
                    "duration_seconds": m.duration_seconds,
                })
            })
            .collect::<Vec<_>>()
    };
    Ok(Json(json!({ "results": items })))
}
