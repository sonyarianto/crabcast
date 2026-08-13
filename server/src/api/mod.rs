pub mod error;
pub mod health;
pub mod sse;
pub mod stations;

use std::sync::Arc;

use axum::Router;
use sqlx::SqlitePool;

use crate::api::error::ApiError;
use crate::api::sse::SseHub;
use crate::stations::supervisor::Supervisor;

/// Shared app state for every route module.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub supervisor: Supervisor,
    pub hub: Arc<SseHub>,
}

pub fn router(pool: SqlitePool, supervisor: Supervisor) -> Router {
    let state = AppState {
        pool,
        supervisor,
        hub: Arc::new(SseHub::new()),
    };
    Router::new()
        .merge(health::routes())
        .merge(stations::routes())
        .with_state(state)
        .fallback(not_found)
}

async fn not_found() -> Result<axum::response::Response, ApiError> {
    Err(ApiError {
        status: axum::http::StatusCode::NOT_FOUND,
        message: "no such route".into(),
    })
}
