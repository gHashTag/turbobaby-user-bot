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

fn validate_cart(req: &Cart) -> Result<(), StatusCode> {
    if !req.total.is_finite() || req.total < 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.items.len() > 100 { return Err(StatusCode::BAD_REQUEST); }
    for item in &req.items {
        if item.strain_id.len() > 200 { return Err(StatusCode::BAD_REQUEST); }
        if !item.quantity.is_finite() || item.quantity <= 0.0 || item.quantity > 1_000_000.0 {
            return Err(StatusCode::BAD_REQUEST);
        }
        if !item.price_per_gram.is_finite() || item.price_per_gram < 0.0 || item.price_per_gram > 1_000_000.0 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    Ok(())
}

// Save cart (optional - for future cart persistence feature)
async fn save_cart(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<Cart>,
) -> Result<Json<Value>, StatusCode> {
    check_owner(&headers, &state, req.telegram_id)?;
    check_not_blocked(&state, req.telegram_id).await?;
    validate_cart(&req)?;
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

#[cfg(test)]
mod tests {
    use super::{Cart, CartItem, validate_cart};
    use axum::http::StatusCode;

    fn valid_cart() -> Cart {
        Cart {
            telegram_id: 123,
            items: vec![CartItem {
                strain_id: "strain-1".into(),
                quantity: 1.0,
                price_per_gram: 100.0,
            }],
            total: 100.0,
        }
    }

    #[test]
    fn test_validate_cart_ok() {
        assert!(validate_cart(&valid_cart()).is_ok());
    }

    #[test]
    fn test_validate_cart_total_negative() {
        let mut req = valid_cart();
        req.total = -1.0;
        assert_eq!(validate_cart(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_cart_total_nan() {
        let mut req = valid_cart();
        req.total = f64::NAN;
        assert_eq!(validate_cart(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_cart_too_many_items() {
        let mut req = valid_cart();
        req.items = (0..101).map(|i| CartItem {
            strain_id: format!("strain-{i}"),
            quantity: 1.0,
            price_per_gram: 1.0,
        }).collect();
        assert_eq!(validate_cart(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_cart_strain_id_too_long() {
        let mut req = valid_cart();
        req.items[0].strain_id = "a".repeat(201);
        assert_eq!(validate_cart(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_cart_quantity_zero() {
        let mut req = valid_cart();
        req.items[0].quantity = 0.0;
        assert_eq!(validate_cart(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_cart_quantity_negative() {
        let mut req = valid_cart();
        req.items[0].quantity = -1.0;
        assert_eq!(validate_cart(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_cart_quantity_too_high() {
        let mut req = valid_cart();
        req.items[0].quantity = 2_000_000.0;
        assert_eq!(validate_cart(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_cart_price_negative() {
        let mut req = valid_cart();
        req.items[0].price_per_gram = -1.0;
        assert_eq!(validate_cart(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_cart_price_too_high() {
        let mut req = valid_cart();
        req.items[0].price_per_gram = 2_000_000.0;
        assert_eq!(validate_cart(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }
}