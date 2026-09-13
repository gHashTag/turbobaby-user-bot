//! You could not pick a seed in the garden, and nothing was broken.
//!
//! `GET /api/garden/products` ordered by `catalog, name` — by the *spelling* of
//! the category. Alphabetically `accessory` precedes `strain`, so measured
//! against production the list came back as 70 accessories, then 7 sets, then
//! the 18 strains at positions 77–94, then 31 teas. The picker is a two-column
//! grid inside an 85vh sheet, so a seed was about thirty-nine rows below the
//! fold, behind the grinders and the rolling papers. Every request succeeded;
//! every seed was present; nobody could find one.
//!
//! The ordering is now a written rank — a `strain` is the seed, a `set`
//! contains strains, nothing else is a plant — rather than whatever the
//! alphabet happens to do to the category names.
//!
//! Run with:
//! ```sh
//! DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/woody_test \
//!   cargo test --features backend --test integration_garden_products_order -- --ignored --test-threads=1
//! ```

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::make_app_with_db;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use tower::ServiceExt;

async fn products(app: &axum::Router) -> Vec<serde_json::Value> {
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/garden/products")
        .body(Body::empty())
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 22)
        .await
        .expect("body");
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    v["products"].as_array().cloned().unwrap_or_default()
}

fn catalogs(list: &[serde_json::Value]) -> Vec<String> {
    list.iter()
        .map(|p| p["catalog"].as_str().unwrap_or("").to_string())
        .collect()
}

/// Seed the catalog so the test does not depend on what the shop happens to
/// stock. An accessory named `AAA` is the adversarial case: under the old
/// ordering it sorted to position zero and pushed every seed down.
async fn seed(db: &turbobaby_bot::db::Database) {
    for sql in [
        "INSERT INTO accessories (id, name, price, is_available, garden_eligible) \
         VALUES ('test-acc-aaa', 'AAA test accessory', 100, true, true) \
         ON CONFLICT (id) DO UPDATE SET is_available = true, garden_eligible = true",
        "INSERT INTO strains (id, name, price_per_gram, is_available, garden_eligible) \
         VALUES ('test-strain-zzz', 'ZZZ test strain', 300, true, true) \
         ON CONFLICT (id) DO UPDATE SET is_available = true, garden_eligible = true",
    ] {
        db.orm
            .execute(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await
            .expect("seed the catalog");
    }
}

#[tokio::test]
#[ignore]
async fn a_seed_is_the_first_thing_offered_in_the_garden() {
    let Some((app, db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };
    seed(&db).await;

    let list = products(&app).await;
    assert!(
        !list.is_empty(),
        "the garden offered nothing at all; this test cannot say anything about order"
    );
    let cats = catalogs(&list);
    assert!(
        cats.iter().any(|c| c == "strain"),
        "no strain is garden-eligible, so the ordering is untested: {cats:?}"
    );

    assert_eq!(
        cats.first().map(String::as_str),
        Some("strain"),
        "the first thing a gardener is offered must be a seed, not a {:?}",
        cats.first()
    );

    // Every seed before anything that is not a seed. The discriminating check:
    // under the old `ORDER BY catalog, name` an accessory named 'AAA' — seeded
    // above — sorts first, so this fails on the shipped behaviour rather than
    // merely describing the new one.
    let last_strain = cats.iter().rposition(|c| c == "strain").expect("checked");
    let first_other = cats
        .iter()
        .position(|c| c != "strain")
        .unwrap_or(cats.len());
    assert!(
        last_strain < first_other,
        "seeds are interleaved with everything else: last seed at {last_strain}, \
         first non-seed at {first_other}"
    );
}

/// Sets contain strains, so they come next; everything that is not a plant
/// comes after both.
#[tokio::test]
#[ignore]
async fn what_is_not_a_plant_comes_after_what_is() {
    let Some((app, db)) = make_app_with_db().await else {
        return;
    };
    seed(&db).await;
    let cats = catalogs(&products(&app).await);

    let rank = |c: &str| match c {
        "strain" => 0,
        "set" => 1,
        _ => 2,
    };
    let ranks: Vec<u8> = cats.iter().map(|c| rank(c)).collect();
    let mut sorted = ranks.clone();
    sorted.sort_unstable();
    assert_eq!(
        ranks, sorted,
        "the list is not in seed → set → everything-else order: {cats:?}"
    );
}
