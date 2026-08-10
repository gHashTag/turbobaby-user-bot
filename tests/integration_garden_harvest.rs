//! Integration tests for the garden harvest → reward path.
//!
//! `src/api/garden.rs` sat at 28.3%. Watering already had tests; harvesting
//! did not, and it is the step that mints a discount reward — a plant that
//! could be harvested twice would mint two.
//!
//! Note on the contract: `harvest_plant` answers **200 with
//! `success: false`** for refusals (not ready, already harvested, unknown
//! plant) rather than a 4xx. That is unusual enough that a caller checking
//! only the status code would treat every refusal as a success, so the tests
//! assert on the body, and pin the shape so it cannot change silently.
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
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::Value;
use tower::ServiceExt;

const BOT_TOKEN: &str = "dummy_test_token";

struct Resp {
    status: StatusCode,
    body: Value,
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

fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}

/// Seed a plant owned by a fresh user. `completed` decides whether it is
/// ready to harvest.
async fn seed_plant(db: &woody_weed_bot::db::Database, completed: bool) -> (String, i64) {
    let suffix = rand_suffix();
    let telegram_id: i64 = 999_400_000 + (suffix as i64 % 500_000);
    let strain_id = uuid::Uuid::new_v4().to_string();
    let name = format!("harvest-test-{suffix}");

    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO strains (id, name, price_per_gram, is_available) \
             VALUES ($1, $2, 100, TRUE)",
            [strain_id.clone().into(), name.clone().into()],
        ))
        .await
        .expect("seed strain");

    let plant_id = uuid::Uuid::new_v4().to_string();
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO garden_plants \
               (id, user_id, strain_id, strain_name, current_stage, planted_at, \
                is_completed, water_count, last_watered_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, 5, NULL)",
            [
                plant_id.clone().into(),
                telegram_id.to_string().into(),
                strain_id.into(),
                name.into(),
                if completed { "harvest" } else { "seed" }.into(),
                chrono::Utc::now().timestamp_millis().into(),
                completed.into(),
            ],
        ))
        .await
        .expect("seed plant");

    (plant_id, telegram_id)
}

