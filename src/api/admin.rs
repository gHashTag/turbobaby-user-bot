use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// Admin API routes
use crate::api::auth::{check_admin, validate_telegram_id_param};
use crate::db::entities::{lab_certificate, strain_review};
use crate::AppState;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

// Cycle #128: removed LOGIN_LOCK static. It was introduced to pair
// with a per-attempt `tokio::time::sleep(3s)` that cycle #126 deleted,
// so the mutex's stated purpose ("throttle to one every ~3s") evaporated
// with the sleep. The cycle #106 + #126 per-IP rate-limit (10/5min via
// `record_failed_admin_attempt`) is the right place for brute-force
// defence — it works per-IP, doesn't penalise legitimate parallel
// admins on different IPs, and doesn't hold a global mutex across an
// HMAC verify.

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

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(get_all_users))
        .route("/admin/stats", get(get_stats))
        .route("/admin/data", get(get_stats))
        .route("/admin/managers", get(get_managers).post(create_manager))
        .route("/admin/managers/:telegram_id/stats", get(get_manager_stats))
        .route(
            "/admin/managers/:telegram_id",
            put(update_manager).delete(delete_manager),
        )
        .route("/admin/check", get(check_admin_access))
        .route("/admin/login", post(admin_login))
        .route("/admin/notify-deploy", post(manual_notify_deploy))
        .route("/admin/ping", get(ping))
        .route("/debug/validate-initdata", post(debug_validate_init_data))
        // Cycle #136: TЗ #2 §5 self-service marketing-badge toggle.
        // GET returns the current `loyalty_config.marketing_badges_hidden`;
        // PUT { hidden: bool } updates it. Closes the "без участия
        // программиста" gap that #133-B's env-only flag left open.
        .route(
            "/admin/marketing-display",
            get(get_marketing_display).put(set_marketing_display),
        )
        // Variant C: community / retention admin surface.
        .route("/admin/reviews", get(list_reviews_admin))
        .route("/admin/reviews/:id/moderate", post(moderate_review))
        .route("/admin/strains/:id/lab-cert", post(create_lab_cert))
        .route("/admin/line-broadcast", post(line_broadcast))
}

