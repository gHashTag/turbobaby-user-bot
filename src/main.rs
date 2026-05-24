// Backend-only modules (not included in WASM builds)
#[cfg(not(target_arch = "wasm32"))]
mod config;
#[cfg(not(target_arch = "wasm32"))]
mod db;
#[cfg(not(target_arch = "wasm32"))]
mod s3;
#[cfg(not(target_arch = "wasm32"))]
mod bot;
#[cfg(not(target_arch = "wasm32"))]
mod api;
#[cfg(not(target_arch = "wasm32"))]
mod ai;
#[cfg(not(target_arch = "wasm32"))]
mod locales;
#[cfg(not(target_arch = "wasm32"))]
pub mod metrics;
#[cfg(not(target_arch = "wasm32"))]
pub mod notify;

// Business logic modules (shared between backend and web)
pub mod trios;

// UI module (WASM only — uses web-sys which doesn't compile for native)
#[cfg(target_arch = "wasm32")]
pub mod ui;


#[cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
#[cfg(not(target_arch = "wasm32"))]
use axum::{Router, routing::get};
#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;
#[cfg(not(target_arch = "wasm32"))]
use teloxide::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use tower_http::cors::{Any, CorsLayer};
#[cfg(not(target_arch = "wasm32"))]
use tower_http::services::{ServeDir, ServeFile};
#[cfg(not(target_arch = "wasm32"))]
use tower_http::set_header::SetResponseHeaderLayer;
#[cfg(not(target_arch = "wasm32"))]
// use tower_http::compression::CompressionLayer;
#[cfg(not(target_arch = "wasm32"))]
use axum::http::{HeaderName, HeaderValue};
use bytes::Bytes;
#[cfg(not(target_arch = "wasm32"))]
use axum::middleware::Next;
#[cfg(not(target_arch = "wasm32"))]
use axum::http::Request;
#[cfg(not(target_arch = "wasm32"))]
use axum::response::IntoResponse;
#[cfg(not(target_arch = "wasm32"))]
use tracing::info;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use teloxide::dispatching::Dispatcher;

#[cfg(not(target_arch = "wasm32"))]
use crate::config::Config;
#[cfg(not(target_arch = "wasm32"))]
use crate::db::Database;
#[cfg(not(target_arch = "wasm32"))]
use crate::api::cache::ETagCache;
#[cfg(not(target_arch = "wasm32"))]
use teloxide::Bot;
#[cfg(not(target_arch = "wasm32"))]
use axum_prometheus::PrometheusMetricLayer;


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

#[cfg(not(target_arch = "wasm32"))]
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

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
        let bot = state.bot.clone();
        let config = state.config.clone();
        let status = resp.status().as_u16();
        tokio::spawn(async move {
            let text = format!(
                "\u{1F6A8} <b>5xx Error on prod</b>\n\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\u{1F4CD} {method} {path}\n\u{1F4A5} HTTP {status}",
                method = html_escape(&method), path = html_escape(&path), status = status
            );
            crate::notify::notify_admins(&bot, &config, &text).await;
        });
    }
    resp
}

