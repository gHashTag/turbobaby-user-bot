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
//! REWRITTEN 2026-09-26 on the rental catalogue. Until then these tests seeded
//! a `strains` row, a table migration 083 dropped, and three of them failed on
//! every database migrated to today's schema. The checkout takes rental lines
//! only since the owner's ruling of 2026-09-24 (rental only, Phuket only), so
//! the order is now one `bike_rental` line of the seeded `nmax-155` family
//! (migration 082), dated ten days ahead. What each test is FOR is unchanged.
//!
//! Test flow of the idempotency test:
//!   1. POST an anonymous order (no telegram_id → skips owner check;
//!      anon rate-limit isolated by client_ip header) with one rental line.
//!      A rental is quoted at the door, so a cart of rental lines claims a
//!      subtotal and total of 0 (`check_full_subtotal`).
//!   2. Assert 200 + order_id + no replay flag.
//!   3. POST the same body with the same idempotency key. Assert 200 +
//!      same order_id + `idempotent_replay: true`.
//!   4. DB assertion: exactly one row in `orders` for this `customer_name`.
//!      Catches a regression where the replay branch double-inserts.

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use tower::ServiceExt;

use turbobaby_bot::db::entities::order::{Column as OrderCol, Entity as OrderEntity};

/// The family every test books: offered, with a published rate (449 THB a
/// day) and deposit (3000 THB) in `migrations/082_bikes_seed.sql`.
const FAMILY: &str = "nmax-155";

/// One rental line of [`FAMILY`], from ten days ahead for `days` days, with
/// the given deposit (none agreed when `null`).
fn rental_line(days: i64, deposit: serde_json::Value) -> serde_json::Value {
    let start = chrono::Utc::now().date_naive() + chrono::Duration::days(10);
    let end = start + chrono::Duration::days(days - 1);
    json!({
        "quantity": 1.0,
        "fulfillment": "pickup",
        "bike": {
            "bike_key": FAMILY,
            "bike_name": "Yamaha NMAX 155",
            "deal": {
                "kind": "bike_rental",
                "rental_start": start.to_string(),
                "rental_end": end.to_string(),
                "rate_thb_day": null,
                "deposit": deposit,
            }
        }
    })
}

async fn orders_for(db: &turbobaby_bot::db::Database, customer_name: &str) -> Vec<String> {
    OrderEntity::find()
        .filter(OrderCol::CustomerName.eq(customer_name.to_string()))
        .all(&db.orm)
        .await
        .expect("orders query")
        .into_iter()
        .map(|o| o.id)
        .collect()
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn create_order_idempotent_replay_returns_same_order_id() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    // Unique customer_name lets us count rows for THIS test run only —
    // multiple `--ignored` invocations against the same DB don't collide.
    let suffix = rand_suffix();
    let customer_name = format!("integration-test-customer-{}", suffix);
    let idem_key = uuid::Uuid::new_v4().to_string();

    let body = json!({
        "telegram_id": null,  // anonymous path — skips check_owner
        "customer_name": customer_name,
        "customer_phone": "+66000000000",
        "customer_telegram": null,
        "items": [rental_line(3, serde_json::Value::Null)],
        "subtotal": 0.0,
        "bonus_used": 0.0,
        "total": 0.0,
        "shop_id": null,
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
    let rows = orders_for(&db, &customer_name).await;
    assert_eq!(
        rows,
        [first_order_id],
        "expected exactly 1 order row for {}",
        customer_name
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

    let customer_name = format!("integration-pickup-customer-{}", rand_suffix());

    let body = json!({
        "telegram_id": null,
        "customer_name": customer_name,
        // Local Thai mobile, exactly as a customer's own phone shows it.
        "customer_phone": "081 234 5678",
        "customer_telegram": null,
        // Collected at the office: the rental line says so.
        "items": [rental_line(1, serde_json::Value::Null)],
        "subtotal": 0.0,
        "bonus_used": 0.0,
        "total": 0.0,
        "shop_id": null,
        "fulfillment": "pickup",
        // No delivery_address at all — this is a collect-in-store order.
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

/// The 20+ box is removed for now (owner, 2026-09-25, «Пока убираем»), so the
/// real endpoint must not refuse an order over `age_confirmed`: absent (what a
/// current client sends), `false`, and `true` (what a cached old client still
/// sends) all go on to the same checks. Each body carries an empty cart, which
/// needs no database row, so the answer is the empty cart's 400 -- where until
/// 2026-09-25 an absent or `false` value was refused first, with 422.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn create_order_does_not_require_age_confirmation() {
    let Some((app, _db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    // One client address per request: anonymous orders are rate-limited per IP.
    for (age, client_ip) in [
        (None, "127.0.0.3"),
        (Some(false), "127.0.0.5"),
        (Some(true), "127.0.0.6"),
    ] {
        let mut body = json!({
            "telegram_id": null,
            "customer_name": format!("integration-age-customer-{}", rand_suffix()),
            "customer_phone": "+66812345678",
            "items": [],
            "subtotal": 100.0,
            "total": 100.0,
        });
        if let Some(age) = age {
            body["age_confirmed"] = json!(age);
        }
        let idem_key = uuid::Uuid::new_v4().to_string();
        let resp = post_order(app.clone(), &idem_key, &body, client_ip).await;
        assert_eq!(
            resp.status,
            StatusCode::BAD_REQUEST,
            "age_confirmed={age:?}: the refusal must be the empty cart's, not an age refusal, \
             body: {}",
            resp.body
        );
    }
}

/// A client that claims money the server did not compute must not get an
/// order: the server holds every line to its own figures and refuses the
/// mismatch before anything is stored.
///
/// On the rental catalogue that is two figures. A rental is quoted at the
/// door, so the cart's own subtotal is 0 and a client that sums the day rate
/// into it (449 × 3 here) is refused; and a rental's money deposit must be the
/// family's published one, so a client that under-reports it (1 000 against
/// the published 3 000) is refused too. Neither leaves an order row.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn create_order_rejects_client_supplied_wrong_total() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    let suffix = rand_suffix();
    let claimed = 449.0 * 3.0;
    let under_reported_deposit = json!({
        "form": "money", "amount": 1000.0, "currency": "THB", "method": "cash THB"
    });
    for (case, items, subtotal, client_ip) in [
        (
            "the day rate summed into the subtotal",
            json!([rental_line(3, serde_json::Value::Null)]),
            claimed,
            "127.0.0.4",
        ),
        (
            "a deposit under the published one",
            json!([rental_line(3, under_reported_deposit)]),
            0.0,
            "127.0.0.7",
        ),
    ] {
        let customer_name = format!("integration-fraud-customer-{suffix}-{client_ip}");
        let body = json!({
            "telegram_id": null,
            "customer_name": customer_name,
            "customer_phone": "+66812345678",
            "items": items,
            "subtotal": subtotal,
            "bonus_used": 0.0,
            "total": subtotal,
        });

        let resp = post_order(
            app.clone(),
            &uuid::Uuid::new_v4().to_string(),
            &body,
            client_ip,
        )
        .await;
        assert_eq!(
            resp.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{case}: the server's figure must win, body: {}",
            resp.body
        );
        assert!(
            orders_for(&db, &customer_name).await.is_empty(),
            "{case}: a refused order was stored"
        );
    }
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
