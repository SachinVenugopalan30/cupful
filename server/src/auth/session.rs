// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cookie session layer on a SQLite-backed store.

use async_trait::async_trait;
use serde_json::Value;
use sqlx::SqlitePool;
use tower_sessions::session::{Id, Record};
use tower_sessions::session_store::{self, SessionStore};
use tower_sessions::{Expiry, SessionManagerLayer, cookie};

use super::USER_KEY;

pub fn session_layer(db: SqlitePool, secure: bool) -> SessionManagerLayer<SqliteStore> {
    SessionManagerLayer::new(SqliteStore(db))
        .with_name("cupful_session")
        .with_http_only(true)
        .with_secure(secure)
        .with_same_site(cookie::SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(cookie::time::Duration::days(30)))
        .with_always_save(true)
}

/// tower-sessions store on our pool. (tower-sessions-sqlx-store still targets sqlx 0.8.)
#[derive(Clone, Debug)]
pub struct SqliteStore(SqlitePool);

fn backend(e: impl std::fmt::Display) -> session_store::Error {
    session_store::Error::Backend(e.to_string())
}

impl SqliteStore {
    async fn upsert(&self, r: &Record, replace: bool) -> session_store::Result<bool> {
        let id = r.id.to_string();
        let user_id = r.data.get(USER_KEY).and_then(Value::as_i64);
        let data = serde_json::to_string(&r.data).map_err(backend)?;
        let expiry = r.expiry_date.unix_timestamp();
        let res = sqlx::query!(
            "INSERT INTO sessions (id, user_id, data, expiry_date) VALUES (?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET user_id = excluded.user_id, data = excluded.data,
                 expiry_date = excluded.expiry_date
             WHERE ?",
            id,
            user_id,
            data,
            expiry,
            replace
        )
        .execute(&self.0)
        .await
        .map_err(backend)?;
        Ok(res.rows_affected() == 1)
    }
}

#[async_trait]
impl SessionStore for SqliteStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        while !self.upsert(record, false).await? {
            record.id = Id::default();
        }
        Ok(())
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        self.upsert(record, true).await.map(|_| ())
    }

    async fn load(&self, id: &Id) -> session_store::Result<Option<Record>> {
        let id_str = id.to_string();
        let now = jiff::Timestamp::now().as_second();
        let Some(row) = sqlx::query!(
            "SELECT data, expiry_date FROM sessions WHERE id = ? AND expiry_date > ?",
            id_str,
            now
        )
        .fetch_optional(&self.0)
        .await
        .map_err(backend)?
        else {
            return Ok(None);
        };
        Ok(Some(Record {
            id: *id,
            data: serde_json::from_str(&row.data).map_err(backend)?,
            expiry_date: cookie::time::OffsetDateTime::from_unix_timestamp(row.expiry_date)
                .map_err(backend)?,
        }))
    }

    async fn delete(&self, id: &Id) -> session_store::Result<()> {
        let id = id.to_string();
        sqlx::query!("DELETE FROM sessions WHERE id = ?", id)
            .execute(&self.0)
            .await
            .map_err(backend)?;
        Ok(())
    }
}
