// SPDX-License-Identifier: AGPL-3.0-or-later
//! Google Health: OAuth connect, steps sync and disconnect against a mocked Google (PLAN.md §15).
mod common;
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

#[tokio::test]
async fn hidden_when_not_configured() {
    let (app, _) = common::app().await;
    let mut a = admin(&app).await;
    let (status, body) = a.get("/api/google").await;
    assert_eq!(
        (status, body),
        (StatusCode::OK, json!({"configured": false}))
    );
    for (m, p) in [
        ("GET", "/api/google/connect"),
        ("GET", "/api/google/callback?code=x&state=y"),
        ("POST", "/api/google/sync"),
        ("DELETE", "/api/google"),
    ] {
        let (status, _) = a.req(m, p, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{m} {p}");
    }
}

#[tokio::test]
async fn connect_redirects_with_pkce_state_scope_and_offline() {
    let (g, _, _, mut a) = setup().await;
    let (status, loc) = nav(&mut a, "/api/google/connect").await;
    assert!(status.is_redirection(), "{status}");
    let loc = loc.unwrap();
    assert!(loc.starts_with(&format!("{}/auth?", g.uri())));
    let p = params(&loc);
    assert_eq!(p["client_id"], "cid.apps.googleusercontent.com");
    assert_eq!(p["redirect_uri"], "https://cupful.test/api/google/callback");
    assert_eq!(p["response_type"], "code");
    assert_eq!(p["scope"], cupful::google::SCOPE);
    assert_eq!(p["code_challenge_method"], "S256");
    assert_eq!(p["code_challenge"].len(), 43);
    assert_eq!(p["access_type"], "offline");
    assert_eq!(p["prompt"], "consent");
    assert!(p["state"].len() >= 32);
    assert!(!loc.contains("shh-secret"));
}

#[tokio::test]
async fn callback_rejects_missing_or_wrong_state() {
    let (g, _, _, mut a) = setup().await;
    token_ok(&g).await;
    // No sign-in started.
    let (status, _) = a.get("/api/google/callback?code=c&state=s").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Started, but the state differs.
    nav(&mut a, "/api/google/connect").await;
    let (status, _) = a.get("/api/google/callback?code=c&state=forged").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // A failed attempt used up the pending sign-in: even the right state is gone.
    let (status, _) = a.get("/api/google/callback?code=c").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, st) = a.get("/api/google").await;
    assert_eq!(st["connected"], false);
    assert!(g.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn callback_exchanges_code_with_verifier_and_stores_encrypted_tokens() {
    let (g, _, state, mut a) = setup().await;
    token_ok(&g).await;
    let (_, loc) = nav(&mut a, "/api/google/connect").await;
    let p = params(&loc.unwrap());
    let (status, loc) = nav(
        &mut a,
        &format!("/api/google/callback?code=the-code&state={}", p["state"]),
    )
    .await;
    assert!(status.is_redirection());
    assert_eq!(loc.unwrap(), "/settings?google=connected");

    let reqs = g.received_requests().await.unwrap();
    let body = String::from_utf8(reqs[0].body.clone()).unwrap();
    let form: std::collections::HashMap<String, String> =
        reqwest::Url::parse(&format!("http://x/?{body}"))
            .unwrap()
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
    assert_eq!(form["grant_type"], "authorization_code");
    assert_eq!(form["code"], "the-code");
    assert_eq!(form["client_secret"], "shh-secret");
    assert_eq!(
        form["redirect_uri"],
        "https://cupful.test/api/google/callback"
    );
    // The verifier hashes to the challenge sent in the first leg.
    assert_eq!(
        B64URL.encode(Sha256::digest(form["code_verifier"].as_bytes())),
        p["code_challenge"]
    );

    let (refresh, access): (String, String) =
        sqlx::query_as("SELECT refresh_token, access_token FROM google_connections")
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert!(refresh.starts_with("v1:") && !refresh.contains("rt-new"));
    assert!(access.starts_with("v1:") && !access.contains("at-new"));
    assert_eq!(state.secrets.decrypt(&refresh).unwrap(), "rt-new");

    let (_, st) = a.get("/api/google").await;
    assert_eq!(st["configured"], true);
    assert_eq!(st["connected"], true);
    assert_eq!(st["needs_reconnect"], false);

    // The state is one-time.
    let (status, _) = a
        .get(&format!("/api/google/callback?code=x&state={}", p["state"]))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn callback_uses_the_redirect_uri_from_connect_not_the_callback_host() {
    let g = MockServer::start().await;
    let (app, _) = app_with(Config {
        product_lookup: false,
        google_client_id: Some("cid".into()),
        google_client_secret: Some(Secret::new("shh-secret".into())),
        google_auth_url: format!("{}/auth", g.uri()),
        google_token_url: format!("{}/token", g.uri()),
        ..Config::default()
    })
    .await;
    let mut a = admin(&app).await;
    token_ok(&g).await;
    let get = |uri: String, host: &'static str, ck: Option<String>| {
        let mut b = Request::get(uri).header("host", host);
        if let Some(ck) = ck {
            b = b.header("cookie", ck);
        }
        b.body(Body::empty()).unwrap()
    };
    let (_, h, _) = send(
        &app,
        get("/api/google/connect".into(), "a.test", a.cookie.clone()),
    )
    .await;
    let p = params(h.get("location").unwrap().to_str().unwrap());
    assert!(p["redirect_uri"].contains("a.test"));
    if let Some(sc) = h.get("set-cookie") {
        a.cookie = Some(sc.to_str().unwrap().split(';').next().unwrap().to_string());
    }
    let uri = format!("/api/google/callback?code=c&state={}", p["state"]);
    send(&app, get(uri, "b.test", a.cookie.clone())).await;
    let body = String::from_utf8(g.received_requests().await.unwrap()[0].body.clone()).unwrap();
    let sent = &params(&format!("http://x/?{body}"))["redirect_uri"];
    assert_eq!(sent, &p["redirect_uri"]);
}

#[tokio::test]
async fn sync_maps_rollups_to_dates_and_writes_source_google() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    let (y, t) = (day(-1), day(0));
    Mock::given(method("POST"))
        .and(path(format!("/v4{ROLLUP}")))
        .and(header("authorization", "Bearer at-old"))
        .respond_with(rollups(&[(y, "8123"), (t, "4000")]))
        .expect(1)
        .mount(&g)
        .await;
    let (status, body) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({"days": 2, "changed": 2}));

    let (_, d) = a.get(&format!("/api/days/{y}")).await;
    assert_eq!(
        (d["steps"].clone(), d["steps_source"].clone()),
        (json!(8123), json!("google"))
    );
    let (_, d) = a.get(&format!("/api/days/{t}")).await;
    assert_eq!(d["steps"], 4000);

    // 14 civil days ending today, closed-open.
    let req = &g.received_requests().await.unwrap()[0];
    let sent: Value = serde_json::from_slice(&req.body).unwrap();
    let (s, e) = (day(-13), day(1));
    assert_eq!(
        sent["range"],
        json!({
            "start": {"date": {"year": s.year(), "month": s.month(), "day": s.day()}},
            "end": {"date": {"year": e.year(), "month": e.month(), "day": e.day()}},
        })
    );
    assert_eq!(sent["windowSizeDays"], 1);

    let (_, st) = a.get("/api/google").await;
    assert!(st["last_sync_at"].is_string());
    assert_eq!(st["last_error"], json!(null));
}

