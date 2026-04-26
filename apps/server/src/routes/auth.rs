use crate::auth::{
    AuthSession, build_clear_cookie, build_session_cookie, create_session, delete_session,
    require_admin, set_cookie, validate_credentials, verify_password,
};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::{now_rfc3339, validate_email, validate_password};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/session", get(session))
        .route("/users", post(create_user))
}

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    csrf_token: String,
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<(HeaderMap, Json<LoginResponse>), AppError> {
    validate_credentials(&req.email, &req.password)?;

    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT id, password_hash FROM users WHERE email = ?",
    )
    .bind(req.email.trim())
    .fetch_optional(&state.db.pool)
    .await?;

    let Some((user_id, password_hash)) = row else {
        return Err(AppError::Unauthorized);
    };

    if !verify_password(&req.password, &password_hash)? {
        return Err(AppError::Unauthorized);
    }

    let (token, csrf) = create_session(&state, &user_id).await?;

    sqlx::query("UPDATE users SET last_login_at = ? WHERE id = ?")
        .bind(now_rfc3339())
        .bind(&user_id)
        .execute(&state.db.pool)
        .await?;

    let mut headers = HeaderMap::new();
    set_cookie(
        &mut headers,
        build_session_cookie(&token, state.config.cookie_secure),
    )?;
    Ok((headers, Json(LoginResponse { csrf_token: csrf })))
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(HeaderMap, ()), AppError> {
    let token = super::extract_session_token(&headers);
    if let Some(token) = token {
        let _ = delete_session(&state, &token).await;
    }
    let mut out = HeaderMap::new();
    set_cookie(&mut out, build_clear_cookie(state.config.cookie_secure))?;
    Ok((out, ()))
}

#[derive(Serialize)]
struct SessionResponse {
    user: crate::auth::User,
    csrf_token: String,
}

async fn session(auth: AuthSession) -> Result<Json<SessionResponse>, AppError> {
    Ok(Json(SessionResponse {
        user: auth.user,
        csrf_token: auth.csrf_token,
    }))
}

#[derive(Deserialize)]
struct CreateUserRequest {
    email: String,
    password: String,
    role: Option<String>,
}

#[derive(Serialize)]
struct CreateUserResponse {
    id: String,
}

async fn create_user(
    State(state): State<AppState>,
    auth: AuthSession,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<CreateUserResponse>, AppError> {
    require_admin(&auth)?;
    validate_email(&req.email)?;
    validate_password(&req.password)?;

    let role = req.role.unwrap_or_else(|| "user".to_string());
    if role != "user" && role != "admin" {
        return Err(AppError::bad_request("invalid role"));
    }

    let hash = crate::auth::hash_password(&req.password)?;
    let id = Uuid::new_v4().to_string();
    let res = sqlx::query(
        "INSERT INTO users (id, email, password_hash, role, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(req.email.trim())
    .bind(hash)
    .bind(role)
    .bind(now_rfc3339())
    .execute(&state.db.pool)
    .await;

    match res {
        Ok(_) => Ok(Json(CreateUserResponse { id })),
        Err(_) => Err(AppError::conflict("email already exists")),
    }
}
