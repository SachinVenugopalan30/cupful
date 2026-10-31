// SPDX-License-Identifier: AGPL-3.0-or-later
//! `cupful seed-demo`: a `demo` user with believable history, for screenshots and trying the
//! charts (PLAN.md §13). Same seed and day count, same rows (ids and timestamps included).

use jiff::ToSpan;
use jiff::civil::Date;
use sqlx::SqlitePool;

const USERNAME: &str = "demo";
const TIMEZONE: &str = "UTC";
const BREAKFASTS: &[&[&str]] = &[
    &["Oatmeal, cooked with water", "Milk, whole"],
    &["Egg, scrambled", "Toast, whole wheat"],
    &["Pancakes", "Milk, whole"],
    &["Greek yogurt, plain, whole milk", "Dates, medjool"],
    &["Egg, fried", "Toast, white bread"],
];
const LUNCHES: &[&[&str]] = &[
    &[
        "Rice, white, cooked",
        "Chickpeas (chana), boiled",
        "Tomato, raw",
    ],
    &[
        "Chicken breast, cooked (roasted)",
        "Quinoa, cooked",
        "Broccoli, boiled",
    ],
    &["Quinoa, cooked", "Chickpeas (chana), boiled", "Tomato, raw"],
    &["Pasta, cooked", "Chicken, minced, cooked"],
    &[
        "Rice, brown, cooked",
        "Egg, hard-boiled",
        "Broccoli, boiled",
    ],
];
const DINNERS: &[&[&str]] = &[
    &["Pasta, cooked", "Chicken, minced, cooked", "Tomato, raw"],
    &["Salmon, cooked", "Potato, boiled", "Green beans, boiled"],
    &[
        "Rice, white, cooked",
        "Chicken thigh, cooked (roasted)",
        "Cabbage, boiled",
    ],
    &["Tofu, firm", "Rice, brown, cooked", "Broccoli, boiled"],
    &["Beef steak (sirloin), cooked", "Sweet potato, boiled"],
];
const SNACKS: &[&[&str]] = &[
    &["Peanuts, roasted"],
    &["Popcorn, air-popped"],
    &["Dates, medjool"],
    &["Yogurt, plain low fat"],
];
/// Gym sessions rotate through these.
const SPLITS: &[&[&str]] = &[
    &["Chest", "Arms"],
    &["Back", "Shoulders"],
    &["Legs", "Core"],
];

/// SplitMix64: tiny, and fine for fake data.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// `lo..hi`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo) as u64) as i64
    }

    fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.next() as usize % items.len()]
    }
}

/// Creates the `demo` user with `days` days of history ending on `today` and returns their
/// password. Refuses if `demo` exists, unless `force`, which deletes that user (and so only their
/// data) first. The password is random, not derived from `seed`.
pub async fn seed(
    db: &SqlitePool,
    today: Date,
    days: u32,
    seed: u64,
    force: bool,
) -> Result<String, String> {
    seed_rows(db, today, days, seed, force)
        .await
        .map_err(|e| e.to_string())
}

type BoxErr = Box<dyn std::error::Error>;

