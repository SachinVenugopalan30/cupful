// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON backup and restore (PLAN.md §14 M5).

use axum::Json;
use axum::response::Response;
use serde_json::Value;

use crate::AppState;
use crate::auth::User;
use crate::error::{Error, JsonBody, Result};

pub async fn export(_s: AppState, _user: User) -> Result<Response> {
    Err(Error::not_found())
}

pub async fn restore(_s: AppState, _user: User, _body: JsonBody<Value>) -> Result<Json<Value>> {
    Err(Error::not_found())
}
