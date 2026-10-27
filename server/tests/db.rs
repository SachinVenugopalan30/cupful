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
