use crate::error::AppError;
use crate::state::AppState;
use crate::util::{now_rfc3339, random_token_hex, validate_email, validate_password};
use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use axum::extract::FromRequestParts;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Method, request::Parts};
use axum_extra::extract::cookie::{Cookie, SameSite};
use rand_core::OsRng;
use serde::Serialize;
use sha2::Digest;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

pub const SESSION_COOKIE: &str = "mv_session";
pub const CSRF_HEADER: &str = "x-csrf-token";

#[derive(Clone, Debug, Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub role: String,
}

#[derive(Clone, Debug)]
pub struct AuthSession {
    pub user: User,
    pub csrf_token: String,
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    validate_password(password)?;
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| AppError::Internal)?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed = PasswordHash::new(hash).map_err(|_| AppError::Internal)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub fn validate_credentials(email: &str, password: &str) -> Result<(), AppError> {
    validate_email(email)?;
    validate_password(password)?;
    Ok(())
}

pub fn session_expires_at() -> OffsetDateTime {
    OffsetDateTime::now_utc() + Duration::days(30)
}

pub async fn create_session(state: &AppState, user_id: &str) -> Result<(String, String), AppError> {
    let session_id = Uuid::new_v4().to_string();
    let token = random_token_hex(32);
    let token_hash = session_token_hash(&state.config.session_secret, &token);
    let csrf_token = random_token_hex(32);
    let now = now_rfc3339();
    let expires_at = session_expires_at()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| AppError::Internal)?;

    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, csrf_token, expires_at, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&session_id)
    .bind(user_id)
    .bind(token_hash)
    .bind(&csrf_token)
    .bind(expires_at)
    .bind(now)
    .execute(&state.db.pool)
    .await?;

    Ok((token, csrf_token))
}

pub async fn delete_session(state: &AppState, token: &str) -> Result<(), AppError> {
    let token_hash = session_token_hash(&state.config.session_secret, token);
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(token_hash)
        .execute(&state.db.pool)
        .await?;
    Ok(())
}

pub fn build_session_cookie(token: &str, cookie_secure: bool) -> Cookie<'static> {
    let mut c = Cookie::new(SESSION_COOKIE, token.to_string());
    c.set_http_only(true);
    c.set_same_site(SameSite::Lax);
    c.set_secure(cookie_secure);
    c.set_path("/");
    c.set_max_age(time::Duration::days(30));
    c
}

pub fn build_clear_cookie(cookie_secure: bool) -> Cookie<'static> {
    let mut c = Cookie::new(SESSION_COOKIE, "");
    c.set_http_only(true);
    c.set_same_site(SameSite::Lax);
    c.set_secure(cookie_secure);
    c.set_path("/");
    c.make_removal();
    c
}

pub fn set_cookie(headers: &mut HeaderMap, cookie: Cookie<'static>) -> Result<(), AppError> {
    let v = HeaderValue::from_str(&cookie.to_string()).map_err(|_| AppError::Internal)?;
    headers.append(SET_COOKIE, v);
    Ok(())
}

fn cookie_from_headers(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let s = part.trim();
        if let Some((k, v)) = s.split_once('=')
            && k.trim() == SESSION_COOKIE
        {
            return Some(v.trim().to_string());
        }
    }
    None
}

pub fn require_csrf(
    method: &Method,
    headers: &HeaderMap,
    session: &AuthSession,
) -> Result<(), AppError> {
    if matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    ) {
        return Ok(());
    }
    let v = headers
        .get(CSRF_HEADER)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if v.is_empty() || v != session.csrf_token {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

impl FromRequestParts<AppState> for AuthSession {
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        let headers = parts.headers.clone();
        let method = parts.method.clone();
        let pool = state.db.pool.clone();
        let now = now_rfc3339();
        let secret = state.config.session_secret.clone();

        async move {
            let token = cookie_from_headers(&headers).ok_or(AppError::Unauthorized)?;
            let token_hash = session_token_hash(&secret, &token);

            let row = sqlx::query_as::<_, (String, String, String, String)>(
                "SELECT s.csrf_token, u.id, u.email, u.role
                 FROM sessions s
                 JOIN users u ON u.id = s.user_id
                 WHERE s.token_hash = ? AND s.expires_at > ?",
            )
            .bind(token_hash)
            .bind(now)
            .fetch_optional(&pool)
            .await?;

            let (csrf_token, user_id, email, role) = row.ok_or(AppError::Unauthorized)?;

            let tmp = AuthSession {
                user: User {
                    id: user_id.clone(),
                    email: email.clone(),
                    role: role.clone(),
                },
                csrf_token: csrf_token.clone(),
            };
            require_csrf(&method, &headers, &tmp)?;
            Ok(tmp)
        }
    }
}

pub fn require_admin(session: &AuthSession) -> Result<(), AppError> {
    if session.user.role == "admin" {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

fn session_token_hash(secret: &str, token: &str) -> String {
    let mut h = sha2::Sha256::new();
    h.update(secret.as_bytes());
    h.update(b":");
    h.update(token.as_bytes());
    hex::encode(h.finalize())
}
