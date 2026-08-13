//! Auth routes: first-run bootstrap, login/logout, session info, password.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde::Serialize;
use tower_sessions::Session;
use uuid::Uuid;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::auth::{self, CurrentUser};
use crate::db::users::{self, User};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/setup", get(setup_status))
        .route("/api/auth/bootstrap", post(bootstrap))
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/me", get(me))
        .route("/api/auth/password", post(change_password))
}

/// First-run detection: `{ "needed": true }` while the users table is empty.
async fn setup_status(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let needed = users::count(&state.pool).await? == 0;
    Ok(Json(serde_json::json!({ "needed": needed })))
}

#[derive(Deserialize)]
struct BootstrapRequest {
    username: String,
    password: String,
    #[serde(default)]
    display_name: String,
}

/// First-run setup: create the initial super admin. Only allowed while the
/// users table is empty (email-less bootstrap).
async fn bootstrap(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<BootstrapRequest>,
) -> ApiResult<(StatusCode, Json<AuthResponse>)> {
    if users::count(&state.pool).await? > 0 {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            message: "users already exist; bootstrap is closed".into(),
        });
    }
    validate_credentials(&req.username, &req.password)?;

    let password_hash = auth::hash_password(&req.password).map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("password hashing failed: {e}"),
    })?;
    let user = users::create(
        &state.pool,
        &Uuid::new_v4().to_string(),
        &req.username,
        &password_hash,
        &req.display_name,
        true,
    )
    .await?;

    auth::login_session(&session, &user.id).await?;
    users::log_audit(
        &state.pool,
        Some(&user.id),
        "bootstrap",
        "users",
        &user.username,
    )
    .await?;

    let token = auth::csrf_token(&session).await?;
    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            user,
            csrf_token: token,
        }),
    ))
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct AuthResponse {
    user: User,
    csrf_token: String,
}

async fn login(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let Some(row) = users::get_by_username(&state.pool, &req.username).await? else {
        return Err(bad_login());
    };
    if !auth::verify_password(&req.password, &row.password_hash) {
        return Err(bad_login());
    }
    auth::login_session(&session, &row.id).await?;
    users::log_audit(&state.pool, Some(&row.id), "login", "users", &row.username).await?;

    let token = auth::csrf_token(&session).await?;
    Ok(Json(AuthResponse {
        user: row.into(),
        csrf_token: token,
    }))
}

async fn logout(State(state): State<AppState>, session: Session) -> ApiResult<StatusCode> {
    if let Ok(Some(user_id)) = session.get::<String>(auth::SESSION_USER_KEY).await {
        users::log_audit(&state.pool, Some(&user_id), "logout", "users", "").await?;
    }
    session.delete().await.map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("session error: {e}"),
    })?;
    Ok(StatusCode::NO_CONTENT)
}

/// Current session: user, role grants, CSRF token.
#[derive(Serialize)]
struct MeResponse {
    user: User,
    roles: Vec<users::RoleGrant>,
    csrf_token: String,
}

async fn me(
    State(state): State<AppState>,
    session: Session,
    user: CurrentUser,
) -> ApiResult<Json<MeResponse>> {
    let grants = users::grants_for(&state.pool, &user.user.id).await?;
    let token = auth::csrf_token(&session).await?;
    Ok(Json(MeResponse {
        user: user.user,
        roles: grants,
        csrf_token: token,
    }))
}

#[derive(Deserialize)]
struct PasswordRequest {
    current_password: String,
    new_password: String,
}

/// Change the caller's own password.
async fn change_password(
    State(state): State<AppState>,
    _session: Session,
    user: CurrentUser,
    Json(req): Json<PasswordRequest>,
) -> ApiResult<StatusCode> {
    let row = users::get(&state.pool, &user.user.id)
        .await?
        .ok_or_else(auth::unauthorized)?;
    if !auth::verify_password(&req.current_password, &row.password_hash) {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "current password is incorrect".into(),
        });
    }
    if req.new_password.len() < 8 {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "new password must be at least 8 characters".into(),
        });
    }
    let hash = auth::hash_password(&req.new_password).map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("password hashing failed: {e}"),
    })?;
    users::set_password(&state.pool, &user.user.id, &hash).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "password_change",
        "users",
        "",
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn validate_credentials(username: &str, password: &str) -> ApiResult<()> {
    if username.trim().is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "username must not be empty".into(),
        });
    }
    if password.len() < 8 {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "password must be at least 8 characters".into(),
        });
    }
    Ok(())
}

fn bad_login() -> ApiError {
    ApiError {
        status: StatusCode::UNAUTHORIZED,
        message: "invalid username or password".into(),
    }
}
