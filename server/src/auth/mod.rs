//! Session auth: argon2 passwords, tower-sessions, current-user extraction,
//! CSRF token checks, and permission helpers (Phase 2).

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sqlx::SqlitePool;
use tower_cookies::Key;
use tower_sessions::Session;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::db::users::{self, RoleGrant, User, UserRow};

/// Session key holding the logged-in user's id.
pub const SESSION_USER_KEY: &str = "user_id";
/// Session key holding the CSRF token (synchronizer token pattern).
pub const SESSION_CSRF_KEY: &str = "csrf_token";

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// The authenticated user, loaded fresh from the DB on every request so
/// deleted/disabled users are rejected immediately.
#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub user: User,
    pub grants: Vec<RoleGrant>,
}

impl CurrentUser {
    pub fn is_super_admin(&self) -> bool {
        self.user.is_super_admin
    }

    pub fn has_role(&self, role: &str, station_id: &str) -> bool {
        users::has_role(&self.grants, role, station_id)
    }

    /// Create/update/delete stations: super admin or station_manager
    /// (global or for that station).
    pub fn can_manage_stations(&self, station_id: &str) -> bool {
        self.is_super_admin() || self.has_role(users::ROLE_STATION_MANAGER, station_id)
    }

    /// Create a new station: super admin or a station_manager with a global
    /// grant (a station-scoped manager has no rights on stations they don't
    /// own yet).
    pub fn can_create_stations(&self) -> bool {
        self.is_super_admin()
            || self
                .grants
                .iter()
                .any(|g| g.role == users::ROLE_STATION_MANAGER && g.station_id.is_none())
    }

    /// Send control commands (skip, jingle, queue): station_manager or dj
    /// for that station (or global).
    pub fn can_control_station(&self, station_id: &str) -> bool {
        self.is_super_admin()
            || self.has_role(users::ROLE_STATION_MANAGER, station_id)
            || self.has_role(users::ROLE_DJ, station_id)
    }

    /// Upload/edit/delete media library files: super admin, a global
    /// station_manager, or a global media_editor. Media is a single global
    /// library in this phase (per-station scoping arrives with playlists).
    pub fn can_manage_media(&self) -> bool {
        self.is_super_admin()
            || self.grants.iter().any(|g| {
                (g.role == users::ROLE_MEDIA_EDITOR || g.role == users::ROLE_STATION_MANAGER)
                    && g.station_id.is_none()
            })
    }
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|e| ApiError {
                status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                message: format!("session error: {}", e.1),
            })?;
        let user_id: Option<String> =
            session.get(SESSION_USER_KEY).await.map_err(|e| ApiError {
                status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                message: format!("session error: {e}"),
            })?;
        let Some(user_id) = user_id else {
            return Err(unauthorized());
        };
        let Some(row) = users::get(&state.pool, &user_id).await? else {
            return Err(unauthorized());
        };
        let grants = users::grants_for(&state.pool, &user_id).await?;
        Ok(CurrentUser {
            user: row.into(),
            grants,
        })
    }
}

/// Reject unless the request carries the session's CSRF token. Mutating
/// endpoints must use this (synchronizer token pattern against cookies).
pub async fn check_csrf(session: &Session, token: &str) -> ApiResult<()> {
    let expected: Option<String> = session.get(SESSION_CSRF_KEY).await.map_err(|e| ApiError {
        status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("session error: {e}"),
    })?;
    match expected {
        Some(expected) if !expected.is_empty() && expected == token => Ok(()),
        _ => Err(ApiError {
            status: axum::http::StatusCode::FORBIDDEN,
            message: "invalid CSRF token".into(),
        }),
    }
}

/// Extractor for mutating handlers: validates `X-CSRF-Token` against the
/// session's synchronizer token. Use alongside `CurrentUser`.
pub struct Csrf;

impl FromRequestParts<AppState> for Csrf {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|e| ApiError {
                status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                message: format!("session error: {}", e.1),
            })?;
        let token = parts
            .headers
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        check_csrf(&session, token).await?;
        Ok(Csrf)
    }
}

pub fn require_super_admin(user: &CurrentUser) -> ApiResult<()> {
    if user.is_super_admin() {
        Ok(())
    } else {
        Err(ApiError {
            status: axum::http::StatusCode::FORBIDDEN,
            message: "super admin required".into(),
        })
    }
}

pub fn unauthorized() -> ApiError {
    ApiError {
        status: axum::http::StatusCode::UNAUTHORIZED,
        message: "authentication required".into(),
    }
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        User {
            id: row.id,
            username: row.username,
            display_name: row.display_name,
            is_super_admin: row.is_super_admin,
            created_at: row.created_at,
        }
    }
}

