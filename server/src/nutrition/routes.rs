// SPDX-License-Identifier: AGPL-3.0-or-later
//! `POST /api/estimate` and the review screen's "save as my food / alias / portion" endpoints.

use axum::Json;
use axum::extract::multipart::MultipartRejection;
use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use super::foods::{Food, normalize, visible};
use super::{Estimate, MAX_DESCRIPTION_CHARS, MAX_ITEM_GRAMS, MAX_KCAL_PER_100G};
use crate::AppState;
use crate::ai::{self, Image, Purpose};
use crate::auth::User;
use crate::error::{Error, JsonBody, Result};

/// JPEG, PNG or WebP by magic bytes; the declared content type proves nothing.
fn image_type(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Multipart form: `description` (text) and an optional `photo`. The photo lives only in this
/// request's memory and goes to the provider; it is never written anywhere.
pub async fn estimate(
    State(s): State<AppState>,
    user: User,
    form: std::result::Result<Multipart, MultipartRejection>,
) -> Result<Json<Estimate>> {
    let mut form = form.map_err(|e| Error::Status(e.status(), e.body_text()))?;
    let (mut description, mut photo) = (String::new(), None);
    loop {
        let field = form
            .next_field()
            .await
            .map_err(|e| Error::Status(e.status(), e.body_text()))?;
        let Some(field) = field else { break };
        let name = field.name().unwrap_or_default().to_string();
        let bytes = field
            .bytes()
            .await
            .map_err(|e| Error::Status(e.status(), e.body_text()))?;
        match name.as_str() {
            "description" => {
                description = String::from_utf8(bytes.to_vec())
                    .map_err(|_| Error::bad_request("description is not UTF-8"))?
            }
            "photo" => photo = Some(bytes),
            _ => {}
        }
    }
    let description = description.trim();
    if description.is_empty() {
        return Err(Error::bad_request("description is required"));
    }
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(Error::bad_request(format!(
            "description is longer than {MAX_DESCRIPTION_CHARS} characters"
        )));
    }
    let images = match photo {
        None => vec![],
        Some(b) if b.len() > crate::ai::MAX_IMAGE_BYTES => {
            return Err(Error::Status(
                StatusCode::PAYLOAD_TOO_LARGE,
                "photo is larger than 5 MB".into(),
            ));
        }
        Some(b) => {
            let media_type = image_type(&b).ok_or_else(|| {
                Error::Status(
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "photo must be a JPEG, PNG or WebP image".into(),
                )
            })?;
            vec![Image {
                bytes: b.to_vec(),
                media_type: media_type.into(),
            }]
        }
    };
    let purpose = if images.is_empty() {
        Purpose::Text
    } else {
        Purpose::Vision
    };
    let (provider_id, provider) = ai::for_user(&s, user.id, purpose).await?;
    let mut est = super::estimate(&s.db, user.id, provider.as_ref(), description, images).await?;
    est.provider_id = Some(provider_id);
    Ok(Json(est))
}

#[derive(Deserialize)]
pub struct NewFood {
    name: String,
    kcal: f64,
    protein_g: f64,
    carbs_g: f64,
    fat_g: f64,
    #[serde(default)]
    is_prepared: bool,
    #[serde(default)]
    default_portion_g: Option<f64>,
}

fn check_name(s: &str, what: &str) -> Result<String> {
    let s = s.trim();
    if s.is_empty() || s.chars().count() > 200 || normalize(s).is_empty() {
        return Err(Error::bad_request(format!(
            "{what} must be 1-200 characters"
        )));
    }
    Ok(s.to_string())
}

/// A private food (per 100 g), visible only to its owner.
pub async fn create_food(
    State(s): State<AppState>,
    user: User,
    JsonBody(f): JsonBody<NewFood>,
) -> Result<Json<Food>> {
    let name = check_name(&f.name, "name")?;
    let per100 = [f.kcal, f.protein_g, f.carbs_g, f.fat_g];
    if per100.iter().any(|v| !v.is_finite() || *v < 0.0)
        || f.kcal > MAX_KCAL_PER_100G
        || f.protein_g + f.carbs_g + f.fat_g > 100.0
    {
        return Err(Error::bad_request(
            "values are per 100 g: kcal 0-900, macros 0-100 g in total",
        ));
    }
    if f.default_portion_g
        .is_some_and(|g| !(g > 0.0 && g <= MAX_ITEM_GRAMS))
    {
        return Err(Error::bad_request("default_portion_g must be 0-2000"));
    }
    let (norm, owner) = (normalize(&name), user.id.to_string());
    let id = sqlx::query_scalar!(
        "INSERT INTO foods (source, source_ref, user_id, name, name_norm, is_prepared, kcal,
                            protein_g, carbs_g, fat_g, default_portion_g)
         VALUES ('user', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        owner,
        user.id,
        name,
        norm,
        f.is_prepared,
        f.kcal,
        f.protein_g,
        f.carbs_g,
        f.fat_g,
        f.default_portion_g
    )
    .fetch_one(&s.db)
    .await?;
    let food = visible(&s.db, user.id, id)
        .await?
        .ok_or_else(Error::not_found)?;
    Ok(Json(food))
}

