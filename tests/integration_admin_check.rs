//! Integration tests for `GET /api/admin/check` — the admin-status endpoint.
//!
//! End-to-end coverage (via `tower::ServiceExt::oneshot` against the real
//! `Router`) for the Wave #36 fix (W-69): the password-token auth path must
//! report `telegram_id: 0` (the sentinel `check_admin` uses for shared-password
//! auth), NEVER the client-supplied `?telegram_id=` query value — unverified
//! input must not be echoed back as the authenticated identity (OWASP A01).
//!
//! `#[ignore]` like the rest of the integration suite: needs a connectable
//! `DATABASE_URL` (this endpoint does no DB query itself, but `make_app`
//! builds the full `AppState`, which connects to PG). Run with a THROWAWAY DB:
//!
//! ```sh
//! DATABASE_URL=postgres://postgres@localhost/woody_test \
//!   cargo test --features backend --test integration_admin_check -- --ignored --test-threads=1
//! ```
//!
//! `--test-threads=1`: each test builds its own `AppState` and runs
//! `run_migrations()`; concurrent DDL on one DB deadlocks, so serialize.
//! NEVER point `DATABASE_URL` at a shared/production database — the harness
//! migrates and the suite writes rows. Use a throwaway local DB.

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn admin_check_without_auth_is_unauthorized() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/check?telegram_id=999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot");

    // No initData, no X-Admin-Token → deny.
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn admin_check_token_path_returns_sentinel_zero_not_echoed_id() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };

    // test_config (tests/common/mod.rs) uses admin_password "test_password"
    // and bot_token "dummy_test_token".
    let token =
        woody_weed_bot::api::auth::generate_admin_token("test_password", "dummy_test_token");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/check?telegram_id=999") // attacker-claimed id
                .header("X-Admin-Token", token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot");

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    assert_eq!(body["is_admin"], true, "valid token must authenticate");
    // The W-69 regression assertion: the response must NOT echo the
    // client-supplied 999; password auth reports the sentinel 0.
    assert_eq!(
        body["telegram_id"], 0,
        "password-token path must report telegram_id 0, never the client-supplied query id"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn admin_check_with_bad_token_is_unauthorized() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/check?telegram_id=42")
                .header("X-Admin-Token", "not-the-real-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
