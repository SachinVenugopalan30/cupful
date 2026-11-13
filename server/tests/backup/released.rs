// SPDX-License-Identifier: AGPL-3.0-or-later
//! Backups written by released builds keep restoring. `fixtures/backup-v0.3.2.json` was exported by
//! the v0.3.2 code itself; never regenerate it from a newer build. Add a fixture per release that
//! changes the format, and keep the old ones.

use super::*;

fn fixture(name: &str) -> Value {
    let path = format!("{}/../fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[tokio::test]
async fn a_v0_3_2_backup_restores_into_this_build() {
    let doc = fixture("backup-v0.3.2.json");
    assert_eq!(doc["version"], 1, "the fixture is a version 1 document");

    let (app, _state) = app().await;
    let mut a = admin(&app).await;
    let (status, counts) = restore(&mut a, &doc).await;
    assert_eq!(status, StatusCode::OK, "{counts}");

    let got = export(&mut a).await;
    // A shared Open Food Facts food the target does not have restores as the user's own private
    // copy (see old_backup_with_a_shared_label_food_restores_privately). Everything else, profile
    // versions, days, meals and their numbers, aliases, portions and products, comes back unchanged.
    assert_eq!(
        without_food_owners(&stable(&got)),
        without_food_owners(&stable(&doc))
    );
    let bar = |d: &Value| {
        d["foods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == "Off oat bar")
            .cloned()
            .unwrap()
    };
    assert_eq!(bar(&doc)["source"], "off");
    assert_eq!(bar(&got)["source"], "user");
    assert_eq!(bar(&got)["kcal"], bar(&doc)["kcal"]);
}

/// The document with each food's owner fields (`source`, `source_ref`, `user_id`, and the item's
/// copy of them) removed: the only part a restore into another instance may change.
fn without_food_owners(doc: &Value) -> Value {
    let mut d = doc.clone();
    for food in d["foods"].as_array_mut().unwrap() {
        let f = food.as_object_mut().unwrap();
        for k in ["source", "source_ref", "user_id"] {
            f.remove(k);
        }
    }
    for meal in d["meals"].as_array_mut().unwrap() {
        for item in meal["items"].as_array_mut().unwrap() {
            item.as_object_mut().unwrap().remove("food");
        }
    }
    d
}
