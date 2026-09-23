use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, patch, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// Admin API routes
use crate::api::auth::{check_admin, validate_telegram_id_param};
use crate::db::entities::delivery_zone;
use crate::AppState;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DbBackend, Set, Statement};
use std::collections::HashSet;
use teloxide::payloads::{SendMessageSetters, SendPhotoSetters};
use teloxide::prelude::Requester;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, InputFile, ParseMode, WebAppInfo,
};

// Cycle #128: removed LOGIN_LOCK static. It was introduced to pair
// with a per-attempt `tokio::time::sleep(3s)` that cycle #126 deleted,
// so the mutex's stated purpose ("throttle to one every ~3s") evaporated
// with the sleep. The cycle #106 + #126 per-IP rate-limit (10/5min via
// `record_failed_admin_attempt`) is the right place for brute-force
// defence — it works per-IP, doesn't penalise legitimate parallel
// admins on different IPs, and doesn't hold a global mutex across an
// HMAC verify.

// Per-admin Telegram broadcast rate-limit: 1 attempt per 10 minutes.
// Keyed by admin telegram_id (from check_admin) to prevent double-clicks
// and accidental spam to the whole user base.
static BROADCAST_RATE_LIMIT: std::sync::LazyLock<crate::api::rate_limit::SyncSlidingWindowStore> =
    std::sync::LazyLock::new(crate::api::rate_limit::new_sync_store);
const BROADCAST_RL_WINDOW: std::time::Duration = std::time::Duration::from_secs(600);
const BROADCAST_RL_MAX_ATTEMPTS: usize = 1;
const BROADCAST_RL_MAX_KEYS: usize = 100;

#[derive(Deserialize)]
struct AdminCheckQuery {
    telegram_id: i64,
}

#[derive(Deserialize)]
struct TelegramBroadcastRequest {
    text: String,
    photo_url: Option<String>,
    product: Option<BroadcastProduct>,
    button_text: Option<String>,
}

#[derive(Deserialize)]
struct BroadcastProduct {
    kind: String,
    id: String,
}

/// Build an inline keyboard with a single CTA that opens the Mini App on the
/// advertised product. Unknown catalog kinds are treated as "no markup" so a
/// typo in the admin UI does not break the whole broadcast.
///
/// Uses a WebApp button (not a plain URL) so the Mini App opens directly
/// inside Telegram with `start_param` populated from the deep link.
fn broadcast_reply_markup(
    web_app_url: &str,
    product: &BroadcastProduct,
    button_text: Option<&str>,
) -> Option<InlineKeyboardMarkup> {
    let prefix = match product.kind.as_str() {
        "strain" => "p_strain",
        "accessory" => "p_acc",
        "tea" => "p_tea",
        "set" => "p_set",
        "event" => "p_event",
        _ => return None,
    };
    let label = button_text
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("Открыть в магазине");

    // Build the Mini App URL with the product context. Telegram passes the
    // `startapp` value via `initDataUnsafe.start_param` when the button opens
    // the WebApp.
    let mini_app_url = build_web_app_url(web_app_url, &format!("{prefix}_{id}", id = product.id))?;
    let button = InlineKeyboardButton::web_app(label.to_string(), WebAppInfo { url: mini_app_url });
    Some(InlineKeyboardMarkup::new(vec![vec![button]]))
}

/// Append `startapp={param}` to the configured Mini App URL preserving any
/// existing query parameters (e.g. `?cache=180`). Falls back to a plain
/// `t.me` deep link if the configured URL is missing or invalid.
fn build_web_app_url(web_app_url: &str, startapp_param: &str) -> Option<url::Url> {
    if web_app_url.is_empty() {
        return None;
    }
    let mut url = web_app_url.parse::<url::Url>().ok()?;
    url.query_pairs_mut()
        .append_pair("startapp", startapp_param);
    Some(url)
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
        // Variant C оставила три маршрута поверх таблиц, которые миграция 083
        // удалила: `/admin/reviews`, `/admin/reviews/:id/moderate` и
        // `/admin/strains/:id/lab-cert` читали `strain_reviews` и
        // `lab_certificates`. Каждый вызов гарантированно отвечал 500.
        // Маршрут, который не может ответить, — не маршрут.
        .route("/admin/broadcast", post(telegram_broadcast))
        .route("/admin/broadcast/test", post(telegram_broadcast_test))
        .route(
            "/admin/delivery-zones",
            get(list_delivery_zones_admin).post(create_delivery_zone),
        )
        .route(
            "/admin/delivery-zones/:id",
            patch(update_delivery_zone).delete(delete_delivery_zone),
        )
}

