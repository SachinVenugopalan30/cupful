// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use serde_json::json;

#[tokio::test]
async fn state_reports_setup_needed_until_an_admin_exists() {
    let (app, _) = app().await;
    let mut anon = Client::new(&app);
    let (_, s) = anon.get("/api/auth/state").await;
    assert_eq!(s, json!({"needs_setup": true, "user": null}));

    let mut a = admin(&app).await;
    let (_, s) = a.get("/api/auth/state").await;
    assert_eq!(s["needs_setup"], false);
    assert_eq!(s["user"]["username"], "admin");
    assert_eq!(s["user"]["is_admin"], true);
}

#[tokio::test]
async fn setup_only_works_once() {
    let (app, _) = app().await;
    admin(&app).await;
    let (status, _) = Client::new(&app)
        .post(
            "/api/auth/setup",
            json!({"username": "eve", "password": PASSWORD, "timezone": "UTC"}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn simultaneous_setups_create_exactly_one_admin() {
    let (app, state) = app().await;
    let body = |n: &str| json!({"username": n, "password": PASSWORD, "timezone": "UTC"});
    let (mut a, mut b) = (Client::new(&app), Client::new(&app));
    let (ra, rb) = tokio::join!(
        a.post("/api/auth/setup", body("a")),
        b.post("/api/auth/setup", body("b"))
    );
    let mut statuses = [ra.0, rb.0];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CONFLICT]);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn setup_validates_input() {
    let (app, _) = app().await;
    let mut c = Client::new(&app);
    for body in [
        json!({"username": "admin", "password": "short", "timezone": "UTC"}),
        json!({"username": "admin", "password": PASSWORD, "timezone": "Mars/Olympus"}),
        json!({"username": "", "password": PASSWORD, "timezone": "UTC"}),
        json!({"username": "has space", "password": PASSWORD, "timezone": "UTC"}),
    ] {
        let (status, _) = c.post("/api/auth/setup", body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
}

#[tokio::test]
async fn login_checks_password_and_rotates_the_session_id() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let setup_cookie = a.cookie.clone().unwrap();

    let (status, _) = a
        .post(
            "/api/auth/login",
            json!({"username": "admin", "password": "wrong password"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = a
        .post(
            "/api/auth/login",
            json!({"username": "ADMIN", "password": PASSWORD}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(
        a.cookie.clone().unwrap(),
        setup_cookie,
        "login must issue a new session id"
    );

    // The pre-login session id no longer works.
    let mut old = Client {
        app: app.clone(),
        cookie: Some(setup_cookie),
    };
    let (_, s) = old.get("/api/auth/state").await;
    assert_eq!(s["user"], json!(null));
}

#[tokio::test]
async fn unknown_user_gets_the_same_error_as_a_wrong_password() {
    let (app, _) = app().await;
    admin(&app).await;
    let (status, body) = Client::new(&app)
        .post(
            "/api/auth/login",
            json!({"username": "nobody", "password": PASSWORD}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "invalid username or password");
}

#[tokio::test]
async fn login_is_rate_limited_per_username() {
    let (app, _) = app().await;
    admin(&app).await;
    let mut c = Client::new(&app);
    for _ in 0..5 {
        let (status, _) = c
            .post(
                "/api/auth/login",
                json!({"username": "admin", "password": "nope nope"}),
            )
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, _) = c
        .post(
            "/api/auth/login",
            json!({"username": "admin", "password": PASSWORD}),
        )
        .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn logout_ends_the_session() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let cookie = a.cookie.clone();
    let (status, _) = a.post("/api/auth/logout", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let mut replay = Client {
        app: app.clone(),
        cookie,
    };
    let (_, s) = replay.get("/api/auth/state").await;
    assert_eq!(s["user"], json!(null));
}

#[tokio::test]
async fn mutating_requests_need_the_csrf_header() {
    let (app, _) = app().await;
    let req = Request::post("/api/auth/setup")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"username": "admin", "password": PASSWORD, "timezone": "UTC"}).to_string(),
        ))
        .unwrap();
    let (status, _, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn session_cookie_is_httponly_secure_and_lax() {
    let (app, _) = app().await;
    let req = Request::post("/api/auth/setup")
        .header("content-type", "application/json")
        .header("x-cupful", "1")
        .body(Body::from(
            json!({"username": "admin", "password": PASSWORD, "timezone": "UTC"}).to_string(),
        ))
        .unwrap();
    let (_, headers, _) = send(&app, req).await;
    let cookie = headers["set-cookie"].to_str().unwrap();
    for attr in ["HttpOnly", "Secure", "SameSite=Lax", "Max-Age="] {
        assert!(cookie.contains(attr), "{cookie} lacks {attr}");
    }
}

#[tokio::test]
async fn reset_password_replaces_the_hash_and_ends_sessions() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    cupful::auth::reset_password(&state.db, "admin", "a brand new password")
        .await
        .unwrap();
    let (_, s) = a.get("/api/auth/state").await;
    assert_eq!(s["user"], json!(null), "old sessions must end");
    let (status, _) = a
        .post(
            "/api/auth/login",
            json!({"username": "admin", "password": PASSWORD}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = a
        .post(
            "/api/auth/login",
            json!({"username": "admin", "password": "a brand new password"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        cupful::auth::reset_password(&state.db, "ghost", "whatever123")
            .await
            .is_err()
    );
}
