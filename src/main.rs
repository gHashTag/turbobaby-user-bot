// Backend-only modules (not included in WASM builds)
#[cfg(not(target_arch = "wasm32"))]
mod ai;
#[cfg(not(target_arch = "wasm32"))]
mod api;
#[cfg(not(target_arch = "wasm32"))]
mod bot;
#[cfg(not(target_arch = "wasm32"))]
mod config;
#[cfg(not(target_arch = "wasm32"))]
mod db;
#[cfg(not(target_arch = "wasm32"))]
mod locales;
#[cfg(not(target_arch = "wasm32"))]
pub mod metrics;
#[cfg(not(target_arch = "wasm32"))]
pub mod notify;
#[cfg(not(target_arch = "wasm32"))]
mod s3;
#[cfg(not(target_arch = "wasm32"))]
pub mod util;

// Business logic modules (shared between backend and web)
pub mod trios;

// UI module (WASM only — uses web-sys which doesn't compile for native)
#[cfg(target_arch = "wasm32")]
pub mod ui;

#[cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
#[cfg(not(target_arch = "wasm32"))]
use axum::http::Request;
#[cfg(not(target_arch = "wasm32"))]
// use tower_http::compression::CompressionLayer;
#[cfg(not(target_arch = "wasm32"))]
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
#[cfg(not(target_arch = "wasm32"))]
use axum::middleware::Next;
#[cfg(not(target_arch = "wasm32"))]
use axum::response::IntoResponse;
#[cfg(not(target_arch = "wasm32"))]
use axum::{routing::get, Router};
use bytes::Bytes;
#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use teloxide::dispatching::Dispatcher;
#[cfg(not(target_arch = "wasm32"))]
use teloxide::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use tower_http::cors::{Any, CorsLayer};
#[cfg(not(target_arch = "wasm32"))]
use tower_http::services::ServeDir;
#[cfg(not(target_arch = "wasm32"))]
use tower_http::set_header::SetResponseHeaderLayer;
#[cfg(not(target_arch = "wasm32"))]
use tracing::info;

#[cfg(not(target_arch = "wasm32"))]
use crate::api::cache::ETagCache;
#[cfg(not(target_arch = "wasm32"))]
use crate::config::Config;
#[cfg(not(target_arch = "wasm32"))]
use crate::db::Database;
#[cfg(not(target_arch = "wasm32"))]
use axum_prometheus::PrometheusMetricLayer;
#[cfg(not(target_arch = "wasm32"))]
use teloxide::Bot;

#[cfg(not(target_arch = "wasm32"))]
/// Application shared state
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub config: Arc<Config>,
    pub bot: Arc<Bot>,
    pub cache: Arc<ETagCache>,
}

#[cfg(not(target_arch = "wasm32"))]
struct CachedFile {
    raw: Bytes,
    gzip: Option<Bytes>,
    br: Option<Bytes>,
    content_type: &'static str,
}

/// Global rate-limit for 5xx admin alerts (one per minute) to prevent DoS amplification.
#[cfg(not(target_arch = "wasm32"))]
static LAST_5XX_ALERT: std::sync::LazyLock<tokio::sync::Mutex<Option<std::time::Instant>>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(None));

#[cfg(not(target_arch = "wasm32"))]
async fn alert_5xx_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> axum::response::Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let resp = next.run(req).await;
    if resp.status().is_server_error() {
        let should_alert = {
            let mut last = LAST_5XX_ALERT.lock().await;
            let now = std::time::Instant::now();
            if let Some(t) = *last {
                if now.saturating_duration_since(t).as_secs() < 60 {
                    false
                } else {
                    *last = Some(now);
                    true
                }
            } else {
                *last = Some(now);
                true
            }
        };
        if should_alert {
            let bot = state.bot.clone();
            let config = state.config.clone();
            let status = resp.status().as_u16();
            let safe_method: String = method.chars().take(20).collect();
            let safe_path: String = path.chars().take(200).collect();
            tokio::spawn(async move {
                // Cycle #149: notify_admins sends with parse_mode=Html now.
                // `safe_path` is request-URI-derived; could contain `<` or
                // `&` (path traversal probes, malformed URIs). Escape.
                let text = format!(
                    "\u{1F6A8} 5xx Error on prod\n\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\u{1F4CD} {} {}\n\u{1F4A5} HTTP {}",
                    crate::util::html_escape(&safe_method),
                    crate::util::html_escape(&safe_path),
                    status
                );
                crate::notify::notify_admins(&bot, &config, &text).await;
            });
        }
    }
    resp
}

#[cfg(not(target_arch = "wasm32"))]
fn content_type_for(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        Some("html") => "text/html",
        Some("js") => "text/javascript",
        Some("css") => "text/css",
        Some("wasm") => "application/wasm",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        _ => "application/octet-stream",
    }
}

fn pick_encoding(headers: &axum::http::HeaderMap) -> Option<&'static str> {
    let accept = headers
        .get(axum::http::header::ACCEPT_ENCODING)?
        .to_str()
        .ok()?;
    for part in accept.split(',') {
        let part = part.trim();
        if part == "br" {
            return Some("br");
        }
        if part == "gzip" {
            return Some("gzip");
        }
    }
    None
}

/// Cycle #124: shared loop body for daily-ish DB retention sweeps.
///
/// Pre-#124 there were three near-identical `tokio::spawn` blocks in
/// `main` for `cleanup_old_idempotency_keys` (#63), `cleanup_old_fraud_events`
/// (#63), and `cleanup_old_block_history` (#66). Each clone the ORM
/// handle, build a tokio interval, skip the first tick (so cold-start
/// serialisation isn't blocked behind a DELETE), then loop on the
/// cleanup with the same Ok-0 / Ok-N / Err logging.
///
/// All three cleanup helpers share the signature
/// `async fn(&DatabaseConnection, u32) -> Result<u64, DbErr>`, so the
/// helper takes a closure that re-binds the retention constant per
/// callsite. The closure receives an owned `DatabaseConnection`
/// (cheap — it's an `Arc`-of-pool under the hood) so the body can
/// `.await` without borrowing across yield points.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_ttl_sweep<F, Fut>(
    orm: sea_orm::DatabaseConnection,
    label: &'static str,
    interval_secs: u64,
    mut cleanup: F,
) where
    F: FnMut(sea_orm::DatabaseConnection) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<u64, sea_orm::DbErr>> + Send,
{
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        interval.tick().await; // discard the immediate first tick
        loop {
            interval.tick().await;
            match cleanup(orm.clone()).await {
                Ok(0) => {}
                Ok(deleted) => {
                    info!(deleted, "{}: TTL sweep removed expired rows", label);
                }
                Err(e) => {
                    tracing::warn!("{}: TTL sweep failed: {}", label, e);
                }
            }
        }
    });
}

/// Cycle #121: detect whether we're running in a production-shaped
/// environment, *before* the tracing subscriber is initialised. Mirror
/// the logic in `Config::from_env` (which sets `is_production` the
/// same way) since this check has to run earlier — before `Config`
/// is parsed.
///
/// "Production" here is purely a logging-format switch: JSON output
/// for prod (aggregator-friendly: Railway / Datadog / Grafana Loki
/// can structure each line), human-readable text for everything else.
///
/// Takes the two env values directly so the helper is testable
/// without touching the process env.
#[cfg(not(target_arch = "wasm32"))]
fn is_production_env(node_env: Option<&str>, railway_env: Option<&str>) -> bool {
    node_env == Some("production") || railway_env.is_some()
}

