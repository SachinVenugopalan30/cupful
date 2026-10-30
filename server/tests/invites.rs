// SPDX-License-Identifier: AGPL-3.0-or-later
//! Invite links: admin create/list/revoke, one-use acceptance, expiry, rate limits (PLAN.md §10).
mod common;
use common::*;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;

const NEW_USER: &str = "newbie";

async fn create_invite(c: &mut Client) -> (i64, String) {
    let (status, body) = c.post("/api/invites", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    (
        body["id"].as_i64().unwrap(),
        body["token"].as_str().unwrap().to_string(),
    )
}

fn accept_body(token: &str, username: &str) -> serde_json::Value {
    json!({
        "token": token, "username": username,
        "password": PASSWORD, "timezone": "Asia/Kolkata",
    })
}

async fn accept(c: &mut Client, token: &str, username: &str) -> (StatusCode, serde_json::Value) {
    c.post("/api/auth/accept-invite", accept_body(token, username))
        .await
}

#[tokio::test]
async fn admin_can_create_list_and_revoke() {
    let (app, _) = app().await;
    let mut admin = admin(&app).await;
    let (first, _) = create_invite(&mut admin).await;
    let (second, _) = create_invite(&mut admin).await;

    let (status, list) = admin.get("/api/invites").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["invites"].as_array().unwrap().len(), 2);

    let (status, _) = admin
        .req("DELETE", &format!("/api/invites/{second}"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, list) = admin.get("/api/invites").await;
    let ids: Vec<i64> = list["invites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, [first]);
    let (status, _) = admin
        .req("DELETE", &format!("/api/invites/{second}"), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn anonymous_and_non_admin_are_refused() {
    let (app, state) = app().await;
    let mut admin = admin(&app).await;
    let (id, _) = create_invite(&mut admin).await;
    let mut anon = Client::new(&app);
    let mut bob = user(&app, &state, "bob").await;
    let (status, _) = anon.get("/api/invites").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = anon.post("/api/invites", json!({})).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = anon
        .req("DELETE", &format!("/api/invites/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = bob.get("/api/invites").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = bob.post("/api/invites", json!({})).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = bob.req("DELETE", &format!("/api/invites/{id}"), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (_, list) = admin.get("/api/invites").await;
    assert_eq!(list["invites"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn list_never_returns_a_token() {
    let (app, _) = app().await;
    let mut admin = admin(&app).await;
    let (_, token) = create_invite(&mut admin).await;
    let (_, list) = admin.get("/api/invites").await;
    assert!(list["invites"][0].get("token").is_none());
    assert!(!list.to_string().contains(&token), "{list}");
}

#[tokio::test]
async fn token_hash_differs_from_the_token() {
    let (app, state) = app().await;
    let mut admin = admin(&app).await;
    let (id, token) = create_invite(&mut admin).await;
    let hash: String = sqlx::query_scalar("SELECT token_hash FROM invites WHERE id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_ne!(hash, token);
    assert_eq!(hash.len(), 64, "sha-256 hex");
    assert!(token.len() >= 43 && !token.contains('='), "{token}");
}

#[tokio::test]
async fn accept_creates_a_user_and_logs_in_once() {
    let (app, state) = app().await;
    let mut admin = admin(&app).await;
    let (id, token) = create_invite(&mut admin).await;
    let mut guest = Client::new(&app);
    let (status, body) = accept(&mut guest, &token, NEW_USER).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let user_id = body["id"].as_i64().unwrap();

    let (_, session) = guest.get("/api/auth/state").await;
    assert_eq!(session["user"]["id"], user_id);
    assert_eq!(session["user"]["is_admin"], false);
    let (_, profile) = guest.get("/api/profile").await;
    assert_eq!(profile["version"], json!(null));

    let (status, _) = accept(&mut guest, &token, "someone").await;
    assert_eq!(status, StatusCode::GONE);
    let invites: i64 = sqlx::query_scalar("SELECT count(*) FROM invites WHERE id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(invites, 1, "the row stays as history");
}

#[tokio::test]
async fn invite_row_records_used_by() {
    let (app, state) = app().await;
    let mut admin = admin(&app).await;
    let (id, token) = create_invite(&mut admin).await;
    let mut guest = Client::new(&app);
    let (status, body) = accept(&mut guest, &token, NEW_USER).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let used_by: Option<i64> = sqlx::query_scalar("SELECT used_by FROM invites WHERE id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(used_by, body["id"].as_i64());
}

#[tokio::test]
async fn revoked_invite_is_gone_and_revoke_of_used_is_404() {
    let (app, _) = app().await;
    let mut admin = admin(&app).await;
    let (id, token) = create_invite(&mut admin).await;
    let (status, _) = admin
        .req("DELETE", &format!("/api/invites/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let mut guest = Client::new(&app);
    let (status, _) = accept(&mut guest, &token, NEW_USER).await;
    assert_eq!(status, StatusCode::GONE);

    let (used, used_token) = create_invite(&mut admin).await;
    let (status, _) = accept(&mut guest, &used_token, "other").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = admin
        .req("DELETE", &format!("/api/invites/{used}"), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unknown_token_is_gone() {
    let (app, _) = app().await;
    admin(&app).await;
    let mut guest = Client::new(&app);
    let (status, _) = accept(&mut guest, "not-a-real-token", NEW_USER).await;
    assert_eq!(status, StatusCode::GONE);
}

#[tokio::test]
async fn expired_invite_is_gone_and_the_boundary_counts_as_expired() {
    let (app, state) = app().await;
    let mut admin = admin(&app).await;
    let (id, token) = create_invite(&mut admin).await;
    let now = jiff::Timestamp::now().as_millisecond();
    sqlx::query("UPDATE invites SET expires_at = ? WHERE id = ?")
        .bind(now)
        .bind(id)
        .execute(&state.db)
        .await
        .unwrap();
    let mut guest = Client::new(&app);
    let (status, _) = accept(&mut guest, &token, NEW_USER).await;
    assert_eq!(status, StatusCode::GONE, "expires_at > now is required");
}

#[tokio::test]
async fn username_taken_is_conflict_and_leaves_the_invite_unused() {
    let (app, state) = app().await;
    let mut admin = admin(&app).await;
    let (id, token) = create_invite(&mut admin).await;
    let mut guest = Client::new(&app);
    let (status, _) = accept(&mut guest, &token, "admin").await;
    assert_eq!(status, StatusCode::CONFLICT);
    let used_at: Option<i64> = sqlx::query_scalar("SELECT used_at FROM invites WHERE id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(used_at, None);
    let (status, _) = accept(&mut guest, &token, NEW_USER).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn accepts_need_the_csrf_header() {
    let (app, _) = app().await;
    let mut admin = admin(&app).await;
    let (_, token) = create_invite(&mut admin).await;
    let req = Request::post("/api/auth/accept-invite")
        .header("content-type", "application/json")
        .body(Body::from(accept_body(&token, NEW_USER).to_string()))
        .unwrap();
    let (status, _, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn identity_mode_refuses_create_and_accept() {
    let (app, _) = app_with(cupful::Config {
        trust_tailscale: true,
        ..cupful::Config::default()
    })
    .await;
    let mut admin = admin(&app).await;
    let (status, body) = admin.post("/api/invites", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let mut guest = Client::new(&app);
    let (status, body) = accept(&mut guest, "anything", NEW_USER).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

#[tokio::test]
async fn rate_limit_is_per_invite() {
    let (app, _) = app().await;
    let mut admin = admin(&app).await;
    let (_, token) = create_invite(&mut admin).await;
    let (_, other) = create_invite(&mut admin).await;
    let mut guest = Client::new(&app);
    for _ in 0..10 {
        // Invalid username, so no user is created; the attempt still counts.
        let (status, _) = accept(&mut guest, &token, "").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, body) = accept(&mut guest, &token, NEW_USER).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    let (status, _) = accept(&mut guest, &other, NEW_USER).await;
    assert_eq!(status, StatusCode::OK, "another invite has its own bucket");
}
