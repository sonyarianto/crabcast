use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

/// Open the SQLite pool and run migrations at boot, before serving requests.
pub async fn init(database_url: &str) -> anyhow::Result<SqlitePool> {
    let options = database_url
        .parse::<SqliteConnectOptions>()
        .unwrap_or_else(|_| {
            // Bare paths (e.g. "crabcast.db") are valid SQLite URLs.
            format!("sqlite:{database_url}").parse().unwrap()
        })
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}
