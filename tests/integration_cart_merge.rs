//! Regression test for the `POST /api/cart/merge` 5xx on prod.
//!
//! Both `carts.telegram_id` and `cart_items (cart_id, kind, catalog_id)` are
//! UNIQUE, and the handler used to read-then-insert against them. Two merges
//! landing at once for the same user — which is exactly what a double-tapped
//! "reorder" button produces — raced: the loser hit a duplicate-key error,
//! which the handler mapped to `INTERNAL_SERVER_ERROR`. Inside the merge
//! transaction the failed statement also aborted every write before it, so a
//! multi-item cart could be silently dropped.
//!
//! The handler now upserts (`ON CONFLICT`) instead, so both writers succeed
//! and Postgres serializes them.
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

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn concurrent_merges_do_not_500_and_sum_quantities() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    let Some(strain_id) = first_strain_id(&db).await else {
        eprintln!("no strains seeded — skipping");
        return;
    };
    let tid: i64 = 999_800_000 + (std::process::id() as i64 % 100_000);
    cleanup(&db, tid).await;

    let body = json!({
        "telegram_id": tid,
        "items": [{
            "id": "",
            "kind": "strain",
            "catalog_id": strain_id,
            "quantity": 2,
            "unit_price": 0.0,
            "name": "",
        }],
    });

    // Fire both merges concurrently against a user that has no cart yet, so
    // they race on BOTH unique constraints at once.
    let (a, b) = tokio::join!(
        post_merge(app.clone(), tid, body.clone()),
        post_merge(app.clone(), tid, body.clone()),
    );

    assert_eq!(a, StatusCode::OK, "first concurrent merge must not 5xx");
    assert_eq!(b, StatusCode::OK, "second concurrent merge must not 5xx");

    // Exactly one cart, one line, and both merges counted.
    assert_eq!(cart_count(&db, tid).await, 1, "must not create two carts");
    let (lines, qty) = cart_line_state(&db, tid).await;
    assert_eq!(lines, 1, "same catalog item must collapse into one line");
    assert_eq!(qty, 4, "both merges must be applied (2 + 2)");

    cleanup(&db, tid).await;
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn duplicate_lines_in_one_payload_are_summed_not_rejected() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    let Some(strain_id) = first_strain_id(&db).await else {
        eprintln!("no strains seeded — skipping");
        return;
    };
    let tid: i64 = 999_700_000 + (std::process::id() as i64 % 100_000);
    cleanup(&db, tid).await;

    let line = json!({
        "id": "",
        "kind": "strain",
        "catalog_id": strain_id,
        "quantity": 3,
        "unit_price": 0.0,
        "name": "",
    });
    let status = post_merge(
        app,
        tid,
        json!({ "telegram_id": tid, "items": [line.clone(), line] }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let (lines, qty) = cart_line_state(&db, tid).await;
    assert_eq!(lines, 1);
    assert_eq!(qty, 6, "duplicate payload lines must be summed");

    cleanup(&db, tid).await;
}

async fn post_merge(app: axum::Router, tid: i64, body: serde_json::Value) -> StatusCode {
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
    if status.is_server_error() {
        let bytes = resp.into_body().collect().await.map(|b| b.to_bytes());
        eprintln!("merge 5xx body: {bytes:?}");
    }
    status
}

async fn first_strain_id(db: &woody_weed_bot::db::Database) -> Option<String> {
    scalar::<String>(db, "SELECT id FROM strains LIMIT 1", "id").await
}

async fn cart_count(db: &woody_weed_bot::db::Database, tid: i64) -> i64 {
    scalar::<i64>(
        db,
        &format!("SELECT count(*)::int8 AS n FROM carts WHERE telegram_id = {tid}"),
        "n",
    )
    .await
    .unwrap_or(-1)
}

/// `(number of cart_items rows, total quantity)` for the user's cart.
async fn cart_line_state(db: &woody_weed_bot::db::Database, tid: i64) -> (i64, i64) {
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
    db: &woody_weed_bot::db::Database,
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

async fn cleanup(db: &woody_weed_bot::db::Database, tid: i64) {
    // cart_items cascades from carts.
    let _ = db
        .orm
        .execute(Statement::from_string(
            DbBackend::Postgres,
            format!("DELETE FROM carts WHERE telegram_id = {tid}"),
        ))
        .await;
}
