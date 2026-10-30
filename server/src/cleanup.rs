// SPDX-License-Identifier: AGPL-3.0-or-later
//! Daily cleanup: expired op ids, old tombstones, expired sessions (PLAN.md §6).

use std::time::Duration;

use sqlx::SqlitePool;

const DAY_MS: i64 = 86_400_000;
const OP_ID_RETENTION_MS: i64 = 30 * DAY_MS;
const INVITE_RETENTION_MS: i64 = 30 * DAY_MS;
pub const TOMBSTONE_RETENTION_MS: i64 = 180 * DAY_MS;

pub async fn run(db: &SqlitePool, now_ms: i64) -> Result<(), sqlx::Error> {
    let ops_before = now_ms - OP_ID_RETENTION_MS;
    let tombstones_before = now_ms - TOMBSTONE_RETENTION_MS;
    let invites_before = now_ms - INVITE_RETENTION_MS;
    let now_s = now_ms / 1000;
    sqlx::query!("DELETE FROM sync_ops WHERE received_at < ?", ops_before)
        .execute(db)
        .await?;
    // Items go with their meal (ON DELETE CASCADE).
    sqlx::query!("DELETE FROM meals WHERE deleted_at < ?", tombstones_before)
        .execute(db)
        .await?;
    sqlx::query!("DELETE FROM sessions WHERE expiry_date < ?", now_s)
        .execute(db)
        .await?;
    // Used or not, an invite expired more than 30 days ago is only history.
    sqlx::query!("DELETE FROM invites WHERE expires_at < ?", invites_before)
        .execute(db)
        .await?;
    Ok(())
}

pub fn spawn(db: SqlitePool) {
    tokio::spawn(async move {
        let mut every = tokio::time::interval(Duration::from_secs(24 * 3600));
        loop {
            every.tick().await;
            if let Err(e) = run(&db, crate::time::now_ms()).await {
                tracing::warn!("cleanup failed: {e}");
            }
        }
    });
}
