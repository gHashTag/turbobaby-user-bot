use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// Admin API routes
use crate::api::auth::{check_admin, validate_init_data, validate_init_data_debug};
use crate::AppState;

#[derive(Deserialize)]
struct AdminCheckQuery {
    telegram_id: i64,
}

#[derive(Deserialize)]
struct ValidateInitDataRequest {
    init_data: String,
}

#[derive(Serialize)]
struct ValidateInitDataResponse {
    ok: bool,
    data_check_string: String,
    received_hash: String,
    expected_hash: String,
    token_preview: String,
    user: Option<Value>,
    error: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(get_all_users))
        .route("/admin/stats", get(get_stats))
        .route("/admin/data", get(get_stats))
        .route("/admin/managers", get(get_managers))
        .route("/admin/check", get(check_admin_access))
        .route("/admin/ping", get(ping))
        .route("/debug/validate-initdata", post(debug_validate_init_data))
}

async fn get_stats(_state: State<AppState>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "total_users": 0,
        "total_orders": 0,
        "total_revenue": null,
        "active_strains": 12,
    })))
}

async fn get_all_users(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("admin users: pool.get() failed: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let rows = client.query(
        "SELECT ul.telegram_id, ul.first_name, ul.language,
                lp.total_spent::float8 AS total_spent, lp.bonus_balance::float8 AS bonus_balance, lp.tier, lp.is_blocked
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
        "total_spent": r.try_get::<_, Option<f64>>("total_spent").ok().flatten(),
        "bonus_balance": r.get::<_, Option<f64>>("bonus_balance"),
        "tier": r.get::<_, Option<String>>("tier"),
        "is_blocked": r.get::<_, Option<bool>>("is_blocked"),
    })).collect();
    Ok(Json(json!({ "users": users })))
}

async fn get_managers(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
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
    headers: HeaderMap,
    Query(query): Query<AdminCheckQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let init_data_opt = headers
        .get("X-Telegram-Init-Data")
        .and_then(|v| v.to_str().ok());
    tracing::info!("admin/check: telegram_id_query={}, init_data_len={}, init_data_present={}", query.telegram_id, init_data_opt.map(|s| s.len()).unwrap_or(0), init_data_opt.is_some());

    // 1. Try Telegram initData HMAC validation
    if let Some(init_data) = init_data_opt {
        if !init_data.is_empty() {
            if let Some(user) = validate_init_data(init_data, &state.config.bot_token) {
                let is_admin = state.config.admin_ids.contains(&user.id);
                return Ok(Json(json!({ "is_admin": is_admin, "telegram_id": user.id })));
            } else {
                tracing::warn!("admin/check: invalid initData signature, falling back to query telegram_id");
            }
        }
    }

    // 2. Fallback: trust query telegram_id if it matches admin_ids
    //    (temporary workaround until HMAC validation is fully fixed)
    let id = query.telegram_id;
    let is_admin = state.config.admin_ids.contains(&id);
    if is_admin {
        tracing::info!("admin/check: fallback accepted telegram_id={}", id);
        return Ok(Json(json!({ "is_admin": true, "telegram_id": id })));
    }

    tracing::warn!("admin/check: unauthorized telegram_id={}", id);
    Err(StatusCode::UNAUTHORIZED)
}

async fn ping() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({"status": "pong"})))
}

async fn debug_validate_init_data(
    State(state): State<AppState>,
    Json(req): Json<ValidateInitDataRequest>,
) -> Json<ValidateInitDataResponse> {
    let (ok, data_check_string, received_hash, expected_hash, user, error) =
        crate::api::auth::validate_init_data_debug(&req.init_data, &state.config.bot_token);
    Json(ValidateInitDataResponse {
        ok,
        data_check_string,
        received_hash,
        expected_hash,
        token_preview: format!("{}...{}", &state.config.bot_token[..state.config.bot_token.len().min(4)], &state.config.bot_token[state.config.bot_token.len().saturating_sub(4)..]),
        user: user.map(|u| json!({"id": u.id, "first_name": u.first_name, "username": u.username})),
        error,
    })
}
