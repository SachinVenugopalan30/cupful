// SPDX-License-Identifier: AGPL-3.0-or-later
use jiff::civil::Date;
use sqlx::SqlitePool;

use crate::error::{Error, Result};

pub fn now_ms() -> i64 {
    jiff::Timestamp::now().as_millisecond()
}

pub fn rfc3339(ms: i64) -> Result<String> {
    jiff::Timestamp::from_millisecond(ms)
        .map(|t| t.to_string())
        .map_err(|e| Error::Internal(e.to_string()))
}

pub fn parse_date(s: &str) -> Result<Date> {
    s.parse()
        .map_err(|_| Error::bad_request(format!("invalid date {s:?}; use YYYY-MM-DD")))
}

pub fn today_in(tz: &str) -> Result<Date> {
    let tz = jiff::tz::TimeZone::get(tz).map_err(|e| Error::Internal(e.to_string()))?;
    Ok(jiff::Zoned::now().with_time_zone(tz).date())
}

/// The user's local date, in their profile timezone.
pub async fn user_today(db: &SqlitePool, user_id: i64) -> Result<Date> {
    let tz = sqlx::query_scalar!("SELECT timezone FROM profiles WHERE user_id = ?", user_id)
        .fetch_one(db)
        .await?;
    today_in(&tz)
}
