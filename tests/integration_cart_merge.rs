//! Regression test for the `POST /api/cart/merge` 5xx on prod.
//!
//! Both `carts.telegram_id` and migration 081's partial unique index over
//! undated `cart_items (cart_id, kind, catalog_id)` serialize writes to one
//! logical row. The handler used to read-then-insert against the predecessor
//! constraints. Two merges landing at once for the same user — which is
//! exactly what a double-tapped "reorder" button produces — raced: the loser
//! hit a duplicate-key error, which the handler mapped to
//! `INTERNAL_SERVER_ERROR`. Inside the merge transaction the failed statement
//! also aborted every write before it, so a multi-item cart could be silently
//! dropped.
//!
//! The handler now upserts (`ON CONFLICT`) instead, so both writers succeed
//! and Postgres serializes them.
//!
//! REWRITTEN 2026-09-26 on the rental catalogue. Until then both tests merged
//! an available accessory, and migration 085 hid every accessory, so both
//! failed on a database migrated to today's schema. What the merge can reach
//! today, measured from `src/api/cart.rs`: its write gate (`parse_kind`) still
//! names only the old catalogue's four kinds, each of which 083 dropped or 085
//! hid, and does not name `bike_rental`, the one kind a cart serves
//! (`trios::pricing::SERVED_CART_KINDS`). Opening that gate is a separate
//! change (`specs/turbobaby/cart_persistence.t27`). So through the API a merge
//! of rental lines writes no line, and these tests hold what is still the
//! handler's: two concurrent merges for a user with no cart both answer 200
//! and leave ONE cart (the `carts.telegram_id` race), and a payload with
//! duplicate lines is answered, not rejected. The summing of two writers on
//! one rental line -- the upsert this file used to reach through an accessory
//! -- is run against the database at the write step itself, in
//! `concurrent_upserts_of_one_rental_line_sum_their_quantities`
//! (`src/api/cart.rs`).
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
use sea_orm::{ConnectionTrait, DbBackend, Statement, TryGetable};
use serde_json::json;
use tower::ServiceExt;

const BOT_TOKEN: &str = "dummy_test_token";

/// A rental line of the seeded `nmax-155` family (migration 082), as a
/// device cart replays it.
fn rental_line(quantity: i32) -> serde_json::Value {
    json!({
        "id": "",
        "kind": "bike_rental",
        "catalog_id": "nmax-155",
        "quantity": quantity,
        "unit_price": 0.0,
        "name": "",
    })
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn concurrent_merges_do_not_500_and_leave_one_cart() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    let tid: i64 = 999_800_000 + (std::process::id() as i64 % 100_000);
    cleanup(&db, tid).await;

    let body = json!({ "telegram_id": tid, "items": [rental_line(2)] });

    // Fire both merges concurrently against a user that has no cart yet, so
    // they race on the cart's unique constraint.
    let (a, b) = tokio::join!(
        post_merge(app.clone(), tid, body.clone()),
        post_merge(app.clone(), tid, body.clone()),
    );

    for (merge, (status, served)) in [("first", a), ("second", b)] {
        assert_eq!(
            status,
            StatusCode::OK,
            "{merge} concurrent merge must not 5xx"
        );
        // The served cart: the merge wrote no line (its gate names no rental
        // kind), and nothing else is served.
        assert_eq!(served["items"], json!([]), "{merge}: {served}");
        assert_eq!(served["total"], json!(0.0), "{merge}: {served}");
    }

    // Exactly one cart, and no line written through the API.
    assert_eq!(cart_count(&db, tid).await, 1, "must not create two carts");
    let (lines, qty) = cart_line_state(&db, tid).await;
    assert_eq!((lines, qty), (0, 0), "the merge gate admits no rental line");

    cleanup(&db, tid).await;
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn duplicate_lines_in_one_payload_are_not_rejected() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    let tid: i64 = 999_700_000 + (std::process::id() as i64 % 100_000);
    cleanup(&db, tid).await;

    let (status, served) = post_merge(
        app,
        tid,
        json!({ "telegram_id": tid, "items": [rental_line(3), rental_line(3)] }),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "duplicate payload lines must not be rejected"
    );
    assert_eq!(served["items"], json!([]), "{served}");
    let (lines, qty) = cart_line_state(&db, tid).await;
    assert_eq!((lines, qty), (0, 0), "the merge gate admits no rental line");

    cleanup(&db, tid).await;
}

async fn post_merge(
    app: axum::Router,
    tid: i64,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("POST")
        .uri("/api/cart/merge")
        .header("Content-Type", "application/json")
        .header(
            "X-Telegram-Init-Data",
            common::make_init_data(tid, BOT_TOKEN),
        )
        .body(Body::from(body.to_string()))
        .expect("request builds");
    let resp = app.oneshot(req).await.expect("router responds");
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .map(|b| b.to_bytes())
        .unwrap_or_default();
    if status.is_server_error() {
        eprintln!("merge 5xx body: {bytes:?}");
    }
    let served = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, served)
}

async fn cart_count(db: &turbobaby_bot::db::Database, tid: i64) -> i64 {
    scalar::<i64>(
        db,
        &format!("SELECT count(*)::int8 AS n FROM carts WHERE telegram_id = {tid}"),
        "n",
    )
    .await
    .unwrap_or(-1)
}

/// `(number of cart_items rows, total quantity)` for the user's cart.
async fn cart_line_state(db: &turbobaby_bot::db::Database, tid: i64) -> (i64, i64) {
    let sql = format!(
        "SELECT count(*)::int8 AS n, COALESCE(sum(quantity), 0)::int8 AS q \
         FROM cart_items i JOIN carts c ON c.id = i.cart_id WHERE c.telegram_id = {tid}"
    );
    let row = db
        .orm
        .query_one(Statement::from_string(DbBackend::Postgres, sql))
        .await
        .expect("query runs")
        .expect("aggregate always returns a row");
    (
        row.try_get("", "n").unwrap_or(-1),
        row.try_get("", "q").unwrap_or(-1),
    )
}

async fn scalar<T: TryGetable>(
    db: &turbobaby_bot::db::Database,
    sql: &str,
    col: &str,
) -> Option<T> {
    db.orm
        .query_one(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get("", col).ok())
}

async fn cleanup(db: &turbobaby_bot::db::Database, tid: i64) {
    // cart_items cascades from carts.
    let _ = db
        .orm
        .execute(Statement::from_string(
            DbBackend::Postgres,
            format!("DELETE FROM carts WHERE telegram_id = {tid}"),
        ))
        .await;
}
