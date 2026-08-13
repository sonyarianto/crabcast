use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use serde::Serialize;

use crate::api::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/health", get(health))
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
    db: &'static str,
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    // Cheap liveness check against the DB; anything more (latency, engine
    // status) lands with later phases.
    let db_ok = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();

    let body = Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        db: if db_ok { "ok" } else { "error" },
    };

    let code = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (code, Json(body))
}
