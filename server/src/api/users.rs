//! Admin routes: user CRUD and the audit log (super admin only).

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use uuid::Uuid;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::auth::{self, CurrentUser};
use crate::db::users::{self, AuditEntry, RoleGrant, UserWithRoles};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/users", get(list_users).post(create_user))
        .route(
            "/api/users/{id}",
            get(get_user).put(update_user).delete(delete_user),
        )
        .route("/api/audit", get(list_audit))
}

async fn list_users(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<Vec<UserWithRoles>>> {
    auth::require_super_admin(&user)?;
    Ok(Json(users::list(&state.pool).await?))
}

#[derive(Deserialize)]
struct UserInput {
    username: String,
    password: Option<String>,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    is_super_admin: bool,
    #[serde(default)]
    roles: Vec<RoleGrant>,
}

async fn create_user(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<UserInput>,
) -> ApiResult<(StatusCode, Json<UserWithRoles>)> {
    auth::require_super_admin(&user)?;

    if input.username.trim().is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "username must not be empty".into(),
        });
    }
    let Some(password) = input.password.as_deref() else {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "password is required".into(),
        });
    };
    if password.len() < 8 {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "password must be at least 8 characters".into(),
        });
    }
    if users::get_by_username(&state.pool, &input.username)
        .await?
        .is_some()
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            message: "username already taken".into(),
        });
    }

    let hash = auth::hash_password(password).map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("password hashing failed: {e}"),
    })?;
    let id = Uuid::new_v4().to_string();
    let created = users::create(
        &state.pool,
        &id,
        &input.username,
        &hash,
        &input.display_name,
        input.is_super_admin,
    )
    .await?;
    users::set_grants(&state.pool, &id, &input.roles).await?;
    let roles = users::grants_for(&state.pool, &id).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "user.create",
        "users",
        &format!("{} ({id})", input.username),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(UserWithRoles {
            user: created,
            roles,
        }),
    ))
}

async fn get_user(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<UserWithRoles>> {
    auth::require_super_admin(&user)?;
    let Some(row) = users::get(&state.pool, &id).await? else {
        return Err(ApiError::not_found("user", &id));
    };
    let roles = users::grants_for(&state.pool, &id).await?;
    Ok(Json(UserWithRoles {
        user: row.into(),
        roles,
    }))
}

async fn update_user(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Json(input): Json<UserInput>,
) -> ApiResult<Json<UserWithRoles>> {
    auth::require_super_admin(&user)?;

    let Some(row) = users::get(&state.pool, &id).await? else {
        return Err(ApiError::not_found("user", &id));
    };

    // The last super admin cannot demote or delete themselves; without this
    // the system could be left with no admin at all.
    if *row.is_super_admin && id == user.user.id && !input.is_super_admin {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "cannot demote the last super admin".into(),
        });
    }

    users::update(&state.pool, &id, &input.display_name, input.is_super_admin).await?;
    if let Some(password) = input.password.as_deref() {
        if password.len() < 8 {
            return Err(ApiError {
                status: StatusCode::BAD_REQUEST,
                message: "password must be at least 8 characters".into(),
            });
        }
        let hash = auth::hash_password(password).map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("password hashing failed: {e}"),
        })?;
        users::set_password(&state.pool, &id, &hash).await?;
    }
    users::set_grants(&state.pool, &id, &input.roles).await?;

    let updated = users::get(&state.pool, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("user", &id))?;
    let roles = users::grants_for(&state.pool, &id).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "user.update",
        "users",
        &format!("{} ({id})", input.username),
    )
    .await?;

    Ok(Json(UserWithRoles {
        user: updated.into(),
        roles,
    }))
}

async fn delete_user(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    auth::require_super_admin(&user)?;
    let Some(row) = users::get(&state.pool, &id).await? else {
        return Err(ApiError::not_found("user", &id));
    };
    if *row.is_super_admin && id == user.user.id {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "cannot delete your own account".into(),
        });
    }
    users::delete(&state.pool, &id).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "user.delete",
        "users",
        &format!("{} ({id})", row.username),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct AuditQuery {
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    100
}

async fn list_audit(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<AuditQuery>,
) -> ApiResult<Json<Vec<AuditEntry>>> {
    auth::require_super_admin(&user)?;
    Ok(Json(
        users::list_audit(&state.pool, q.limit.clamp(1, 500)).await?,
    ))
}
