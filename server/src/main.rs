mod api;
mod control;
mod db;
mod lua;
mod stations;

use std::net::SocketAddr;

use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::api::sse::SseHub;
use crate::stations::supervisor::Supervisor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("crabcast_server=info,tower_http=info")),
        )
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:crabcast.db".to_string());
    let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let data_dir =
        std::env::var("CRABCAST_DATA_DIR").unwrap_or_else(|_| "station-data".to_string());

    tracing::info!(%database_url, "connecting to database");
    let pool = db::init(&database_url).await?;

    let supervisor = Supervisor::new(data_dir, pool.clone());

    // Boot: start every station's engine (best-effort; logs failures).
    let stations = db::stations::list(&pool).await?;
    tracing::info!(count = stations.len(), "starting station engines");
    supervisor.start_all(&stations).await;

    let _hub = SseHub::new();
    let app = api::router(pool, supervisor.clone())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = bind.parse()?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "crabcast-server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("stopping station engines");
    supervisor.shutdown().await;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install ctrl-c handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
