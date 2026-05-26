use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::error;

use crate::api::auth::check_admin;
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

async fn create_order(
    State(state): State<AppState>,
    Json(req): Json<CreateOrderRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Validation
    if req.items.is_empty() {
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
    // Sanity-check frontend math: total must equal subtotal minus bonus (within 1 satang).
    let expected_total = (req.subtotal - bonus_used).max(0.0);
    if (req.total - expected_total).abs() > 0.01 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let items_json = serde_json::to_value(&req.items)
        .map_err(|e| { error!("items serialization failed: {}", e); StatusCode::BAD_REQUEST })?;

    // Atomic transaction: deduct bonus (if any) and insert order together.
    let mut client = state.db.pool.get().await.map_err(|e| { error!("create_order pool error: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let tx = client.transaction().await.map_err(|e| { error!("create_order tx error: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    if bonus_used > 0.0 {
        if let Some(tid) = req.telegram_id {
            let row = tx.query_opt(
                "SELECT bonus_balance::float8 FROM loyalty_profiles WHERE telegram_id = $1",
                &[&tid],
            ).await.map_err(|e| { error!("bonus check query error: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
            let balance: f64 = row.and_then(|r| r.try_get::<_, Option<f64>>(0).ok().flatten()).unwrap_or(0.0);
            if balance < bonus_used {
                let _ = tx.rollback().await;
                return Err(StatusCode::BAD_REQUEST);
            }
            tx.execute(
                "UPDATE loyalty_profiles SET bonus_balance = GREATEST(0, bonus_balance - $1) WHERE telegram_id = $2",
                &[&bonus_used, &tid],
            ).await.map_err(|e| { error!("bonus deduction error: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
        } else {
            let _ = tx.rollback().await;
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    tx.execute(
        "INSERT INTO orders (id, telegram_id, customer_name, customer_phone, customer_telegram, items, subtotal, bonus_used, total, status, shop_id) VALUES ($1, $2, $3, $4, $5, $6, $7::float8, $8::float8, $9::float8, 'pending', $10)",
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

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

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
        source, items_text, subtotal, bonus_used, total, &order_id[order_id.len().saturating_sub(6)..]
    );

    let btns = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("✅ Confirm", format!("confirm_{}", order_id)),
        InlineKeyboardButton::callback("❌ Reject", format!("reject_{}", order_id)),
    ]]);

    for admin_id in &config.admin_ids {
        let _ = bot.send_message(teloxide::types::ChatId(*admin_id), &text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(btns.clone())
            .await;
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
    check_admin(&headers, &state)?;
    const VALID_STATUSES: &[&str] = &["pending", "confirmed", "completed", "rejected", "ready", "cancelled"];
    if !VALID_STATUSES.contains(&req.status.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.status == "completed" {
        let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
        let exists = client.query_opt("SELECT 1 FROM orders WHERE id = $1", &[&id]).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
        if exists.is_none() {
            return Err(StatusCode::NOT_FOUND);
        }
        if let Err(e) = crate::db::orders::complete_order_and_update_loyalty(&state.db.pool, &id).await {
            tracing::error!("update_order_status: complete_order_and_update_loyalty error: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    } else {
        let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
        let rows = client.execute("UPDATE orders SET status = $1 WHERE id = $2", &[&req.status, &id])
            .await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
        if rows == 0 {
            return Err(StatusCode::NOT_FOUND);
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
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let rows = client.query(
        "SELECT id, telegram_id, customer_name, customer_phone, customer_telegram, items, subtotal::float8, bonus_used::float8, total::float8, status, shop_id, created_at FROM orders WHERE telegram_id = $1 ORDER BY created_at DESC LIMIT 50",
        &[&telegram_id],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "orders": rows.iter().map(Order::from_row).collect::<Vec<_>>() })))
}