async fn get_stats(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    // Cycle #92: SeaORM via raw `Statement` (pattern #15). All 5 queries
    // are simple aggregates / GROUP BY; typed builder would be heavier
    // than the SQL itself.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let orm = &state.db.orm;

    let total_orders: i64 = orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS n FROM orders".to_string(),
        ))
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get::<i64>("", "n").ok())
        .unwrap_or(0);

    let total_revenue: f64 = orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COALESCE(SUM(total)::float8, 0.0) AS v FROM orders WHERE status = 'completed'"
                .to_string(),
        ))
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get::<f64>("", "v").ok())
        .map(|v| if v.is_finite() { v.max(0.0) } else { 0.0 })
        .unwrap_or(0.0);

    let active_strains: i64 = orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS n FROM strains WHERE is_available = true".to_string(),
        ))
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get::<i64>("", "n").ok())
        .unwrap_or(0);

    let total_users: i64 = orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS n FROM user_languages".to_string(),
        ))
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get::<i64>("", "n").ok())
        .unwrap_or(0);

    // Top strains by order count (avoid CROSS JOIN via subquery)
    let top_strains: Vec<Value> = orm
        .query_all(Statement::from_string(
            DbBackend::Postgres,
            r#"
            SELECT s.name AS name, COUNT(*)::bigint AS cnt
            FROM (
                SELECT (jsonb_array_elements(items)->>'id') as sid
                FROM orders
                WHERE status = 'completed'
            ) item
            JOIN strains s ON item.sid = s.id
            GROUP BY s.name
            ORDER BY cnt DESC
            LIMIT 5
            "#
            .to_string(),
        ))
        .await
        .map(|rows| {
            rows.iter()
                .map(|r| {
                    let name: String = r.try_get("", "name").unwrap_or_default();
                    let count: i64 = r.try_get("", "cnt").unwrap_or(0);
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
    // Cycle #92: SeaORM via Statement (LEFT JOIN + NULLS LAST ordering,
    // pattern #15).
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let rows = state.db.orm.query_all(Statement::from_string(
        DbBackend::Postgres,
        "SELECT ul.telegram_id, ul.first_name, ul.language,
                lp.total_spent::float8 AS total_spent, lp.bonus_balance::float8 AS bonus_balance, lp.tier, lp.is_blocked
         FROM user_languages ul
         LEFT JOIN loyalty_profiles lp ON ul.telegram_id = lp.telegram_id
         ORDER BY lp.total_spent DESC NULLS LAST LIMIT 500".to_string(),
    )).await.map_err(|e| {
        tracing::error!("admin users: query failed: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let users: Vec<Value> = rows.iter().map(|r| json!({
        "telegram_id": r.try_get::<i64>("", "telegram_id").unwrap_or(0),
        "first_name": r.try_get::<Option<String>>("", "first_name").ok().flatten(),
        "language": r.try_get::<Option<String>>("", "language").ok().flatten(),
        "total_spent": r.try_get::<Option<f64>>("", "total_spent").ok().flatten().filter(|v| v.is_finite()),
        "bonus_balance": r.try_get::<Option<f64>>("", "bonus_balance").ok().flatten().filter(|v| v.is_finite()),
        "tier": r.try_get::<Option<String>>("", "tier").ok().flatten(),
        "is_blocked": r.try_get::<Option<bool>>("", "is_blocked").ok().flatten(),
    })).collect();
    Ok(Json(json!({ "users": users })))
}

async fn get_managers(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let rows = state.db.orm.query_all(Statement::from_string(
        DbBackend::Postgres,
        "SELECT telegram_id, name, username, ref_code, commission_rate::float8 FROM managers ORDER BY name LIMIT 500".to_string(),
    )).await.map_err(|e| {
        tracing::error!("admin managers: query failed: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let managers: Vec<Value> = rows.iter().map(|r| json!({
        "telegram_id": r.try_get::<i64>("", "telegram_id").unwrap_or(0),
        "name": r.try_get::<Option<String>>("", "name").ok().flatten(),
        "username": r.try_get::<Option<String>>("", "username").ok().flatten(),
        "ref_code": r.try_get::<Option<String>>("", "ref_code").ok().flatten(),
        "commission_rate": r.try_get::<Option<f64>>("", "commission_rate").ok().flatten().filter(|v| v.is_finite()),
    })).collect();
    Ok(Json(json!({ "managers": managers })))
}

async fn get_manager_stats(
    Path(telegram_id): Path<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    check_admin(&headers, &state)?;
    // NOTE: orders.referrer_id column does not exist in current schema — using 0 as placeholder.
    // When the column is added, replace 0::int with the real subquery.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT \
            0::int as orders_count, \
            (SELECT COUNT(*) FROM referral_events WHERE referrer_id = $1)::int as referrals_count, \
            (SELECT MAX(created_at) FROM referral_events WHERE referrer_id = $1) as last_referral",
            [telegram_id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("manager stats: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            tracing::error!("manager stats: no row");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({
        "telegram_id": telegram_id,
        "orders_count": row.try_get::<i32>("", "orders_count").unwrap_or(0),
        "referrals_count": row.try_get::<i32>("", "referrals_count").unwrap_or(0),
        "last_referral": row.try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "last_referral").ok().flatten().map(|d| d.to_rfc3339()),
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

fn validate_manager_fields(
    name: &Option<String>,
    username: &Option<String>,
    ref_code: &Option<String>,
    commission_rate: Option<f64>,
) -> Result<(), StatusCode> {
    if let Some(ref n) = name {
        if n.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref u) = username {
        if u.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref c) = ref_code {
        if c.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(r) = commission_rate {
        if !r.is_finite() || !(0.0..=100.0).contains(&r) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    Ok(())
}

async fn create_manager(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateManagerRequest>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(req.telegram_id)?;
    check_admin(&headers, &state)?;
    validate_manager_fields(&req.name, &req.username, &req.ref_code, req.commission_rate)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO managers (telegram_id, name, username, ref_code, commission_rate) VALUES ($1, $2, $3, $4, $5)",
        [
            req.telegram_id.into(),
            req.name.clone().into(),
            req.username.clone().into(),
            req.ref_code.clone().into(),
            req.commission_rate.into(),
        ],
    )).await.map_err(|e| {
        tracing::error!("create_manager: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(
        json!({ "success": true, "telegram_id": req.telegram_id }),
    ))
}

async fn update_manager(
    Path(telegram_id): Path<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UpdateManagerRequest>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    check_admin(&headers, &state)?;
    validate_manager_fields(&req.name, &req.username, &req.ref_code, req.commission_rate)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE managers SET \
            name = COALESCE($2, name), \
            username = COALESCE($3, username), \
            ref_code = COALESCE($4, ref_code), \
            commission_rate = COALESCE($5, commission_rate) \
         WHERE telegram_id = $1",
            [
                telegram_id.into(),
                req.name.clone().into(),
                req.username.clone().into(),
                req.ref_code.clone().into(),
                req.commission_rate.into(),
            ],
        ))
        .await
        .map_err(|e| {
            tracing::error!("update_manager: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_manager(
    Path(telegram_id): Path<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM managers WHERE telegram_id = $1",
            [telegram_id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("delete_manager: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "success": true })))
}

async fn check_admin_access(
    headers: HeaderMap,
    Query(query): Query<AdminCheckQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // telegram_id is client-supplied and unverified; it is only used for
    // display. The real identity comes from verified initData or the
    // X-Admin-Token password path.
    let init_data_present = headers.get("X-Telegram-Init-Data").is_some();
    let token_present = headers.get("X-Admin-Token").is_some();
    tracing::debug!(
        "admin/check: telegram_id_query={}, init_data_present={} token_present={}",
        query.telegram_id,
        init_data_present,
        token_present
    );

    let reason = diagnose_admin_auth_failure(&headers, &state, init_data_present, token_present);

    match crate::api::auth::check_admin(&headers, &state) {
        Ok(telegram_id) => {
            // check_admin returns 0 for password-token auth (shared password has
            // no specific telegram_id) and the real user id for initData auth.
            // Never echo the client-supplied query.telegram_id (OWASP A01).
            if telegram_id == 0 {
                tracing::info!("admin/check: token authenticated (no specific telegram_id)");
            } else {
                tracing::debug!("admin/check: authenticated telegram_id={}", telegram_id);
            }
            Ok(Json(
                json!({ "is_admin": true, "telegram_id": telegram_id }),
            ))
        }
        Err(status) => {
            tracing::warn!("admin/check: unauthorized (status={})", status.as_u16());
            Err((
                status,
                Json(json!({
                    "is_admin": false,
                    "telegram_id": 0,
                    "reason": reason.unwrap_or("unauthorized")
                })),
            ))
        }
    }
}

/// Diagnostic reason for a 401 on /api/admin/check. Does NOT perform the
/// authoritative auth check — it only inspects the headers to tell the UI
/// (and support) why the request was rejected.
fn diagnose_admin_auth_failure(
    headers: &HeaderMap,
    state: &AppState,
    init_data_present: bool,
    token_present: bool,
) -> Option<&'static str> {
    if init_data_present {
        let init_data = headers
            .get("X-Telegram-Init-Data")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if init_data.is_empty() {
            return Some("empty_init_data");
        }
        if let Some(user) = crate::api::auth::validate_init_data(init_data, &state.config.bot_token)
        {
            if !state.config.admin_ids.contains(&user.id) {
                return Some("not_admin");
            }
            // Would have succeeded; fall through to generic unauthorized.
        } else {
            return Some("invalid_init_data");
        }
    }
    if token_present {
        return Some("invalid_token");
    }
    Some("no_credentials")
}

fn validate_admin_login(req: &AdminLoginRequest) -> Result<(), StatusCode> {
    if req.password.len() > 1000 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

async fn admin_login(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<AdminLoginRequest>,
) -> Result<Json<Value>, StatusCode> {
    validate_admin_login(&req)?;
    tracing::info!(
        "admin_login: attempt (admin_password_set={} password_len={} telegram_id={})",
        state.config.admin_password.is_some(),
        req.password.len(),
        req.telegram_id.unwrap_or(0)
    );
    let valid = if let Some(ref password) = state.config.admin_password {
        crate::api::auth::verify_admin_token(
            &crate::api::auth::generate_admin_token(&req.password, &state.config.bot_token),
            &state.config.bot_token,
            password,
        )
    } else {
        false
    };
    if valid {
        if let Some(ref password) = state.config.admin_password {
            let token = crate::api::auth::generate_admin_token(password, &state.config.bot_token);
            let telegram_id = req.telegram_id.unwrap_or(0);
            return Ok(Json(
                json!({ "success": true, "token": token, "telegram_id": telegram_id }),
            ));
        }
    }
    tracing::warn!("admin_login: invalid password attempt");
    // Cycle #126: was `tokio::time::sleep(3s)` which held one tokio task
    // per failed attempt — easy to exhaust under coordinated probing.
    // Replaced with the cycle #106 sliding-window-log rate-limit (same
    // ADMIN_AUTH_RATE_LIMIT store as `check_admin` so attempts across
    // both paths count against the same per-IP bucket). After 10 fails
    // in 5 min from one IP, returns 429 immediately — strictly stronger
    // defence than the per-request slowdown.
    crate::api::auth::record_failed_admin_attempt(&headers)?;
    Err(StatusCode::UNAUTHORIZED)
}

#[derive(Deserialize)]
struct ManualNotifyDeployRequest {
    #[serde(default)]
    note: String,
}

async fn manual_notify_deploy(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<ManualNotifyDeployRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let note = if req.note.trim().is_empty() {
        "manual trigger from admin panel"
    } else {
        req.note.trim()
    };
    let text = crate::notify::notify_deploy_manual(
        &state.bot,
        &state.config,
        note,
    )
    .await;
    Ok(Json(json!({
        "success": true,
        "sent_to": state.config.admin_ids.len(),
        "message_preview": text,
    })))
}

async fn ping() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({"status": "pong"})))
}

/// Cycle #136: read the current "hide marketing badges" toggle state.
/// Returns both inputs so the UI can show whether the env override is
/// forcing the value (admin click won't help in that case).
async fn get_marketing_display(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    use crate::db::entities::loyalty_config;
    use sea_orm::EntityTrait;
    let db_value = match loyalty_config::Entity::find_by_id(1)
        .one(&state.db.orm)
        .await
    {
        Ok(Some(m)) => m.marketing_badges_hidden,
        Ok(None) => false,
        Err(e) => {
            tracing::error!("get_marketing_display: read failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    Ok(Json(json!({
        "hidden": db_value || state.config.hide_marketing_badges,
        "db_hidden": db_value,
        "env_override": state.config.hide_marketing_badges,
    })))
}

/// Cycle #136: PUT { hidden: bool } — update the DB toggle. The env
/// override is intentionally not reachable from here (ops would set
/// `HIDE_MARKETING_BADGES` in the deployment).
async fn set_marketing_display(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let hidden = body
        .get("hidden")
        .and_then(|v| v.as_bool())
        .ok_or(StatusCode::BAD_REQUEST)?;
    use crate::db::entities::loyalty_config::{ActiveModel, Column, Entity as Lc};
    use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
    let result = Lc::update_many()
        .col_expr(
            Column::MarketingBadgesHidden,
            sea_orm::sea_query::Expr::value(hidden),
        )
        .filter(Column::Id.eq(1))
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("set_marketing_display: update failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    // No-row case: row id=1 doesn't exist yet. Insert it.
    if result.rows_affected == 0 {
        let am = ActiveModel {
            id: Set(1),
            config: Set(serde_json::json!({})),
            marketing_badges_hidden: Set(hidden),
        };
        Lc::insert(am).exec(&state.db.orm).await.map_err(|e| {
            tracing::error!("set_marketing_display: seed insert failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }
    // Strain catalog cache needs to flip with the toggle so customers
    // see the change on next /api/strains immediately, not after a
    // 60-second TTL.
    crate::api::cache::invalidate_strains(&state.cache).await;
    tracing::info!(
        "marketing-display set: hidden={} (env_override={})",
        hidden,
        state.config.hide_marketing_badges
    );
    Ok(Json(json!({ "success": true, "hidden": hidden })))
}

async fn debug_validate_init_data(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<ValidateInitDataRequest>,
) -> Result<Json<ValidateInitDataResponse>, StatusCode> {
    check_admin(&headers, &state)?;
    let info = crate::api::auth::validate_init_data_debug(&req.init_data, &state.config.bot_token);
    Ok(Json(ValidateInitDataResponse {
        ok: info.ok,
        data_check_string: info.data_check_string_decoded,
        received_hash: info.hash,
        user: info
            .user
            .map(|u| json!({"id": u.id, "first_name": u.first_name, "username": u.username})),
        error: info.error,
    }))
}

#[derive(Deserialize)]
struct ModerateReviewRequest {
    approved: bool,
}

#[derive(Deserialize)]
struct CreateLabCertRequest {
    certificate_url: String,
    #[serde(default)]
    tested_at: Option<String>,
    #[serde(default)]
    thc_percent: Option<f64>,
    #[serde(default)]
    cbd_percent: Option<f64>,
}

#[derive(Deserialize)]
struct LineBroadcastRequest {
    text: String,
}

async fn list_reviews_admin(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;

    let mut query = strain_review::Entity::find();
    if q.get("pending")
        .map(|s| s == "1" || s == "true")
        .unwrap_or(false)
    {
        query = query.filter(strain_review::Column::Approved.eq(false));
    }
    let rows = query
        .order_by_desc(strain_review::Column::CreatedAt)
        .limit(100)
        .all(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("list_reviews_admin DB error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let out: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "telegram_id": r.telegram_id,
                "strain_id": r.strain_id,
                "order_id": r.order_id,
                "rating": r.rating,
                "comment": r.comment,
                "approved": r.approved,
                "created_at": r.created_at.to_rfc3339(),
            })
        })
        .collect();
    Ok(Json(json!({ "reviews": out })))
}

async fn moderate_review(
    headers: HeaderMap,
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(req): Json<ModerateReviewRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let model = strain_review::Entity::find_by_id(&id)
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("moderate_review DB error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut active: strain_review::ActiveModel = model.into();
    active.approved = Set(req.approved);
    active.update(&state.db.orm).await.map_err(|e| {
        tracing::error!("moderate_review update error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "success": true, "approved": req.approved })))
}

async fn create_lab_cert(
    headers: HeaderMap,
    Path(strain_id): Path<String>,
    State(state): State<AppState>,
    Json(req): Json<CreateLabCertRequest>,
) -> Result<Json<Value>, StatusCode> {
    let admin_id = check_admin(&headers, &state)?;
    if req.certificate_url.is_empty() || req.certificate_url.len() > 2048 {
        return Err(StatusCode::BAD_REQUEST);
    }
    crate::api::validate_url(&Some(req.certificate_url.clone()))?;

    let tested_at = req
        .tested_at
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    if let Some(v) = req.thc_percent {
        if !v.is_finite() || !(0.0..=100.0).contains(&v) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(v) = req.cbd_percent {
        if !v.is_finite() || !(0.0..=100.0).contains(&v) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Local::now().fixed_offset();
    let active = lab_certificate::ActiveModel {
        id: Set(id.clone()),
        strain_id: Set(strain_id.clone()),
        certificate_url: Set(req.certificate_url),
        tested_at: Set(tested_at),
        thc_percent: Set(req.thc_percent),
        cbd_percent: Set(req.cbd_percent),
        uploaded_by_telegram_id: Set(Some(admin_id)),
        created_at: Set(now.into()),
    };
    active.insert(&state.db.orm).await.map_err(|e| {
        tracing::error!("create_lab_cert insert error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({ "id": id, "strain_id": strain_id })))
}

async fn line_broadcast(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<LineBroadcastRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let token = state
        .config
        .line_channel_access_token
        .as_deref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let (status, body) = crate::line::broadcast_message(token, &req.text)
        .await
        .map_err(|e| {
            tracing::warn!("line_broadcast rejected: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    if status.is_success() {
        Ok(Json(
            json!({ "success": true, "line_status": status.as_u16() }),
        ))
    } else {
        tracing::warn!("LINE broadcast returned {}: {}", status, body);
        Err(StatusCode::BAD_GATEWAY)
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_admin_login, validate_manager_fields, AdminLoginRequest};
    use axum::http::StatusCode;

    #[test]
    fn test_validate_manager_fields_ok() {
        assert!(validate_manager_fields(
            &Some("Name".into()),
            &Some("user".into()),
            &Some("code".into()),
            Some(10.0)
        )
        .is_ok());
    }

    #[test]
    fn test_validate_manager_fields_all_none() {
        assert!(validate_manager_fields(&None, &None, &None, None).is_ok());
    }

    #[test]
    fn test_validate_manager_name_too_long() {
        assert_eq!(
            validate_manager_fields(&Some("a".repeat(201)), &None, &None, None).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_manager_username_too_long() {
        assert_eq!(
            validate_manager_fields(&None, &Some("a".repeat(201)), &None, None).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_manager_ref_code_too_long() {
        assert_eq!(
            validate_manager_fields(&None, &None, &Some("a".repeat(201)), None).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_manager_commission_negative() {
        assert_eq!(
            validate_manager_fields(&None, &None, &None, Some(-1.0)).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_manager_commission_too_high() {
        assert_eq!(
            validate_manager_fields(&None, &None, &None, Some(101.0)).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_manager_commission_nan() {
        assert_eq!(
            validate_manager_fields(&None, &None, &None, Some(f64::NAN)).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_admin_login_ok() {
        let req = AdminLoginRequest {
            password: "secret".into(),
            telegram_id: None,
        };
        assert!(validate_admin_login(&req).is_ok());
    }

    #[test]
    fn test_validate_admin_login_password_too_long() {
        let req = AdminLoginRequest {
            password: "a".repeat(1001),
            telegram_id: None,
        };
        assert_eq!(
            validate_admin_login(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }
}
