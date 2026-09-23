//! The delivery-zone picker must offer places the shop can actually reach.
//!
//! Migration 060 seeded a single placeholder zone named "Паттайя" — a city on
//! the mainland — and nothing replaced it. Migration 071 replaced it with the
//! villages of another shop's island (Koh Phangan), which were no more
//! reachable. Migration 087 (owner, 2026-09-24) deactivates those and inserts
//! TurboBaby's own 18 Phuket zones at the owner's fees, with no ETA: none is
//! published, so the API serves `null` for both minute fields. This pins it.
//!
//! Marked `#[ignore]` — runs with:
//!
//! ```sh
//! DATABASE_URL=postgres://... cargo test --features backend -- --ignored
//! ```

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

async fn zones(app: axum::Router) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/delivery/zones")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_picker_never_offers_the_mainland_placeholder() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let (status, body) = zones(app).await;
    assert_eq!(status, StatusCode::OK);

    let names: Vec<String> = body["zones"]
        .as_array()
        .expect("zones array")
        .iter()
        .flat_map(|z| {
            [
                z["name"].as_str().unwrap_or_default().to_string(),
                z["name_en"].as_str().unwrap_or_default().to_string(),
            ]
        })
        .collect();

    assert!(
        !names
            .iter()
            .any(|n| n.contains("Паттайя") || n.contains("Pattaya")),
        "the shop is on Phuket; Pattaya must not be selectable: {names:?}"
    );
}

/// The owner's Phuket table (owner, 2026-09-24): `(name_en, fee)`.
const PHUKET_ZONES: [(&str, f64); 18] = [
    ("Bang Tao", 290.0),
    ("Surin", 290.0),
    ("Kamala", 290.0),
    ("Patong", 290.0),
    ("Thalang North", 390.0),
    ("Karon", 390.0),
    ("Phuket Town", 490.0),
    ("Pa Khlok", 490.0),
    ("Thalang East", 490.0),
    ("Kata", 490.0),
    ("Kathu", 490.0),
    ("Rawai", 590.0),
    ("Chalong", 590.0),
    ("Cape Panwa", 590.0),
    ("Nai Thon", 590.0),
    ("Nai Harn", 590.0),
    ("Airport", 690.0),
    ("Mai Khao", 990.0),
];

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_picker_offers_the_owners_phuket_zones_at_his_fees() {
    // An empty picker is as broken as a wrong one — the customer then has no
    // delivery option at all.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let (status, body) = zones(app).await;
    assert_eq!(status, StatusCode::OK);

    let offered: Vec<(String, f64)> = body["zones"]
        .as_array()
        .expect("zones array")
        .iter()
        .map(|z| {
            (
                z["name_en"].as_str().unwrap_or_default().to_string(),
                z["delivery_fee_baht"].as_f64().unwrap_or(-1.0),
            )
        })
        .collect();

    for (expected, fee) in PHUKET_ZONES {
        assert!(
            offered.iter().any(|(n, f)| n == expected && *f == fee),
            "{expected} must be offered at {fee}: {offered:?}"
        );
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_picker_never_offers_the_other_islands_villages() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let (status, body) = zones(app).await;
    assert_eq!(status, StatusCode::OK);

    let names: Vec<String> = body["zones"]
        .as_array()
        .expect("zones array")
        .iter()
        .map(|z| z["name_en"].as_str().unwrap_or_default().to_string())
        .collect();

    for village in ["Thong Sala", "Haad Rin", "Srithanu", "Bottle Beach"] {
        assert!(
            !names.iter().any(|n| n == village),
            "{village} is on Koh Phangan and must not be offered: {names:?}"
        );
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn every_offered_zone_has_a_usable_fee_and_no_eta() {
    // The fee drives what the customer is shown before paying, so a negative
    // one would show as nonsense at checkout. No ETA is published (owner,
    // 2026-09-24), so both minute fields are null for every offered zone.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let (_, body) = zones(app).await;
    for z in body["zones"].as_array().expect("zones array") {
        let fee = z["delivery_fee_baht"].as_f64().unwrap_or(-1.0);
        let name = z["name"].as_str().unwrap_or("?");
        assert!(fee >= 0.0 && fee.is_finite(), "{name}: bad fee {fee}");
        assert!(
            z["min_eta_minutes"].is_null() && z["max_eta_minutes"].is_null(),
            "{name}: an ETA nobody published: {z}"
        );
    }
}
