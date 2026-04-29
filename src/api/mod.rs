pub mod orders;
pub mod strains;
pub mod loyalty;
pub mod admin;
pub mod upload;
pub mod catalog;
pub mod quest;
pub mod happy_hour;

use axum::{
    Router,
    routing::get,
    response::Json,
};
use serde_json::{json, Value};
use crate::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .nest("/api", api_routes(state.clone()))
}

fn api_routes(state: AppState) -> Router {
    Router::new()
        .merge(orders::routes())
        .merge(strains::routes())
        .merge(loyalty::routes())
        .merge(admin::routes())
        .merge(upload::routes())
        .merge(catalog::routes())
        .merge(quest::routes())
        .merge(happy_hour::routes())
        .with_state(state)
}

async fn health_handler() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "woody-weed-bot" }))
}
