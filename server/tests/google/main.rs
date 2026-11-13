// SPDX-License-Identifier: AGPL-3.0-or-later
//! Google Health: OAuth connect, steps sync, workouts and disconnect against a mocked Google (PLAN.md §15).
/// Shared helpers live at tests/common/mod.rs, outside this target directory.
#[path = "../common/mod.rs"]
mod common;

mod connection;
mod steps;
mod workouts;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use common::*;
use cupful::crypto::Secret;
use cupful::{AppState, Config};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TZ: &str = "Asia/Kolkata";
const ROLLUP: &str = "/users/me/dataTypes/steps/dataPoints:dailyRollUp";

fn day(offset: i32) -> jiff::civil::Date {
    today(TZ)
        .checked_add(jiff::Span::new().days(offset))
        .unwrap()
}

async fn setup() -> (MockServer, Router, AppState, Client) {
    let g = MockServer::start().await;
    let (app, state) = app_with(Config {
        product_lookup: false,
        google_client_id: Some("cid.apps.googleusercontent.com".into()),
        google_client_secret: Some(Secret::new("shh-secret".into())),
        public_url: Some("https://cupful.test".into()),
        google_auth_url: format!("{}/auth", g.uri()),
        google_token_url: format!("{}/token", g.uri()),
        google_revoke_url: format!("{}/revoke", g.uri()),
        google_health_url: format!("{}/v4", g.uri()),
        ..Config::default()
    })
    .await;
    let mut a = admin(&app).await;
    a.put("/api/profile", profile_body()).await;
    (g, app, state, a)
}

async fn user_id(state: &AppState, name: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
        .bind(name)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

/// Stores a connection as a finished OAuth flow would. `access_ttl_s` < 0 means already expired.
async fn link(state: &AppState, uid: i64, access_ttl_s: i64) {
    let refresh = state.secrets.seal(&state.db, "rt-old").await.unwrap();
    let access = state.secrets.seal(&state.db, "at-old").await.unwrap();
    let exp = jiff::Timestamp::now().as_millisecond() + access_ttl_s * 1000;
    sqlx::query(
        "INSERT INTO google_connections (user_id, refresh_token, access_token, access_expires_at, connected_at)
         VALUES (?, ?, ?, ?, 0)",
    )
    .bind(uid)
    .bind(refresh)
    .bind(access)
    .bind(exp)
    .execute(&state.db)
    .await
    .unwrap();
}

/// GET with the client's cookie; returns status and the `Location` header.
async fn nav(c: &mut Client, uri: &str) -> (StatusCode, Option<String>) {
    let mut b = Request::get(uri);
    if let Some(ck) = &c.cookie {
        b = b.header("cookie", ck);
    }
    let (status, headers, _) = send(&c.app, b.body(Body::empty()).unwrap()).await;
    if let Some(sc) = headers.get("set-cookie") {
        c.cookie = Some(sc.to_str().unwrap().split(';').next().unwrap().to_string());
    }
    let loc = headers
        .get("location")
        .map(|v| v.to_str().unwrap().to_string());
    (status, loc)
}

fn params(url: &str) -> std::collections::HashMap<String, String> {
    reqwest::Url::parse(url)
        .unwrap()
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

fn rollups(points: &[(jiff::civil::Date, &str)]) -> ResponseTemplate {
    let pts: Vec<Value> = points
        .iter()
        .map(|(d, n)| {
            let (y, m, dd) = (d.year(), d.month(), d.day());
            json!({
                "civilStartTime": {"date": {"year": y, "month": m, "day": dd}},
                "civilEndTime": {"date": {"year": y, "month": m, "day": dd + 1}},
                "steps": {"countSum": n},
            })
        })
        .collect();
    ResponseTemplate::new(200).set_body_json(json!({ "rollupDataPoints": pts }))
}

async fn token_ok(g: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"access_token": "at-new", "refresh_token": "rt-new", "expires_in": 3599}),
        ))
        .mount(g)
        .await;
}

const EXERCISE: &str = "/v4/users/me/dataTypes/exercise/dataPoints";

/// One Google exercise data point starting at `hour`:00 (profile timezone) on `d`.
fn session(id: &str, kind: &str, d: jiff::civil::Date, hour: i8, mins: i64) -> Value {
    let start = d.at(hour, 0, 0, 0).in_tz(TZ).unwrap().timestamp();
    let end = start + jiff::SignedDuration::from_secs(mins * 60 + 120);
    json!({
        "name": format!("users/me/dataTypes/exercise/dataPoints/{id}"),
        "exercise": {
            "interval": {"startTime": start.to_string(), "endTime": end.to_string()},
            "exerciseType": kind,
            "displayName": format!("{kind} session"),
            "activeDuration": format!("{}s", mins * 60),
            "metricsSummary": {
                "caloriesKcal": 412.0, "distanceMillimeters": 5_200_000.0,
                "averageHeartRateBeatsPerMinute": "148"
            },
        }
    })
}

fn page(points: Vec<Value>, next: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({"dataPoints": points, "nextPageToken": next}))
}

async fn steps_empty(g: &MockServer) {
    Mock::given(method("POST"))
        .and(path(format!("/v4{ROLLUP}")))
        .respond_with(rollups(&[]))
        .mount(g)
        .await;
}

async fn exercise(g: &MockServer, points: Vec<Value>) {
    Mock::given(method("GET"))
        .and(path(EXERCISE))
        .respond_with(page(points, ""))
        .mount(g)
        .await;
}

async fn synced(a: &mut Client) -> Value {
    let (status, body) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

fn range() -> String {
    format!("/api/workouts?from={}&to={}", day(-13), day(0))
}
