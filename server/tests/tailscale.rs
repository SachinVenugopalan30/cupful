// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use common::*;
use cupful::auth::tailscale::bind_tailscale;
use cupful::{AppState, Config};
use serde_json::{Value, json};

fn trusting() -> Config {
    Config {
        trust_tailscale: true,
        ..Config::default()
    }
}

async fn request(
    app: &Router,
    method: &str,
    uri: &str,
    login: Option<&str>,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Bytes) {
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("x-cupful", "1");
    if let Some(login) = login {
        b = b.header("tailscale-user-login", login);
    }
    if let Some(cookie) = cookie {
        b = b.header("cookie", cookie);
    }
    let req = match body {
        Some(v) => b
            .header("content-type", "application/json")
            .body(Body::from(v.to_string())),
        None => b.body(Body::empty()),
    }
    .unwrap();
    send(app, req).await
}

async fn state_json(
    app: &Router,
    uri: &str,
    login: Option<&str>,
    cookie: Option<&str>,
) -> (StatusCode, Value) {
    let (status, _, bytes) = request(app, "GET", uri, login, cookie, None).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn users(state: &AppState) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&state.db)
        .await
        .unwrap()
}

async fn login_of(state: &AppState, username: &str) -> Option<String> {
    sqlx::query_scalar("SELECT tailscale_login FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(&state.db)
        .await
        .unwrap()
        .flatten()
}

#[tokio::test]
async fn mode_off_does_not_authenticate_or_create_a_user() {
    let (app, state) = app().await;
    admin(&app).await;

    let (status, body) = state_json(&app, "/api/auth/state", Some("alice@example.com"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["identity_mode"], Value::Null);
    assert_eq!(body["user"], Value::Null);
    assert_eq!(users(&state).await, 1, "only the admin exists");
}

#[tokio::test]
async fn mode_on_creates_and_logs_in_a_non_admin_user() {
    let (app, state) = app_with(trusting()).await;
    admin(&app).await;

    let (status, body) = state_json(&app, "/api/auth/state", Some("Alice@Example.COM"), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["identity_mode"], "tailscale");
    assert_eq!(body["user"]["username"], "alice");
    assert_eq!(body["user"]["is_admin"], false);
    assert_eq!(
        login_of(&state, "alice").await.as_deref(),
        Some("alice@example.com")
    );

    let hash: Option<String> =
        sqlx::query_scalar("SELECT password_hash FROM users WHERE username = 'alice'")
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert_eq!(hash, None, "identity users have no password");
    let tz: String = sqlx::query_scalar(
        "SELECT p.timezone FROM profiles p JOIN users u ON u.id = p.user_id WHERE u.username = 'alice'",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(tz, "UTC");
}

#[tokio::test]
async fn mode_on_reuses_the_bound_user() {
    let (app, state) = app_with(trusting()).await;
    admin(&app).await;

    let (_, first) = state_json(&app, "/api/auth/state", Some("alice@example.com"), None).await;
    let (_, second) = state_json(&app, "/api/auth/state", Some("ALICE@example.com"), None).await;
    assert_eq!(first["user"]["id"], second["user"]["id"]);
    assert_eq!(users(&state).await, 2, "admin plus one mapped user");
}

#[tokio::test]
async fn an_authenticated_session_is_never_overridden() {
    let (app, state) = app_with(trusting()).await;
    let mut a = admin(&app).await;
    let cookie = a.cookie.clone().unwrap();

    let (_, body) = state_json(
        &app,
        "/api/auth/state",
        Some("mallory@example.com"),
        Some(&cookie),
    )
    .await;
    assert_eq!(body["user"]["username"], "admin");
    assert_eq!(body["user"]["is_admin"], true);
    assert_eq!(users(&state).await, 1);
    // The admin's session still works after the header request.
    let (status, body) = a.get("/api/auth/state").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["username"], "admin");
}

#[tokio::test]
async fn no_admin_means_no_mapping_and_setup_still_works() {
    let (app, state) = app_with(trusting()).await;

    let (status, body) = state_json(&app, "/api/auth/state", Some("alice@example.com"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["needs_setup"], true);
    assert_eq!(body["user"], Value::Null);
    assert_eq!(users(&state).await, 0);

    let (status, _, _) = request(
        &app,
        "POST",
        "/api/auth/setup",
        Some("alice@example.com"),
        None,
        Some(json!({"username": "admin", "password": PASSWORD, "timezone": "UTC"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(users(&state).await, 1);
    assert_eq!(
        login_of(&state, "admin").await,
        None,
        "setup is not a mapping"
    );
}

#[tokio::test]
async fn health_and_static_paths_create_nothing() {
    let (app, state) = app_with(trusting()).await;
    admin(&app).await;

    let (status, _, _) = request(
        &app,
        "GET",
        "/api/health",
        Some("alice@example.com"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, _, _) = request(&app, "GET", "/", Some("alice@example.com"), None, None).await;
    let (_, _, _) = request(&app, "GET", "/setup", Some("alice@example.com"), None, None).await;
    assert_eq!(
        users(&state).await,
        1,
        "no user from health or static paths"
    );
}

#[tokio::test]
async fn login_uniqueness_is_case_insensitive() {
    let (app, state) = app_with(trusting()).await;
    admin(&app).await;

    state_json(&app, "/api/auth/state", Some("bob@example.com"), None).await;
    state_json(&app, "/api/auth/state", Some("BOB@EXAMPLE.COM"), None).await;
    let n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE tailscale_login = 'bob@example.com'")
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn usernames_are_derived_and_collisions_get_a_suffix() {
    let (app, state) = app_with(trusting()).await;
    admin(&app).await;

    // "admin" is taken, so the second holder becomes "admin-2".
    let (_, body) = state_json(&app, "/api/auth/state", Some("Admin@Example.com"), None).await;
    assert_eq!(body["user"]["username"], "admin-2");
    assert_eq!(
        login_of(&state, "admin-2").await.as_deref(),
        Some("admin@example.com")
    );

    // Filtered characters and the empty fallback.
    let (_, body) = state_json(&app, "/api/auth/state", Some("no reply@example.com"), None).await;
    assert_eq!(body["user"]["username"], "noreply");
    let (_, body) = state_json(&app, "/api/auth/state", Some("!!!!@example.com"), None).await;
    assert_eq!(body["user"]["username"], "tailscale");
}

#[tokio::test]
async fn malformed_headers_are_ignored() {
    let (app, state) = app_with(trusting()).await;
    admin(&app).await;

    let too_long = "a".repeat(321);
    let (status, body) = state_json(&app, "/api/auth/state", Some(&too_long), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"], Value::Null);
    let (_, body) = state_json(&app, "/api/auth/state", Some("   "), None).await;
    assert_eq!(body["user"], Value::Null);
    assert_eq!(users(&state).await, 1);
}

#[test]
fn non_loopback_bind_fails_identity_validation() {
    use cupful::config::validate_identity;
    use std::net::SocketAddr;

    let mut c = Config {
        trust_tailscale: true,
        bind: "0.0.0.0:8080".parse::<SocketAddr>().unwrap(),
        ..Config::default()
    };
    assert!(validate_identity(&c).unwrap_err().contains("not loopback"));

    c.bind = "[::1]:8080".parse().unwrap();
    assert!(validate_identity(&c).is_ok());
    c.bind = "127.0.0.1:8080".parse().unwrap();
    assert!(validate_identity(&c).is_ok());

    c.bind = "0.0.0.0:8080".parse().unwrap();
    c.trust_tailscale = false;
    assert!(validate_identity(&c).is_ok());
    assert!(!Config::default().trust_tailscale);
}

#[tokio::test]
async fn bind_tailscale_sets_a_lowercased_login_and_reports_conflicts() {
    let (_, state) = app().await;
    cupful::auth::create_user(&state.db, "owner", PASSWORD, "UTC", true)
        .await
        .unwrap();
    cupful::auth::create_user(&state.db, "second", PASSWORD, "UTC", false)
        .await
        .unwrap();

    bind_tailscale(&state.db, "owner", "Owner@Example.COM")
        .await
        .unwrap();
    assert_eq!(
        login_of(&state, "owner").await.as_deref(),
        Some("owner@example.com")
    );

    // The same binding again is a no-op.
    bind_tailscale(&state.db, "owner", "OWNER@example.com")
        .await
        .unwrap();
    // One login per user.
    let err = bind_tailscale(&state.db, "owner", "other@example.com")
        .await
        .unwrap_err();
    assert!(err.contains("already bound"), "{err}");
    // One user per login.
    let err = bind_tailscale(&state.db, "second", "owner@example.com")
        .await
        .unwrap_err();
    assert!(err.contains("already bound to user \"owner\""), "{err}");
    // Unknown user and invalid login.
    let err = bind_tailscale(&state.db, "nobody", "x@example.com")
        .await
        .unwrap_err();
    assert!(err.contains("no user named"), "{err}");
    let err = bind_tailscale(&state.db, "second", "  ").await.unwrap_err();
    assert!(err.contains("invalid Tailscale login"), "{err}");
}
