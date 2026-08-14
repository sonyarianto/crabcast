//! Public, no-auth endpoints (Phase 7): station brand + now playing +
//! history for the public page / embeddable widget, and a lightweight
//! library search powering the listener request form.

use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

use crate::api::AppState;
use crate::api::error::ApiResult;
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
    now: Option<song_history::SongHistory>,
    history: Vec<song_history::SongHistory>,
}

async fn station_public(
    State(state): State<AppState>,
    Path(station_id): Path<String>,
) -> ApiResult<Json<PublicStation>> {
    let station = stations::get(&state.pool, &station_id).await?;
    let rules = requests::ensure_rules(&state.pool, &station_id).await?;
    let now = song_history::now_playing(&state.pool, &station_id).await?;
    let history = song_history::recent(&state.pool, &station_id, 15).await?;

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
        now,
        history,
    }))
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
