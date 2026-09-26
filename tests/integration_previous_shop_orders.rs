//! A customer does not see an order of the previous shop, against a real
//! database and the live router.
//!
//! Owner, 2026-09-26, asked why the lines of the previous shop's orders are
//! shown at all (verbatim in `src/trios/legacy_view.rs`); the operator read it
//! as: a customer must not see the previous shop's orders at all. An order
//! naming the previous shop, or holding no bike line, is neither listed, nor
//! answered by id, nor counted on the profile, and nothing is written for it.
//! A rental beside a line of a retired kind is this shop's order and stays
//! shown, with that line under the neutral name. The admin reads, which are
//! the archive, see every row whole. The rule is recorded in
//! `specs/turbobaby/order_presentation.t27` (`PREVIOUS_SHOP_ORDER_*`), and
//! `tests/legacy_view_wiring.rs` holds the handlers to it as text; this file
//! runs it.
//!
//! The rows are seeded directly, in the shapes the previous shop and this
//! checkout store, with neutral stand-in names. `#[ignore]`; run against a
//! THROWAWAY local database:
//!
//! ```sh
//! DATABASE_URL=postgres://postgres@127.0.0.1:<port>/<fresh db> \
//!   cargo test --features backend --test integration_previous_shop_orders -- --include-ignored --test-threads=1
//! ```

#![cfg(feature = "backend")]
// A panic is how a test reports failure.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::{json, Value};
use tower::ServiceExt;

const BOT_TOKEN: &str = "dummy_test_token";
const LIVE_SHOP: &str = "\u{1f3e0} TurboBaby";
const OLD_SHOP: &str = "\u{1f3e0} Woody Phangan";

fn bike_line() -> Value {
    json!({
        "quantity": 1.0, "unit_price": null, "fulfillment": "pickup",
        "bike": { "bike_key": "nmax-155", "bike_name": "Yamaha NMAX 155",
                  "deal": { "kind": "bike_rental", "rental_start": "2026-10-01",
                            "rental_end": "2026-10-03", "rate_thb_day": null, "deposit": null } }
    })
}

fn old_line() -> Value {
    json!({ "accessory_id": "stored-item-4p", "accessory_name": "Stored item name",
            "quantity": 1.0, "unit_price": 500.0, "is_accessory": true })
}

async fn exec(db: &turbobaby_bot::db::Database, sql: &str, values: Vec<sea_orm::Value>) {
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            values,
        ))
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn clean(db: &turbobaby_bot::db::Database, tid: i64) {
    exec(
        db,
        "DELETE FROM orders WHERE telegram_id = $1",
        vec![tid.into()],
    )
    .await;
    exec(
        db,
        "DELETE FROM loyalty_profiles WHERE telegram_id = $1",
        vec![tid.into()],
    )
    .await;
}

/// One stored order, `age_hours` old.
async fn seed_order(
    db: &turbobaby_bot::db::Database,
    id: &str,
    tid: i64,
    items: Value,
    shop: Option<&str>,
    age_hours: i64,
) {
    exec(
        db,
        "INSERT INTO orders (id, telegram_id, items, total, status, shop_id, created_at) \
         VALUES ($1, $2, $3::jsonb, 0, 'pending', $4, NOW() - ($5 * INTERVAL '1 hour'))",
        vec![
            id.into(),
            tid.into(),
            items.to_string().into(),
            shop.map(str::to_string).into(),
            age_hours.into(),
        ],
    )
    .await;
}

