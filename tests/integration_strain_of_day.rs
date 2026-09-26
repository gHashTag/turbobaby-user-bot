//! The strain-of-day carousel is retired, and this file holds the retirement.
//!
//! Until 2026-09-26 this file held five tests of the carousel's cap (cycle
//! #52: off-sale strains holding slots invisibly, so the owner could not pick
//! a strain of the day at all). Each seeded `strains` rows, and migration 083
//! dropped that table with the rest of the old catalogue, so all five failed
//! on every database migrated to today's schema. The feature went with the
//! table: the owner's rulings of 2026-09-24 (rental only, Phuket only) and
//! 2026-09-25 («всё что касается канабиса нигде не должно быть») retired it,
//! no router merges the strain routes (`src/api/mod.rs`), and the bot's
//! strain-of-day buttons answer with the rental menu (`src/bot/callbacks.rs`).
//! The rental catalogue has no carousel of its own for a cap to guard.
//!
//! So the five tests of the cap were removed (git keeps them: 6580911 wrote
//! them), and what is tested instead is that the retired thing cannot be
//! reached: the write the admin screen used and the read the carousel used
//! both answer the API's JSON not-found, and the table they read and wrote is
//! gone. `db.get_strains_of_day` still exists in `src/db/mod.rs` and reads that
//! table; nothing routed calls it, and it is not exercised here.
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

fn admin_token() -> String {
    turbobaby_bot::api::auth::generate_admin_token("test_password", "dummy_test_token")
}

async fn call(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("X-Admin-Token", admin_token());
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let request = request
        .body(match body {
            Some(b) => Body::from(serde_json::to_vec(&b).unwrap()),
            None => Body::empty(),
        })
        .unwrap();
    let response = app.oneshot(request).await.expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

/// The admin's pick and the carousel's read: neither is served.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_strain_of_day_routes_are_not_served() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    let pick = json!({ "is_strain_of_day": true, "discount": 15.0 });
    for (method, uri, body) in [
        (
            "PUT",
            "/api/strains/stored-item-4p/strain-of-day",
            Some(pick.clone()),
        ),
        (
            "PUT",
            "/api/strains/stored-item-4p/strain-of-day",
            Some(json!({ "is_strain_of_day": false, "discount": 0.0 })),
        ),
        ("GET", "/api/strains/strain-of-day", None),
        ("GET", "/api/strains", None),
    ] {
        let (status, body) = call(app.clone(), method, uri, body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {uri}: {body}");
        assert_eq!(
            body["error"], "not_found",
            "{method} {uri}: the API's own miss: {body}"
        );
    }
}

/// And there is nothing left for them to read or write.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_table_the_carousel_read_is_gone() {
    let Some((_app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };
    let row = db
        .orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT to_regclass('public.strains') IS NULL AS gone".to_string(),
        ))
        .await
        .expect("query")
        .expect("one row");
    assert!(
        row.try_get::<bool>("", "gone").expect("a flag"),
        "migration 083 dropped the table"
    );
}
