// SPDX-License-Identifier: AGPL-3.0-or-later
//! `POST /api/import/fuel-ledger`: Fuel Ledger (and Cupful's own) CSV into meals and day activity.
//!
//! The handler reads rows (`rows`), groups them into meals (`drafts`) and writes the result in one
//! transaction (`write`).

mod drafts;
mod rows;
mod write;

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::AppState;
use crate::auth::User;
use crate::error::{Error, Result, TextBody};
use crate::time::{now_ms, user_today};

pub async fn import(State(s): State<AppState>, user: User, body: TextBody) -> Result<Json<Value>> {
    let today = user_today(&s.db, user.id).await?;
    let tomorrow = today
        .tomorrow()
        .map_err(|e| Error::Internal(e.to_string()))?;

    let (rows, days) = rows::parse(&body.0, tomorrow)?;
    let drafts = drafts::group(&rows)?;
    let counts = write::apply(&s, user.id, &drafts, &days, now_ms(), tomorrow).await?;

    Ok(Json(json!({
        "meals_created": counts.created,
        "meals_updated": counts.updated,
        "meals_unchanged": counts.unchanged,
        "meals_skipped_deleted": counts.skipped_deleted,
        "meals_skipped_edited": counts.skipped_edited,
        "items": counts.items,
        "days": counts.days,
    })))
}
