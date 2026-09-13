//! Integration tests for `check_owner` — the owner-gate guarding all
//! per-user data endpoints (e.g. `GET /api/loyalty/:telegram_id`).
//!
//! Exercises the gate end-to-end with REAL forged Telegram `initData` (signed
//! HMAC via `common::make_init_data`), through the live `Router`:
//!   - a request whose initData user == the path id is ACCEPTED;
//!   - a request whose initData user != the path id is FORBIDDEN (403);
//!   - a request with no initData is UNAUTHORIZED (401).
//! All-API: it seeds the profile via the admin `add_bonus` endpoint, so no raw
//! SQL / schema knowledge is needed.
//!
//! `#[ignore]` like the rest of the suite. Run against a THROWAWAY local DB:
//!
//! ```sh
//! DATABASE_URL=postgres://postgres@localhost/woody_test \
//!   cargo test --features backend --test integration_owner_auth -- --ignored --test-threads=1
//! ```

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;
use turbobaby_bot::api::auth::generate_admin_token;

const BOT_TOKEN: &str = "dummy_test_token";

async fn add_bonus_as_admin(app: &axum::Router, tid: i64, amount: f64) -> StatusCode {
    let token = generate_admin_token("test_password", BOT_TOKEN);
    let body = json!({ "amount": amount, "tx_type": "e2e_seed" });
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/loyalty/{tid}/bonus"))
                .header("content-type", "application/json")
                .header("x-admin-token", token)
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .expect("router.oneshot add_bonus")
        .status()
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn owner_gate_accepts_matching_init_data_user() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid: i64 = 555_100_000 + (std::process::id() as i64 % 100_000);

    // Seed a loyalty profile for `tid` via the admin endpoint.
    assert_eq!(add_bonus_as_admin(&app, tid, 50.0).await, StatusCode::OK);

    // Owner request: initData user == path id → accepted, profile returned.
    let init_data = common::make_init_data(tid, BOT_TOKEN);
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/loyalty/{tid}"))
                .header("X-Telegram-Init-Data", init_data)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot get_profile");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(body["profile"]["telegram_id"].as_i64(), Some(tid));
    assert_eq!(body["profile"]["bonus_balance"].as_f64(), Some(50.0));
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn owner_gate_rejects_mismatched_init_data_user() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let victim: i64 = 555_200_000 + (std::process::id() as i64 % 100_000);
    let attacker: i64 = victim + 1;

    // initData proves the caller is `attacker`, but they request `victim`'s data.
    let init_data = common::make_init_data(attacker, BOT_TOKEN);
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/loyalty/{victim}"))
                .header("X-Telegram-Init-Data", init_data)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot");

    // check_owner: user.id (attacker) != expected (victim) → FORBIDDEN.
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn owner_gate_rejects_missing_init_data() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid: i64 = 555_300_000;

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/loyalty/{tid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
