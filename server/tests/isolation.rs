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
    "POST /api/estimate",
    "POST /api/foods",
    "POST /api/aliases",
    "POST /api/portions",
    "GET /api/products",
    "POST /api/products",
    "GET /api/products/{id}",
    "DELETE /api/products/{id}",
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

// ---- private foods, aliases and portions (§8) ----

struct Reply(serde_json::Value);

#[async_trait::async_trait]
impl cupful::ai::Provider for Reply {
    async fn complete_json(
        &self,
        _: cupful::ai::JsonRequest,
    ) -> Result<serde_json::Value, cupful::ai::ProviderError> {
        Ok(self.0.clone())
    }
    fn capabilities(&self) -> cupful::ai::Capabilities {
        Default::default()
    }
}

/// What `user_id`'s estimate of `food` matched (the pipeline `POST /api/estimate` runs), if anything.
async fn matched(db: &sqlx::SqlitePool, user_id: i64, food: &str) -> Option<i64> {
    let reply = json!({"items": [{"food": food, "amount": 1, "unit": "", "grams": 100,
        "cooking": "none", "mentions_oil": false, "estimate": null}]});
    let est = cupful::nutrition::estimate(db, user_id, &Reply(reply), food, vec![])
        .await
        .unwrap();
    est.items[0].food_id
}

async fn id_of(state: &cupful::AppState, name: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
        .bind(name)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn b_never_matches_a_private_foods_aliases_or_portions() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let mut b = user(&app, &state, "bob").await;
    let (_, food) = a
        .post(
            "/api/foods",
            json!({"name": "Zebra cake", "kcal": 400, "protein_g": 5, "carbs_g": 60, "fat_g": 15}),
        )
        .await;
    let food_id = food["id"].as_i64().unwrap();
    let rice: i64 = sqlx::query_scalar("SELECT id FROM foods WHERE name = 'Rice, white, cooked'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    let (s1, _) = a
        .post("/api/aliases", json!({"alias": "zc", "food_id": food_id}))
        .await;
    let (s2, _) = a
        .post("/api/aliases", json!({"alias": "dinner", "food_id": rice}))
        .await;
    let (s3, _) = a
        .post(
            "/api/portions",
            json!({"phrase": "my cake", "food_id": food_id, "grams": 80}),
        )
        .await;
    assert_eq!([s1, s2, s3], [StatusCode::OK; 3]);
    let (ida, idb) = (id_of(&state, "admin").await, id_of(&state, "bob").await);
    for food in ["zebra cake", "zc", "my cake"] {
        assert_eq!(
            matched(&state.db, ida, food).await,
            Some(food_id),
            "A: {food}"
        );
    }
    assert_eq!(matched(&state.db, ida, "dinner").await, Some(rice));
    // B may match shipped foods (e.g. "zebra cake" → a cake staple), never A's private food or A's alias target.
    for food in ["zebra cake", "zc", "my cake"] {
        assert_ne!(
            matched(&state.db, idb, food).await,
            Some(food_id),
            "B: {food}"
        );
    }
    assert_ne!(
        matched(&state.db, idb, "dinner").await,
        Some(rice),
        "B: dinner"
    );
    // B can't alias, or save a portion of, A's private food.
    let (status, _) = b
        .post("/api/aliases", json!({"alias": "mine", "food_id": food_id}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = b
        .post(
            "/api/portions",
            json!({"phrase": "mine", "food_id": food_id, "grams": 50}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // B's own food and alias with A's names don't change what A matches.
    let (_, bfood) = b
        .post(
            "/api/foods",
            json!({"name": "Zebra cake", "kcal": 100, "protein_g": 1, "carbs_g": 20, "fat_g": 1}),
        )
        .await;
    b.post("/api/aliases", json!({"alias": "zc", "food_id": rice}))
        .await;
    assert_eq!(matched(&state.db, ida, "zc").await, Some(food_id));
    assert_eq!(matched(&state.db, ida, "zebra cake").await, Some(food_id));
    assert_eq!(
        matched(&state.db, idb, "zebra cake").await,
        bfood["id"].as_i64()
    );
}

fn label(name: &str) -> serde_json::Value {
    json!({"name": name, "brand": null, "barcode": "96385074", "basis": "100g",
           "kcal": 400.0, "protein_g": 5.0, "carbs_g": 70.0, "fat_g": 10.0,
           "serving_g": 20.0, "serving_text": null})
}

#[tokio::test]
async fn b_cannot_see_change_or_delete_a_saved_products() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let mut b = user(&app, &state, "bob").await;
    let (_, p) = a.post("/api/products", label("A's biscuit")).await;
    let (_, list) = b.get("/api/products").await;
    assert_eq!(list["products"], json!([]));
    let (status, _) = b
        .req("DELETE", &format!("/api/products/{}", p["food_id"]), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // B's own label entry for the same barcode is a new row; A's entry and list are unchanged.
    let (_, bobs) = b.post("/api/products", label("B's biscuit")).await;
    assert_ne!(bobs["food_id"], p["food_id"]);
    let (_, list) = a.get("/api/products").await;
    assert_eq!(list["products"].as_array().unwrap().len(), 1);
    assert_eq!(list["products"][0]["name"], "A's biscuit");
    let (_, found) = a.get("/api/products/96385074").await;
    assert_eq!(found["name"], "A's biscuit");
}

#[tokio::test]
async fn b_cannot_find_or_log_a_private_food() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let mut b = user(&app, &state, "bob").await;
    let food_id: i64 = sqlx::query_scalar(
        "INSERT INTO foods (source, source_ref, user_id, name, name_norm, barcode, kcal, protein_g,
             carbs_g, fat_g)
         SELECT 'user', id, id, 'secret', 'secret', '96385074', 100, 1, 1, 1
         FROM users WHERE username = 'admin' RETURNING id",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    let (status, _) = b.get("/api/products/96385074").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let d = today("Asia/Kolkata").to_string();
    let mut it = item(&uuid(), 100.0);
    it["food_id"] = json!(food_id);
    let o = || {
        op(
            "meal.upsert",
            &at(0),
            meal(&uuid(), &d, json!([it.clone()])),
        )
    };
    assert_eq!(b.sync(vec![o()]).await, ["rejected"]);
    let (_, list) = b.get("/api/products").await;
    assert_eq!(list["products"], json!([]));
    assert_eq!(a.sync(vec![o()]).await, ["applied"]);
}
