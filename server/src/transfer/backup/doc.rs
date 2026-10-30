// SPDX-License-Identifier: AGPL-3.0-or-later
//! The backup document: one struct per entity, used both to read rows (`query_as!`) and to
//! write and read the JSON. Nested parts (`items`, `portions`, `food`) are added by wrapper
//! structs that flatten the row.

use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub(super) const FORMAT: &str = "cupful-backup";
pub(super) const VERSION: u32 = 1;

/// A timestamp as unix milliseconds. JSON form: RFC 3339 (what export writes); import also
/// accepts integer milliseconds.
#[derive(Clone, Copy, Default, sqlx::Type)]
#[sqlx(transparent)]
pub(super) struct Ts(pub(super) i64);

impl Serialize for Ts {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let t = jiff::Timestamp::from_millisecond(self.0).map_err(S::Error::custom)?;
        s.collect_str(&t)
    }
}

impl<'de> Deserialize<'de> for Ts {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Text(String),
            Millis(i64),
        }
        Ok(Ts(match Raw::deserialize(d)? {
            Raw::Text(s) => s
                .parse::<jiff::Timestamp>()
                .map_err(|_| D::Error::custom("timestamps must be RFC 3339"))?
                .as_millisecond(),
            Raw::Millis(ms) => ms,
        }))
    }
}

/// `days.trained` is JSON text in the database and an array in the document.
mod trained {
    use super::*;

    pub fn ser<S: Serializer>(v: &Option<String>, s: S) -> Result<S::Ok, S::Error> {
        let parts: Vec<String> = v
            .as_deref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default();
        parts.serialize(s)
    }

    pub fn de<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
        Option::<Vec<String>>::deserialize(d)?
            .map(|p| serde_json::to_string(&p).map_err(D::Error::custom))
            .transpose()
    }
}

/// A JSON `null` where the database has an empty string.
fn null_default<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

#[derive(Serialize, Deserialize)]
pub(super) struct Doc {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    #[serde(default)]
    pub username: Option<String>,
    pub profile: Profile,
    pub profile_versions: Vec<Version>,
    pub days: Vec<Day>,
    pub meals: Vec<Meal>,
    pub foods: Vec<Food>,
    pub aliases: Vec<Alias>,
    pub portions: Vec<UserPortion>,
    pub products: Vec<Product>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Profile {
    pub timezone: String,
    pub weight_unit: String,
    pub height_unit: String,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Version {
    pub effective_from: String,
    pub sex: String,
    pub birth_year: i64,
    pub height_cm: f64,
    pub start_weight_kg: f64,
    pub goal_kg: Option<f64>,
    pub pace_kg_per_week: f64,
    pub typical_steps: i64,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Day {
    pub date: String,
    pub steps: Option<i64>,
    pub steps_source: Option<String>,
    pub steps_updated_at: Option<Ts>,
    pub gym_minutes: Option<i64>,
    pub gym_source: Option<String>,
    pub gym_updated_at: Option<Ts>,
    #[serde(
        default,
        serialize_with = "trained::ser",
        deserialize_with = "trained::de"
    )]
    pub trained: Option<String>,
    pub trained_updated_at: Option<Ts>,
    pub weight_kg: Option<f64>,
    pub weight_updated_at: Option<Ts>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct MealCore {
    pub id: String,
    pub date: String,
    pub meal_type: String,
    pub description: String,
    pub scale: f64,
    pub assumptions: Option<String>,
    pub confidence: Option<String>,
    pub source: String,
    pub provider_id: Option<i64>,
    pub updated_at: Ts,
    #[serde(default)]
    pub received_at: Ts,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Meal {
    #[serde(flatten)]
    pub core: MealCore,
    pub items: Vec<Item>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct ItemCore {
    #[serde(skip)]
    pub meal_id: String,
    pub id: String,
    #[serde(default)]
    pub position: i64,
    pub name: String,
    #[serde(default, deserialize_with = "null_default")]
    pub portion_text: String,
    pub grams: Option<f64>,
    pub kcal: f64,
    pub protein_g: f64,
    pub carbs_g: f64,
    pub fat_g: f64,
    pub food_id: Option<i64>,
    pub origin: String,
}

/// Where an item's food came from, so a reader can tell shared rows from private ones.
#[derive(Serialize, Deserialize)]
pub(super) struct FoodRef {
    pub source: String,
    pub source_ref: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Item {
    #[serde(flatten)]
    pub core: ItemCore,
    #[serde(default)]
    pub food: Option<FoodRef>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct FoodCore {
    pub id: i64,
    pub source: String,
    pub source_ref: Option<String>,
    pub user_id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub name_norm: String,
    pub brand: Option<String>,
    pub barcode: Option<String>,
    #[serde(default)]
    pub is_prepared: bool,
    pub kcal: f64,
    pub protein_g: f64,
    pub carbs_g: f64,
    pub fat_g: f64,
    pub default_portion_g: Option<f64>,
    pub serving_g: Option<f64>,
    pub serving_text: Option<String>,
    pub fetched_at: Option<Ts>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct FoodPortion {
    #[serde(skip)]
    pub food_id: i64,
    pub unit: String,
    pub grams: f64,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Food {
    #[serde(flatten)]
    pub core: FoodCore,
    #[serde(default)]
    pub portions: Vec<FoodPortion>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Alias {
    #[serde(default)]
    pub id: i64,
    pub food_id: i64,
    pub alias: String,
    #[serde(default)]
    pub alias_norm: String,
}

/// A remembered portion phrase (`user_portions`).
#[derive(Serialize, Deserialize)]
pub(super) struct UserPortion {
    pub phrase_norm: String,
    pub food_id: i64,
    pub grams: f64,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Product {
    pub food_id: i64,
    pub last_servings: Option<f64>,
    pub last_used_at: Ts,
}
