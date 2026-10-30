// SPDX-License-Identifier: AGPL-3.0-or-later
//! The instruction and JSON Schema sent to the AI provider.

use serde_json::{Value, json};

use super::MAX_ITEMS;

pub(super) const INSTRUCTION: &str = "You turn a meal description (and photo, if any) into a list of foods. \
Reply with JSON only, matching the schema. One item per distinct food: split foods eaten together \
(\"toast with butter\" is toast and butter), but keep a named dish whole (a burrito, pizza, sandwich \
or curry is one item). List every food and drink mentioned, even small amounts (a splash of \
milk, a teaspoon of sugar). For each item give: \
`food`, a plain generic name (e.g. \"cooked white rice\", \"chicken breast\"); `amount` and `unit` \
as the user said them (e.g. 1 \"small cup\", 2 \"slices\", 150 \"g\"); `grams`, your best estimate of \
the edible weight; `cooking`, the method (\"none\", \"boiled\", \"shallow-fried\", \"deep-fried\", \
\"sauteed\", \"tadka\", \"roasted in oil\", \"grilled\", ...); `mentions_oil`, true only if the user \
mentioned oil, ghee or butter; `estimate`, your kcal, protein, carbs and fat in grams for the whole \
item. Don't add cooking oil as its own item unless the user listed it. `assumptions` is one short \
sentence; `confidence` is low, medium or high.";

pub(super) fn schema() -> Value {
    let num = json!({"type": "number"});
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["items", "assumptions", "confidence"],
        "properties": {
            "items": {"type": "array", "maxItems": MAX_ITEMS, "items": {
                "type": "object",
                "additionalProperties": false,
                "required": ["food", "amount", "unit", "grams", "cooking", "mentions_oil", "estimate"],
                "properties": {
                    "food": {"type": "string"}, "amount": num, "unit": {"type": "string"},
                    "grams": num, "cooking": {"type": "string"}, "mentions_oil": {"type": "boolean"},
                    "estimate": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["kcal", "protein", "carbs", "fat"],
                        "properties": {"kcal": num, "protein": num, "carbs": num, "fat": num}
                    }
                }
            }},
            "assumptions": {"type": "string"},
            "confidence": {"type": "string", "enum": ["low", "medium", "high"]}
        }
    })
}
