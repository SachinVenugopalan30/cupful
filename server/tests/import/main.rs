// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fuel Ledger CSV import (PLAN.md §14 M5).
//!
//! `sample` and `merge` cover whole-file round trips, `errors` the rows that are refused, and
//! `activity` the steps/gym/weight columns.
/// Shared helpers live at tests/common/mod.rs, outside this target directory.
#[path = "../common/mod.rs"]
mod common;

mod activity;
mod errors;
mod merge;
mod sample;

use axum::http::StatusCode;
use common::*;
use serde_json::Value;

pub(crate) const TZ: &str = "Asia/Kolkata";
pub(crate) const HEADER: &str = "date,meal_id,item_index,meal,description,food,portion,kcal,protein_g,carbs_g,fat_g,steps,gym_minutes,trained,weight_kg";

pub(crate) fn csv(rows: &[&str]) -> String {
    format!("{HEADER}\n{}\n", rows.join("\n"))
}

pub(crate) fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/../fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

pub(crate) async fn import(c: &mut Client, body: &str) -> (StatusCode, Value) {
    c.post_text("/api/import/fuel-ledger", body).await
}

pub(crate) async fn meals(c: &mut Client, date: &str) -> Value {
    let (status, body) = c.get(&format!("/api/meals?date={date}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["meals"].clone()
}

/// The user's CSV export, as a string ready to import back.
pub(crate) async fn exported_csv(c: &mut Client) -> String {
    let (status, _, bytes) = c.get_bytes("/api/export.csv").await;
    assert_eq!(status, StatusCode::OK);
    String::from_utf8(bytes.to_vec()).unwrap()
}

pub(crate) fn error(body: &Value) -> String {
    body["error"].as_str().unwrap_or_default().to_string()
}