async fn call(app: &axum::Router, method: &str, uri: String, tid: i64) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header(
            "X-Telegram-Init-Data",
            common::make_init_data(tid, BOT_TOKEN),
        )
        .body(Body::empty())
        .expect("request builds");
    let response = app.clone().oneshot(request).await.expect("router answers");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn stored_status(db: &turbobaby_bot::db::Database, id: &str) -> String {
    db.orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT status FROM orders WHERE id = $1",
            [id.into()],
        ))
        .await
        .expect("query")
        .expect("the row is still there")
        .try_get("", "status")
        .expect("a status")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_previous_shops_orders_are_not_listed_answered_or_counted() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid: i64 = 557_100_000 + (std::process::id() as i64 % 100_000);
    clean(&db, tid).await;
    exec(
        &db,
        "INSERT INTO loyalty_profiles (telegram_id, bonus_balance, total_spent) VALUES ($1, 0, 0)",
        vec![tid.into()],
    )
    .await;

    let p = format!("prev-shop-{tid}");
    // This shop's: a rental, and a rental beside a line of the old catalogue.
    seed_order(
        &db,
        &format!("{p}-rental"),
        tid,
        json!([bike_line()]),
        Some(LIVE_SHOP),
        90,
    )
    .await;
    seed_order(
        &db,
        &format!("{p}-mixed"),
        tid,
        json!([bike_line(), old_line()]),
        Some(LIVE_SHOP),
        80,
    )
    .await;
    // The previous shop's: the old catalogue alone, under either shop, a rental
    // naming the previous shop, and items that are not a list.
    seed_order(
        &db,
        &format!("{p}-old"),
        tid,
        json!([old_line()]),
        Some(OLD_SHOP),
        70,
    )
    .await;
    seed_order(
        &db,
        &format!("{p}-oldhere"),
        tid,
        json!([old_line()]),
        Some(LIVE_SHOP),
        60,
    )
    .await;
    seed_order(
        &db,
        &format!("{p}-oldbike"),
        tid,
        json!([bike_line()]),
        Some(OLD_SHOP),
        50,
    )
    .await;
    seed_order(&db, &format!("{p}-empty"), tid, json!([]), None, 40).await;
    // Sixty more of the previous shop's, all newer than both of this shop's:
    // with the old `.limit(50)` they would have filled the list.
    for i in 0..60 {
        seed_order(
            &db,
            &format!("{p}-flood-{i:02}"),
            tid,
            json!([old_line()]),
            Some(OLD_SHOP),
            30,
        )
        .await;
    }

    // The list: this shop's two, newest first, and nothing else.
    let (status, body) = call(&app, "GET", format!("/api/orders/user/{tid}"), tid).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<&str> = body["orders"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|o| o["id"].as_str().expect("an id"))
        .collect();
    assert_eq!(ids, [format!("{p}-mixed"), format!("{p}-rental")], "{body}");
    // The mixed order keeps its rental and masks the other line.
    let mixed = &body["orders"][0]["items"];
    assert_eq!(mixed[0]["bike"]["bike_key"], "nmax-155", "{mixed}");
    assert_eq!(
        mixed[1]["strain_name"], "Позиция прежнего каталога",
        "{mixed}"
    );
    assert!(!mixed.to_string().contains("Stored item name"), "{mixed}");

    // By id: each of the previous shop's answers as a missing order would.
    for hidden in ["old", "oldhere", "oldbike", "empty", "flood-00"] {
        let id = format!("{p}-{hidden}");
        for path in ["details", "status"] {
            let (status, _) = call(
                &app,
                "GET",
                format!("/api/orders/{id}/{path}?telegram_id={tid}"),
                tid,
            )
            .await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{id} {path}");
        }
        let (status, _) = call(
            &app,
            "POST",
            format!("/api/orders/{id}/cancel?telegram_id={tid}"),
            tid,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id} cancel");
        // Nothing was written: still pending, as stored.
        assert_eq!(stored_status(&db, &id).await, "pending", "{id}");
    }
    let missing = format!("{p}-never-stored");
    let (status, _) = call(
        &app,
        "GET",
        format!("/api/orders/{missing}/details?telegram_id={tid}"),
        tid,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the same answer as a missing order"
    );

    // This shop's answer by id as before.
    for shown in ["rental", "mixed"] {
        let id = format!("{p}-{shown}");
        for path in ["details", "status"] {
            let (status, body) = call(
                &app,
                "GET",
                format!("/api/orders/{id}/{path}?telegram_id={tid}"),
                tid,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{id} {path}: {body}");
        }
    }

    // The profile counts only the two it can list.
    let (status, body) = call(&app, "GET", format!("/api/loyalty/{tid}"), tid).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["profile"]["orders_count"], 2, "{body}");

    // The archive: the admin read serves the previous shop's order whole.
    let admin_token = turbobaby_bot::api::auth::generate_admin_token("test_password", BOT_TOKEN);
    let request = Request::builder()
        .uri(format!("/api/orders/{p}-old"))
        .header("x-admin-token", admin_token)
        .body(Body::empty())
        .expect("request builds");
    let response = app.clone().oneshot(request).await.expect("router answers");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let admin: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(admin["order"]["shop_id"], OLD_SHOP, "{admin}");
    assert_eq!(
        admin["order"]["items"][0]["accessory_name"],
        "Stored item name"
    );

    clean(&db, tid).await;
}
