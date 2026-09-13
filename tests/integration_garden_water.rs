//! Reproduction test for the user report "растения не поливаются".
//!
//! Exercises `POST /api/garden/plants/:id/water` end-to-end through the
//! real Axum router against a live Postgres:
//!   1. a fresh plant (`last_watered_at IS NULL`) must water successfully;
//!   2. an immediate second water must be rejected with "Cooldown active";
//!   3. a plant last watered 25h ago must water successfully.
//!
//! Run with:
//! ```sh
//! DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/woody_test \
//!   cargo test --features backend --no-default-features \
//!   --test integration_garden_water -- --ignored --test-threads=1
//! ```

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::{make_app_with_db, make_init_data};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use tower::ServiceExt;

fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos())
        % 900_000
}

async fn water(
    app: &axum::Router,
    plant_id: &str,
    telegram_id: i64,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/garden/plants/{plant_id}/water"))
        .header(
            "X-Telegram-Init-Data",
            make_init_data(telegram_id, "dummy_test_token"),
        )
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let json = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap_or(
        serde_json::Value::String(String::from_utf8_lossy(&bytes).to_string()),
    );
    (status, json)
}

/// Seed a strain + plant, return (plant_id, telegram_id).
async fn seed_plant(
    db: &turbobaby_bot::db::Database,
    last_watered_at: Option<i64>,
) -> (String, i64) {
    let suffix = rand_suffix();
    let telegram_id: i64 = 999_300_000 + (suffix as i64);
    let strain_id = uuid::Uuid::new_v4().to_string();
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO strains (id, name, price_per_gram, is_available) \
             VALUES ($1, $2, 100, TRUE)",
            [
                strain_id.clone().into(),
                format!("water-test-{suffix}").into(),
            ],
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
             VALUES ($1, $2, $3, $4, 'seed', $5, false, 0, $6)",
            [
                plant_id.clone().into(),
                telegram_id.to_string().into(),
                strain_id.into(),
                format!("water-test-{suffix}").into(),
                chrono::Utc::now().timestamp_millis().into(),
                last_watered_at.into(),
            ],
        ))
        .await
        .expect("seed plant");
    (plant_id, telegram_id)
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn fresh_plant_can_be_watered() {
    let Some((app, db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let (plant_id, tid) = seed_plant(&db, None).await;

    let (status, body) = water(&app, &plant_id, tid).await;
    assert_eq!(status, StatusCode::OK, "unexpected status, body={body:?}");
    assert_eq!(
        body.get("success").and_then(|v| v.as_bool()),
        Some(true),
        "fresh plant must water, body={body:?}"
    );
    assert_eq!(body.get("water_count").and_then(|v| v.as_i64()), Some(1));
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn second_water_is_on_cooldown() {
    let Some((app, db)) = make_app_with_db().await else {
        return;
    };
    let (plant_id, tid) = seed_plant(&db, None).await;

    let (_s1, b1) = water(&app, &plant_id, tid).await;
    assert_eq!(b1.get("success").and_then(|v| v.as_bool()), Some(true));

    let (status, body) = water(&app, &plant_id, tid).await;
    assert_eq!(status, StatusCode::OK, "body={body:?}");
    assert_eq!(
        body.get("success").and_then(|v| v.as_bool()),
        Some(false),
        "second water must be rejected, body={body:?}"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn plant_watered_25h_ago_can_be_watered_again() {
    let Some((app, db)) = make_app_with_db().await else {
        return;
    };
    let long_ago = chrono::Utc::now().timestamp_millis() - 25 * 60 * 60 * 1000;
    let (plant_id, tid) = seed_plant(&db, Some(long_ago)).await;

    let (status, body) = water(&app, &plant_id, tid).await;
    assert_eq!(status, StatusCode::OK, "body={body:?}");
    assert_eq!(
        body.get("success").and_then(|v| v.as_bool()),
        Some(true),
        "cooldown expired → must water, body={body:?}"
    );
}
