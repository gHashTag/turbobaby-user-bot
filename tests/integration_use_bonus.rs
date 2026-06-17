//! Integration test for `use_bonus` (cycle #164):
//!   1. X-Idempotency-Key replay returns the same `tx_id` and
//!      `idempotent_replay: true` (cycle #160 phase 2b contract).
//!   2. A successful deduction writes a `bonus_transactions` ledger row
//!      with `amount = -X, tx_type = "admin_deduction", id = <tx_id>`
//!      (cycle #161 — restores the
//!      `SUM(amount) == bonus_balance` bookkeeping equation).
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
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::json;
use tower::ServiceExt;

use woody_weed_bot::api::auth::generate_admin_token;
use woody_weed_bot::db::entities::bonus_transaction::{Column as BtCol, Entity as BtEntity};

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn use_bonus_replay_and_deduction_ledger() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    let admin_token = generate_admin_token("test_password", "dummy_test_token");
    let target_tid: i64 = 999_800_000 + (rand_suffix() as i64);

    // Seed: grant 200 bonus to the target user. Idempotency key
    // separate from the use_bonus one so they don't collide.
    let seed_key = uuid::Uuid::new_v4().to_string();
    let seed_body = json!({
        "amount": 200.0_f64,
        "tx_type": "integration_test_seed",
        "description": "cycle #164 seed",
        "related_order_id": null,
    });
    let seed_resp = post(
        app.clone(),
        "POST",
        &format!("/api/loyalty/{}/bonus", target_tid),
        &admin_token,
        &seed_key,
        Some(&seed_body),
    )
    .await;
    assert_eq!(seed_resp.status, StatusCode::OK, "seed grant must succeed");

    // First use_bonus — deduct 50, expect 200 + tx_id.
    let use_key = uuid::Uuid::new_v4().to_string();
    let use_body = json!({ "amount": 50.0_f64 });
    let first = post(
        app.clone(),
        "POST",
        &format!("/api/loyalty/{}/use-bonus", target_tid),
        &admin_token,
        &use_key,
        Some(&use_body),
    )
    .await;
    assert_eq!(first.status, StatusCode::OK, "first use_bonus must succeed");
    let first_tx_id = first.body["tx_id"]
        .as_str()
        .expect("first response must carry tx_id")
        .to_string();
    assert!(
        first.body["idempotent_replay"].is_null() || first.body["idempotent_replay"] == false,
        "first call must NOT be marked as a replay"
    );

    // Second use_bonus — same key + body, expect replay.
    let second = post(
        app,
        "POST",
        &format!("/api/loyalty/{}/use-bonus", target_tid),
        &admin_token,
        &use_key,
        Some(&use_body),
    )
    .await;
    assert_eq!(second.status, StatusCode::OK, "replay must succeed");
    assert_eq!(
        second.body["tx_id"].as_str(),
        Some(first_tx_id.as_str()),
        "replay must return the original tx_id"
    );
    assert_eq!(
        second.body["idempotent_replay"], true,
        "replay must carry idempotent_replay: true"
    );

    // Cycle #161 ledger assertion. `bonus_transactions` should have:
    //   * 1 grant row (200, seed)
    //   * 1 deduction row (-50, tx_id matches first_tx_id)
    //   * NO duplicate deduction (the replay used the cached tx_id
    //     instead of inserting a second row)
    let rows = BtEntity::find()
        .filter(BtCol::TelegramId.eq(target_tid))
        .order_by_asc(BtCol::CreatedAt)
        .all(&db.orm)
        .await
        .expect("bonus_transactions query");
    assert_eq!(
        rows.len(),
        2,
        "expected exactly 2 ledger rows (1 grant + 1 deduction), got: {:#?}",
        rows
    );

    let deduction = rows
        .iter()
        .find(|r| r.tx_type == "admin_deduction")
        .expect("a deduction row must exist");
    assert_eq!(
        deduction.id, first_tx_id,
        "deduction id must match the cycle-#160 synthetic tx_id"
    );
    assert!(
        (deduction.amount - (-50.0)).abs() < 1e-9,
        "deduction amount must be -50.0, got {}",
        deduction.amount
    );
}

/// Overdraw guard on a FUNDED account: with balance 50, deducting 100 must be
/// rejected (400) and leave the balance EXACTLY 50 — no partial deduction, no
/// `GREATEST(0, …)` clamp to 0, no debit ledger row. (The existing negative
/// test covers the empty-balance case; this covers a non-zero insufficient
/// balance, where a partial-deduction bug would actually move money.)
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn use_bonus_overdraw_rejected_balance_unchanged() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let admin_token = generate_admin_token("test_password", "dummy_test_token");
    let tid: i64 = 999_700_000 + (rand_suffix() as i64);

    // Seed 50 bonus.
    let seed = post(
        app.clone(),
        "POST",
        &format!("/api/loyalty/{tid}/bonus"),
        &admin_token,
        &uuid::Uuid::new_v4().to_string(),
        Some(&json!({ "amount": 50.0_f64, "tx_type": "overdraw_seed" })),
    )
    .await;
    assert_eq!(seed.status, StatusCode::OK);

    // Attempt to deduct 100 (> balance) → 400.
    let over = post(
        app.clone(),
        "POST",
        &format!("/api/loyalty/{tid}/use-bonus"),
        &admin_token,
        &uuid::Uuid::new_v4().to_string(),
        Some(&json!({ "amount": 100.0_f64 })),
    )
    .await;
    assert_eq!(
        over.status,
        StatusCode::BAD_REQUEST,
        "overdraw must be rejected, body: {}",
        over.body
    );

    // Balance unchanged (still exactly 50) — via owner GET.
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
        Some(50.0),
        "rejected overdraw must leave the balance untouched"
    );

    // No debit ledger row — only the seed grant exists.
    let deductions = BtEntity::find()
        .filter(BtCol::TelegramId.eq(tid))
        .filter(BtCol::TxType.eq("admin_deduction"))
        .all(&db.orm)
        .await
        .expect("ledger query");
    assert!(
        deductions.is_empty(),
        "a rejected overdraw must NOT write a deduction ledger row, got: {deductions:#?}"
    );
}

struct Resp {
    status: StatusCode,
    body: serde_json::Value,
}

async fn post(
    app: axum::Router,
    method: &'static str,
    path: &str,
    admin_token: &str,
    idem_key: &str,
    body: Option<&serde_json::Value>,
) -> Resp {
    let body_bytes = body
        .map(|b| serde_json::to_vec(b).unwrap())
        .unwrap_or_default();
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .header("x-admin-token", admin_token)
        .header("x-idempotency-key", idem_key)
        .body(Body::from(body_bytes))
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
