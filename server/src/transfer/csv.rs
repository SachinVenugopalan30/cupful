// SPDX-License-Identifier: AGPL-3.0-or-later
//! CSV export and Fuel Ledger import (PLAN.md §14 M5).

use axum::Json;
use axum::response::Response;
use serde_json::Value;

use crate::AppState;
use crate::auth::User;
use crate::error::{Error, Result, TextBody};

pub async fn export(_s: AppState, _user: User) -> Result<Response> {
    Err(Error::not_found())
}

pub async fn import(_s: AppState, _user: User, _body: TextBody) -> Result<Json<Value>> {
    Err(Error::not_found())
}
