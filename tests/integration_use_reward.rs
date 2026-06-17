//! Integration tests for `POST /api/garden/rewards/:id/use` — the garden
//! reward redemption (a financial mutation: credits bonus points).
//!
//! End-to-end coverage of the happy path + idempotency for the Wave #39 work
//! (W-72.1 fail-loud reward reads + unified `reward_is_active` boundary):
//!   - redeeming a valid reward returns its discount/bonus and credits the
//!     bonus to the user's loyalty balance;
//!   - the reward is then marked used — a second redemption is rejected
//!     (atomic `WHERE is_used = false` guard, no double credit).
//! Owner-gated (`check_owner`) → uses a real signed initData via
//! `common::make_init_data`. The reward row is seeded directly (no harvest
//! flow needed); everything else is exercised through the live Router.
//!
//! `#[ignore]`. Run against a THROWAWAY local DB:
//!
//! ```sh
//! DATABASE_URL=postgres://postgres@localhost/woody_test \
//!   cargo test --features backend --test integration_use_reward -- --ignored --test-threads=1
//! ```

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

const BOT_TOKEN: &str = "dummy_test_token";

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn use_reward_credits_bonus_then_rejects_second_use() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let tid: i64 = 556_100_000 + (std::process::id() as i64 % 100_000);
    let reward_id = format!("e2e-reward-{tid}");
    let now = now_millis();
    let expires_at = now + 86_400_000; // +1 day, comfortably in the future
    let discount = 15i32;
    let bonus = 250i32;

    // Seed a valid, unused reward owned by `tid`.
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO garden_rewards (id, plant_id, user_id, strain_id, strain_name, \
             discount_percent, bonus_points, expires_at, is_used, created_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,false,$9)",
            [
                reward_id.clone().into(),
                "e2e-plant".into(),
                tid.to_string().into(),
                "e2e-strain".into(),
                "E2E Strain".into(),
                discount.into(),
                bonus.into(),
                expires_at.into(),
                now.into(),
            ],
        ))
        .await
        .expect("seed garden_rewards");

    let init_data = common::make_init_data(tid, BOT_TOKEN);

    // 1. Redeem → 200, returns the reward's discount/bonus.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/garden/rewards/{reward_id}/use"))
                .header("X-Telegram-Init-Data", init_data.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot use #1");
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes())
            .expect("json");
    assert_eq!(body["success"], true, "first redemption must succeed");
    assert_eq!(body["discount_percent"].as_i64(), Some(discount as i64));
    assert_eq!(body["bonus_points"].as_i64(), Some(bonus as i64));

    // 2. Bonus credited to the loyalty balance (owner GET).
    let prof = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/loyalty/{tid}"))
                .header("X-Telegram-Init-Data", init_data.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot profile");
    assert_eq!(prof.status(), StatusCode::OK);
    let prof_body: serde_json::Value =
        serde_json::from_slice(&prof.into_body().collect().await.unwrap().to_bytes())
            .expect("json");
    assert_eq!(
        prof_body["profile"]["bonus_balance"].as_f64(),
        Some(bonus as f64),
        "bonus must be credited exactly once"
    );

    // 3. Second redemption → rejected (reward already used; no double credit).
    let resp2 = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/garden/rewards/{reward_id}/use"))
                .header("X-Telegram-Init-Data", init_data)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot use #2");
    assert_eq!(resp2.status(), StatusCode::OK);
    let body2: serde_json::Value =
        serde_json::from_slice(&resp2.into_body().collect().await.unwrap().to_bytes())
            .expect("json");
    assert_eq!(
        body2["success"], false,
        "second redemption of the same reward must be rejected"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn use_reward_rejects_non_owner() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let owner: i64 = 556_200_000 + (std::process::id() as i64 % 100_000);
    let attacker: i64 = owner + 1;
    let reward_id = format!("e2e-reward-owner-{owner}");
    let now = now_millis();

    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO garden_rewards (id, plant_id, user_id, strain_id, strain_name, \
             discount_percent, bonus_points, expires_at, is_used, created_at) \
             VALUES ($1,'p',$2,'s','S',10,100,$3,false,$4)",
            [
                reward_id.clone().into(),
                owner.to_string().into(),
                (now + 86_400_000).into(),
                now.into(),
            ],
        ))
        .await
        .expect("seed garden_rewards");

    // Attacker (valid initData for their own id) tries to redeem owner's reward.
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/garden/rewards/{reward_id}/use"))
                .header(
                    "X-Telegram-Init-Data",
                    common::make_init_data(attacker, BOT_TOKEN),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot");

    // check_owner: initData user (attacker) != reward.user_id (owner) → 403.
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
