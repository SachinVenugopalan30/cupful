// SPDX-License-Identifier: AGPL-3.0-or-later
//! Nutrition pipeline (PLAN.md §8): parse with the user's AI provider, validate, convert household
//! measures, match against the food database, add cooking oil. Nothing is saved here; the client
//! saves the reviewed meal through `/api/sync`.

pub mod foods;
pub mod golden;
mod measures;
mod prompt;
mod routes;
#[cfg(test)]
mod tests;

pub use routes::{
    create_alias, create_food, create_portion, delete_alias, estimate as estimate_handler,
    list_aliases,
};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::ai::{Image, JsonRequest, Provider, ProviderError};
use crate::error::{Error, Result};
use foods::{Food, Match, normalize};
use measures::{food_portion, has, is_weight, spoon_grams, unit_grams, uses_oil, without_sizes};
use prompt::{INSTRUCTION, schema};

pub const MAX_ITEMS: usize = 15;
pub const MAX_DESCRIPTION_CHARS: usize = 1000;
pub const MAX_ITEM_GRAMS: f64 = 2000.0;
pub const MAX_KCAL_PER_100G: f64 = 900.0;
/// "Cooking oil" item per oil-cooked portion: 2 tsp.
pub const OIL_PER_PORTION_G: f64 = 9.0;

#[derive(Deserialize)]
struct Reply {
    items: Vec<RawItem>,
    #[serde(default)]
    assumptions: Option<String>,
    #[serde(default)]
    confidence: Option<String>,
}

#[derive(Deserialize)]
struct RawItem {
    food: String,
    #[serde(default)]
    amount: Option<f64>,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    grams: Option<f64>,
    #[serde(default)]
    cooking: Option<String>,
    #[serde(default)]
    mentions_oil: bool,
    #[serde(default)]
    estimate: Option<Macros>,
}

/// kcal and grams of protein, carbs and fat.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Macros {
    pub kcal: f64,
    pub protein: f64,
    pub carbs: f64,
    pub fat: f64,
}

impl Macros {
    fn scale(self, k: f64) -> Self {
        Macros {
            kcal: self.kcal * k,
            protein: self.protein * k,
            carbs: self.carbs * k,
            fat: self.fat * k,
        }
    }
}

