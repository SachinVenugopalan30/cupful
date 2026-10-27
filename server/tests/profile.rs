// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use axum::http::StatusCode;
use common::*;
use serde_json::json;

#[tokio::test]
async fn new_user_has_timezone_and_default_units_but_no_version() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (status, p) = a.get("/api/profile").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        p,
        json!({"timezone": "Asia/Kolkata", "weight_unit": "kg", "height_unit": "cm", "version": null})
    );
}

#[tokio::test]
async fn saving_creates_a_version_effective_today_and_same_day_saves_replace_it() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let (status, body) = a.put("/api/profile", profile_body()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mut b = profile_body();
    b["typical_steps"] = json!(9000);
    b["weight_unit"] = json!("lb");
    a.put("/api/profile", b).await;

    let (_, p) = a.get("/api/profile").await;
    assert_eq!(p["weight_unit"], "lb");
    assert_eq!(p["version"]["typical_steps"], 9000);
    assert_eq!(
        p["version"]["effective_from"],
        today("Asia/Kolkata").to_string()
    );
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM profile_versions")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn saving_keeps_earlier_versions() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    sqlx::query("INSERT INTO profile_versions (user_id, effective_from, sex, birth_year, height_cm, start_weight_kg, goal_kg, pace_kg_per_week, typical_steps) VALUES (1, '2020-01-01', 'male', 1990, 180, 90, NULL, 0.5, 5000)")
        .execute(&state.db).await.unwrap();
    a.put("/api/profile", profile_body()).await;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM profile_versions")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(n, 2);
}

#[tokio::test]
async fn profile_is_validated() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    for (field, bad) in [
        ("timezone", json!("Nowhere/Land")),
        ("sex", json!("other")),
        ("birth_year", json!(1800)),
        ("height_cm", json!(20)),
        ("start_weight_kg", json!(-5)),
        ("goal_kg", json!(1000)),
        ("pace_kg_per_week", json!(2.0)),
        ("typical_steps", json!(-1)),
        ("weight_unit", json!("stone")),
    ] {
        let mut b = profile_body();
        b[field] = bad;
        let (status, _) = a.put("/api/profile", b).await;
        assert!(status.is_client_error(), "{field} accepted");
    }
}
