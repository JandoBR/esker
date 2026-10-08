mod auth;
mod config;
mod error;
mod items;
mod state;
mod user;
mod util;

use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use tokio::sync::OnceCell;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing_subscriber::EnvFilter;

use crate::{config::Config, state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cfg = Config::from_env()?;
    std::fs::create_dir_all(cfg.data_dir.join("files"))?;

    let options = SqliteConnectOptions::new()
        .filename(cfg.data_dir.join("board.db"))
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    sqlx::migrate!("./migrations").run(&db).await?;

    let http = openidconnect::reqwest::ClientBuilder::new()
        // Following redirects in the provider client opens up SSRF.
        .redirect(openidconnect::reqwest::redirect::Policy::none())
        .build()?;

    let state = Arc::new(AppState {
        cfg: cfg.clone(),
        db,
        http,
        metadata: OnceCell::new(),
    });

    // Expire items in the background; reads already hide them.
    {
        let cleanup = state.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(600));
            loop {
                ticker.tick().await;
                if let Err(error) = items::cleanup_expired(&cleanup).await {
                    tracing::warn!("expiry cleanup failed: {error}");
                }
            }
        });
    }

    let api = Router::new()
        .route("/health", get(health))
        .route("/me", get(auth::me))
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/auth/logout", post(auth::logout))
        .route("/items", get(items::list_items).post(items::create_item))
        .route(
            "/items/{id}",
            get(items::get_item)
                .patch(items::update_item)
                .delete(items::delete_item),
        )
        .route("/items/{id}/file", get(items::get_item_file_legacy))
        .route("/items/{id}/files", post(items::add_item_files))
        .route(
            "/items/{id}/files/{file_id}",
            get(items::get_item_file).delete(items::delete_item_file),
        )
        .with_state(state.clone());

    // The default 2 MB body limit would block uploads; enforce our own instead.
    let body_limit = if cfg.max_upload_bytes > 0 {
        DefaultBodyLimit::max(cfg.max_upload_bytes)
    } else {
        DefaultBodyLimit::disable()
    };

    let index = cfg.static_dir.join("index.html");
    let app = Router::new()
        .nest("/api", api)
        .fallback_service(ServeDir::new(&cfg.static_dir).fallback(ServeFile::new(index)))
        .layer(TraceLayer::new_for_http())
        .layer(body_limit);

    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    tracing::info!("listening on {}", cfg.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}