async fn get_stats(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    // Cycle #92: SeaORM via raw `Statement` (pattern #15). All three queries
    // are simple aggregates; typed builder would be heavier than the SQL
    // itself.
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

    // Здесь стояли ещё две величины — `active_strains` и `top_strains`. Обе
    // читали `strains`, удалённую миграцией 083, и обе гасили ошибку:
    // `.ok()…unwrap_or(0)` и `.unwrap_or_default()`. Наружу уходило
    // `active_strains: 0` и `top_strains: []` — не «нет данных», а «ноль
    // товаров», то есть ровно тот случай, который D9 запрещает: отсутствие,
    // показанное как ноль. Админ-экран их и так не читал
    // (`admin_screen.rs` разбирает только `total_orders` и `total_revenue`),
    // так что удаление ничего не гасит на экране.
    //
    // Замены нет намеренно: метрика по парку — сколько байков свободно,
    // сколько в аренде — это новая величина, а не переименование старой.
    // Сущности `bike`/`bike_unit` для неё уже есть, запроса пока нет.
    Ok(Json(json!({
        "total_users": total_users,
        "total_orders": total_orders,
        "total_revenue": total_revenue,
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
    let text = crate::notify::notify_deploy_manual(&state.bot, &state.config, note).await;
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

// Здесь жили три обработчика Variant C: list_reviews_admin,
// moderate_review и create_lab_cert. Все три обращались к таблицам,
// удалённым миграцией 083 (strain_reviews, lab_certificates, strains),
// и отвечали 500 на каждый вызов. Удалены вместе со своими маршрутами.

async fn telegram_broadcast(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<TelegramBroadcastRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let admin_id =
        check_admin(&headers, &state).map_err(|e| (e, Json(json!({ "error": "unauthorized" }))))?;

    let text = req.text.trim();
    if text.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Текст рассылки пуст" })),
        ));
    }

    // Build optional marketing CTA: a t.me deep link that opens the Mini App
    // directly on the chosen product. Validate the product kind now so a typo
    // in the admin UI fails fast without consuming the rate-limit bucket.
    let reply_markup = req.product.as_ref().and_then(|p| {
        broadcast_reply_markup(&state.config.web_app_url, p, req.button_text.as_deref())
    });
    if req.product.is_some() && reply_markup.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Неизвестный тип товара" })),
        ));
    }

    // Decide between photo + caption and plain text. A photo message converts
    // better, but we keep text-only as a fallback when no image is supplied.
    let has_photo = req
        .photo_url
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .is_some();
    let photo_url = req.photo_url.clone().unwrap_or_default();
    if has_photo && crate::api::validate_url(&req.photo_url).is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Некорректный URL фото" })),
        ));
    }

    // Rate-limit by admin id. check_admin returns 0 for token-only logins,
    // which means all token-only admins share one bucket — acceptable because
    // there is typically only one such login and it prevents spam.
    let key = admin_id.to_string();
    if !crate::api::rate_limit::check_and_record_sync(
        &BROADCAST_RATE_LIMIT,
        &key,
        BROADCAST_RL_WINDOW,
        BROADCAST_RL_MAX_ATTEMPTS,
        BROADCAST_RL_MAX_KEYS,
    ) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": "Попробуйте через 10 минут" })),
        ));
    }

    // Telegram photo caption limit is 1024 characters. If the admin text is
    // longer, truncate with an ellipsis instead of failing the whole broadcast.
    let display_text = if has_photo && text.len() > 1024 {
        format!("{}…", &text[..1021])
    } else {
        text.to_string()
    };

    // Collect every unique telegram_id that has ever interacted with the bot.
    // user_languages is canonical, but loyalty_profiles also holds users who
    // placed orders without explicitly setting a language, so we union both.
    let user_ids: Vec<i64> = state
        .db
        .orm
        .query_all(Statement::from_string(
            DbBackend::Postgres,
            "SELECT telegram_id FROM user_languages \
             UNION \
             SELECT telegram_id FROM loyalty_profiles"
                .to_string(),
        ))
        .await
        .map_err(|e| {
            tracing::error!("telegram_broadcast: failed to load user ids: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Ошибка загрузки получателей" })),
            )
        })?
        .iter()
        .filter_map(|r| r.try_get::<i64>("", "telegram_id").ok())
        .collect();

    let mut sent = 0usize;
    let mut failed = 0usize;
    let mut dedup = HashSet::<i64>::new();
    for telegram_id in user_ids {
        if telegram_id <= 0 || !dedup.insert(telegram_id) {
            continue;
        }
        let result = if has_photo {
            let mut call = state
                .bot
                .send_photo(
                    teloxide::types::ChatId(telegram_id),
                    InputFile::url(photo_url.parse().map_err(|_| {
                        (
                            StatusCode::BAD_REQUEST,
                            Json(json!({ "error": "Некорректный URL фото" })),
                        )
                    })?),
                )
                .caption(display_text.clone())
                .parse_mode(ParseMode::Html);
            if let Some(ref markup) = reply_markup {
                call = call.reply_markup(markup.clone());
            }
            call.await
        } else {
            let mut call = state
                .bot
                .send_message(teloxide::types::ChatId(telegram_id), display_text.clone())
                .parse_mode(ParseMode::Html);
            if let Some(ref markup) = reply_markup {
                call = call.reply_markup(markup.clone());
            }
            call.await
        };
        match result {
            Ok(_) => {
                sent += 1;
                tracing::debug!("telegram_broadcast: sent to {}", telegram_id);
            }
            Err(e) => {
                failed += 1;
                tracing::warn!(
                    "telegram_broadcast: failed to send to {}: {}",
                    telegram_id,
                    e
                );
            }
        }
    }

    tracing::info!(
        "telegram_broadcast: admin_id={} sent={} failed={} recipients={}",
        admin_id,
        sent,
        failed,
        dedup.len()
    );
    Ok(Json(json!({
        "success": true,
        "sent": sent,
        "failed": failed,
        "recipients": dedup.len(),
    })))
}

