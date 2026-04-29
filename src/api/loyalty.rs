use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;
use crate::db::loyalty::LoyaltyProfile;

#[derive(Debug, Deserialize)]
pub struct AddBonusRequest {
    pub amount: f64,
    pub tx_type: String,
    pub description: Option<String>,
    pub related_order_id: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/loyalty/:telegram_id", get(get_profile))
        .route("/loyalty/:telegram_id/bonus", post(add_bonus))
        .route("/loyalty/:telegram_id/use-bonus", post(use_bonus))
        .route("/loyalty/leaderboard", get(get_leaderboard))
        .route("/loyalty/config", get(get_loyalty_config))
        .route("/loyalty/config", post(update_loyalty_config))
}

async fn get_profile(State(state): State<AppState>, Path(telegram_id): Path<i64>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = client.query_opt(
        "SELECT telegram_id, total_spent, bonus_balance, tier, referral_code, referred_by, referral_count, first_purchase_at, manager_telegram_id, is_blocked FROM loyalty_profiles WHERE telegram_id = $1",
        &[&telegram_id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match row {
        Some(r) => Ok(Json(json!({ "profile": LoyaltyProfile::from_row(&r) }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn add_bonus(State(state): State<AppState>, Path(telegram_id): Path<i64>, Json(req): Json<AddBonusRequest>) -> Result<Json<Value>, StatusCode> {
    let tx_id = uuid::Uuid::new_v4().to_string();
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "INSERT INTO bonus_transactions (id, telegram_id, amount, tx_type, description, related_order_id) VALUES ($1,$2,$3,$4,$5,$6)",
        &[&tx_id, &telegram_id, &req.amount, &req.tx_type, &req.description, &req.related_order_id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "UPDATE loyalty_profiles SET bonus_balance = bonus_balance + $1 WHERE telegram_id = $2",
        &[&req.amount, &telegram_id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true, "tx_id": tx_id })))
}

async fn use_bonus(State(state): State<AppState>, Path(telegram_id): Path<i64>, Json(body): Json<Value>) -> Result<Json<Value>, StatusCode> {
    let amount = body["amount"].as_f64().unwrap_or(0.0);
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let result = client.execute(
        "UPDATE loyalty_profiles SET bonus_balance = GREATEST(0, bonus_balance - $1) WHERE telegram_id = $2 AND bonus_balance >= $1",
        &[&amount, &telegram_id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if result == 0 { return Err(StatusCode::BAD_REQUEST); }
    Ok(Json(json!({ "success": true })))
}

async fn get_leaderboard(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = client.query(
        "SELECT telegram_id, total_spent, tier FROM loyalty_profiles ORDER BY total_spent DESC LIMIT 20",
        &[],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let leaderboard: Vec<Value> = rows.iter().map(|r| json!({
        "telegram_id": r.get::<_, i64>("telegram_id"),
        "total_spent": r.get::<_, f64>("total_spent"),
        "tier": r.get::<_, String>("tier"),
    })).collect();
    Ok(Json(json!({ "leaderboard": leaderboard })))
}

async fn get_loyalty_config(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = client.query_opt("SELECT config FROM loyalty_config WHERE id = 1", &[])
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match row {
        Some(r) => Ok(Json(json!({ "config": r.get::<_, Value>("config") }))),
        None => Ok(Json(json!({ "config": null }))),
    }
}

async fn update_loyalty_config(State(state): State<AppState>, Json(body): Json<Value>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "INSERT INTO loyalty_config (id, config) VALUES (1, $1) ON CONFLICT (id) DO UPDATE SET config = $1",
        &[&body],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}
