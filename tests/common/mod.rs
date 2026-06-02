//! Shared integration-test harness (cycle #162, after the #161
//! abandonment + #162 lib.rs restructure unblocked it).
//!
//! Spins up the full Axum `Router` against a real `AppState`. Tests opt
//! in via `#[ignore]` so the default `cargo test` run stays hermetic;
//! the integration suite is invoked with:
//!
//! ```sh
//! DATABASE_URL=postgres://... cargo test --features backend -- --ignored
//! ```
//!
//! Without `DATABASE_URL` set, `make_app()` returns `None` and the
//! ignored tests skip cleanly. Runtime guard rather than compile-time
//! `cfg` so running on a CI container without live PG is "not
//! exercised" rather than a build failure.
//!
//! Harness construction order:
//!   1. `Config` — minimal, all required fields stubbed.
//!   2. `Database::connect(database_url)` — real PG handshake; runs
//!      the migration chain to keep the test schema current.
//!   3. `Bot::new("dummy_test_token")` — Teloxide accepts any string
//!      at construction; tests that don't call `bot.send_*` work fine.
//!   4. `ETagCache::new()` — pure in-memory.

#![cfg(feature = "backend")]
#![allow(dead_code)] // Helpers exposed for tests that may not all exist yet.

use std::sync::Arc;

use axum::Router;
use teloxide::Bot;

use woody_weed_bot::api::cache::ETagCache;
use woody_weed_bot::config::Config;
use woody_weed_bot::db::Database;
use woody_weed_bot::AppState;

/// Build a test `AppState` + `Router` against `DATABASE_URL`. Returns
/// `None` if env var unset — callers should `return` rather than fail.
pub async fn make_app() -> Option<Router> {
    let database_url = std::env::var("DATABASE_URL").ok()?;

    let config = Arc::new(test_config(&database_url));
    let db = Database::connect(&database_url)
        .await
        .expect("integration test: Database::connect failed");
    db.run_migrations()
        .await
        .expect("integration test: run_migrations failed");
    let bot = Arc::new(Bot::new("dummy_test_token"));
    let cache = Arc::new(ETagCache::new());

    let state = AppState {
        db: Arc::new(db),
        config,
        bot,
        cache,
    };

    Some(woody_weed_bot::api::router(state))
}

/// Minimal `Config` for tests. All required fields populated; secrets
/// and external integrations stubbed so no test triggers a network
/// call under the default exercise path.
fn test_config(database_url: &str) -> Config {
    Config {
        bot_token: "dummy_test_token".into(),
        web_app_url: "http://localhost".into(),
        port: 0,
        webhook_path: "/webhook".into(),
        app_url: "http://localhost".into(),
        is_production: false,
        admin_ids: vec![42],
        bot_username: "test_bot".into(),
        database_url: database_url.to_string(),
        grok_api_key: "".into(),
        glm_api_key: "".into(),
        s3_bucket: None,
        s3_endpoint: None,
        s3_internal_endpoint: None,
        s3_public_url: None,
        s3_region: None,
        s3_access_key: None,
        s3_secret_key: None,
        backup_assets: false,
        admin_password: Some("test_password".into()),
        hide_marketing_badges: false,
    }
}
