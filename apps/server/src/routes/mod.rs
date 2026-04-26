pub mod admin;
pub mod albums;
pub mod auth;
pub mod dupes;
pub mod files;
pub mod share;
pub mod tags;

use crate::error::AppError;
use crate::state::AppState;
use axum::Router;
use axum::http::HeaderMap;
use axum::http::HeaderName;
use axum::http::Method;
use axum::http::StatusCode;
use axum::http::header::COOKIE;
use axum::http::header::{ACCEPT, CONTENT_TYPE};
use axum::response::IntoResponse;
use axum::routing::get;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

pub fn app_router(state: AppState) -> Router {
    let api = Router::new()
        .nest("/auth", auth::router())
        .nest("/files", files::router())
        .nest("/tags", tags::router())
        .nest("/albums", albums::router())
        .nest("/dupes", dupes::router())
        .nest("/share", share::router())
        .nest("/admin", admin::router())
        .route("/health", get(health))
        .with_state(state.clone());

    let cors = cors_layer(&state);

    let mut app = Router::new()
        .nest("/api", api.layer(cors))
        .route("/s/{token}", get(share::public_get))
        .with_state(state.clone());

    if let Some(dist) = &state.config.web_dist {
        let index = dist.join("index.html");
        let static_svc = ServeDir::new(dist).not_found_service(ServeFile::new(index));
        app = app.fallback_service(static_svc);
    } else {
        app = app.fallback(fallback);
    }

    app
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn fallback() -> AppError {
    AppError::NotFound
}

pub(crate) fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let s = part.trim();
        if let Some((k, v)) = s.split_once('=')
            && k.trim() == crate::auth::SESSION_COOKIE
        {
            return Some(v.trim().to_string());
        }
    }
    None
}

fn cors_layer(_state: &AppState) -> CorsLayer {
    let allow_origin = std::env::var("CORS_ALLOW_ORIGIN").ok().unwrap_or_default();
    let allow_origin = allow_origin.trim();
    let allow_headers = [
        CONTENT_TYPE,
        ACCEPT,
        HeaderName::from_static("x-csrf-token"),
    ];
    let allow_methods = [
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
        Method::OPTIONS,
    ];

    if allow_origin.is_empty() {
        return CorsLayer::new()
            .allow_origin([
                "http://localhost:5173".parse().unwrap(),
                "http://127.0.0.1:5173".parse().unwrap(),
            ])
            .allow_headers(allow_headers)
            .allow_methods(allow_methods)
            .allow_credentials(true);
    }

    if allow_origin == "*" {
        return CorsLayer::new()
            .allow_origin(Any)
            .allow_headers(Any)
            .allow_methods(Any);
    }

    let origins = allow_origin
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect::<Vec<_>>();

    if origins.is_empty() {
        return CorsLayer::new()
            .allow_origin([
                "http://localhost:5173".parse().unwrap(),
                "http://127.0.0.1:5173".parse().unwrap(),
            ])
            .allow_headers(allow_headers)
            .allow_methods(allow_methods)
            .allow_credentials(true);
    }

    CorsLayer::new()
        .allow_origin(origins)
        .allow_headers(allow_headers)
        .allow_methods(allow_methods)
        .allow_credentials(true)
}
