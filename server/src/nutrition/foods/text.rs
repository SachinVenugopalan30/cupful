// SPDX-License-Identifier: AGPL-3.0-or-later
//! Name normalization and household-unit parsing.

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
