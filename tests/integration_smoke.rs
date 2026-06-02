//! Integration smoke test — does the Axum `Router` boot and serve a
//! basic request end-to-end? (cycle #162)
//!
//! Marked `#[ignore]` so the default `cargo test` run stays hermetic.
//! Invoke with:
//!
//! ```sh
//! DATABASE_URL=postgres://... cargo test --features backend -- --ignored
//! ```
//!
//! Without `DATABASE_URL` the harness returns `None` and the test
//! cleanly skips (prints a notice instead of failing).
//!
//! Cycle #162 unblocked this — `src/lib.rs` now exposes the backend
//! modules under `#[cfg(all(not(target_arch = "wasm32"), feature =
//! "backend"))]`, so `tests/*.rs` integration test binaries can reach
//! `woody_weed_bot::api::router`, `woody_weed_bot::AppState`, etc.

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt; // for `.oneshot()`

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn smoke_ping_returns_ok() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping integration smoke test");
        return;
    };

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/ping")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router.oneshot");

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "woody-weed-bot");
}
