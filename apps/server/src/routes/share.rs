use crate::config::Config;
use crate::error::AppError;
use crate::state::AppState;
use crate::util::{now_rfc3339, random_token_hex, safe_filename, sha256_hex_str};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new().route("/", post(create))
}

#[derive(Deserialize)]
struct CreateShareRequest {
    kind: String,
    target_id: String,
    expires_in_seconds: Option<i64>,
}

#[derive(Serialize)]
struct CreateShareResponse {
    url: String,
}

async fn create(
    State(state): State<AppState>,
    crate::auth::AuthSession { user, .. }: crate::auth::AuthSession,
    Json(req): Json<CreateShareRequest>,
) -> Result<Json<CreateShareResponse>, AppError> {
    let kind = req.kind.trim();
    if kind != "file" && kind != "album" {
        return Err(AppError::bad_request("invalid kind"));
    }

    let token = random_token_hex(32);
    let token_hash = sha256_hex_str(&token);
    let id = Uuid::new_v4().to_string();
    let created_at = now_rfc3339();
    let expires_at = match req.expires_in_seconds {
        Some(secs) if secs > 0 => {
            let t = OffsetDateTime::now_utc() + Duration::seconds(secs);
            Some(
                t.format(&time::format_description::well_known::Rfc3339)
                    .map_err(|_| AppError::Internal)?,
            )
        }
        _ => None,
    };

    sqlx::query(
        "INSERT INTO share_links (id, kind, target_id, token_hash, expires_at, created_at, created_by)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(kind)
    .bind(req.target_id)
    .bind(token_hash)
    .bind(expires_at)
    .bind(created_at)
    .bind(user.id)
    .execute(&state.db.pool)
    .await?;

    let url = format!(
        "{}/s/{}",
        state.config.public_base_url.trim_end_matches('/'),
        token
    );
    Ok(Json(CreateShareResponse { url }))
}

pub async fn public_get(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Response, AppError> {
    if token.len() < 16 || token.len() > 256 {
        return Err(AppError::NotFound);
    }

    let token_hash = sha256_hex_str(&token);
    let now = now_rfc3339();

    let row = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT id, kind, expires_at FROM share_links WHERE token_hash = ?",
    )
    .bind(&token_hash)
    .fetch_optional(&state.db.pool)
    .await?;

    let Some((share_id, kind, expires_at)) = row else {
        return Err(AppError::NotFound);
    };

    if let Some(expires_at) = &expires_at
        && expires_at <= &now
    {
        return Err(AppError::NotFound);
    }

    sqlx::query("UPDATE share_links SET download_count = download_count + 1 WHERE id = ?")
        .bind(&share_id)
        .execute(&state.db.pool)
        .await?;

    if kind == "file" {
        return public_file(&state, &token_hash).await;
    }

    public_album(&state, &token_hash).await
}

async fn public_file(state: &AppState, token_hash: &str) -> Result<Response, AppError> {
    let row = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT f.original_name, f.stored_path, f.size_bytes, f.mime
         FROM share_links s
         JOIN files f ON f.id = s.target_id
         WHERE s.token_hash = ? AND s.kind = 'file' AND f.deleted_at IS NULL",
    )
    .bind(token_hash)
    .fetch_optional(&state.db.pool)
    .await?;

    let Some((name, rel, size, mime)) = row else {
        return Err(AppError::NotFound);
    };

    Config::validate_rel_path(&rel)?;
    let path = state.config.storage_object_full_path(&rel);
    let f = tokio::fs::File::open(path).await?;
    let stream = tokio_util::io::ReaderStream::new(f);
    let body = Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&mime)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&size.to_string()).unwrap_or(HeaderValue::from_static("0")),
    );
    let disp = format!("attachment; filename=\"{}\"", safe_filename(&name));
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&disp).map_err(|_| AppError::Internal)?,
    );
    Ok((headers, body).into_response())
}

#[derive(Serialize)]
struct AlbumPublicResponse {
    album_id: String,
    name: String,
    items: Vec<AlbumPublicItem>,
}

#[derive(Serialize, sqlx::FromRow)]
struct AlbumPublicItem {
    file_id: String,
    original_name: String,
    mime: String,
    size_bytes: i64,
}

async fn public_album(state: &AppState, token_hash: &str) -> Result<Response, AppError> {
    let album = sqlx::query_as::<_, (String, String)>(
        "SELECT a.id, a.name
         FROM share_links s
         JOIN albums a ON a.id = s.target_id
         WHERE s.token_hash = ? AND s.kind = 'album'",
    )
    .bind(token_hash)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let items = sqlx::query_as::<_, AlbumPublicItem>(
        "SELECT f.id as file_id, f.original_name, f.mime, f.size_bytes
         FROM share_links s
         JOIN album_items ai ON ai.album_id = s.target_id
         JOIN files f ON f.id = ai.file_id
         WHERE s.token_hash = ? AND s.kind = 'album' AND f.deleted_at IS NULL
         ORDER BY ai.position ASC",
    )
    .bind(token_hash)
    .fetch_all(&state.db.pool)
    .await?;

    Ok(Json(AlbumPublicResponse {
        album_id: album.0,
        name: album.1,
        items,
    })
    .into_response())
}