#[cfg(not(target_arch = "wasm32"))]
async fn cache_middleware(req: Request<axum::body::Body>, next: Next) -> axum::response::Response {
    let path = req.uri().path().to_string();
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.remove(axum::http::header::CACHE_CONTROL);
    if path.ends_with(".html") || path == "/" || !path.contains('-') {
        h.insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, no-cache, must-revalidate, max-age=0"),
        );
    } else if path.ends_with(".wasm") || path.ends_with(".js") || path.ends_with(".css") {
        // Hashed assets — safe to cache immutably
        h.insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    }
    resp
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<()> {
    // Must be called before ANY rustls usage
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("🤖 Starting Woody Bot (Rust)...");

    let config = Arc::new(Config::from_env()?);
    info!("Environment: {}", if config.is_production { "Production" } else { "Development" });
    info!("Token present: {}", if !config.bot_token.is_empty() { "YES" } else { "NO" });
    info!("Web App URL: {}", config.web_app_url);

    let db = Arc::new(Database::connect(&config.database_url).await?);
    db.run_migrations().await?;
    info!("✅ Database connected");

    let bot = Bot::new(&config.bot_token);
    let bot_arc = Arc::new(bot.clone());
    let bot_arc_for_state = bot_arc.clone();
    let db_for_bot = db.clone();
    let config_for_bot = config.clone();
    tokio::spawn(async move {
        use teloxide::update_listeners::Polling;
        use teloxide::types::AllowedUpdate;
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
            .delete_webhook().await
            .build();
        Dispatcher::builder(bot.clone(), handler)
            .dependencies(dptree::deps![Arc::clone(&db_for_bot), Arc::clone(&config_for_bot)])
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
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST, axum::http::Method::PUT, axum::http::Method::DELETE])
        .allow_headers(Any)
        .expose_headers(Any);

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
    let assets_cache_layer = || SetResponseHeaderLayer::if_not_present(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    // HTML must NEVER be cached — Telegram WebApp aggressively keeps the
    // index.html in cache, which breaks deploys (new WASM hash never
    // fetched). `no-store` forces a re-validate on every load.
    let html_no_cache_layer = || SetResponseHeaderLayer::overriding(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate, max-age=0"),
    );

    // Load dist files into memory to bypass slow Railway disk I/O
    // for the WASM bundle (~4 MB) and hashed JS/CSS.
    let mut static_cache: std::collections::HashMap<String, CachedFile> =
        std::collections::HashMap::new();

    fn content_type_for(path: &std::path::Path) -> &'static str {
        match path.extension().and_then(|e| e.to_str()) {
            Some("html") => "text/html",
            Some("js") => "text/javascript",
            Some("css") => "text/css",
            Some("wasm") => "application/wasm",
            Some("svg") => "image/svg+xml",
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            _ => "application/octet-stream",
        }
    }

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
        let key = path.strip_prefix("dist/").unwrap_or(path)
            .to_string_lossy().to_string();
        let raw = match load_file(path) {
            Some(b) => b,
            None => return,
        };
        let br_path = path.with_extension(
            format!("{}.{}", path.extension().unwrap_or_default().to_string_lossy(), "br")
        );
        let gz_path = path.with_extension(
            format!("{}.{}", path.extension().unwrap_or_default().to_string_lossy(), "gz")
        );
        let br = load_file(&br_path);
        let gzip = load_file(&gz_path);
        cache.insert(key, CachedFile {
            raw,
            gzip,
            br,
            content_type: content_type_for(path),
        });
    }

    match std::fs::read_dir("dist") {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if path.file_name() != Some(std::ffi::OsStr::new("assets")) {
                        if let Ok(sub) = std::fs::read_dir(&path) {
                            for sub_entry in sub.flatten() {
                                let sub_path = sub_entry.path();
                                if sub_path.is_file() {
                                    add_to_cache(&mut static_cache, &sub_path);
                                }
                            }
                        }
                    }
                } else {
                    add_to_cache(&mut static_cache, &path);
                }
            }
        }
        Err(e) => {
            println!("ERROR: failed to read_dir(dist): {}", e);
        }
    }
    println!("DEBUG: Cached {} dist files in memory", static_cache.len());

    // /assets, /styles, /images — stable URLs, day-long browser cache.
    let static_assets = Router::new()
        .nest_service("/styles", ServeDir::new("styles"))
        .nest_service("/assets", ServeDir::new("assets"))
        .nest_service("/images", ServeDir::new("assets"))
        .nest_service("/uploads", ServeDir::new("/data/uploads"))
        .layer(assets_cache_layer());

    // Trunk emits hashed JS/WASM/CSS at the root of dist/ with absolute paths
    // (e.g. /woody-weed-bot-<hash>.js). We serve dist/ as the root static dir;
    // when a request doesn't match a file, ServeDir falls back to index.html
    // (which is NOT hashed, so we need to override caching for it separately).
    //
    // In-memory fallback handler using the pre-loaded static_cache.
    let static_cache = std::sync::Arc::new(static_cache);

    fn pick_encoding(headers: &axum::http::HeaderMap) -> Option<&'static str> {
        let accept = headers.get(axum::http::header::ACCEPT_ENCODING)?
            .to_str().ok()?;
        if accept.contains("br") {
            Some("br")
        } else if accept.contains("gzip") {
            Some("gzip")
        } else {
            None
        }
    }

    let serve_dist = {
        let static_cache = static_cache.clone();
        move |uri: axum::http::Uri, headers: axum::http::HeaderMap| {
            let static_cache = static_cache.clone();
            async move {
                let path = uri.path().trim_start_matches('/');
                if path.contains("..") {
                    return (axum::http::StatusCode::NOT_FOUND, "Not found").into_response();
                }
                let cached = static_cache.get(path)
                    .or_else(|| static_cache.get("index.html"));
                if let Some(file) = cached {
                    let (body, encoding) = match pick_encoding(&headers) {
                        Some("br") if file.br.is_some() => (file.br.clone().unwrap(), Some("br")),
                        Some("gzip") if file.gzip.is_some() => (file.gzip.clone().unwrap(), Some("gzip")),
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
        .layer(html_no_cache_layer());

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

    let app = Router::new()
        .merge(openapi_router)
        // CORS layer MUST be first!
        .layer(cors)
        .layer(ngrok_bypass)
        // Backend API routes (must be before static to avoid conflicts)
        .merge(api::router(app_state.clone()).layer(axum::middleware::from_fn_with_state(app_state.clone(), alert_5xx_middleware)))
        // Prometheus /metrics endpoint — placed after API routes so prometheus_layer counts API calls
        .route("/metrics", get(|| async move { metric_handle.render() }))
        .layer(prometheus_layer)
        // SPA routes - serve index.html for client-side routing
        .merge(spa_routes)
        // SPA routes - these should be served by the fallback
        .merge(static_assets)
        // Debug endpoint to inspect static cache
        .route("/api/debug/dist", get({
            let keys: Vec<String> = static_cache.keys().cloned().collect();
            move || async move {
                axum::Json(serde_json::json!({
                    "count": keys.len(),
                    "keys": keys,
                }))
            }
        }))
        // Test endpoint: return 4 MB of zeros to check if Railway throttles large bodies
        .route("/api/debug/large", get(|| async move {
            let zeros = vec![0u8; 4 * 1024 * 1024];
            ([(axum::http::header::CONTENT_TYPE, "application/octet-stream")], zeros)
        }))
        // Single top-level fallback: serve hashed bundles from dist/, fall
        // back to SPA index.html if path not found. We apply no-store cache
        // headers globally on the fallback; immutable cache for hashed
        // assets would be ideal but requires per-file logic. Telegram WebApp
        // cache busting is the priority — no-store keeps deploys landing.
        .fallback(serve_dist);
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
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("🛑 Shutdown signal received, starting graceful shutdown...");
}

// WASM entry point is now in src/lib.rs via #[wasm_bindgen(start)]
// This file is only used for the native backend (Axum server)