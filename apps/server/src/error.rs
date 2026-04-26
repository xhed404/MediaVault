use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("payload too large")]
    PayloadTooLarge,
    #[error("config error: {0}")]
    Config(String),
    #[error("internal error")]
    Internal,
}

impl AppError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }

    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden".to_string()),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not_found".to_string()),
            AppError::Conflict(m) => (StatusCode::CONFLICT, m.clone()),
            AppError::PayloadTooLarge => {
                (StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large".into())
            }
            AppError::Config(m) => (StatusCode::INTERNAL_SERVER_ERROR, m.clone()),
            AppError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal".into()),
        };

        (status, axum::Json(ErrorBody { error: msg })).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(_: sqlx::Error) -> Self {
        AppError::Internal
    }
}

impl From<sqlx::migrate::MigrateError> for AppError {
    fn from(_: sqlx::migrate::MigrateError) -> Self {
        AppError::Internal
    }
}

impl From<std::io::Error> for AppError {
    fn from(_: std::io::Error) -> Self {
        AppError::Internal
    }
}
