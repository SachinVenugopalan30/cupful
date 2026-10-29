// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use serde_json::json;

const TZ: &str = "Asia/Kolkata";

fn day(offset: i32) -> String {
    today(TZ)
        .checked_add(jiff::Span::new().days(offset))
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn token_is_shown_once_and_stored_hashed() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let (status, created) = a
        .post("/api/ingest-tokens", json!({"label": "iPhone"}))
        .await;
    assert_eq!(status, StatusCode::OK);
    let token = created["token"].as_str().unwrap();
    assert!(token.starts_with("cupful_") && token.len() > 40);
    assert_eq!(created["label"], "iPhone");
    assert!(created["created_at"].is_string());

    let stored: String = sqlx::query_scalar("SELECT token_hash FROM ingest_tokens")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert!(!stored.contains(token) && stored != token);

    let (_, list) = a.get("/api/ingest-tokens").await;
    let t = &list["tokens"][0];
    assert_eq!(t["id"], created["id"]);
    assert_eq!(t["label"], "iPhone");
    assert_eq!(t["last_used_at"], json!(null));
    assert!(t.get("token").is_none());
}

#[tokio::test]
async fn labels_are_1_to_100_chars() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    for label in ["".to_string(), "x".repeat(101)] {
        let (status, _) = a
            .post("/api/ingest-tokens", json!({ "label": label }))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, _) = a
        .post("/api/ingest-tokens", json!({"label": "x".repeat(100)}))
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn ingest_fills_steps_and_marks_the_source() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (_, token) = a.token("iPhone").await;
    let d = day(0);
    let (status, body) = ingest(
        &app,
        Some(&token),
        json!({"date": d, "steps": 8123, "gym_minutes": 45}),
    )
    .await;
    assert_eq!((status, body), (StatusCode::OK, json!({"changed": true})));
    let (_, got) = a.get(&format!("/api/days/{d}")).await;
    assert_eq!(got["steps"], 8123);
    assert_eq!(got["steps_source"], "shortcut");
    assert_eq!(got["gym_minutes"], 45);
    assert_eq!(got["gym_source"], "shortcut");
    let (_, list) = a.get("/api/ingest-tokens").await;
    assert!(list["tokens"][0]["last_used_at"].is_string());
}

#[tokio::test]
async fn manual_steps_are_kept() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (_, token) = a.token("iPhone").await;
    let d = day(0);
    a.sync(vec![op(
        "day.patch",
        &at(-60),
        json!({"date": d, "steps": 5000}),
    )])
    .await;
    let (status, body) = ingest(&app, Some(&token), json!({"date": d, "steps": 9000})).await;
    assert_eq!((status, body), (StatusCode::OK, json!({"changed": false})));
    let (_, got) = a.get(&format!("/api/days/{d}")).await;
    assert_eq!(got["steps"], 5000);
    assert_eq!(got["steps_source"], "manual");
}

#[tokio::test]
async fn bad_requests_are_refused() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (_, token) = a.token("iPhone").await;
    let t = Some(token.as_str());
    for body in [
        json!({"date": day(-15), "steps": 1}),
        json!({"date": day(2), "steps": 1}),
        json!({"date": day(0), "steps": 150_001}),
        json!({"date": day(0), "steps": -1}),
        json!({"date": day(0), "gym_minutes": 601}),
        json!({"date": day(0)}),
        json!({"date": "nope", "steps": 1}),
    ] {
        let (status, _) = ingest(&app, t, body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    // Only steps and gym_minutes are accepted from a token.
    let (status, _) = ingest(
        &app,
        t,
        json!({"date": day(0), "steps": 1, "weight_kg": 80}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // Shortcuts may send floats.
    let (status, _) = ingest(&app, t, json!({"date": day(0), "steps": 8123.4})).await;
    assert_eq!(status, StatusCode::OK);
    let (_, got) = a.get(&format!("/api/days/{}", day(0))).await;
    assert_eq!(got["steps"], 8123);
    for ok in [day(-14), day(1)] {
        let (status, _) = ingest(&app, t, json!({"date": ok, "steps": 1})).await;
        assert_eq!(status, StatusCode::OK, "{ok}");
    }
}

#[tokio::test]
async fn bodies_over_4kb_are_refused() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (_, token) = a.token("iPhone").await;
    let pad = "x".repeat(5000);
    let body = json!({"date": day(0), "steps": 1, "pad": pad});
    let (status, _) = ingest(&app, Some(&token), body).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn sixty_an_hour_per_token() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (_, t1) = a.token("one").await;
    let (_, t2) = a.token("two").await;
    let body = json!({"date": day(0), "steps": 1});
    for _ in 0..60 {
        let (status, _) = ingest(&app, Some(&t1), body.clone()).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, _) = ingest(&app, Some(&t1), body.clone()).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let (status, _) = ingest(&app, Some(&t2), body).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn missing_unknown_and_revoked_tokens_are_401() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (id, token) = a.token("iPhone").await;
    let body = json!({"date": day(0), "steps": 1});
    let (status, _) = ingest(&app, None, body.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = ingest(&app, Some("cupful_nope"), body.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = a
        .req("DELETE", &format!("/api/ingest-tokens/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = ingest(&app, Some(&token), body).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // A session cookie is not a token.
    let req = Request::post("/api/ingest/activity")
        .header("cookie", a.cookie.clone().unwrap())
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(send(&app, req).await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn csrf_exemption_is_only_for_ingest() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (_, token) = a.token("iPhone").await;
    let req = Request::post("/api/ingest-tokens")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(json!({"label": "x"}).to_string()))
        .unwrap();
    assert_eq!(send(&app, req).await.0, StatusCode::FORBIDDEN);
}
