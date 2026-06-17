//! Negative-path integration tests for the X-Idempotency-Key contract
//! and adjacent validators (cycle #166). Cycles #163/#164/#165 covered
//! the happy + replay paths; the failure-path contract is equally
//! load-bearing and lacked coverage.
//!
//! Marked `#[ignore]` — runs with:
//!
//! ```sh
//! DATABASE_URL=postgres://... cargo test --features backend -- --ignored
//! ```
//!
//! Each test owns its own target_tid suffix to keep independent.

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use tower::ServiceExt;

use woody_weed_bot::api::auth::generate_admin_token;
use woody_weed_bot::db::entities::loyalty_idempotency_key::{
    Column as LikCol, Entity as LikEntity,
};

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn malformed_idempotency_key_rejected_400() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let admin_token = generate_admin_token("test_password", "dummy_test_token");
    let target_tid: i64 = 999_700_000 + (rand_suffix() as i64);
    let body = json!({
        "amount": 100.0_f64,
        "tx_type": "neg_test",
    });

    // `is_valid_idempotency_key` rejects anything outside `[A-Za-z0-9_-]`.
    // NOTE: control chars (e.g. '\n') are NOT exercised here — they are invalid
    // HTTP *header values*, so `Request::builder().header(...)` rejects them
    // before the request reaches the handler (that's the http layer's job, a
    // different test boundary). These cases are all valid header values that
    // DO reach the handler and must come back 400.
    for bad_key in [
        "has space",
        "with/slash",
        "with\"quote",
        "with.dot",
        &"a".repeat(101),
    ] {
        let resp = post_add_bonus(app.clone(), target_tid, &admin_token, bad_key, &body).await;
        assert_eq!(
            resp.status,
            StatusCode::BAD_REQUEST,
            "key {:?} must be rejected, got {} body: {}",
            bad_key,
            resp.status,
            resp.body
        );
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn add_bonus_above_max_amount_rejected_400() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let admin_token = generate_admin_token("test_password", "dummy_test_token");
    let target_tid: i64 = 999_600_000 + (rand_suffix() as i64);
    let idem_key = uuid::Uuid::new_v4().to_string();
    // Cycle #151: ADD_BONUS_MAX_AMOUNT = 1_000_000.
    let body = json!({
        "amount": 1_000_000.01_f64,
        "tx_type": "neg_test",
    });
    let resp = post_add_bonus(app, target_tid, &admin_token, &idem_key, &body).await;
    assert_eq!(
        resp.status,
        StatusCode::BAD_REQUEST,
        "above-max amount must be rejected, body: {}",
        resp.body
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn missing_admin_token_rejected_unauthorized() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let target_tid: i64 = 999_500_000 + (rand_suffix() as i64);
    let idem_key = uuid::Uuid::new_v4().to_string();
    let body = json!({ "amount": 100.0_f64, "tx_type": "neg_test" });

    // Build a request WITHOUT x-admin-token / initData.
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/loyalty/{}/bonus", target_tid))
        .header("content-type", "application/json")
        .header("x-idempotency-key", idem_key)
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let response = app.oneshot(request).await.expect("router.oneshot");
    // `check_admin` returns UNAUTHORIZED when neither auth path is
    // present (cycle #142 covered the metric label).
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "missing admin token must yield 401"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn use_bonus_insufficient_funds_skips_idempotency_record() {
    // Cycle #160 contract: insufficient-funds rejection drops the
    // whole tx, including the idempotency-key INSERT. The caller's
    // retry should see the SAME 400 (not be replayed-200 from a
    // "your failure took" cache).
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let admin_token = generate_admin_token("test_password", "dummy_test_token");
    let target_tid: i64 = 999_400_000 + (rand_suffix() as i64);
    let idem_key = uuid::Uuid::new_v4().to_string();
    // No prior bonus granted → balance is 0 (or row doesn't exist).
    // `use_bonus` 50 must fail with the WHERE-guard.
    let body = json!({ "amount": 50.0_f64 });
    let resp = post_use_bonus(app, target_tid, &admin_token, &idem_key, &body).await;
    assert_eq!(
        resp.status,
        StatusCode::BAD_REQUEST,
        "use_bonus on empty balance must yield 400, body: {}",
        resp.body
    );
    // Crucially: the idempotency-key INSERT must NOT have run.
    // Otherwise the next retry with the same key (after admin grants
    // bonus) would replay the 400 forever.
    let rows = LikEntity::find()
        .filter(LikCol::Key.eq(idem_key.clone()))
        .all(&db.orm)
        .await
        .expect("loyalty_idempotency_keys query");
    assert!(
        rows.is_empty(),
        "failed use_bonus must NOT write an idempotency-key row, found: {:?}",
        rows
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn add_bonus_works_without_idempotency_header() {
    // The header is OPTIONAL — clients that don't send it should
    // still get a successful response and a fresh tx_id each call.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let admin_token = generate_admin_token("test_password", "dummy_test_token");
    let target_tid: i64 = 999_300_000 + (rand_suffix() as i64);
    let body = json!({
        "amount": 25.0_f64,
        "tx_type": "no_header_test",
    });
    // Build request WITHOUT x-idempotency-key.
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/loyalty/{}/bonus", target_tid))
        .header("content-type", "application/json")
        .header("x-admin-token", &admin_token)
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let response = app.oneshot(request).await.expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    assert_eq!(
        status,
        StatusCode::OK,
        "missing header should not block, body: {}",
        body_json
    );
    assert_eq!(body_json["success"], true);
    assert!(body_json["tx_id"].is_string(), "must still return a tx_id");
}

// ── helpers ─────────────────────────────────────────────────────────

struct Resp {
    status: StatusCode,
    body: serde_json::Value,
}

async fn post_add_bonus(
    app: axum::Router,
    target_tid: i64,
    admin_token: &str,
    idem_key: &str,
    body: &serde_json::Value,
) -> Resp {
    post_with_idem(
        app,
        &format!("/api/loyalty/{}/bonus", target_tid),
        admin_token,
        idem_key,
        body,
    )
    .await
}

async fn post_use_bonus(
    app: axum::Router,
    target_tid: i64,
    admin_token: &str,
    idem_key: &str,
    body: &serde_json::Value,
) -> Resp {
    post_with_idem(
        app,
        &format!("/api/loyalty/{}/use-bonus", target_tid),
        admin_token,
        idem_key,
        body,
    )
    .await
}

async fn post_with_idem(
    app: axum::Router,
    path: &str,
    admin_token: &str,
    idem_key: &str,
    body: &serde_json::Value,
) -> Resp {
    let request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .header("x-admin-token", admin_token)
        .header("x-idempotency-key", idem_key)
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap();
    let response = app.oneshot(request).await.expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    Resp { status, body }
}

fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}
