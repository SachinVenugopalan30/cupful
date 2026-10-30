// SPDX-License-Identifier: AGPL-3.0-or-later
//! User A must never read, change or delete user B's data (PLAN.md §10).
//!
//! `routes` holds the coverage guarantee that every route has a test here. `users`, `foods`,
//! `products` and `tokens` hold the tests themselves, one group of resources each.
/// Shared helpers live at tests/common/mod.rs, outside this target directory.
#[path = "../common/mod.rs"]
mod common;

mod foods;
mod products;
mod routes;
mod tokens;
mod users;

use common::*;
use cupful::AppState;
use serde_json::json;

pub(crate) const D: &str = "2026-01-05";

/// A has a profile, a meal and a day on `D`; B is a second, logged-in user.
pub(crate) async fn two_users() -> (Client, Client, String, String) {
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

pub(crate) async fn a_is_untouched(a: &mut Client, meal_id: &str) {
    let (_, m) = a.get(&format!("/api/meals?date={D}")).await;
    assert_eq!(m["meals"][0]["id"], meal_id);
    assert_eq!(m["meals"][0]["totals"]["kcal"], 500.0);
    let (_, day) = a.get(&format!("/api/days/{D}")).await;
    assert_eq!(day["weight_kg"], 81.0);
    assert_eq!(day["steps"], 7000);
}

/// What `user_id`'s estimate of `food` matched (the pipeline `POST /api/estimate` runs), if anything.
pub(crate) async fn matched(db: &sqlx::SqlitePool, user_id: i64, food: &str) -> Option<i64> {
    let reply = json!({"items": [{"food": food, "amount": 1, "unit": "", "grams": 100,
        "cooking": "none", "mentions_oil": false, "estimate": null}]});
    let est = cupful::nutrition::estimate(db, user_id, &FixedReply(reply), food, vec![])
        .await
        .unwrap();
    est.items[0].food_id
}

pub(crate) async fn id_of(state: &AppState, name: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
        .bind(name)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

pub(crate) fn label(name: &str) -> serde_json::Value {
    json!({"name": name, "brand": null, "barcode": "96385074", "basis": "100g",
           "kcal": 400.0, "protein_g": 5.0, "carbs_g": 70.0, "fat_g": 10.0,
           "serving_g": 20.0, "serving_text": null})
}
