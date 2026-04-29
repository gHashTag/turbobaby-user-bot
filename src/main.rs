mod config;
mod db;
mod ai;
mod locales;
mod s3;
mod bot;
mod api;

use anyhow::Result;
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use teloxide::prelude::*;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::info;

use crate::config::Config;
use crate::db::Database;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub bot: Bot,
    pub config: Arc<Config>,
}

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

    let state = AppState {
        db: db.clone(),
        bot: bot.clone(),
        config: config.clone(),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(api::router(state.clone()))
        .nest_service("/uploads", ServeDir::new("/data/uploads"))
        .layer(cors);

    let bot_state = state.clone();
    tokio::spawn(async move {
        let handler = bot::create_handler();
        Dispatcher::builder(bot_state.bot.clone(), handler)
            .dependencies(dptree::deps![Arc::clone(&bot_state.db), Arc::clone(&bot_state.config)])
            .build()
            .dispatch()
            .await;
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("🚀 HTTP server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
