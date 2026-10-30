// SPDX-License-Identifier: AGPL-3.0-or-later
//! Daily calorie budget (PLAN.md §7). Pure functions; f64 throughout, rounded only on serialization.

pub use jiff::civil::Date;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Sex {
    Male,
    Female,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProfileVersion {
    pub effective_from: Date,
    pub sex: Sex,
    pub birth_year: i16,
    pub height_cm: f64,
    pub start_weight_kg: f64,
    pub goal_kg: Option<f64>,
    pub pace_kg_per_week: f64,
    pub typical_steps: i64,
}

/// What was logged for the day. Missing steps make the budget provisional.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct DayActivity {
    pub steps: Option<i64>,
    pub gym_minutes: Option<i64>,
    #[serde(default)]
    pub trained: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Budget {
    pub date: Date,
    pub age: i16,
    #[serde(serialize_with = "grams")]
    pub weight_kg: f64,
    pub steps: i64,
    pub met: u8,
    #[serde(serialize_with = "kcal")]
    pub bmr: f64,
    #[serde(serialize_with = "kcal")]
    pub base: f64,
    #[serde(serialize_with = "kcal")]
    pub steps_kcal: f64,
    #[serde(serialize_with = "kcal")]
    pub gym_kcal: f64,
    #[serde(serialize_with = "kcal")]
    pub maintenance: f64,
    #[serde(serialize_with = "kcal")]
    pub deficit: f64,
    #[serde(serialize_with = "kcal")]
    pub floor: f64,
    #[serde(serialize_with = "kcal")]
    pub budget: f64,
    #[serde(serialize_with = "grams")]
    pub protein_g: f64,
    #[serde(serialize_with = "grams")]
    pub fat_g: f64,
    #[serde(serialize_with = "grams")]
    pub carbs_g: f64,
    pub flags: Vec<&'static str>,
}

/// `f64::round` rounds half away from zero, as §7 asks.
fn kcal<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_i64(v.round() as i64)
}

fn grams<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_f64(crate::nutrition::r1(*v))
}

/// The profile version in effect on `date`.
pub fn version_on(date: Date, versions: &[ProfileVersion]) -> Option<&ProfileVersion> {
    versions
        .iter()
        .filter(|v| v.effective_from <= date)
        .max_by_key(|v| v.effective_from)
}

/// `logged_weight` is the most recent weigh-in on or before `date`, if any.
pub fn compute(
    date: Date,
    v: &ProfileVersion,
    logged_weight: Option<f64>,
    day: &DayActivity,
) -> Budget {
    let mut flags = Vec::new();
    let kg = logged_weight.unwrap_or(v.start_weight_kg);
    // Approximate by up to a year (about 5 kcal of BMR); birthdays aren't stored.
    let age = date.year() - v.birth_year;
    let steps = day.steps.unwrap_or_else(|| {
        flags.push("provisional");
        v.typical_steps
    });
    let male = v.sex == Sex::Male;

    let bmr =
        10.0 * kg + 6.25 * v.height_cm - 5.0 * f64::from(age) + if male { 5.0 } else { -161.0 };
    let base = bmr * 1.2;
    let steps_kcal = (steps - 2500).max(0) as f64 * 0.00057 * kg;
    let met = if day.trained == ["Cardio"] { 7 } else { 5 };
    let gym_kcal = f64::from(met - 1) * kg * day.gym_minutes.unwrap_or(0) as f64 / 60.0;
    let maintenance = base + steps_kcal + gym_kcal;

    let mut deficit = v.pace_kg_per_week * 7700.0 / 7.0;
    if deficit > 0.25 * maintenance {
        deficit = 0.25 * maintenance;
        flags.push("deficit_capped");
    }
    // A convention, not a derivation.
    let floor = if male { 1500.0 } else { 1200.0 };
    if floor > maintenance - deficit {
        flags.push("floor_applied");
    }
    if floor > maintenance {
        flags.push("floor_above_maintenance");
    }
    // The outer min only matters when the floor is above maintenance.
    let budget = maintenance.min(floor.max(maintenance - deficit));

    let protein_g = 1.6 * v.goal_kg.unwrap_or(kg);
    let protein_kcal = 4.0 * protein_g;
    let fat_g = (0.25 * budget).min(budget - protein_kcal).max(0.0) / 9.0;
    let carbs_g = (budget - protein_kcal - 9.0 * fat_g).max(0.0) / 4.0;
    if protein_kcal + 0.25 * budget > budget {
        flags.push("macros_overcommitted");
    }

    Budget {
        date,
        age,
        weight_kg: kg,
        steps,
        met,
        bmr,
        base,
        steps_kcal,
        gym_kcal,
        maintenance,
        deficit,
        floor,
        budget,
        protein_g,
        fat_g,
        carbs_g,
        flags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn fixtures() {
        let cases: Vec<Value> =
            serde_json::from_str(include_str!("../../fixtures/budget_cases.json")).unwrap();
        assert!(cases.len() >= 15);
        for case in cases {
            let name = case["name"].as_str().unwrap();
            let date: Date = serde_json::from_value(case["date"].clone()).unwrap();
            let versions: Vec<ProfileVersion> =
                serde_json::from_value(case["versions"].clone()).unwrap();
            let day: DayActivity = serde_json::from_value(case["day"].clone()).unwrap();
            let weight = case["weight_kg"].as_f64();
            let Some(v) = version_on(date, &versions) else {
                assert!(
                    case["expect"].is_null(),
                    "{name}: expected a profile version"
                );
                continue;
            };
            let got = serde_json::to_value(compute(date, v, weight, &day)).unwrap();
            for (key, want) in case["expect"].as_object().expect(name) {
                assert_eq!(&got[key], want, "{name}: {key}");
            }
        }
    }
}
