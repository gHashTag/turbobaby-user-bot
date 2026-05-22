pub mod auth;
pub mod orders;
pub mod strains;
pub mod loyalty;
pub mod admin;
pub mod upload;
pub mod catalog;
pub mod quest;
pub mod happy_hour;
pub mod garden;
pub mod referrals;
pub mod tech_tree;
pub mod cart;
pub mod cache;
#[cfg(feature = "utoipa")]
pub mod openapi;

use axum::{
    Router,
    routing::get,
    response::Json,
};
use serde_json::{json, Value};
use crate::AppState;

pub fn router(state: crate::AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .nest("/api", api_routes(state.clone()))
}

fn api_routes(state: AppState) -> Router {
    Router::new()
        .route("/ping", get(ping_handler))
        .merge(orders::routes())
        .merge(strains::routes())
        .merge(loyalty::routes())
        .merge(admin::routes())
        .merge(upload::routes())
        .merge(catalog::routes())
        .merge(quest::routes())
        .merge(happy_hour::routes())
        .merge(garden::routes())
        .merge(referrals::routes())
        .merge(tech_tree::routes())
        .merge(cart::routes())
        .with_state(state)
}

async fn ping_handler() -> Json<Value> {
    Json(json!({"status": "ok", "service": "woody-weed-bot"}))
}

async fn health_handler() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "woody-weed-bot" }))
}
