// SPDX-License-Identifier: AGPL-3.0-or-later
//! Export, import and backup (M5). Handlers live here; the formats live in the submodules.

pub mod backup;
pub mod csv;

use axum::Json;
use axum::extract::State;
use axum::response::Response;
use serde_json::Value;

use crate::AppState;
use crate::auth::User;
use crate::error::{JsonBody, Result, TextBody};

pub async fn export_csv(State(s): State<AppState>, user: User) -> Result<Response> {
    csv::export(s, user).await
}

pub async fn import_csv(
    State(s): State<AppState>,
    user: User,
    body: TextBody,
) -> Result<Json<Value>> {
    csv::import(s, user, body).await
}

pub async fn backup(State(s): State<AppState>, user: User) -> Result<Response> {
    backup::export(s, user).await
}

pub async fn restore(
    State(s): State<AppState>,
    user: User,
    body: JsonBody<Value>,
) -> Result<Json<Value>> {
    backup::restore(s, user, body).await
}
