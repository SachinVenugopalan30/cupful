// SPDX-License-Identifier: AGPL-3.0-or-later
//! What a restore leaves alone: credentials and rows shared with other users.
use axum::http::StatusCode;
use serde_json::json;

use crate::common::*;
use crate::{export, restore, rich_data};

#[tokio::test]
async fn credentials_tokens_providers_and_google_survive() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&state, &mut a).await;
    let doc = export(&mut a).await;

    let (token_id, _) = a.token("phone").await;
    let (status, provider) = a
        .post(
            "/api/ai/providers",
            json!({"label": "local", "kind": "openai_compatible",
                   "base_url": "http://127.0.0.1:1/v1", "model": "m"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{provider}");
    let provider_id = provider["id"].as_i64().unwrap();
    sqlx::query(
        "INSERT INTO google_connections (user_id, refresh_token, connected_at)
         VALUES ((SELECT id FROM users ORDER BY id LIMIT 1), 'sealed', 1)",
    )
    .execute(&state.db)
    .await
    .unwrap();

    let (status, body) = restore(&mut a, &doc).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tokens: Vec<i64> = sqlx::query_scalar("SELECT id FROM ingest_tokens ORDER BY id")
        .fetch_all(&state.db)
        .await
        .unwrap();
    assert_eq!(tokens, [token_id]);
    let providers: Vec<i64> = sqlx::query_scalar("SELECT id FROM ai_providers ORDER BY id")
        .fetch_all(&state.db)
        .await
        .unwrap();
    assert_eq!(providers, [provider_id]);
    let google: i64 = sqlx::query_scalar("SELECT count(*) FROM google_connections")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(google, 1);
}

#[tokio::test]
async fn shared_and_label_rows_are_not_modified() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let f = rich_data(&state, &mut a).await;
    let doc = export(&mut a).await;

    let snapshot = |id: i64| {
        let db = state.db.clone();
        async move {
            sqlx::query_as::<_, (i64, String, Option<i64>, Option<String>, f64, String)>(
                "SELECT id, source, user_id, source_ref, kcal, name FROM foods WHERE id = ?",
            )
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap()
        }
    };
    let label_before = snapshot(f.label_food).await;
    let staple_before = snapshot(f.staple).await;
    let shared_count: i64 = sqlx::query_scalar("SELECT count(*) FROM foods WHERE user_id IS NULL")
        .fetch_one(&state.db)
        .await
        .unwrap();

    let (status, body) = restore(&mut a, &doc).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(snapshot(f.label_food).await, label_before);
    assert_eq!(snapshot(f.staple).await, staple_before);
    let shared_after: i64 = sqlx::query_scalar("SELECT count(*) FROM foods WHERE user_id IS NULL")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(shared_after, shared_count, "no new shared rows");
}
