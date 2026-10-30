// SPDX-License-Identifier: AGPL-3.0-or-later
//! Matching a name against the foods a user may see.

use sqlx::SqlitePool;

use super::{AMBIGUOUS_MIN, Food, MARGIN, MATCHED_MIN, MAX_CANDIDATES, Match};

/// FTS rows fetched (bm25 order) before scoring.
const FTS_LIMIT: i64 = 50;

/// Token overlap of query `q` with name `name`, 0..=1: the mean of how much of the query the name
/// covers and their Jaccard overlap, so "paneer" prefers "Paneer" over "Palak paneer".
pub(super) fn score(q: &str, name: &str) -> f64 {
    let q: std::collections::HashSet<_> = q.split(' ').collect();
    let n: std::collections::HashSet<_> = name.split(' ').collect();
    let common = q.intersection(&n).count() as f64;
    (common / q.len() as f64 + common / q.union(&n).count() as f64) / 2.0
}

/// `scored` is (score, preferred, food) in search order; preferred = a staple or the user's own food.
pub(super) fn classify<T>(mut scored: Vec<(f64, bool, T)>) -> Match<T> {
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
