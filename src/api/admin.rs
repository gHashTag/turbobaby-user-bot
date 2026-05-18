use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;

#[derive(Deserialize)]
struct AdminCheckQuery {
    telegram_id: i64,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(get_all_users))
        .route("/admin/stats", get(get_stats))
        .route("/admin/managers", get(get_managers))
        .route("/admin/check", get(check_admin_access))
}

async fn get_all_users(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("admin users: pool.get() failed: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let rows = client.query(
        "SELECT ul.telegram_id, ul.first_name, ul.language,
                lp.total_spent, lp.bonus_balance, lp.tier, lp.is_blocked
         FROM user_languages ul
         LEFT JOIN loyalty_profiles lp ON ul.telegram_id = lp.telegram_id
         ORDER BY lp.total_spent DESC NULLS LAST LIMIT 500",
        &[],
    ).await.map_err(|e| {
        tracing::error!("admin users: query failed: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let users: Vec<Value> = rows.iter().map(|r| json!({
        "telegram_id": r.get::<_, i64>("telegram_id"),
        "first_name": r.get::<_, Option<String>>("first_name"),
        "language": r.get::<_, Option<String>>("language"),
        "total_spent": r.get::<_, Option<f64>>("total_spent"),
        "bonus_balance": r.get::<_, Option<f64>>("bonus_balance"),
        "tier": r.get::<_, Option<String>>("tier"),
        "is_blocked": r.get::<_, Option<bool>>("is_blocked"),
    })).collect();
    Ok(Json(json!({ "users": users })))
}

async fn get_stats(_state: AppState) -> Result<Json<Value>, StatusCode> {
    tracing::info!("admin stats: called");

    Ok(Json(json!({
        "total_users": 0,
        "total_orders": 0,
        "total_revenue": null,
        "active_strains": 12,
    })))
}

async fn get_managers(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("admin managers: pool.get() failed: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let rows = client.query(
        "SELECT telegram_id, name, username, ref_code FROM managers ORDER BY name",
        &[],
    ).await.map_err(|e| {
        tracing::error!("admin managers: query failed: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let managers: Vec<Value> = rows.iter().map(|r| json!({
        "telegram_id": r.get::<_, i64>("telegram_id"),
        "name": r.get::<_, Option<String>>("name"),
        "username": r.get::<_, Option<String>>("username"),
        "ref_code": r.get::<_, Option<String>>("ref_code"),
    })).collect();
    Ok(Json(json!({ "managers": managers })))
}

async fn check_admin_access(
    Query(query): Query<AdminCheckQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let is_admin = state.config.admin_ids.contains(&query.telegram_id);
    Ok(Json(json!({ "is_admin": is_admin })))
}
