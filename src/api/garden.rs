//! Garden API endpoints

use crate::api::auth::{check_admin, check_not_blocked, validate_telegram_id_param};
use crate::trios::garden;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        // User plants — `POST /garden/plants` (plant_seed handler)
        // removed in cycle #169. Seeding is now an automatic
        // side-effect of `complete_order_and_update_loyalty` (cycle
        // #168); the manual endpoint had zero callers across the UI
        // and backend.
        .route("/garden/plants", get(get_user_plants))
        .route("/garden/plants/:id/water", post(water_plant))
        .route("/garden/plants/:id/harvest", post(harvest_plant))
        // Rewards
        .route("/garden/rewards", get(get_user_rewards))
        .route("/garden/rewards/:id/use", post(use_reward))
        // Config
        .route("/garden/config", get(get_config))
        .route("/garden/config", put(update_config))
}

// ── Request/Response Types ────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct UserPlantsQuery {
    pub telegram_id: i64,
}

// Cycle #169: `PlantSeedRequest` removed alongside the handler.
// Forward-write of garden_plants is done inside
// `complete_order_and_update_loyalty` (cycle #168) using the order's
// own `items` JSONB — no separate request type needed.

#[derive(Debug, Serialize)]
pub(crate) struct PlantResponse {
    pub id: String,
    pub user_id: String,
    pub strain_id: String,
    pub strain_name: String,
    pub current_stage: String,
    pub stage_name: String,
    pub stage_emoji: String,
    pub planted_at: i64,
    pub is_completed: bool,
    pub harvested_at: Option<i64>,
    pub reward_claimed: bool,
    pub water_count: u32,
    pub progress: u8,
    pub can_water: bool,
    pub next_water_at: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct RewardResponse {
    pub id: String,
    pub plant_id: String,
    pub strain_name: String,
    pub discount_percent: u32,
    pub bonus_points: u32,
    pub expires_at: i64,
    pub is_used: bool,
    pub is_active: bool,
}

// ── Plant Endpoints ───────────────────────────────────────────────

async fn get_user_plants(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<UserPlantsQuery>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(query.telegram_id)?;
    crate::api::auth::check_owner(&headers, &state, query.telegram_id)?;
    check_not_blocked(&state, query.telegram_id).await?;
    // Cycle #94: SeaORM via Statement. No `garden_plant` entity — wire
    // shape (`PlantResponse`) is a heavily-derived view (`progress`,
    // `stage_name`, `stage_emoji` computed from raw columns), so an
    // entity would only model 11 of the JSON fields anyway.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let user_id = query.telegram_id.to_string();

    let rows = state
        .db
        .orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id, user_id, strain_id, strain_name, current_stage, planted_at, \
                is_completed, harvested_at, reward_claimed, water_count, last_watered_at \
         FROM garden_plants \
         WHERE user_id = $1 AND is_completed = false \
         ORDER BY planted_at DESC LIMIT 200",
            [user_id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("get_user_plants: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let now = chrono::Utc::now().timestamp_millis();
    let plants: Vec<PlantResponse> = rows
        .iter()
        .map(|r| {
            let db_stage = r.try_get::<String>("", "current_stage").unwrap_or_default();
            let current_stage =
                garden::GrowthStage::from_db_name(&db_stage).unwrap_or(garden::GrowthStage::Final);
            let plant = garden::Plant {
                id: r.try_get("", "id").unwrap_or_default(),
                user_id: r.try_get("", "user_id").unwrap_or_default(),
                strain_id: r.try_get("", "strain_id").unwrap_or_default(),
                strain_name: r.try_get("", "strain_name").unwrap_or_default(),
                current_stage,
                planted_at: r.try_get("", "planted_at").unwrap_or(0),
                is_completed: r.try_get("", "is_completed").unwrap_or(false),
                harvested_at: r.try_get("", "harvested_at").ok(),
                reward_claimed: r.try_get("", "reward_claimed").unwrap_or(false),
                water_count: r.try_get("", "water_count").unwrap_or(0),
                last_watered_at: r.try_get("", "last_watered_at").ok(),
            };

            let progress = garden::calculate_progress(&plant, now);
            PlantResponse {
                id: plant.id,
                user_id: plant.user_id,
                strain_id: plant.strain_id,
                strain_name: plant.strain_name,
                current_stage: plant.current_stage.db_name().to_string(),
                stage_name: progress.stage_name,
                stage_emoji: progress.stage_emoji,
                planted_at: plant.planted_at,
                is_completed: plant.is_completed,
                harvested_at: plant.harvested_at,
                reward_claimed: plant.reward_claimed,
                water_count: plant.water_count,
                progress: progress.total_progress,
                can_water: progress.can_water,
                next_water_at: progress.next_water_at,
            }
        })
        .collect();

    Ok(Json(json!({ "plants": plants })))
}

// Cycle #169: `plant_seed` handler + `validate_plant_seed_request`
// validator removed. The forward-write of `garden_plants` is now
// the cycle-#168 side-effect of `complete_order_and_update_loyalty`
// — the manual endpoint had zero callers in the UI or backend.

async fn water_plant(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Cycle #94: SeaORM tx with raw SELECT … FOR UPDATE (Statement-based;
    // no garden_plant entity to use `lock_exclusive` against). Conditional
    // UPDATE with cooldown guard preserved via WHERE clause.
    use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};

    let now = chrono::Utc::now().timestamp_millis();
    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("water_plant tx.begin: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let row = tx
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT user_id, current_stage, is_completed, water_count, last_watered_at \
             FROM garden_plants \
             WHERE id = $1 \
             FOR UPDATE",
            [id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("water_plant FOR UPDATE: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let Some(r) = row else {
        return Ok(Json(
            json!({ "success": false, "error": "Plant not found" }),
        ));
    };

    let user_id: String = r.try_get("", "user_id").unwrap_or_default();
    let tid = user_id
        .parse::<i64>()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::api::auth::check_owner(&headers, &state, tid)?;
    check_not_blocked(&state, tid).await?;

    let is_completed: bool = r.try_get("", "is_completed").unwrap_or(false);
    let water_count: i32 = r.try_get("", "water_count").unwrap_or(0);
    let last_watered_at: Option<i64> = r.try_get("", "last_watered_at").ok();

    if is_completed {
        return Ok(Json(
            json!({ "success": false, "error": "Plant already completed" }),
        ));
    }

    if water_count >= 13 {
        return Ok(Json(
            json!({ "success": false, "error": "Plant already at final stage" }),
        ));
    }
    let new_count = water_count
        .checked_add(1)
        .ok_or(StatusCode::BAD_REQUEST)?
        .max(0) as u32;
    let new_stage = garden::GrowthStage::from_index(new_count as usize)
        .unwrap_or(garden::GrowthStage::Final)
        .db_name()
        .to_string();
    let new_completed = new_count >= 13;
    let cooldown_ms = garden::WATER_COOLDOWN_MS;
    let max_last_water = now.saturating_sub(cooldown_ms);

    let rows = tx
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE garden_plants \
             SET water_count = $1, current_stage = $2, is_completed = $3, last_watered_at = $4 \
             WHERE id = $5 AND (last_watered_at IS NULL OR last_watered_at <= $6)",
            [
                (new_count as i32).into(),
                new_stage.clone().into(),
                new_completed.into(),
                now.into(),
                id.into(),
                max_last_water.into(),
            ],
        ))
        .await
        .map_err(|e| {
            tracing::error!("water_plant update: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if rows.rows_affected() == 0 {
        let next_water_at = last_watered_at
            .map(|t| t.saturating_add(garden::WATER_COOLDOWN_MS))
            .unwrap_or(now);
        // tx drops → auto-rollback.
        return Ok(Json(json!({
            "success": false,
            "error": "Cooldown active",
            "next_water_at": next_water_at
        })));
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("water_plant commit: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({
        "success": true,
        "water_count": new_count,
        "current_stage": new_stage,
        "is_completed": new_completed
    })))
}

async fn harvest_plant(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Cycle #94: SeaORM tx. FOR UPDATE row lock + config read + atomic
    // UPDATE (WHERE harvested_at IS NULL) + INSERT reward. Drop-rollback
    // removes the explicit .rollback() arm.
    use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};

    let now = chrono::Utc::now().timestamp_millis();
    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("harvest_plant tx.begin: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let row = tx
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT user_id, strain_id, strain_name, is_completed, harvested_at \
             FROM garden_plants \
             WHERE id = $1 \
             FOR UPDATE",
            [id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("harvest_plant FOR UPDATE: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let Some(r) = row else {
        return Ok(Json(
            json!({ "success": false, "error": "Plant not found" }),
        ));
    };

    let user_id: String = r.try_get("", "user_id").unwrap_or_default();
    let tid = user_id
        .parse::<i64>()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::api::auth::check_owner(&headers, &state, tid)?;
    check_not_blocked(&state, tid).await?;

    let is_completed: bool = r.try_get("", "is_completed").unwrap_or(false);
    let harvested_at: Option<i64> = r.try_get("", "harvested_at").ok();

    if !is_completed {
        return Ok(Json(
            json!({ "success": false, "error": "Plant not ready for harvest" }),
        ));
    }

    if harvested_at.is_some() {
        return Ok(Json(
            json!({ "success": false, "error": "Already harvested" }),
        ));
    }

    let strain_id: String = r.try_get("", "strain_id").unwrap_or_default();
    let strain_name: String = r.try_get("", "strain_name").unwrap_or_default();

    let reward_id = uuid::Uuid::new_v4().to_string();

    // Read garden config inside transaction for consistency
    let config_row = tx.query_one(Statement::from_string(
        DbBackend::Postgres,
        "SELECT reward_discount_percent, reward_bonus_points, reward_expiration_days FROM garden_config WHERE id = 1".to_string(),
    )).await.map_err(|e| {
        tracing::error!("harvest_plant config: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let (discount_percent, bonus_points, expiration_days) = match config_row {
        Some(r) => (
            r.try_get::<i32>("", "reward_discount_percent")
                .unwrap_or(10),
            r.try_get::<i32>("", "reward_bonus_points").unwrap_or(100),
            r.try_get::<i32>("", "reward_expiration_days").unwrap_or(7),
        ),
        None => (10, 100, 7),
    };
    let expires_at = now.saturating_add(expiration_days as i64 * 24 * 60 * 60 * 1000);

    // Atomically mark plant as harvested — WHERE harvested_at IS NULL prevents race
    let rows = tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE garden_plants SET harvested_at = $1, reward_claimed = true WHERE id = $2 AND harvested_at IS NULL AND reward_claimed = false",
        [now.into(), id.clone().into()],
    )).await.map_err(|e| {
        tracing::error!("harvest_plant update: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if rows.rows_affected() == 0 {
        // tx drops → auto-rollback.
        return Ok(Json(
            json!({ "success": false, "error": "Already harvested" }),
        ));
    }

    // Create reward
    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO garden_rewards (id, plant_id, user_id, strain_id, strain_name, \
                                    discount_percent, bonus_points, expires_at, is_used, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        [
            reward_id.clone().into(),
            id.into(),
            user_id.clone().into(),
            strain_id.into(),
            strain_name.clone().into(),
            discount_percent.into(),
            bonus_points.into(),
            expires_at.into(),
            false.into(),
            now.into(),
        ],
    )).await.map_err(|e| {
        tracing::error!("harvest_plant insert reward: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await.map_err(|e| {
        tracing::error!("harvest_plant commit: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Notify admins about garden reward
    let bot = state.bot.clone();
    let config = state.config.clone();
    let notify_discount = discount_percent;
    let notify_bonus = bonus_points;
    tokio::spawn(async move {
        // Cycle #149: notify_admins now sends with parse_mode=Html, so
        // user-controlled fields must be escaped at the format site.
        // `strain_name` is admin-controlled (catalog), but admins still
        // could enter "Critical <Mass>" — escape to be safe.
        let text = format!(
            "\u{1F33F} Garden reward \u{0432}\u{044B}\u{0434}\u{0430}\u{043D}\n\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\u{1F194} {}\n\u{1F381} {} ({}% / {}pts)",
            user_id, crate::util::html_escape(&strain_name), notify_discount, notify_bonus
        );
        crate::notify::notify_admins(&bot, &config, &text).await;
    });

    crate::metrics::garden_reward_claimed();

    Ok(Json(json!({
        "success": true,
        "reward_id": reward_id,
        "discount_percent": discount_percent,
        "bonus_points": bonus_points,
        "expires_at": expires_at
    })))
}

// ── Reward Endpoints ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct UserRewardsQuery {
    pub telegram_id: i64,
}

async fn get_user_rewards(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<UserRewardsQuery>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(query.telegram_id)?;
    crate::api::auth::check_owner(&headers, &state, query.telegram_id)?;
    check_not_blocked(&state, query.telegram_id).await?;
    // Cycle #94: SeaORM via Statement.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let user_id = query.telegram_id.to_string();
    let now = chrono::Utc::now().timestamp_millis();

    let rows = state.db.orm.query_all(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id, plant_id, strain_name, discount_percent, bonus_points, expires_at, is_used \
         FROM garden_rewards \
         WHERE user_id = $1 \
         ORDER BY created_at DESC LIMIT 200",
        [user_id.into()],
    )).await.map_err(|e| {
        tracing::error!("get_user_rewards: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let rewards: Vec<RewardResponse> = rows
        .iter()
        .map(|r| {
            let expires_at: i64 = r.try_get("", "expires_at").unwrap_or(0);
            let is_used: bool = r.try_get("", "is_used").unwrap_or(false);
            RewardResponse {
                id: r.try_get("", "id").unwrap_or_default(),
                plant_id: r.try_get("", "plant_id").unwrap_or_default(),
                strain_name: r.try_get("", "strain_name").unwrap_or_default(),
                discount_percent: r.try_get::<i32>("", "discount_percent").unwrap_or(0).max(0)
                    as u32,
                bonus_points: r.try_get::<i32>("", "bonus_points").unwrap_or(0).max(0) as u32,
                expires_at,
                is_used,
                is_active: !is_used && expires_at > now,
            }
        })
        .collect();

    Ok(Json(json!({ "rewards": rewards })))
}

async fn use_reward(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Cycle #94: SeaORM tx — finally migrates the last db.pool.get() in
    // garden.rs. Uses typed entity for the loyalty upsert (cycle #82's
    // `loyalty_profile`) but raw Statement for the reward + balance
    // updates (no garden_reward entity warranted by single-call site).
    use crate::db::entities::loyalty_profile::{
        ActiveModel as LpAm, Column as LpCol, Entity as LpEntity,
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{
        ActiveValue::Set, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
        Statement, TransactionTrait,
    };

    let now = chrono::Utc::now().timestamp_millis();

    // Read reward (read-only, outside tx — auth happens here).
    let r = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT user_id, is_used, expires_at, discount_percent, bonus_points \
         FROM garden_rewards \
         WHERE id = $1",
            [id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("use_reward query: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let Some(r) = r else {
        return Ok(Json(
            json!({ "success": false, "error": "Reward not found" }),
        ));
    };

    let user_id: String = r.try_get("", "user_id").unwrap_or_default();
    let tid = user_id
        .parse::<i64>()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::api::auth::check_owner(&headers, &state, tid)?;
    check_not_blocked(&state, tid).await?;

    let is_used: bool = r.try_get("", "is_used").unwrap_or(false);
    let expires_at: i64 = r.try_get("", "expires_at").unwrap_or(0);

    if is_used {
        return Ok(Json(
            json!({ "success": false, "error": "Reward already used" }),
        ));
    }

    if expires_at < now {
        return Ok(Json(json!({ "success": false, "error": "Reward expired" })));
    }

    let discount_percent: i32 = r.try_get("", "discount_percent").unwrap_or(0);
    let bonus_points: i32 = r.try_get("", "bonus_points").unwrap_or(0);

    // Atomically mark used and credit bonus inside a transaction.
    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("use_reward tx.begin: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let rows = tx
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE garden_rewards SET is_used = true WHERE id = $1 AND is_used = false",
            [id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("use_reward mark used: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if rows.rows_affected() == 0 {
        // tx drops → auto-rollback.
        return Ok(Json(
            json!({ "success": false, "error": "Reward already used" }),
        ));
    }

    let bonus_f64 = bonus_points as f64;
    // Idempotent profile seed via pattern #9.
    let lp_seed = LpAm {
        telegram_id: Set(tid),
        bonus_balance: Set(Some(0.0)),
        total_spent: Set(Some(0.0)),
        ..Default::default()
    };
    LpEntity::insert(lp_seed)
        .on_conflict(
            OnConflict::column(LpCol::TelegramId)
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("use_reward loyalty seed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    // Bump balance via column-expr UPDATE (pattern #11).
    let updated = LpEntity::update_many()
        .col_expr(
            LpCol::BonusBalance,
            sea_orm::sea_query::Expr::cust_with_values("bonus_balance + $1", [bonus_f64]),
        )
        .filter(LpCol::TelegramId.eq(tid))
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("use_reward balance bump: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if updated.rows_affected == 0 {
        tracing::error!(
            "use_reward: loyalty profile missing for telegram_id={} after upsert (race?)",
            tid
        );
        // tx drops → auto-rollback.
        return Ok(Json(
            json!({ "success": false, "error": "Loyalty profile not found" }),
        ));
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("use_reward commit: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({
        "success": true,
        "discount_percent": discount_percent,
        "bonus_points": bonus_points
    })))
}

// ── Config Endpoints ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct ConfigUpdateRequest {
    pub is_enabled: Option<bool>,
    pub reward_discount_percent: Option<u32>,
    pub reward_bonus_points: Option<u32>,
    pub reward_expiration_days: Option<u32>,
}

async fn get_config(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // Cycle #94: SeaORM via Statement (no entity — singleton `garden_config` row).
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let row = state.db.orm.query_one(Statement::from_string(
        DbBackend::Postgres,
        "SELECT is_enabled, reward_discount_percent, reward_bonus_points, reward_expiration_days \
         FROM garden_config \
         LIMIT 1".to_string(),
    )).await.map_err(|e| {
        tracing::error!("get_config: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let config = match row {
        Some(r) => garden::GameConfig {
            is_enabled: r.try_get("", "is_enabled").unwrap_or(false),
            reward_discount_percent: r
                .try_get::<i32>("", "reward_discount_percent")
                .unwrap_or(0)
                .max(0) as u32,
            reward_bonus_points: r
                .try_get::<i32>("", "reward_bonus_points")
                .unwrap_or(0)
                .max(0) as u32,
            reward_expiration_days: r
                .try_get::<i32>("", "reward_expiration_days")
                .unwrap_or(0)
                .max(0) as u32,
        },
        None => garden::GameConfig::default(),
    };

    Ok(Json(json!({
        "is_enabled": config.is_enabled,
        "reward_discount_percent": config.reward_discount_percent as f64,
        "reward_bonus_points": config.reward_bonus_points as f64,
        "reward_expiration_days": config.reward_expiration_days as f64,
    })))
}

fn validate_garden_config_update(req: &ConfigUpdateRequest) -> Result<(), StatusCode> {
    if let Some(p) = req.reward_discount_percent {
        if p > 100 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(p) = req.reward_bonus_points {
        if p > 1_000_000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(d) = req.reward_expiration_days {
        if d == 0 || d > 365 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    Ok(())
}

async fn update_config(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<ConfigUpdateRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_garden_config_update(&req)?;
    // Cycle #94: SeaORM via Statement. COALESCE($n, col) partial-update
    // semantics preserved.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    // BUG-5: is_enabled в БД — BOOLEAN, раньше передавался i32 → type mismatch.
    let is_enabled = req.is_enabled;
    let reward_discount_percent = req.reward_discount_percent.map(|p| p as i32);
    let reward_bonus_points = req.reward_bonus_points.map(|p| p as i32);
    let reward_expiration_days = req.reward_expiration_days.map(|d| d as i32);
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE garden_config \
         SET is_enabled = COALESCE($1, is_enabled), \
             reward_discount_percent = COALESCE($2, reward_discount_percent), \
             reward_bonus_points = COALESCE($3, reward_bonus_points), \
             reward_expiration_days = COALESCE($4, reward_expiration_days), \
             updated_at = NOW() \
         WHERE id = 1",
            [
                is_enabled.into(),
                reward_discount_percent.into(),
                reward_bonus_points.into(),
                reward_expiration_days.into(),
            ],
        ))
        .await
        .map_err(|e| {
            tracing::error!("update_config: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(json!({ "success": true })))
}

#[cfg(test)]
mod tests {
    use super::{validate_garden_config_update, ConfigUpdateRequest};
    use axum::http::StatusCode;

    #[test]
    fn test_validate_garden_config_ok() {
        let req = ConfigUpdateRequest {
            is_enabled: Some(true),
            reward_discount_percent: Some(50),
            reward_bonus_points: Some(100),
            reward_expiration_days: Some(7),
        };
        assert!(validate_garden_config_update(&req).is_ok());
    }

    #[test]
    fn test_validate_garden_config_all_none() {
        let req = ConfigUpdateRequest {
            is_enabled: None,
            reward_discount_percent: None,
            reward_bonus_points: None,
            reward_expiration_days: None,
        };
        assert!(validate_garden_config_update(&req).is_ok());
    }

    #[test]
    fn test_validate_garden_config_discount_too_high() {
        let req = ConfigUpdateRequest {
            is_enabled: None,
            reward_discount_percent: Some(101),
            reward_bonus_points: None,
            reward_expiration_days: None,
        };
        assert_eq!(
            validate_garden_config_update(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_garden_config_bonus_too_high() {
        let req = ConfigUpdateRequest {
            is_enabled: None,
            reward_discount_percent: None,
            reward_bonus_points: Some(1_000_001),
            reward_expiration_days: None,
        };
        assert_eq!(
            validate_garden_config_update(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_garden_config_days_zero() {
        let req = ConfigUpdateRequest {
            is_enabled: None,
            reward_discount_percent: None,
            reward_bonus_points: None,
            reward_expiration_days: Some(0),
        };
        assert_eq!(
            validate_garden_config_update(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_garden_config_days_too_high() {
        let req = ConfigUpdateRequest {
            is_enabled: None,
            reward_discount_percent: None,
            reward_bonus_points: None,
            reward_expiration_days: Some(366),
        };
        assert_eq!(
            validate_garden_config_update(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    // Cycle #169: plant_seed validator tests removed alongside the
    // handler. The forward-write of garden_plants happens inside
    // `complete_order_and_update_loyalty` (cycle #168) and is
    // covered by `tests/integration_garden_seed.rs`.
}
