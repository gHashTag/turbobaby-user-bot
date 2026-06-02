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
                let text = format!(
                    "\u{1F6A8} 5xx Error on prod\n\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\u{1F4CD} {} {}\n\u{1F4A5} HTTP {}",
                    safe_method, safe_path, status
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
    info!("✅ Database connected");

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
                    let cache_header = if exact.is_some() && path.contains('-') {
                        "public, max-age=31536000, immutable"
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

    // SPA handler that serves index.html from memory (avoids slow Railway disk I/O)
    let spa_handler = {
        let static_cache = static_cache.clone();
        move || async move {
            if let Some(file) = static_cache.get("index.html") {
                axum::response::Html(String::from_utf8_lossy(&file.raw).to_string())
            } else {
                axum::response::Html("<h1>App not found</h1>".to_string())
            }
        }
    };

    // SPA routes that should return index.html for client-side routing
    let spa_routes = Router::new()
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

// WASM entry point is now in src/lib.rs via #[wasm_bindgen(start)]
// This file is only used for the native backend (Axum server)
