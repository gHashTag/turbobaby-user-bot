use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::api::auth::{check_owner, check_not_blocked};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CartItem {
    pub strain_id: String,
    pub quantity: f64,
    pub price_per_gram: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Cart {
    pub telegram_id: i64,
    pub items: Vec<CartItem>,
    pub total: f64,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/cart", get(get_cart).post(save_cart).delete(clear_cart))
        .route("/cart/:telegram_id", get(get_cart_by_id))
}

// Get cart (Telegram WebApp stores cart locally, this is for persistence)
async fn get_cart(
    axum::extract::Query(_params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    // For Telegram Mini Apps, cart is stored in localStorage on client
    // This endpoint returns an empty cart - actual cart lives on the client
    Ok(json!({"items": [], "total": 0}).into())
}

// Save cart (optional - for future cart persistence feature)
async fn save_cart(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<Cart>,
) -> Result<Json<Value>, StatusCode> {
    check_owner(&headers, &state, req.telegram_id)?;
    check_not_blocked(&state, req.telegram_id).await?;
    // Cart persistence can be implemented here later
    Ok(json!({"success": true}).into())
}

// Clear cart
async fn clear_cart(
    axum::extract::Query(_params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    Ok(json!({"success": true}).into())
}

// Get cart by telegram_id (owner only)
async fn get_cart_by_id(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;
    // Cart data not persisted on backend yet
    Ok(json!({"telegram_id": telegram_id, "items": [], "total": 0}).into())
}