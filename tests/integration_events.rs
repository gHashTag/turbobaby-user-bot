//! Integration tests for events calendar + booking endpoints.

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sea_orm::ConnectionTrait;
use tower::ServiceExt;

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn list_events_returns_public_events() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration events test");
        return;
    };

    // Ensure clean state for this test.
    let _ = db
        .orm
        .execute_unprepared("TRUNCATE events, event_bookings")
        .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/events")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot");

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    let events = body
        .get("events")
        .and_then(|v| v.as_array())
        .expect("events array");
    assert!(events.is_empty());
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn list_events_with_date_range_returns_ok() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration events test");
        return;
    };

    // Ensure clean state for this test.
    let _ = db
        .orm
        .execute_unprepared("TRUNCATE events, event_bookings")
        .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/events?from=2026-07-20T00:00:00Z&to=2026-07-26T23:59:59Z")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot");

    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    assert_eq!(status, StatusCode::OK);
    let events = body
        .get("events")
        .and_then(|v| v.as_array())
        .expect("events array");
    assert!(events.is_empty());
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn admin_can_create_and_list_event() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration events test");
        return;
    };

    let _ = db
        .orm
        .execute_unprepared("TRUNCATE events, event_bookings")
        .await;

    let init_data = common::make_init_data(42, "dummy_test_token");
    let admin_token = "test_password";

    let create_body = serde_json::json!({
        "title": "Test Event",
        "starts_at": "2026-07-25T18:00:00Z",
        "is_public": true,
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/admin/events")
                .method("POST")
                .header("X-Telegram-Init-Data", &init_data)
                .header("X-Admin-Token", admin_token)
                .header("X-Admin-Telegram-Id", "42")
                .header("content-type", "application/json")
                .body(Body::from(create_body.to_string()))
                .unwrap(),
        )
        .await
        .expect("create event");

    assert_eq!(response.status(), StatusCode::OK, "create event failed");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/events")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("list events");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    let events = body
        .get("events")
        .and_then(|v| v.as_array())
        .expect("events array");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["title"], "Test Event");
}

async fn create_public_event(
    app: axum::Router,
    title: &str,
    starts_at: &str,
    max_seats: i32,
) -> (axum::Router, String) {
    create_public_event_with_stars(app, title, starts_at, max_seats, None).await
}

async fn create_public_event_with_stars(
    app: axum::Router,
    title: &str,
    starts_at: &str,
    max_seats: i32,
    price_stars: Option<i64>,
) -> (axum::Router, String) {
    let init_data = common::make_init_data(42, "dummy_test_token");
    let admin_token = "test_password";
    let mut create_body = serde_json::json!({
        "title": title,
        "starts_at": starts_at,
        "is_public": true,
        "max_seats": max_seats,
    });
    if let Some(s) = price_stars {
        create_body["price_stars"] = serde_json::json!(s);
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/admin/events")
                .method("POST")
                .header("X-Telegram-Init-Data", &init_data)
                .header("X-Admin-Token", admin_token)
                .header("content-type", "application/json")
                .body(Body::from(create_body.to_string()))
                .unwrap(),
        )
        .await
        .expect("create event");
    assert_eq!(response.status(), StatusCode::OK, "create event failed");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    let id = body["id"].as_str().expect("event id").to_string();
    (app, id)
}

async fn book_event(
    app: axum::Router,
    event_id: &str,
    telegram_id: i64,
) -> (axum::Router, StatusCode, serde_json::Value) {
    book_event_with_idempotency(app, event_id, telegram_id, None).await
}

