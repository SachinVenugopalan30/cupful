// SPDX-License-Identifier: AGPL-3.0-or-later
//! Profiles, meals, days, budgets and trends.
use axum::http::StatusCode;
use serde_json::json;

use crate::common::*;
use crate::{D, a_is_untouched, two_users};

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

#[tokio::test]
async fn b_cannot_read_a_trends() {
    let (_, mut b, _, _) = two_users().await;
    let (_, t) = b.get("/api/trends?from=2026-01-04&to=2026-01-06").await;
    let days = t["days"].as_array().unwrap();
    assert_eq!(days.len(), 3);
    for d in days {
        for k in ["eaten_kcal", "budget_kcal", "weight_kg", "steps"] {
            assert!(d[k].is_null(), "{k}");
        }
    }
}