/// Cycle #120: resolve the tracing-subscriber filter from `RUST_LOG`,
/// defaulting to `info` when the env var is absent.
///
/// The pre-#120 code (`EnvFilter::from_default_env()`) silently emitted
/// an OFF-level filter when `RUST_LOG` was unset — *every* log dropped,
/// including the startup banner and fraud-event warnings. Containers
/// in production rarely have `RUST_LOG` set explicitly (Railway, Docker,
/// k8s), so this was a black-hole logging mode hiding in plain sight.
///
/// Behaviour:
///   * `Some(s)` with valid directives → use them as-is.
///   * `Some(s)` malformed             → fall back to `"info"` (lossy).
///   * `None` (env unset)              → `"info"`.
///
/// Take `Option<String>` so the helper is unit-testable without touching
/// the process env.
#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::print_stderr)] // Tracing subscriber isn't built yet at this callsite; eprintln is the only way to surface a malformed RUST_LOG.
fn resolve_log_filter(raw: Option<String>) -> tracing_subscriber::EnvFilter {
    use tracing_subscriber::EnvFilter;
    const DEFAULT: &str = "info";
    match raw.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => EnvFilter::try_new(s).unwrap_or_else(|e| {
            // Can't log yet — the subscriber is what we're building.
            // eprintln so the malformed directive doesn't get swallowed.
            eprintln!(
                "warn: RUST_LOG directive {:?} is malformed ({}); using default {:?}",
                s, e, DEFAULT
            );
            EnvFilter::new(DEFAULT)
        }),
        None => EnvFilter::new(DEFAULT),
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
#[allow(clippy::expect_used)] // Boot-time initialisation: rustls provider install must succeed or the process can't serve any TLS.
async fn main() -> Result<()> {
    // Must be called before ANY rustls usage
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Cycle #121: pick log format based on env BEFORE the subscriber is
    // built. `Config::from_env` resolves the same `is_production` flag
    // later, but the subscriber must exist by then to capture its logs.
    // Duplicating the read (rather than threading state) is the lesser
    // evil; the helper is unit-tested so the two paths can't drift.
    let node_env = std::env::var("NODE_ENV").ok();
    let railway_env = std::env::var("RAILWAY_ENVIRONMENT").ok();
    let env_filter = resolve_log_filter(std::env::var("RUST_LOG").ok());
    if is_production_env(node_env.as_deref(), railway_env.as_deref()) {
        // JSON: one object per event, parseable by Loki / Datadog /
        // Railway's log viewer. No ANSI colours, no human formatting.
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .json()
            .init();
    } else {
        // Human-readable: ANSI colours + indented spans. The default
        // for `cargo run` and local dev.
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }

    info!("🤖 Starting Woody Bot (Rust)...");

    let config = Arc::new(Config::from_env()?);
    info!(
        "Environment: {}",
        if config.is_production {
            "Production"
        } else {
            "Development"
        }
    );
    info!(
        "Token present: {}",
        if !config.bot_token.is_empty() {
            "YES"
        } else {
            "NO"
        }
    );
    info!(
        "Admin password set: {}",
        if config.admin_password.is_some() {
            "YES"
        } else {
            "NO"
        }
    );
    info!("Admin IDs: {:?}", config.admin_ids);
    info!("Web App URL: {}", config.web_app_url);

    let db = Arc::new(Database::connect(&config.database_url).await?);
    db.run_migrations().await?;
    // Best-effort schema self-check: surface a prod DB that's behind on
    // migrations BY NAME at startup, instead of waiting for the first 500
    // (cf. the recurring GET /api/sets incident). Non-fatal.
    let missing_cols = db.missing_critical_columns().await;
    // Publish as a Prometheus gauge so the existing monitoring (same path as
    // the 5xx alerts) can fire on `schema_missing_columns > 0` — not just a
    // log line someone has to be looking at. Set even when 0 so the series
    // exists and "recovered" is observable.
    crate::metrics::schema_missing_columns(missing_cols.len() as u64);
    if missing_cols.is_empty() {
        info!("✅ Database connected");
    } else {
        tracing::error!(
            "🚨 SCHEMA SELF-CHECK: live DB is missing {} expected column(s) — \
             catalog endpoints will degrade/500 until migrations are applied: {:?}",
            missing_cols.len(),
            missing_cols
        );
        info!("✅ Database connected (with schema warnings above)");
    }

    let ai_client = Arc::new(crate::ai::AiClient::new(
        config.grok_api_key.clone(),
        config.glm_api_key.clone(),
    ));

    let bot = Bot::new(&config.bot_token);
    let bot_arc = Arc::new(bot.clone());
    let bot_arc_for_state = bot_arc.clone();
    let db_for_bot = db.clone();
    let config_for_bot = config.clone();
    let ai_client_for_bot = ai_client.clone();

    // Background TTL sweep for `order_idempotency_keys` (cycle #58 / A).
    // The table is append-only inside create_order (migration 029, cycle
    // #57); without periodic cleanup it grows unbounded. 24 h covers any
    // realistic client retry window — Stripe defaults to 24 h for the same
    // reason. The first tick is skipped so cold-start serialization isn't
    // blocked behind a DELETE.
    //
    // Cycle #124: all three sweeps go through `spawn_ttl_sweep`. Retention
    // constants are pinned per-callsite so a future "let's keep idempotency
    // keys for a week" tweak only touches the relevant closure.
    spawn_ttl_sweep(db.orm.clone(), "idempotency_keys", 3600, |orm| async move {
        crate::db::orders::cleanup_old_idempotency_keys(&orm, 24).await
    });

    // Background TTL sweep for `order_fraud_events` (cycle #63 / A).
    // Append-only since cycle #59. 30-day retention — admin /engage panel
    // shows a 24h window but trend reviews ("how many subtotal mismatches
    // this month?") look back further. Daily tick is enough; the table is
    // low-cardinality compared to idempotency keys.
    spawn_ttl_sweep(db.orm.clone(), "fraud_events", 86_400, |orm| async move {
        crate::db::orders::cleanup_old_fraud_events(&orm, 30).await
    });

    // Background TTL sweep for `block_history` (cycle #66). Append-only
    // since cycle #64. 90-day retention — block decisions are operational
    // / compliance records that admins genuinely look back at across
    // quarters ("did we wrongly block user X three months ago?"). Daily
    // tick same as the fraud sweep; both tables are tiny vs idempotency.
    spawn_ttl_sweep(db.orm.clone(), "block_history", 86_400, |orm| async move {
        crate::db::orders::cleanup_old_block_history(&orm, 90).await
    });

    tokio::spawn(async move {
        use teloxide::types::AllowedUpdate;
        use teloxide::update_listeners::Polling;
        let handler = bot::create_handler();
        // Явно запрашиваем все нужные типы апдейтов — включая CallbackQuery.
        // Без этого Telegram помнит старый фильтр (напр. только ["message"])
        // и inline-кнопки "Confirm/Reject" под заказами никогда не доходят до бота.
        let listener = Polling::builder(bot.clone())
            .allowed_updates(vec![
                AllowedUpdate::Message,
                AllowedUpdate::EditedMessage,
                AllowedUpdate::CallbackQuery,
                AllowedUpdate::InlineQuery,
                AllowedUpdate::MyChatMember,
                AllowedUpdate::ChatMember,
            ])
            .delete_webhook()
            .await
            .build();
        Dispatcher::builder(bot.clone(), handler)
            .dependencies(dptree::deps![
                Arc::clone(&db_for_bot),
                Arc::clone(&config_for_bot),
                Arc::clone(&ai_client_for_bot)
            ])
            .build()
            .dispatch_with_listener(
                listener,
                teloxide::error_handlers::LoggingErrorHandler::with_custom_text(
                    "An error from the update listener",
                ),
            )
            .await;
    });

    let app_state = AppState {
        db: db.clone(),
        config: config.clone(),
        bot: bot_arc_for_state,
        cache: Arc::new(ETagCache::new()),
    };

    // Prometheus metrics layer — tracks request count, latency histograms, errors per endpoint
    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::HeaderName::from_static("x-telegram-init-data"),
            axum::http::header::HeaderName::from_static("x-admin-token"),
            axum::http::header::HeaderName::from_static("x-admin-telegram-id"),
            axum::http::header::ACCEPT,
            axum::http::header::ACCEPT_ENCODING,
        ])
        .expose_headers([axum::http::header::CONTENT_ENCODING]);

    // Bypass ngrok interstitial page on free tier
    let ngrok_bypass = SetResponseHeaderLayer::overriding(
        HeaderName::from_static("ngrok-skip-browser-warning"),
        HeaderValue::from_static("true"),
    );

    // ── Performance layers ────────────────────────────────────────────
    // Brotli / Gzip / Zstd compression for text assets (> 1 KB).
    // DISABLED: we now serve pre-compressed .br / .gz files directly from
    // memory, avoiding runtime CPU overhead.
    // let compression = CompressionLayer::new()
    //     .br(true)
    //     .gzip(true)
    //     .zstd(true);

    // For `/assets/*` images the URL is stable so 1 day is enough.
    // (Hashed Trunk bundles are served via the SPA fallback with no-store
    // because Telegram WebApp cache-busting takes priority over CDN caching.)
    let assets_cache_layer = || {
        SetResponseHeaderLayer::if_not_present(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=86400"),
        )
    };
    let nosniff_layer = || {
        SetResponseHeaderLayer::if_not_present(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        )
    };
    // HTML must NEVER be cached — Telegram WebApp aggressively keeps the
    // index.html in cache, which breaks deploys (new WASM hash never
    // fetched). `no-store` forces a re-validate on every load.
    let html_no_cache_layer = || {
        SetResponseHeaderLayer::overriding(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, no-cache, must-revalidate, max-age=0"),
        )
    };
    // CSP for Telegram Mini App: allow self, WASM eval, inline styles, and API/S3 images.
    let csp_layer = || {
        SetResponseHeaderLayer::if_not_present(
        axum::http::header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; \
             script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval' https://telegram.org; \
             style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
             img-src 'self' data: https: blob:; \
             media-src 'self' data: https: blob:; \
             connect-src 'self' https:; \
             font-src 'self' https://fonts.gstatic.com; \
             frame-ancestors https://*.telegram.org"
        ),
    )
    };
    let referrer_layer = || {
        SetResponseHeaderLayer::if_not_present(
            axum::http::header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        )
    };

    // Load dist files into memory to bypass slow Railway disk I/O
    // for the WASM bundle (~4 MB) and hashed JS/CSS.
    let mut static_cache: std::collections::HashMap<String, CachedFile> =
        std::collections::HashMap::new();

    fn load_file(path: &std::path::Path) -> Option<Bytes> {
        std::fs::read(path).ok().map(Bytes::from)
    }

    fn add_to_cache(
        cache: &mut std::collections::HashMap<String, CachedFile>,
        path: &std::path::Path,
    ) {
        // Skip pre-compressed sidecar files — they are handled together with the original
        if path.extension().and_then(|e| e.to_str()) == Some("br")
            || path.extension().and_then(|e| e.to_str()) == Some("gz")
        {
            return;
        }
        let key = path
            .strip_prefix("dist/")
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        let raw = match load_file(path) {
            Some(b) => b,
            None => return,
        };
        let br_path = path.with_extension(format!(
            "{}.{}",
            path.extension().unwrap_or_default().to_string_lossy(),
            "br"
        ));
        let gz_path = path.with_extension(format!(
            "{}.{}",
            path.extension().unwrap_or_default().to_string_lossy(),
            "gz"
        ));
        let br = load_file(&br_path);
        let gzip = load_file(&gz_path);
        cache.insert(
            key,
            CachedFile {
                raw,
                gzip,
                br,
                content_type: content_type_for(path),
            },
        );
    }

    fn walk_dir(cache: &mut std::collections::HashMap<String, CachedFile>, dir: &std::path::Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk_dir(cache, &path);
                } else {
                    add_to_cache(cache, &path);
                }
            }
        }
    }

    walk_dir(&mut static_cache, std::path::Path::new("dist"));
    tracing::info!("Cached {} dist files in memory", static_cache.len());

    // /assets, /styles, /images — stable URLs, day-long browser cache.
    let static_assets = Router::new()
        .nest_service("/styles", ServeDir::new("styles"))
        .nest_service("/assets", ServeDir::new("assets"))
        .nest_service("/images", ServeDir::new("assets"))
        .nest_service("/uploads", ServeDir::new("/data/uploads"))
        .layer(assets_cache_layer())
        .layer(nosniff_layer());

    // Trunk emits hashed JS/WASM/CSS at the root of dist/ with absolute paths
    // (e.g. /woody-weed-bot-<hash>.js). We serve dist/ as the root static dir;
    // when a request doesn't match a file, ServeDir falls back to index.html
    // (which is NOT hashed, so we need to override caching for it separately).
    //
    // In-memory fallback handler using the pre-loaded static_cache.
    let static_cache = std::sync::Arc::new(static_cache);

    let serve_dist = {
        let static_cache = static_cache.clone();
        move |uri: axum::http::Uri, headers: axum::http::HeaderMap| {
            let static_cache = static_cache.clone();
            async move {
                let path = uri.path().trim_start_matches('/');
                if path.contains("..") || path.contains('\\') || path.contains('\0') {
                    return (axum::http::StatusCode::NOT_FOUND, "Not found").into_response();
                }
                let exact = static_cache.get(path);
                let cached = exact.or_else(|| static_cache.get("index.html"));
                if let Some(file) = cached {
                    let (body, encoding) = match pick_encoding(&headers) {
                        Some("br") => {
                            if let Some(br) = &file.br {
                                (br.clone(), Some("br"))
                            } else {
                                (file.raw.clone(), None)
                            }
                        }
                        Some("gzip") => {
                            if let Some(gz) = &file.gzip {
                                (gz.clone(), Some("gzip"))
                            } else {
                                (file.raw.clone(), None)
                            }
                        }
                        _ => (file.raw.clone(), None),
                    };
                    let mut resp = (axum::http::StatusCode::OK, body).into_response();
                    resp.headers_mut().insert(
                        axum::http::header::CONTENT_TYPE,
                        axum::http::HeaderValue::from_static(file.content_type),
                    );
                    if let Some(enc) = encoding {
                        resp.headers_mut().insert(
                            axum::http::header::CONTENT_ENCODING,
                            axum::http::HeaderValue::from_static(enc),
                        );
                    }
                    // Cache control: only exact hashed assets are immutable; HTML/fallback must never be cached
                    // Cycle #169: path.contains('-') was too broad — snippet dirs like
                    // dioxus-web-bf44d47c344f35d0 contain '-', so inline1.js got
                    // immutable even though its name is stable across builds. This
                    // caused stale snippet exports (get_select_data) to be cached
                    // forever in Safari/WebView. Immutable now requires the hash
                    // to be in the file name itself, not just the parent dir.
                    let cache_header = if exact.is_some() {
                        let file_name = std::path::Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("");
                        if file_name.contains('-') {
                            "public, max-age=31536000, immutable"
                        } else {
                            "no-store, no-cache, must-revalidate, max-age=0"
                        }
                    } else {
                        "no-store, no-cache, must-revalidate, max-age=0"
                    };
                    resp.headers_mut().insert(
                        axum::http::header::CACHE_CONTROL,
                        axum::http::HeaderValue::from_static(cache_header),
                    );
                    resp
                } else {
                    (axum::http::StatusCode::NOT_FOUND, "Not found").into_response()
                }
            }
        }
    };

    // SPA handler: Telegram WebView (iOS WKWebView) aggressively caches
    // HTML responses and ignores Cache-Control headers. A query-param
    // redirect (?v=4) busts the cache for every SPA route — not just /.
    // When v=4 is present we serve index.html; otherwise redirect to
    // the same path with ?v=4 appended.
    let spa_handler = {
        let static_cache = static_cache.clone();
        move |uri: axum::http::Uri| async move {
            if uri.query().map(|q| q.contains("v=4")).unwrap_or(false) {
                let html = if let Some(file) = static_cache.get("index.html") {
                    String::from_utf8_lossy(&file.raw).to_string()
                } else {
                    "<h1>App not found</h1>".to_string()
                };
                axum::response::Html(html).into_response()
            } else {
                let path = uri.path();
                let existing = uri.query().unwrap_or("");
                let sep = if existing.is_empty() { "" } else { "&" };
                axum::response::Redirect::to(&format!("{}?{}{}v=4", path, existing, sep))
                    .into_response()
            }
        }
    };
    let spa_routes = Router::new()
        .route("/", get(spa_handler.clone()))
        .route("/menu", get(spa_handler.clone()))
        .route("/sets", get(spa_handler.clone()))
        .route("/sommelier", get(spa_handler.clone()))
        .route("/accessories", get(spa_handler.clone()))
        .route("/tea", get(spa_handler.clone()))
        .route("/cart", get(spa_handler.clone()))
        .route("/checkout", get(spa_handler.clone()))
        .route("/success", get(spa_handler.clone()))
        .route("/orders", get(spa_handler.clone()))
        .route("/profile", get(spa_handler.clone()))
        .route("/garden", get(spa_handler.clone()))
        .route("/quest", get(spa_handler.clone()))
        .route("/game", get(spa_handler.clone()))
        .route("/referrals", get(spa_handler.clone()))
        .route("/treasure-hunt", get(spa_handler.clone()))
        .route("/ar-hunt", get(spa_handler.clone()))
        .route("/location-quest", get(spa_handler.clone()))
        .route("/tech-tree", get(spa_handler.clone()))
        .route("/admin", get(spa_handler))
        .layer(html_no_cache_layer())
        .layer(csp_layer())
        .layer(referrer_layer());

    // OpenAPI JSON spec — served at /api-docs/openapi.json (no Swagger UI binary to keep musl build slim)
    #[cfg(feature = "utoipa")]
    let openapi_router: Router = {
        use utoipa::OpenApi;
        let spec_json: std::sync::Arc<String> = std::sync::Arc::new(
            crate::api::openapi::ApiDoc::openapi()
                .to_json()
                .unwrap_or_else(|_| "{}".to_string()),
        );
        Router::new().route(
            "/api-docs/openapi.json",
            get(move || {
                let spec_json = spec_json.clone();
                async move {
                    (
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        (*spec_json).clone(),
                    )
                }
            }),
        )
    };
    #[cfg(not(feature = "utoipa"))]
    let openapi_router: Router = Router::new();

    let api_router = api::router(app_state.clone()).layer(axum::middleware::from_fn_with_state(
        app_state.clone(),
        alert_5xx_middleware,
    ));

    let mut app = Router::new()
        .merge(openapi_router)
        .merge(api_router)
        // CORS layer MUST be first! (applied globally after all backend route merges)
        .layer(cors)
        .layer(ngrok_bypass);

    // Prometheus /metrics endpoint — gated behind admin auth
    let metrics_auth_state = app_state.clone();
    app = app.route(
        "/metrics",
        get(move |headers: HeaderMap| async move {
            if crate::api::auth::check_admin(&headers, &metrics_auth_state).is_err() {
                return StatusCode::UNAUTHORIZED.into_response();
            }
            metric_handle.render().into_response()
        }),
    );

    app = app
        .layer(prometheus_layer)
        // SPA routes - serve index.html for client-side routing
        .merge(spa_routes)
        // SPA routes - these should be served by the fallback
        .merge(static_assets)
        // Single top-level fallback: serve hashed bundles from dist/, fall
        // back to SPA index.html if path not found. We apply no-store cache
        // headers globally on the fallback; immutable cache for hashed
        // assets would be ideal but requires per-file logic. Telegram WebApp
        // cache busting is the priority — no-store keeps deploys landing.
        .fallback(serve_dist)
        // Global security headers: ensure fallback and static assets also get CSP,
        // Referrer-Policy, and X-Content-Type-Options (API responses are unaffected).
        .layer(csp_layer())
        .layer(referrer_layer())
        .layer(nosniff_layer());
    // .layer(compression);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("🚀 HTTP server listening on {}", addr);
    info!("🎨 UI assets served from: styles/, assets/");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::expect_used)] // tokio signal handlers are infallible in practice; panic-on-bug is the conventional pattern.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("🛑 Shutdown signal received, starting graceful shutdown...");
}

