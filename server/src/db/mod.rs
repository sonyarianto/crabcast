use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

pub mod analytics;
pub mod media;
pub mod playlists;
pub mod requests;
pub mod song_history;
pub mod stations;
pub mod streamers;
pub mod users;

/// RFC3339 UTC timestamp matching the stations table's `strftime` format.
pub fn now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

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
