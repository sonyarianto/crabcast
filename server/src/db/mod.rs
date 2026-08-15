use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use sqlx::any::{AnyPoolOptions, AnyTypeInfo, AnyValueRef};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::{AnyPool, Decode, Encode, Type};

pub mod analytics;
pub mod media;
pub mod notification_webhooks;
pub mod playlists;
pub mod podcasts;
pub mod requests;
pub mod song_history;
pub mod stations;
pub mod streamers;
pub mod tokens;
pub mod users;

/// Which database backend this process is running against (one per
/// process). Queries that use dialect-specific SQL consult this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbKind {
    Sqlite,
    Postgres,
}

static KIND: OnceLock<DbKind> = OnceLock::new();

/// The backend kind, set by [`init`] from the `DATABASE_URL` scheme.
pub fn kind() -> DbKind {
    *KIND.get().unwrap_or(&DbKind::Sqlite)
}

fn detect_kind(database_url: &str) -> DbKind {
    if database_url.starts_with("postgres:")
        || database_url.starts_with("postgresql:")
        || database_url.starts_with("pg:")
    {
        DbKind::Postgres
    } else {
        DbKind::Sqlite
    }
}

/// RFC3339 UTC timestamp matching the stations table's `strftime` format.
pub fn now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

/// Dialect-neutral SQL expression for the current timestamp, producing the
/// shared RFC3339 shape (`YYYY-MM-DDTHH:MM:SS.mmmZ`, 3-digit fraction) that
/// the TEXT timestamp columns use on both backends. Postgres gets the same
/// shape via `to_char` so stored values compare identically.
pub fn now_sql() -> &'static str {
    match kind() {
        DbKind::Postgres => "to_char(now(), 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')",
        DbKind::Sqlite => "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    }
}

/// Boolean that decodes from both native `BOOLEAN` columns (Postgres) and
/// SQLite's `INTEGER` 0/1 storage. The `Any` driver is strict about column
/// kinds, so a plain `bool` fails on SQLite; this wrapper accepts both and
/// serializes exactly like a bool.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DbBool(pub bool);

impl From<bool> for DbBool {
    fn from(b: bool) -> Self {
        DbBool(b)
    }
}

impl From<DbBool> for bool {
    fn from(b: DbBool) -> Self {
        b.0
    }
}

impl std::ops::Deref for DbBool {
    type Target = bool;

    fn deref(&self) -> &bool {
        &self.0
    }
}

impl std::fmt::Display for DbBool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::ops::Not for DbBool {
    type Output = bool;

    fn not(self) -> bool {
        !self.0
    }
}

impl PartialEq<bool> for DbBool {
    fn eq(&self, other: &bool) -> bool {
        self.0 == *other
    }
}

impl PartialEq<DbBool> for bool {
    fn eq(&self, other: &DbBool) -> bool {
        *self == other.0
    }
}

impl Type<sqlx::any::Any> for DbBool {
    fn type_info() -> AnyTypeInfo {
        <bool as Type<sqlx::any::Any>>::type_info()
    }

    fn compatible(ty: &AnyTypeInfo) -> bool {
        matches!(
            ty.kind(),
            sqlx::any::AnyTypeInfoKind::Bool
                | sqlx::any::AnyTypeInfoKind::SmallInt
                | sqlx::any::AnyTypeInfoKind::Integer
                | sqlx::any::AnyTypeInfoKind::BigInt
        )
    }
}

impl<'q> Encode<'q, sqlx::any::Any> for DbBool {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::any::Any as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<IsNull, BoxDynError> {
        <bool as Encode<'q, sqlx::any::Any>>::encode_by_ref(&self.0, buf)
    }
}

impl<'r> Decode<'r, sqlx::any::Any> for DbBool {
    fn decode(value: AnyValueRef<'r>) -> Result<Self, BoxDynError> {
        if let Ok(b) = <bool as Decode<'r, sqlx::any::Any>>::decode(value.clone()) {
            return Ok(DbBool(b));
        }
        let n = <i64 as Decode<'r, sqlx::any::Any>>::decode(value)?;
        Ok(DbBool(n != 0))
    }
}

static SQLITE_MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
static POSTGRES_MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations-pg");

/// Open the database pool (SQLite or Postgres, from the `DATABASE_URL`
/// scheme) and run the matching migrations at boot, before serving
/// requests.
pub async fn init(database_url: &str) -> anyhow::Result<AnyPool> {
    sqlx::any::install_default_drivers();

    let kind = detect_kind(database_url);
    let _ = KIND.set(kind);

    // Bare paths (e.g. "crabcast.db") are valid SQLite URLs.
    let url = if kind == DbKind::Sqlite && !database_url.contains(':') {
        format!("sqlite:{database_url}")
    } else {
        database_url.to_string()
    };

    let pool = AnyPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await?;

    match kind {
        DbKind::Sqlite => SQLITE_MIGRATIONS.run(&pool).await?,
        DbKind::Postgres => POSTGRES_MIGRATIONS.run(&pool).await?,
    }

    Ok(pool)
}