#[cfg(test)]
mod tests {
    use super::content_type_for;
    use std::path::Path;

    #[test]
    fn test_content_type_for_html() {
        assert_eq!(content_type_for(Path::new("index.html")), "text/html");
    }

    #[test]
    fn test_content_type_for_js() {
        assert_eq!(content_type_for(Path::new("app.js")), "text/javascript");
    }

    #[test]
    fn test_content_type_for_css() {
        assert_eq!(content_type_for(Path::new("style.css")), "text/css");
    }

    #[test]
    fn test_content_type_for_wasm() {
        assert_eq!(content_type_for(Path::new("app.wasm")), "application/wasm");
    }

    #[test]
    fn test_content_type_for_svg() {
        assert_eq!(content_type_for(Path::new("logo.svg")), "image/svg+xml");
    }

    #[test]
    fn test_content_type_for_png() {
        assert_eq!(content_type_for(Path::new("image.png")), "image/png");
    }

    #[test]
    fn test_content_type_for_jpg() {
        assert_eq!(content_type_for(Path::new("photo.jpg")), "image/jpeg");
    }

    #[test]
    fn test_content_type_for_jpeg() {
        assert_eq!(content_type_for(Path::new("photo.jpeg")), "image/jpeg");
    }

    #[test]
    fn test_content_type_for_uppercase() {
        assert_eq!(content_type_for(Path::new("photo.JPG")), "image/jpeg");
        assert_eq!(content_type_for(Path::new("app.JS")), "text/javascript");
    }

    #[test]
    fn test_content_type_for_unknown() {
        assert_eq!(
            content_type_for(Path::new("data.bin")),
            "application/octet-stream"
        );
        assert_eq!(
            content_type_for(Path::new("noext")),
            "application/octet-stream"
        );
    }