/// Test-only broadcast: send the same rich message to every configured admin
/// without touching the main broadcast rate-limit bucket. This lets admins
/// preview the exact photo/caption/button layout before spamming all users.
async fn telegram_broadcast_test(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<TelegramBroadcastRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let admin_id =
        check_admin(&headers, &state).map_err(|e| (e, Json(json!({ "error": "unauthorized" }))))?;

    let text = req.text.trim();
    if text.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Текст рассылки пуст" })),
        ));
    }

    // Validate the marketing CTA shape up-front so the UI catches typos before
    // we send anything to a real admin chat.
    let reply_markup = req.product.as_ref().and_then(|p| {
        broadcast_reply_markup(&state.config.web_app_url, p, req.button_text.as_deref())
    });
    if req.product.is_some() && reply_markup.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Неизвестный тип товара" })),
        ));
    }

    let has_photo = req
        .photo_url
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .is_some();
    let photo_url = req.photo_url.clone().unwrap_or_default();
    if has_photo && crate::api::validate_url(&req.photo_url).is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Некорректный URL фото" })),
        ));
    }

    // NO rate-limit here — this is a preview to admins only.
    let display_text = if has_photo && text.len() > 1024 {
        format!("{}…", &text[..1021])
    } else {
        text.to_string()
    };

    let mut sent = 0usize;
    let mut failed = 0usize;
    let mut dedup = HashSet::<i64>::new();
    for admin_telegram_id in &state.config.admin_ids {
        let telegram_id = *admin_telegram_id;
        if telegram_id <= 0 || !dedup.insert(telegram_id) {
            continue;
        }
        let result = if has_photo {
            let mut call = state
                .bot
                .send_photo(
                    teloxide::types::ChatId(telegram_id),
                    InputFile::url(photo_url.parse().map_err(|_| {
                        (
                            StatusCode::BAD_REQUEST,
                            Json(json!({ "error": "Некорректный URL фото" })),
                        )
                    })?),
                )
                .caption(display_text.clone())
                .parse_mode(ParseMode::Html);
            if let Some(ref markup) = reply_markup {
                call = call.reply_markup(markup.clone());
            }
            call.await
        } else {
            let mut call = state
                .bot
                .send_message(teloxide::types::ChatId(telegram_id), display_text.clone())
                .parse_mode(ParseMode::Html);
            if let Some(ref markup) = reply_markup {
                call = call.reply_markup(markup.clone());
            }
            call.await
        };
        match result {
            Ok(_) => {
                sent += 1;
                tracing::debug!("telegram_broadcast_test: sent to admin {}", telegram_id);
            }
            Err(e) => {
                failed += 1;
                tracing::warn!(
                    "telegram_broadcast_test: failed to send to admin {}: {}",
                    telegram_id,
                    e
                );
            }
        }
    }

    tracing::info!(
        "telegram_broadcast_test: admin_id={} sent={} failed={} recipients={}",
        admin_id,
        sent,
        failed,
        dedup.len()
    );
    Ok(Json(json!({
        "success": true,
        "sent": sent,
        "failed": failed,
        "recipients": dedup.len(),
        "test": true,
    })))
}

