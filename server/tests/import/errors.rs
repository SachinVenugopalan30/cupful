// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rows and headers the importer refuses, each with a line reference where possible.
use axum::http::StatusCode;

use crate::common::*;
use crate::{TZ, csv, error, import};

#[tokio::test]
async fn a_blank_kcal_with_food_is_rejected_but_zero_is_a_value() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let bad = format!("{d},,0,lunch,soup,Soup,1 bowl,,,,,,,,");
    let (status, res) = import(&mut a, &csv(&[&bad])).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("line 2"), "{res}");
    assert!(error(&res).contains("kcal"), "{res}");

    let zero = format!("{d},,0,lunch,soup,Soup,1 bowl,0,0,0,0,,,,");
    let (status, res) = import(&mut a, &csv(&[&zero])).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    let m = crate::meals(&mut a, &d).await;
    assert_eq!(m[0]["items"][0]["kcal"], 0.0);
}

#[tokio::test]
async fn food_without_kcal_and_kcal_without_food_are_both_rejected() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let (status, res) = import(
        &mut a,
        &csv(&[&format!("{d},,0,lunch,soup,Soup,1 bowl,,,,,,,,")]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("kcal is required"), "{res}");

    let (status, res) = import(
        &mut a,
        &csv(&[&format!("{d},,0,lunch,soup,,1 bowl,100,,,,,,,")]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("food is required"), "{res}");
}

#[tokio::test]
async fn duplicate_and_missing_item_indexes_are_rejected_with_a_line() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = "12121212-1212-4212-8212-121212121212";
    let dup = csv(&[
        &format!("{d},{id},0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,"),
        &format!("{d},{id},0,lunch,soup,Tea,1 cup,5,0,1,0,,,,"),
    ]);
    let (status, res) = import(&mut a, &dup).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("line 3"), "{res}");
    assert!(error(&res).contains("duplicate item_index"), "{res}");

    let missing = csv(&[&format!("{d},{id},,lunch,soup,Soup,1 bowl,100,1,2,3,,,,")]);
    let (status, res) = import(&mut a, &missing).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("item_index"), "{res}");
}

#[tokio::test]
async fn an_inconsistent_meal_group_is_rejected() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = "34343434-3434-4434-8434-343434343434";
    let body = csv(&[
        &format!("{d},{id},0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,"),
        &format!("{d},{id},1,lunch,other soup,Tea,1 cup,5,0,1,0,,,,"),
    ]);
    let (status, res) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("line 3"), "{res}");
    assert!(error(&res).contains("inconsistent"), "{res}");
}

#[tokio::test]
async fn files_that_mix_present_and_absent_meal_ids_are_rejected() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = "56565656-5656-4656-8656-565656565656";
    let body = csv(&[
        &format!("{d},{id},0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,"),
        &format!("{d},,1,lunch,soup,Tea,1 cup,5,0,1,0,,,,"),
    ]);
    let (status, res) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("line 3"), "{res}");
    assert!(error(&res).contains("meal_id"), "{res}");
}

#[tokio::test]
async fn unknown_meal_types_and_future_dates_are_rejected_with_a_line() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let (status, res) = import(
        &mut a,
        &csv(&[&format!("{d},,0,brunch,soup,Soup,1 bowl,100,1,2,3,,,,")]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("line 2"), "{res}");

    let future = today(TZ).tomorrow().unwrap().tomorrow().unwrap();
    let (status, res) = import(
        &mut a,
        &csv(&[&format!("{future},,0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,")]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("line 2"), "{res}");
    assert!(error(&res).contains("future"), "{res}");
}

#[tokio::test]
async fn a_wrong_header_is_rejected() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (status, res) = import(&mut a, "date,meal\ntoday,lunch\n").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("header"), "{res}");
}
