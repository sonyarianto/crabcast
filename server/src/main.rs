use std::net::SocketAddr;
use std::sync::Arc;

use crabcast_server::analytics::poller::AnalyticsPoller;
use crabcast_server::api;
use crabcast_server::api::sse::SseHub;
use crabcast_server::auth;
use crabcast_server::db;
use crabcast_server::media;
use crabcast_server::media::LocalStorage;
use crabcast_server::stations::supervisor::Supervisor;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

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
    let media_dir = std::env::var("CRABCAST_MEDIA_DIR").unwrap_or_else(|_| "media".to_string());

    tracing::info!(%database_url, "connecting to database");
    let pool = db::init(&database_url).await?;

    let supervisor = Supervisor::new(data_dir, media_dir.clone(), pool.clone());
    let storage: Arc<dyn media::Storage> = Arc::new(LocalStorage::new(media_dir.clone().into()));

    // Boot: start every station's engine (best-effort; logs failures).
    let stations = db::stations::list(&pool).await?;
    tracing::info!(count = stations.len(), "starting station engines");
    supervisor.start_all(&stations).await;

    let _hub = SseHub::new();

    // Phase 8: background analytics — listener polling, alerts, retention.
    let poller = AnalyticsPoller::new(pool.clone(), supervisor.clone(), media_dir.clone().into());
    tokio::spawn(async move { poller.run().await });

    // Sessions use a scheme-typed store (tower-sessions needs a concrete
    // pool, so it gets its own from the same URL rather than the AnyPool).
    let session_store = match db::kind() {
        db::DbKind::Postgres => {
            auth::SessionStoreAny::Postgres(tower_sessions_sqlx_store::PostgresStore::new(
                sqlx::postgres::PgPoolOptions::new()
                    .max_connections(4)
                    .connect(&database_url)
                    .await?,
            ))
        }
        db::DbKind::Sqlite => {
            let url = if database_url.contains(':') {
                database_url.clone()
            } else {
                format!("sqlite:{database_url}")
            };
            auth::SessionStoreAny::Sqlite(tower_sessions_sqlx_store::SqliteStore::new(
                sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(4)
                    .connect(&url)
                    .await?,
            ))
        }
    };

    let app = api::router(pool.clone(), supervisor.clone(), storage)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(auth::session_layer(session_store, auth::session_key()));

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
