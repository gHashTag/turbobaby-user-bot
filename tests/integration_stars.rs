//! Integration tests for the Stars (⭐) internal-currency API.
//!
//! Stars are money: the game credits them and the shop spends them at
//! 1 Star = 1 THB. `src/api/stars.rs` had **0% coverage** before this file,
//! which means nothing guarded the two properties that matter most — that a
//! balance cannot go negative, and that a retried credit cannot pay out twice.
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
use serde_json::{json, Value};
use tower::ServiceExt;

const BOT_TOKEN: &str = "dummy_test_token";

struct Resp {
    status: StatusCode,
    body: Value,
}

/// Every test uses its own telegram_id so runs never share a balance.
fn fresh_telegram_id() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Stay inside the range `validate_telegram_id_param` accepts.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    900_000_000 + (nanos % 90_000_000) as i64
}

async fn send(app: axum::Router, request: Request<Body>) -> Resp {
    let response = app.oneshot(request).await.expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    Resp { status, body }
}

async fn add_stars(app: axum::Router, tid: i64, amount: i64, external_tx_id: &str) -> Resp {
    let request = Request::builder()
        .method("POST")
        .uri("/api/stars/add")
        .header("content-type", "application/json")
        .header("X-Telegram-Init-Data", common::make_init_data(tid, BOT_TOKEN))
        .body(Body::from(
            serde_json::to_vec(&json!({
                "telegram_id": tid,
                "amount": amount,
                "source": "woodshop",
                "reason": "level_complete",
                "external_tx_id": external_tx_id,
            }))
            .unwrap(),
        ))
        .unwrap();
    send(app, request).await
}

async fn spend_stars(app: axum::Router, tid: i64, amount: i64) -> Resp {
    let request = Request::builder()
        .method("POST")
        .uri("/api/stars/spend")
        .header("content-type", "application/json")
        .header("X-Telegram-Init-Data", common::make_init_data(tid, BOT_TOKEN))
        .body(Body::from(
            serde_json::to_vec(&json!({
                "telegram_id": tid,
                "amount": amount,
                "reason": "purchase",
            }))
            .unwrap(),
        ))
        .unwrap();
    send(app, request).await
}

async fn balance(app: axum::Router, tid: i64) -> Resp {
    let request = Request::builder()
        .uri(format!("/api/stars/balance/{tid}"))
        .header("X-Telegram-Init-Data", common::make_init_data(tid, BOT_TOKEN))
        .body(Body::empty())
        .unwrap();
    send(app, request).await
}

