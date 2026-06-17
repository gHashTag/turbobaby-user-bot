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

/// The replay test above checks the *response* contract; this checks the
/// *effect* the doc-comment claims but never asserted: two POSTs with the same
/// idempotency key credit the balance EXACTLY ONCE and write EXACTLY ONE
/// `bonus_transactions` ledger row (verify the ledger count, not just the
/// balance — a guard that fires on balance but not the audit log would slip
/// past a balance-only check).
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn add_bonus_same_key_credits_once_and_writes_one_ledger_row() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let admin_token = generate_admin_token("test_password", "dummy_test_token");
    let idem_key = uuid::Uuid::new_v4().to_string();
    let tid: i64 = 999_800_000 + (rand_suffix() as i64);
    let body = json!({ "amount": 100.0_f64, "tx_type": "integration_ledger_test" });

    // Fire the SAME (key, body) twice.
    let r1 = post_add_bonus(app.clone(), tid, &admin_token, &idem_key, &body).await;
    assert_eq!(r1.status, StatusCode::OK);
    let r2 = post_add_bonus(app.clone(), tid, &admin_token, &idem_key, &body).await;
    assert_eq!(r2.status, StatusCode::OK);
    assert_eq!(
        r2.body["idempotent_replay"], true,
        "second call is a replay"
    );

    // Effect 1: balance credited exactly once (100, not 200) — via owner GET.
    let init = common::make_init_data(tid, "dummy_test_token");
    let prof = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/loyalty/{tid}"))
                .header("X-Telegram-Init-Data", init)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot profile");
    assert_eq!(prof.status(), StatusCode::OK);
    let pbody: serde_json::Value =
        serde_json::from_slice(&prof.into_body().collect().await.unwrap().to_bytes())
            .expect("json");
    assert_eq!(
        pbody["profile"]["bonus_balance"].as_f64(),
        Some(100.0),
        "balance must be credited exactly once"
    );

    // Effect 2: exactly one ledger row for this user.
    let row = db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS n FROM bonus_transactions WHERE telegram_id = $1",
            [tid.into()],
        ))
        .await
        .expect("count query")
        .expect("count row");
    let n: i64 = row.try_get("", "n").expect("n");
    assert_eq!(
        n, 1,
        "same idempotency key must write exactly one bonus_transactions row"
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
