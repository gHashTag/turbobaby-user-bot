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
