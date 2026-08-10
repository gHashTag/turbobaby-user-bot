//! The delivery-zone picker must offer places the shop can actually reach.
//!
//! Migration 060 seeded a single placeholder zone named "Паттайя" — a city on
//! the mainland, roughly 700 km from the shop — and nothing replaced it. For
//! months the only delivery zone a customer could choose at checkout was the
//! wrong province, while the real island zones sat unused in
//! `src/delivery.rs`. Migration 071 corrects the table; this pins it.
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
        "the shop is on Koh Phangan; Pattaya must not be selectable: {names:?}"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_picker_offers_the_island_zones() {
    // An empty picker is as broken as a wrong one — the customer then has no
    // delivery option at all.
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

    for expected in ["Thong Sala", "Haad Rin", "Srithanu", "Bottle Beach"] {
        assert!(
            names.iter().any(|n| n == expected),
            "{expected} must be offered: {names:?}"
        );
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn every_offered_zone_has_a_usable_fee_and_eta() {
    // These drive what the customer is quoted before paying, so a negative fee
    // or a backwards ETA window would show as nonsense at checkout.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let (_, body) = zones(app).await;
    for z in body["zones"].as_array().expect("zones array") {
        let fee = z["delivery_fee_baht"].as_f64().unwrap_or(-1.0);
        let min = z["min_eta_minutes"].as_i64().unwrap_or(-1);
        let max = z["max_eta_minutes"].as_i64().unwrap_or(-1);
        let name = z["name"].as_str().unwrap_or("?");
        assert!(fee >= 0.0 && fee.is_finite(), "{name}: bad fee {fee}");
        assert!(min >= 0, "{name}: negative min ETA");
        assert!(
            max >= min,
            "{name}: ETA window runs backwards ({min}..{max})"
        );
    }
}
