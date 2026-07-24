//! Integration tests for the frontend error telemetry sink.

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use sea_orm::{ConnectionTrait, Statement};
use tower::ServiceExt;

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn client_errors_accepts_valid_report() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration client_errors test");
        return;
    };

    let _ = db
        .orm
        .execute_unprepared("TRUNCATE client_error_logs")
        .await;

    let body = serde_json::json!({
        "source": "wasm",
        "message": "Importing binding name 'foo' not found",
        "stack": "Error: stack trace\n    at Module.js:1:1",
        "url_path": "/events",
        "user_agent": "Mozilla/5.0 Test",
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/client-errors")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .expect("router.oneshot");

    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // Verify the row was persisted.
    let rows = db
        .orm
        .query_one(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT source, message_hash, message, stack, url_path, user_agent \
             FROM client_error_logs ORDER BY id DESC LIMIT 1"
                .to_string(),
        ))
        .await
        .expect("query row");
    let row = rows.expect("no row written");
    let source = row.try_get::<String>("", "source").expect("source");
    assert_eq!(source, "wasm");
    let message = row.try_get::<String>("", "message").expect("message");
    assert!(message.contains("Importing binding name"));
    let stack = row.try_get::<String>("", "stack").expect("stack");
    assert!(stack.contains("stack trace"));
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn client_errors_rejects_empty_message() {
    let Some((app, _db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration client_errors test");
        return;
    };

    let body = serde_json::json!({
        "source": "wasm",
        "message": "   ",
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/client-errors")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .expect("router.oneshot");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn client_errors_normalizes_unknown_source() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration client_errors test");
        return;
    };

    let _ = db
        .orm
        .execute_unprepared("TRUNCATE client_error_logs")
        .await;

    let body = serde_json::json!({
        "source": "browser_extension",
        "message": "something failed",
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/client-errors")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .expect("router.oneshot");

    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let rows = db
        .orm
        .query_one(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT source FROM client_error_logs ORDER BY id DESC LIMIT 1".to_string(),
        ))
        .await
        .expect("query row");
    let row = rows.expect("no row written");
    let source = row.try_get::<String>("", "source").expect("source");
    assert_eq!(source, "unknown");
}
