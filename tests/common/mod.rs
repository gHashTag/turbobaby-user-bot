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
use woody_weed_bot::delivery::DeliveryZones;
use woody_weed_bot::promptpay::QrConfig;
use woody_weed_bot::AppState;

/// Build a test `AppState` + `Router` against `DATABASE_URL`. Returns
/// `None` if env var unset — callers should `return` rather than fail.
pub(crate) async fn make_app() -> Option<Router> {
    let (app, _db) = make_app_with_db().await?;
    Some(app)
}

/// Variant that returns the `Database` handle alongside the `Router`
/// so DB-level assertions (e.g. "exactly one ledger row exists")
/// don't need to spin up a second connection. The handle is the
/// SAME `Arc<Database>` plumbed through the `AppState`.
pub(crate) async fn make_app_with_db() -> Option<(Router, Arc<Database>)> {
    let database_url = std::env::var("DATABASE_URL").ok()?;

    // SAFETY GUARD: this harness runs `run_migrations()` and the suite writes
    // rows. If `DATABASE_URL` happens to point at a shared/production database
    // (a real footgun — prod DSNs are commonly exported into the shell), the
    // tests would migrate and mutate production data. Refuse unless the DSN is
    // clearly a local or explicitly test-named database.
    assert!(
        is_safe_test_dsn(&database_url),
        "refusing to run integration tests against DATABASE_URL={database_url:?} — it is not a \
         local/test database. The harness migrates and writes rows; point it at a THROWAWAY DB \
         (host localhost/127.0.0.1, or a database name containing \"test\")."
    );

    let config = Arc::new(test_config(&database_url));
    let db = Database::connect(&database_url)
        .await
        .expect("integration test: Database::connect failed");
    db.run_migrations()
        .await
        .expect("integration test: run_migrations failed");
    let db = Arc::new(db);
    let bot = Arc::new(Bot::new("dummy_test_token"));
    let cache = Arc::new(ETagCache::new());

    let state = AppState {
        db: db.clone(),
        config,
        bot,
        cache,
    };

    Some((woody_weed_bot::api::router(state), db))
}

/// Forge a VALID Telegram `initData` query string for `user_id`, signed with
/// `bot_token`, so owner-gated endpoints (`check_owner`) can be exercised in
/// integration tests. Mirrors `validate_init_data`'s algorithm exactly:
/// `secret = HMAC-SHA256("WebAppData", bot_token)`, then
/// `hash = HMAC-SHA256(secret, <sorted "k=v" lines over DECODED values>)`.
/// Only `auth_date` (freshness) and `user` (identity) are required.
pub(crate) fn make_init_data(user_id: i64, bot_token: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs();
    let user_json = format!("{{\"id\":{user_id},\"first_name\":\"Test\"}}");
    // data_check_string: keys sorted ascending (auth_date < user), DECODED values.
    let data_check_string = format!("auth_date={now}\nuser={user_json}");

    let mut secret = HmacSha256::new_from_slice(b"WebAppData").unwrap();
    secret.update(bot_token.as_bytes());
    let secret_key = secret.finalize().into_bytes();

    let mut mac = HmacSha256::new_from_slice(&secret_key).unwrap();
    mac.update(data_check_string.as_bytes());
    let hash = hex::encode(mac.finalize().into_bytes());

    // URL-encode the user value so JSON punctuation can't break `&`/`=` parsing.
    let user_enc = urlencoding::encode(&user_json);
    format!("user={user_enc}&auth_date={now}&hash={hash}")
}

/// Pure guard: is this DSN safe for a destructive integration-test run?
/// Allows local hosts (localhost/127.0.0.1/::1) or any database whose name
/// contains "test". Blocks remote production-looking DSNs. Pure so it can be
/// unit-tested without a DB.
pub(crate) fn is_safe_test_dsn(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let host_is_local =
        lower.contains("@localhost") || lower.contains("@127.0.0.1") || lower.contains("@[::1]");
    // database name = the path segment after the last '/', ignoring any ?query.
    let db_is_test = lower
        .rsplit('/')
        .next()
        .map(|tail| tail.split('?').next().unwrap_or(tail))
        .map(|db| db.contains("test"))
        .unwrap_or(false);
    host_is_local || db_is_test
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
        delivery_zones: DeliveryZones::default(),
        promptpay: QrConfig::default(),
    }
}
