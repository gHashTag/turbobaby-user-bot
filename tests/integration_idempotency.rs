//! Integration test for X-Idempotency-Key on `POST /api/loyalty/:tid/bonus`
//! (cycle #163). First real DB-backed test against the harness that
//! cycle #162 unblocked.
//!
//! Marked `#[ignore]` — runs with:
//!
//! ```sh
//! DATABASE_URL=postgres://... cargo test --features backend -- --ignored
//! ```
//!
//! Asserts the cycle-#159 retry-replay contract: two POSTs with the
//! same `X-Idempotency-Key` produce one `bonus_transactions` row, and
//! the second response carries the same `tx_id` plus the
//! `idempotent_replay: true` flag.

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

use woody_weed_bot::api::auth::generate_admin_token;

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn add_bonus_idempotent_replay_returns_same_tx_id() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    // Match `tests/common/mod.rs::test_config` exactly.
    let admin_token = generate_admin_token("test_password", "dummy_test_token");
    let idem_key = uuid::Uuid::new_v4().to_string();
    // Pick a target tid far above real user ids to avoid colliding with
    // any production-shaped fixtures somebody may have left in the test
    // DB. Different idem_keys per run mean repeated test runs don't
    // collide with each other.
    let target_tid: i64 = 999_900_000 + (rand_suffix() as i64);

    let body = json!({
        "amount": 100.0_f64,
        "tx_type": "integration_test_grant",
        "description": "cycle #163 integration",
        "related_order_id": null,
    });

    // First call — fresh idempotency key.
    let first = post_add_bonus(app.clone(), target_tid, &admin_token, &idem_key, &body).await;
    assert_eq!(first.status, StatusCode::OK, "first call must succeed");
    assert_eq!(first.body["success"], true);
    let first_tx_id = first.body["tx_id"]
        .as_str()
        .expect("first response must carry tx_id")
        .to_string();
    assert!(
        first.body["idempotent_replay"].is_null() || first.body["idempotent_replay"] == false,
        "first call must NOT be marked as a replay"
    );

    // Second call — same key + body. Must replay the cached tx_id.
    let second = post_add_bonus(app, target_tid, &admin_token, &idem_key, &body).await;
    assert_eq!(second.status, StatusCode::OK, "replay must succeed");
    assert_eq!(second.body["success"], true);
    assert_eq!(
        second.body["tx_id"].as_str(),
        Some(first_tx_id.as_str()),
        "replay must return the original tx_id"
    );
    assert_eq!(
        second.body["idempotent_replay"], true,
        "replay must carry idempotent_replay: true"
    );
}

struct AddBonusResponse {
    status: StatusCode,
    body: serde_json::Value,
}

async fn post_add_bonus(
    app: axum::Router,
    target_tid: i64,
    admin_token: &str,
    idem_key: &str,
    body: &serde_json::Value,
) -> AddBonusResponse {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/loyalty/{}/bonus", target_tid))
                .header("content-type", "application/json")
                .header("x-admin-token", admin_token)
                .header("x-idempotency-key", idem_key)
                .body(Body::from(serde_json::to_vec(body).unwrap()))
                .unwrap(),
        )
        .await
        .expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    AddBonusResponse { status, body }
}

/// Cheap random suffix so two `cargo test --ignored` runs on the same
/// DB don't accidentally use the same `target_tid` and step on each
/// other's loyalty_profiles rows.
fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}
