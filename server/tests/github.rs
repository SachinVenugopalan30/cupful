// SPDX-License-Identifier: AGPL-3.0-or-later
//! The public GitHub link metadata: stars are fetched once an hour, failures included.
mod common;

use axum::http::StatusCode;
use common::*;
use cupful::Config;
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const REPO_PATH: &str = "/repos/SachinVenugopalan30/cupful";
const REPO_URL: &str = "https://github.com/SachinVenugopalan30/cupful";

async fn app_for(github: &MockServer) -> axum::Router {
    app_with(Config {
        github_api_url: github.uri(),
        ..Config::default()
    })
    .await
    .0
}

async fn link(app: &axum::Router) -> (StatusCode, Value) {
    let (status, _, bytes) = send(app, get("/api/github")).await;
    (status, json(&bytes))
}

#[tokio::test]
async fn stars_are_fetched_once_and_cached() {
    let github = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(REPO_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"stargazers_count": 7})))
        .expect(1)
        .mount(&github)
        .await;
    let app = app_for(&github).await;

    for _ in 0..2 {
        let (status, body) = link(&app).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body, json!({"url": REPO_URL, "stars": 7}));
    }
    github.verify().await;
}

#[tokio::test]
async fn a_failure_hides_the_count_and_is_cached() {
    let github = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(REPO_PATH))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&github)
        .await;
    let app = app_for(&github).await;

    for _ in 0..2 {
        let (status, body) = link(&app).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body, json!({"url": REPO_URL, "stars": null}));
    }
    github.verify().await;
}
