// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use common::*;

const DAY: i64 = 86_400_000;

#[tokio::test]
async fn cleanup_drops_expired_op_ids_tombstones_and_sessions() {
    let (app, state) = app().await;
    admin(&app).await;
    let db = &state.db;
    let now = jiff::Timestamp::now().as_millisecond();
    for (op, t) in [("old", now - 31 * DAY), ("new", now - 29 * DAY)] {
        sqlx::query("INSERT INTO sync_ops VALUES (1, ?, ?)")
            .bind(op)
            .bind(t)
            .execute(db)
            .await
            .unwrap();
    }
    for (id, deleted) in [
        ("m-old", Some(now - 181 * DAY)),
        ("m-new", Some(now - 179 * DAY)),
        ("m-live", None),
    ] {
        sqlx::query("INSERT INTO meals (id, user_id, date, meal_type, description, source, updated_at, received_at, deleted_at) VALUES (?, 1, '2026-01-01', 'lunch', '', 'manual', 0, 0, ?)")
            .bind(id).bind(deleted).execute(db).await.unwrap();
    }
    sqlx::query("INSERT INTO sessions VALUES ('expired', 1, '{}', ?)")
        .bind(now / 1000 - 1)
        .execute(db)
        .await
        .unwrap();
    for (hash, expires) in [
        ("old-invite", now - 31 * DAY),
        ("new-invite", now - 29 * DAY),
    ] {
        sqlx::query(
            "INSERT INTO invites (token_hash, created_by, created_at, expires_at)
             VALUES (?, 1, ?, ?)",
        )
        .bind(hash)
        .bind(now)
        .bind(expires)
        .execute(db)
        .await
        .unwrap();
    }

    cupful::cleanup::run(db, now).await.unwrap();

    let ops: Vec<String> = sqlx::query_scalar("SELECT op_id FROM sync_ops")
        .fetch_all(db)
        .await
        .unwrap();
    assert_eq!(ops, ["new"]);
    let meals: Vec<String> = sqlx::query_scalar("SELECT id FROM meals ORDER BY id")
        .fetch_all(db)
        .await
        .unwrap();
    assert_eq!(meals, ["m-live", "m-new"]);
    let expired: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions WHERE id = 'expired'")
        .fetch_one(db)
        .await
        .unwrap();
    assert_eq!(expired, 0);
    let live: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions")
        .fetch_one(db)
        .await
        .unwrap();
    assert_eq!(live, 1, "the admin's live session stays");
    let invites: Vec<String> = sqlx::query_scalar("SELECT token_hash FROM invites")
        .fetch_all(db)
        .await
        .unwrap();
    assert_eq!(invites, ["new-invite"]);
}
