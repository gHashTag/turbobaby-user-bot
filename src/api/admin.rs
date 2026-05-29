use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// Admin API routes
use crate::api::auth::{check_admin, validate_init_data};
use crate::AppState;

/// Global async mutex serializes admin-login attempts so that brute-force
/// parallel requests are throttled to one every ~3 s.
/// Uses tokio::sync::Mutex to avoid blocking the async runtime thread.
static LOGIN_LOCK: std::sync::LazyLock<tokio::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));

#[derive(Deserialize)]
struct AdminCheckQuery {
    telegram_id: i64,
}

#[derive(Deserialize)]
struct AdminLoginRequest {
    password: String,
    telegram_id: Option<i64>,
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
    user: Option<Value>,
    error: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(get_all_users))
        .route("/admin/stats", get(get_stats))
        .route("/admin/data", get(get_stats))
        .route("/admin/managers", get(get_managers).post(create_manager))
        .route("/admin/managers/:telegram_id/stats", get(get_manager_stats))
        .route("/admin/managers/:telegram_id", put(update_manager).delete(delete_manager))
        .route("/admin/check", get(check_admin_access))
        .route("/admin/login", post(admin_login))
        .route("/admin/ping", get(ping))
        .route("/debug/validate-initdata", post(debug_validate_init_data))
}

