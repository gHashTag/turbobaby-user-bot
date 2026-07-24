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
    let _ = db.orm.execute_unprepared("TRUNCATE events, event_bookings").await;

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
    let events = body.get("events").and_then(|v| v.as_array()).expect("events array");
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
    let _ = db.orm.execute_unprepared("TRUNCATE events, event_bookings").await;

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
    let events = body.get("events").and_then(|v| v.as_array()).expect("events array");
    assert!(events.is_empty());
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn admin_can_create_and_list_event() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration events test");
        return;
    };

    let _ = db.orm.execute_unprepared("TRUNCATE events, event_bookings").await;

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
    let events = body.get("events").and_then(|v| v.as_array()).expect("events array");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["title"], "Test Event");
}

async fn create_public_event(
    app: axum::Router,
    title: &str,
    starts_at: &str,
    max_seats: i32,
) -> (axum::Router, String) {
    let init_data = common::make_init_data(42, "dummy_test_token");
    let admin_token = "test_password";
    let create_body = serde_json::json!({
        "title": title,
        "starts_at": starts_at,
        "is_public": true,
        "max_seats": max_seats,
    });
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
    let init_data = common::make_init_data(telegram_id, "dummy_test_token");
    let body = serde_json::json!({ "telegram_id": telegram_id });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/events/{}/book", event_id))
                .method("POST")
                .header("X-Telegram-Init-Data", &init_data)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .expect("book event");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    (app, status, body)
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn user_can_list_and_cancel_own_booking() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration events test");
        return;
    };
    let _ = db.orm.execute_unprepared("TRUNCATE events, event_bookings").await;

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
    let _ = db.orm.execute_unprepared("TRUNCATE events, event_bookings").await;

    let (app, event_id) = create_public_event(app, "Admin Cancel", "2030-07-25T18:00:00Z", 10).await;
    let user_id: i64 = 1002;
    let (app, status, book_body) = book_event(app, &event_id, user_id).await;
    assert_eq!(status, StatusCode::OK);
    let booking_id = book_body["booking_id"].as_str().expect("booking id").to_string();

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
    let _ = db.orm.execute_unprepared("TRUNCATE events, event_bookings").await;

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
                .body(Body::from(serde_json::json!({ "telegram_id": waiter_id }).to_string()))
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
async fn timezone_edge_case_lists_bangkok_midnight_range() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration events test");
        return;
    };
    let _ = db.orm.execute_unprepared("TRUNCATE events, event_bookings").await;

    // Event at 01:00 Asia/Bangkok = 18:00 UTC the previous day.
    let (app, _event_id) =
        create_public_event(app, "Late Night", "2030-08-10T18:00:00Z", 10).await;

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
