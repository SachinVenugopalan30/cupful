// SPDX-License-Identifier: AGPL-3.0-or-later
//! Food database: name normalization, shipped staples and aliases, the USDA importer, and matching
//! (PLAN.md §8 step 4, "Food database").

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

static STAPLES: &str = include_str!("../../../data/staples.csv");
static ALIASES: &str = include_str!("../../../data/aliases.csv");
static PORTIONS: &str = include_str!("../../../data/portions.csv");

/// Classification thresholds on the token-overlap score (0..=1). Tuned with the golden tests.
pub const MATCHED_MIN: f64 = 0.6;
pub const AMBIGUOUS_MIN: f64 = 0.3;
/// A match also needs this lead over the runner-up, else it's ambiguous.
pub const MARGIN: f64 = 0.15;
pub const MAX_CANDIDATES: usize = 3;
/// FTS rows fetched (bm25 order) before scoring.
const FTS_LIMIT: i64 = 50;

fn canon(s: &str) -> &str {
    match s {
        "tablespoon" | "tbs" | "tbl" => "tbsp",
        "teaspoon" => "tsp",
        "ounce" => "oz",
        "fluid ounce" | "fl" => "fl oz",
        "gram" | "gm" => "g",
        other => other,
    }
}

fn number(w: &str) -> Option<f64> {
    match w.split_once('/') {
        Some((a, b)) => Some(a.parse::<f64>().ok()? / b.parse::<f64>().ok()?),
        None => w.parse().ok(),
    }
}

/// Lookup keys for a household unit as the model or a user wrote it: the whole phrase, then its
/// first word ("large egg" → "large egg", "large"); a slice also tries USDA's "piece".
pub fn unit_keys(unit: &str) -> Vec<String> {
    let norm = normalize(unit);
    let words: Vec<&str> = norm
        .split(' ')
        .filter(|w| !w.is_empty() && number(w).is_none())
        .collect();
    let Some(first) = words.first() else {
        return vec![];
    };
    let mut keys = vec![
        canon(&words.join(" ")).to_string(),
        canon(first).to_string(),
    ];
    if keys[1] == "slice" {
        keys.push("piece".into());
    }
    keys.dedup();
    keys
}

/// A USDA portion's unit and amount: ("cup", 0.5) from "cups, chopped" with amount 0.5, or
/// ("piece", 1) from "1 piece, medium pizza". None for portions that aren't a household unit.
pub fn portion_unit(text: &str, amount: Option<f64>) -> Option<(String, f64)> {
    let t = text.to_lowercase();
    let skip = [
        "quantity not specified",
        "racc",
        "nlea",
        "surface inch",
        "cubic inch",
    ];
    if skip.iter().any(|s| t.contains(s)) {
        return None;
    }
    let mut words = t.split_whitespace().peekable();
    let leading = words.peek().and_then(|w| number(w));
    if leading.is_some() {
        words.next();
    }
    let rest = words.collect::<Vec<_>>().join(" ");
    let head = rest.split([',', '(']).next()?.trim();
    // "medium or regular slice" is a slice; "medium whole" is a medium (tomato).
    const NOUNS: [&str; 8] = [
        "slice", "piece", "cup", "bar", "link", "patty", "fillet", "wedge",
    ];
    let keys = unit_keys(head);
    let unit = match head.rsplit(' ').next() {
        Some(last) if head.contains(' ') && NOUNS.contains(&last) => last.to_string(),
        _ => keys.get(1).or(keys.first()).cloned()?,
    };
    let amount = amount.or(leading).filter(|a| *a > 0.0).unwrap_or(1.0);
    Some((unit, amount))
}

/// Lowercase, punctuation to spaces, simple plurals singularized.
pub fn normalize(s: &str) -> String {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(singular)
        .collect::<Vec<_>>()
        .join(" ")
}

// ponytail: suffix rules only ("chaas" -> "chaa"); harmless because stored names and queries
// go through the same function. A stemmer if matching quality ever needs it.
fn singular(w: &str) -> String {
    let n = w.len();
    let stem = if n > 4 && w.ends_with("ies") {
        return format!("{}y", &w[..n - 3]);
    } else if n > 4 && w.ends_with("oes") {
        &w[..n - 2]
    } else if n > 3 && w.ends_with('s') && !(w.ends_with("ss") || w.ends_with("us")) {
        &w[..n - 1]
    } else {
        w
    };
    stem.to_string()
}

