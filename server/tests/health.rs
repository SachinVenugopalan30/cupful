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
