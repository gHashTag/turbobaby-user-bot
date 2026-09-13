//! Integration test for cycle-#57 idempotency on `POST /api/orders`
//! (cycle #165). This is the OLDEST untested behavioural change in the
//! codebase — cycle #57 shipped X-Idempotency-Key for orders before any
//! integration test infrastructure existed.
//!
//! Marked `#[ignore]` — runs with:
//!
//! ```sh
//! DATABASE_URL=postgres://... cargo test --features backend -- --ignored
//! ```
//!
//! Test flow:
//!   1. Seed a fresh `strains` row with a known `price_per_gram = 100.0`
//!      so the server-side price authority recompute has a deterministic
//!      target.
//!   2. POST an anonymous order (no telegram_id → skips owner check;
//!      anon rate-limit isolated by client_ip header) with one strain
//!      item, quantity 2.5, subtotal = total = 250.
//!   3. Assert 200 + order_id + no replay flag.
//!   4. POST the same body with the same idempotency key. Assert 200 +
//!      same order_id + `idempotent_replay: true`.
//!   5. DB assertion: exactly one row in `orders` for this `customer_name`.
//!      Catches a regression where the replay branch double-inserts.

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sea_orm::{ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, Statement};
use serde_json::json;
use tower::ServiceExt;

use turbobaby_bot::db::entities::order::{Column as OrderCol, Entity as OrderEntity};

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn create_order_idempotent_replay_returns_same_order_id() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    // Seed a fresh strain so the server-side price-authority recompute
    // has a row to look at. Unique name per run prevents UNIQUE clashes
    // with seed migration 008 entries.
    let strain_id = uuid::Uuid::new_v4().to_string();
    let suffix = rand_suffix();
    let strain_name = format!("integration-test-strain-{}", suffix);
    let price_per_gram = 100.0_f64;
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO strains (id, name, price_per_gram, is_available) \
             VALUES ($1, $2, $3, TRUE)",
            [
                strain_id.clone().into(),
                strain_name.clone().into(),
                price_per_gram.into(),
            ],
        ))
        .await
        .expect("seed strain INSERT");

    // Unique customer_name lets us count rows for THIS test run only —
    // multiple `--ignored` invocations against the same DB don't collide.
    let customer_name = format!("integration-test-customer-{}", suffix);
    let idem_key = uuid::Uuid::new_v4().to_string();
    let quantity = 2.5_f64;
    let expected_total = price_per_gram * quantity;

    let body = json!({
        "telegram_id": null,  // anonymous path — skips check_owner
        "customer_name": customer_name,
        "customer_phone": "+66000000000",
        "customer_telegram": null,
        "items": [{
            "strain_id": strain_id,
            "strain_name": strain_name,
            "quantity": quantity,
        }],
        "subtotal": expected_total,
        "bonus_used": 0.0,
        "total": expected_total,
        "shop_id": null,
        "age_confirmed": true,
        "delivery_zone_id": null,
    });

    let first = post_order(app.clone(), &idem_key, &body, "127.0.0.1").await;
    assert_eq!(
        first.status,
        StatusCode::OK,
        "first call must succeed, body: {}",
        first.body
    );
    assert_eq!(first.body["success"], true);
    let first_order_id = first.body["order_id"]
        .as_str()
        .expect("first response must carry order_id")
        .to_string();
    assert!(
        first.body["idempotent_replay"].is_null() || first.body["idempotent_replay"] == false,
        "first call must NOT be marked as a replay"
    );

    // Replay: same key + body.
    let second = post_order(app, &idem_key, &body, "127.0.0.1").await;
    assert_eq!(
        second.status,
        StatusCode::OK,
        "replay must succeed, body: {}",
        second.body
    );
    assert_eq!(
        second.body["order_id"].as_str(),
        Some(first_order_id.as_str()),
        "replay must return the original order_id"
    );
    assert_eq!(
        second.body["idempotent_replay"], true,
        "replay must carry idempotent_replay: true"
    );

    // DB assertion: exactly one row in `orders` for this test customer.
    // A regression that double-inserts on replay would show 2 rows.
    let rows = OrderEntity::find()
        .filter(OrderCol::CustomerName.eq(customer_name.clone()))
        .all(&db.orm)
        .await
        .expect("orders query");
    assert_eq!(
        rows.len(),
        1,
        "expected exactly 1 order row for {}, got {}",
        customer_name,
        rows.len()
    );
    assert_eq!(
        rows[0].id, first_order_id,
        "the lone order row must match the returned order_id"
    );
}

