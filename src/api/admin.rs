use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(get_all_users))
        .route("/admin/stats", get(get_stats))
        .route("/admin/managers", get(get_managers))
}

async fn get_all_users(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let rows = sqlx::query!(
        "SELECT ul.telegram_id, ul.first_name, ul.language,
                lp.total_spent, lp.bonus_balance, lp.tier, lp.is_blocked
         FROM user_languages ul
         LEFT JOIN loyalty_profiles lp ON ul.telegram_id = lp.telegram_id
         ORDER BY lp.total_spent DESC NULLS LAST LIMIT 500"
    )
    .fetch_all(&state.db.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let users: Vec<Value> = rows.iter().map(|r| json!({
        "telegram_id": r.telegram_id,
        "first_name": r.first_name,
        "language": r.language,
        "total_spent": r.total_spent,
        "bonus_balance": r.bonus_balance,
        "tier": r.tier,
        "is_blocked": r.is_blocked,
    })).collect();

    Ok(Json(json!({ "users": users })))
}

async fn get_stats(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let total_users = sqlx::query_scalar!("SELECT COUNT(*) FROM user_languages")
        .fetch_one(&state.db.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let total_orders = sqlx::query_scalar!("SELECT COUNT(*) FROM orders")
        .fetch_one(&state.db.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let total_revenue: Option<f64> = sqlx::query_scalar!("SELECT SUM(total) FROM orders WHERE status = 'completed'")
        .fetch_one(&state.db.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let active_strains = sqlx::query_scalar!("SELECT COUNT(*) FROM strains WHERE is_available = true")
        .fetch_one(&state.db.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "total_users": total_users,
        "total_orders": total_orders,
        "total_revenue": total_revenue.unwrap_or(0.0),
        "active_strains": active_strains,
    })))
}

async fn get_managers(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let rows = sqlx::query!(
        "SELECT telegram_id, name, username, ref_code FROM managers ORDER BY name"
    )
    .fetch_all(&state.db.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let managers: Vec<Value> = rows.iter().map(|r| json!({
        "telegram_id": r.telegram_id,
        "name": r.name,
        "username": r.username,
        "ref_code": r.ref_code,
    })).collect();

    Ok(Json(json!({ "managers": managers })))
}