/// Rows of a simple CSV: quoted fields with `""` escapes, no newlines inside fields.
fn csv(s: &str) -> Vec<Vec<String>> {
    s.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let (mut row, mut field, mut quoted) = (vec![], String::new(), false);
            let mut chars = line.chars().peekable();
            while let Some(c) = chars.next() {
                match c {
                    '"' if quoted && chars.peek() == Some(&'"') => {
                        field.push('"');
                        chars.next();
                    }
                    '"' => quoted = !quoted,
                    ',' if !quoted => row.push(std::mem::take(&mut field)),
                    c => field.push(c),
                }
            }
            row.push(field);
            row
        })
        .collect()
}

/// Upserts the shipped staples and replaces the shipped aliases. Idempotent; runs at startup.
pub async fn seed(db: &SqlitePool) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    let mut by_name = HashMap::new();
    // name,is_prepared,kcal,protein_g,carbs_g,fat_g,default_portion_g,portion_text,fdc_id,fdc_description
    for r in csv(STAPLES).into_iter().skip(1) {
        let num = |i: usize| -> f64 { r[i].parse().expect("staples.csv: number") };
        let (name, norm, prepared) = (&r[0], normalize(&r[0]), r[1] == "1");
        let (kcal, protein, carbs, fat) = (num(2), num(3), num(4), num(5));
        let portion = r[6].parse::<f64>().ok();
        let id = sqlx::query_scalar!(
            "INSERT INTO foods (source, source_ref, name, name_norm, is_prepared, kcal, protein_g,
                                carbs_g, fat_g, default_portion_g)
             VALUES ('staple', ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (source, source_ref) WHERE source IN ('staple', 'usda', 'off') DO UPDATE SET
                name = excluded.name, name_norm = excluded.name_norm,
                is_prepared = excluded.is_prepared, kcal = excluded.kcal,
                protein_g = excluded.protein_g, carbs_g = excluded.carbs_g, fat_g = excluded.fat_g,
                default_portion_g = excluded.default_portion_g
             RETURNING id",
            r[8],
            name,
            norm,
            prepared,
            kcal,
            protein,
            carbs,
            fat,
            portion
        )
        .fetch_one(&mut *tx)
        .await?;
        by_name.insert(name.clone(), id);
    }
    // fdc_id,amount,unit,grams
    sqlx::query!(
        "DELETE FROM food_portions WHERE food_id IN (SELECT id FROM foods WHERE source = 'staple')"
    )
    .execute(&mut *tx)
    .await?;
    let mut portions = csv(PORTIONS).into_iter().skip(1).collect::<Vec<_>>();
    // Within a food and unit the first row wins: plain before modified, then medium/regular sizes.
    portions.sort_by_key(|r| {
        let t = r[2].to_lowercase();
        (
            t.contains(','),
            !(t.contains("medium") || t.contains("regular")),
        )
    });
    for r in portions {
        let Some((unit, amount)) = portion_unit(&r[2], r[1].parse().ok()) else {
            continue;
        };
        let grams = r[3].parse::<f64>().expect("portions.csv: grams") / amount;
        sqlx::query!(
            "INSERT OR IGNORE INTO food_portions (food_id, unit, grams)
             SELECT id, ?, ? FROM foods WHERE source = 'staple' AND source_ref = ?",
            unit,
            grams,
            r[0]
        )
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query!("DELETE FROM food_aliases WHERE user_id IS NULL")
        .execute(&mut *tx)
        .await?;
    for r in csv(ALIASES).into_iter().skip(1) {
        let food_id = match r[1].strip_prefix("fdc:") {
            // USDA targets exist only after `cupful import-foods`.
            Some(fdc) => {
                sqlx::query_scalar!(
                    "SELECT id FROM foods WHERE source = 'usda' AND source_ref = ?",
                    fdc
                )
                .fetch_optional(&mut *tx)
                .await?
            }
            None => Some(*by_name.get(&r[1]).expect("aliases.csv: unknown staple")),
        };
        if let Some(food_id) = food_id {
            let norm = normalize(&r[0]);
            sqlx::query!(
                "INSERT INTO food_aliases (food_id, alias, alias_norm) VALUES (?, ?, ?)",
                food_id,
                r[0],
                norm
            )
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsdaFood {
    fdc_id: i64,
    description: String,
    #[serde(default)]
    food_nutrients: Vec<UsdaNutrient>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsdaNutrient {
    nutrient: UsdaNutrientId,
    amount: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsdaNutrientId {
    number: String,
    #[serde(default)]
    unit_name: String,
}

impl UsdaFood {
    /// kcal, protein, carbs, fat per 100 g. Foundation foods often give energy only as Atwater
    /// factors (958 specific, 957 general) and carbs only "by summation" (205.2).
    fn macros(&self) -> Option<[f64; 4]> {
        let get = |number: &str, unit: &str| {
            self.food_nutrients
                .iter()
                .find(|n| {
                    n.nutrient.number == number && n.nutrient.unit_name.eq_ignore_ascii_case(unit)
                })
                .and_then(|n| n.amount)
        };
        Some([
            get("208", "kcal")
                .or_else(|| get("958", "kcal"))
                .or_else(|| get("957", "kcal"))?,
            get("203", "g")?,
            get("205", "g").or_else(|| get("205.2", "g"))?,
            get("204", "g")?,
        ])
    }
}

/// Imports a FoodData Central Foundation or SR Legacy JSON download (the unzipped `.json`) as
/// `usda` foods. Idempotent: rows upsert on the FDC id. Returns (imported, skipped).
pub async fn import_usda(
    db: &SqlitePool,
    path: &std::path::Path,
) -> Result<(usize, usize), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    // One top-level key: "FoundationFoods" or "SRLegacyFoods". The Foundation file has null entries.
    let data: HashMap<String, Vec<Option<UsdaFood>>> =
        serde_json::from_reader(std::io::BufReader::new(file)).map_err(|e| {
            format!(
                "{}: not a FoodData Central JSON download: {e}",
                path.display()
            )
        })?;
    let foods = match (data.get("FoundationFoods"), data.get("SRLegacyFoods")) {
        (Some(f), _) | (None, Some(f)) => f,
        _ => return Err("expected a Foundation or SR Legacy JSON file".into()),
    };
    let db_err = |e: sqlx::Error| e.to_string();
    let mut tx = db.begin().await.map_err(db_err)?;
    let (mut imported, mut skipped) = (0, 0);
    for f in foods.iter().flatten() {
        let Some([kcal, protein, carbs, fat]) = f
            .macros()
            .filter(|m| (0.0..=super::MAX_KCAL_PER_100G).contains(&m[0]))
        else {
            skipped += 1;
            continue;
        };
        let norm = normalize(&f.description);
        // ponytail: "fried" in the name is the only cooking-fat signal USDA names give us.
        let prepared = norm.split(' ').any(|w| w == "fried");
        let fdc = f.fdc_id.to_string();
        sqlx::query!(
            "INSERT INTO foods (source, source_ref, name, name_norm, is_prepared, kcal, protein_g, carbs_g, fat_g)
             VALUES ('usda', ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (source, source_ref) WHERE source IN ('staple', 'usda', 'off') DO UPDATE SET
                name = excluded.name, name_norm = excluded.name_norm,
                is_prepared = excluded.is_prepared, kcal = excluded.kcal,
                protein_g = excluded.protein_g, carbs_g = excluded.carbs_g, fat_g = excluded.fat_g",
            fdc,
            f.description,
            norm,
            prepared,
            kcal,
            protein,
            carbs,
            fat
        )
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        imported += 1;
    }
    tx.commit().await.map_err(db_err)?;
    // Shipped aliases that point at USDA entries can resolve now.
    seed(db).await.map_err(db_err)?;
    Ok((imported, skipped))
}

/// A food row with its per-100 g values.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Food {
    pub id: i64,
    pub name: String,
    pub source: String,
    pub is_prepared: bool,
    pub kcal: f64,
    pub protein_g: f64,
    pub carbs_g: f64,
    pub fat_g: f64,
    pub default_portion_g: Option<f64>,
}

#[derive(Debug, PartialEq)]
pub enum Match<T = Food> {
    Matched(T),
    /// Up to `MAX_CANDIDATES`, best first.
    Ambiguous(Vec<T>),
    Unmatched,
}

/// Token overlap of query `q` with name `name`, 0..=1: the mean of how much of the query the name
/// covers and their Jaccard overlap, so "paneer" prefers "Paneer" over "Palak paneer".
fn score(q: &str, name: &str) -> f64 {
    let q: std::collections::HashSet<_> = q.split(' ').collect();
    let n: std::collections::HashSet<_> = name.split(' ').collect();
    let common = q.intersection(&n).count() as f64;
    (common / q.len() as f64 + common / q.union(&n).count() as f64) / 2.0
}

/// `scored` is (score, preferred, food) in search order; preferred = a staple or the user's own food.
fn classify<T>(mut scored: Vec<(f64, bool, T)>) -> Match<T> {
    scored.sort_by(|x, y| y.0.total_cmp(&x.0).then(y.1.cmp(&x.1)));
    // Prefer staples: a good staple hit makes near-identical USDA entries irrelevant.
    if scored.iter().any(|(s, p, _)| *p && *s >= MATCHED_MIN) {
        scored.retain(|(_, p, _)| *p);
    }
    scored.retain(|(s, _, _)| *s >= AMBIGUOUS_MIN);
    let second = scored.get(1).map_or(0.0, |x| x.0);
    match scored.first() {
        None => Match::Unmatched,
        Some((top, _, _)) if *top >= MATCHED_MIN && top - second >= MARGIN => {
            Match::Matched(scored.swap_remove(0).2)
        }
        Some(_) => Match::Ambiguous(
            scored
                .into_iter()
                .take(MAX_CANDIDATES)
                .map(|x| x.2)
                .collect(),
        ),
    }
}

/// Finds `name_norm` among the foods `user_id` may see: shared ones and their own.
pub async fn find(db: &SqlitePool, user_id: i64, name_norm: &str) -> Result<Match, sqlx::Error> {
    if name_norm.is_empty() {
        return Ok(Match::Unmatched);
    }
    // 1. Exact alias, the user's own first.
    let exact = sqlx::query_as!(
        Food,
        r#"SELECT f.id, f.name, f.source, f.is_prepared AS "is_prepared: bool", f.kcal, f.protein_g,
                  f.carbs_g, f.fat_g, f.default_portion_g
           FROM food_aliases a JOIN foods f ON f.id = a.food_id
           WHERE a.alias_norm = ? AND (a.user_id IS NULL OR a.user_id = ?)
             AND (f.user_id IS NULL OR f.user_id = ?)
           ORDER BY a.user_id IS NULL, a.id LIMIT 1"#,
        name_norm,
        user_id,
        user_id
    )
    .fetch_optional(db)
    .await?;
    if let Some(f) = exact {
        return Ok(Match::Matched(f));
    }
    // 2. Full-text search (bm25 order), then score each food by its best name or alias.
    let query = name_norm
        .split(' ')
        .map(|t| format!("\"{t}\""))
        .collect::<Vec<_>>()
        .join(" OR ");
    // Runtime query: sqlx's compile-time describe crashes on FTS5 virtual tables.
    type Row = (
        i64,
        String,
        String,
        bool,
        f64,
        f64,
        f64,
        f64,
        Option<f64>,
        Option<i64>,
        String,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT f.id, f.name, f.source, f.is_prepared, f.kcal, f.protein_g, f.carbs_g, f.fat_g,
                f.default_portion_g, f.user_id, foods_fts.text
         FROM foods_fts JOIN foods f ON f.id = foods_fts.food_id
         WHERE foods_fts MATCH ? AND (foods_fts.user_id IS NULL OR foods_fts.user_id = ?)
           AND (f.user_id IS NULL OR f.user_id = ?)
         ORDER BY bm25(foods_fts) LIMIT ?",
    )
    .bind(query)
    .bind(user_id)
    .bind(user_id)
    .bind(FTS_LIMIT)
    .fetch_all(db)
    .await?;
    let mut scored: Vec<(f64, bool, Food)> = vec![];
    for (
        id,
        name,
        source,
        is_prepared,
        kcal,
        protein_g,
        carbs_g,
        fat_g,
        default_portion_g,
        owner,
        text,
    ) in rows
    {
        let s = score(name_norm, &text);
        if let Some(seen) = scored.iter_mut().find(|x| x.2.id == id) {
            seen.0 = seen.0.max(s);
            continue;
        }
        let preferred = source == "staple" || owner == Some(user_id);
        scored.push((
            s,
            preferred,
            Food {
                id,
                name,
                source,
                is_prepared,
                kcal,
                protein_g,
                carbs_g,
                fat_g,
                default_portion_g,
            },
        ));
    }
    Ok(classify(scored))
}

/// A food `user_id` may use (shared or their own), by id.
pub async fn visible(db: &SqlitePool, user_id: i64, id: i64) -> Result<Option<Food>, sqlx::Error> {
    sqlx::query_as!(
        Food,
        r#"SELECT id, name, source, is_prepared AS "is_prepared: bool", kcal, protein_g, carbs_g,
                  fat_g, default_portion_g
           FROM foods WHERE id = ? AND (user_id IS NULL OR user_id = ?)"#,
        id,
        user_id
    )
    .fetch_optional(db)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usda_portion_text_becomes_a_unit_and_amount() {
        let u = |t: &str, a: Option<f64>| portion_unit(t, a);
        assert_eq!(u("cup", Some(1.0)), Some(("cup".into(), 1.0)));
        assert_eq!(u("scoop", Some(3.0)), Some(("scoop".into(), 3.0)));
        assert_eq!(u("cups, chopped", Some(0.5)), Some(("cup".into(), 0.5)));
        assert_eq!(
            u("1 piece, medium pizza", None),
            Some(("piece".into(), 1.0))
        );
        assert_eq!(u("2 tablespoons", None), Some(("tbsp".into(), 2.0)));
        assert_eq!(u("1/2 cup", None), Some(("cup".into(), 0.5)));
        assert_eq!(u("fl oz", Some(1.0)), Some(("fl oz".into(), 1.0)));
        assert_eq!(
            u("medium (2-1/4\" to 3\" dia)", Some(1.0)),
            Some(("medium".into(), 1.0))
        );
        assert_eq!(
            u("1 medium or regular slice", None),
            Some(("slice".into(), 1.0))
        );
        assert_eq!(
            u("1 large or thick slice", None),
            Some(("slice".into(), 1.0))
        );
        assert_eq!(
            u("medium whole (2-3/5\" dia)", Some(1.0)),
            Some(("medium".into(), 1.0))
        );
        assert_eq!(u("Quantity not specified", None), None);
        assert_eq!(u("1 surface inch", None), None);
        assert_eq!(u("", Some(1.0)), None);
    }

    #[test]
    fn units_have_canonical_lookup_keys() {
        assert_eq!(unit_keys("Tablespoons"), ["tbsp"]);
        assert_eq!(unit_keys("1 scoop"), ["scoop"]);
        assert_eq!(unit_keys("large egg"), ["large egg", "large"]);
        assert_eq!(unit_keys("slice"), ["slice", "piece"]);
        assert_eq!(unit_keys("fl oz"), ["fl oz"]);
        assert!(unit_keys("").is_empty());
    }

    #[tokio::test]
    async fn staples_get_usda_grams_per_unit() {
        let db = db().await;
        let grams = |name: &'static str, unit: &'static str| {
            let db = db.clone();
            async move {
                sqlx::query_scalar!(
                    "SELECT p.grams FROM food_portions p JOIN foods f ON f.id = p.food_id
                     WHERE f.source = 'staple' AND f.name = ? AND p.unit = ?",
                    name,
                    unit
                )
                .fetch_optional(&db)
                .await
                .unwrap()
            }
        };
        let scoop = grams("Whey protein powder", "scoop").await.unwrap();
        assert!((scoop - 86.0 / 3.0).abs() < 0.01, "{scoop}");
        assert_eq!(
            grams("Oats, rolled, dry", "cup").await,
            Some(81.0),
            "first plain cup wins"
        );
        assert_eq!(grams("Espresso", "fl oz").await, Some(29.6));
        assert_eq!(
            grams("Toast, white bread", "slice").await,
            Some(25.0),
            "a regular slice, not snack-size"
        );
        seed(&db).await.unwrap();
        assert_eq!(
            grams("Oats, rolled, dry", "cup").await,
            Some(81.0),
            "reseeding is idempotent"
        );
    }

    #[test]
    fn normalize_lowercases_strips_punctuation_and_singularizes() {
        assert_eq!(normalize("Cooked White Rice!"), "cooked white rice");
        assert_eq!(
            normalize("Chickpeas (chana), boiled"),
            "chickpea chana boiled"
        );
        assert_eq!(normalize("2 eggs"), "2 egg");
        assert_eq!(normalize("rotis idlis"), "roti idli");
        assert_eq!(normalize("tomatoes"), "tomato");
        assert_eq!(normalize("hummus, glass"), "hummus glass");
        assert_eq!(normalize("  shallow-fried  "), "shallow fried");
        assert_eq!(normalize("berries"), "berry");
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn csv_handles_quotes_and_escapes() {
        assert_eq!(
            csv("a,\"b, c\",\"say \"\"hi\"\"\"\n\nx,,y\n"),
            vec![vec!["a", "b, c", "say \"hi\""], vec!["x", "", "y"]]
        );
    }

    #[test]
    fn score_is_token_overlap() {
        assert_eq!(score("rice white cooked", "cooked white rice"), 1.0);
        assert_eq!(score("chicken", "chicken curry"), 0.75);
        assert!(score("paneer", "paneer") - score("paneer", "palak paneer") >= MARGIN);
        assert_eq!(score("pizza", "rice"), 0.0);
    }

    #[test]
    fn classify_matched_needs_a_high_score_and_a_margin() {
        assert_eq!(
            classify(vec![(1.0, true, "a"), (0.5, true, "b")]),
            Match::Matched("a")
        );
        assert_eq!(
            classify(vec![(0.7, true, "a"), (0.65, true, "b")]),
            Match::Ambiguous(vec!["a", "b"])
        );
    }

    #[test]
    fn classify_between_thresholds_is_ambiguous_with_at_most_three() {
        let c = vec![
            (0.5, true, "a"),
            (0.5, true, "b"),
            (0.4, true, "c"),
            (0.35, true, "d"),
            (0.1, true, "e"),
        ];
        assert_eq!(classify(c), Match::Ambiguous(vec!["a", "b", "c"]));
    }

    #[test]
    fn classify_low_or_empty_is_unmatched() {
        assert_eq!(classify(vec![(0.2, true, "a")]), Match::Unmatched);
        assert_eq!(classify::<&str>(vec![]), Match::Unmatched);
    }

    #[test]
    fn classify_prefers_staples_over_equal_usda_entries() {
        assert_eq!(
            classify(vec![(1.0, false, "usda"), (1.0, true, "staple")]),
            Match::Matched("staple")
        );
    }

    #[test]
    fn shipped_data_parses() {
        let staples = csv(STAPLES);
        assert_eq!(staples[0][0], "name");
        for r in &staples[1..] {
            assert_eq!(r.len(), 10, "{r:?}");
            let kcal: f64 = r[2].parse().unwrap();
            assert!((0.0..=900.0).contains(&kcal), "{r:?}");
            assert!(
                r[8].parse::<i64>().is_ok(),
                "every staple cites an FDC id: {r:?}"
            );
        }
        for r in &csv(ALIASES)[1..] {
            assert_eq!(r.len(), 2, "{r:?}");
        }
    }

    async fn db() -> SqlitePool {
        crate::db::memory().await // seeds
    }

    #[tokio::test]
    async fn seed_is_idempotent() {
        let db = db().await;
        let count = || async {
            sqlx::query_scalar!("SELECT (SELECT count(*) FROM foods) + (SELECT count(*) FROM food_aliases) + (SELECT count(*) FROM foods_fts)")
                .fetch_one(&db)
                .await
                .unwrap()
        };
        let before = count().await;
        seed(&db).await.unwrap();
        assert_eq!(count().await, before);
    }

    #[tokio::test]
    async fn find_uses_aliases_then_search() {
        let db = db().await;
        let name = |m: Match| match m {
            Match::Matched(f) => f.name,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            name(find(&db, 0, "chapati").await.unwrap()),
            "Roti (chapati), whole wheat"
        );
        assert_eq!(
            name(find(&db, 0, &normalize("cooked white rice")).await.unwrap()),
            "Rice, white, cooked"
        );
        assert!(
            matches!(find(&db, 0, "chicken").await.unwrap(), Match::Ambiguous(c) if c.len() > 1)
        );
        assert_eq!(find(&db, 0, "xyzzy").await.unwrap(), Match::Unmatched);
    }
}
