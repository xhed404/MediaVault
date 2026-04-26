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
        .route("/groups", get(groups))
        .route("/apply", post(apply))
}

#[derive(Serialize)]
struct DupeGroup {
    sha256: String,
    count: i64,
    size_bytes: i64,
    file_ids: Vec<String>,
}

async fn groups(
    State(state): State<AppState>,
    _auth: AuthSession,
) -> Result<Json<Vec<DupeGroup>>, AppError> {
    let rows = sqlx::query_as::<_, (String, i64, i64, String)>(
        "SELECT sha256, COUNT(1) as c, MAX(size_bytes) as size_bytes, GROUP_CONCAT(id) as ids
         FROM files
         WHERE deleted_at IS NULL
         GROUP BY sha256
         HAVING c > 1
         ORDER BY c DESC
         LIMIT 200",
    )
    .fetch_all(&state.db.pool)
    .await?;

    let out = rows
        .into_iter()
        .map(|(sha256, count, size_bytes, ids)| DupeGroup {
            sha256,
            count,
            size_bytes,
            file_ids: ids
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        })
        .collect::<Vec<_>>();

    Ok(Json(out))
}

#[derive(Deserialize)]
struct ApplyRequest {
    keep_file_id: String,
    delete_file_ids: Vec<String>,
}

async fn apply(
    State(state): State<AppState>,
    auth: AuthSession,
    Json(req): Json<ApplyRequest>,
) -> Result<(), AppError> {
    if req.delete_file_ids.is_empty() {
        return Err(AppError::bad_request("delete_file_ids empty"));
    }

    let keep_sha = sqlx::query_scalar::<_, String>(
        "SELECT sha256 FROM files WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(&req.keep_file_id)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let mut tx = state.db.pool.begin().await?;

    for id in &req.delete_file_ids {
        let sha = sqlx::query_scalar::<_, String>(
            "SELECT sha256 FROM files WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;

        if sha != keep_sha {
            return Err(AppError::bad_request("files must have same sha256"));
        }
    }

    let now = now_rfc3339();
    for id in &req.delete_file_ids {
        sqlx::query("UPDATE files SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
            .bind(&now)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query(
        "INSERT INTO audit_log (id, user_id, action, target_type, target_id, meta_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&auth.user.id)
    .bind("dupes_applied")
    .bind("dupes")
    .bind(&req.keep_file_id)
    .bind(serde_json::json!({"deleted": req.delete_file_ids}).to_string())
    .bind(now)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}