#[tokio::test]
async fn sync_never_overwrites_manual_steps() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    let y = day(-1);
    a.sync(vec![op(
        "day.patch",
        &at(-60),
        json!({"date": y.to_string(), "steps": 1234}),
    )])
    .await;
    Mock::given(method("POST"))
        .and(path(format!("/v4{ROLLUP}")))
        .respond_with(rollups(&[(y, "9999")]))
        .mount(&g)
        .await;
    let (_, body) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(body["changed"], 0);
    let (_, d) = a.get(&format!("/api/days/{y}")).await;
    assert_eq!(
        (d["steps"].clone(), d["steps_source"].clone()),
        (json!(1234), json!("manual"))
    );
}

#[tokio::test]
async fn expired_access_token_is_refreshed() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, -10).await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=rt-old"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"access_token": "at-fresh", "expires_in": 3599})),
        )
        .expect(1)
        .mount(&g)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/v4{ROLLUP}")))
        .and(header("authorization", "Bearer at-fresh"))
        .respond_with(rollups(&[(day(0), "10")]))
        .expect(2)
        .mount(&g)
        .await;
    let (status, body) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // The new token is stored (encrypted) and reused without another refresh.
    let (status, _) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn invalid_grant_marks_reconnect_and_stops_retrying() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, -10).await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(
            json!({"error": "invalid_grant", "error_description": "Token has been expired or revoked."}),
        ))
        .expect(1)
        .mount(&g)
        .await;
    let (status, body) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "reconnect");
    let (_, st) = a.get("/api/google").await;
    assert_eq!(st["needs_reconnect"], true);
    assert!(st["last_error"].as_str().unwrap().contains("reconnect"));
    // No second call to Google.
    let (status, _) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn missing_scope_asks_to_reconnect() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    Mock::given(method("POST"))
        .and(path(format!("/v4{ROLLUP}")))
        .respond_with(ResponseTemplate::new(403).set_body_json(
            json!({"error": {"status": "PERMISSION_DENIED", "details": [{"reason": "MISSING_OAUTH_SCOPE"}]}}),
        ))
        .mount(&g)
        .await;
    let (status, body) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(
        (status, &body["code"]),
        (StatusCode::CONFLICT, &json!("reconnect"))
    );
    let (_, st) = a.get("/api/google").await;
    assert_eq!(st["needs_reconnect"], true);
}

#[tokio::test]
async fn disconnect_revokes_and_deletes() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    Mock::given(method("POST"))
        .and(path("/revoke"))
        .and(body_string_contains("token=rt-old"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&g)
        .await;
    let (status, _) = a.req("DELETE", "/api/google", None).await;
    assert_eq!(status, StatusCode::OK);
    let (_, st) = a.get("/api/google").await;
    assert_eq!(st["connected"], false);
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM google_connections")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

// ---- isolation (PLAN.md §10) ----

#[tokio::test]
async fn b_cannot_see_touch_or_sync_a_connection() {
    let (g, app, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    let mut b = user(&app, &state, "bob").await;
    b.put("/api/profile", profile_body()).await;

    let (_, st) = b.get("/api/google").await;
    assert_eq!(
        (st["configured"].clone(), st["connected"].clone()),
        (json!(true), json!(false))
    );
    let (status, _) = b.post("/api/google/sync", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "bob has nothing to sync");
    let (status, _) = b.req("DELETE", "/api/google", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // B's callback can't use A's pending sign-in: sessions are separate.
    let (_, loc) = nav(&mut a, "/api/google/connect").await;
    let state_a = params(&loc.unwrap())["state"].clone();
    let (status, _) = b
        .get(&format!("/api/google/callback?code=x&state={state_a}"))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // B's connect works and issues its own state.
    let (status, loc) = nav(&mut b, "/api/google/connect").await;
    assert!(status.is_redirection());
    assert_ne!(params(&loc.unwrap())["state"], state_a);

    let (_, st) = a.get("/api/google").await;
    assert_eq!(st["connected"], true);
    assert!(g.received_requests().await.unwrap().is_empty());
}