async fn harvest(app: axum::Router, plant_id: &str, tid: i64) -> Resp {
    send(
        app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/garden/plants/{plant_id}/harvest"))
            .header(
                "X-Telegram-Init-Data",
                common::make_init_data(tid, BOT_TOKEN),
            )
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

async fn reward_count(db: &woody_weed_bot::db::Database, tid: i64) -> i64 {
    db.orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*) AS n FROM garden_rewards WHERE user_id = $1",
            [tid.to_string().into()],
        ))
        .await
        .expect("reward count query")
        .and_then(|r| r.try_get::<i64>("", "n").ok())
        .unwrap_or(0)
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn harvesting_a_ready_plant_mints_exactly_one_reward() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let (plant_id, tid) = seed_plant(&db, true).await;
    // Delta, not an absolute: the suite shares one database across runs, so a
    // count of zero depends on what earlier runs happened to leave behind.
    let before = reward_count(&db, tid).await;

    let resp = harvest(app, &plant_id, tid).await;
    assert_eq!(resp.status, StatusCode::OK, "body: {}", resp.body);
    assert_eq!(
        resp.body["success"], true,
        "a ready plant must harvest: {}",
        resp.body
    );
    assert_eq!(
        reward_count(&db, tid).await - before,
        1,
        "harvesting must mint exactly one reward"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn a_plant_cannot_be_harvested_twice() {
    // The reward is a discount. A second harvest would mint a second one from
    // the same plant — free money.
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let (plant_id, tid) = seed_plant(&db, true).await;

    assert_eq!(
        harvest(app.clone(), &plant_id, tid).await.body["success"],
        true
    );
    // Delta, not an absolute: the suite shares one database across runs.
    let after_first = reward_count(&db, tid).await;

    let second = harvest(app, &plant_id, tid).await;
    assert_eq!(second.status, StatusCode::OK);
    assert_eq!(
        second.body["success"], false,
        "the second harvest must be refused: {}",
        second.body
    );
    assert_eq!(second.body["error"], "Already harvested");
    assert_eq!(
        reward_count(&db, tid).await,
        after_first,
        "a refused second harvest must not mint another reward"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn an_unripe_plant_cannot_be_harvested() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let (plant_id, tid) = seed_plant(&db, false).await;
    let before = reward_count(&db, tid).await;

    let resp = harvest(app, &plant_id, tid).await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.body["success"], false);
    assert_eq!(resp.body["error"], "Plant not ready for harvest");
    assert_eq!(
        reward_count(&db, tid).await,
        before,
        "an unripe plant must mint nothing"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn harvesting_an_unknown_plant_reports_not_found_without_failing() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let resp = harvest(app, &uuid::Uuid::new_v4().to_string(), 999_499_999).await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.body["success"], false);
    assert_eq!(resp.body["error"], "Plant not found");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn an_over_long_plant_id_is_refused_before_the_database() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let resp = harvest(app, &"x".repeat(201), 999_499_998).await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_reward_reaches_the_owner_rewards_list() {
    // A minted reward the user cannot see is the same as no reward.
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let (plant_id, tid) = seed_plant(&db, true).await;
    let before = reward_count(&db, tid).await;
    harvest(app.clone(), &plant_id, tid).await;

    let list = send(
        app,
        Request::builder()
            .uri(format!("/api/garden/rewards?telegram_id={tid}"))
            .header(
                "X-Telegram-Init-Data",
                common::make_init_data(tid, BOT_TOKEN),
            )
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(list.status, StatusCode::OK, "body: {}", list.body);
    let rewards = list.body["rewards"].as_array().expect("rewards array");
    // Delta plus a property, not an absolute count: the suite shares one
    // database across runs, so "exactly one reward" would depend on what an
    // earlier run left for this id.
    assert_eq!(
        reward_count(&db, tid).await - before,
        1,
        "harvesting must add exactly one reward"
    );
    assert!(
        rewards.iter().any(|r| r["is_active"] == true),
        "the harvested reward must appear in the owner's list and be usable: {}",
        list.body
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn both_leaderboard_kinds_come_back_ordered() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = 999_490_000_i64;

    for kind in ["harvest", "streak"] {
        let resp = send(
            app.clone(),
            Request::builder()
                .uri(format!(
                    "/api/garden/leaderboard?telegram_id={tid}&kind={kind}&limit=50"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(resp.status, StatusCode::OK, "{kind}: {}", resp.body);
        let entries = resp.body["entries"].as_array().expect("entries array");
        let scores: Vec<i64> = entries.iter().filter_map(|e| e["score"].as_i64()).collect();
        assert!(
            scores.windows(2).all(|w| w[0] >= w[1]),
            "{kind} leaderboard must be ordered high to low, got {scores:?}"
        );
        let ranks: Vec<i64> = entries.iter().filter_map(|e| e["rank"].as_i64()).collect();
        assert_eq!(
            ranks,
            (1..=ranks.len() as i64).collect::<Vec<_>>(),
            "{kind} ranks must run 1..n with no gaps"
        );
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn an_unknown_leaderboard_kind_is_refused() {
    // `kind` is interpolated into one of two hand-written queries; anything
    // outside the allow-list must be rejected rather than defaulted.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    for kind in ["", "harvests", "'; DROP TABLE garden_plants; --"] {
        let resp = send(
            app.clone(),
            Request::builder()
                .uri(format!(
                    "/api/garden/leaderboard?telegram_id=999490001&kind={}",
                    urlencoding::encode(kind)
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(
            resp.status,
            StatusCode::BAD_REQUEST,
            "kind {kind:?} must be refused"
        );
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_harvested_plant_reaches_the_harvest_leaderboard() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let (plant_id, tid) = seed_plant(&db, true).await;
    harvest(app.clone(), &plant_id, tid).await;

    let resp = send(
        app,
        Request::builder()
            .uri(format!(
                "/api/garden/leaderboard?telegram_id={tid}&kind=harvest&limit=100"
            ))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK, "body: {}", resp.body);
    let me = resp.body["user"]["score"].as_i64();
    // At least one, not exactly one: the suite shares a database across runs,
    // so a reused telegram_id may already carry earlier harvests. What matters
    // is that this harvest is counted.
    assert!(
        me.unwrap_or(0) >= 1,
        "the harvester's own score must count their harvest: {}",
        resp.body
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn garden_products_are_public() {
    // The picker that starts a plant must render for a logged-out first-time
    // visitor, otherwise the whole feature is unreachable.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let resp = send(
        app,
        Request::builder()
            .uri("/api/garden/products")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert!(
        resp.body.is_object(),
        "expected a JSON object, got {}",
        resp.body
    );
}
