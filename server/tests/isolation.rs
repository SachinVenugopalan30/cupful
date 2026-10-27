// SPDX-License-Identifier: AGPL-3.0-or-later
//! User A must never read, change or delete user B's data (PLAN.md §10).
mod common;
use axum::http::StatusCode;
use common::*;
use cupful::Access;
use serde_json::json;

/// Every `Access::User` route, each covered by a test below. Adding a route without adding it here fails.
const COVERED: &[&str] = &[
    "GET /api/profile",
    "PUT /api/profile",
    "POST /api/sync",
    "GET /api/days",
    "GET /api/days/{date}",
    "GET /api/meals",
    "GET /api/budget/{date}",
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
        let uri = path.replace("{date}", "2026-01-01")
            + if path == "/api/meals" {
                "?date=2026-01-01"
            } else if path == "/api/days" {
                "?from=2026-01-01&to=2026-01-02"
            } else {
                ""
            };
        let body = (method != "GET").then(|| json!({}));
        let (status, _) = anon.req(method, &uri, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
    }
}

const D: &str = "2026-01-05";

/// A has a profile, a meal and a day on `D`; B is a second, logged-in user.
async fn two_users() -> (Client, Client, String, String) {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let b = user(&app, &state, "bob").await;
    a.put("/api/profile", profile_body()).await;
    sqlx::query("UPDATE profile_versions SET effective_from = '2026-01-01'")
        .execute(&state.db)
        .await
        .unwrap();
    let (meal_id, item_id) = (uuid(), uuid());
    let res = a
        .sync(vec![
            op(
                "meal.upsert",
                &at(-60),
                meal(&meal_id, D, json!([item(&item_id, 500.0)])),
            ),
            op(
                "day.patch",
                &at(-60),
                json!({"date": D, "weight_kg": 81.0, "steps": 7000}),
            ),
        ])
        .await;
    assert_eq!(res, ["applied", "applied"]);
    (a, b, meal_id, item_id)
}

async fn a_is_untouched(a: &mut Client, meal_id: &str) {
    let (_, m) = a.get(&format!("/api/meals?date={D}")).await;
    assert_eq!(m["meals"][0]["id"], meal_id);
    assert_eq!(m["meals"][0]["totals"]["kcal"], 500.0);
    let (_, day) = a.get(&format!("/api/days/{D}")).await;
    assert_eq!(day["weight_kg"], 81.0);
    assert_eq!(day["steps"], 7000);
}

#[tokio::test]
async fn b_cannot_read_a_profile() {
    let (_, mut b, _, _) = two_users().await;
    let (_, p) = b.get("/api/profile").await;
    assert_eq!(p["version"], json!(null));
}

#[tokio::test]
async fn b_saving_a_profile_does_not_change_a() {
    let (mut a, mut b, _, _) = two_users().await;
    let mut body = profile_body();
    body["typical_steps"] = json!(1);
    body["weight_unit"] = json!("lb");
    b.put("/api/profile", body).await;
    let (_, p) = a.get("/api/profile").await;
    assert_eq!(p["weight_unit"], "kg");
    assert_eq!(p["version"]["typical_steps"], 8000);
}

#[tokio::test]
async fn b_cannot_read_a_meals_or_days() {
    let (_, mut b, _, _) = two_users().await;
    let (_, m) = b.get(&format!("/api/meals?date={D}")).await;
    assert_eq!(m["meals"], json!([]));
    let (_, day) = b.get(&format!("/api/days/{D}")).await;
    assert_eq!(day["weight_kg"], json!(null));
    let (_, days) = b.get("/api/days?from=2026-01-01&to=2026-01-31").await;
    assert_eq!(days["days"], json!([]));
}

#[tokio::test]
async fn b_cannot_read_a_budget() {
    let (_, mut b, _, _) = two_users().await;
    let (status, _) = b.get(&format!("/api/budget/{D}")).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "B has no profile of their own"
    );
}

#[tokio::test]
async fn b_cannot_overwrite_or_delete_a_meal() {
    let (mut a, mut b, meal_id, item_id) = two_users().await;
    let res = b
        .sync(vec![
            op(
                "meal.upsert",
                &at(0),
                meal(&meal_id, D, json!([item(&uuid(), 1.0)])),
            ),
            op("meal.delete", &at(0), json!({"id": meal_id})),
            // B's own meal can't adopt A's item id either.
            op(
                "meal.upsert",
                &at(0),
                meal(&uuid(), D, json!([item(&item_id, 1.0)])),
            ),
        ])
        .await;
    assert_eq!(res, ["rejected", "rejected", "rejected"]);
    a_is_untouched(&mut a, &meal_id).await;
}

#[tokio::test]
async fn b_day_patches_only_touch_b() {
    let (mut a, mut b, meal_id, _) = two_users().await;
    assert_eq!(
        b.sync(vec![op(
            "day.patch",
            &at(0),
            json!({"date": D, "weight_kg": null, "steps": 1})
        )])
        .await,
        ["applied"]
    );
    a_is_untouched(&mut a, &meal_id).await;
}
