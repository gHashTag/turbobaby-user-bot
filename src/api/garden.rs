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
        .route("/garden/force-seed", post(force_seed))
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
    pub last_watered_at: Option<i64>,
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
    tracing::info!(user_id = %user_id, "get_user_plants query start");

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
            tracing::error!(user_id = %user_id, error = %e, "get_user_plants query failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!(user_id = %user_id, row_count = rows.len(), "get_user_plants query done");

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
                last_watered_at: plant.last_watered_at,
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

    // Fail loud on the state reads: this is a state mutation. Wave #38 made a
    // corrupt *value* (negative count) fail via next_water_step, but a read
    // *error* on `water_count` would still `.unwrap_or(0)` → silently reset the
    // plant to Seed; a read error on `is_completed` would let a finished plant
    // be re-watered. Propagate the DbErr (tx auto-rolls back on the `?`).
    let is_completed: bool = r.try_get("", "is_completed").map_err(|e| {
        tracing::error!("water_plant: corrupt is_completed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let water_count: i32 = r.try_get("", "water_count").map_err(|e| {
        tracing::error!("water_plant: corrupt water_count: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let last_watered_at: Option<i64> = r.try_get("", "last_watered_at").ok();

    if is_completed {
        return Ok(Json(
            json!({ "success": false, "error": "Plant already completed" }),
        ));
    }

    // Validate the persisted count via the pure core (mirrors Plant::water).
    // A corrupt (negative / out-of-range) count must fail loud, NOT be silently
    // clamped to Seed — silent clamping would hide data rot and reset the
    // player's plant. `>= 13` is a friendly "already final" no-op.
    let (new_count, new_stage, new_completed) = match garden::next_water_step(water_count) {
        garden::WaterStep::Advance {
            new_count,
            stage,
            completed,
        } => (new_count, stage.to_string(), completed),
        garden::WaterStep::AtFinalStage => {
            return Ok(Json(
                json!({ "success": false, "error": "Plant already at final stage" }),
            ));
        }
        garden::WaterStep::Corrupt => {
            tracing::error!(
                "water_plant: corrupt water_count={} for plant id={} — refusing to clamp",
                water_count,
                id
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
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
    let (raw_discount, raw_bonus, raw_days) = match config_row {
        Some(r) => (
            r.try_get::<i32>("", "reward_discount_percent")
                .unwrap_or(10),
            r.try_get::<i32>("", "reward_bonus_points").unwrap_or(100),
            r.try_get::<i32>("", "reward_expiration_days").unwrap_or(7),
        ),
        None => (10, 100, 7),
    };
    // Sanitize before these become a financial reward: a corrupt/negative config
    // value (only possible via DB tampering — the admin API is u32) must never
    // subtract from a user's bonus_balance via use_reward. Fail loud (log), not
    // silent. See trios::garden::sanitize_reward_config.
    let sane = garden::sanitize_reward_config(raw_discount, raw_bonus, raw_days);
    if sane.corrupted {
        tracing::error!(
            "harvest_plant: garden_config out of range (discount={raw_discount}, \
             bonus={raw_bonus}, days={raw_days}) — clamped to ({}, {}, {}); check garden_config row",
            sane.discount_percent,
            sane.bonus_points,
            sane.expiration_days,
        );
    }
    let (discount_percent, bonus_points, expiration_days) = (
        sane.discount_percent,
        sane.bonus_points,
        sane.expiration_days,
    );
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
                is_active: garden::reward_is_active(is_used, expires_at, now),
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

    // FAIL LOUD on the reward-row reads: this is a financial mutation. A silent
    // `.unwrap_or(0)` on `bonus_points`/`discount_percent` would let the user
    // consume the reward (marked is_used) while being credited nothing — silent
    // value loss with no downstream guard. Propagate the DbErr → 500 (the tx
    // hasn't started yet, so nothing is consumed). Mirrors loyalty.rs, which
    // propagates try_get errors on financial fields.
    fn read_err(field: &'static str) -> impl Fn(sea_orm::DbErr) -> StatusCode {
        move |e: sea_orm::DbErr| {
            tracing::error!("use_reward: corrupt reward field {field}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
    let is_used: bool = r.try_get("", "is_used").map_err(read_err("is_used"))?;
    let expires_at: i64 = r
        .try_get("", "expires_at")
        .map_err(read_err("expires_at"))?;
    let discount_percent: i32 = r
        .try_get("", "discount_percent")
        .map_err(read_err("discount_percent"))?;
    let bonus_points: i32 = r
        .try_get("", "bonus_points")
        .map_err(read_err("bonus_points"))?;

    if is_used {
        return Ok(Json(
            json!({ "success": false, "error": "Reward already used" }),
        ));
    }

    // `<= now`: a reward at exactly its expiry is expired — unified with the
    // rewards-list `reward_is_active` boundary (which uses `expires_at > now`).
    if !garden::reward_is_active(is_used, expires_at, now) {
        return Ok(Json(json!({ "success": false, "error": "Reward expired" })));
    }

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

// Cycle #169F: emergency self-service seeding endpoint. Users whose
// completed orders somehow failed to trigger the automatic garden
// seeding (e.g. migration gaps, zero telegram_id in orders, etc.) can
// hit this endpoint and receive a seed immediately.
#[derive(Debug, Deserialize)]
pub(crate) struct ForceSeedRequest {
    pub telegram_id: i64,
}

// `first_seedable_item` moved to the pure core `trios::garden` (cycle: Wave #66
// clone-dedup) so the order-completion side-effect and `force_seed` share one
// SSOT for seed precedence. Call `garden::first_seedable_item`.

async fn force_seed(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<ForceSeedRequest>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(req.telegram_id)?;
    crate::api::auth::check_owner(&headers, &state, req.telegram_id)?;
    check_not_blocked(&state, req.telegram_id).await?;

    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let tid = req.telegram_id;
    let user_id = tid.to_string();

    // 1. Guard: already has an active plant?
    let has_active = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT 1 FROM garden_plants WHERE user_id = $1 AND is_completed = false LIMIT 1",
            [user_id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("force_seed: active plant check failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if has_active.is_some() {
        return Ok(Json(
            json!({ "success": false, "error": "already_has_plant" }),
        ));
    }

    // 1b. Guard: harvested within the cooldown? Bug fix (discounts too frequent)
    // — cap new garden discounts at ~once per day. Mirrors the same gate added to
    // the order-completion seed path (db::orders) so neither route can re-seed
    // back-to-back after a harvest.
    let now_ms = chrono::Utc::now().timestamp_millis();
    let cooldown_floor = now_ms.saturating_sub(garden::POST_HARVEST_COOLDOWN_MS);
    let recent_harvest = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT 1 FROM garden_plants \
             WHERE user_id = $1 AND harvested_at IS NOT NULL AND harvested_at > $2 LIMIT 1",
            [user_id.clone().into(), cooldown_floor.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("force_seed: harvest cooldown check failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if recent_harvest.is_some() {
        return Ok(Json(
            json!({ "success": false, "error": "harvest_cooldown" }),
        ));
    }

    // 2. Find the earliest confirmed/completed order with a catalog item.
    // Cycle #169G: expanded from 'completed' to ('completed','confirmed','ready')
    // because admins sometimes forget to press "Complete" after confirming.
    let order_row = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id, items FROM orders WHERE telegram_id = $1 AND status IN ('completed','confirmed','ready') ORDER BY created_at ASC LIMIT 1",
            [tid.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("force_seed: order lookup failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let Some(order_row) = order_row else {
        return Ok(Json(
            json!({ "success": false, "error": "no_completed_orders" }),
        ));
    };

    let items: serde_json::Value = order_row
        .try_get("", "items")
        .unwrap_or(serde_json::Value::Null);

    let Some((strain_id, strain_name)) = garden::first_seedable_item(&items) else {
        return Ok(Json(
            json!({ "success": false, "error": "no_catalog_items" }),
        ));
    };

    // 3. Plant the seed
    let plant_id = uuid::Uuid::new_v4().to_string();
    let planted_at = chrono::Utc::now().timestamp_millis();
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO garden_plants \
                  (id, user_id, strain_id, strain_name, current_stage, planted_at, is_completed, water_count) \
             VALUES ($1, $2, $3, $4, 'seed', $5, false, 0)",
            [
                plant_id.clone().into(),
                user_id.into(),
                strain_id.into(),
                strain_name.clone().into(),
                planted_at.into(),
            ],
        ))
        .await
        .map_err(|e| {
            tracing::error!("force_seed: insert failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!(telegram_id = tid, "force_seed: emergency seed planted");
    Ok(Json(
        json!({ "success": true, "plant_id": plant_id, "strain_name": strain_name }),
    ))
}

#[cfg(test)]
mod tests {
    use super::{validate_garden_config_update, ConfigUpdateRequest};
    use crate::trios::garden::first_seedable_item;
    use axum::http::StatusCode;
    use serde_json::json;

    // Regression guard for the garden seed-backfill firefighting (migrations
    // 026/036/037, force_seed): which order item becomes the planted seed.

    #[test]
    fn seedable_picks_strain() {
        let items = json!([{ "strain_id": "s1", "strain_name": "OG Kush", "quantity": 1 }]);
        assert_eq!(
            first_seedable_item(&items),
            Some(("s1".to_string(), "OG Kush".to_string()))
        );
    }

    #[test]
    fn seedable_accessory_only_order_still_plants() {
        // The exact 036 bug class: an accessory-only order used to seed nothing.
        let items = json!([{ "accessory_id": "a1", "accessory_name": "Grinder" }]);
        assert_eq!(
            first_seedable_item(&items),
            Some(("a1".to_string(), "Grinder".to_string()))
        );
    }

    #[test]
    fn seedable_tea_and_set_only_orders_plant() {
        let tea = json!([{ "tea_id": "t1", "tea_name": "Chamomile" }]);
        assert_eq!(
            first_seedable_item(&tea),
            Some(("t1".to_string(), "Chamomile".to_string()))
        );
        let set = json!([{ "set_id": "set1", "set_name": "Party Pack" }]);
        assert_eq!(
            first_seedable_item(&set),
            Some(("set1".to_string(), "Party Pack".to_string()))
        );
    }

    /// Ordered, de-duplicated `*_id` keys delimited by `q` (e.g. `"` for Rust
    /// string literals, `'` for SQL `->>'..'`), scanning `hay` left to right.
    fn ordered_id_keys(hay: &str, q: char) -> Vec<String> {
        let bytes = hay.as_bytes();
        let mut out: Vec<String> = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] as char == q {
                // read identifier until the closing quote
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && (bytes[j].is_ascii_lowercase() || bytes[j] == b'_') {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] as char == q && j > start {
                    let tok = &hay[start..j];
                    if tok.ends_with("_id") && !out.iter().any(|k| k == tok) {
                        out.push(tok.to_string());
                    }
                    i = j + 1;
                    continue;
                }
            }
            i += 1;
        }
        out
    }

    /// The Rust `first_seedable_item` precedence and the SQL universal-backfill
    /// (migration 037) must agree on **which** catalog id keys seed a plant and
    /// in **what order** — otherwise a new catalog type wired into one but not
    /// the other means live force_seed and the batch backfill disagree. The
    /// 026→036→037 hotfix history is exactly this kind of drift.
    #[test]
    fn seed_keys_match_sql_backfill_037() {
        let manifest = env!("CARGO_MANIFEST_DIR");

        // Rust: scope to the `first_seedable_item` fn body in the pure core
        // `trios::garden` (Wave #66 moved it there as the shared SSOT for the
        // two write paths). Scope ends at the next pure fn.
        let rs =
            std::fs::read_to_string(std::path::Path::new(manifest).join("src/trios/garden.rs"))
                .expect("read trios/garden.rs");
        let fn_start = rs.find("fn first_seedable_item").expect("fn present");
        let fn_end = rs[fn_start..]
            .find("fn filter_active_rewards")
            .map(|o| fn_start + o)
            .unwrap_or(rs.len());
        let rust_keys = ordered_id_keys(&rs[fn_start..fn_end], '"');

        // SQL: `->>'<key>'` JSON extractions in migration 037.
        let sql = std::fs::read_to_string(
            std::path::Path::new(manifest).join("migrations/037_garden_universal_backfill.sql"),
        )
        .expect("read 037");
        let sql_keys = ordered_id_keys(&sql, '\'');

        assert_eq!(
            rust_keys,
            vec!["strain_id", "set_id", "accessory_id", "tea_id"],
            "first_seedable_item key set/order changed — update the SQL backfill + this test"
        );
        assert_eq!(
            rust_keys, sql_keys,
            "garden seed precedence DRIFT: first_seedable_item (Rust) = {rust_keys:?} but \
             migration 037 (SQL) = {sql_keys:?}. A catalog type was wired into one path \
             but not the other."
        );
    }

    #[test]
    fn seedable_prefers_strain_within_an_item() {
        // An item carrying several ids resolves by precedence strain > set >
        // accessory > tea.
        let items = json!([{
            "tea_id": "t1", "tea_name": "Tea",
            "accessory_id": "a1", "accessory_name": "Acc",
            "strain_id": "s1", "strain_name": "Strain"
        }]);
        assert_eq!(
            first_seedable_item(&items),
            Some(("s1".to_string(), "Strain".to_string()))
        );
    }

    #[test]
    fn seedable_skips_items_without_catalog_ids() {
        // Empty-string ids and id-less rows are skipped; first real id wins.
        let items = json!([
            { "strain_id": "", "note": "blank" },
            { "quantity": 2 },
            { "tea_id": "t9", "tea_name": "Mint" }
        ]);
        assert_eq!(
            first_seedable_item(&items),
            Some(("t9".to_string(), "Mint".to_string()))
        );
    }

    #[test]
    fn seedable_none_when_no_catalog_items() {
        assert_eq!(first_seedable_item(&json!([])), None);
        assert_eq!(first_seedable_item(&json!([{ "quantity": 1 }])), None);
        assert_eq!(first_seedable_item(&serde_json::Value::Null), None);
    }

    #[test]
    fn seedable_missing_name_yields_empty_string() {
        let items = json!([{ "strain_id": "s1" }]);
        assert_eq!(
            first_seedable_item(&items),
            Some(("s1".to_string(), String::new()))
        );
    }

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
