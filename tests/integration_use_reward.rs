//! The garden reward redemption is retired, and this file holds the
//! retirement.
//!
//! Until 2026-09-26 this file tested `POST /api/garden/rewards/:id/use` -- a
//! financial mutation that credited bonus points -- end to end: a valid reward
//! credited its bonus once and was refused the second time, and a non-owner
//! was refused with 403. Both tests seeded `garden_rewards`, which migration
//! 083 dropped with the garden, so both failed on every database migrated to
//! today's schema. The garden was the previous shop's mechanic (DECISIONS.md
//! D5); no router merges `/api/garden/*` (`src/api/mod.rs`), and the owner's
//! rulings of 2026-09-24 (rental only) and 2026-09-25 retired what was left.
//!
//! So the two tests now hold the retirement, keeping what each was for. The
//! redemption credits nothing: its route answers the API's JSON not-found, no
//! bonus row is written and the balance does not move, and the table is gone.
//! A non-owner is answered exactly as the owner is, so the missing route says
//! nothing about whose reward an id was.
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
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use tower::ServiceExt;

const BOT_TOKEN: &str = "dummy_test_token";

async fn redeem(
    app: axum::Router,
    reward_id: &str,
    caller: i64,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/garden/rewards/{reward_id}/use"))
                .header(
                    "X-Telegram-Init-Data",
                    common::make_init_data(caller, BOT_TOKEN),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

async fn scalar_i64(db: &turbobaby_bot::db::Database, sql: &str, tid: i64) -> i64 {
    db.orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [tid.into()],
        ))
        .await
        .expect("query")
        .and_then(|r| r.try_get::<i64>("", "n").ok())
        .unwrap_or(-1)
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn use_reward_is_retired_and_credits_nothing() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };

    let tid: i64 = 556_100_000 + (std::process::id() as i64 % 100_000);
    let reward_id = format!("e2e-reward-{tid}");
    let bonus_rows = "SELECT count(*)::int8 AS n FROM bonus_transactions WHERE telegram_id = $1";
    let rows_before = scalar_i64(&db, bonus_rows, tid).await;

    // Twice, as the redemption test used to redeem: neither is served.
    for attempt in ["first", "second"] {
        let (status, body) = redeem(app.clone(), &reward_id, tid).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{attempt}: {body}");
        assert_eq!(
            body["error"], "not_found",
            "{attempt}: the API's own miss: {body}"
        );
    }

    // Nothing was credited.
    assert_eq!(
        scalar_i64(&db, bonus_rows, tid).await,
        rows_before,
        "a bonus row was written"
    );
    let gone = db
        .orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT to_regclass('public.garden_rewards') IS NULL AS gone".to_string(),
        ))
        .await
        .expect("query")
        .expect("one row")
        .try_get::<bool>("", "gone")
        .expect("a flag");
    assert!(gone, "migration 083 dropped the table");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn use_reward_answers_a_non_owner_exactly_as_the_owner() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };

    let owner: i64 = 556_200_000 + (std::process::id() as i64 % 100_000);
    let attacker: i64 = owner + 1;
    let reward_id = format!("e2e-reward-owner-{owner}");

    let (as_owner, _) = redeem(app.clone(), &reward_id, owner).await;
    let (as_attacker, body) = redeem(app, &reward_id, attacker).await;
    assert_eq!(as_attacker, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(as_attacker, as_owner, "the answer depends on who asks");
}