// ── Credit ───────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn crediting_stars_raises_the_balance_and_writes_a_ledger_row() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();

    let resp = add_stars(
        app.clone(),
        tid,
        250,
        &uuid::Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK, "body: {}", resp.body);

    assert_eq!(balance(app.clone(), tid).await.body["balance"], 250);

    // The ledger is append-only and must show the credit with its snapshot.
    let history = send(
        app,
        Request::builder()
            .uri(format!("/api/stars/history/{tid}"))
            .header("X-Telegram-Init-Data", common::make_init_data(tid, BOT_TOKEN))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(history.status, StatusCode::OK);
    let txs = history.body["transactions"].as_array().expect("ledger array");
    assert_eq!(txs.len(), 1, "expected exactly one ledger row");
    assert_eq!(txs[0]["amount"], 250);
    assert_eq!(
        txs[0]["balance_after"], 250,
        "the ledger must snapshot the post-credit balance"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn a_replayed_credit_pays_out_only_once() {
    // The game retries on flaky mobile networks. Without the external_tx_id
    // guard every retry would mint free currency.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();
    let tx_id = uuid::Uuid::new_v4().to_string();

    assert_eq!(
        add_stars(app.clone(), tid, 100, &tx_id).await.status,
        StatusCode::OK
    );
    let replay = add_stars(app.clone(), tid, 100, &tx_id).await;
    assert_eq!(replay.status, StatusCode::OK, "a replay is not an error");
    assert_eq!(
        replay.body["idempotent_replay"], true,
        "the replay must be reported as such: {}",
        replay.body
    );

    assert_eq!(
        balance(app, tid).await.body["balance"],
        100,
        "a replayed credit must not double the balance"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn distinct_credits_accumulate() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();

    for _ in 0..3 {
        assert_eq!(
            add_stars(app.clone(), tid, 40, &uuid::Uuid::new_v4().to_string())
                .await
                .status,
            StatusCode::OK
        );
    }
    assert_eq!(balance(app, tid).await.body["balance"], 120);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn nonsensical_credit_amounts_are_refused() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();

    // Zero and negative would let a "credit" silently debit; the ceiling stops
    // a fat-fingered or hostile client minting a fortune in one call.
    for amount in [0_i64, -50, 1_000_001] {
        let resp = add_stars(app.clone(), tid, amount, &uuid::Uuid::new_v4().to_string()).await;
        assert_eq!(
            resp.status,
            StatusCode::BAD_REQUEST,
            "amount {amount} must be refused"
        );
    }
    assert_eq!(
        balance(app, tid).await.body["balance"],
        0,
        "no refused credit may touch the balance"
    );
}

// ── Debit ────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn spending_reduces_the_balance() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();

    add_stars(app.clone(), tid, 300, &uuid::Uuid::new_v4().to_string()).await;
    let resp = spend_stars(app.clone(), tid, 120).await;
    assert_eq!(resp.status, StatusCode::OK, "body: {}", resp.body);

    assert_eq!(balance(app, tid).await.body["balance"], 180);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn a_balance_can_never_go_negative() {
    // The single most costly failure this API could have: spending currency
    // the user does not own.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();

    add_stars(app.clone(), tid, 50, &uuid::Uuid::new_v4().to_string()).await;
    let resp = spend_stars(app.clone(), tid, 51).await;
    assert_eq!(
        resp.status,
        StatusCode::PAYMENT_REQUIRED,
        "overdraft must be refused, body: {}",
        resp.body
    );
    assert_eq!(
        balance(app, tid).await.body["balance"],
        50,
        "a refused debit must leave the balance untouched"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn spending_the_exact_balance_is_allowed() {
    // Boundary: `balance >= amount`, not `>`. Off-by-one here would strand
    // the last Star of every user forever.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();

    add_stars(app.clone(), tid, 75, &uuid::Uuid::new_v4().to_string()).await;
    assert_eq!(spend_stars(app.clone(), tid, 75).await.status, StatusCode::OK);
    assert_eq!(balance(app, tid).await.body["balance"], 0);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn spending_from_an_untouched_account_is_refused() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();

    assert_eq!(
        spend_stars(app.clone(), tid, 1).await.status,
        StatusCode::PAYMENT_REQUIRED,
        "a user with no Stars cannot spend one"
    );
    assert_eq!(balance(app, tid).await.body["balance"], 0);
}

// ── Authorisation ────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn reading_a_balance_requires_authentication() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();

    let resp = send(
        app,
        Request::builder()
            .uri(format!("/api/stars/balance/{tid}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn one_user_cannot_read_or_spend_another_users_stars() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let victim = fresh_telegram_id();
    let attacker = victim + 1;

    // Valid initData — for the attacker — pointed at the victim's account.
    let read = send(
        app.clone(),
        Request::builder()
            .uri(format!("/api/stars/balance/{victim}"))
            .header(
                "X-Telegram-Init-Data",
                common::make_init_data(attacker, BOT_TOKEN),
            )
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(
        read.status,
        StatusCode::FORBIDDEN,
        "initData for another user must not read this balance"
    );

    let spend = send(
        app,
        Request::builder()
            .method("POST")
            .uri("/api/stars/spend")
            .header("content-type", "application/json")
            .header(
                "X-Telegram-Init-Data",
                common::make_init_data(attacker, BOT_TOKEN),
            )
            .body(Body::from(
                serde_json::to_vec(&json!({
                    "telegram_id": victim,
                    "amount": 1,
                    "reason": "purchase",
                }))
                .unwrap(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(
        spend.status,
        StatusCode::FORBIDDEN,
        "initData for another user must not spend their Stars"
    );
}

/// CHARACTERISATION TEST — records a known security weakness, not a desired
/// property.
///
/// `check_owner` falls back to `lenient_owner_verify` (user id + auth_date
/// freshness, **no HMAC**) whenever strict validation fails. The fallback is
/// deliberate — see `src/api/auth.rs`, added "while the HMAC drift is
/// root-caused" — but its effect is that initData signed with any secret, or
/// none, authenticates as whoever it claims to be. Anyone who can reach the
/// API can therefore read and spend another user's Stars.
///
/// This test asserts today's behaviour so the gap is visible in the suite and
/// so that closing it is a deliberate, test-updating act rather than an
/// accident. Flip the assertion to `assert_ne!` the moment the fallback goes.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn forged_init_data_currently_still_authenticates() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();

    let resp = send(
        app,
        Request::builder()
            .uri(format!("/api/stars/balance/{tid}"))
            .header(
                "X-Telegram-Init-Data",
                common::make_init_data(tid, "not_the_real_bot_token"),
            )
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "expected the documented lenient fallback to accept unsigned initData; \
         if this now fails, the HMAC fallback was removed — delete this test \
         and assert rejection instead"
    );
}
