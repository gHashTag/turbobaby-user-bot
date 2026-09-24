//! Integration tests for event media flow: gallery photos + video URL.
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
use serde_json::json;
use tower::ServiceExt;

use turbobaby_bot::api::auth::generate_admin_token;

const BOT_TOKEN: &str = "dummy_test_token";
const ADMIN_PASSWORD: &str = "test_password";

fn starts_at_future() -> String {
    let dt = chrono::Utc::now() + chrono::Duration::hours(48);
    dt.to_rfc3339()
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn create_event_with_photos_and_video_round_trips() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    let admin_token = generate_admin_token(ADMIN_PASSWORD, BOT_TOKEN);
    let starts_at = starts_at_future();

    let body = json!({
        "title": "Media event",
        "starts_at": starts_at,
        "max_seats": 10,
        "price_stars": 50,
        "video_url": "https://example.com/video.mp4",
        "photos": [
            "https://example.com/photo-a.jpg",
            "https://example.com/photo-b.jpg"
        ],
    });

    let create_resp = post_admin_events(app.clone(), &admin_token, &body).await;
    assert_eq!(
        create_resp.status,
        StatusCode::OK,
        "create event must succeed"
    );
    assert_eq!(create_resp.body["success"], true);
    let event_id = create_resp.body["id"]
        .as_str()
        .expect("event id")
        .to_string();

    // Events left every customer surface on the owner's ruling of 2026-09-24
    // (rental only): the admin API stores no event as public, so the public
    // detail does not answer this one. The gallery round-trips through the
    // admin detail, which reads the same rows.
    let detail = get_event_public(app.clone(), &event_id).await;
    assert_eq!(
        detail.status,
        StatusCode::NOT_FOUND,
        "a new event reached the public detail"
    );

    let admin_detail = get_event_admin(app.clone(), &event_id, &admin_token).await;
    assert_eq!(
        admin_detail.status,
        StatusCode::OK,
        "admin detail must succeed"
    );
    let ev = &admin_detail.body["event"];
    assert_eq!(ev["video_url"], "https://example.com/video.mp4");
    let photos = ev["photos"].as_array().expect("photos array");
    assert_eq!(photos.len(), 2);
    assert!(photos
        .iter()
        .any(|p| p == "https://example.com/photo-a.jpg"));
    assert!(photos
        .iter()
        .any(|p| p == "https://example.com/photo-b.jpg"));
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn create_event_with_invalid_photo_url_rolls_back() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    let admin_token = generate_admin_token(ADMIN_PASSWORD, BOT_TOKEN);
    let starts_at = starts_at_future();

    let body = json!({
        "title": "Bad photo event",
        "starts_at": starts_at,
        "max_seats": 10,
        "photos": ["https://example.com/ok.jpg", "javascript:alert(1)"],
    });

    let create_resp = post_admin_events(app.clone(), &admin_token, &body).await;
    assert_eq!(
        create_resp.status,
        StatusCode::BAD_REQUEST,
        "invalid photo URL must fail"
    );
    assert!(create_resp.body["error"].is_string() || create_resp.body["success"].is_null());

    let list_resp = list_events_admin(app, &admin_token).await;
    assert_eq!(list_resp.status, StatusCode::OK);
    let titles: Vec<String> = list_resp.body["events"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|e| e["title"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        !titles.contains(&"Bad photo event".to_string()),
        "rolled-back event must not appear in admin list"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn update_event_replaces_photos() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping integration test");
        return;
    };

    let admin_token = generate_admin_token(ADMIN_PASSWORD, BOT_TOKEN);
    let starts_at = starts_at_future();

    let create_body = json!({
        "title": "Replace photos event",
        "starts_at": starts_at,
        "max_seats": 10,
        "photos": ["https://example.com/old1.jpg", "https://example.com/old2.jpg"],
    });

    let create_resp = post_admin_events(app.clone(), &admin_token, &create_body).await;
    assert_eq!(create_resp.status, StatusCode::OK);
    let event_id = create_resp.body["id"].as_str().unwrap().to_string();

    let update_body = json!({
        "photos": ["https://example.com/new1.jpg"],
    });
    let update_resp = put_admin_event(app.clone(), &event_id, &admin_token, &update_body).await;
    assert_eq!(update_resp.status, StatusCode::OK);

    // Read back through the admin detail: the public one answers no event the
    // admin API created since 2026-09-24 (see the first test in this file).
    let detail = get_event_admin(app, &event_id, &admin_token).await;
    assert_eq!(detail.status, StatusCode::OK, "admin detail must succeed");
    let photos = detail.body["event"]["photos"].as_array().unwrap();
    assert_eq!(photos.len(), 1);
    assert_eq!(photos[0], "https://example.com/new1.jpg");
}

async fn post_admin_events(
    app: axum::Router,
    admin_token: &str,
    body: &serde_json::Value,
) -> JsonResponse {
    send_json(app, "POST", "/api/admin/events", Some(admin_token), body).await
}

async fn put_admin_event(
    app: axum::Router,
    event_id: &str,
    admin_token: &str,
    body: &serde_json::Value,
) -> JsonResponse {
    send_json(
        app,
        "PUT",
        &format!("/api/admin/events/{}", event_id),
        Some(admin_token),
        body,
    )
    .await
}

async fn get_event_public(app: axum::Router, event_id: &str) -> JsonResponse {
    send_json(
        app,
        "GET",
        &format!("/api/events/{}", event_id),
        None,
        &json!({}),
    )
    .await
}

async fn get_event_admin(app: axum::Router, event_id: &str, admin_token: &str) -> JsonResponse {
    send_json(
        app,
        "GET",
        &format!("/api/admin/events/{}", event_id),
        Some(admin_token),
        &json!({}),
    )
    .await
}

async fn list_events_admin(app: axum::Router, admin_token: &str) -> JsonResponse {
    send_json(
        app,
        "GET",
        "/api/admin/events",
        Some(admin_token),
        &json!({}),
    )
    .await
}

async fn send_json(
    app: axum::Router,
    method: &str,
    uri: &str,
    admin_token: Option<&str>,
    body: &serde_json::Value,
) -> JsonResponse {
    let mut builder = Request::builder().method(method).uri(uri);
    if method != "GET" {
        builder = builder.header("content-type", "application/json");
    }
    if let Some(token) = admin_token {
        builder = builder.header("x-admin-token", token);
    }

    let body_bytes = if method == "GET" {
        Body::empty()
    } else {
        Body::from(serde_json::to_vec(body).unwrap())
    };

    let response = app
        .oneshot(builder.body(body_bytes).unwrap())
        .await
        .expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    JsonResponse { status, body }
}

struct JsonResponse {
    status: StatusCode,
    body: serde_json::Value,
}
