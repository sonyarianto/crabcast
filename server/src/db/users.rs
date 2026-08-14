//! User, role and audit-log repositories (Phase 2 auth).

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::SqlitePool;

pub const ROLE_STATION_MANAGER: &str = "station_manager";
pub const ROLE_DJ: &str = "dj";
pub const ROLE_MEDIA_EDITOR: &str = "media_editor";

/// Full user row; password hash is never serialized.
#[derive(Debug, FromRow)]
pub struct UserRow {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub display_name: String,
    pub is_super_admin: bool,
    pub created_at: String,
}

const USER_COLUMNS: &str = "id, username, password_hash, display_name, is_super_admin, created_at";

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub is_super_admin: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleGrant {
    pub role: String,
    /// None = global role; Some(station id) = scoped to one station.
    pub station_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserWithRoles {
    #[serde(flatten)]
    pub user: User,
    pub roles: Vec<RoleGrant>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct AuditEntry {
    pub id: i64,
    pub user_id: Option<String>,
    pub action: String,
    pub target: String,
    pub detail: String,
    pub created_at: String,
}

pub async fn count(pool: &SqlitePool) -> sqlx::Result<i64> {
    let row = sqlx::query_scalar::<_, i64>("select count(*) from users")
        .fetch_one(pool)
        .await?;
    Ok(row)
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    username: &str,
    password_hash: &str,
    display_name: &str,
    is_super_admin: bool,
) -> sqlx::Result<User> {
    let now = crate::db::now();
    sqlx::query(
        "insert into users (id, username, password_hash, display_name, is_super_admin, created_at, updated_at)
         values (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(username)
    .bind(password_hash)
    .bind(display_name)
    .bind(is_super_admin)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(User {
        id: id.to_string(),
        username: username.to_string(),
        display_name: display_name.to_string(),
        is_super_admin,
        created_at: now,
    })
}

pub async fn get(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<UserRow>> {
    sqlx::query_as::<_, UserRow>(&format!("select {USER_COLUMNS} from users where id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn get_by_username(pool: &SqlitePool, username: &str) -> sqlx::Result<Option<UserRow>> {
    sqlx::query_as::<_, UserRow>(&format!(
        "select {USER_COLUMNS} from users where username = ?"
    ))
    .bind(username)
    .fetch_optional(pool)
    .await
}

pub async fn list(pool: &SqlitePool) -> sqlx::Result<Vec<UserWithRoles>> {
    let rows = sqlx::query_as::<_, UserRow>(&format!(
        "select {USER_COLUMNS} from users order by created_at"
    ))
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let grants = grants_for(pool, &row.id).await?;
        out.push(UserWithRoles {
            user: row.into_public(),
            roles: grants,
        });
    }
    Ok(out)
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    display_name: &str,
    is_super_admin: bool,
) -> sqlx::Result<()> {
    sqlx::query(
        "update users set display_name = ?, is_super_admin = ?, updated_at = ? where id = ?",
    )
    .bind(display_name)
    .bind(is_super_admin)
    .bind(crate::db::now())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_password(pool: &SqlitePool, id: &str, password_hash: &str) -> sqlx::Result<()> {
    sqlx::query("update users set password_hash = ?, updated_at = ? where id = ?")
        .bind(password_hash)
        .bind(crate::db::now())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<()> {
    sqlx::query("delete from users where id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

impl UserRow {
    fn into_public(self) -> User {
        User {
            id: self.id,
            username: self.username,
            display_name: self.display_name,
            is_super_admin: self.is_super_admin,
            created_at: self.created_at,
        }
    }
}

/// Replace a user's role grants; existing grants for other scopes are kept.
pub async fn set_grants(
    pool: &SqlitePool,
    user_id: &str,
    grants: &[RoleGrant],
) -> sqlx::Result<()> {
    sqlx::query("delete from user_roles where user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    for g in grants {
        sqlx::query(
            "insert into user_roles (user_id, role_id, station_id)
             select ?, id, ? from roles where name = ?",
        )
        .bind(user_id)
        .bind(&g.station_id)
        .bind(&g.role)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn grants_for(pool: &SqlitePool, user_id: &str) -> sqlx::Result<Vec<RoleGrant>> {
    sqlx::query_as::<_, (String, Option<String>)>(
        "select r.name, ur.station_id from user_roles ur
         join roles r on r.id = ur.role_id where ur.user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map(|rows| {
        rows.into_iter()
            .map(|(role, station_id)| RoleGrant { role, station_id })
            .collect()
    })
}

/// True if the user holds `role` for `station_id` (or globally).
pub fn has_role(grants: &[RoleGrant], role: &str, station_id: &str) -> bool {
    grants.iter().any(|g| {
        g.role == role && (g.station_id.is_none() || g.station_id.as_deref() == Some(station_id))
    })
}

pub async fn log_audit(
    pool: &SqlitePool,
    user_id: Option<&str>,
    action: &str,
    target: &str,
    detail: &str,
) -> sqlx::Result<()> {
    sqlx::query("insert into audit_log (user_id, action, target, detail, created_at) values (?, ?, ?, ?, ?)")
        .bind(user_id)
        .bind(action)
        .bind(target)
        .bind(detail)
        .bind(crate::db::now())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_audit(pool: &SqlitePool, limit: i64) -> sqlx::Result<Vec<AuditEntry>> {
    sqlx::query_as::<_, AuditEntry>("select * from audit_log order by id desc limit ?")
        .bind(limit)
        .fetch_all(pool)
        .await
}
