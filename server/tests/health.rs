// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use axum::http::StatusCode;
use common::*;

#[tokio::test]
async fn health_reports_version_and_source_url() {
    let (app, _) = app().await;
    let (status, _, body) = send(&app, get("/api/health")).await;
    assert_eq!(status, StatusCode::OK);
    let v = json(&body);
    assert_eq!(v["status"], "ok");
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        v["source_url"],
        "https://github.com/SachinVenugopalan30/cupful"
    );
}

#[tokio::test]
async fn unknown_api_path_is_json_404_not_the_spa() {
    let (app, _) = app().await;
    let (status, headers, _) = send(&app, get("/api/nope")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(headers["content-type"], "application/json");
}

#[tokio::test]
async fn unknown_page_path_serves_spa_index() {
    let (app, _) = app().await;
    let (status, headers, body) = send(&app, get("/today")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert!(String::from_utf8_lossy(&body).contains("<html"));
}

#[tokio::test]
async fn spa_responses_carry_security_headers() {
    let (app, _) = app().await;
    let (_, h, _) = send(&app, get("/today")).await;
    assert_eq!(h["x-content-type-options"], "nosniff");
    assert_eq!(h["referrer-policy"], "same-origin");
    assert_eq!(h["x-frame-options"], "DENY");
    let csp = h["content-security-policy"].to_str().unwrap();
    assert!(
        csp.contains("frame-ancestors 'none'") && csp.contains("'sha256-"),
        "{csp}"
    );
}

#[tokio::test]
async fn malformed_bodies_and_queries_get_json_errors() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let req = axum::http::Request::post("/api/sync")
        .header("x-cupful", "1")
        .header("content-type", "application/json")
        .header("cookie", a.cookie.clone().unwrap())
        .body(axum::body::Body::from("{not json"))
        .unwrap();
    let (status, headers, body) = send(&app, req).await;
    assert!(status.is_client_error());
    assert_eq!(headers["content-type"], "application/json");
    assert!(json(&body)["error"].is_string());

    let (status, body) = a.get("/api/meals").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].is_string(), "{body}");
}