async fn seed_rows(
    db: &SqlitePool,
    today: Date,
    days: u32,
    seed: u64,
    force: bool,
) -> Result<String, BoxErr> {
    let existing = sqlx::query_scalar!("SELECT id FROM users WHERE username = ?", USERNAME)
        .fetch_optional(db)
        .await?;
    if let Some(id) = existing {
        if !force {
            return Err(
                format!("user {USERNAME:?} already exists; pass --force to replace it").into(),
            );
        }
        // Sessions have no foreign key; the rest cascades.
        sqlx::query!("DELETE FROM sessions WHERE user_id = ?", id)
            .execute(db)
            .await?;
        sqlx::query!("DELETE FROM users WHERE id = ?", id)
            .execute(db)
            .await?;
    }
    // The first user on a fresh instance is the admin, so the demo can open Settings.
    let first = sqlx::query_scalar!("SELECT NOT EXISTS (SELECT 1 FROM users) AS \"v!: bool\"")
        .fetch_one(db)
        .await?;
    let password = crate::crypto::random_token()[..20].to_string();
    let user = crate::auth::create_user(db, USERNAME, &password, TIMEZONE, first)
        .await
        .map_err(|e| format!("{e:?}"))?;

    let mut rng = Rng(seed);
    let start = today.saturating_sub(i64::from(days.saturating_sub(1)).days());
    let mut tx = db.begin().await?;
    let start_s = start.to_string();
    sqlx::query!(
        "INSERT INTO profile_versions (user_id, effective_from, sex, birth_year, height_cm,
             start_weight_kg, goal_kg, pace_kg_per_week, typical_steps)
         VALUES (?, ?, 'male', 1990, 180.0, 85.0, 75.0, 0.5, 8000)",
        user,
        start_s
    )
    .execute(&mut *tx)
    .await?;
    let mut sessions = 0;
    for i in 0..i64::from(days) {
        let date = start.saturating_add(i.days());
        let at = date
            .at(12, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)?
            .timestamp()
            .as_millisecond();
        let d = date.to_string();

        if !rng.chance(1.0 / 15.0) {
            let mut meals = vec![("lunch", rng.pick(LUNCHES)), ("dinner", rng.pick(DINNERS))];
            if rng.chance(0.75) {
                meals.insert(0, ("breakfast", rng.pick(BREAKFASTS)));
            }
            if rng.chance(0.5) {
                meals.push(("snack", rng.pick(SNACKS)));
            }
            for (meal_type, foods) in meals {
                insert_meal(&mut tx, &mut rng, user, &d, meal_type, foods, at).await?;
            }
        }

        let gym_day =
            [0, 2, 4].contains(&date.weekday().to_monday_zero_offset()) && rng.chance(0.9);
        let weekend = date.weekday().to_monday_zero_offset() >= 5;
        let steps = (!rng.chance(0.12)).then(|| {
            let base = if weekend {
                rng.range(7000, 14000)
            } else {
                rng.range(5500, 10500)
            };
            base + if gym_day { 1500 } else { 0 }
        });
        let steps_source = if rng.chance(0.75) {
            "shortcut"
        } else {
            "manual"
        };
        let gym = gym_day.then(|| rng.range(40, 76));
        let trained = gym.map(|_| {
            sessions += 1;
            serde_json::to_string(SPLITS[(sessions - 1) % SPLITS.len()]).expect("strings serialize")
        });
        // About half a kilo a week down, plus morning noise.
        let weight = rng.chance(0.85).then(|| {
            crate::nutrition::r1(
                85.0 - 0.5 / 7.0 * i as f64 + (rng.unit() + rng.unit() - 1.0) * 0.6,
            )
        });
        if steps.is_none() && gym.is_none() && weight.is_none() {
            continue;
        }
        let steps_source = steps.map(|_| steps_source);
        let gym_source = gym.map(|_| "manual");
        let (steps_at, gym_at, weight_at) =
            (steps.map(|_| at), gym.map(|_| at), weight.map(|_| at));
        sqlx::query!(
            "INSERT INTO days (user_id, date, steps, steps_source, steps_updated_at, gym_minutes,
                 gym_source, gym_updated_at, trained, trained_updated_at, weight_kg, weight_updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            user,
            d,
            steps,
            steps_source,
            steps_at,
            gym,
            gym_source,
            gym_at,
            trained,
            gym_at,
            weight,
            weight_at
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(password)
}

async fn insert_meal(
    tx: &mut sqlx::SqliteConnection,
    rng: &mut Rng,
    user: i64,
    date: &str,
    meal_type: &str,
    foods: &[&str],
    at: i64,
) -> Result<(), BoxErr> {
    let meal_id = uuid::Uuid::from_u64_pair(rng.next(), rng.next()).to_string();
    let description = foods.join(" + ");
    sqlx::query!(
        "INSERT INTO meals (id, user_id, date, meal_type, description, source, updated_at, received_at)
         VALUES (?, ?, ?, ?, ?, 'manual', ?, ?)",
        meal_id,
        user,
        date,
        meal_type,
        description,
        at,
        at
    )
    .execute(&mut *tx)
    .await?;
    for (position, name) in (0i64..).zip(foods) {
        let food = sqlx::query!(
            "SELECT id, kcal, protein_g, carbs_g, fat_g, default_portion_g FROM foods
             WHERE source = 'staple' AND name = ?",
            name
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| format!("staple food {name:?} is missing"))?;
        let grams = (food.default_portion_g.unwrap_or(100.0)
            * rng.pick(&[1.0, 1.5, 1.5, 2.0, 2.0]))
        .round();
        let per = |v: f64| crate::nutrition::r1(v * grams / 100.0);
        let item_id = uuid::Uuid::from_u64_pair(rng.next(), rng.next()).to_string();
        let portion = format!("{grams} g");
        let (kcal, protein, carbs, fat) = (
            per(food.kcal),
            per(food.protein_g),
            per(food.carbs_g),
            per(food.fat_g),
        );
        sqlx::query!(
            "INSERT INTO meal_items (id, meal_id, position, name, portion_text, grams, kcal,
                 protein_g, carbs_g, fat_g, food_id, origin)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'food_db')",
            item_id,
            meal_id,
            position,
            name,
            portion,
            grams,
            kcal,
            protein,
            carbs,
            fat,
            food.id
        )
        .execute(&mut *tx)
        .await?;
    }
    Ok(())
}
