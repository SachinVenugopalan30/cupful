// SPDX-License-Identifier: AGPL-3.0-or-later
pub mod ai;
mod assets;
pub mod auth;
pub mod budget;
pub mod cleanup;
pub mod config;
pub mod db;
pub mod error;
pub mod products;
pub mod profile;
mod reads;
pub mod sync;
mod time;

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, State};
use axum::routing::{MethodRouter, delete, get, post, put};
use axum::{Json, Router, middleware};
pub use config::Config;
use serde_json::{Value, json};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<Config>,
    pub login_limiter: Arc<auth::RateLimiter>,
    pub products: Arc<products::Lookup>,
}

impl AppState {
    pub fn new(db: SqlitePool, config: Config) -> Self {
        Self {
            db,
            products: Arc::new(products::Lookup::new(&config)),
            config: Arc::new(config),
            login_limiter: Arc::new(auth::RateLimiter::new(5, Duration::from_secs(60))),
        }
    }
}

/// Who may call a route. `User` routes read or write the logged-in user's own data and each needs
/// an isolation test (`tests/isolation.rs` checks this list).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Access {
    Public,
    User,
}

/// Every API route, declared once so tests can enumerate them.
fn routes() -> Vec<(&'static str, &'static str, Access, MethodRouter<AppState>)> {
    use Access::*;
    vec![
        ("GET", "/api/health", Public, get(health)),
        ("GET", "/api/auth/state", Public, get(auth::auth_state)),
        ("POST", "/api/auth/setup", Public, post(auth::setup)),
        ("POST", "/api/auth/login", Public, post(auth::login)),
        ("POST", "/api/auth/logout", Public, post(auth::logout)),
        ("GET", "/api/profile", User, get(profile::get)),
        ("PUT", "/api/profile", User, put(profile::put)),
        (
            "POST",
            "/api/sync",
            User,
            post(sync::sync).layer(DefaultBodyLimit::max(1 << 20)),
        ),
        ("GET", "/api/days", User, get(reads::days)),
        ("GET", "/api/days/{date}", User, get(reads::day)),
        ("GET", "/api/meals", User, get(reads::meals)),
        ("GET", "/api/budget/{date}", User, get(reads::budget)),
        ("GET", "/api/products", User, get(products::list)),
        ("POST", "/api/products", User, post(products::create)),
        // `{id}` is a barcode for GET and a food id for DELETE (one name per path segment).
        ("GET", "/api/products/{id}", User, get(products::lookup)),
        (
            "DELETE",
            "/api/products/{id}",
            User,
            delete(products::remove),
        ),
    ]
}

/// `(method, path, access)` for every API route.
pub fn route_table() -> Vec<(&'static str, &'static str, Access)> {
    routes().into_iter().map(|(m, p, a, _)| (m, p, a)).collect()
}

pub fn router(state: AppState) -> Router {
    let mut r = Router::new();
    for (_, path, _, handler) in routes() {
        r = r.route(path, handler);
    }
    r.fallback(assets::serve)
        .layer(auth::session_layer(
            state.db.clone(),
            state.config.secure_cookies,
        ))
        .layer(middleware::from_fn(auth::csrf))
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
