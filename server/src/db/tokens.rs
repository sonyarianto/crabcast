//! API tokens (Phase 9): Bearer credentials for the REST API. Only the
//! sha256 of each raw secret is stored; the raw value is returned exactly
//! once at creation. Revoking sets `revoked_at` (kept for audit history).

use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::AnyPool;
use sqlx::FromRow;

use crate::api::error::ApiError;
use crate::db::now;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ApiToken {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

/// A freshly created token: the persisted row plus the raw secret (shown
/// once, never stored).
#[derive(Debug, Serialize)]
pub struct NewToken {
    #[serde(flatten)]
    pub token: ApiToken,
    /// The plaintext secret, only ever returned by this response.
    pub secret: String,
}

/// The raw token format: a `cb_` prefix (easy to spot in logs, easy to
/// revoke by scanning) plus 32 random bytes as hex.
pub fn generate_secret() -> String {
    use argon2::password_hash::rand_core::RngCore;
    let mut bytes = [0u8; 32];
    argon2::password_hash::rand_core::OsRng.fill_bytes(&mut bytes);
    format!(
        "cb_{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

pub fn hash_secret(secret: &str) -> String {
    format!("{:x}", Sha256::digest(secret.as_bytes()))
}

pub async fn create(pool: &AnyPool, user_id: &str, name: &str) -> Result<NewToken, ApiError> {
    let secret = generate_secret();
    let id = uuid::Uuid::new_v4().to_string();
    let ts = now();
    sqlx::query(
        "INSERT INTO api_tokens (id, user_id, name, token_hash, created_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(name)
    .bind(hash_secret(&secret))
    .bind(&ts)
    .execute(pool)
    .await?;
    let token = ApiToken {
        id,
        user_id: user_id.into(),
        name: name.into(),
        created_at: ts,
        last_used_at: None,
        revoked_at: None,
    };
    Ok(NewToken { token, secret })
}

pub async fn list(pool: &AnyPool, user_id: &str) -> Result<Vec<ApiToken>, ApiError> {
    Ok(sqlx::query_as::<_, ApiToken>(
        "SELECT id, user_id, name, created_at, last_used_at, revoked_at \
FROM api_tokens WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

/// Revoke a token owned by `user_id`; returns false when not found or not
/// owned (super admins can revoke any token by passing any user_id — see
/// the caller's permission check).
pub async fn revoke(pool: &AnyPool, id: &str, user_id: &str) -> Result<bool, ApiError> {
    let affected = sqlx::query(
        "UPDATE api_tokens SET revoked_at = $1 WHERE id = $2 AND user_id = $3 AND revoked_at IS NULL",
    )
    .bind(now())
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(affected.rows_affected() > 0)
}

/// The id of the user owning a live token with this hash, if any. Touches
/// `last_used_at` on success (best-effort).
pub async fn user_id_for_token(
    pool: &AnyPool,
    token_hash: &str,
) -> Result<Option<String>, ApiError> {
    let user_id: Option<String> = sqlx::query_scalar(
        "SELECT user_id FROM api_tokens WHERE token_hash = $1 AND revoked_at IS NULL",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    if user_id.is_some() {
        let _ = sqlx::query("UPDATE api_tokens SET last_used_at = $1 WHERE token_hash = $2")
            .bind(now())
            .bind(token_hash)
            .execute(pool)
            .await;
    }
    Ok(user_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::AnyPool;
    use sqlx::any::AnyPoolOptions;

    async fn test_pool() -> AnyPool {
        sqlx::any::install_default_drivers();
        AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool")
    }

    async fn seed(pool: &AnyPool) {
        sqlx::query("CREATE TABLE users (id TEXT PRIMARY KEY, username TEXT NOT NULL)")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO users (id, username) VALUES ('u1', 'alice')")
            .execute(pool)
            .await
            .unwrap();
        let mig = std::fs::read_to_string("migrations/0010_api_tokens.sql").unwrap();
        sqlx::raw_sql(&mig).execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn secret_is_hashed_and_auth_roundtrips() {
        let pool = test_pool().await;
        seed(&pool).await;

        let new = create(&pool, "u1", "ci").await.unwrap();
        assert!(new.secret.starts_with("cb_"));
        assert_eq!(new.secret.len(), 3 + 64);
        // Only the hash is stored.
        let stored: String = sqlx::query_scalar("SELECT token_hash FROM api_tokens WHERE id = $1")
            .bind(&new.token.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored, hash_secret(&new.secret));
        assert_ne!(stored, new.secret);

        // Auth by raw secret resolves to the owner and stamps last_used_at.
        let uid = user_id_for_token(&pool, &hash_secret(&new.secret))
            .await
            .unwrap();
        assert_eq!(uid.as_deref(), Some("u1"));
        let row: Option<String> =
            sqlx::query_scalar("SELECT last_used_at FROM api_tokens WHERE id = $1")
                .bind(&new.token.id)
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert!(row.is_some() && row.unwrap() != "");

        // Wrong secret → no user.
        assert!(
            user_id_for_token(&pool, &hash_secret("cb_wrong"))
                .await
                .unwrap()
                .is_none()
        );

        // Revoked tokens stop authenticating.
        assert!(revoke(&pool, &new.token.id, "u1").await.unwrap());
        assert!(
            user_id_for_token(&pool, &hash_secret(&new.secret))
                .await
                .unwrap()
                .is_none()
        );
        // Revoking someone else's token is a no-op.
        assert!(!revoke(&pool, &new.token.id, "u2").await.unwrap());
    }

    #[test]
    fn generated_secrets_are_unique_and_prefixed() {
        let a = generate_secret();
        let b = generate_secret();
        assert_ne!(a, b);
        assert!(a.starts_with("cb_"));
        assert!(b.starts_with("cb_"));
    }
}