/// Session layer with an encrypted cookie. The key comes from
/// `CRABCAST_SESSION_SECRET` (any string); when unset a random key is
/// generated, logging the user out on every restart.
pub fn session_layer(
    pool: SqlitePool,
    key: Key,
) -> tower_sessions::SessionManagerLayer<
    tower_sessions_sqlx_store::SqliteStore,
    tower_sessions::service::PrivateCookie,
> {
    use tower_sessions::cookie::SameSite;
    use tower_sessions::{Expiry, SessionManagerLayer};
    use tower_sessions_sqlx_store::SqliteStore;

    SessionManagerLayer::new(SqliteStore::new(pool))
        .with_name("crabcast.session")
        .with_secure(false)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(time::Duration::days(14)))
        .with_private(key)
}

/// Cookie encryption key from `CRABCAST_SESSION_SECRET`, or a random one
/// (with a warning) when unset.
pub fn session_key() -> Key {
    match std::env::var("CRABCAST_SESSION_SECRET") {
        Ok(secret) if !secret.is_empty() => {
            let mut key = [0u8; 64];
            key[..secret.len().min(64)].copy_from_slice(secret.as_bytes());
            Key::from(&key)
        }
        _ => {
            tracing::warn!(
                "CRABCAST_SESSION_SECRET unset; using a random session key \
                 (all sessions expire on restart)"
            );
            Key::generate()
        }
    }
}

pub async fn login_session(session: &Session, user_id: &str) -> ApiResult<()> {
    session
        .insert(SESSION_USER_KEY, user_id.to_string())
        .await
        .map_err(|e| ApiError {
            status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("session error: {e}"),
        })?;
    // Rotate the CSRF token on login to prevent fixation.
    let token = new_csrf_token();
    session
        .insert(SESSION_CSRF_KEY, token)
        .await
        .map_err(|e| ApiError {
            status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("session error: {e}"),
        })?;
    Ok(())
}

/// The session's CSRF token, generating one on first use. The web app reads
/// it from `GET /api/auth/me` and echoes it in `X-CSRF-Token` on mutations.
pub async fn csrf_token(session: &Session) -> ApiResult<String> {
    let existing: Option<String> = session.get(SESSION_CSRF_KEY).await.map_err(|e| ApiError {
        status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("session error: {e}"),
    })?;
    if let Some(t) = existing.filter(|t| !t.is_empty()) {
        return Ok(t);
    }
    let token = new_csrf_token();
    session
        .insert(SESSION_CSRF_KEY, &token)
        .await
        .map_err(|e| ApiError {
            status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("session error: {e}"),
        })?;
    Ok(token)
}

fn new_csrf_token() -> String {
    use argon2::password_hash::rand_core::RngCore;
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_roundtrip() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong password", &hash));
    }

    #[test]
    fn password_hashes_are_salted() {
        let a = hash_password("same").unwrap();
        let b = hash_password("same").unwrap();
        assert_ne!(a, b);
        assert!(a.starts_with("$argon2"));
    }

    #[test]
    fn verify_rejects_garbage_hash() {
        assert!(!verify_password("anything", "not-a-valid-hash"));
    }

    fn user(grants: Vec<RoleGrant>) -> CurrentUser {
        CurrentUser {
            user: User {
                id: "u1".into(),
                username: "tester".into(),
                display_name: "Tester".into(),
                is_super_admin: false,
                created_at: "now".into(),
            },
            grants,
        }
    }

    #[test]
    fn super_admin_bypasses_role_checks() {
        let mut u = user(vec![]);
        u.user.is_super_admin = true;
        assert!(u.can_manage_stations("s1"));
        assert!(u.can_control_station("s1"));
    }

    #[test]
    fn station_manager_scope_is_respected() {
        let scoped = user(vec![RoleGrant {
            role: users::ROLE_STATION_MANAGER.into(),
            station_id: Some("s1".into()),
        }]);
        assert!(scoped.can_manage_stations("s1"));
        assert!(!scoped.can_manage_stations("s2"));
        assert!(!scoped.can_create_stations());

        let global = user(vec![RoleGrant {
            role: users::ROLE_STATION_MANAGER.into(),
            station_id: None,
        }]);
        assert!(global.can_manage_stations("s2"));
        assert!(global.can_create_stations());
    }

    #[test]
    fn dj_can_control_but_not_manage() {
        let dj = user(vec![RoleGrant {
            role: users::ROLE_DJ.into(),
            station_id: Some("s1".into()),
        }]);
        assert!(dj.can_control_station("s1"));
        assert!(!dj.can_control_station("s2"));
        assert!(!dj.can_manage_stations("s1"));
    }

    #[test]
    fn csrf_tokens_are_random() {
        assert_ne!(new_csrf_token(), new_csrf_token());
        assert_eq!(new_csrf_token().len(), 64);
    }
}
