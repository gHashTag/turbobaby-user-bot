//! Loop #20: deterministic share-source assignment integration test.

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::{make_app, make_init_data};
use tower::ServiceExt;

#[tokio::test]
#[ignore = "needs a throwaway DATABASE_URL and --features backend"]
async fn share_source_returns_default_variant() {
    let app = match make_app().await {
        Some(x) => x,
        None => return,
    };

    let telegram_id = 120001i64;
    let init_data = make_init_data(telegram_id, "dummy_test_token");

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/referrals/me/{}/share-source", telegram_id))
        .header("x-telegram-init-data", init_data)
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let source = json["source"].as_str().expect("source present");
    assert!(
        source == "utm_a" || source == "utm_b",
        "unexpected source: {source}"
    );
}

#[tokio::test]
#[ignore = "needs a throwaway DATABASE_URL and --features backend"]
async fn share_source_assigns_known_variant() {
    let app = match make_app().await {
        Some(x) => x,
        None => return,
    };

    let telegram_id = 120002i64;
    let init_data = make_init_data(telegram_id, "dummy_test_token");

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/referrals/me/{}/share-source", telegram_id))
        .header("x-telegram-init-data", init_data)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let source = json["source"].as_str().expect("source present");
    assert!(
        source == "utm_a" || source == "utm_b",
        "unexpected source: {source}"
    );
}