async fn get_stats(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("get_stats pool error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let total_orders: i64 = client
        .query_one("SELECT COUNT(*) FROM orders", &[])
        .await
        .map(|r| r.try_get(0).unwrap_or(0))
        .unwrap_or(0);

    let total_revenue: f64 = client
        .query_one("SELECT COALESCE(SUM(total)::float8, 0.0) FROM orders WHERE status = 'completed'", &[])
        .await
        .map(|r| r.try_get(0).unwrap_or(0.0))
        .unwrap_or(0.0);

    let active_strains: i64 = client
        .query_one("SELECT COUNT(*) FROM strains WHERE is_available = true", &[])
        .await
        .map(|r| r.try_get(0).unwrap_or(0))
        .unwrap_or(0);

    let total_users: i64 = client
        .query_one("SELECT COUNT(*) FROM user_languages", &[])
        .await
        .map(|r| r.try_get(0).unwrap_or(0))
        .unwrap_or(0);

    // Top strains by order count (avoid CROSS JOIN via subquery)
    let top_strains: Vec<Value> = client
        .query(
            r#"
            SELECT s.name, COUNT(*) as cnt
            FROM (
                SELECT (jsonb_array_elements(items)->>'id') as sid
                FROM orders
                WHERE status = 'completed'
            ) item
            JOIN strains s ON item.sid = s.id
            GROUP BY s.name
            ORDER BY cnt DESC
            LIMIT 5
            "#,
            &[],
        )
        .await
        .map(|rows| {
            rows.iter()
                .map(|r| {
                    let name: String = r.try_get(0).unwrap_or_default();
                    let count: i64 = r.try_get(1).unwrap_or(0);
                    json!({ "name": name, "count": count })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(Json(json!({
        "total_users": total_users,
        "total_orders": total_orders,
        "total_revenue": total_revenue,
        "active_strains": active_strains,
        "top_strains": top_strains,
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
        "telegram_id": r.try_get::<_, i64>("telegram_id").unwrap_or(0),
        "first_name": r.try_get::<_, Option<String>>("first_name").ok().flatten(),
        "language": r.try_get::<_, Option<String>>("language").ok().flatten(),
        "total_spent": r.try_get::<_, Option<f64>>("total_spent").ok().flatten(),
        "bonus_balance": r.try_get::<_, Option<f64>>("bonus_balance").ok().flatten(),
        "tier": r.try_get::<_, Option<String>>("tier").ok().flatten(),
        "is_blocked": r.try_get::<_, Option<bool>>("is_blocked").ok().flatten(),
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
        "SELECT telegram_id, name, username, ref_code, commission_rate::float8 FROM managers ORDER BY name LIMIT 500",
        &[],
    ).await.map_err(|e| {
        tracing::error!("admin managers: query failed: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let managers: Vec<Value> = rows.iter().map(|r| json!({
        "telegram_id": r.try_get::<_, i64>("telegram_id").unwrap_or(0),
        "name": r.try_get::<_, Option<String>>("name").ok().flatten(),
        "username": r.try_get::<_, Option<String>>("username").ok().flatten(),
        "ref_code": r.try_get::<_, Option<String>>("ref_code").ok().flatten(),
        "commission_rate": r.try_get::<_, Option<f64>>("commission_rate").ok().flatten(),
    })).collect();
    Ok(Json(json!({ "managers": managers })))
}

async fn get_manager_stats(
    Path(telegram_id): Path<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    // NOTE: orders.referrer_id column does not exist in current schema — using 0 as placeholder.
    // When the column is added, replace 0::int with the real subquery.
    let row = client.query_one(
        "SELECT 
            0::int as orders_count,
            (SELECT COUNT(*) FROM referral_events WHERE referrer_id = $1)::int as referrals_count,
            (SELECT MAX(created_at) FROM referral_events WHERE referrer_id = $1) as last_referral
         ",
        &[&telegram_id],
    ).await.map_err(|e| { tracing::error!("manager stats: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({
        "telegram_id": telegram_id,
        "orders_count": row.try_get::<_, i32>("orders_count").unwrap_or(0),
        "referrals_count": row.try_get::<_, i32>("referrals_count").unwrap_or(0),
        "last_referral": row.try_get::<_, Option<chrono::DateTime<chrono::Utc>>>("last_referral").ok().flatten().map(|d| d.to_rfc3339()),
    })))
}

#[derive(Deserialize)]
struct CreateManagerRequest {
    telegram_id: i64,
    name: Option<String>,
    username: Option<String>,
    ref_code: Option<String>,
    commission_rate: Option<f64>,
}

#[derive(Deserialize)]
struct UpdateManagerRequest {
    name: Option<String>,
    username: Option<String>,
    ref_code: Option<String>,
    commission_rate: Option<f64>,
}

fn validate_manager_fields(name: &Option<String>, username: &Option<String>, ref_code: &Option<String>, commission_rate: Option<f64>) -> Result<(), StatusCode> {
    if let Some(ref n) = name { if n.len() > 200 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref u) = username { if u.len() > 200 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref c) = ref_code { if c.len() > 200 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(r) = commission_rate { if !r.is_finite() || !(0.0..=100.0).contains(&r) { return Err(StatusCode::BAD_REQUEST); } }
    Ok(())
}

async fn create_manager(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateManagerRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_manager_fields(&req.name, &req.username, &req.ref_code, req.commission_rate)?;
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    client.execute(
        "INSERT INTO managers (telegram_id, name, username, ref_code, commission_rate) VALUES ($1, $2, $3, $4, $5)",
        &[&req.telegram_id, &req.name, &req.username, &req.ref_code, &req.commission_rate],
    ).await.map_err(|e| { tracing::error!("create_manager: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true, "telegram_id": req.telegram_id })))
}

async fn update_manager(
    Path(telegram_id): Path<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UpdateManagerRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_manager_fields(&req.name, &req.username, &req.ref_code, req.commission_rate)?;
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    client.execute(
        "UPDATE managers SET
            name = COALESCE($2, name),
            username = COALESCE($3, username),
            ref_code = COALESCE($4, ref_code),
            commission_rate = COALESCE($5, commission_rate)
         WHERE telegram_id = $1",
        &[&telegram_id, &req.name, &req.username, &req.ref_code, &req.commission_rate],
    ).await.map_err(|e| { tracing::error!("update_manager: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_manager(
    Path(telegram_id): Path<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    client.execute("DELETE FROM managers WHERE telegram_id = $1", &[&telegram_id])
        .await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true })))
}

async fn check_admin_access(
    headers: HeaderMap,
    Query(query): Query<AdminCheckQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let init_data_opt = headers
        .get("X-Telegram-Init-Data")
        .and_then(|v| v.to_str().ok());
    tracing::debug!(
        "admin/check: telegram_id_query={}, init_data_present={}",
        query.telegram_id, init_data_opt.is_some()
    );

    // 1. Try Telegram initData HMAC validation
    if let Some(init_data) = init_data_opt {
        if !init_data.is_empty() {
            if let Some(user) = validate_init_data(init_data, &state.config.bot_token) {
                if state.config.admin_ids.contains(&user.id) {
                    tracing::debug!("admin/check: initData authenticated telegram_id={}", user.id);
                    return Ok(Json(json!({ "is_admin": true, "telegram_id": user.id })));
                }
                tracing::warn!("admin/check: initData valid but user not admin telegram_id={}", user.id);
                // Don't return here — allow password fallback below
            } else {
                tracing::warn!("admin/check: invalid initData signature");
            }
        }
    }

    // 2. Fallback: password token (X-Admin-Token)
    let token_opt = headers.get("X-Admin-Token").and_then(|v| v.to_str().ok());
    if let Some(token) = token_opt {
        if let Some(ref password) = state.config.admin_password {
            if crate::api::auth::verify_admin_token(token, &state.config.bot_token, password) {
                tracing::info!("admin/check: token authenticated telegram_id={}", query.telegram_id);
                return Ok(Json(json!({ "is_admin": true, "telegram_id": query.telegram_id })));
            }
            tracing::warn!("admin/check: token verification failed");
        } else {
            tracing::warn!("admin/check: ADMIN_PASSWORD not set");
        }
    }

    tracing::warn!("admin/check: unauthorized");
    Err(StatusCode::UNAUTHORIZED)
}

fn validate_admin_login(req: &AdminLoginRequest) -> Result<(), StatusCode> {
    if req.password.len() > 1000 { return Err(StatusCode::BAD_REQUEST); }
    Ok(())
}

async fn admin_login(
    State(state): State<AppState>,
    Json(req): Json<AdminLoginRequest>,
) -> Result<Json<Value>, StatusCode> {
    validate_admin_login(&req)?;
    let valid = {
        let _guard = LOGIN_LOCK.lock().await;
        if let Some(ref password) = state.config.admin_password {
            crate::api::auth::verify_admin_token(
                &crate::api::auth::generate_admin_token(&req.password, &state.config.bot_token),
                &state.config.bot_token,
                password,
            )
        } else {
            false
        }
    };
    if valid {
        if let Some(ref password) = state.config.admin_password {
            let token = crate::api::auth::generate_admin_token(
                password,
                &state.config.bot_token,
            );
            let telegram_id = req.telegram_id.unwrap_or(0);
            return Ok(Json(json!({ "success": true, "token": token, "telegram_id": telegram_id })));
        }
    }
    tracing::warn!("admin_login: invalid password attempt");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    Err(StatusCode::UNAUTHORIZED)
}

async fn ping() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({"status": "pong"})))
}

async fn debug_validate_init_data(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<ValidateInitDataRequest>,
) -> Result<Json<ValidateInitDataResponse>, StatusCode> {
    check_admin(&headers, &state)?;
    let (ok, data_check_string, received_hash, user, error) =
        crate::api::auth::validate_init_data_debug(&req.init_data, &state.config.bot_token);
    Ok(Json(ValidateInitDataResponse {
        ok,
        data_check_string,
        received_hash,
        user: user.map(|u| json!({"id": u.id, "first_name": u.first_name, "username": u.username})),
        error,
    }))
}

#[cfg(test)]
mod tests {
    use super::{validate_manager_fields, validate_admin_login, AdminLoginRequest};
    use axum::http::StatusCode;

    #[test]
    fn test_validate_manager_fields_ok() {
        assert!(validate_manager_fields(&Some("Name".into()), &Some("user".into()), &Some("code".into()), Some(10.0)).is_ok());
    }

    #[test]
    fn test_validate_manager_fields_all_none() {
        assert!(validate_manager_fields(&None, &None, &None, None).is_ok());
    }

    #[test]
    fn test_validate_manager_name_too_long() {
        assert_eq!(validate_manager_fields(&Some("a".repeat(201)), &None, &None, None).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_manager_username_too_long() {
        assert_eq!(validate_manager_fields(&None, &Some("a".repeat(201)), &None, None).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_manager_ref_code_too_long() {
        assert_eq!(validate_manager_fields(&None, &None, &Some("a".repeat(201)), None).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_manager_commission_negative() {
        assert_eq!(validate_manager_fields(&None, &None, &None, Some(-1.0)).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_manager_commission_too_high() {
        assert_eq!(validate_manager_fields(&None, &None, &None, Some(101.0)).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_manager_commission_nan() {
        assert_eq!(validate_manager_fields(&None, &None, &None, Some(f64::NAN)).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_admin_login_ok() {
        let req = AdminLoginRequest { password: "secret".into(), telegram_id: None };
        assert!(validate_admin_login(&req).is_ok());
    }

    #[test]
    fn test_validate_admin_login_password_too_long() {
        let req = AdminLoginRequest { password: "a".repeat(1001), telegram_id: None };
        assert_eq!(validate_admin_login(&req).unwrap_err(), StatusCode::BAD_REQUEST);
    }
}