#[derive(Deserialize)]
pub struct NewAlias {
    alias: String,
    food_id: i64,
}

/// The user's own name for a food they can see. Replaces their earlier alias with the same name.
pub async fn create_alias(
    State(s): State<AppState>,
    user: User,
    JsonBody(a): JsonBody<NewAlias>,
) -> Result<Json<Value>> {
    let alias = check_name(&a.alias, "alias")?;
    visible(&s.db, user.id, a.food_id)
        .await?
        .ok_or_else(Error::not_found)?;
    let norm = normalize(&alias);
    // Config writes can overlap a background offline-queue flush; take the write lock up front
    // so SQLite waits on busy_timeout instead of failing a deferred upgrade with "database is locked".
    let mut tx = s.db.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query!(
        "DELETE FROM food_aliases WHERE user_id = ? AND alias_norm = ?",
        user.id,
        norm
    )
    .execute(&mut *tx)
    .await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO food_aliases (food_id, alias, alias_norm, user_id) VALUES (?, ?, ?, ?) RETURNING id",
        a.food_id,
        alias,
        norm,
        user.id
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"id": id, "alias": alias, "food_id": a.food_id}),
    ))
}

/// The user's own aliases, newest first. Shipped aliases are never listed.
pub async fn list_aliases(State(s): State<AppState>, user: User) -> Result<Json<Value>> {
    let rows = sqlx::query!(
        "SELECT a.id, a.alias, a.food_id, f.name AS food_name, f.brand, f.barcode, f.source
         FROM food_aliases a JOIN foods f ON f.id = a.food_id
         WHERE a.user_id = ?
         ORDER BY a.id DESC",
        user.id
    )
    .fetch_all(&s.db)
    .await?;
    let aliases: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({"id": r.id, "alias": r.alias, "food_id": r.food_id, "food_name": r.food_name,
                   "brand": r.brand, "barcode": r.barcode, "source": r.source})
        })
        .collect();
    Ok(Json(json!({ "aliases": aliases })))
}

pub async fn delete_alias(
    State(s): State<AppState>,
    user: User,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let n = sqlx::query!(
        "DELETE FROM food_aliases WHERE id = ? AND user_id = ?",
        id,
        user.id
    )
    .execute(&s.db)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(Error::not_found());
    }
    Ok(Json(json!({})))
}

#[derive(Deserialize)]
pub struct NewPortion {
    phrase: String,
    food_id: i64,
    grams: f64,
}

/// "My usual chicken" = this food, this many grams. Checked before the measures table.
pub async fn create_portion(
    State(s): State<AppState>,
    user: User,
    JsonBody(p): JsonBody<NewPortion>,
) -> Result<Json<Value>> {
    let phrase = check_name(&p.phrase, "phrase")?;
    if !(p.grams > 0.0 && p.grams <= MAX_ITEM_GRAMS) {
        return Err(Error::bad_request("grams must be 0-2000"));
    }
    visible(&s.db, user.id, p.food_id)
        .await?
        .ok_or_else(Error::not_found)?;
    let norm = normalize(&phrase);
    sqlx::query!(
        "INSERT INTO user_portions (user_id, phrase_norm, food_id, grams) VALUES (?, ?, ?, ?)
         ON CONFLICT (user_id, phrase_norm) DO UPDATE SET food_id = excluded.food_id, grams = excluded.grams",
        user.id,
        norm,
        p.food_id,
        p.grams
    )
    .execute(&s.db)
    .await?;
    Ok(Json(
        json!({"phrase": phrase, "food_id": p.food_id, "grams": p.grams}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_types_by_magic_bytes() {
        assert_eq!(image_type(&[0xFF, 0xD8, 0xFF, 0xE0, 0]), Some("image/jpeg"));
        assert_eq!(image_type(b"\x89PNG\r\n\x1a\n...."), Some("image/png"));
        assert_eq!(image_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(image_type(b"GIF89a......"), None);
        assert_eq!(image_type(b"RIFF\0\0\0\0WAVE"), None);
        assert_eq!(image_type(b""), None);
    }
}
