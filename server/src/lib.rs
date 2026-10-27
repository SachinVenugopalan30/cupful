// SPDX-License-Identifier: AGPL-3.0-or-later
mod assets;
pub mod config;
pub mod db;
pub mod error;

use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
pub use config::Config;
use serde_json::{Value, json};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(db: SqlitePool, config: Config) -> Self {
        Self {
            db,
            config: Arc::new(config),
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .fallback(assets::serve)
        .with_state(state)
}

async fn health(State(s): State<AppState>) -> error::Result<Json<Value>> {
    sqlx::query("SELECT 1").execute(&s.db).await?;
    Ok(Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "source_url": s.config.source_url,
    })))
}
