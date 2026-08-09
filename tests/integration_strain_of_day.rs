//! Integration tests for the strain-of-day carousel cap.
//!
//! Regression: the owner could not pick a strain of the day at all. Every
//! `PUT /api/strains/:id/strain-of-day` answered 409 `sotd_limit` while
//! `GET /api/strains/strain-of-day` returned an empty list — so the message
//! "at most 3, unset one" named a set the owner could not see anywhere.
//!
//! Cause: the write-side cap counted `is_strain_of_day = true` only, while
//! the read side (`Database::get_strains_of_day`) also requires
//! `is_available = true`. Strains flagged as strain-of-day and later taken
//! off sale therefore held carousel slots invisibly. Three of them
//! deadlocked the feature permanently.
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
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::json;
use tower::ServiceExt;

/// Seed a strain and return its id.
async fn seed_strain(db: &woody_weed_bot::db::Database, name: &str, available: bool) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO strains (id, name, price_per_gram, is_available) \
             VALUES ($1, $2, $3, $4)",
            [
                id.clone().into(),
                name.into(),
                100.0_f64.into(),
                available.into(),
            ],
        ))
        .await
        .expect("seed strain INSERT");
    id
}

/// Flag a strain as strain-of-day directly in the DB, bypassing the API —
/// this is the state the owner's database was already in.
async fn flag_as_sotd(db: &woody_weed_bot::db::Database, id: &str) {
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE strains SET is_strain_of_day = TRUE, strain_of_day_set_at = NOW() \
             WHERE id = $1",
            [id.into()],
        ))
        .await
        .expect("flag strain-of-day UPDATE");
}

/// Clear every strain-of-day flag so each test starts from a known carousel.
async fn clear_all_sotd(db: &woody_weed_bot::db::Database) {
    db.orm
        .execute(Statement::from_string(
            DbBackend::Postgres,
            "UPDATE strains SET is_strain_of_day = FALSE",
        ))
        .await
        .expect("clear strain-of-day UPDATE");
}

fn admin_token() -> String {
    woody_weed_bot::api::auth::generate_admin_token("test_password", "dummy_test_token")
}

struct Resp {
    status: StatusCode,
    body: serde_json::Value,
}

async fn set_sotd(app: axum::Router, id: &str, enabled: bool, discount: f64) -> Resp {
    let request = Request::builder()
        .method("PUT")
        .uri(format!("/api/strains/{id}/strain-of-day"))
        .header("content-type", "application/json")
        .header("X-Admin-Token", admin_token())
        .body(Body::from(
            serde_json::to_vec(&json!({
                "is_strain_of_day": enabled,
                "discount": discount,
            }))
            .unwrap(),
        ))
        .unwrap();
    let response = app.oneshot(request).await.expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    Resp { status, body }
}

/// The exact production deadlock: three flagged-but-unavailable strains must
/// not consume carousel slots, because the carousel never shows them.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn unavailable_featured_strains_do_not_block_a_new_pick() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };
    clear_all_sotd(&db).await;

    let suffix = rand_suffix();
    for i in 0..3 {
        let id = seed_strain(&db, &format!("sotd-offsale-{suffix}-{i}"), false).await;
        flag_as_sotd(&db, &id).await;
    }

    // The carousel is empty — nothing the owner could possibly unset.
    let shown = db.get_strains_of_day().await.expect("read side");
    assert!(
        shown.is_empty(),
        "off-sale strains must not appear in the carousel, got {shown:?}"
    );

    let target = seed_strain(&db, &format!("sotd-target-{suffix}"), true).await;
    let resp = set_sotd(app, &target, true, 15.0).await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "an invisible off-sale strain must not hold a slot, body: {}",
        resp.body
    );

    let shown = db.get_strains_of_day().await.expect("read side");
    assert_eq!(shown.len(), 1, "the new pick must reach the carousel");
    assert_eq!(shown[0].id, target);
}