impl From<&Food> for Macros {
    fn from(f: &Food) -> Self {
        Macros {
            kcal: f.kcal,
            protein: f.protein_g,
            carbs: f.carbs_g,
            fat: f.fat_g,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Estimate {
    pub items: Vec<Item>,
    pub assumptions: Option<String>,
    pub confidence: Option<String>,
    pub provider_id: Option<i64>,
    /// Items the pipeline had to drop, in words for the review screen.
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Item {
    pub name: String,
    pub portion_text: String,
    pub grams: f64,
    /// `None` when there is neither a database match nor a plausible model estimate: the user edits it.
    pub kcal: Option<f64>,
    pub protein_g: Option<f64>,
    pub carbs_g: Option<f64>,
    pub fat_g: Option<f64>,
    /// `food_db` or `ai_estimate`, as in `meal_items.origin`.
    pub origin: &'static str,
    pub food_id: Option<i64>,
    pub food_name: Option<String>,
    /// `matched`, `ambiguous` or `unmatched`.
    #[serde(rename = "match")]
    pub status: &'static str,
    pub candidates: Vec<Candidate>,
}

/// An alternative food for an ambiguous item, with values for the item's grams.
#[derive(Debug, Serialize)]
pub struct Candidate {
    pub food_id: i64,
    pub name: String,
    pub kcal: f64,
    pub protein_g: f64,
    pub carbs_g: f64,
    pub fat_g: f64,
}

pub(crate) fn r1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

fn valid_grams(g: Option<f64>) -> Option<f64> {
    g.filter(|g| *g > 0.0 && *g <= MAX_ITEM_GRAMS)
}

/// The model's estimate as per-100 g values, or `None` if it's implausible. Fixes kcal from the
/// macros when they disagree by more than 20 % + 10 kcal.
fn per_100g(e: Macros, grams: f64) -> Option<Macros> {
    let from_macros = 4.0 * e.protein + 4.0 * e.carbs + 9.0 * e.fat;
    let e = if (from_macros - e.kcal).abs() > 0.2 * e.kcal.abs() + 10.0 {
        Macros {
            kcal: from_macros,
            ..e
        }
    } else {
        e
    };
    let p = e.scale(100.0 / grams);
    let ok = [p.protein, p.carbs, p.fat].iter().all(|v| *v >= 0.0)
        && (0.0..=MAX_KCAL_PER_100G).contains(&p.kcal);
    ok.then_some(p)
}

/// Foods the description names (by a known alias) that no item covers: small models drop items
/// ("a splash of milk"). An alias counts as covered when its food was matched or any of its words is
/// in an item's name, so this errs toward saying nothing.
async fn missed(
    db: &SqlitePool,
    user_id: i64,
    description: &str,
    items: &[Item],
) -> std::result::Result<Option<String>, sqlx::Error> {
    let desc = format!(" {} ", normalize(description));
    let names: Vec<String> = items.iter().map(|i| normalize(&i.name)).collect();
    let covered = |w: &str| names.iter().any(|n| n.split(' ').any(|x| x == w));
    let matched: Vec<i64> = items.iter().filter_map(|i| i.food_id).collect();
    let mut found: Vec<String> = vec![];
    for a in sqlx::query!(
        "SELECT alias_norm, food_id FROM food_aliases
         WHERE (user_id IS NULL OR user_id = ?) AND instr(?, ' ' || alias_norm || ' ') > 0",
        user_id,
        desc
    )
    .fetch_all(db)
    .await?
    {
        if !matched.contains(&a.food_id) && !a.alias_norm.split(' ').any(covered) {
            found.push(a.alias_norm);
        }
    }
    found.sort_unstable();
    found.dedup();
    // "milk" inside "lactose free milk" is the same mention.
    let outer: Vec<&String> = found
        .iter()
        .filter(|a| {
            !found
                .iter()
                .any(|b| b != *a && format!(" {b} ").contains(&format!(" {a} ")))
        })
        .collect();
    Ok((!outer.is_empty()).then(|| {
        let list = outer
            .iter()
            .map(|a| a.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!("The estimate may have missed: {list}. Add it below if you had it.")
    }))
}

/// The user's saved portion for this phrase, if any: (food, grams per amount).
async fn user_portion(
    db: &SqlitePool,
    user_id: i64,
    phrases: [&str; 2],
) -> std::result::Result<Option<(Food, f64)>, sqlx::Error> {
    let [a, b] = phrases;
    let row = sqlx::query!(
        r#"SELECT p.grams, f.id, f.name, f.source, f.is_prepared AS "is_prepared: bool", f.kcal,
                  f.protein_g, f.carbs_g, f.fat_g, f.default_portion_g
           FROM user_portions p JOIN foods f ON f.id = p.food_id
           WHERE p.user_id = ? AND p.phrase_norm IN (?, ?) AND (f.user_id IS NULL OR f.user_id = ?)
           ORDER BY length(p.phrase_norm) DESC LIMIT 1"#,
        user_id,
        a,
        b,
        user_id
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| {
        (
            Food {
                id: r.id,
                name: r.name,
                source: r.source,
                is_prepared: r.is_prepared,
                kcal: r.kcal,
                protein_g: r.protein_g,
                carbs_g: r.carbs_g,
                fat_g: r.fat_g,
                default_portion_g: r.default_portion_g,
            },
            r.grams,
        )
    }))
}

/// Runs the pipeline for `user_id` (0 matches shared foods only, as `cupful eval` does).
pub async fn estimate(
    db: &SqlitePool,
    user_id: i64,
    provider: &dyn Provider,
    description: &str,
    images: Vec<Image>,
) -> Result<Estimate> {
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(Error::bad_request(format!(
            "description is longer than {MAX_DESCRIPTION_CHARS} characters"
        )));
    }
    let raw = provider
        .complete_json(JsonRequest {
            instruction: INSTRUCTION.into(),
            user_text: description.into(),
            images,
            schema: schema(),
        })
        .await?;
    tracing::debug!(reply = %raw, "model reply");
    let reply: Reply =
        serde_json::from_value(raw).map_err(|e| ProviderError::Unparseable(e.to_string()))?;
    if reply.items.len() > MAX_ITEMS {
        return Err(Error::bad_request(format!(
            "that's more than {MAX_ITEMS} foods; split it into several meals"
        )));
    }
    let oil_listed = has(&normalize(description), &["oil", "ghee"])
        || reply
            .items
            .iter()
            .any(|i| i.mentions_oil || has(&normalize(&i.food), &["oil", "ghee"]));
    let (mut items, mut warnings, mut oil_portions) = (vec![], vec![], 0.0);
    for raw in reply.items {
        let food_norm = normalize(&raw.food);
        let unit = raw.unit.as_deref().unwrap_or("").trim();
        let unit_norm = normalize(unit);
        let amount = raw
            .amount
            .filter(|a| *a > 0.0 && a.is_finite())
            .unwrap_or(1.0);
        let portion = user_portion(
            db,
            user_id,
            [&normalize(&format!("{unit} {}", raw.food)), &food_norm],
        )
        .await?;
        let matched = match &portion {
            Some((f, _)) => Match::Matched(f.clone()),
            None => foods::find(db, user_id, &without_sizes(&food_norm)).await?,
        };
        let (default_portion, usda_unit) = match &matched {
            Match::Matched(f) => {
                let own = food_portion(db, f.id, unit).await?;
                // A product's label gives one serving: any other count of it ("a scoop") is one.
                let product = matches!(f.source.as_str(), "off" | "user")
                    && !is_weight(&unit_norm)
                    && spoon_grams(&unit_norm).is_none();
                let serving = if product && own.is_none() {
                    food_portion(db, f.id, "serving").await?
                } else {
                    None
                };
                (f.default_portion_g, own.or(serving))
            }
            _ => (None, None),
        };
        // Within 10x of the food's normal portion, for one portion or for `amount` of them (models
        // sometimes put the weight in `amount`); else it's likely the count echoed as a weight
        // ("1 g" of salmon).
        let portions = if is_weight(&unit_norm) { 1.0 } else { amount };
        let plausible = |g: &f64| {
            default_portion.is_none_or(|d| {
                [d, d * portions]
                    .iter()
                    .any(|p| (p / 10.0..=p * 10.0).contains(g))
            })
        };
        let model_grams = valid_grams(raw.grams).filter(plausible);
        // Measured portions beat the model's guess: small models misjudge weights badly
        // (a scoop of protein powder as 15 g), while "1 scoop" in USDA's data is measured.
        let measured = unit_grams(&unit_norm, &food_norm)
            .or(usda_unit)
            .or_else(|| spoon_grams(&unit_norm))
            .map(|g| g * amount)
            .filter(plausible);
        let grams = portion
            .as_ref()
            .map(|p| p.1 * amount)
            .or(measured)
            .or(model_grams)
            .or_else(|| default_portion.map(|g| g * amount))
            .filter(|g| *g <= MAX_ITEM_GRAMS);
        let Some(grams) = grams else {
            warnings.push(format!(
                "Couldn't tell how much {} that was; add it by hand.",
                raw.food
            ));
            continue;
        };
        let est = raw
            .estimate
            .and_then(|e| per_100g(e, model_grams.unwrap_or(grams)));
        let portion_text = if unit_norm.is_empty() && raw.amount.is_none() {
            format!("{} g", r1(grams))
        } else {
            format!("{amount} {unit}").trim().to_string()
        };
        let mut item = Item {
            name: raw.food.clone(),
            portion_text,
            grams: r1(grams),
            kcal: None,
            protein_g: None,
            carbs_g: None,
            fat_g: None,
            origin: "ai_estimate",
            food_id: None,
            food_name: None,
            status: "unmatched",
            candidates: vec![],
        };
        let set = |item: &mut Item, per100: Macros| {
            let m = per100.scale(grams / 100.0);
            (item.kcal, item.protein_g, item.carbs_g, item.fat_g) = (
                Some(r1(m.kcal)),
                Some(r1(m.protein)),
                Some(r1(m.carbs)),
                Some(r1(m.fat)),
            );
        };
        match matched {
            Match::Matched(f) => {
                set(&mut item, Macros::from(&f));
                if !f.is_prepared && !oil_listed && uses_oil(raw.cooking.as_deref().unwrap_or("")) {
                    oil_portions += amount;
                }
                (item.origin, item.status, item.food_id, item.food_name) =
                    ("food_db", "matched", Some(f.id), Some(f.name));
            }
            Match::Ambiguous(cands) => {
                item.status = "ambiguous";
                match est {
                    Some(e) => set(&mut item, e),
                    None => {
                        set(&mut item, Macros::from(&cands[0]));
                        (item.origin, item.food_id, item.food_name) =
                            ("food_db", Some(cands[0].id), Some(cands[0].name.clone()));
                    }
                }
                item.candidates = cands
                    .iter()
                    .map(|f| {
                        let m = Macros::from(f).scale(grams / 100.0);
                        Candidate {
                            food_id: f.id,
                            name: f.name.clone(),
                            kcal: r1(m.kcal),
                            protein_g: r1(m.protein),
                            carbs_g: r1(m.carbs),
                            fat_g: r1(m.fat),
                        }
                    })
                    .collect();
            }
            Match::Unmatched => {
                if let Some(e) = est {
                    set(&mut item, e);
                }
            }
        }
        items.push(item);
    }
    if oil_portions > 0.0 {
        let oil = foods::find(db, user_id, "cooking oil").await?;
        if let Match::Matched(f) = oil {
            let grams = OIL_PER_PORTION_G * oil_portions;
            let m = Macros::from(&f).scale(grams / 100.0);
            items.push(Item {
                name: "Cooking oil".into(),
                portion_text: format!("{} tsp", 2.0 * oil_portions),
                grams: r1(grams),
                kcal: Some(r1(m.kcal)),
                protein_g: Some(r1(m.protein)),
                carbs_g: Some(r1(m.carbs)),
                fat_g: Some(r1(m.fat)),
                origin: "food_db",
                food_id: Some(f.id),
                food_name: Some(f.name),
                status: "matched",
                candidates: vec![],
            });
        }
    }
    warnings.extend(missed(db, user_id, description, &items).await?);
    Ok(Estimate {
        items,
        assumptions: reply.assumptions,
        confidence: reply
            .confidence
            .filter(|c| ["low", "medium", "high"].contains(&c.as_str())),
        provider_id: None,
        warnings,
    })
}
