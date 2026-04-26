use crate::auth::AuthSession;
use crate::error::AppError;
use crate::state::AppState;
use crate::util::now_rfc3339;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/attach", post(attach))
        .route("/detach", post(detach))
}

#[derive(Serialize, sqlx::FromRow)]
struct TagItem {
    id: String,
    name: String,
    created_at: String,
}

async fn list(
    State(state): State<AppState>,
    _auth: AuthSession,
) -> Result<Json<Vec<TagItem>>, AppError> {
    let rows =
        sqlx::query_as::<_, TagItem>("SELECT id, name, created_at FROM tags ORDER BY name ASC")
            .fetch_all(&state.db.pool)
            .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct CreateTagRequest {
    name: String,
}

async fn create(
    State(state): State<AppState>,
    _auth: AuthSession,
    Json(req): Json<CreateTagRequest>,
) -> Result<Json<TagItem>, AppError> {
    let name = req.name.trim();
    if name.is_empty() || name.len() > 40 {
        return Err(AppError::bad_request("invalid tag name"));
    }

    let id = Uuid::new_v4().to_string();
    let created_at = now_rfc3339();
    let res = sqlx::query("INSERT INTO tags (id, name, created_at) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(name)
        .bind(&created_at)
        .execute(&state.db.pool)
        .await;

    match res {
        Ok(_) => Ok(Json(TagItem {
            id,
            name: name.to_string(),
            created_at,
        })),
        Err(_) => Err(AppError::conflict("tag exists")),
    }
}

#[derive(Deserialize)]
struct AttachRequest {
    file_id: String,
    tag_name: String,
}

async fn attach(
    State(state): State<AppState>,
    _auth: AuthSession,
    Json(req): Json<AttachRequest>,
) -> Result<(), AppError> {
    let tag_name = req.tag_name.trim();
    if tag_name.is_empty() || tag_name.len() > 40 {
        return Err(AppError::bad_request("invalid tag name"));
    }

    let now = now_rfc3339();

    let mut tx = state.db.pool.begin().await?;

    let tag_id = sqlx::query_scalar::<_, String>("SELECT id FROM tags WHERE name = ?")
        .bind(tag_name)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    sqlx::query("INSERT OR IGNORE INTO tags (id, name, created_at) VALUES (?, ?, ?)")
        .bind(&tag_id)
        .bind(tag_name)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

    sqlx::query("INSERT OR IGNORE INTO file_tags (file_id, tag_id, created_at) VALUES (?, ?, ?)")
        .bind(&req.file_id)
        .bind(&tag_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

#[derive(Deserialize)]
struct DetachRequest {
    file_id: String,
    tag_id: String,
}

async fn detach(
    State(state): State<AppState>,
    _auth: AuthSession,
    Json(req): Json<DetachRequest>,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM file_tags WHERE file_id = ? AND tag_id = ?")
        .bind(req.file_id)
        .bind(req.tag_id)
        .execute(&state.db.pool)
        .await?;
    Ok(())
}
