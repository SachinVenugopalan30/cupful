// SPDX-License-Identifier: AGPL-3.0-or-later
//! Provider settings API, the daily cap and env seeding (PLAN.md §9).
mod common;
use std::collections::HashMap;

use axum::http::StatusCode;
use common::*;
use cupful::ai::{self, ProviderError, Purpose};
use cupful::{AppState, Config};
use serde_json::{Value, json};
use wiremock::matchers::{header, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const KEY: &str = "sk-settings-test-key-123456";

fn body(base_url: &str) -> Value {
    json!({
        "label": "Ollama", "kind": "openai_compatible", "base_url": base_url,
        "model": "llama3.2", "api_key": KEY, "extra_headers": {"x-extra": "hdr-secret"},
        "supports_vision": true, "is_local": false,
        "is_default_text": true, "is_default_vision": true
    })
}

#[tokio::test]
async fn crud_never_returns_secrets() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (status, created) = a
        .post("/api/ai/providers", body("http://localhost:11434/v1"))
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created["id"].as_i64().unwrap();

    let (_, list) = a.get("/api/ai/providers").await;
    let text = list.to_string();
    assert!(!text.contains(KEY) && !text.contains("hdr-secret") && !text.contains("v1:"));
    let p = &list["providers"][0];
    assert_eq!(p["has_key"], true);
    assert_eq!(p["has_extra_headers"], true);
    assert_eq!(p["is_default_text"], true);
    assert!(p.get("api_key").is_none());

    // Update without api_key keeps it; null clears it.
    let mut b = body("http://localhost:11434/v1");
    b.as_object_mut().unwrap().remove("api_key");
    b["label"] = json!("Renamed");
    let (status, _) = a.put(&format!("/api/ai/providers/{id}"), b.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let (_, list) = a.get("/api/ai/providers").await;
    assert_eq!(list["providers"][0]["label"], "Renamed");
    assert_eq!(list["providers"][0]["has_key"], true);
    b["api_key"] = Value::Null;
    a.put(&format!("/api/ai/providers/{id}"), b).await;
    let (_, list) = a.get("/api/ai/providers").await;
    assert_eq!(list["providers"][0]["has_key"], false);

    let (status, _) = a
        .req("DELETE", &format!("/api/ai/providers/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, list) = a.get("/api/ai/providers").await;
    assert_eq!(list["providers"], json!([]));
    let (status, _) = a
        .req("DELETE", &format!("/api/ai/providers/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn invalid_bodies_are_rejected() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    for (field, value) in [
        ("base_url", json!("ftp://x")),
        ("kind", json!("bedrock")),
        ("model", json!("")),
        ("extra_headers", json!({"bad header": "x"})),
        ("supports_vision", json!(false)), // default vision provider without vision
    ] {
        let mut b = body("http://localhost:11434/v1");
        b[field] = value;
        let (status, res) = a.post("/api/ai/providers", b).await;
        assert!(status.is_client_error(), "{field}: {status} {res}");
    }
}

#[tokio::test]
async fn setting_a_default_moves_it() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let (_, one) = a.post("/api/ai/providers", body("http://one/v1")).await;
    let (_, two) = a.post("/api/ai/providers", body("http://two/v1")).await;
    let (_, list) = a.get("/api/ai/providers").await;
    assert_eq!(list["providers"][0]["is_default_text"], false);
    assert_eq!(list["providers"][1]["is_default_text"], true);
    let (id, _) = ai::for_user(&state, 1, Purpose::Vision).await.unwrap();
    assert_eq!(id, two["id"].as_i64().unwrap());
    assert_ne!(id, one["id"].as_i64().unwrap());
}

#[tokio::test]
async fn by_id_decrypts_and_test_connection_reports_plainly() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/chat/completions"))
        .and(header("authorization", format!("Bearer {KEY}").as_str()))
        .and(header("x-extra", "hdr-secret"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"choices": [{"message": {"content": "{\"ok\": true}"}}]})),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_string(format!("bad key {KEY}")))
        .mount(&server)
        .await;
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (_, created) = a
        .post("/api/ai/providers", body(&format!("{}/v1", server.uri())))
        .await;
    let id = created["id"].as_i64().unwrap();
    let (status, ok) = a
        .post(&format!("/api/ai/providers/{id}/test"), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ok["ok"], true, "{ok}");
    let (_, bad) = a
        .post(&format!("/api/ai/providers/{id}/test"), json!({}))
        .await;
    assert_eq!(bad["ok"], false);
    assert_eq!(bad["code"], "provider_auth");
    assert!(!bad.to_string().contains(KEY), "{bad}");
    let (status, _) = a.post("/api/ai/providers/999/test", json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---- daily cap ----

async fn capped_app(cap: i64) -> (AppState, i64, i64) {
    let (app, state) = app_with(Config {
        ai_daily_cap: cap,
        product_lookup: false,
        ..Config::default()
    })
    .await;
    let mut a = admin(&app).await;
    let (_, hosted) = a.post("/api/ai/providers", body("http://hosted/v1")).await;
    let mut local = body("http://local/v1");
    local["is_local"] = json!(true);
    local["is_default_text"] = json!(false);
    let (_, local) = a.post("/api/ai/providers", local).await;
    (
        state,
        hosted["id"].as_i64().unwrap(),
        local["id"].as_i64().unwrap(),
    )
}

async fn calls(state: &AppState, user_id: i64) -> Vec<(String, i64, i64)> {
    sqlx::query_as("SELECT date, provider_id, calls FROM ai_usage WHERE user_id = ? ORDER BY date")
        .bind(user_id)
        .fetch_all(&state.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn hosted_calls_are_capped_per_user_per_local_date() {
    let (state, hosted, _) = capped_app(2).await;
    let bob = cupful::auth::create_user(&state.db, "bob", PASSWORD, "Pacific/Kiritimati", false)
        .await
        .unwrap();
    for user in [1, bob] {
        for _ in 0..2 {
            assert_eq!(
                ai::for_user(&state, user, Purpose::Text).await.unwrap().0,
                hosted
            );
        }
        assert_eq!(
            ai::for_user(&state, user, Purpose::Text).await.err(),
            Some(ProviderError::DailyCapReached)
        );
    }
    // Counted on each user's own local date (UTC+14 is often a day ahead of Asia/Kolkata).
    let bob_date = today("Pacific/Kiritimati").to_string();
    assert_eq!(calls(&state, bob).await, [(bob_date, hosted, 2)]);
    assert_eq!(
        calls(&state, 1).await,
        [(today("Asia/Kolkata").to_string(), hosted, 2)]
    );
    // Yesterday's usage doesn't count today.
    sqlx::query("UPDATE ai_usage SET date = '2000-01-01'")
        .execute(&state.db)
        .await
        .unwrap();
    assert!(ai::for_user(&state, bob, Purpose::Text).await.is_ok());
}

#[tokio::test]
async fn local_providers_are_not_capped() {
    let (state, _, local) = capped_app(1).await;
    sqlx::query("UPDATE ai_providers SET is_default_text = (id = ?)")
        .bind(local)
        .execute(&state.db)
        .await
        .unwrap();
    for _ in 0..5 {
        assert_eq!(
            ai::for_user(&state, 1, Purpose::Text).await.unwrap().0,
            local
        );
    }
}

#[tokio::test]
async fn no_default_provider_is_not_configured() {
    let (_, state) = app().await;
    assert_eq!(
        ai::for_user(&state, 1, Purpose::Text).await.err(),
        Some(ProviderError::NotConfigured)
    );
    assert_eq!(
        ai::by_id(&state, 7).await.err(),
        Some(ProviderError::NotConfigured)
    );
}

// ---- env seeding ----

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (format!("CUPFUL_PROVIDER_{k}"), v.to_string()))
        .collect();
    move |k| map.get(k).cloned()
}

#[tokio::test]
async fn env_seeds_an_empty_table_once_then_the_database_wins() {
    let (app, state) = app().await;
    let first = env(&[
        ("BASE_URL", "http://host.docker.internal:11434/v1"),
        ("MODEL", "llama3.2"),
        ("LOCAL", "true"),
        ("API_KEY", "sk-seeded-key-0000"),
    ]);
    ai::settings::seed_from_env(&state, &first).await.unwrap();
    ai::settings::seed_from_env(&state, &first).await.unwrap();
    let other = env(&[("BASE_URL", "http://elsewhere/v1"), ("MODEL", "other")]);
    ai::settings::seed_from_env(&state, &other).await.unwrap();

    let mut a = admin(&app).await;
    let (_, list) = a.get("/api/ai/providers").await;
    let ps = list["providers"].as_array().unwrap();
    assert_eq!(ps.len(), 1, "{list}");
    assert_eq!(ps[0]["base_url"], "http://host.docker.internal:11434/v1");
    assert_eq!(ps[0]["is_local"], true);
    assert_eq!(ps[0]["has_key"], true);
    assert_eq!(ps[0]["is_default_text"], true);
    assert_eq!(ps[0]["is_default_vision"], false);
}

#[tokio::test]
async fn env_seeding_needs_a_model_and_valid_values() {
    let (_, state) = app().await;
    assert!(
        ai::settings::seed_from_env(&state, env(&[("BASE_URL", "http://x/v1")]))
            .await
            .is_err()
    );
    let bad_kind = env(&[
        ("BASE_URL", "http://x/v1"),
        ("MODEL", "m"),
        ("KIND", "bedrock"),
    ]);
    assert!(ai::settings::seed_from_env(&state, bad_kind).await.is_err());
    ai::settings::seed_from_env(&state, env(&[])).await.unwrap();
}
