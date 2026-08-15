//! API token routes (Phase 9): users manage their own Bearer tokens. The
//! raw secret is returned exactly once at creation; listing/revoking only
//! touches the stored (hashed) rows. Super admins may revoke any token.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::Deserialize;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::auth::{Csrf, CurrentUser};
use crate::db::tokens;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/tokens", get(list_tokens).post(create_token))
        .route("/api/tokens/{id}", delete(revoke_token))
}

async fn list_tokens(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<Vec<tokens::ApiToken>>> {
    Ok(Json(tokens::list(&state.pool, &user.user.id).await?))
}

#[derive(Deserialize)]
struct CreateTokenRequest {
    #[serde(default)]
    name: String,
}

async fn create_token(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Json(req): Json<CreateTokenRequest>,
) -> ApiResult<(StatusCode, Json<tokens::NewToken>)> {
    let name = req.name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(ApiError::bad_request("token name must be 1-64 characters"));
    }
    let created = tokens::create(&state.pool, &user.user.id, name).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

async fn revoke_token(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let revoked = if user.is_super_admin() {
        // Super admins can revoke any token.
        sqlx::query("UPDATE api_tokens SET revoked_at = $1 WHERE id = $2 AND revoked_at IS NULL")
            .bind(crate::db::now())
            .bind(&id)
            .execute(&state.pool)
            .await?
            .rows_affected()
            > 0
    } else {
        tokens::revoke(&state.pool, &id, &user.user.id).await?
    };
    if !revoked {
        return Err(ApiError::not_found("token", &id));
    }
    Ok(StatusCode::NO_CONTENT)
}
