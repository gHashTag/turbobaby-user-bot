use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::auth::check_admin;
use crate::AppState;


#[derive(Debug, Deserialize)]
pub struct AddBonusRequest {
    pub amount: f64,
    pub tx_type: String,
    pub description: Option<String>,
    pub related_order_id: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/loyalty/tiers", get(get_loyalty_tiers))
        .route("/loyalty/:telegram_id", get(get_profile))
        .route("/loyalty/:telegram_id/bonus", post(add_bonus))
        .route("/loyalty/:telegram_id/use-bonus", post(use_bonus))
        .route("/loyalty/leaderboard", get(get_leaderboard))
        .route("/loyalty/config", get(get_loyalty_config))
        .route("/loyalty/config", post(update_loyalty_config))
}

async fn get_loyalty_tiers(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let rows = client.query(
        "SELECT tier, name, min_points, discount_percent, points_multiplier, perks, icon, color \
         FROM loyalty_tiers ORDER BY min_points ASC",
        &[],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let tiers: Vec<Value> = rows.iter().map(|r| json!({
        "tier":             r.get::<_, String>("tier"),
        "name":             r.get::<_, String>("name"),
        "min_points":       r.get::<_, i32>("min_points"),
        "discount_percent": r.get::<_, i32>("discount_percent"),
        "points_multiplier":r.get::<_, f32>("points_multiplier"),
        "perks":            r.get::<_, Vec<String>>("perks"),
        "icon":             r.get::<_, String>("icon"),
        "color":            r.get::<_, String>("color"),
    })).collect();
    Ok(Json(json!({ "tiers": tiers })))
}

async fn get_profile(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    crate::api::auth::check_owner(&headers, &state, telegram_id)?;
    // SeaORM-версия: sqlx безопасно читает NUMERIC в f64.
    use sea_orm::{Statement, DbBackend, ConnectionTrait};
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT telegram_id, total_spent::float8 AS total_spent, bonus_balance::float8 AS bonus_balance, tier, referral_code, referred_by, referral_count, first_purchase_at, manager_telegram_id, is_blocked FROM loyalty_profiles WHERE telegram_id = $1",
        [telegram_id.into()],
    );
    let row = state.db.orm.query_one(stmt).await
        .map_err(|e| { tracing::error!("get_profile sea-orm: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    match row {
        Some(r) => {
            let profile = json!({
                "telegram_id": r.try_get::<i64>("", "telegram_id").unwrap_or(0),
                "total_spent": r.try_get::<Option<f64>>("", "total_spent").ok().flatten(),
                "bonus_balance": r.try_get::<Option<f64>>("", "bonus_balance").ok().flatten().unwrap_or(0.0),
                "tier": r.try_get::<String>("", "tier").unwrap_or_default(),
                "referral_code": r.try_get::<Option<String>>("", "referral_code").ok().flatten(),
                "referred_by": r.try_get::<Option<i64>>("", "referred_by").ok().flatten(),
                "referral_count": r.try_get::<i32>("", "referral_count").unwrap_or(0),
                "first_purchase_at": r.try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "first_purchase_at").ok().flatten(),
                "manager_telegram_id": r.try_get::<Option<i64>>("", "manager_telegram_id").ok().flatten(),
                "is_blocked": r.try_get::<bool>("", "is_blocked").unwrap_or(false),
            });
            Ok(Json(json!({ "profile": profile })))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn add_bonus(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
    Json(req): Json<AddBonusRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    if req.amount < 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let tx_id = uuid::Uuid::new_v4().to_string();
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    client.execute(
        "INSERT INTO bonus_transactions (id, telegram_id, amount, tx_type, description, related_order_id) VALUES ($1,$2,$3,$4,$5,$6)",
        &[&tx_id, &telegram_id, &req.amount, &req.tx_type, &req.description, &req.related_order_id],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    client.execute(
        "UPDATE loyalty_profiles SET bonus_balance = bonus_balance + $1 WHERE telegram_id = $2",
        &[&req.amount, &telegram_id],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true, "tx_id": tx_id })))
}

async fn use_bonus(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let amount = body["amount"].as_f64().unwrap_or(0.0);
    if amount <= 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let result = client.execute(
        "UPDATE loyalty_profiles SET bonus_balance = GREATEST(0, bonus_balance - $1) WHERE telegram_id = $2 AND bonus_balance >= $1",
        &[&amount, &telegram_id],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    if result == 0 { return Err(StatusCode::BAD_REQUEST); }
    Ok(Json(json!({ "success": true })))
}

async fn get_leaderboard(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // SeaORM-версия: обходит все проблемы с NUMERIC ↔ f64,
    // потому что sqlx из коробки умеет читать numeric в f64.
    use sea_orm::{Statement, DbBackend, ConnectionTrait};
    let stmt = Statement::from_string(
        DbBackend::Postgres,
        "SELECT lp.telegram_id, lp.total_spent::float8 AS total_spent, lp.tier, ul.first_name FROM loyalty_profiles lp LEFT JOIN user_languages ul ON lp.telegram_id = ul.telegram_id ORDER BY lp.total_spent DESC NULLS LAST LIMIT 20".to_string(),
    );
    let rows = state.db.orm.query_all(stmt).await
        .map_err(|e| { tracing::error!("get_leaderboard sea-orm: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    let leaderboard: Vec<Value> = rows.iter().map(|r| {
        let telegram_id: i64 = r.try_get("", "telegram_id").unwrap_or(0);
        let total_spent: Option<f64> = r.try_get("", "total_spent").ok();
        let tier: String = r.try_get("", "tier").unwrap_or_default();
        let first_name: Option<String> = r.try_get("", "first_name").ok();
        json!({
            "telegram_id": telegram_id,
            "first_name": first_name,
            "total_spent": total_spent.unwrap_or(0.0),
            "tier": tier,
        })
    }).collect();
    Ok(Json(json!({ "leaderboard": leaderboard })))
}

async fn get_loyalty_config(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let row = client.query_opt("SELECT config FROM loyalty_config WHERE id = 1", &[])
        .await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    match row {
        Some(r) => Ok(Json(json!({ "config": r.get::<_, Value>("config") }))),
        None => Ok(Json(json!({ "config": null }))),
    }
}

async fn update_loyalty_config(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    client.execute(
        "INSERT INTO loyalty_config (id, config) VALUES (1, $1) ON CONFLICT (id) DO UPDATE SET config = $1",
        &[&body],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true })))
}
