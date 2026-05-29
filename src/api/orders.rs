use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::error;

use crate::api::auth::{check_admin, check_not_blocked};
use crate::AppState;
use crate::db::orders::{Order, OrderItem};

#[derive(Debug, Deserialize)]
pub struct CreateOrderRequest {
    pub telegram_id: Option<i64>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub customer_telegram: Option<String>,
    pub items: Vec<OrderItem>,
    pub subtotal: f64,
    pub bonus_used: Option<f64>,
    pub total: f64,
    pub shop_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrderStatusRequest {
    pub status: String,
    #[allow(dead_code)]
    pub admin_telegram_id: Option<i64>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/orders", post(create_order))
        .route("/orders", get(get_orders))
        .route("/orders/:id", get(get_order))
        .route("/orders/:id/status", put(update_order_status))
        .route("/orders/user/:telegram_id", get(get_user_orders))
}

/// Validates a CreateOrderRequest. Returns the sanitized bonus_used on success.
fn validate_create_order(req: &CreateOrderRequest) -> Result<f64, StatusCode> {
    if let Some(ref name) = req.customer_name { if name.len() > 200 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref phone) = req.customer_phone { if phone.len() > 50 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref tg) = req.customer_telegram { if tg.len() > 100 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref shop_id) = req.shop_id { if shop_id.len() > 200 { return Err(StatusCode::BAD_REQUEST); } }
    if req.items.is_empty() || req.items.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.items.iter().any(|i| {
        i.strain_id.as_ref().is_some_and(|n| n.len() > 200)
        || i.strain_name.as_ref().is_some_and(|n| n.len() > 200)
        || i.accessory_id.as_ref().is_some_and(|n| n.len() > 200)
        || i.accessory_name.as_ref().is_some_and(|n| n.len() > 200)
        || i.tea_id.as_ref().is_some_and(|n| n.len() > 200)
        || i.tea_name.as_ref().is_some_and(|n| n.len() > 200)
        || i.set_id.as_ref().is_some_and(|n| n.len() > 200)
        || i.set_name.as_ref().is_some_and(|n| n.len() > 200)
    }) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.items.iter().any(|i| !i.quantity.is_finite() || i.quantity <= 0.0) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !req.total.is_finite() || req.total < 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !req.subtotal.is_finite() || req.subtotal < 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let bonus_used = req.bonus_used.unwrap_or(0.0).max(0.0);
    if !bonus_used.is_finite() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if bonus_used > req.subtotal + 0.01 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let expected_total = (req.subtotal - bonus_used).max(0.0);
    if (req.total - expected_total).abs() > 0.01 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(bonus_used)
}

async fn create_order(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<CreateOrderRequest>,
) -> Result<Json<Value>, StatusCode> {
    // If telegram_id is provided, verify ownership and blocked status.
    if let Some(tid) = req.telegram_id {
        crate::api::auth::check_owner(&headers, &state, tid)?;
        check_not_blocked(&state, tid).await?;
    }

    let bonus_used = validate_create_order(&req)?;

    let id = uuid::Uuid::new_v4().to_string();
    let items_json = serde_json::to_value(&req.items)
        .map_err(|e| { error!("items serialization failed: {}", e); StatusCode::BAD_REQUEST })?;

    // Atomic transaction: rate-limit check, bonus deduction, and insert order together.
    let mut client = state.db.pool.get().await.map_err(|e| { error!("create_order pool error: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let tx = client.transaction().await.map_err(|e| { error!("create_order tx error: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    // Serialize order creation per user to close the rate-limit race window.
    if let Some(tid) = req.telegram_id {
        tx.execute("SELECT pg_advisory_xact_lock($1)", &[&tid])
            .await
            .map_err(|e| { error!("advisory lock error: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    }

    // Rate-limit inside tx to close the race window.
    if let Some(tid) = req.telegram_id {
        let recent = tx.query_opt(
            "SELECT 1 FROM orders WHERE telegram_id = $1 AND created_at > NOW() - INTERVAL '1 minute' LIMIT 1",
            &[&tid],
        ).await.map_err(|e| { error!("rate-limit check error: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
        if recent.is_some() {
            if let Err(e) = tx.rollback().await { tracing::error!("create_order rollback error: {}", e); }
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    // Atomic bonus deduction: UPDATE with built-in balance guard.
    if bonus_used > 0.0 {
        if let Some(tid) = req.telegram_id {
            let deducted = tx.execute(
                "UPDATE loyalty_profiles SET bonus_balance = GREATEST(0, bonus_balance - $1) WHERE telegram_id = $2 AND bonus_balance >= $1",
                &[&bonus_used, &tid],
            ).await.map_err(|e| { error!("bonus deduction error: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
            if deducted == 0 {
                if let Err(e) = tx.rollback().await { tracing::error!("create_order rollback error: {}", e); }
                return Err(StatusCode::BAD_REQUEST);
            }
        } else {
            if let Err(e) = tx.rollback().await { tracing::error!("create_order rollback error: {}", e); }
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    tx.execute(
        "INSERT INTO orders (id, telegram_id, customer_name, customer_phone, customer_telegram, items, subtotal, bonus_used, total, status, shop_id) VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7::float8, $8::float8, $9::float8, 'pending', $10)",
        &[&id, &req.telegram_id, &req.customer_name, &req.customer_phone, &req.customer_telegram, &items_json, &req.subtotal, &bonus_used, &req.total, &req.shop_id],
    ).await.map_err(|e| { error!("create_order insert error: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    if let Err(e) = tx.commit().await {
        error!("create_order commit error: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    crate::metrics::order_created();

    let bot = state.bot.clone();
    let config = state.config.clone();
    let order_id = id.clone();
    let items_v = items_json.clone();
    tokio::spawn(async move {
        notify_admins(&bot, &config, &order_id, &req.customer_name, &req.customer_telegram, &items_v, req.subtotal, bonus_used, req.total).await;
    });

    Ok(Json(json!({ "success": true, "order_id": id })))
}

use crate::util::html_escape;

#[allow(clippy::too_many_arguments)]
async fn notify_admins(
    bot: &teloxide::Bot,
    config: &crate::config::Config,
    order_id: &str,
    customer_name: &Option<String>,
    customer_telegram: &Option<String>,
    items: &Value,
    subtotal: f64,
    bonus_used: f64,
    total: f64,
) {
    use teloxide::prelude::*;
    use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

    let items_text = items.as_array().map(|arr| {
        arr.iter().map(|item| {
            let name = item["strain_name"].as_str()
                .or(item["accessory_name"].as_str())
                .or(item["tea_name"].as_str())
                .or(item["set_name"].as_str())
                .unwrap_or("?");
            let qty = item["quantity"].as_f64().unwrap_or(0.0);
            format!("  • {} × {}g", html_escape(name), qty)
        }).collect::<Vec<_>>().join("\n")
    }).unwrap_or_default();

    let source = customer_telegram.as_ref()
        .map(|t| format!("@{}", html_escape(t)))
        .or_else(|| customer_name.as_ref().map(|n| html_escape(n)))
        .unwrap_or_else(|| "Anonymous".into());

    let text = format!(
        "🚨 <b>New Order!</b>\n━━━━━━━━━━━━━━━━\n👤 {}\n📦 Items:\n{}\n━━━━━━━━━━━━━━━━\n💰 Subtotal: {} ฿\n🎁 Bonus: -{} ฿\n💳 Total: {} ฿\n🔖 #{}",
        source, items_text, subtotal, bonus_used, total, html_escape(&order_id[order_id.len().saturating_sub(6)..])
    );

    let btns = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("✅ Confirm", format!("confirm_{}", order_id)),
        InlineKeyboardButton::callback("❌ Reject", format!("reject_{}", order_id)),
    ]]);

    for admin_id in &config.admin_ids {
        if let Err(e) = bot.send_message(teloxide::types::ChatId(*admin_id), &text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(btns.clone())
            .await
        {
            tracing::warn!("notify_admins (order) failed for admin_id={}: {}", admin_id, e);
        }
    }
}

async fn get_orders(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let limit = params.get("limit").and_then(|v| v.parse::<i64>().ok()).unwrap_or(100).clamp(1, 500);
    let offset = params.get("offset").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0).max(0);
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let rows = client.query(
        "SELECT id, telegram_id, customer_name, customer_phone, customer_telegram, items, subtotal::float8, bonus_used::float8, total::float8, status, shop_id, created_at FROM orders ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        &[&limit, &offset],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let orders: Vec<Order> = rows.iter().map(Order::from_row).collect();
    Ok(Json(json!({ "orders": orders })))
}

async fn get_order(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 { return Err(StatusCode::BAD_REQUEST); }
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let row = client.query_opt(
        "SELECT id, telegram_id, customer_name, customer_phone, customer_telegram, items, subtotal::float8, bonus_used::float8, total::float8, status, shop_id, created_at FROM orders WHERE id = $1",
        &[&id],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    match row {
        Some(r) => Ok(Json(json!({ "order": Order::from_row(&r) }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn update_order_status(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateOrderStatusRequest>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 { return Err(StatusCode::BAD_REQUEST); }
    check_admin(&headers, &state)?;
    if req.status.len() > 50 { return Err(StatusCode::BAD_REQUEST); }
    const VALID_STATUSES: &[&str] = &["pending", "confirmed", "completed", "rejected", "ready", "cancelled"];
    if !VALID_STATUSES.contains(&req.status.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let current_status: Option<String> = client.query_opt("SELECT status FROM orders WHERE id = $1", &[&id])
        .await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?
        .map(|r| r.try_get("status").unwrap_or_default());
    match current_status {
        Some(ref current) if (current == "completed" || current == "rejected" || current == "cancelled")
            && req.status != *current => {
                return Err(StatusCode::BAD_REQUEST);
            }
        None => return Err(StatusCode::NOT_FOUND),
        _ => {}
    }

    if req.status == "completed" {
        if let Err(e) = crate::db::orders::complete_order_and_update_loyalty(&state.db.pool, &id).await {
            tracing::error!("update_order_status: complete_order_and_update_loyalty error: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    } else {
        let mut client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
        if req.status == "rejected" {
            let tx = client.transaction().await.map_err(|e| { tracing::error!("DB tx error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
            if let Some(r) = tx.query_opt("SELECT telegram_id, bonus_used::float8, status FROM orders WHERE id = $1 FOR UPDATE", &[&id]).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })? {
                let current_status: String = r.try_get("status").unwrap_or_default();
                if current_status != "rejected" && current_status != "completed" {
                    let bonus: f64 = r.try_get::<_, f64>("bonus_used").unwrap_or(0.0);
                    let tid: Option<i64> = r.try_get("telegram_id").ok().flatten();
                    if bonus > 0.0 {
                        if let Some(tid) = tid {
                            tx.execute(
                                "INSERT INTO loyalty_profiles (telegram_id, bonus_balance, total_spent) VALUES ($1, 0, 0) ON CONFLICT (telegram_id) DO NOTHING",
                                &[&tid],
                            ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
                            tx.execute(
                                "UPDATE loyalty_profiles SET bonus_balance = bonus_balance + $1 WHERE telegram_id = $2",
                                &[&bonus, &tid],
                            ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
                        }
                    }
                }
            }
            let rows = tx.execute("UPDATE orders SET status = $1 WHERE id = $2", &[&req.status, &id])
                .await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
            if rows == 0 {
                if let Err(e) = tx.rollback().await {
                    tracing::error!("update_order_status rollback error: {:?}", e);
                }
                return Err(StatusCode::NOT_FOUND);
            }
            tx.commit().await.map_err(|e| { tracing::error!("DB commit error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
        } else {
            let rows = client.execute("UPDATE orders SET status = $1 WHERE id = $2", &[&req.status, &id])
                .await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
            if rows == 0 {
                return Err(StatusCode::NOT_FOUND);
            }
        }
    }
    Ok(Json(json!({ "success": true })))
}

async fn get_user_orders(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    crate::api::auth::check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let rows = client.query(
        "SELECT id, telegram_id, customer_name, customer_phone, customer_telegram, items, subtotal::float8, bonus_used::float8, total::float8, status, shop_id, created_at FROM orders WHERE telegram_id = $1 ORDER BY created_at DESC LIMIT 50",
        &[&telegram_id],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "orders": rows.iter().map(Order::from_row).collect::<Vec<_>>() })))
}

#[cfg(test)]
mod tests {
    use super::{CreateOrderRequest, validate_create_order};
    use axum::http::StatusCode;
    use crate::db::orders::OrderItem;

    fn valid_req() -> CreateOrderRequest {
        CreateOrderRequest {
            telegram_id: Some(1),
            customer_name: Some("Alice".into()),
            customer_phone: Some("+123".into()),
            customer_telegram: Some("alice".into()),
            items: vec![OrderItem {
                strain_id: Some("s1".into()),
                strain_name: Some("Indica".into()),
                accessory_id: None,
                accessory_name: None,
                tea_id: None,
                tea_name: None,
                set_id: None,
                set_name: None,
                quantity: 1.0,
                is_set: None,
                is_accessory: None,
                is_tea: None,
                is_tea_set: None,
            }],
            subtotal: 100.0,
            bonus_used: Some(10.0),
            total: 90.0,
            shop_id: None,
        }
    }

    #[test]
    fn test_validate_ok() {
        let req = valid_req();
        assert_eq!(validate_create_order(&req).unwrap(), 10.0);
    }

    #[test]
    fn test_validate_no_bonus() {
        let mut req = valid_req();
        req.bonus_used = None;
        req.total = 100.0;
        assert_eq!(validate_create_order(&req).unwrap(), 0.0);
    }

    #[test]
    fn test_validate_empty_items() {
        let mut req = valid_req();
        req.items = vec![];
        assert_eq!(validate_create_order(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_too_many_items() {
        let mut req = valid_req();
        req.items = (0..101).map(|_| OrderItem {
            strain_id: Some("s".into()),
            strain_name: Some("X".into()),
            accessory_id: None,
            accessory_name: None,
            tea_id: None,
            tea_name: None,
            set_id: None,
            set_name: None,
            quantity: 1.0,
            is_set: None,
            is_accessory: None,
            is_tea: None,
            is_tea_set: None,
        }).collect();
        assert_eq!(validate_create_order(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_name_too_long() {
        let mut req = valid_req();
        req.customer_name = Some("a".repeat(201));
        assert_eq!(validate_create_order(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_phone_too_long() {
        let mut req = valid_req();
        req.customer_phone = Some("a".repeat(51));
        assert_eq!(validate_create_order(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_telegram_too_long() {
        let mut req = valid_req();
        req.customer_telegram = Some("a".repeat(101));
        assert_eq!(validate_create_order(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_shop_id_too_long() {
        let mut req = valid_req();
        req.shop_id = Some("a".repeat(201));
        assert_eq!(validate_create_order(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_strain_id_too_long() {
        let mut req = valid_req();
        req.items[0].strain_id = Some("a".repeat(201));
        assert_eq!(validate_create_order(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_negative_total() {
        let mut req = valid_req();
        req.total = -1.0;
        assert_eq!(validate_create_order(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_nan_total() {
        let mut req = valid_req();
        req.total = f64::NAN;
        assert_eq!(validate_create_order(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_negative_quantity() {
        let mut req = valid_req();
        req.items[0].quantity = -1.0;
        assert_eq!(validate_create_order(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_bonus_exceeds_subtotal() {
        let mut req = valid_req();
        req.bonus_used = Some(101.0);
        req.total = -1.0; // will fail before math check, but let's set valid total
        req.total = 0.0;
        assert_eq!(validate_create_order(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_total_mismatch() {
        let mut req = valid_req();
        req.total = 95.0; // expected 90.0
        assert_eq!(validate_create_order(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_total_tolerance() {
        let mut req = valid_req();
        req.total = 90.009; // within 0.01 of expected 90.0
        assert_eq!(validate_create_order(&req).unwrap(), 10.0);
    }
}
