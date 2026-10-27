// SPDX-License-Identifier: AGPL-3.0-or-later
use std::path::Path;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

fn options() -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true)
        .synchronous(SqliteSynchronous::Normal)
}

pub async fn open(path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let opts = options()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new().connect_with(opts).await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

/// Fresh migrated in-memory database. One connection, since each in-memory connection is its own database.
pub async fn memory() -> SqlitePool {
    let opts = options().in_memory(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("open in-memory db");
    MIGRATOR.run(&pool).await.expect("migrate");
    pool
}
