use crate::auth::AuthSession;
use crate::config::Config;
use crate::error::AppError;
use crate::state::AppState;
use crate::util::{now_rfc3339, safe_filename};
use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use sqlx::{QueryBuilder, Sqlite};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/upload", post(upload))
        .route("/", get(list))
        .route("/{id}", get(get_one).delete(delete_one))
        .route("/{id}/download", get(download))
}

#[derive(Serialize, sqlx::FromRow)]
struct FileItem {
    id: String,
    original_name: String,
    mime: String,
    size_bytes: i64,
    sha256: String,
    created_at: String,
    deleted_at: Option<String>,
}

#[derive(Deserialize)]
struct ListQuery {
    q: Option<String>,
    mime_prefix: Option<String>,
    page: Option<u32>,
    per_page: Option<u32>,
    include_deleted: Option<bool>,
}

async fn list(
    State(state): State<AppState>,
    _auth: AuthSession,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<FileItem>>, AppError> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(50).clamp(1, 200);
    let offset = (page - 1) as i64 * per_page as i64;

    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, original_name, mime, size_bytes, sha256, created_at, deleted_at FROM files",
    );
    let mut first = true;

    if q.include_deleted != Some(true) {
        qb.push(if first { " WHERE " } else { " AND " });
        first = false;
        qb.push("deleted_at IS NULL");
    }

    if let Some(search) = q.q.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        qb.push(if first { " WHERE " } else { " AND " });
        first = false;
        qb.push("original_name LIKE ");
        qb.push_bind(format!("%{}%", search.replace('%', "")));
    }

    if let Some(prefix) = q
        .mime_prefix
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        qb.push(if first { " WHERE " } else { " AND " });
        qb.push("mime LIKE ");
        qb.push_bind(format!("{}%", prefix.replace('%', "")));
    }

    qb.push(" ORDER BY created_at DESC LIMIT ");
    qb.push_bind(per_page as i64);
    qb.push(" OFFSET ");
    qb.push_bind(offset);

    let rows = qb
        .build_query_as::<FileItem>()
        .fetch_all(&state.db.pool)
        .await?;
    Ok(Json(rows))
}

#[derive(Serialize)]
struct UploadResponse {
    id: String,
}

async fn upload(
    State(state): State<AppState>,
    auth: AuthSession,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<UploadResponse>), AppError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad_request("invalid multipart"))?
    {
        if field.name() != Some("file") {
            continue;
        }

        let filename = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "file".to_string());
        let original_name = safe_filename(&filename);

        let mime = field
            .content_type()
            .map(|m| m.to_string())
            .unwrap_or_else(|| {
                mime_guess::from_path(&original_name)
                    .first_or_octet_stream()
                    .to_string()
            });

        let tmp_name = format!("{}.part", Uuid::new_v4());
        let tmp_path = state.config.storage_tmp_dir().join(tmp_name);
        let mut f = tokio::fs::File::create(&tmp_path).await?;

        let mut size: u64 = 0;
        let mut hasher = sha2::Sha256::new();
        let mut field = field;
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|_| AppError::bad_request("upload failed"))?
        {
            size = size.saturating_add(chunk.len() as u64);
            if size > state.config.max_upload_bytes {
                let _ = tokio::fs::remove_file(&tmp_path).await;
                return Err(AppError::PayloadTooLarge);
            }
            hasher.update(&chunk);
            f.write_all(&chunk).await?;
        }
        f.flush().await?;
        drop(f);

        let sha256 = hex::encode(hasher.finalize());
        let rel = state.config.object_rel_path(&sha256);
        Config::validate_rel_path(&rel)?;
        let target_path = state.config.storage_object_full_path(&rel);
        if let Some(parent) = target_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        if tokio::fs::metadata(&target_path).await.is_err() {
            match tokio::fs::rename(&tmp_path, &target_path).await {
                Ok(_) => {}
                Err(_) => {
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                }
            }
        } else {
            let _ = tokio::fs::remove_file(&tmp_path).await;
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO files (id, original_name, stored_path, mime, size_bytes, sha256, created_at, uploaded_by)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(original_name)
        .bind(rel)
        .bind(mime)
        .bind(size as i64)
        .bind(sha256)
        .bind(now_rfc3339())
        .bind(&auth.user.id)
        .execute(&state.db.pool)
        .await?;

        return Ok((StatusCode::CREATED, Json(UploadResponse { id })));
    }

    Err(AppError::bad_request("file field missing"))
}

async fn get_one(
    State(state): State<AppState>,
    _auth: AuthSession,
    Path(id): Path<String>,
) -> Result<Json<FileItem>, AppError> {
    let row = sqlx::query_as::<_, FileItem>(
        "SELECT id, original_name, mime, size_bytes, sha256, created_at, deleted_at FROM files WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db.pool)
    .await?;

    row.map(Json).ok_or(AppError::NotFound)
}

async fn delete_one(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let res = sqlx::query("UPDATE files SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
        .bind(now_rfc3339())
        .bind(&id)
        .execute(&state.db.pool)
        .await?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    sqlx::query(
        "INSERT INTO audit_log (id, user_id, action, target_type, target_id, meta_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&auth.user.id)
    .bind("file_deleted")
    .bind("file")
    .bind(&id)
    .bind("{}")
    .bind(now_rfc3339())
    .execute(&state.db.pool)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn download(
    State(state): State<AppState>,
    _auth: AuthSession,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let row = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT original_name, stored_path, size_bytes, mime FROM files WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
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
    Ok((headers, body))
}