    #[test]
    fn test_pick_encoding_br() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::ACCEPT_ENCODING,
            "br, gzip".parse().unwrap(),
        );
        assert_eq!(super::pick_encoding(&headers), Some("br"));
    }

    #[test]
    fn test_pick_encoding_gzip() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::ACCEPT_ENCODING,
            "gzip, deflate".parse().unwrap(),
        );
        assert_eq!(super::pick_encoding(&headers), Some("gzip"));
    }

    #[test]
    fn test_pick_encoding_none() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::ACCEPT_ENCODING,
            "identity".parse().unwrap(),
        );
        assert_eq!(super::pick_encoding(&headers), None);
    }

    #[test]
    fn test_pick_encoding_no_header() {
        let headers = axum::http::HeaderMap::new();
        assert_eq!(super::pick_encoding(&headers), None);
    }

    #[test]
    fn test_pick_encoding_no_false_br() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::ACCEPT_ENCODING,
            "abbr, identity".parse().unwrap(),
        );
        assert_eq!(super::pick_encoding(&headers), None);
    }

    // ── resolve_log_filter (cycle #120) ─────────────────────────────
    //
    // The pre-#120 code dropped *every* log when RUST_LOG was unset.
    // These tests pin the new behaviour: unset/empty -> info default,
    // valid directive used as-is, malformed -> fallback. Re-running
    // them on a tracing-subscriber upgrade is cheap insurance.

    #[test]
    fn resolve_log_filter_none_uses_info_default() {
        let f = super::resolve_log_filter(None);
        // EnvFilter's Display includes the directives.
        let s = format!("{}", f);
        assert!(s.contains("info"), "expected 'info' in {:?}", s);
    }

    #[test]
    fn resolve_log_filter_empty_string_uses_default() {
        let f = super::resolve_log_filter(Some(String::new()));
        assert!(format!("{}", f).contains("info"));
    }

    #[test]
    fn resolve_log_filter_whitespace_uses_default() {
        let f = super::resolve_log_filter(Some("   ".into()));
        assert!(format!("{}", f).contains("info"));
    }

    #[test]
    fn resolve_log_filter_valid_directive_passes_through() {
        let f = super::resolve_log_filter(Some("debug,sea_orm=warn".into()));
        let s = format!("{}", f);
        assert!(s.contains("debug"), "expected 'debug' in {:?}", s);
        assert!(
            s.contains("sea_orm=warn"),
            "expected 'sea_orm=warn' in {:?}",
            s
        );
    }

    #[test]
    fn resolve_log_filter_malformed_falls_back() {
        // A directive with an invalid level keyword.
        let f = super::resolve_log_filter(Some("this_is=not_a_level".into()));
        assert!(format!("{}", f).contains("info"));
    }

    // ── spawn_ttl_sweep (cycle #125) ────────────────────────────────
    //
    // Uses tokio's paused time so we don't actually wait 60+ seconds.
    // The cleanup closure ignores its `DatabaseConnection` argument and
    // just increments a counter. We assert: (a) the first interval tick
    // is discarded (cold-start serialisation contract from #63),
    // (b) subsequent ticks fire `cleanup`. Concrete tick counts under
    // paused time are tokio-impl-defined; the test only pins the
    // weaker invariant "cleanup ran at least once after advancing
    // past the first interval".

    #[tokio::test(start_paused = true)]
    async fn spawn_ttl_sweep_invokes_cleanup_after_interval() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();

        super::spawn_ttl_sweep(
            sea_orm::DatabaseConnection::Disconnected,
            "test_sweep",
            1, // 1-second interval — paused time makes this instant
            move |_orm| {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<u64, sea_orm::DbErr>(0)
                }
            },
        );

        // Let the spawned task reach its first `tick().await` (the
        // discarded one) and the second one (the first real cleanup
        // tick).
        for _ in 0..5 {
            tokio::time::advance(std::time::Duration::from_millis(500)).await;
            tokio::task::yield_now().await;
        }

        assert!(
            counter.load(Ordering::SeqCst) >= 1,
            "cleanup never ran after 2.5s of paused-time advance — \
             interval/spawn wiring is broken"
        );
    }

    /// `Err` from cleanup must not poison the loop — subsequent ticks
    /// should still fire. Without this, a transient DB hiccup would
    /// silently stop all retention sweeps until the next bot restart.
    #[tokio::test(start_paused = true)]
    async fn spawn_ttl_sweep_keeps_ticking_after_cleanup_error() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();

        super::spawn_ttl_sweep(
            sea_orm::DatabaseConnection::Disconnected,
            "fail_then_recover",
            1,
            move |_orm| {
                let c = c.clone();
                async move {
                    let n = c.fetch_add(1, Ordering::SeqCst);
                    if n == 0 {
                        // First call fails — does the loop survive?
                        Err(sea_orm::DbErr::Custom("simulated transient".into()))
                    } else {
                        Ok::<u64, sea_orm::DbErr>(0)
                    }
                }
            },
        );

        for _ in 0..10 {
            tokio::time::advance(std::time::Duration::from_millis(500)).await;
            tokio::task::yield_now().await;
        }

        assert!(
            counter.load(Ordering::SeqCst) >= 2,
            "loop stopped after first Err — expected >= 2 invocations, got {}",
            counter.load(Ordering::SeqCst)
        );
    }

    // ── is_production_env (cycle #121) ──────────────────────────────

    #[test]
    fn is_production_env_neither_set() {
        assert!(!super::is_production_env(None, None));
    }

    #[test]
    fn is_production_env_node_env_production() {
        assert!(super::is_production_env(Some("production"), None));
    }

    #[test]
    fn is_production_env_node_env_other_values_are_dev() {
        // Mirror Config::from_env — only the literal "production" trips
        // the prod flag. "prod", "PROD", "Production" all stay dev.
        assert!(!super::is_production_env(Some("prod"), None));
        assert!(!super::is_production_env(Some(""), None));
        assert!(!super::is_production_env(Some("development"), None));
    }

    #[test]
    fn is_production_env_any_railway_env_is_prod() {
        // Railway sets RAILWAY_ENVIRONMENT to the env name ("production",
        // "staging", etc.). Any non-None value is enough — we don't
        // discriminate between Railway environments here.
        assert!(super::is_production_env(None, Some("production")));
        assert!(super::is_production_env(None, Some("staging")));
        assert!(super::is_production_env(None, Some("")));
    }

    #[test]
    fn is_production_env_either_signal_enough() {
        // OR semantics — either env var being prod-shaped flips the flag.
        assert!(super::is_production_env(
            Some("production"),
            Some("staging")
        ));
        assert!(super::is_production_env(Some("development"), Some("any")));
    }
}

/// Cycle #133-A: catches "added a class name in JSX but forgot the CSS
/// rule" — the exact bug that hid in cycle #131 until cycle #132 fixed
/// it. Walks every `src/ui/**/*.rs`, extracts class names from `class:
/// "..."` literals, then checks each name has a `.name` selector in
/// some `styles/*.css`. Pure string scan — no parser deps.
///
/// Tolerant by construction: skips dynamic expressions (`class: if
/// cond { ... }`, `class: format!(...)`, `class: "{var}"`), so the
/// test only flags **literal-string** class names. Allowlist below
/// for class names that intentionally have no CSS (e.g. styled
/// inline via `style:` attribute, or matched on attribute selectors
/// the regex can't see).
#[cfg(test)]
mod css_class_consistency_tests {
    use std::collections::HashSet;

    /// Classes that appear as literal strings in JSX but legitimately
    /// have no `.class` rule in `styles/`. Document the reason inline.
    const ALLOWLIST: &[&str] = &[
        // Cycle #133-A: pre-existing JSX-CSS mismatches surfaced when
        // this test first ran. Each is a single low-traffic callsite
        // already paired with inline `style:` attribute or relying on
        // browser defaults — none breaks production rendering. Defer
        // proper CSS rules to a focused cleanup cycle; the test now
        // guards against *new* mismatches.
        "pixel-input",     // 1 ref — inline-styled text input
        "btn-green",       // 2 refs — game/garden buttons
        "plant-btn",       // 1 ref — garden grow button
        "card-bg",         // 1 ref — admin background div
        "chip-close",      // 1 ref — selected-chip close button
        "harvest-btn",     // 1 ref — garden harvest button
        "cart-item-image", // 2 refs — cart row image wrapper
    ];

    fn ui_class_literals() -> HashSet<String> {
        let mut classes = HashSet::new();
        let manifest = env!("CARGO_MANIFEST_DIR");
        let ui_dir = std::path::Path::new(manifest).join("src/ui");
        fn walk(p: &std::path::Path, classes: &mut HashSet<String>) {
            for entry in std::fs::read_dir(p)
                .expect("readable")
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, classes);
                } else if path
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s == "rs")
                {
                    let src = match std::fs::read_to_string(&path) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    // Find every `class: "<literal>"`. Skip lines that have
                    // braces in the value (interpolation) or look like
                    // `class: if ...`. Pragmatic: keeps the parser tiny.
                    for line in src.lines() {
                        let trimmed = line.trim_start();
                        if !trimmed.starts_with("class: \"") {
                            continue;
                        }
                        let after = &trimmed[8..]; // past `class: "`
                        let end = match after.find('"') {
                            Some(i) => i,
                            None => continue,
                        };
                        let value = &after[..end];
                        // Templating with `{var}` is dynamic; skip the whole literal.
                        if value.contains('{') {
                            continue;
                        }
                        for token in value.split_ascii_whitespace() {
                            if token.is_empty() {
                                continue;
                            }
                            // Plain identifiers + dashes (CSS-name-shaped).
                            if token
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                            {
                                classes.insert(token.to_string());
                            }
                        }
                    }
                }
            }
        }
        walk(&ui_dir, &mut classes);
        classes
    }

    fn css_selector_names() -> HashSet<String> {
        let mut names = HashSet::new();
        let manifest = env!("CARGO_MANIFEST_DIR");
        let css_dir = std::path::Path::new(manifest).join("styles");
        for entry in std::fs::read_dir(&css_dir)
            .expect("styles/ readable")
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("css") {
                continue;
            }
            let src = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            // Match `.identifier` everywhere — covers `.foo`, `.foo:hover`,
            // `.foo .bar`, etc. Identifier ends at first non-alnum/dash/underscore.
            let bytes = src.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_alphabetic() {
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len()
                        && (bytes[end].is_ascii_alphanumeric()
                            || bytes[end] == b'-'
                            || bytes[end] == b'_')
                    {
                        end += 1;
                    }
                    if end > start {
                        names.insert(String::from_utf8_lossy(&bytes[start..end]).into_owned());
                    }
                    i = end;
                } else {
                    i += 1;
                }
            }
        }
        names
    }

    #[test]
    fn every_jsx_class_literal_has_a_css_rule() {
        let ui = ui_class_literals();
        let css = css_selector_names();
        assert!(!ui.is_empty(), "no UI class literals parsed");
        assert!(!css.is_empty(), "no CSS selectors parsed");

        let allow: HashSet<&str> = ALLOWLIST.iter().copied().collect();
        let missing: Vec<&String> = ui
            .iter()
            .filter(|c| !css.contains(*c) && !allow.contains(c.as_str()))
            .collect();

        assert!(
            missing.is_empty(),
            "JSX class names with no matching CSS rule ({}): {:?}\n\
             Either add a `.<name>` selector under styles/, or list the \
             class in ALLOWLIST with a rationale.",
            missing.len(),
            missing
        );
    }
}

/// Wave loop: keeps interactive controls finger-sized. A clickable element
/// (`cursor:pointer`) that pins an explicit `width`/`height` below 44 CSS px
/// is a Fitts's-Law / WCAG 2.5.5 (AAA, 44px) / Apple-HIG (44pt) violation —
/// on a mobile Telegram Mini App that means doubled mis-tap rates (the cart
/// stepper sat at 30px and the catalog video buttons at 28–36px before this
/// sweep). Padding-sized controls aren't checked (they set no fixed
/// dimension). Pure string scan — mirrors `css_class_consistency_tests`.
#[cfg(test)]
mod touch_target_tests {
    use std::collections::HashSet;

    const MIN_PX: u32 = 44;

    /// Normalized (whitespace-stripped) styles for intentionally-small
    /// clickable targets. Document the reason inline.
    const ALLOWLIST: &[&str] = &[
        // Admin bulk-select native checkbox — native form controls are
        // conventionally smaller and covered by WCAG 2.5.8's spacing exception.
        "width:18px;height:18px;cursor:pointer;flex-shrink:0;",
    ];