// ── Loop #11: dynamic delivery zones admin CRUD ─────────────────────────────

#[derive(Deserialize)]
struct CreateZoneRequest {
    name: String,
    #[serde(default)]
    name_en: Option<String>,
    #[serde(default)]
    fee: Option<f64>,
    #[serde(default)]
    min_order: Option<f64>,
    #[serde(default)]
    eta_min: Option<i32>,
    #[serde(default)]
    eta_max: Option<i32>,
    #[serde(default)]
    sort_order: Option<i32>,
    #[serde(default)]
    is_active: Option<bool>,
}

#[derive(Deserialize)]
struct UpdateZoneRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    name_en: Option<Option<String>>,
    #[serde(default)]
    fee: Option<f64>,
    #[serde(default)]
    min_order: Option<f64>,
    #[serde(default, deserialize_with = "present_or_null")]
    eta_min: Option<Option<i32>>,
    #[serde(default, deserialize_with = "present_or_null")]
    eta_max: Option<Option<i32>>,
    #[serde(default)]
    sort_order: Option<i32>,
    #[serde(default)]
    is_active: Option<bool>,
}

fn sanitize_f64(v: f64) -> f64 {
    if v.is_finite() && v >= 0.0 {
        v
    } else {
        0.0
    }
}

fn validate_zone_name(name: &str) -> Result<(), StatusCode> {
    if name.is_empty() || name.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// A stored lower ETA edge from an admin write: a negative becomes 0.
fn clamp_eta_min(eta_min: i32) -> i32 {
    eta_min.max(0)
}

/// A stored upper ETA edge: never negative, never below a stored lower edge.
fn clamp_eta_max(eta_max: i32, eta_min: Option<i32>) -> i32 {
    eta_max.max(eta_min.unwrap_or(0)).max(0)
}

/// The ETA a created zone stores: what the admin sent, clamped, or nothing.
/// Until 2026-09-24 an absent edge was stored as 30 and 60 minutes, numbers
/// nobody measured; no ETA is published (owner, 2026-09-24), so migration 087
/// made both columns nullable and an absent edge now stays NULL.
fn created_zone_eta(eta_min: Option<i32>, eta_max: Option<i32>) -> (Option<i32>, Option<i32>) {
    (
        eta_min.map(clamp_eta_min),
        eta_max.map(|max| clamp_eta_max(max, eta_min)),
    )
}

/// The ETA an update writes, per edge: `None` leaves the column alone and
/// `Some(None)` clears it. The upper edge is floored by the lower one the row
/// will hold after this update.
fn updated_zone_eta(
    stored_min: Option<i32>,
    eta_min: Option<Option<i32>>,
    eta_max: Option<Option<i32>>,
) -> (Option<Option<i32>>, Option<Option<i32>>) {
    let floor = eta_min.unwrap_or(stored_min);
    (
        eta_min.map(|min| min.map(clamp_eta_min)),
        eta_max.map(|max| max.map(|max| clamp_eta_max(max, floor))),
    )
}

/// For an update body: an absent key is `None`, `null` is `Some(None)`, a
/// number is `Some(Some(n))`. serde alone reads `null` and "absent" alike.
fn present_or_null<'de, D>(deserializer: D) -> Result<Option<Option<i32>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<i32>::deserialize(deserializer).map(Some)
}

