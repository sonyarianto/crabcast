pub mod analytics;
pub mod auth;
pub mod backup;
pub mod error;
pub mod health;
pub mod jingles;
pub mod media;
pub mod playlists;
pub mod public;
pub mod requests;
pub mod sse;
pub mod stations;
pub mod streamers;
pub mod tokens;
pub mod users;

use std::sync::Arc;

use axum::Router;
use sqlx::SqlitePool;

use crate::api::error::ApiError;
use crate::api::sse::SseHub;
use crate::media::Storage;
use crate::stations::supervisor::Supervisor;

/// Shared app state for every route module.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub supervisor: Supervisor,
    pub hub: Arc<SseHub>,
    pub storage: Arc<dyn Storage>,
}

pub fn router(pool: SqlitePool, supervisor: Supervisor, storage: Arc<dyn Storage>) -> Router {
    let state = AppState {
        pool,
        supervisor,
        hub: Arc::new(SseHub::new()),
        storage,
    };
    Router::new()
        .merge(health::routes())
        .merge(backup::routes())
        .merge(analytics::routes())
        .merge(auth::routes())
        .merge(users::routes())
        .merge(stations::routes())
        .merge(media::routes())
        .merge(playlists::routes())
        .merge(streamers::routes())
        .merge(tokens::routes())
        .merge(requests::routes())
        .merge(jingles::routes())
        .merge(public::routes())
        .with_state(state)
        .fallback(not_found)
}

async fn not_found() -> Result<axum::response::Response, ApiError> {
    Err(ApiError {
        status: axum::http::StatusCode::NOT_FOUND,
        message: "no such route".into(),
    })
}
