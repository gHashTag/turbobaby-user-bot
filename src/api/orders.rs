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
    let id = uuid::Uuid::new_v4().to_string();
    let items_json = serde_json::to_value(&req.items).unwrap_or(json!([]));
    let bonus_used = req.bonus_used.unwrap_or(0.0);

    // BUG-3 fix via SeaORM: subtotal/bonus_used/total могут быть NUMERIC на проде.
    // sqlx + ::float8 каст и ::jsonb cast решают все варианты.
    let items_str = items_json.to_string();
    use sea_orm::{Statement, DbBackend, ConnectionTrait};
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO orders (id, telegram_id, customer_name, customer_phone, customer_telegram, items, subtotal, bonus_used, total, status, shop_id) VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7::float8, $8::float8, $9::float8, 'pending', $10)",
        [id.clone().into(), req.telegram_id.into(), req.customer_name.clone().into(), req.customer_phone.clone().into(), req.customer_telegram.clone().into(), items_str.into(), req.subtotal.into(), bonus_used.into(), req.total.into(), req.shop_id.clone().into()],
    );
    state.db.orm.execute(stmt).await
        .map_err(|e| { error!("create_order sea-orm: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

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
            format!("  • {} × {}g", name, qty)
        }).collect::<Vec<_>>().join("\n")
    }).unwrap_or_default();

    let source = customer_telegram.as_ref()
        .map(|t| format!("@{}", t))
        .or_else(|| customer_name.clone())
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
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = client.query(
        "SELECT id, telegram_id, customer_name, customer_phone, customer_telegram, items, subtotal, bonus_used, total, status, shop_id, created_at FROM orders ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        &[&limit, &offset],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let orders: Vec<Order> = rows.iter().map(Order::from_row).collect();
    Ok(Json(json!({ "orders": orders })))
}

async fn get_order(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = client.query_opt(
        "SELECT id, telegram_id, customer_name, customer_phone, customer_telegram, items, subtotal, bonus_used, total, status, shop_id, created_at FROM orders WHERE id = $1",
        &[&id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
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
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute("UPDATE orders SET status = $1 WHERE id = $2", &[&req.status, &id])
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn get_user_orders(State(state): State<AppState>, Path(telegram_id): Path<i64>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = client.query(
        "SELECT id, telegram_id, customer_name, customer_phone, customer_telegram, items, subtotal, bonus_used, total, status, shop_id, created_at FROM orders WHERE telegram_id = $1 ORDER BY created_at DESC LIMIT 50",
        &[&telegram_id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "orders": rows.iter().map(Order::from_row).collect::<Vec<_>>() })))
}