async fn list_delivery_zones_admin(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    use delivery_zone::{Column as ZoneCol, Entity as ZoneEntity};
    use sea_orm::{EntityTrait, QueryOrder};
    let models = ZoneEntity::find()
        .order_by_asc(ZoneCol::SortOrder)
        .order_by_asc(ZoneCol::Name)
        .all(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("list_delivery_zones_admin: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "zones": models })))
}

async fn create_delivery_zone(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<CreateZoneRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_zone_name(&req.name)?;
    let now = chrono::DateTime::from(chrono::Utc::now());
    let (eta_min, eta_max) = created_zone_eta(req.eta_min, req.eta_max);
    let am = delivery_zone::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        name: Set(req.name),
        name_en: Set(req.name_en),
        fee: Set(req.fee.map(sanitize_f64).unwrap_or(0.0)),
        min_order: Set(req.min_order.map(sanitize_f64).unwrap_or(0.0)),
        eta_min: Set(eta_min),
        eta_max: Set(eta_max),
        sort_order: Set(req.sort_order.unwrap_or(0)),
        is_active: Set(req.is_active.unwrap_or(true)),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
    };
    let model = am.insert(&state.db.orm).await.map_err(|e| {
        tracing::error!("create_delivery_zone insert: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "zone": model })))
}

async fn update_delivery_zone(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateZoneRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let id_uuid = uuid::Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    use delivery_zone::Entity as ZoneEntity;
    use sea_orm::EntityTrait;
    let model = ZoneEntity::find_by_id(id_uuid)
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("update_delivery_zone lookup: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut am: delivery_zone::ActiveModel = model.into();
    if let Some(name) = req.name {
        validate_zone_name(&name)?;
        am.name = Set(name);
    }
    if let Some(name_en) = req.name_en {
        am.name_en = Set(name_en);
    }
    if let Some(fee) = req.fee {
        am.fee = Set(sanitize_f64(fee));
    }
    if let Some(min_order) = req.min_order {
        am.min_order = Set(sanitize_f64(min_order));
    }
    let (eta_min, eta_max) =
        updated_zone_eta(am.eta_min.clone().unwrap(), req.eta_min, req.eta_max);
    if let Some(eta_min) = eta_min {
        am.eta_min = Set(eta_min);
    }
    if let Some(eta_max) = eta_max {
        am.eta_max = Set(eta_max);
    }
    if let Some(sort_order) = req.sort_order {
        am.sort_order = Set(sort_order);
    }
    if let Some(is_active) = req.is_active {
        am.is_active = Set(is_active);
    }
    am.updated_at = Set(Some(chrono::DateTime::from(chrono::Utc::now())));
    let model = am.update(&state.db.orm).await.map_err(|e| {
        tracing::error!("update_delivery_zone update: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "zone": model })))
}

async fn delete_delivery_zone(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let id_uuid = uuid::Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    use delivery_zone::Entity as ZoneEntity;
    use sea_orm::EntityTrait;
    let res = ZoneEntity::delete_by_id(id_uuid)
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("delete_delivery_zone: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if res.rows_affected == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(json!({ "success": true })))
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

#[cfg(test)]
mod zone_eta_tests {
    use super::*;

    #[test]
    fn a_zone_created_without_minutes_stores_no_eta() {
        let req: CreateZoneRequest =
            serde_json::from_str(r#"{"name":"Патонг","name_en":"Patong","fee":290}"#)
                .expect("a zone needs no ETA");
        assert_eq!(created_zone_eta(req.eta_min, req.eta_max), (None, None));
    }

    #[test]
    fn a_created_zone_stores_the_edges_it_was_given_and_invents_none() {
        assert_eq!(created_zone_eta(Some(20), None), (Some(20), None));
        assert_eq!(created_zone_eta(None, Some(40)), (None, Some(40)));
        assert_eq!(created_zone_eta(Some(-5), Some(-1)), (Some(0), Some(0)));
        assert_eq!(created_zone_eta(Some(30), Some(10)), (Some(30), Some(30)));
    }

    #[test]
    fn an_update_tells_an_absent_key_from_a_null() {
        let absent: UpdateZoneRequest = serde_json::from_str("{}").expect("empty body");
        assert_eq!((absent.eta_min, absent.eta_max), (None, None));
        let cleared: UpdateZoneRequest =
            serde_json::from_str(r#"{"eta_min":null,"eta_max":null}"#).expect("nulls");
        assert_eq!((cleared.eta_min, cleared.eta_max), (Some(None), Some(None)));
        let set: UpdateZoneRequest =
            serde_json::from_str(r#"{"eta_min":15,"eta_max":25}"#).expect("numbers");
        assert_eq!((set.eta_min, set.eta_max), (Some(Some(15)), Some(Some(25))));
    }

    #[test]
    fn an_update_leaves_clears_or_sets_each_edge() {
        assert_eq!(updated_zone_eta(Some(20), None, None), (None, None));
        assert_eq!(
            updated_zone_eta(Some(20), Some(None), Some(None)),
            (Some(None), Some(None))
        );
        assert_eq!(
            updated_zone_eta(Some(20), None, Some(Some(10))),
            (None, Some(Some(20))),
            "the stored lower edge floors a new upper one"
        );
        assert_eq!(
            updated_zone_eta(Some(20), Some(None), Some(Some(10))),
            (Some(None), Some(Some(10))),
            "a cleared lower edge floors nothing"
        );
    }
}