    /// Returns `Some((dim, px))` if a `width`/`height` declaration (exact key,
    /// so `min-`/`max-` are ignored) pins a px value below `MIN_PX`.
    fn dim_under_min(style_norm: &str) -> Option<(&'static str, u32)> {
        for decl in style_norm.split(';') {
            let Some((k, v)) = decl.split_once(':') else {
                continue;
            };
            let dim = match k {
                "width" => "width",
                "height" => "height",
                _ => continue,
            };
            if let Some(px) = v.strip_suffix("px").and_then(|n| n.parse::<u32>().ok()) {
                if px < MIN_PX {
                    return Some((dim, px));
                }
            }
        }
        None
    }

    fn walk(p: &std::path::Path, f: &mut dyn FnMut(&std::path::Path, &str)) {
        for e in std::fs::read_dir(p)
            .expect("readable")
            .filter_map(|e| e.ok())
        {
            let path = e.path();
            if path.is_dir() {
                walk(&path, f);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                if let Ok(src) = std::fs::read_to_string(&path) {
                    f(&path, &src);
                }
            }
        }
    }

    #[test]
    fn clickable_targets_meet_min_size() {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let ui = std::path::Path::new(manifest).join("src/ui");
        let allow: HashSet<&str> = ALLOWLIST.iter().copied().collect();
        let mut violations = Vec::new();
        let mut checked = 0usize;

        walk(&ui, &mut |path, src| {
            // Extract each `style: "..."` literal (no escaped quotes inside).
            let mut i = 0;
            while let Some(rel) = src[i..].find("style:") {
                let after = i + rel + "style:".len();
                let Some(qrel) = src[after..].find('"') else {
                    break;
                };
                let q = after + qrel;
                let Some(erel) = src[q + 1..].find('"') else {
                    break;
                };
                let end = q + 1 + erel;
                let norm: String = src[q + 1..end]
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                i = end + 1;
                if !norm.contains("cursor:pointer") || allow.contains(norm.as_str()) {
                    continue;
                }
                checked += 1;
                if let Some((dim, px)) = dim_under_min(&norm) {
                    let file = path.file_name().and_then(|s| s.to_str()).unwrap_or("?");
                    violations.push(format!("{file}: clickable {dim}:{px}px < {MIN_PX}px"));
                }
            }
        });

        assert!(checked > 0, "no clickable styles parsed — parser broken?");
        assert!(
            violations.is_empty(),
            "{} clickable target(s) below {MIN_PX}px (Fitts's Law / WCAG 2.5.5) — \
             enlarge to >= 44px or add the normalized style to ALLOWLIST with a \
             rationale:\n  {}",
            violations.len(),
            violations.join("\n  ")
        );
    }
}

/// Wave loop: every `<img>` needs an `alt` (WCAG 1.1.1 Non-text Content) —
/// descriptive for informative images, `alt=""` for decorative. A missing
/// alt makes the image invisible to screen-reader users. In our rsx, `img {`
/// elements declare `alt:` right after `src:`, so a short forward window is
/// enough. Pure string scan — mirrors `css_class_consistency_tests`; runs on
/// host. Measurable WCAG subset only — does not judge alt *quality*.
#[cfg(test)]
mod img_alt_tests {
    #[test]
    fn every_img_has_alt() {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let ui = std::path::Path::new(manifest).join("src/ui");
        let mut missing = Vec::new();
        let mut checked = 0usize;

        fn walk(p: &std::path::Path, f: &mut dyn FnMut(&std::path::Path, &str)) {
            for e in std::fs::read_dir(p)
                .expect("readable")
                .filter_map(|e| e.ok())
            {
                let path = e.path();
                if path.is_dir() {
                    walk(&path, f);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    if let Ok(src) = std::fs::read_to_string(&path) {
                        f(&path, &src);
                    }
                }
            }
        }

        walk(&ui, &mut |path, src| {
            let lines: Vec<&str> = src.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if !line.contains("img {") {
                    continue;
                }
                checked += 1;
                // alt is declared at the top of the element; an 8-line window
                // covers both single-line and multi-line `img { ... }` forms.
                let end = (i + 8).min(lines.len());
                let window = lines[i..end].join("\n");
                if !window.contains("alt:") {
                    let file = path.file_name().and_then(|s| s.to_str()).unwrap_or("?");
                    missing.push(format!("{file}:{}", i + 1));
                }
            }
        });

        assert!(checked > 0, "no img elements parsed — parser broken?");
        assert!(
            missing.is_empty(),
            "{} <img> without an alt attribute (WCAG 1.1.1) — add a descriptive \
             `alt: \"…\"` (or `alt: \"\"` if purely decorative):\n  {}",
            missing.len(),
            missing.join("\n  ")
        );
    }
}

/// Wave loop: catches a bilingual catalog field that is **fetched but
/// never shown** — declared on a UI screen DTO (`*_en` / `*_localized`)
/// yet never read, so the English/localized text the API sends silently
/// never reaches the user. The screen DTOs carry `#[allow(dead_code)]`
/// (some serde fields are intentionally unused), which suppresses rustc's
/// own dead-field warning — so a dropped `localized()` wiring goes unseen.
/// Concrete prior-art: `ApiSet.description_localized` was declared but the
/// API never emitted it and nothing read it (found + removed this Wave).
///
/// Invariant: every `*_en` / `*_localized` field declared in any
/// `src/ui/screens/*.rs` must be referenced at least once beyond its own
/// declaration. Pure string scan — mirrors `css_class_consistency_tests`.
#[cfg(test)]
mod bilingual_field_wiring_tests {
    /// Count word-boundary occurrences of `ident` in `hay`.
    fn count_ident(hay: &str, ident: &str) -> usize {
        let mut n = 0;
        let bytes = hay.as_bytes();
        let mut start = 0;
        while let Some(pos) = hay[start..].find(ident) {
            let abs = start + pos;
            let before_ok =
                abs == 0 || (!bytes[abs - 1].is_ascii_alphanumeric() && bytes[abs - 1] != b'_');
            let end = abs + ident.len();
            let after_ok =
                end >= bytes.len() || (!bytes[end].is_ascii_alphanumeric() && bytes[end] != b'_');
            if before_ok && after_ok {
                n += 1;
            }
            start = abs + ident.len();
        }
        n
    }

    /// `Some(field_name)` if `line` declares/initializes a field whose name
    /// ends in `_en` or `_localized` (e.g. `name_en: Option<String>,` or
    /// `description_en: if ... { .. }`). Splits on the first `:` and checks
    /// the left side is a bare identifier with the bilingual suffix.
    fn bilingual_field(line: &str) -> Option<String> {
        let t = line.trim();
        let colon = t.find(':')?;
        let name = &t[..colon];
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return None;
        }
        if name.ends_with("_en") || name.ends_with("_localized") {
            Some(name.to_string())
        } else {
            None
        }
    }

    #[test]
    fn every_bilingual_field_is_consumed() {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let dir = std::path::Path::new(manifest).join("src/ui/screens");
        let mut dead = Vec::new();
        let mut checked = 0usize;

        for entry in std::fs::read_dir(&dir)
            .expect("src/ui/screens readable")
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            let src = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let file = path.file_name().and_then(|s| s.to_str()).unwrap_or("?");

            // Collect candidate field names, then require each to appear
            // more than once in the file (declaration + at least one use).
            let mut names: Vec<String> = src.lines().filter_map(bilingual_field).collect();
            names.sort();
            names.dedup();
            for name in names {
                checked += 1;
                if count_ident(&src, &name) < 2 {
                    dead.push(format!("{file}: `{name}` declared but never consumed"));
                }
            }
        }

        assert!(checked > 0, "no bilingual fields parsed — parser broken?");
        assert!(
            dead.is_empty(),
            "{} bilingual field(s) fetched but never shown — wire them through \
             `lang::localized()` or remove the dead field:\n  {}",
            dead.len(),
            dead.join("\n  ")
        );
    }
}

/// Catches "added `pub mod X;` to wire up a new screen, then renamed
/// or replaced its only caller, leaving an entire dead parallel
/// module branch the compiler can't see". Concrete prior-art: cycle
/// #173 deleted `src/ui/lib.rs` + 6 sibling test-scaffold files that
/// formed a closed reference cycle — every item was `pub`, so rustc
/// emitted no warnings even though *nothing outside the cycle*
/// touched them.
///
/// Walks every `mod.rs` / `lib.rs` / `main.rs` under `src/`, parses
/// `pub mod X;` lines, and asserts each `X` is mentioned by name in
/// at least one `.rs` file outside its own subtree (= `<dir>/X.rs`
/// for single-file modules or `<dir>/X/` for folder modules). The
/// declaring file is included in the cross-ref scan with only the
/// literal `pub mod X;` line stripped, so re-export patterns like
/// `pub use X::Item;` legitimately count as wiring.
///
/// Tolerant by construction: matches by word boundary, so a 3-letter
/// module name could in principle false-positive on comment text.
/// Allowlist `ALLOWED_UNWIRED_MODS` for the rare case of a
/// deliberately-unused module.
#[cfg(test)]
mod module_wiring_tests {
    use std::path::{Path, PathBuf};

    const ALLOWED_UNWIRED_MODS: &[&str] = &[
        // `src/db/macros.rs` exports `try_get_warn!` via `#[macro_export]`,
        // which routes the macro to the crate root (`crate::try_get_warn!`)
        // rather than `crate::db::macros::try_get_warn!`. The module name
        // therefore never appears in any callsite even though the macro is
        // used 20+ times across the backend.
        "macros",
    ];

