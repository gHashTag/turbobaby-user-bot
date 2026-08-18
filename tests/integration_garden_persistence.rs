//! Does the garden survive a reload?
//!
//! Reported: watering advances the plant on screen — Семечка 0/14 becomes
//! Росток 1/14 — and reopening the app shows it back at the start.
//!
//! A reload is not a special code path. It is the same two requests the app
//! makes on boot: `GET /api/garden/plants` and `GET /api/garden/streak`. So the
//! test is the cycle, driven through the real router: choose a plant, read it,
//! water it, read it again the way a fresh boot would, and require the second
//! read to carry what the first write said.
//!
//! Nothing here re-implements the rules. `next_water_step` decides the stage
//! and `update_streak_on_water` decides the streak; this only asserts that what
//! the API reported after the write is what the API reports after the read.
//!
//! Run with:
//! ```sh
//! DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/woody_test \
//!   cargo test --features backend --test integration_garden_persistence -- --ignored --test-threads=1
//! ```

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::{make_app_with_db, make_init_data};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use tower::ServiceExt;

const PLAYER: i64 = 960_101;

async fn call(
    app: &axum::Router,
    method: Method,
    uri: &str,
    body: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(
            "X-Telegram-Init-Data",
            make_init_data(PLAYER, "dummy_test_token"),
        )
        .header("content-type", "application/json");
    let req = match body {
        Some(b) => builder.body(Body::from(b.to_string())),
        None => builder.body(Body::empty()),
    }
    .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .expect("body");
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::String(
        String::from_utf8_lossy(&bytes).to_string(),
    ));
    (status, json)
}

/// What a fresh boot sees: the plant list for this player.
async fn plants_on_boot(app: &axum::Router) -> Vec<serde_json::Value> {
    let (status, body) = call(
        app,
        Method::GET,
        &format!("/api/garden/plants?telegram_id={PLAYER}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "GET /api/garden/plants: {body}");
    body["plants"].as_array().cloned().unwrap_or_default()
}

async fn a_garden_eligible_strain(db: &woody_weed_bot::db::Database) -> String {
    db.orm
        .execute(Statement::from_string(
            DbBackend::Postgres,
            "INSERT INTO strains (id, name, price_per_gram, is_available, garden_eligible) \
             VALUES ('persist-test-strain', 'Persist Test Strain', 300, true, true) \
             ON CONFLICT (id) DO UPDATE SET is_available = true, garden_eligible = true"
                .to_string(),
        ))
        .await
        .expect("seed a strain to plant");
    "persist-test-strain".to_string()
}

#[tokio::test]
#[ignore]
async fn watering_survives_a_reload() {
    let Some((app, db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };
    let strain = a_garden_eligible_strain(&db).await;
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM garden_plants WHERE user_id = $1",
            [PLAYER.to_string().into()],
        ))
        .await
        .expect("clean slate");

    // Plant something, the way the picker does.
    let (status, body) = call(
        &app,
        Method::POST,
        "/api/garden/plants/choose",
        Some(&format!(
            r#"{{"telegram_id":{PLAYER},"catalog":"strain","product_id":"{strain}"}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "choose: {body}");
    assert_eq!(
        body["success"].as_bool(),
        Some(true),
        "the plant was not created: {body}"
    );

    // A reload right after planting must show the plant.
    let after_planting = plants_on_boot(&app).await;
    assert_eq!(
        after_planting.len(),
        1,
        "a reload straight after planting shows {} plants: {after_planting:?}",
        after_planting.len()
    );
    let plant_id = after_planting[0]["id"].as_str().expect("id").to_string();
    let before = after_planting[0]["water_count"].as_i64().unwrap_or(-1);
    assert_eq!(before, 0, "a fresh plant starts unwatered");

    // Water it once.
    let (status, watered) = call(
        &app,
        Method::POST,
        &format!("/api/garden/plants/{plant_id}/water"),
        Some(&format!(r#"{{"telegram_id":{PLAYER}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "water: {watered}");
    assert_eq!(
        watered["success"].as_bool(),
        Some(true),
        "watering was refused: {watered}"
    );

    // THE RELOAD. Same request the app makes on boot, nothing cached.
    let after_reload = plants_on_boot(&app).await;
    assert_eq!(after_reload.len(), 1, "the plant vanished on reload");
    let reloaded = &after_reload[0];

    assert_eq!(
        reloaded["water_count"].as_i64(),
        Some(1),
        "the watering did not survive the reload: the plant came back at {:?}",
        reloaded["water_count"]
    );
    assert_eq!(
        reloaded["id"].as_str(),
        Some(plant_id.as_str()),
        "a different plant came back"
    );
    assert_ne!(
        reloaded["current_stage"].as_str(),
        Some("seed"),
        "the stage went back to a seed after one watering: {reloaded:?}"
    );
    assert!(
        reloaded["last_watered_at"].as_i64().unwrap_or(0) > 0,
        "nothing recorded when it was watered, so the cooldown restarts on every \
         reload: {reloaded:?}"
    );
}

/// The streak is a second piece of state the screen prints, read from its own
/// endpoint. It has to survive the same reload.
#[tokio::test]
#[ignore]
async fn the_streak_survives_a_reload() {
    let Some((app, db)) = make_app_with_db().await else {
        return;
    };
    let strain = a_garden_eligible_strain(&db).await;
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM garden_plants WHERE user_id = $1",
            [PLAYER.to_string().into()],
        ))
        .await
        .expect("clean slate");

    let _ = call(
        &app,
        Method::POST,
        "/api/garden/plants/choose",
        Some(&format!(
            r#"{{"telegram_id":{PLAYER},"catalog":"strain","product_id":"{strain}"}}"#
        )),
    )
    .await;
    let id = plants_on_boot(&app).await[0]["id"]
        .as_str()
        .expect("id")
        .to_string();
    let _ = call(
        &app,
        Method::POST,
        &format!("/api/garden/plants/{id}/water"),
        Some(&format!(r#"{{"telegram_id":{PLAYER}}}"#)),
    )
    .await;

    let (status, streak) = call(
        &app,
        Method::GET,
        &format!("/api/garden/streak?telegram_id={PLAYER}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "GET /api/garden/streak: {streak}");
    assert_eq!(
        streak["has_plant"].as_bool(),
        Some(true),
        "the streak endpoint says there is no plant after one was planted and \
         watered: {streak}"
    );
    assert!(
        streak["streak"].as_i64().unwrap_or(0) >= 1,
        "one watering left a streak of {:?}",
        streak["streak"]
    );
}
