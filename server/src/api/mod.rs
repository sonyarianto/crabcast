pub mod health;

use axum::Router;
use sqlx::SqlitePool;

use crate::api::health::HealthState;

pub fn router(pool: SqlitePool) -> Router {
    Router::new()
        .merge(health::routes())
        .with_state(HealthState { pool })
}