/// The cap still applies to strains that really do occupy the carousel, and
/// the refusal names them so the owner knows what to unset.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn three_available_featured_strains_still_cap_the_carousel() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };
    clear_all_sotd(&db).await;

    let suffix = rand_suffix();
    let mut expected = Vec::new();
    for i in 0..3 {
        let name = format!("sotd-onsale-{suffix}-{i}");
        let id = seed_strain(&db, &name, true).await;
        flag_as_sotd(&db, &id).await;
        expected.push(name);
    }

    let target = seed_strain(&db, &format!("sotd-fourth-{suffix}"), true).await;
    let resp = set_sotd(app, &target, true, 10.0).await;
    assert_eq!(
        resp.status,
        StatusCode::CONFLICT,
        "a fourth featured strain must be refused, body: {}",
        resp.body
    );
    assert_eq!(resp.body["error"], "sotd_limit");

    // The owner must be told which three are holding the slots.
    let current: Vec<String> = resp.body["current"]
        .as_array()
        .expect("409 must list the current strains")
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect();
    for name in &expected {
        assert!(
            current.contains(name),
            "{name} occupies a slot but is missing from the 409 payload: {current:?}"
        );
    }
}

/// Re-picking a strain that is already featured must not count itself out.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn updating_an_already_featured_strain_is_allowed_at_the_cap() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };
    clear_all_sotd(&db).await;

    let suffix = rand_suffix();
    let mut ids = Vec::new();
    for i in 0..3 {
        let id = seed_strain(&db, &format!("sotd-self-{suffix}-{i}"), true).await;
        flag_as_sotd(&db, &id).await;
        ids.push(id);
    }

    // Changing the discount on one of the three is not a fourth pick.
    let resp = set_sotd(app, &ids[0], true, 42.0).await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "editing an existing pick must not hit the cap, body: {}",
        resp.body
    );

    let shown = db.get_strains_of_day().await.expect("read side");
    let updated = shown
        .iter()
        .find(|s| s.id == ids[0])
        .expect("the edited strain must still be featured");
    assert_eq!(
        updated.strain_of_day_discount, 42.0,
        "the new discount must be persisted"
    );
}

/// Unsetting is always allowed — it is the owner's only way out of a full
/// carousel, so it must never be gated by the cap.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn unsetting_is_always_allowed() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };
    clear_all_sotd(&db).await;

    let suffix = rand_suffix();
    let mut ids = Vec::new();
    for i in 0..3 {
        let id = seed_strain(&db, &format!("sotd-unset-{suffix}-{i}"), true).await;
        flag_as_sotd(&db, &id).await;
        ids.push(id);
    }

    let resp = set_sotd(app, &ids[0], false, 0.0).await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "unsetting must always succeed, body: {}",
        resp.body
    );

    let shown = db.get_strains_of_day().await.expect("read side");
    assert!(
        !shown.iter().any(|s| s.id == ids[0]),
        "the unset strain must leave the carousel"
    );
}

/// The write cap and the read query must agree about what "featured" means.
/// They disagreed for `is_available`, which is what caused the deadlock.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn cap_and_carousel_agree_on_what_occupies_a_slot() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };
    clear_all_sotd(&db).await;

    let suffix = rand_suffix();
    // Two visible + two invisible: only the visible pair holds slots, so a
    // third pick must still be accepted.
    for i in 0..2 {
        let id = seed_strain(&db, &format!("sotd-mix-on-{suffix}-{i}"), true).await;
        flag_as_sotd(&db, &id).await;
    }
    for i in 0..2 {
        let id = seed_strain(&db, &format!("sotd-mix-off-{suffix}-{i}"), false).await;
        flag_as_sotd(&db, &id).await;
    }

    let visible = db.get_strains_of_day().await.expect("read side").len();
    assert_eq!(visible, 2, "only available strains occupy the carousel");

    let target = seed_strain(&db, &format!("sotd-mix-third-{suffix}"), true).await;
    let resp = set_sotd(app, &target, true, 5.0).await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "the third visible pick must be accepted, body: {}",
        resp.body
    );
    assert_eq!(
        db.get_strains_of_day().await.expect("read side").len(),
        3,
        "carousel must now hold exactly three"
    );
}

fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}
