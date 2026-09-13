//! Integration tests for `POST /api/quest/locations` (admin location create).
//!
//! End-to-end coverage for the Wave #42 fix (W-77): the create handler returns
//! the real BIGSERIAL `id` from `INSERT … RETURNING id`, never a fabricated
//! `id: 0`. Also covers the admin-gate (no token → 401) and that the created
//! location is then retrievable via the list endpoint (exercises
//! `get_quest_locations`' fail-loud id read too).
//!
//! `#[ignore]` like the rest of the suite. Run against a THROWAWAY local DB:
//!
//! ```sh
//! DATABASE_URL=postgres://postgres@localhost/woody_test \
//!   cargo test --features backend --test integration_quest_location -- --ignored --test-threads=1
//! ```

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use turbobaby_bot::api::auth::generate_admin_token;

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn create_quest_location_without_admin_is_unauthorized() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/quest/locations")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"unauth loc"}"#))
                .unwrap(),
        )
        .await
        .expect("router.oneshot");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn create_quest_location_returns_real_positive_id() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let token = generate_admin_token("test_password", "dummy_test_token");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/quest/locations")
                .header("content-type", "application/json")
                .header("x-admin-token", &token)
                .body(Body::from(
                    r#"{"name":"e2e quest loc","category":"location"}"#,
                ))
                .unwrap(),
        )
        .await
        .expect("router.oneshot");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    assert_eq!(body["success"], true);

    // W-77 regression: id must be the real auto-generated BIGSERIAL — a
    // positive integer, NOT the fabricated `0` the old `.unwrap_or(0)` returned.
    let id = body["id"].as_i64().expect("id must be an integer");
    assert!(
        id > 0,
        "created location id must be a real positive id, got {id}"
    );

    // Round-trip: the new id must appear in the list (also exercises
    // get_quest_locations' fail-loud id read).
    let list = app
        .oneshot(
            Request::builder()
                .uri("/api/quest/locations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot list");
    assert_eq!(list.status(), StatusCode::OK);
    let bytes = list.into_body().collect().await.unwrap().to_bytes();
    let list_body: serde_json::Value = serde_json::from_slice(&bytes).expect("list json");
    let found = list_body["locations"]
        .as_array()
        .expect("locations array")
        .iter()
        .any(|loc| loc["id"].as_i64() == Some(id));
    assert!(
        found,
        "created location id {id} must be retrievable in the list"
    );
}
