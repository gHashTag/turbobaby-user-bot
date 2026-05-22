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
use tower_http::compression::CompressionLayer;
#[cfg(not(target_arch = "wasm32"))]
use axum::http::{HeaderName, HeaderValue};
#[cfg(not(target_arch = "wasm32"))]
use axum::middleware::Next;
#[cfg(not(target_arch = "wasm32"))]
use axum::http::Request;
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
async fn spa_handler() -> impl axum::response::IntoResponse {
    let html = tokio::fs::read_to_string("dist/index.html").await.unwrap_or_else(|_| "<h1>App not found</h1>".to_string());
    axum::response::Html(html)
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
        let handler = bot::create_handler();
        Dispatcher::builder(bot.clone(), handler)
            .dependencies(dptree::deps![Arc::clone(&db_for_bot), Arc::clone(&config_for_bot)])
            .build()
            .dispatch()
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
    // Brotli / Gzip / Zstd compression for everything (WASM 2 MB → ~600 KB).
    // Default predicate skips application/wasm, so use a permissive
    // size-only predicate (anything > 1 KB gets compressed).
    use tower_http::compression::predicate::SizeAbove;
    let compression = CompressionLayer::new()
        .br(true)
        .gzip(true)
        .zstd(true)
        .compress_when(SizeAbove::new(1024));

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
    // Single fallback strategy: one Router with one fallback_service on dist/
    // that itself falls back to index.html via ServeDir::not_found_service.
    let spa_index = ServeFile::new("dist/index.html");
    let dist_service = ServeDir::new("dist").not_found_service(spa_index);

    // SPA routes that should return index.html for client-side routing
    let spa_routes = Router::new()
        .route("/menu", get(spa_handler))
        .route("/sets", get(spa_handler))
        .route("/sommelier", get(spa_handler))
        .route("/accessories", get(spa_handler))
        .route("/tea", get(spa_handler))
        .route("/cart", get(spa_handler))
        .route("/checkout", get(spa_handler))
        .route("/success", get(spa_handler))
        .route("/orders", get(spa_handler))
        .route("/profile", get(spa_handler))
        .route("/garden", get(spa_handler))
        .route("/quest", get(spa_handler))
        .route("/game", get(spa_handler))
        .route("/referrals", get(spa_handler))
        .route("/treasure-hunt", get(spa_handler))
        .route("/ar-hunt", get(spa_handler))
        .route("/location-quest", get(spa_handler))
        .route("/tech-tree", get(spa_handler))
        .route("/admin", get(spa_handler))
        .layer(html_no_cache_layer());

    // Swagger UI — served at /swagger-ui/ (utoipa-swagger-ui 8.x, compatible with axum 0.7)
    #[cfg(feature = "utoipa-swagger-ui")]
    let swagger_router: Router = {
        use utoipa::OpenApi;
        let ui = utoipa_swagger_ui::SwaggerUi::new("/swagger-ui")
            .url("/api-docs/openapi.json", crate::api::openapi::ApiDoc::openapi());
        Router::from(ui)
    };
    #[cfg(not(feature = "utoipa-swagger-ui"))]
    let swagger_router: Router = Router::new();

    let app = Router::new()
        .merge(swagger_router)
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
        // Single top-level fallback: serve hashed bundles from dist/, fall
        // back to SPA index.html if path not found. We apply no-store cache
        // headers globally on the fallback; immutable cache for hashed
        // assets would be ideal but requires per-file logic. Telegram WebApp
        // cache busting is the priority — no-store keeps deploys landing.
        .fallback_service(
            tower::ServiceBuilder::new()
                .layer(axum::middleware::from_fn(cache_middleware))
                .service(dist_service),
        )
        // Compression applies to *all* responses (API JSON, WASM, HTML, CSS).
        .layer(compression);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("🚀 HTTP server listening on {}", addr);
    info!("🎨 UI assets served from: styles/, assets/");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// WASM entry point is now in src/lib.rs via #[wasm_bindgen(start)]
// This file is only used for the native backend (Axum server)