/// Regression: an order placed with a local phone number and no delivery
/// address — the in-store ("offline") case — must be accepted end to end.
///
/// Both halves of this used to be rejected: the phone because the gate
/// demanded a literal leading `+`, the missing address because checkout
/// required one unconditionally. The client never even sent the request, so
/// nothing in the server logs showed the failure.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn create_order_accepts_local_phone_and_pickup_without_address() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    let strain_id = uuid::Uuid::new_v4().to_string();
    let suffix = rand_suffix();
    let strain_name = format!("integration-pickup-strain-{}", suffix);
    let price_per_gram = 100.0_f64;
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO strains (id, name, price_per_gram, is_available) \
             VALUES ($1, $2, $3, TRUE)",
            [
                strain_id.clone().into(),
                strain_name.clone().into(),
                price_per_gram.into(),
            ],
        ))
        .await
        .expect("seed strain INSERT");

    let customer_name = format!("integration-pickup-customer-{}", suffix);
    let quantity = 1.0_f64;
    let expected_total = price_per_gram * quantity;

    let body = json!({
        "telegram_id": null,
        "customer_name": customer_name,
        // Local Thai mobile, exactly as a customer's own phone shows it.
        "customer_phone": "081 234 5678",
        "customer_telegram": null,
        "items": [{
            "strain_id": strain_id,
            "strain_name": strain_name,
            "quantity": quantity,
        }],
        "subtotal": expected_total,
        "bonus_used": 0.0,
        "total": expected_total,
        "shop_id": null,
        "fulfillment": "pickup",
        // No delivery_address at all — this is a collect-in-store order.
        "age_confirmed": true,
        "delivery_zone_id": null,
    });

    let resp = post_order(app, &uuid::Uuid::new_v4().to_string(), &body, "127.0.0.2").await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "pickup order with a local phone must be accepted, body: {}",
        resp.body
    );
    let order_id = resp.body["order_id"]
        .as_str()
        .expect("response must carry order_id");

    // The stored number must be E.164 so the courier/admin see one canonical
    // form regardless of how the customer typed it.
    let rows = OrderEntity::find()
        .filter(OrderCol::CustomerName.eq(customer_name.clone()))
        .all(&db.orm)
        .await
        .expect("orders query");
    assert_eq!(rows.len(), 1, "expected exactly one order row");
    assert_eq!(rows[0].id, order_id);
    assert_eq!(
        rows[0].customer_phone.as_deref(),
        Some("+66812345678"),
        "phone must be normalised to E.164 before it is stored"
    );
}

/// An order must never be created without an explicit 20+ confirmation.
/// This is a compliance gate, so it is asserted against the real endpoint
/// rather than only against the pure validator.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn create_order_rejects_missing_age_confirmation() {
    let Some((app, _db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    let body = json!({
        "telegram_id": null,
        "customer_name": format!("integration-age-customer-{}", rand_suffix()),
        "customer_phone": "+66812345678",
        "items": [{ "strain_id": uuid::Uuid::new_v4().to_string(), "quantity": 1.0 }],
        "subtotal": 100.0,
        "total": 100.0,
        "age_confirmed": false,
    });

    let resp = post_order(app, &uuid::Uuid::new_v4().to_string(), &body, "127.0.0.3").await;
    assert_eq!(
        resp.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "an unconfirmed-age order must be refused, body: {}",
        resp.body
    );
}

/// A client that under-reports the total must not get a cheap order: the
/// server recomputes from DB prices and rejects the mismatch.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn create_order_rejects_client_supplied_wrong_total() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    let strain_id = uuid::Uuid::new_v4().to_string();
    let suffix = rand_suffix();
    let strain_name = format!("integration-fraud-strain-{}", suffix);
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

    let body = json!({
        "telegram_id": null,
        "customer_name": format!("integration-fraud-customer-{}", suffix),
        "customer_phone": "+66812345678",
        "items": [{
            "strain_id": strain_id,
            "strain_name": strain_name,
            "quantity": 10.0,
        }],
        // Real price is 10 × 100 = 1000.
        "subtotal": 1.0,
        "bonus_used": 0.0,
        "total": 1.0,
        "age_confirmed": true,
    });

    let resp = post_order(app, &uuid::Uuid::new_v4().to_string(), &body, "127.0.0.4").await;
    assert_ne!(
        resp.status,
        StatusCode::OK,
        "a mismatched subtotal must not create an order, body: {}",
        resp.body
    );
}

struct OrderResp {
    status: StatusCode,
    body: serde_json::Value,
}

/// `client_ip` must be unique per test: anonymous orders are rate-limited to
/// 3/minute per IP, so tests sharing one address make the later ones 429 and
/// assert against the wrong failure.
async fn post_order(
    app: axum::Router,
    idem_key: &str,
    body: &serde_json::Value,
    client_ip: &str,
) -> OrderResp {
    let request = Request::builder()
        .method("POST")
        .uri("/api/orders")
        .header("content-type", "application/json")
        .header("x-idempotency-key", idem_key)
        // Per-test client IP via the standard forwarded header: the
        // anonymous-order rate-limit store is keyed on it.
        .header("x-forwarded-for", client_ip)
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap();
    let response = app.oneshot(request).await.expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    OrderResp { status, body }
}

fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}
