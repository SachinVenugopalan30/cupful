// SPDX-License-Identifier: AGPL-3.0-or-later
#![allow(dead_code)]

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use cupful::{AppState, Config};
use http_body_util::BodyExt;
use tower::ServiceExt;

/// Barcode lookups are off so no test reaches the real Open Food Facts; product tests use `app_with`.
pub async fn app() -> (Router, AppState) {
    app_with(Config {
        product_lookup: false,
        ..Config::default()
    })
    .await
}

pub async fn app_with(config: Config) -> (Router, AppState) {
    let state = AppState::new(cupful::db::memory().await, config);
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

/// Cookie-carrying API client. Sends the CSRF header on every request.
#[derive(Clone)]
pub struct Client {
    pub app: Router,
    pub cookie: Option<String>,
}

impl Client {
    pub fn new(app: &Router) -> Self {
        Self {
            app: app.clone(),
            cookie: None,
        }
    }

    pub async fn req(
        &mut self,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let mut b = Request::builder()
            .method(method)
            .uri(uri)
            .header("x-cupful", "1");
        if let Some(c) = &self.cookie {
            b = b.header("cookie", c);
        }
        let req = match body {
            Some(v) => b
                .header("content-type", "application/json")
                .body(Body::from(v.to_string())),
            None => b.body(Body::empty()),
        }
        .unwrap();
        let (status, headers, bytes) = send(&self.app, req).await;
        if let Some(sc) = headers.get("set-cookie") {
            let sc = sc.to_str().unwrap();
            let pair = sc.split(';').next().unwrap().to_string();
            // An expired cookie (logout) clears it.
            self.cookie = if sc.contains("Max-Age=0") {
                None
            } else {
                Some(pair)
            };
        }
        let v = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
        };
        (status, v)
    }

    pub async fn get(&mut self, uri: &str) -> (StatusCode, serde_json::Value) {
        self.req("GET", uri, None).await
    }

    pub async fn post(
        &mut self,
        uri: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        self.req("POST", uri, Some(body)).await
    }

    pub async fn put(
        &mut self,
        uri: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        self.req("PUT", uri, Some(body)).await
    }

    /// Posts a text body (CSV import) with the CSRF header; the reply is parsed as JSON.
    pub async fn post_text(&mut self, uri: &str, body: &str) -> (StatusCode, serde_json::Value) {
        let (status, _, bytes) = self.post_raw(uri, "text/csv", body.to_string()).await;
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        )
    }

    /// Posts any body with an explicit content type and the CSRF header; returns the raw reply.
    pub async fn post_raw(
        &mut self,
        uri: &str,
        content_type: &str,
        body: impl Into<Body>,
    ) -> (StatusCode, HeaderMap, Bytes) {
        let mut b = Request::post(uri)
            .header("x-cupful", "1")
            .header("content-type", content_type);
        if let Some(c) = &self.cookie {
            b = b.header("cookie", c);
        }
        send(&self.app, b.body(body.into()).unwrap()).await
    }

    /// GET without JSON parsing (exports and backups).
    pub async fn get_bytes(&mut self, uri: &str) -> (StatusCode, HeaderMap, Bytes) {
        let mut b = Request::get(uri);
        if let Some(c) = &self.cookie {
            b = b.header("cookie", c);
        }
        send(&self.app, b.body(Body::empty()).unwrap()).await
    }
}

pub const PASSWORD: &str = "correct horse battery";

/// Runs first-run setup and returns the logged-in admin.
pub async fn admin(app: &Router) -> Client {
    let mut c = Client::new(app);
    let (status, body) = c
        .post("/api/auth/setup", serde_json::json!({"username": "admin", "password": PASSWORD, "timezone": "Asia/Kolkata"}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    c
}

/// Creates a non-admin user directly and logs them in.
pub async fn user(app: &Router, state: &AppState, name: &str) -> Client {
    cupful::auth::create_user(&state.db, name, PASSWORD, "Asia/Kolkata", false)
        .await
        .unwrap();
    let mut c = Client::new(app);
    let (status, body) = c
        .post(
            "/api/auth/login",
            serde_json::json!({"username": name, "password": PASSWORD}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    c
}

pub fn today(tz: &str) -> jiff::civil::Date {
    jiff::Zoned::now()
        .with_time_zone(jiff::tz::TimeZone::get(tz).unwrap())
        .date()
}

/// RFC 3339 timestamp `secs` seconds from now (negative = past).
pub fn at(secs: i64) -> String {
    (jiff::Timestamp::now() + jiff::SignedDuration::from_secs(secs)).to_string()
}

pub fn uuid() -> String {
    uuid::Uuid::now_v7().to_string()
}

pub fn profile_body() -> serde_json::Value {
    serde_json::json!({
        "timezone": "Asia/Kolkata", "weight_unit": "kg", "height_unit": "cm",
        "sex": "male", "birth_year": 1990, "height_cm": 180.0, "start_weight_kg": 85.0,
        "goal_kg": 75.0, "pace_kg_per_week": 0.5, "typical_steps": 8000
    })
}

pub fn meal(id: &str, date: &str, items: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": id, "date": date, "meal_type": "lunch", "description": "rice and chicken",
        "scale": 1.0, "assumptions": null, "confidence": "medium", "source": "manual",
        "provider_id": null, "items": items
    })
}

pub fn item(id: &str, kcal: f64) -> serde_json::Value {
    serde_json::json!({
        "id": id, "name": "rice", "portion_text": "1 cup", "grams": 150.0,
        "kcal": kcal, "protein_g": 4.0, "carbs_g": 40.0, "fat_g": 0.5,
        "food_id": null, "origin": "ai_estimate"
    })
}

pub fn op(kind: &str, at: &str, payload: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"op_id": uuid(), "kind": kind, "at": at, "payload": payload})
}

impl Client {
    /// Posts ops to /api/sync and returns each op's status.
    pub async fn sync(&mut self, ops: Vec<serde_json::Value>) -> Vec<String> {
        let (status, body) = self
            .post("/api/sync", serde_json::json!({ "ops": ops }))
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["status"].as_str().unwrap().to_string())
            .collect()
    }

    /// Creates an ingest token and returns `(id, token)`.
    pub async fn token(&mut self, label: &str) -> (i64, String) {
        let (status, body) = self
            .post("/api/ingest-tokens", serde_json::json!({ "label": label }))
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        (
            body["id"].as_i64().unwrap(),
            body["token"].as_str().unwrap().to_string(),
        )
    }
}

/// AI provider stub that returns one fixed reply (nutrition estimates).
pub struct FixedReply(pub serde_json::Value);

#[async_trait::async_trait]
impl cupful::ai::Provider for FixedReply {
    async fn complete_json(
        &self,
        _: cupful::ai::JsonRequest,
    ) -> Result<serde_json::Value, cupful::ai::ProviderError> {
        Ok(self.0.clone())
    }

    fn capabilities(&self) -> cupful::ai::Capabilities {
        Default::default()
    }
}

/// `POST /api/ingest/activity` as a Shortcut would: bearer token, no cookie, no CSRF header.
pub async fn ingest(
    app: &Router,
    token: Option<&str>,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let mut b = Request::post("/api/ingest/activity").header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let (status, _, bytes) = send(app, b.body(Body::from(body.to_string())).unwrap()).await;
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}
