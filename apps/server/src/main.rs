mod auth;
mod config;
mod db;
mod error;
mod routes;
mod state;
mod util;

use crate::config::Config;
use crate::db::Database;
use crate::routes::app_router;
use crate::state::AppState;
use axum::Router;
use std::net::SocketAddr;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("mediavault_server=info,tower_http=info")),
        )
        .init();

    let config = Config::from_env()?;
    let db = Database::connect(&config).await?;
    db.bootstrap(&config).await?;

    tokio::fs::create_dir_all(config.storage_objects_dir()).await?;
    tokio::fs::create_dir_all(config.storage_tmp_dir()).await?;

    let state = AppState { config, db };

    let app: Router = app_router(state.clone()).layer(TraceLayer::new_for_http());

    let addr: SocketAddr = state.config.bind_addr.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
            let _ = sig.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
