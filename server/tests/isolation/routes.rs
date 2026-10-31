// SPDX-License-Identifier: AGPL-3.0-or-later
//! The route-coverage guarantee: every user route is listed here, and every route demands the
//! right caller.
use axum::http::StatusCode;
use serde_json::json;

use crate::common::*;
use cupful::Access;

/// Every `Access::User` route, each covered by a test in this target. Adding a route without
/// adding it here fails.
const COVERED: &[&str] = &[
    "GET /api/profile",
    "PUT /api/profile",
    "POST /api/sync",
    "GET /api/days",
    "GET /api/days/{date}",
    "GET /api/meals",
    "GET /api/budget/{date}",
    "POST /api/estimate",
    "POST /api/foods",
    "GET /api/aliases",
    "POST /api/aliases",
    "DELETE /api/aliases/{id}",
    "POST /api/portions",
    "GET /api/products",
    "POST /api/products",
    "GET /api/products/{id}",
    "DELETE /api/products/{id}",
    "GET /api/ingest-tokens",
    "POST /api/ingest-tokens",
    "DELETE /api/ingest-tokens/{id}",
    "GET /api/trends",
    "GET /api/export.csv",
    "POST /api/import/fuel-ledger",
    "GET /api/backup",
    "POST /api/restore",
    "GET /api/google",
    "DELETE /api/google",
    "GET /api/google/connect",
    "GET /api/google/callback",
    "POST /api/google/sync",
    "PUT /api/google/gym-types",
    "GET /api/workouts",
];

#[test]
fn every_user_route_has_an_isolation_test() {
    for (method, path, access) in cupful::route_table() {
        let key = format!("{method} {path}");
        if access == Access::User {
            assert!(
                COVERED.contains(&key.as_str()),
                "{key} has no isolation test"
            );
        }
    }
}

#[tokio::test]
async fn user_routes_need_a_login() {
    let (app, _) = app().await;
    admin(&app).await;
    let mut anon = Client::new(&app);
    for (method, path, access) in cupful::route_table() {
        if access != Access::User {
            continue;
        }
        let uri = path
            .replace("{date}", "2026-01-01")
            .replace("{id}", "96385074")
            + if path == "/api/meals" {
                "?date=2026-01-01"
            } else if path == "/api/days" || path == "/api/workouts" || path == "/api/trends" {
                "?from=2026-01-01&to=2026-01-02"
            } else {
                ""
            };
        let body = (method != "GET").then(|| json!({}));
        let (status, _) = anon.req(method, &uri, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
    }
}

#[tokio::test]
async fn admin_routes_refuse_other_users_and_anonymous() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let (_, p) = a
        .post(
            "/api/ai/providers",
            json!({"label": "x", "kind": "openai_compatible", "base_url": "http://x/v1", "model": "m"}),
        )
        .await;
    let id = p["id"].to_string();
    let mut bob = user(&app, &state, "bob").await;
    let mut anon = Client::new(&app);
    let admin_routes: Vec<_> = cupful::route_table()
        .into_iter()
        .filter(|r| r.2 == Access::Admin)
        .collect();
    assert!(!admin_routes.is_empty());
    for (method, path, _) in admin_routes {
        let uri = path.replace("{id}", &id);
        let body = (method != "GET" && method != "DELETE").then(|| json!({}));
        let (status, _) = bob.req(method, &uri, body.clone()).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}");
        let (status, _) = anon.req(method, &uri, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
    }
    let (_, list) = a.get("/api/ai/providers").await;
    assert_eq!(
        list["providers"].as_array().unwrap().len(),
        1,
        "bob deleted nothing"
    );
}
