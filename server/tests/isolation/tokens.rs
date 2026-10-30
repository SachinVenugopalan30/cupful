// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ingest tokens and the token-authenticated route.
use axum::http::StatusCode;
use serde_json::json;

use crate::common::*;
use crate::two_users;
use cupful::Access;

#[tokio::test]
async fn b_cannot_list_or_revoke_a_tokens() {
    let (mut a, mut b, _, _) = two_users().await;
    let (id, _) = a.token("iPhone").await;
    let (_, list) = b.get("/api/ingest-tokens").await;
    assert_eq!(list["tokens"], json!([]));
    let (status, _) = b
        .req("DELETE", &format!("/api/ingest-tokens/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, list) = a.get("/api/ingest-tokens").await;
    assert_eq!(list["tokens"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn token_routes_need_a_valid_token_and_stay_with_its_owner() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let mut b = user(&app, &state, "bob").await;
    let (_, ta) = a.token("a").await;
    let (id_b, tb) = b.token("b").await;
    let day = today("Asia/Kolkata").to_string();
    let body = json!({"date": day, "steps": 1234});
    let mut routes = 0;
    for (method, path, access) in cupful::route_table() {
        if access != Access::Token {
            continue;
        }
        routes += 1;
        assert_eq!((method, path), ("POST", "/api/ingest/activity"));
        let (status, _) = ingest(&app, None, body.clone()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _) = ingest(&app, Some("cupful_unknown"), body.clone()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        // A's token writes only A's day.
        let (status, _) = ingest(&app, Some(&ta), body.clone()).await;
        assert_eq!(status, StatusCode::OK);
        let (_, got) = b.get(&format!("/api/days/{day}")).await;
        assert_eq!(got["steps"], json!(null));
        let (_, got) = a.get(&format!("/api/days/{day}")).await;
        assert_eq!(got["steps"], 1234);
        // B's token stops working once B revokes it.
        let uri = format!("/api/ingest-tokens/{id_b}");
        b.req("DELETE", &uri, None).await;
        let (status, _) = ingest(&app, Some(&tb), body.clone()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(routes, 1);
}
