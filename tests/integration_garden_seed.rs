//! Integration test for cycle #168 garden-seed fix.
//!
//! Reported by the user: "не получается посадить семечко в игре".
//! Investigation revealed:
//!   * Garden UI says "Order a strain to get your first seed!" but
//!     `complete_order_and_update_loyalty` never inserted into
//!     `garden_plants` for orders completed after migration 026's
//!     one-shot backfill ran.
//!   * `POST /api/garden/plants` exists but no frontend or backend
//!     code calls it.
//!
//! Cycle #168 fix: insert into `garden_plants` (mirror of migration
//! 026's WHERE-NOT-EXISTS shape) at the end of
//! `complete_order_and_update_loyalty`'s transaction.
//!
//! This test exercises the fix directly — seed a strain + order via
//! SQL, call `complete_order_and_update_loyalty`, assert the plant
//! row exists.
//!
//! Marked `#[ignore]` — runs with:
//!
//! ```sh
//! DATABASE_URL=postgres://... cargo test --features backend -- --ignored
//! ```

#![cfg(feature = "backend")]

mod common;

use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::json;

use woody_weed_bot::db::orders::complete_order_and_update_loyalty;

/// Lightweight inline row shape — there's no SeaORM `garden_plant`
/// entity (the production reads use raw SQL in `src/api/garden.rs`).
/// Query via raw Statement.
#[derive(Debug)]
struct PlantRow {
    id: String,
    strain_id: String,
    strain_name: String,
    current_stage: String,
    is_completed: bool,
    water_count: i32,
}

async fn fetch_plants(db: &woody_weed_bot::db::Database, user_id: &str) -> Vec<PlantRow> {
    let rows = db
        .orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id, strain_id, strain_name, current_stage, is_completed, water_count \
             FROM garden_plants WHERE user_id = $1 ORDER BY planted_at ASC",
            [user_id.into()],
        ))
        .await
        .expect("garden_plants query");
    rows.into_iter()
        .map(|r| PlantRow {
            id: r.try_get::<String>("", "id").unwrap_or_default(),
            strain_id: r.try_get::<String>("", "strain_id").unwrap_or_default(),
            strain_name: r.try_get::<String>("", "strain_name").unwrap_or_default(),
            current_stage: r.try_get::<String>("", "current_stage").unwrap_or_default(),
            is_completed: r.try_get::<bool>("", "is_completed").unwrap_or(false),
            water_count: r.try_get::<i32>("", "water_count").unwrap_or(0),
        })
        .collect()
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn complete_order_seeds_garden_plant_for_strain_orders() {
    let Some((_app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };

    let suffix = rand_suffix();
    let telegram_id: i64 = 999_200_000 + (suffix as i64);

    // Seed a strain.
    let strain_id = uuid::Uuid::new_v4().to_string();
    let strain_name = format!("integration-seed-{}", suffix);
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO strains (id, name, price_per_gram, is_available) \
             VALUES ($1, $2, $3, TRUE)",
            [
                strain_id.clone().into(),
                strain_name.clone().into(),
                100.0_f64.into(),
            ],
        ))
        .await
        .expect("seed strain INSERT");

    // Seed a pending order with one strain item. Bypass
    // validate_create_order entirely — we're testing the completion
    // path, not the create path.
    let order_id = uuid::Uuid::new_v4().to_string();
    let items = json!([{
        "strain_id": strain_id,
        "strain_name": strain_name,
        "quantity": 1.0,
    }]);
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO orders (id, telegram_id, customer_name, items, subtotal, bonus_used, total, status, created_at) \
             VALUES ($1, $2, $3, $4::jsonb, $5, 0, $5, 'pending', NOW())",
            [
                order_id.clone().into(),
                telegram_id.into(),
                format!("test-customer-{}", suffix).into(),
                items.to_string().into(),
                100.0_f64.into(),
            ],
        ))
        .await
        .expect("seed order INSERT");

    // Pre-condition: no garden_plants row for this user.
    let before = fetch_plants(&db, &telegram_id.to_string()).await;
    assert!(
        before.is_empty(),
        "pre-condition: user must have no plants, got: {:?}",
        before
    );

    // Trigger the production path.
    let result = complete_order_and_update_loyalty(&db.orm, &order_id)
        .await
        .expect("complete_order_and_update_loyalty");
    assert!(result.is_some(), "must return Some((tid, is_first))");
    let (tid, is_first) = result.unwrap();
    assert_eq!(tid, telegram_id);
    assert!(is_first, "first order for this user → is_first=true");

    // Post-condition: exactly one plant row for this user, with the
    // right strain.
    let after = fetch_plants(&db, &telegram_id.to_string()).await;
    assert_eq!(
        after.len(),
        1,
        "expected exactly 1 seeded plant, got {} rows",
        after.len()
    );
    let plant = &after[0];
    assert_eq!(plant.strain_id, strain_id);
    assert_eq!(plant.strain_name, strain_name);
    assert_eq!(plant.current_stage, "seed");
    assert!(!plant.is_completed);
    assert_eq!(plant.water_count, 0);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn complete_order_skips_seeding_when_active_plant_exists() {
    // Cycle-#94 invariant: one active plant per user. The seed-on-
    // completion path must NOT insert a second plant if the user
    // already has one with is_completed=false.
    let Some((_app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };

    let suffix = rand_suffix();
    let telegram_id: i64 = 999_100_000 + (suffix as i64);
    let strain_id = uuid::Uuid::new_v4().to_string();
    let strain_name = format!("integration-second-seed-{}", suffix);

    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO strains (id, name, price_per_gram, is_available) \
             VALUES ($1, $2, 100, TRUE)",
            [strain_id.clone().into(), strain_name.clone().into()],
        ))
        .await
        .expect("seed strain");

    // Pre-seed: user already has an active plant.
    let existing_plant_id = uuid::Uuid::new_v4().to_string();
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO garden_plants (id, user_id, strain_id, strain_name, current_stage, planted_at, is_completed, water_count) \
             VALUES ($1, $2, $3, $4, 'seed', 1700000000000, false, 0)",
            [
                existing_plant_id.clone().into(),
                telegram_id.to_string().into(),
                strain_id.clone().into(),
                strain_name.clone().into(),
            ],
        ))
        .await
        .expect("pre-seed existing plant");

    let order_id = uuid::Uuid::new_v4().to_string();
    let items = json!([{
        "strain_id": strain_id,
        "strain_name": strain_name,
        "quantity": 1.0,
    }]);
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO orders (id, telegram_id, customer_name, items, subtotal, bonus_used, total, status, created_at) \
             VALUES ($1, $2, $3, $4::jsonb, 100, 0, 100, 'pending', NOW())",
            [
                order_id.clone().into(),
                telegram_id.into(),
                format!("test-customer-{}", suffix).into(),
                items.to_string().into(),
            ],
        ))
        .await
        .expect("seed order");

    let _ = complete_order_and_update_loyalty(&db.orm, &order_id)
        .await
        .expect("complete_order");

    let plants = fetch_plants(&db, &telegram_id.to_string()).await;
    assert_eq!(
        plants.len(),
        1,
        "must stay at 1 plant (existing one), got {} rows",
        plants.len()
    );
    assert_eq!(
        plants[0].id, existing_plant_id,
        "the lone plant must be the pre-existing one, not a new insert"
    );
}

fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}
