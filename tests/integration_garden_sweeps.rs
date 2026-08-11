//! Guard for the defect class that has now killed garden background work twice.
//!
//! `src/api/garden.rs` stores every time on `garden_plants` / `garden_rewards`
//! as epoch milliseconds in a `BIGINT` — `planted_at`, `last_watered_at`,
//! `harvested_at`, `expires_at`. Twice a later migration added a *new* time
//! column to one of those tables as `TIMESTAMPTZ`, and Postgres then refused
//! the comparison:
//!
//! ```text
//! operator does not exist: timestamp with time zone <= bigint
//! ```
//!
//! Migration 069 fixed `streak_last_watered_at` / `streak_broken_at` after
//! watering 500-ed. Migration 072 fixed `reminder_sent_at` /
//! `expiry_nudge_sent_at` after both background sweeps turned out to have been
//! failing on every tick since the columns were introduced.
//!
//! The second one hid for far longer because the sweeps only log a warning:
//! nothing 500-ed, no customer complained, watering reminders simply never
//! arrived. So presence-checking the column is not enough — the *type* is what
//! breaks, and it breaks silently.
//!
//! Run with:
//! ```sh
//! DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/woody_test \
//!   cargo test --features backend --test integration_garden_sweeps -- --ignored
//! ```

#![cfg(feature = "backend")]

mod common;

use common::make_app_with_db;
use sea_orm::{ConnectionTrait, DbBackend, Statement};

/// Every column `src/api/garden.rs` reads or writes as an `i64` of epoch
/// milliseconds. Add to this list whenever a new one is introduced.
const EPOCH_MILLIS_COLUMNS: &[(&str, &str)] = &[
    ("garden_plants", "planted_at"),
    ("garden_plants", "last_watered_at"),
    ("garden_plants", "harvested_at"),
    ("garden_plants", "reminder_sent_at"),
    ("garden_plants", "streak_last_watered_at"),
    ("garden_plants", "streak_broken_at"),
    ("garden_rewards", "expires_at"),
    ("garden_rewards", "expiry_nudge_sent_at"),
];

#[tokio::test]
#[ignore]
async fn garden_time_columns_are_epoch_millis_bigints() {
    let Some((_app, db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };
    let orm = &db.orm;

    let rows = orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT table_name, column_name, data_type \
             FROM information_schema.columns \
             WHERE table_name IN ('garden_plants', 'garden_rewards')",
            [],
        ))
        .await
        .expect("information_schema query");

    let mut types = std::collections::HashMap::new();
    for r in rows {
        let table: String = r.try_get("", "table_name").unwrap_or_default();
        let column: String = r.try_get("", "column_name").unwrap_or_default();
        let data_type: String = r.try_get("", "data_type").unwrap_or_default();
        types.insert((table, column), data_type);
    }

    let mut wrong = Vec::new();
    for (table, column) in EPOCH_MILLIS_COLUMNS {
        match types.get(&((*table).to_string(), (*column).to_string())) {
            None => wrong.push(format!("{table}.{column} is missing entirely")),
            Some(t) if t != "bigint" => {
                wrong.push(format!("{table}.{column} is {t}, expected bigint"))
            }
            Some(_) => {}
        }
    }
    assert!(
        wrong.is_empty(),
        "garden time columns drifted from the epoch-millis convention the code \
         uses; comparisons against these will fail at runtime:\n  {}",
        wrong.join("\n  ")
    );
}

/// The type guard above says the schema is right. This says Postgres actually
/// accepts the comparisons the sweeps make — the thing that was failing.
#[tokio::test]
#[ignore]
async fn garden_sweep_predicates_are_accepted_by_postgres() {
    let Some((_app, db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };
    let orm = &db.orm;

    let now = chrono::Utc::now().timestamp_millis();

    // Watering reminders: the predicate from `send_garden_reminders`.
    orm.query_all(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT p.id FROM garden_plants p \
         WHERE p.harvested_at IS NULL \
           AND p.is_completed = false \
           AND (p.reminder_sent_at IS NULL OR p.reminder_sent_at <= $1) \
           AND (p.last_watered_at IS NULL OR p.last_watered_at <= $2) \
         LIMIT 1",
        [now.into(), now.into()],
    ))
    .await
    .expect("watering-reminder predicate rejected by Postgres");

    // Reward expiry nudges: the predicate from
    // `send_garden_reward_expiry_nudges`.
    orm.query_all(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT r.id FROM garden_rewards r \
         WHERE r.is_used = false \
           AND r.expires_at > $1 \
           AND r.expires_at <= $2 \
           AND (r.expiry_nudge_sent_at IS NULL OR r.expiry_nudge_sent_at <= $3) \
         LIMIT 1",
        [now.into(), now.into(), now.into()],
    ))
    .await
    .expect("reward-expiry predicate rejected by Postgres");

    // And the writes, which are the other half of each sweep.
    orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE garden_plants SET reminder_sent_at = $1 WHERE id = 'no-such-plant'",
        [now.into()],
    ))
    .await
    .expect("reminder_sent_at write rejected by Postgres");

    orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE garden_rewards SET expiry_nudge_sent_at = $1 WHERE id = 'no-such-reward'",
        [now.into()],
    ))
    .await
    .expect("expiry_nudge_sent_at write rejected by Postgres");
}
