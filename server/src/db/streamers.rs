//! Streamer accounts (Phase 5): named live-DJ identities per station, each
//! with its own Icecast source-protocol password.

use serde::{Deserialize, Serialize};
use sqlx::AnyPool;
use sqlx::FromRow;

use crate::api::error::ApiError;
use crate::db::now;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Streamer {
    pub id: String,
    pub station_id: String,
    pub name: String,
    pub description: String,
    pub source_password: String,
    pub enabled: crate::db::DbBool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamerInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Source-protocol password the DJ connects with. Empty on update keeps
    /// the existing password.
    #[serde(default)]
    pub source_password: String,
    #[serde(default = "default_true")]
    pub enabled: crate::db::DbBool,
}

fn default_true() -> crate::db::DbBool {
    crate::db::DbBool(true)
}

pub async fn list_for_station(pool: &AnyPool, station_id: &str) -> Result<Vec<Streamer>, ApiError> {
    let rows = sqlx::query_as::<_, Streamer>(
        "SELECT id, station_id, name, description, source_password, enabled, created_at, updated_at
         FROM streamers WHERE station_id = $1 ORDER BY name",
    )
    .bind(station_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get(pool: &AnyPool, id: &str) -> Result<Streamer, ApiError> {
    let row = sqlx::query_as::<_, Streamer>(
        "SELECT id, station_id, name, description, source_password, enabled, created_at, updated_at
         FROM streamers WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("streamer", id))?;
    Ok(row)
}

/// Source passwords of enabled streamers — the `extra_passwords` for the
/// station's `input.harbor` (newer, non-empty passwords win on duplicate).
pub async fn enabled_passwords(pool: &AnyPool, station_id: &str) -> Result<Vec<String>, ApiError> {
    let enabled_true = match crate::db::kind() {
        crate::db::DbKind::Postgres => "enabled = TRUE",
        crate::db::DbKind::Sqlite => "enabled = 1",
    };
    let rows: Vec<String> = sqlx::query_scalar(&format!(
        "SELECT source_password FROM streamers\n         WHERE station_id = $1 AND {enabled_true} AND source_password != ''",
    ))
    .bind(station_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn create(
    pool: &AnyPool,
    station_id: &str,
    input: &StreamerInput,
) -> Result<Streamer, ApiError> {
    validate(input)?;
    if input.source_password.trim().is_empty() {
        return Err(ApiError::bad_request("source_password is required"));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let ts = now();
    sqlx::query(
        "INSERT INTO streamers (id, station_id, name, description, source_password, enabled, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(&id)
    .bind(station_id)
    .bind(input.name.trim())
    .bind(input.description.trim())
    .bind(input.source_password.trim())
    .bind(input.enabled)
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn update(pool: &AnyPool, id: &str, input: &StreamerInput) -> Result<Streamer, ApiError> {
    validate(input)?;
    let existing = get(pool, id).await?;
    let password = if input.source_password.trim().is_empty() {
        existing.source_password
    } else {
        input.source_password.trim().to_string()
    };
    sqlx::query(
        "UPDATE streamers SET name = $1, description = $2, source_password = $3, enabled = $4, updated_at = $5
         WHERE id = $6",
    )
    .bind(input.name.trim())
    .bind(input.description.trim())
    .bind(&password)
    .bind(input.enabled)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn delete(pool: &AnyPool, id: &str) -> Result<(), ApiError> {
    let affected = sqlx::query("DELETE FROM streamers WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if affected.rows_affected() == 0 {
        return Err(ApiError::not_found("streamer", id));
    }
    Ok(())
}

fn validate(input: &StreamerInput) -> Result<(), ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::bad_request("streamer name must not be empty"));
    }
    if !input.source_password.trim().is_empty() && input.source_password.trim().len() < 4 {
        return Err(ApiError::bad_request(
            "source_password must be at least 4 characters",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::AnyPool;
    use sqlx::any::AnyPoolOptions;

    async fn test_pool() -> AnyPool {
        sqlx::any::install_default_drivers();
        // In-memory SQLite is per-connection; pin the pool to one connection
        // so every query sees the same database.
        AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool")
    }

    async fn seed(pool: &AnyPool) -> String {
        sqlx::query("CREATE TABLE stations (id TEXT PRIMARY KEY, name TEXT NOT NULL)")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO stations (id, name) VALUES ('st1', 'Test FM')")
            .execute(pool)
            .await
            .unwrap();
        let mig = std::fs::read_to_string("migrations/0006_streamers.sql").unwrap();
        sqlx::raw_sql(&mig).execute(pool).await.unwrap();
        "st1".to_string()
    }

    #[tokio::test]
    async fn crud_and_password_rotation() {
        let pool = test_pool().await;
        let station = seed(&pool).await;

        let s = create(
            &pool,
            &station,
            &StreamerInput {
                name: "DJ Sarah".into(),
                description: "evenings".into(),
                source_password: "sarah-secret".into(),
                enabled: true.into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(s.source_password, "sarah-secret");

        assert_eq!(
            enabled_passwords(&pool, &station).await.unwrap(),
            vec!["sarah-secret".to_string()]
        );

        // Rotating the password keeps the account; old password is gone.
        update(
            &pool,
            &s.id,
            &StreamerInput {
                name: "DJ Sarah".into(),
                description: "evenings".into(),
                source_password: "sarah-new".into(),
                enabled: true.into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            enabled_passwords(&pool, &station).await.unwrap(),
            vec!["sarah-new".to_string()]
        );

        // Empty password on update keeps the existing one.
        update(
            &pool,
            &s.id,
            &StreamerInput {
                name: "DJ Sarah".into(),
                description: "evenings".into(),
                source_password: String::new(),
                enabled: false.into(),
            },
        )
        .await
        .unwrap();
        let got = get(&pool, &s.id).await.unwrap();
        assert!(!got.enabled);
        assert_eq!(got.source_password, "sarah-new");
        // Disabled streamers no longer contribute passwords.
        assert!(enabled_passwords(&pool, &station).await.unwrap().is_empty());

        delete(&pool, &s.id).await.unwrap();
        assert!(get(&pool, &s.id).await.is_err());
    }

    #[tokio::test]
    async fn rejects_blank_name_or_short_password() {
        let pool = test_pool().await;
        let station = seed(&pool).await;
        let bad = StreamerInput {
            name: "  ".into(),
            description: String::new(),
            source_password: "pw".into(),
            enabled: true.into(),
        };
        assert!(create(&pool, &station, &bad).await.is_err());
    }
}
