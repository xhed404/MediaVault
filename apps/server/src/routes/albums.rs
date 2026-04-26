use crate::auth::AuthSession;
use crate::error::AppError;
use crate::state::AppState;
use crate::util::now_rfc3339;
use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(get_one))
        .route("/{id}/items", post(add_item))
        .route("/{id}/items/{file_id}", delete(remove_item))
}

#[derive(Serialize, sqlx::FromRow)]
struct AlbumItem {
    id: String,
    name: String,
    created_by: String,
    created_at: String,
}

async fn list(
    State(state): State<AppState>,
    _auth: AuthSession,
) -> Result<Json<Vec<AlbumItem>>, AppError> {
    let rows = sqlx::query_as::<_, AlbumItem>(
        "SELECT id, name, created_by, created_at FROM albums ORDER BY created_at DESC",
    )
    .fetch_all(&state.db.pool)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct CreateAlbumRequest {
    name: String,
}

async fn create(
    State(state): State<AppState>,
    auth: AuthSession,
    Json(req): Json<CreateAlbumRequest>,
) -> Result<Json<AlbumItem>, AppError> {
    let name = req.name.trim();
    if name.is_empty() || name.len() > 80 {
        return Err(AppError::bad_request("invalid album name"));
    }

    let id = Uuid::new_v4().to_string();
    let created_at = now_rfc3339();
    sqlx::query("INSERT INTO albums (id, name, created_by, created_at) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(name)
        .bind(&auth.user.id)
        .bind(&created_at)
        .execute(&state.db.pool)
        .await?;

    Ok(Json(AlbumItem {
        id,
        name: name.to_string(),
        created_by: auth.user.id,
        created_at,
    }))
}

#[derive(Serialize)]
struct AlbumDetail {
    album: AlbumItem,
    items: Vec<AlbumFileItem>,
}

#[derive(Serialize, sqlx::FromRow)]
struct AlbumFileItem {
    file_id: String,
    position: i64,
    original_name: String,
    mime: String,
    size_bytes: i64,
    sha256: String,
    created_at: String,
}

async fn get_one(
    State(state): State<AppState>,
    _auth: AuthSession,
    Path(id): Path<String>,
) -> Result<Json<AlbumDetail>, AppError> {
    let album = sqlx::query_as::<_, AlbumItem>(
        "SELECT id, name, created_by, created_at FROM albums WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let items = sqlx::query_as::<_, AlbumFileItem>(
        "SELECT ai.file_id, ai.position, f.original_name, f.mime, f.size_bytes, f.sha256, f.created_at
         FROM album_items ai
         JOIN files f ON f.id = ai.file_id
         WHERE ai.album_id = ? AND f.deleted_at IS NULL
         ORDER BY ai.position ASC",
    )
    .bind(&id)
    .fetch_all(&state.db.pool)
    .await?;

    Ok(Json(AlbumDetail { album, items }))
}

#[derive(Deserialize)]
struct AddItemRequest {
    file_id: String,
    position: Option<i64>,
}

async fn add_item(
    State(state): State<AppState>,
    _auth: AuthSession,
    Path(album_id): Path<String>,
    Json(req): Json<AddItemRequest>,
) -> Result<(), AppError> {
    let pos = req.position.unwrap_or(0);
    sqlx::query(
        "INSERT OR REPLACE INTO album_items (album_id, file_id, position, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(album_id)
    .bind(req.file_id)
    .bind(pos)
    .bind(now_rfc3339())
    .execute(&state.db.pool)
    .await?;
    Ok(())
}

async fn remove_item(
    State(state): State<AppState>,
    _auth: AuthSession,
    Path((album_id, file_id)): Path<(String, String)>,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM album_items WHERE album_id = ? AND file_id = ?")
        .bind(album_id)
        .bind(file_id)
        .execute(&state.db.pool)
        .await?;
    Ok(())
}