async fn book_event_with_idempotency(
    app: axum::Router,
    event_id: &str,
    telegram_id: i64,
    idempotency_key: Option<&str>,
) -> (axum::Router, StatusCode, serde_json::Value) {
    let init_data = common::make_init_data(telegram_id, "dummy_test_token");
    let body = serde_json::json!({ "telegram_id": telegram_id });
    let mut req = Request::builder()
        .uri(format!("/api/events/{}/book", event_id))
        .method("POST")
        .header("X-Telegram-Init-Data", &init_data)
        .header("content-type", "application/json");
    if let Some(k) = idempotency_key {
        req = req.header("X-Idempotency-Key", k);
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .expect("book event");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    (app, status, body)
}

async fn add_stars(
    app: axum::Router,
    telegram_id: i64,
    amount: i64,
    external_tx_id: &str,
) -> axum::Router {
    let init_data = common::make_init_data(telegram_id, "dummy_test_token");
    let body = serde_json::json!({
        "telegram_id": telegram_id,
        "amount": amount,
        "source": "test",
        "reason": "integration",
        "external_tx_id": external_tx_id,
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/stars/add")
                .method("POST")
                .header("X-Telegram-Init-Data", &init_data)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .expect("add stars");
    assert_eq!(response.status(), StatusCode::OK, "add stars failed");
    app
}

async fn get_stars_balance(app: axum::Router, telegram_id: i64) -> (axum::Router, i64) {
    let init_data = common::make_init_data(telegram_id, "dummy_test_token");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/stars/balance/{}", telegram_id))
                .header("X-Telegram-Init-Data", &init_data)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("get balance");
    assert_eq!(response.status(), StatusCode::OK, "get balance failed");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    let balance = body["balance"].as_i64().unwrap_or(0);
    (app, balance)
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn user_can_list_and_cancel_own_booking() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration events test");
        return;
    };
    let _ = db
        .orm
        .execute_unprepared("TRUNCATE events, event_bookings")
        .await;

    let (app, event_id) = create_public_event(app, "Bookable", "2030-07-25T18:00:00Z", 10).await;
    let user_id: i64 = 1001;

    let (app, status, _) = book_event(app, &event_id, user_id).await;
    assert_eq!(status, StatusCode::OK, "booking should succeed");

    let init_data = common::make_init_data(user_id, "dummy_test_token");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/events/my-bookings?telegram_id={}", user_id))
                .header("X-Telegram-Init-Data", &init_data)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("my bookings");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    let bookings = body["bookings"].as_array().expect("bookings array");
    assert_eq!(bookings.len(), 1);
    let booking_id = bookings[0]["id"].as_str().expect("booking id").to_string();
    assert_eq!(bookings[0]["event_title"], "Bookable");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/events/bookings/{}/cancel?telegram_id={}",
                    booking_id, user_id
                ))
                .method("PUT")
                .header("X-Telegram-Init-Data", &init_data)
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .expect("cancel");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn admin_can_cancel_user_booking() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration events test");
        return;
    };
    let _ = db
        .orm
        .execute_unprepared("TRUNCATE events, event_bookings")
        .await;

    let (app, event_id) =
        create_public_event(app, "Admin Cancel", "2030-07-25T18:00:00Z", 10).await;
    let user_id: i64 = 1002;
    let (app, status, book_body) = book_event(app, &event_id, user_id).await;
    assert_eq!(status, StatusCode::OK);
    let booking_id = book_body["booking_id"]
        .as_str()
        .expect("booking id")
        .to_string();

    let admin_init = common::make_init_data(42, "dummy_test_token");
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/admin/events/{}/bookings/{}/cancel",
                    event_id, booking_id
                ))
                .method("PUT")
                .header("X-Telegram-Init-Data", &admin_init)
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .expect("admin cancel");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn waitlist_opens_when_capacity_full() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration events test");
        return;
    };
    let _ = db
        .orm
        .execute_unprepared("TRUNCATE events, event_bookings")
        .await;

    let (app, event_id) = create_public_event(app, "Full House", "2030-07-25T18:00:00Z", 2).await;

    let (app, status, _) = book_event(app, &event_id, 2001).await;
    assert_eq!(status, StatusCode::OK);
    let (app, status, _) = book_event(app, &event_id, 2002).await;
    assert_eq!(status, StatusCode::OK);

    // Third user is waitlisted.
    let waiter_id: i64 = 2003;
    let init_data = common::make_init_data(waiter_id, "dummy_test_token");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/events/{}/waitlist", event_id))
                .method("POST")
                .header("X-Telegram-Init-Data", &init_data)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "telegram_id": waiter_id }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("waitlist");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    assert_eq!(body["status"], "waitlisted");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn waitlist_auto_promotes_on_cancel() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration events test");
        return;
    };
    let _ = db
        .orm
        .execute_unprepared("TRUNCATE events, event_bookings")
        .await;

    let (app, event_id) = create_public_event(app, "Promote Demo", "2030-07-25T18:00:00Z", 2).await;

    let (app, _, book1) = book_event(app, &event_id, 2101).await;
    let booking1_id = book1["booking_id"]
        .as_str()
        .expect("booking id")
        .to_string();
    let (app, _, _) = book_event(app, &event_id, 2102).await;

    // Third user joins the waitlist.
    let waiter_id: i64 = 2103;
    let waiter_init = common::make_init_data(waiter_id, "dummy_test_token");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/events/{}/waitlist", event_id))
                .method("POST")
                .header("X-Telegram-Init-Data", &waiter_init)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "telegram_id": waiter_id }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("waitlist");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let wait_body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    assert_eq!(wait_body["status"], "waitlisted");
    let wait_booking_id = wait_body["booking_id"]
        .as_str()
        .expect("waitlist booking id")
        .to_string();

    // Cancel the first confirmed booking — the waitlist entry should be promoted.
    let cancel_init = common::make_init_data(2101, "dummy_test_token");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/events/bookings/{}/cancel?telegram_id={}",
                    booking1_id, 2101
                ))
                .method("PUT")
                .header("X-Telegram-Init-Data", &cancel_init)
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .expect("cancel");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let cancel_body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    assert_eq!(
        cancel_body["promoted_booking_id"].as_str(),
        Some(wait_booking_id.as_str()),
        "cancel should report the promoted waitlist booking"
    );

    // The waitlisted user is now confirmed.
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/events/my-bookings?telegram_id={}", waiter_id))
                .header("X-Telegram-Init-Data", waiter_init)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("my bookings");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    let bookings = body["bookings"].as_array().expect("bookings array");
    assert_eq!(bookings.len(), 1);
    assert_eq!(bookings[0]["id"], wait_booking_id);
    assert_eq!(bookings[0]["status"], "confirmed");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn timezone_edge_case_lists_bangkok_midnight_range() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration events test");
        return;
    };
    let _ = db
        .orm
        .execute_unprepared("TRUNCATE events, event_bookings")
        .await;

    // Event at 01:00 Asia/Bangkok = 18:00 UTC the previous day.
    let (app, _event_id) = create_public_event(app, "Late Night", "2030-08-10T18:00:00Z", 10).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/events?from=2030-08-11T00:00:00%2B07:00&to=2030-08-11T23:59:59%2B07:00")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("list events with Bangkok range");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    let events = body["events"].as_array().expect("events array");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["title"], "Late Night");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn paid_event_booking_deducts_stars() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration events test");
        return;
    };
    db.orm
        .execute_unprepared("TRUNCATE events, event_bookings, user_stars, stars_transactions, stars_idempotency_keys, loyalty_idempotency_keys, loyalty_profiles")
        .await
        .expect("truncate test tables");

    let user_id: i64 = 3001;
    let idem_key = "paid_event_test_001";
    let (app, event_id) =
        create_public_event_with_stars(app, "Paid Workshop", "2030-07-25T18:00:00Z", 10, Some(50))
            .await;

    let app = add_stars(app, user_id, 50, "credit_paid_event_001").await;
    let (app, balance_before) = get_stars_balance(app, user_id).await;
    assert_eq!(balance_before, 50);

    let (app, status, book_body) =
        book_event_with_idempotency(app, &event_id, user_id, Some(idem_key)).await;
    assert_eq!(status, StatusCode::OK, "paid booking should succeed");
    let booking_id = book_body["booking_id"]
        .as_str()
        .expect("booking id")
        .to_string();

    let (app, balance_after) = get_stars_balance(app, user_id).await;
    assert_eq!(balance_after, 0, "stars should be deducted");

    // Idempotency replay must not double-charge.
    let (app, replay_status, replay_body) =
        book_event_with_idempotency(app, &event_id, user_id, Some(idem_key)).await;
    assert_eq!(
        replay_status,
        StatusCode::OK,
        "replay should return cached booking"
    );
    assert_eq!(
        replay_body["booking_id"].as_str(),
        Some(booking_id.as_str()),
        "replay should return same booking id"
    );
    let (_, balance_replay) = get_stars_balance(app.clone(), user_id).await;
    assert_eq!(balance_replay, 0, "replay should not deduct more stars");

    // Booking row records the payment.
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/events/my-bookings?telegram_id={}", user_id))
                .header(
                    "X-Telegram-Init-Data",
                    common::make_init_data(user_id, "dummy_test_token"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("my bookings");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    let bookings = body["bookings"].as_array().expect("bookings array");
    assert_eq!(bookings.len(), 1);
    assert_eq!(bookings[0]["stars_paid"], 50);
    assert!(
        bookings[0]["stars_tx_id"]
            .as_str()
            .map_or(false, |s| !s.is_empty()),
        "stars_tx_id should be set"
    );

    // Ledger row exists.
    let rows = db
        .orm
        .query_all(sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            "SELECT amount, related_order_id FROM stars_transactions WHERE telegram_id = $1 AND reason = 'event_booking'",
            [user_id.into()],
        ))
        .await
        .expect("ledger select");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].try_get::<i64>("", "amount").unwrap_or(0), -50);
    assert_eq!(
        rows[0]
            .try_get::<Option<String>>("", "related_order_id")
            .ok()
            .flatten()
            .as_deref(),
        Some(booking_id.as_str())
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn paid_event_booking_fails_without_stars() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration events test");
        return;
    };
    db.orm
        .execute_unprepared("TRUNCATE events, event_bookings, user_stars, stars_transactions, stars_idempotency_keys")
        .await
        .expect("truncate test tables");

    let user_id: i64 = 3002;
    let (app, event_id) = create_public_event_with_stars(
        app,
        "Expensive Workshop",
        "2030-07-25T18:00:00Z",
        10,
        Some(100),
    )
    .await;

    let (_, status, _) = book_event(app.clone(), &event_id, user_id).await;
    assert_eq!(
        status,
        StatusCode::PAYMENT_REQUIRED,
        "booking without stars should fail"
    );

    let (_, balance) = get_stars_balance(app, user_id).await;
    assert_eq!(balance, 0);
}
