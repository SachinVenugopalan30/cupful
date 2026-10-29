// SPDX-License-Identifier: AGPL-3.0-or-later
use sqlx::Row;

fn temp_db(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cupful-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("cupful.db")
}

#[tokio::test]
async fn file_db_uses_required_pragmas() {
    let pool = cupful::db::open(&temp_db("pragmas")).await.unwrap();
    let mut conn = pool.acquire().await.unwrap();
    for (pragma, want) in [
        ("journal_mode", "wal"),
        ("busy_timeout", "5000"),
        ("foreign_keys", "1"),
        ("synchronous", "1"), // NORMAL
    ] {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!("PRAGMA {pragma}")))
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        let got: String = row
            .try_get::<String, _>(0)
            .or_else(|_| row.try_get::<i64, _>(0).map(|n| n.to_string()))
            .unwrap();
        assert_eq!(got, want, "PRAGMA {pragma}");
    }
}

#[tokio::test]
async fn migrations_backfill_manual_origin_and_product_portions() {
    use sqlx::sqlite::SqlitePoolOptions;
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    // Everything before 20261029000100, then old-shaped rows, then the rest.
    let mut old = sqlx::migrate::Migrator::DEFAULT;
    old.migrations = cupful::db::MIGRATOR
        .migrations
        .iter()
        .filter(|m| m.version < 20261029000100)
        .cloned()
        .collect();
    old.run(&pool).await.unwrap();
    for sql in [
        "INSERT INTO users (id, username, password_hash, is_admin, created_at) VALUES (1, 'u', 'x', 0, 0)",
        "INSERT INTO meals (id, user_id, date, meal_type, description, scale, source, updated_at, received_at)
         VALUES ('m1', 1, '2026-01-01', 'lunch', '', 1, 'manual', 0, 0), ('m2', 1, '2026-01-01', 'lunch', '', 1, 'ai', 0, 0)",
        "INSERT INTO meal_items (id, meal_id, position, name, kcal, protein_g, carbs_g, fat_g, food_id, origin)
         VALUES ('i1', 'm1', 0, 'a', 1, 1, 1, 1, NULL, 'ai_estimate'),
                ('i2', 'm1', 1, 'b', 1, 1, 1, 1, 5, 'food_db'),
                ('i3', 'm2', 0, 'c', 1, 1, 1, 1, NULL, 'ai_estimate')",
        "INSERT INTO foods (id, source, source_ref, name, name_norm, barcode, kcal, protein_g, carbs_g, fat_g, serving_g)
         VALUES (900000, 'off', '96385074', 'Bar', 'bar', '96385074', 400, 5, 60, 10, 25)",
    ] {
        sqlx::query(sql).execute(&pool).await.unwrap();
    }
    cupful::db::MIGRATOR.run(&pool).await.unwrap();
    let origins: Vec<String> = sqlx::query_scalar("SELECT origin FROM meal_items ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(origins, ["manual", "food_db", "ai_estimate"]);
    let (default, serving): (f64, f64) = sqlx::query_as(
        "SELECT f.default_portion_g, p.grams FROM foods f
         JOIN food_portions p ON p.food_id = f.id AND p.unit = 'serving' WHERE f.id = 900000",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((default, serving), (25.0, 25.0));
}

#[tokio::test]
async fn sqlite_has_fts5() {
    let pool = cupful::db::memory().await;
    sqlx::query("CREATE VIRTUAL TABLE temp.f USING fts5(name)")
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn concurrent_writers_do_not_hit_database_is_locked() {
    let pool = cupful::db::open(&temp_db("writers")).await.unwrap();
    sqlx::query("CREATE TABLE w (n INTEGER)")
        .execute(&pool)
        .await
        .unwrap();
    let writer = |pool: sqlx::SqlitePool| async move {
        for n in 0..100 {
            let mut tx = pool.begin().await.unwrap();
            sqlx::query("INSERT INTO w VALUES (?)")
                .bind(n)
                .execute(&mut *tx)
                .await
                .unwrap();
            sqlx::query("UPDATE w SET n = n + 1 WHERE rowid = 1")
                .execute(&mut *tx)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }
    };
    let (a, b) = tokio::join!(
        tokio::spawn(writer(pool.clone())),
        tokio::spawn(writer(pool.clone()))
    );
    a.unwrap();
    b.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM w")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 200);
}

#[tokio::test]
async fn foods_and_aliases_are_searchable_through_triggers() {
    let pool = cupful::db::memory().await;
    sqlx::query("INSERT INTO foods (id, source, name, name_norm, kcal, protein_g, carbs_g, fat_g) VALUES (100000, 'staple', 'Quokka fruit', 'quokka fruit', 165, 31, 0, 3.6)")
        .execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO food_aliases (food_id, alias, alias_norm) VALUES (100000, 'wombatberry', 'wombatberry')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let hit = |q: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>("SELECT food_id FROM foods_fts WHERE foods_fts MATCH ?")
                .bind(q)
                .fetch_optional(&pool)
                .await
                .unwrap()
        }
    };
    assert_eq!(hit("quokka").await, Some(100000));
    assert_eq!(hit("wombatberry").await, Some(100000));
    sqlx::query("DELETE FROM foods WHERE id = 100000")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(hit("wombatberry").await, None);
}
