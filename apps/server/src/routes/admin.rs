use crate::auth::{AuthSession, require_admin};
use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/stats", get(stats))
        .route("/audit", get(audit))
}

#[derive(Serialize)]
struct StatsResponse {
    files_count: i64,
    total_bytes: i64,
    deleted_count: i64,
    dupes_groups: i64,
}

async fn stats(
    State(state): State<AppState>,
    auth: AuthSession,
) -> Result<Json<StatsResponse>, AppError> {
    require_admin(&auth)?;

    let (files_count, total_bytes) = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COUNT(1), COALESCE(SUM(size_bytes), 0) FROM files WHERE deleted_at IS NULL",
    )
    .fetch_one(&state.db.pool)
    .await?;

    let deleted_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM files WHERE deleted_at IS NOT NULL")
            .fetch_one(&state.db.pool)
            .await?;

    let dupes_groups = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(1) FROM (SELECT sha256 FROM files WHERE deleted_at IS NULL GROUP BY sha256 HAVING COUNT(1) > 1)",
    )
    .fetch_one(&state.db.pool)
    .await?;

    Ok(Json(StatsResponse {
        files_count,
        total_bytes,
        deleted_count,
        dupes_groups,
    }))
}

#[derive(Deserialize)]
struct AuditQuery {
    limit: Option<u32>,
}

#[derive(Serialize, sqlx::FromRow)]
struct AuditItem {
    id: String,
    user_id: String,
    action: String,
    target_type: String,
    target_id: String,
    meta_json: String,
    created_at: String,
}

async fn audit(
    State(state): State<AppState>,
    auth: AuthSession,
    Query(q): Query<AuditQuery>,
) -> Result<Json<Vec<AuditItem>>, AppError> {
    require_admin(&auth)?;
    let limit = q.limit.unwrap_or(200).clamp(1, 1000) as i64;
    let rows = sqlx::query_as::<_, AuditItem>(
        "SELECT id, user_id, action, target_type, target_id, meta_json, created_at
         FROM audit_log ORDER BY created_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db.pool)
    .await?;
    Ok(Json(rows))
}