    fn collect_rs_files(root: &Path, out: &mut Vec<PathBuf>) {
        let entries = match std::fs::read_dir(root) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    /// Returns `(declaring_file, module_name, subtree_root)` for
    /// every `pub mod X;` under `src/` declared in a `mod.rs`,
    /// `lib.rs`, or `main.rs` (the three Rust-idiomatic homes for
    /// module-tree declarations). `subtree_root` is the `.rs` file
    /// for single-file modules or the directory for folder modules.
    fn all_pub_mod_decls() -> Vec<(PathBuf, String, PathBuf)> {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let src_root = Path::new(manifest).join("src");
        let mut mod_files = Vec::new();
        collect_rs_files(&src_root, &mut mod_files);
        mod_files.retain(|p| {
            matches!(
                p.file_name().and_then(|s| s.to_str()),
                Some("mod.rs" | "lib.rs" | "main.rs")
            )
        });

        let mut decls = Vec::new();
        for mod_path in &mod_files {
            let src = match std::fs::read_to_string(mod_path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let parent = match mod_path.parent() {
                Some(p) => p,
                None => continue,
            };
            for line in src.lines() {
                let trimmed = line.trim_start();
                // Strip a `//` line comment so commented-out
                // declarations don't trigger.
                let code = match trimmed.find("//") {
                    Some(i) => &trimmed[..i],
                    None => trimmed,
                };
                let code = code.trim();
                // Accept `pub mod NAME;`, `pub(crate) mod NAME;`,
                // `pub(super) mod NAME;`, and plain `mod NAME;`. The
                // visibility flavour doesn't change wiring semantics
                // — only the *existence* of the declaration matters.
                let rest = code
                    .strip_prefix("pub mod ")
                    .or_else(|| code.strip_prefix("pub(crate) mod "))
                    .or_else(|| code.strip_prefix("pub(super) mod "));
                let rest = match rest {
                    Some(r) => r,
                    None => continue,
                };
                let name = match rest.split([';', ' ', '{']).next() {
                    Some(n) if !n.is_empty() => n.trim(),
                    _ => continue,
                };
                let file_form = parent.join(format!("{name}.rs"));
                let dir_form = parent.join(name);
                let subtree = if file_form.is_file() {
                    file_form
                } else if dir_form.is_dir() {
                    dir_form
                } else {
                    continue;
                };
                decls.push((mod_path.clone(), name.to_string(), subtree));
            }
        }
        decls
    }

    fn is_under(path: &Path, root: &Path) -> bool {
        path.starts_with(root)
    }

    fn file_mentions_word(src: &str, word: &str) -> bool {
        let bytes = src.as_bytes();
        let needle = word.as_bytes();
        let mut i = 0;
        while i + needle.len() <= bytes.len() {
            if &bytes[i..i + needle.len()] == needle {
                let before_ok = i == 0 || {
                    let c = bytes[i - 1];
                    !(c.is_ascii_alphanumeric() || c == b'_')
                };
                let after_idx = i + needle.len();
                let after_ok = after_idx >= bytes.len() || {
                    let c = bytes[after_idx];
                    !(c.is_ascii_alphanumeric() || c == b'_')
                };
                if before_ok && after_ok {
                    return true;
                }
            }
            i += 1;
        }
        false
    }

    /// Read a file, but strip the `pub mod NAME;` declaration line for
    /// the module under test. Other lines (including `pub use NAME::*;`
    /// re-exports) survive, so re-exports count as wiring evidence.
    fn read_without_decl(path: &Path, name: &str) -> Option<String> {
        let src = std::fs::read_to_string(path).ok()?;
        // Strip any module declaration of `name`, regardless of its
        // visibility flavour. Order matters for `starts_with` — list
        // the longer prefixes first so e.g. `pub(crate) mod` isn't
        // misread as `mod` (which would leave `(crate) mod NAME;` in
        // the rest and not strip the line).
        let needles: [String; 8] = [
            format!("pub(crate) mod {name};"),
            format!("pub(crate) mod {name}{{"),
            format!("pub(super) mod {name};"),
            format!("pub(super) mod {name}{{"),
            format!("pub mod {name};"),
            format!("pub mod {name}{{"),
            format!("mod {name};"),
            format!("mod {name}{{"),
        ];
        let filtered = src
            .lines()
            .filter(|line| {
                let t = line.trim_start();
                !needles.iter().any(|n| t.starts_with(n))
            })
            .collect::<Vec<_>>()
            .join("\n");
        Some(filtered)
    }

    #[test]
    fn every_pub_mod_has_an_external_reference() {
        let decls = all_pub_mod_decls();
        assert!(!decls.is_empty(), "no pub mod declarations parsed");

        let manifest = env!("CARGO_MANIFEST_DIR");
        let src_root = Path::new(manifest).join("src");
        let mut all_rs = Vec::new();
        collect_rs_files(&src_root, &mut all_rs);

        let mut unwired = Vec::new();
        for (decl_mod, name, subtree) in &decls {
            if ALLOWED_UNWIRED_MODS.contains(&name.as_str()) {
                continue;
            }
            let mut found = false;
            for file in &all_rs {
                if is_under(file, subtree) {
                    continue;
                }
                let src = if file == decl_mod {
                    match read_without_decl(file, name) {
                        Some(s) => s,
                        None => continue,
                    }
                } else {
                    match std::fs::read_to_string(file) {
                        Ok(s) => s,
                        Err(_) => continue,
                    }
                };
                if file_mentions_word(&src, name) {
                    found = true;
                    break;
                }
            }
            if !found {
                unwired.push(format!("{} (declared in {})", name, decl_mod.display()));
            }
        }

        assert!(
            unwired.is_empty(),
            "Modules declared but never referenced outside their own subtree ({}): {:?}\n\
             Either delete the module and its `pub mod` declaration, or list \
             the name in ALLOWED_UNWIRED_MODS with a rationale.",
            unwired.len(),
            unwired
        );
    }
}

/// Catches the exact prod-incident class shipped in commit c8c9546:
/// `src/api/catalog.rs::get_sets` referenced `tea_sets.image_url` in a
/// `SELECT` query, but no migration ever added that column — every
/// public menu page load died with `column tea_sets.image_url does not
/// exist` → 500. The compiler can't see schema drift; this test does
/// the cross-check at build time.
///
/// Walks `migrations/*.sql` to build `{table → columns}` ground truth
/// from `CREATE TABLE` bodies and `ALTER TABLE ADD COLUMN` statements,
/// then scans `src/**/*.rs` for simple-shape `"SELECT c1, c2, ..., cn
/// FROM <table> ..."` string literals and asserts each column appears
/// in the table's schema. Complex SQL (multi-line, joins,
/// subqueries) is skipped — the parser only handles the
/// single-line-FROM shape that covers the failure mode we just shipped.
#[cfg(test)]
mod schema_drift_tests {
    use std::collections::{HashMap, HashSet};
    use std::path::{Path, PathBuf};

    /// Column tokens that legitimately appear in SELECT lists but
    /// aren't real schema columns (computed aliases, RETURNING-style
    /// expressions, subquery shapes). Document the reason inline.
    const ALLOWED_COMPUTED_COLUMNS: &[&str] = &[
        // Aggregate aliases — these are output names, not source
        // columns. Most appear inside `COUNT(*) AS X` and the parser
        // attributes the alias to the FROM table by mistake.
        "count", "total",
        // Computed/JSON expressions that look like identifiers to the
        // tokenizer but aren't column names.
        "now",
    ];

    fn collect_files_with_ext(root: &Path, ext: &str, out: &mut Vec<PathBuf>) {
        let entries = match std::fs::read_dir(root) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                collect_files_with_ext(&path, ext, out);
            } else if path.extension().and_then(|s| s.to_str()) == Some(ext) {
                out.push(path);
            }
        }
    }

    /// Identifier ends at any byte that is not `[A-Za-z0-9_]`.
    fn read_ident(bytes: &[u8], start: usize) -> &[u8] {
        let mut end = start;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        &bytes[start..end]
    }

    /// Skip ASCII whitespace and return next non-whitespace byte index.
    fn skip_ws(bytes: &[u8], start: usize) -> usize {
        let mut i = start;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        i
    }

