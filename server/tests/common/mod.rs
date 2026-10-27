// SPDX-License-Identifier: AGPL-3.0-or-later
#![allow(dead_code)]

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use cupful::{AppState, Config};
use http_body_util::BodyExt;
use tower::ServiceExt;

pub async fn app() -> (Router, AppState) {
    let state = AppState::new(cupful::db::memory().await, Config::default());
    (cupful::router(state.clone()), state)
}

pub async fn send(app: &Router, req: Request<Body>) -> (StatusCode, HeaderMap, Bytes) {
    let res = app.clone().oneshot(req).await.unwrap();
    let (parts, body) = res.into_parts();
    (
        parts.status,
        parts.headers,
        body.collect().await.unwrap().to_bytes(),
    )
}

pub fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

pub fn json(bytes: &Bytes) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap()
}
