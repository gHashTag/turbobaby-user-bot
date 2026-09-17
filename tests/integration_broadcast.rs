//! Integration tests for the admin Telegram broadcast endpoint.
//!
//! These two cases used to live in `tests/integration_reviews.rs` alongside the
//! review and lab-certificate suites. Migration 083 dropped `strain_reviews`,
//! `lab_certificates` and `strains`, so those suites tested endpoints that
//! could only answer 500 — they were deleted with the endpoints. The broadcast
//! surface is live and its coverage moved here rather than going down with the
//! file it happened to share.

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sea_orm::ConnectionTrait;
use tower::ServiceExt;

fn admin_headers() -> (String, &'static str, &'static str) {
    (
        common::make_init_data(42, "dummy_test_token"),
        "test_password",
        "42",
    )
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn telegram_broadcast_with_no_users_returns_success() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration broadcast test");
        return;
    };
    let _ = db
        .orm
        .execute_unprepared("TRUNCATE user_languages, loyalty_profiles, orders")
        .await;

    let (admin_init, admin_token, admin_telegram_id) = admin_headers();
    // `broadcast_reply_markup` never touches the database — it only builds a
    // deep link — so the kind here just has to parse. It names `set` rather
    // than the `strain` the original test used: `strains` no longer exists,
    // and a test should not carry the name of a dropped table forward.
    let body = serde_json::json!({
        "text": "Hello Telegram friends",
        "product": { "kind": "set", "id": "set-42" },
        "button_text": "Open"
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/broadcast")
                .method("POST")
                .header("X-Telegram-Init-Data", admin_init)
                .header("X-Admin-Token", admin_token)
                .header("X-Admin-Telegram-Id", admin_telegram_id)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .expect("telegram broadcast");

    // The broadcast handler returns a structured success response; with the
    // dummy test bot every send fails at the Telegram API layer, so sent=0
    // and failed equals the number of unique recipients found in the test DB.
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("valid json");
    assert_eq!(json["success"], true);
    let recipients = json["recipients"].as_u64().expect("recipients count");
    let sent = json["sent"].as_u64().unwrap_or(0);
    let failed = json["failed"].as_u64().unwrap_or(0);
    assert_eq!(recipients, sent + failed);
}

#[tokio::test]
#[ignore]
async fn telegram_broadcast_with_bad_photo_url_rejected() {
    let Some((app, _db)) = common::make_app_with_db().await else {
        return;
    };
    let (admin_init, admin_token, admin_telegram_id) = admin_headers();
    let body = serde_json::json!({
        "text": "Photo broadcast",
        "photo_url": "not-a-url"
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/broadcast")
                .method("POST")
                .header("X-Telegram-Init-Data", admin_init)
                .header("X-Admin-Token", admin_token)
                .header("X-Admin-Telegram-Id", admin_telegram_id)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .expect("telegram broadcast");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