    /// Parse migrations into `{table → columns}`. Recognises:
    ///   - `CREATE TABLE [IF NOT EXISTS] name ( col TYPE [, ...] );`
    ///   - `ALTER TABLE name ADD COLUMN [IF NOT EXISTS] col TYPE`
    ///
    /// Tolerant: skips column-defs whose first token isn't an
    /// identifier (e.g. `PRIMARY KEY (...)`, `CONSTRAINT ...`),
    /// handles the parenthesis nesting inside `CHECK (...)` clauses
    /// by counting depth.
    fn parse_migration_schema() -> HashMap<String, HashSet<String>> {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let migrations_dir = Path::new(manifest).join("migrations");
        let mut files = Vec::new();
        collect_files_with_ext(&migrations_dir, "sql", &mut files);
        files.sort();

        let mut schema: HashMap<String, HashSet<String>> = HashMap::new();

        for f in &files {
            let src = match std::fs::read_to_string(f) {
                Ok(s) => s,
                Err(_) => continue,
            };
            // Strip `-- line comments` so they don't fool the tokenizer.
            let stripped: String = src
                .lines()
                .map(|line| line.split("--").next().unwrap_or(""))
                .collect::<Vec<_>>()
                .join("\n");
            let upper = stripped.to_ascii_uppercase();
            let bytes = stripped.as_bytes();
            let upper_bytes = upper.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if upper_bytes[i..].starts_with(b"CREATE TABLE") {
                    let mut j = i + b"CREATE TABLE".len();
                    j = skip_ws(bytes, j);
                    if upper_bytes[j..].starts_with(b"IF NOT EXISTS") {
                        j += b"IF NOT EXISTS".len();
                        j = skip_ws(bytes, j);
                    }
                    let name = read_ident(bytes, j);
                    if name.is_empty() {
                        i = j + 1;
                        continue;
                    }
                    let table = String::from_utf8_lossy(name).to_lowercase();
                    j += name.len();
                    j = skip_ws(bytes, j);
                    if j >= bytes.len() || bytes[j] != b'(' {
                        i = j + 1;
                        continue;
                    }
                    // Walk the `(...)` body counting parens; split on
                    // top-level commas only.
                    let body_start = j + 1;
                    let mut depth = 1;
                    let mut k = body_start;
                    while k < bytes.len() && depth > 0 {
                        match bytes[k] {
                            b'(' => depth += 1,
                            b')' => depth -= 1,
                            _ => {}
                        }
                        if depth == 0 {
                            break;
                        }
                        k += 1;
                    }
                    let body = &bytes[body_start..k];
                    let mut p = 0;
                    let mut d = 0;
                    let mut chunk_start = 0;
                    while p <= body.len() {
                        let at_end = p == body.len();
                        let c = if at_end { b',' } else { body[p] };
                        if !at_end {
                            match c {
                                b'(' => d += 1,
                                b')' => d -= 1,
                                _ => {}
                            }
                        }
                        if (c == b',' && d == 0) || at_end {
                            let chunk = &body[chunk_start..p];
                            let trimmed_start = chunk
                                .iter()
                                .position(|b| !b.is_ascii_whitespace())
                                .unwrap_or(chunk.len());
                            let col = read_ident(chunk, trimmed_start);
                            if !col.is_empty() {
                                let col_str = String::from_utf8_lossy(col).to_lowercase();
                                // Skip non-column lines like
                                // PRIMARY/FOREIGN/CONSTRAINT/UNIQUE/CHECK.
                                let kw = matches!(
                                    col_str.as_str(),
                                    "primary"
                                        | "foreign"
                                        | "constraint"
                                        | "unique"
                                        | "check"
                                        | "exclude"
                                );
                                if !kw {
                                    schema.entry(table.clone()).or_default().insert(col_str);
                                }
                            }
                            chunk_start = p + 1;
                        }
                        p += 1;
                    }
                    i = k + 1;
                    continue;
                }
                if upper_bytes[i..].starts_with(b"ALTER TABLE") {
                    let mut j = i + b"ALTER TABLE".len();
                    j = skip_ws(bytes, j);
                    let name = read_ident(bytes, j);
                    if name.is_empty() {
                        i = j + 1;
                        continue;
                    }
                    let table = String::from_utf8_lossy(name).to_lowercase();
                    j += name.len();
                    j = skip_ws(bytes, j);
                    if upper_bytes[j..].starts_with(b"ADD COLUMN") {
                        j += b"ADD COLUMN".len();
                        j = skip_ws(bytes, j);
                        if upper_bytes[j..].starts_with(b"IF NOT EXISTS") {
                            j += b"IF NOT EXISTS".len();
                            j = skip_ws(bytes, j);
                        }
                        let col = read_ident(bytes, j);
                        if !col.is_empty() {
                            let col_str = String::from_utf8_lossy(col).to_lowercase();
                            schema.entry(table).or_default().insert(col_str);
                        }
                        i = j + 1;
                        continue;
                    }
                }
                i += 1;
            }
        }
        schema
    }

    /// Normalise a SELECT-list token to the column name it
    /// ultimately reads from the FROM table.
    ///
    /// `total_price::float8 AS total_price` → `total_price`
    /// `COALESCE(accessory_ids, NULL::text[]) AS accessory_ids` → `accessory_ids`
    /// `MAX(id)` → `id`
    /// `t.col` → `col`
    /// `*` → empty (skipped)
    fn normalise_column(raw: &str) -> Option<String> {
        let s = raw.trim();
        if s.is_empty() || s == "*" {
            return None;
        }
        // Strip everything after ` AS ` (case-insensitive).
        let upper = s.to_ascii_uppercase();
        let before_as = match upper.find(" AS ") {
            Some(i) => &s[..i],
            None => s,
        };
        let s = before_as.trim();
        // If wrapped in a function call like FOO(arg, ...), recurse
        // into the first arg only.
        if let Some(open) = s.find('(') {
            // Use the part inside the outermost parens, up to the first
            // top-level comma.
            let after = &s[open + 1..];
            let mut depth = 1;
            let mut end = 0;
            for (i, b) in after.bytes().enumerate() {
                match b {
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = i;
                            break;
                        }
                    }
                    b',' if depth == 1 => {
                        end = i;
                        break;
                    }
                    _ => {}
                }
            }
            if end > 0 {
                return normalise_column(&after[..end]);
            }
            return None;
        }
        // Strip `::type` cast suffix.
        let s = match s.find("::") {
            Some(i) => &s[..i],
            None => s,
        };
        let s = s.trim();
        // Strip `table.` prefix.
        let s = match s.rfind('.') {
            Some(i) => &s[i + 1..],
            None => s,
        };
        // Must be a bare identifier now. Column names can't start
        // with a digit, so this also filters out `SELECT 1 FROM ...`
        // existence checks.
        let first = s.chars().next()?;
        if !(first.is_ascii_alphabetic() || first == '_') {
            return None;
        }
        if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return None;
        }
        Some(s.to_lowercase())
    }

    /// Read a Rust string literal starting at `start_after_quote`, i.e.
    /// the byte index right after the opening `"`. Returns the logical
    /// string content and the index just past the closing `"`.
    ///
    /// Handles Rust's `\<newline>` line continuation (the `\`, the
    /// newline, and leading whitespace on the next physical line are
    /// stripped). Other escape sequences are passed through as the
    /// escaped character (sufficient for SQL extraction — we don't
    /// care about `\t` vs literal tab semantics).
    ///
    /// Returns `None` on a non-terminated string (ran off end of file).
    fn read_rust_str(bytes: &[u8], start_after_quote: usize) -> Option<(String, usize)> {
        let mut out = String::new();
        let mut i = start_after_quote;
        while i < bytes.len() {
            match bytes[i] {
                b'"' => return Some((out, i + 1)),
                b'\\' => {
                    let nx = *bytes.get(i + 1)?;
                    if nx == b'\n' {
                        i += 2;
                        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
                            i += 1;
                        }
                        continue;
                    }
                    let ch = match nx {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        other => other as char,
                    };
                    out.push(ch);
                    i += 2;
                }
                b => {
                    out.push(b as char);
                    i += 1;
                }
            }
        }
        None
    }

    /// Scan a Rust source file for string literals starting with one
    /// of `needles` (which must include the opening `"`) and return
    /// the logical (continuation-stripped) content of each.
    fn find_sql_literals_starting_with<'a>(src: &str, needles: &'a [&'a str]) -> Vec<String> {
        let mut out = Vec::new();
        let bytes = src.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let mut matched = None;
            for n in needles {
                if i + n.len() <= bytes.len() && &bytes[i..i + n.len()] == n.as_bytes() {
                    matched = Some(*n);
                    break;
                }
            }
            if let Some(n) = matched {
                // Position is at opening `"`; read the full literal.
                let start_after_quote = i + 1;
                if let Some((content, end)) = read_rust_str(bytes, start_after_quote) {
                    // Strip the part before the SQL keyword (just the `"`
                    // is missing now since we positioned past it).
                    // The needle includes opening `"`, so first char of
                    // `content` should be the SQL keyword.
                    out.push(content);
                    i = end;
                    continue;
                }
                i += n.len();
                continue;
            }
            i += 1;
        }
        let _ = needles; // silence the unused-lifetime warn in older toolchains
        out
    }

    /// Find `"SELECT c1, c2, ... FROM <table> ..."` literals (any
    /// line shape — Rust line-continuation is honoured) in a Rust
    /// source file. Returns `(tables, [cols])` per occurrence — the
    /// `tables` Vec is `[primary, ...joined]` for JOIN queries, or
    /// just `[primary]` otherwise. A column is considered valid if
    /// it appears in *any* of those tables' schemas.
    fn find_select_sites_in(src: &str) -> Vec<(Vec<String>, Vec<String>)> {
        find_sql_literals_starting_with(src, &["\"SELECT "])
            .into_iter()
            .filter_map(|s| parse_select(&s))
            .collect()
    }

    /// Parse a SQL string of shape `SELECT <cols> FROM <table> [...]`.
    /// Returns `None` for any shape we don't fully understand (multi-FROM,
    /// CTEs, subqueries in SELECT list).
    fn parse_select(sql: &str) -> Option<(Vec<String>, Vec<String>)> {
        let upper = sql.to_ascii_uppercase();
        if !upper.starts_with("SELECT ") {
            return None;
        }
        let after_select = &sql["SELECT ".len()..];
        let upper_after = &upper["SELECT ".len()..];
        // Find ` FROM ` at top level (depth 0). Bail on subselects.
        let mut depth = 0i32;
        let mut from_at = None;
        let bytes = upper_after.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            if depth == 0 && bytes[i..].starts_with(b" FROM ") {
                from_at = Some(i);
                break;
            }
            i += 1;
        }
        let from_at = from_at?;
        let cols_str = &after_select[..from_at];
        let after_from = &after_select[from_at + " FROM ".len()..];
        // Primary table: first identifier after FROM.
        let tb_bytes = after_from.as_bytes();
        let mut j = 0;
        while j < tb_bytes.len() && (tb_bytes[j].is_ascii_alphanumeric() || tb_bytes[j] == b'_') {
            j += 1;
        }
        if j == 0 {
            return None;
        }
        let primary_table = after_from[..j].to_lowercase();
        let mut tables = vec![primary_table];

        // Extract JOIN target tables from the rest of the FROM-clause.
        // LATERAL subqueries hide their column origins behind an alias
        // we can't resolve cheaply — bail out so we don't generate
        // false positives.
        let upper_rest = upper_after[from_at + " FROM ".len() + j..].to_ascii_uppercase();
        if upper_rest.contains(" LATERAL ") {
            return None;
        }
        // Find every `JOIN <ident>` in the rest. Walk the original-case
        // rest in parallel so we capture the table name verbatim.
        let orig_rest = &after_from[j..];
        let upper_rest_bytes = upper_rest.as_bytes();
        let orig_rest_bytes = orig_rest.as_bytes();
        let mut p = 0;
        while p + 5 < upper_rest_bytes.len() {
            if &upper_rest_bytes[p..p + 5] == b"JOIN " {
                let mut q = p + 5;
                // Skip whitespace
                while q < orig_rest_bytes.len() && orig_rest_bytes[q].is_ascii_whitespace() {
                    q += 1;
                }
                // Read identifier
                let id_start = q;
                while q < orig_rest_bytes.len()
                    && (orig_rest_bytes[q].is_ascii_alphanumeric() || orig_rest_bytes[q] == b'_')
                {
                    q += 1;
                }
                if q > id_start {
                    let t = orig_rest[id_start..q].to_lowercase();
                    // Skip on (a subquery start — shouldn't happen
                    // post-LATERAL bail-out but defensive).
                    if t != "lateral" && !t.is_empty() {
                        tables.push(t);
                    }
                }
                p = q;
                continue;
            }
            p += 1;
        }

        // Split cols_str by top-level commas.
        let mut cols = Vec::new();
        let cb = cols_str.as_bytes();
        let mut d = 0i32;
        let mut start = 0;
        for k in 0..=cb.len() {
            let at_end = k == cb.len();
            let c = if at_end { b',' } else { cb[k] };
            if !at_end {
                match c {
                    b'(' => d += 1,
                    b')' => d -= 1,
                    _ => {}
                }
            }
            if (c == b',' && d == 0) || at_end {
                let chunk = &cols_str[start..k];
                if let Some(col) = normalise_column(chunk) {
                    cols.push(col);
                }
                start = k + 1;
            }
        }
        Some((tables, cols))
    }

    /// Find `"INSERT INTO <table> (c1, c2, ...) VALUES ..."` literals
    /// (multi-line via `\<newline>` continuation supported). Returns
    /// `(table, [cols])` per occurrence.
    fn find_insert_sites_in(src: &str) -> Vec<(String, Vec<String>)> {
        find_sql_literals_starting_with(src, &["\"INSERT INTO "])
            .into_iter()
            .filter_map(|s| parse_insert(&s))
            .collect()
    }

    /// Find `"UPDATE <table> SET c1=v, c2=v, ... [WHERE ...]"` literals
    /// (multi-line via `\<newline>` continuation supported). Returns
    /// `(table, [cols])` per occurrence.
    fn find_update_sites_in(src: &str) -> Vec<(String, Vec<String>)> {
        find_sql_literals_starting_with(src, &["\"UPDATE "])
            .into_iter()
            .filter_map(|s| parse_update(&s))
            .collect()
    }

    /// Parse `INSERT INTO <table> (c1, c2, ...) VALUES (...)`.
    /// Returns `None` for any shape we don't fully understand.
    fn parse_insert(sql: &str) -> Option<(String, Vec<String>)> {
        if !sql.to_ascii_uppercase().starts_with("INSERT INTO ") {
            return None;
        }
        let after = &sql["INSERT INTO ".len()..];
        let table_end = after
            .bytes()
            .position(|b| !(b.is_ascii_alphanumeric() || b == b'_'))?;
        if table_end == 0 {
            return None;
        }
        let table = after[..table_end].to_lowercase();
        let rest = after[table_end..].trim_start();
        if !rest.starts_with('(') {
            return None;
        }
        let cols_body = &rest[1..];
        // Find matching ')' counting depth (the column list can't
        // contain nested parens in practice, but be safe).
        let mut depth = 1i32;
        let mut close = 0;
        for (k, b) in cols_body.bytes().enumerate() {
            match b {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = k;
                        break;
                    }
                }
                _ => {}
            }
        }
        if close == 0 {
            return None;
        }
        let cols_str = &cols_body[..close];
        let cols: Vec<String> = cols_str
            .split(',')
            .filter_map(|c| normalise_column(c.trim()))
            .collect();
        Some((table, cols))
    }

    /// Parse `UPDATE <table> SET c1 = v1, c2 = v2, ... [WHERE ...]`.
    /// Returns `None` for any shape we don't fully understand.
    fn parse_update(sql: &str) -> Option<(String, Vec<String>)> {
        let upper = sql.to_ascii_uppercase();
        if !upper.starts_with("UPDATE ") {
            return None;
        }
        let after = &sql["UPDATE ".len()..];
        let table_end = after
            .bytes()
            .position(|b| !(b.is_ascii_alphanumeric() || b == b'_'))?;
        if table_end == 0 {
            return None;
        }
        let table = after[..table_end].to_lowercase();
        let rest = after[table_end..].trim_start();
        let upper_rest = rest.to_ascii_uppercase();
        if !upper_rest.starts_with("SET ") {
            return None;
        }
        let set_clause = &rest["SET ".len()..];
        // End the SET clause at ` WHERE ` / ` RETURNING ` / end-of-string.
        let upper_set = set_clause.to_ascii_uppercase();
        let mut set_end = set_clause.len();
        for kw in [" WHERE ", " RETURNING "] {
            if let Some(p) = upper_set.find(kw) {
                if p < set_end {
                    set_end = p;
                }
            }
        }
        let set_body = &set_clause[..set_end];
        // Split top-level comma; for each "col = val" chunk grab
        // the column. Parens-depth-aware to survive `col = func(a, b)`.
        let mut cols = Vec::new();
        let bytes = set_body.as_bytes();
        let mut depth = 0i32;
        let mut chunk_start = 0;
        let mut i2 = 0;
        while i2 <= bytes.len() {
            let at_end = i2 == bytes.len();
            let c = if at_end { b',' } else { bytes[i2] };
            if !at_end {
                match c {
                    b'(' => depth += 1,
                    b')' => depth -= 1,
                    _ => {}
                }
            }
            if (c == b',' && depth == 0) || at_end {
                let chunk = &set_body[chunk_start..i2];
                if let Some(eq) = chunk.find('=') {
                    let col_str = chunk[..eq].trim();
                    if let Some(col) = normalise_column(col_str) {
                        cols.push(col);
                    }
                }
                chunk_start = i2 + 1;
            }
            i2 += 1;
        }
        Some((table, cols))
    }

    #[test]
    fn every_inserted_column_is_in_the_migration_schema() {
        let schema = parse_migration_schema();
        assert!(!schema.is_empty(), "parsed empty migration schema");
        let manifest = env!("CARGO_MANIFEST_DIR");
        let src_root = Path::new(manifest).join("src");
        let mut rs_files = Vec::new();
        collect_files_with_ext(&src_root, "rs", &mut rs_files);
        let allowed: HashSet<&str> = ALLOWED_COMPUTED_COLUMNS.iter().copied().collect();
        let mut missing: Vec<String> = Vec::new();
        let mut sites_seen = 0usize;
        for file in &rs_files {
            let src = match std::fs::read_to_string(file) {
                Ok(s) => s,
                Err(_) => continue,
            };
            for (table, cols) in find_insert_sites_in(&src) {
                sites_seen += 1;
                let table_schema = match schema.get(&table) {
                    Some(s) => s,
                    None => continue,
                };
                for col in &cols {
                    if !table_schema.contains(col) && !allowed.contains(col.as_str()) {
                        missing.push(format!(
                            "{}: INSERT INTO {table} ({col}) — column not in migration schema",
                            file.display()
                        ));
                    }
                }
            }
        }
        assert!(sites_seen > 0, "no INSERT sites parsed");
        assert!(
            missing.is_empty(),
            "Columns INSERTed into a table but not declared in any migration ({}): {:?}",
            missing.len(),
            missing
        );
    }

    #[test]
    fn every_updated_column_is_in_the_migration_schema() {
        let schema = parse_migration_schema();
        assert!(!schema.is_empty(), "parsed empty migration schema");
        let manifest = env!("CARGO_MANIFEST_DIR");
        let src_root = Path::new(manifest).join("src");
        let mut rs_files = Vec::new();
        collect_files_with_ext(&src_root, "rs", &mut rs_files);
        let allowed: HashSet<&str> = ALLOWED_COMPUTED_COLUMNS.iter().copied().collect();
        let mut missing: Vec<String> = Vec::new();
        let mut sites_seen = 0usize;
        for file in &rs_files {
            let src = match std::fs::read_to_string(file) {
                Ok(s) => s,
                Err(_) => continue,
            };
            for (table, cols) in find_update_sites_in(&src) {
                sites_seen += 1;
                let table_schema = match schema.get(&table) {
                    Some(s) => s,
                    None => continue,
                };
                for col in &cols {
                    if !table_schema.contains(col) && !allowed.contains(col.as_str()) {
                        missing.push(format!(
                            "{}: UPDATE {table} SET {col}=... — column not in migration schema",
                            file.display()
                        ));
                    }
                }
            }
        }
        assert!(sites_seen > 0, "no UPDATE sites parsed");
        assert!(
            missing.is_empty(),
            "Columns UPDATEd in a table but not declared in any migration ({}): {:?}",
            missing.len(),
            missing
        );
    }

    #[test]
    fn every_selected_column_is_in_the_migration_schema() {
        let schema = parse_migration_schema();
        assert!(
            !schema.is_empty(),
            "parsed empty migration schema — parser is broken"
        );

        let manifest = env!("CARGO_MANIFEST_DIR");
        let src_root = Path::new(manifest).join("src");
        let mut rs_files = Vec::new();
        collect_files_with_ext(&src_root, "rs", &mut rs_files);

        let allowed: HashSet<&str> = ALLOWED_COMPUTED_COLUMNS.iter().copied().collect();
        let mut missing: Vec<String> = Vec::new();
        let mut sites_seen = 0usize;

        for file in &rs_files {
            let src = match std::fs::read_to_string(file) {
                Ok(s) => s,
                Err(_) => continue,
            };
            for (tables, cols) in find_select_sites_in(&src) {
                sites_seen += 1;
                // Build the union of every joined table's columns. If
                // *any* table is unknown to the schema parser, skip
                // the whole site — we'd produce false positives.
                if tables.iter().any(|t| !schema.contains_key(t)) {
                    continue;
                }
                let union: HashSet<&String> = tables
                    .iter()
                    .filter_map(|t| schema.get(t))
                    .flatten()
                    .collect();
                let tables_label = tables.join(",");
                for col in &cols {
                    if !union.contains(col) && !allowed.contains(col.as_str()) {
                        missing.push(format!(
                            "{}: SELECT {col} FROM {tables_label} — column not in any joined table's migration schema",
                            file.display()
                        ));
                    }
                }
            }
        }

        assert!(
            sites_seen > 0,
            "no SELECT sites parsed — the SQL-scan side is broken"
        );
        assert!(
            missing.is_empty(),
            "Columns SELECTed from a table but not declared in any migration ({}): {:?}\n\
             Either add a migration creating the column, or list it in \
             ALLOWED_COMPUTED_COLUMNS with a rationale.",
            missing.len(),
            missing
        );
    }

    /// `db::CRITICAL_COLUMNS` (the startup schema self-check list) is hand-
    /// maintained — tie it to the source of truth so it can't silently drift.
    /// Every entry must (1) be declared in a migration for its table, and
    /// (2) actually be SELECTed from that table by the catalog code. Otherwise
    /// the self-check would alert on a phantom column, or a SELECT could start
    /// depending on a column the self-check never verifies. Closes W-48.
    #[test]
    fn critical_columns_match_migrations_and_selects() {
        let schema = parse_migration_schema();
        let manifest = env!("CARGO_MANIFEST_DIR");
        let catalog = std::fs::read_to_string(Path::new(manifest).join("src/api/catalog.rs"))
            .expect("read catalog.rs");
        let selects = find_select_sites_in(&catalog);
        assert!(
            !selects.is_empty(),
            "no SELECT sites parsed from catalog.rs"
        );

        let mut errors = Vec::new();
        for &(table, cols) in crate::db::CRITICAL_COLUMNS {
            for &col in cols {
                // (1) real migration column for this table
                match schema.get(table) {
                    Some(m) if m.contains(col) => {}
                    _ => errors.push(format!("{table}.{col}: not declared in any migration")),
                }
                // (2) actually SELECTed from this table by the catalog code
                let selected = selects.iter().any(|(tabs, scols)| {
                    tabs.iter().any(|t| t.as_str() == table)
                        && scols.iter().any(|c| c.as_str() == col)
                });
                if !selected {
                    errors.push(format!(
                        "{table}.{col}: in CRITICAL_COLUMNS but no catalog SELECT fetches it"
                    ));
                }
            }
        }
        assert!(
            errors.is_empty(),
            "CRITICAL_COLUMNS drifted from migrations/SELECTs ({}): {:?}",
            errors.len(),
            errors
        );
    }
}

// WASM entry point is now in src/lib.rs via #[wasm_bindgen(start)]
// This file is only used for the native backend (Axum server)
