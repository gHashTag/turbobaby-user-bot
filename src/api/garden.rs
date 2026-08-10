//! Garden API endpoints

use crate::api::auth::{
    check_admin, check_not_blocked, check_owner_lenient, validate_telegram_id_param,
};
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
        .route("/garden/products", get(get_garden_products))
        .route("/garden/eligibility", get(get_garden_eligibility))
        .route("/garden/eligible", put(set_garden_eligible))
        .route("/garden/plants/choose", post(choose_plant))
        .route("/garden/plants", get(get_user_plants))
        .route("/garden/streak", get(get_garden_streak))
        .route("/garden/plants/:id/water", post(water_plant))
        .route("/garden/plants/:id/harvest", post(harvest_plant))
        .route("/garden/plants/:id/reset", post(reset_plant))
        // Rewards
        .route("/garden/rewards", get(get_user_rewards))
        .route("/garden/rewards/:id/use", post(use_reward))
        // Config
        .route("/garden/config", get(get_config))
        .route("/garden/config", put(update_config))
        .route("/garden/force-seed", post(force_seed))
        // Loop #18: social
        .route("/garden/achievements", get(get_user_achievements))
        .route(
            "/garden/achievements/notified",
            post(mark_achievements_notified),
        )
        .route("/garden/leaderboard", get(get_garden_leaderboard))
        .route("/garden/share-events", post(log_share_event))
}

/// Cycle #10: spawn a background loop that sends garden watering/harvest
/// reminders. Runs on server builds only; not WASM.
#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub(crate) fn spawn_garden_reminder_loop(
    orm: sea_orm::DatabaseConnection,
    bot: std::sync::Arc<teloxide::Bot>,
    config: std::sync::Arc<crate::config::Config>,
    interval_secs: u64,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        interval.tick().await; // discard cold-start tick
        loop {
            interval.tick().await;
            match send_garden_reminders(&orm, &bot, &config).await {
                Ok(0) => {}
                Ok(n) => tracing::info!("garden reminders: sent {} reminder(s)", n),
                Err(e) => tracing::warn!("garden reminders sweep failed: {}", e),
            }
        }
    });
}

/// Send reminders for plants that:
/// - have no reminder_sent_at, OR last reminder was > 24h ago,
/// - are not harvested,
/// - and either can_water now (cooldown expired) or are ready to harvest.
///
/// Updates `garden_plants.reminder_sent_at` to avoid spam.
#[cfg(not(target_arch = "wasm32"))]
async fn send_garden_reminders(
    orm: &sea_orm::DatabaseConnection,
    bot: &teloxide::Bot,
    config: &crate::config::Config,
) -> Result<usize, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let now = chrono::Utc::now().timestamp_millis();
    // 24h window between reminders per plant.
    let reminder_cooldown_ms = 24i64 * 60 * 60 * 1000;
    let min_last_reminder = now.saturating_sub(reminder_cooldown_ms);

    // We need the language per user to localise the message. The DB stores
    // `user_languages.telegram_id` for registered users; unrecognised users get
    // English.
    let rows = orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT p.id, p.user_id, p.is_completed, p.harvested_at, p.last_watered_at, \
                COALESCE(ul.language, 'en') AS lang \
         FROM garden_plants p \
         LEFT JOIN user_languages ul ON ul.telegram_id = p.user_id::bigint \
         WHERE p.harvested_at IS NULL \
           AND p.is_completed = false \
           AND (p.reminder_sent_at IS NULL OR p.reminder_sent_at <= $1) \
           AND (p.last_watered_at IS NULL OR p.last_watered_at <= $2) \
           AND p.water_count < 13 \
         LIMIT 500",
            [
                min_last_reminder.into(),
                (now - crate::trios::garden::WATER_COOLDOWN_MS).into(),
            ],
        ))
        .await?;

    let mut sent = 0usize;
    for r in rows {
        let user_id: String = r.try_get("", "user_id").unwrap_or_default();
        let tid = match user_id.parse::<i64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if tid == 0 {
            continue;
        }
        let plant_id: String = r.try_get("", "id").unwrap_or_default();

        crate::bot::notify::notify_garden_reminder(
            bot,
            &std::sync::Arc::new(crate::db::Database::from_conn(orm.clone())),
            &std::sync::Arc::new(config.clone()),
            tid,
            "water",
        )
        .await;
        crate::metrics::garden_reminder_sent("water");
        sent += 1;

        // Mark reminder sent.
        orm.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE garden_plants SET reminder_sent_at = $1 WHERE id = $2",
            [now.into(), plant_id.into()],
        ))
        .await?;
    }

    // Harvest reminders: plants that are completed but not yet harvested.
    let harvest_rows = orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT p.id, p.user_id \
         FROM garden_plants p \
         WHERE p.is_completed = true \
           AND p.harvested_at IS NULL \
           AND (p.reminder_sent_at IS NULL OR p.reminder_sent_at <= $1) \
         LIMIT 500",
            [min_last_reminder.into()],
        ))
        .await?;

    for r in harvest_rows {
        let user_id: String = r.try_get("", "user_id").unwrap_or_default();
        let tid = match user_id.parse::<i64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if tid == 0 {
            continue;
        }
        let plant_id: String = r.try_get("", "id").unwrap_or_default();

        crate::bot::notify::notify_garden_reminder(
            bot,
            &std::sync::Arc::new(crate::db::Database::from_conn(orm.clone())),
            &std::sync::Arc::new(config.clone()),
            tid,
            "harvest",
        )
        .await;
        crate::metrics::garden_reminder_sent("harvest");
        sent += 1;

        orm.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE garden_plants SET reminder_sent_at = $1 WHERE id = $2",
            [now.into(), plant_id.into()],
        ))
        .await?;
    }

    Ok(sent)
}

/// Loop #17: reminder loop for garden rewards nearing expiration.
#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub(crate) fn spawn_garden_reward_expiry_loop(
    orm: sea_orm::DatabaseConnection,
    bot: std::sync::Arc<teloxide::Bot>,
    config: std::sync::Arc<crate::config::Config>,
    interval_secs: u64,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        interval.tick().await;
        loop {
            interval.tick().await;
            match send_garden_reward_expiry_nudges(&orm, &bot, &config).await {
                Ok(0) => {}
                Ok(n) => tracing::info!("garden reward expiry: sent {} nudge(s)", n),
                Err(e) => tracing::warn!("garden reward expiry sweep failed: {}", e),
            }
        }
    });
}

/// Send expiry nudges for active rewards that expire within 24h or 4h and have
/// not been nudged recently. Marks `expiry_nudge_sent_at` to avoid spam.
#[cfg(not(target_arch = "wasm32"))]
async fn send_garden_reward_expiry_nudges(
    orm: &sea_orm::DatabaseConnection,
    bot: &teloxide::Bot,
    config: &crate::config::Config,
) -> Result<usize, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let now_ms = chrono::Utc::now().timestamp_millis();
    let four_hours_ms = 4i64 * 60 * 60 * 1000;
    let twenty_four_hours_ms = 24i64 * 60 * 60 * 1000;
    let nudge_cooldown_ms = twenty_four_hours_ms;
    let min_last_nudge = now_ms.saturating_sub(nudge_cooldown_ms);

    // Select rewards expiring between 4h and 24h from now that haven't been
    // nudged in the last 24h. We deliberately skip rewards expiring in <4h to
    // avoid spamming users who can't react in time.
    let rows = orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT r.id, r.user_id, r.expires_at \
             FROM garden_rewards r \
             WHERE r.is_used = false \
               AND r.expires_at > $1 \
               AND r.expires_at <= $2 \
               AND (r.expiry_nudge_sent_at IS NULL OR r.expiry_nudge_sent_at <= $3) \
             LIMIT 500",
            [
                (now_ms + four_hours_ms).into(),
                (now_ms + twenty_four_hours_ms).into(),
                min_last_nudge.into(),
            ],
        ))
        .await?;

    let mut sent = 0usize;
    for r in rows {
        let user_id: String = r.try_get("", "user_id").unwrap_or_default();
        let tid = match user_id.parse::<i64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if tid == 0 {
            continue;
        }
        let reward_id: String = r.try_get("", "id").unwrap_or_default();
        let expires_at: i64 = r.try_get("", "expires_at").unwrap_or(now_ms);
        let hours_before = ((expires_at - now_ms) / (60 * 60 * 1000)).max(4);

        crate::bot::notify::notify_garden_reward_expiry(
            bot,
            &std::sync::Arc::new(crate::db::Database::from_conn(orm.clone())),
            &std::sync::Arc::new(config.clone()),
            tid,
        )
        .await;
        crate::metrics::garden_reward_expiry_nudge_sent(hours_before);
        sent += 1;

        orm.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE garden_rewards SET expiry_nudge_sent_at = $1 WHERE id = $2",
            [now_ms.into(), reward_id.into()],
        ))
        .await?;
    }

    Ok(sent)
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
    // B3: the chosen target product (seed = its photo). Null for legacy plants.
    pub target_catalog: Option<String>,
    pub target_product_id: Option<String>,
    pub target_name: Option<String>,
    pub target_image_url: Option<String>,
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
    // B4: product-scoped target (lets checkout match the reward to a cart line).
    pub scope: String,
    pub target_catalog: Option<String>,
    pub target_product_id: Option<String>,
}

// ── Plant Endpoints ───────────────────────────────────────────────

async fn get_user_plants(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<UserPlantsQuery>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(query.telegram_id)?;
    crate::api::auth::check_owner_lenient(&headers, &state, query.telegram_id, "garden")?;
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
                is_completed, harvested_at, reward_claimed, water_count, last_watered_at, \
                target_catalog, target_product_id, target_name, target_image_url \
         FROM garden_plants \
         WHERE user_id = $1 AND harvested_at IS NULL \
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
                target_catalog: r
                    .try_get::<Option<String>>("", "target_catalog")
                    .ok()
                    .flatten(),
                target_product_id: r
                    .try_get::<Option<String>>("", "target_product_id")
                    .ok()
                    .flatten(),
                target_name: r
                    .try_get::<Option<String>>("", "target_name")
                    .ok()
                    .flatten(),
                target_image_url: r
                    .try_get::<Option<String>>("", "target_image_url")
                    .ok()
                    .flatten(),
            }
        })
        .collect();

    Ok(Json(json!({ "plants": plants })))
}

/// Loop #17: compact streak/urgency snapshot for the active plant.
async fn get_garden_streak(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<UserPlantsQuery>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(query.telegram_id)?;
    crate::api::auth::check_owner_lenient(&headers, &state, query.telegram_id, "garden")?;
    check_not_blocked(&state, query.telegram_id).await?;

    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let user_id = query.telegram_id.to_string();
    let now = chrono::Utc::now().timestamp_millis();

    let plant_row = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id, user_id, strain_id, strain_name, current_stage, planted_at, \
                    is_completed, harvested_at, reward_claimed, water_count, last_watered_at, \
                    streak, max_streak \
             FROM garden_plants \
             WHERE user_id = $1 AND harvested_at IS NULL \
             ORDER BY planted_at DESC LIMIT 1",
            [user_id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!(user_id = %user_id, error = %e, "get_garden_streak plant query failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let Some(r) = plant_row else {
        return Ok(Json(json!({
            "has_plant": false,
            "streak": 0,
            "max_streak": 0,
        })));
    };

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
    let streak: i32 = r.try_get("", "streak").unwrap_or(0);
    let max_streak: i32 = r.try_get("", "max_streak").unwrap_or(0);

    // Nearest unused reward expiration drives the FOMO countdown.
    let reward_row = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT expires_at FROM garden_rewards \
             WHERE user_id = $1 AND is_used = false AND expires_at > $2 \
             ORDER BY expires_at ASC LIMIT 1",
            [user_id.clone().into(), now.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!(user_id = %user_id, error = %e, "get_garden_streak reward query failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let reward_expires_at: Option<i64> =
        reward_row.and_then(|r| r.try_get::<Option<i64>>("", "expires_at").ok().flatten());

    Ok(Json(json!({
        "has_plant": true,
        "plant_id": plant.id,
        "strain_name": plant.strain_name,
        "streak": streak,
        "max_streak": max_streak,
        "next_water_at": progress.next_water_at,
        "is_ready_to_harvest": progress.is_ready_to_harvest,
        "reward_expires_at": reward_expires_at,
    })))
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
            "SELECT user_id, current_stage, is_completed, water_count, last_watered_at, \
                    streak, max_streak, streak_last_watered_at \
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
    crate::api::auth::check_owner_lenient(&headers, &state, tid, "garden")?;
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
    let streak: i32 = r.try_get("", "streak").unwrap_or(0);
    let max_streak: i32 = r.try_get("", "max_streak").unwrap_or(0);
    let streak_last_watered_at: Option<i64> = r.try_get("", "streak_last_watered_at").ok();

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
    // Loop #17: streak update. We compute it before the UPDATE so we can
    // persist the new streak atomically with the water action.
    let streak_update = garden::update_streak_on_water(
        streak as u32,
        max_streak as u32,
        streak_last_watered_at.unwrap_or(0),
        now,
    );
    let new_streak = match &streak_update {
        garden::StreakUpdate::Advanced { streak, .. }
        | garden::StreakUpdate::Broken { streak, .. } => *streak,
        garden::StreakUpdate::Unchanged { streak, .. } => *streak,
    };
    let new_max_streak = match &streak_update {
        garden::StreakUpdate::Advanced { max_streak, .. }
        | garden::StreakUpdate::Broken { max_streak, .. }
        | garden::StreakUpdate::Unchanged { max_streak, .. } => *max_streak,
    };

    let cooldown_ms = garden::WATER_COOLDOWN_MS;
    let max_last_water = now.saturating_sub(cooldown_ms);

    let rows = tx
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE garden_plants \
             SET water_count = $1, current_stage = $2, is_completed = $3, last_watered_at = $4, \
                 streak = $5, max_streak = $6, streak_last_watered_at = $7 \
             WHERE id = $8 AND (last_watered_at IS NULL OR last_watered_at <= $9)",
            [
                (new_count as i32).into(),
                new_stage.clone().into(),
                new_completed.into(),
                now.into(),
                (new_streak as i32).into(),
                (new_max_streak as i32).into(),
                now.into(),
                id.clone().into(),
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

    tracing::info!(
        plant_id = %id,
        user_id = %user_id,
        old_water_count = water_count,
        new_water_count = new_count,
        new_stage = %new_stage,
        is_completed = new_completed,
        new_streak = new_streak,
        new_max_streak = new_max_streak,
        "water_plant: watered"
    );

    // Loop #17: streak metrics. Broken streaks and milestones are re-engagement
    // signals; emit them after the tx commits so DB errors don't drop metrics.
    if matches!(streak_update, garden::StreakUpdate::Broken { .. }) {
        crate::metrics::garden_streak_broken();
    }
    crate::metrics::garden_streak_milestone(new_streak as i64);

    // Loop #21: notify the referrer when their friend waters the plant.
    if let Some(referrer_id) = crate::db::referrals::get_referrer_of(&state.db.orm, tid)
        .await
        .map_err(|e| {
            tracing::error!("water_plant get_referrer_of: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    {
        let name = crate::db::users::first_name_for(&state.db.orm, tid)
            .await
            .unwrap_or_else(|_| "Friend".to_string());
        if let Err(e) = crate::db::notifications::enqueue_friend_watered(
            &state.db.orm,
            referrer_id,
            &name,
            new_streak as i64,
        )
        .await
        {
            tracing::warn!("water_plant: failed to enqueue friend_watered: {}", e);
        }
        crate::metrics::garden_invite_funnel("watered");
    }

    // Loop #18: evaluate garden achievements after the water is persisted.
    let new_achievements = match evaluate_and_persist_achievements(
        &state.db.orm,
        tid,
        GardenStats {
            water_count: new_count,
            harvest_count: fetch_garden_user_stats(&state.db.orm, &user_id)
                .await
                .map_err(|e| {
                    tracing::error!("water_plant stats: {e}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?
                .harvest_count,
            max_streak: new_max_streak,
        },
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("water_plant achievement unlock: {e}");
            Vec::new()
        }
    };

    Ok(Json(json!({
        "success": true,
        "water_count": new_count,
        "current_stage": new_stage,
        "is_completed": new_completed,
        "streak": new_streak,
        "max_streak": new_max_streak,
        "new_achievements": new_achievements,
    })))
}

/// Loop #18: convenience wrapper that computes achievement unlocks and persists
/// the delta. Swallowing errors inside the wrapper keeps the primary action
/// (water/harvest) from failing because of an achievement bookkeeping issue.
async fn evaluate_and_persist_achievements(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
    stats: GardenStats,
) -> Result<Vec<String>, sea_orm::DbErr> {
    let ids = garden_achievements_to_unlock(stats);
    persist_achievement_unlocks(orm, telegram_id, &ids).await
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
            "SELECT user_id, strain_id, strain_name, is_completed, harvested_at, \
                    target_catalog, target_product_id \
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
    crate::api::auth::check_owner_lenient(&headers, &state, tid, "garden")?;
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
    // B3/B4: carry the chosen target product onto the reward so checkout can
    // scope the discount to it. Legacy plants (no target) → scope 'cart'.
    let target_catalog: Option<String> = r
        .try_get::<Option<String>>("", "target_catalog")
        .ok()
        .flatten();
    let target_product_id: Option<String> = r
        .try_get::<Option<String>>("", "target_product_id")
        .ok()
        .flatten();
    let reward_scope = if target_product_id.is_some() {
        "product"
    } else {
        "cart"
    };

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
                                    discount_percent, bonus_points, expires_at, is_used, created_at, \
                                    target_catalog, target_product_id, scope) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
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
            target_catalog.into(),
            target_product_id.into(),
            reward_scope.into(),
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
    let notify_user_id = user_id.clone();
    tokio::spawn(async move {
        // Cycle #149: notify_admins now sends with parse_mode=Html, so
        // user-controlled fields must be escaped at the format site.
        // `strain_name` is admin-controlled (catalog), but admins still
        // could enter "Critical <Mass>" — escape to be safe.
        let text = format!(
            "\u{1F33F} Garden reward \u{0432}\u{044B}\u{0434}\u{0430}\u{043D}\n\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\u{1F194} {}\n\u{1F381} {} ({}% / {}pts)",
            notify_user_id, crate::util::html_escape(&strain_name), notify_discount, notify_bonus
        );
        crate::notify::notify_admins(&bot, &config, &text).await;
    });

    crate::metrics::garden_reward_claimed();

    // Loop #18: evaluate garden achievements after harvest is persisted.
    let new_achievements = match fetch_garden_user_stats(&state.db.orm, &user_id).await {
        Ok(stats) => match evaluate_and_persist_achievements(&state.db.orm, tid, stats).await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("harvest_plant achievement unlock: {e}");
                Vec::new()
            }
        },
        Err(e) => {
            tracing::error!("harvest_plant stats: {e}");
            Vec::new()
        }
    };

    Ok(Json(json!({
        "success": true,
        "reward_id": reward_id,
        "discount_percent": discount_percent,
        "bonus_points": bonus_points,
        "expires_at": expires_at,
        "new_achievements": new_achievements,
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
    crate::api::auth::check_owner_lenient(&headers, &state, query.telegram_id, "garden")?;
    check_not_blocked(&state, query.telegram_id).await?;
    // Cycle #94: SeaORM via Statement.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let user_id = query.telegram_id.to_string();
    let now = chrono::Utc::now().timestamp_millis();

    let rows = state.db.orm.query_all(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id, plant_id, strain_name, discount_percent, bonus_points, expires_at, is_used, \
                COALESCE(scope, 'cart') AS scope, target_catalog, target_product_id \
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
                scope: r
                    .try_get::<String>("", "scope")
                    .unwrap_or_else(|_| "cart".into()),
                target_catalog: r
                    .try_get::<Option<String>>("", "target_catalog")
                    .ok()
                    .flatten(),
                target_product_id: r
                    .try_get::<Option<String>>("", "target_product_id")
                    .ok()
                    .flatten(),
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
    crate::api::auth::check_owner_lenient(&headers, &state, tid, "garden")?;
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
        crate::metrics::garden_reward_expired();
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

// ── Loop #18: garden social (achievements, leaderboard, sharing) ──

/// Loop #18: per-user garden achievement unlock logic.
#[derive(Debug, Clone, Copy)]
struct GardenStats {
    water_count: u32,
    harvest_count: u32,
    max_streak: u32,
}

/// Given the current garden stats, return the IDs of all garden
/// achievements whose predicates are satisfied. Kept pure so it is
/// unit-testable without a DB.
fn garden_achievements_to_unlock(stats: GardenStats) -> Vec<&'static str> {
    let GardenStats {
        water_count,
        harvest_count,
        max_streak,
    } = stats;
    let mut ids = Vec::new();
    if water_count >= 1 {
        ids.push("garden_first_water");
    }
    if harvest_count >= 1 {
        ids.push("garden_first_harvest");
    }
    if max_streak >= 3 {
        ids.push("garden_streak_3");
    }
    if max_streak >= 7 {
        ids.push("garden_streak_7");
    }
    if max_streak >= 14 {
        ids.push("garden_streak_14");
    }
    if harvest_count >= 5 {
        ids.push("garden_harvest_5");
    }
    if harvest_count >= 25 {
        ids.push("garden_harvest_25");
    }
    // "Perfect grow" = reached harvest with no missed days. A full grow
    // requires 13 consecutive waters (seed -> final), so max_streak
    // must be at least 13 at the moment of harvest.
    if harvest_count >= 1 && max_streak >= 13 {
        ids.push("garden_zero_miss");
    }
    ids
}

/// Fetch garden aggregate stats for a user from the DB.
async fn fetch_garden_user_stats(
    orm: &sea_orm::DatabaseConnection,
    user_id: &str,
) -> Result<GardenStats, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT \
                    COALESCE(MAX(water_count), 0) AS water_count, \
                    COALESCE(MAX(max_streak), 0) AS max_streak, \
                    COUNT(*) FILTER (WHERE harvested_at IS NOT NULL) AS harvest_count \
             FROM garden_plants \
             WHERE user_id = $1",
            [user_id.into()],
        ))
        .await?;
    let Some(r) = row else {
        return Ok(GardenStats {
            water_count: 0,
            harvest_count: 0,
            max_streak: 0,
        });
    };
    Ok(GardenStats {
        water_count: r.try_get::<i32>("", "water_count").unwrap_or(0).max(0) as u32,
        harvest_count: r.try_get::<i64>("", "harvest_count").unwrap_or(0).max(0) as u32,
        max_streak: r.try_get::<i32>("", "max_streak").unwrap_or(0).max(0) as u32,
    })
}

/// Persist any missing achievement unlocks and return the IDs that were
/// actually inserted (new unlocks). Does not touch already-unlocked rows.
async fn persist_achievement_unlocks(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
    achievement_ids: &[&str],
) -> Result<Vec<String>, sea_orm::DbErr> {
    if achievement_ids.is_empty() {
        return Ok(Vec::new());
    }
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    // Read what is already unlocked so we insert only the delta.
    let existing_rows = orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT achievement_id FROM user_achievements WHERE telegram_id = $1",
            [telegram_id.into()],
        ))
        .await?;
    let existing: std::collections::HashSet<String> = existing_rows
        .iter()
        .filter_map(|r| r.try_get::<String>("", "achievement_id").ok())
        .collect();

    let new_ids: Vec<&str> = achievement_ids
        .iter()
        .copied()
        .filter(|id| !existing.contains(*id))
        .collect();
    if new_ids.is_empty() {
        return Ok(Vec::new());
    }

    let now = chrono::Utc::now().timestamp_millis();
    for id in &new_ids {
        orm.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO user_achievements (telegram_id, achievement_id, unlocked_at, notified) \
             VALUES ($1, $2, to_timestamp($3 / 1000.0), false) \
             ON CONFLICT (telegram_id, achievement_id) DO NOTHING",
            [telegram_id.into(), (*id).into(), now.into()],
        ))
        .await?;
        crate::metrics::garden_achievement_unlocked(id);
    }
    Ok(new_ids.into_iter().map(|s| s.to_string()).collect())
}

#[derive(Debug, Deserialize)]
pub(crate) struct LeaderboardQuery {
    pub telegram_id: i64,
    #[serde(default = "leaderboard_default_kind")]
    pub kind: String,
    #[serde(default = "leaderboard_default_limit")]
    pub limit: u32,
}

fn leaderboard_default_kind() -> String {
    "streak".to_string()
}

fn leaderboard_default_limit() -> u32 {
    10
}

fn anonymize_leaderboard_name(telegram_id: i64) -> String {
    // Do not expose raw telegram_ids to other users; use a stable,
    // anonymous handle derived from the last 4 digits of the id.
    format!("Grower #{:04}", telegram_id.rem_euclid(10000))
}

/// GET /api/garden/leaderboard — top growers by streak or harvest count.
///
/// Returns an anonymised top-N list plus the requesting user's rank/score.
async fn get_garden_leaderboard(
    Query(query): Query<LeaderboardQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(query.telegram_id)?;
    let kind = match query.kind.as_str() {
        "streak" | "harvest" => query.kind.as_str(),
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    let limit = query.limit.clamp(1, 100) as i64;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let (sql, rank_sql) = if kind == "streak" {
        (
            "SELECT user_id::bigint AS telegram_id, MAX(max_streak) AS score \
             FROM garden_plants \
             GROUP BY user_id \
             HAVING MAX(max_streak) > 0 \
             ORDER BY score DESC, MIN(planted_at) ASC \
             LIMIT $1",
            "SELECT COUNT(*) + 1 AS rank, (SELECT MAX(max_streak) FROM garden_plants WHERE user_id = $1) AS score \
             FROM garden_plants p \
             WHERE user_id <> $1 AND max_streak > (SELECT COALESCE(MAX(max_streak),0) FROM garden_plants WHERE user_id = $1)",
        )
    } else {
        (
            "SELECT user_id::bigint AS telegram_id, COUNT(*) AS score \
             FROM garden_plants \
             WHERE harvested_at IS NOT NULL \
             GROUP BY user_id \
             ORDER BY score DESC, MIN(harvested_at) ASC \
             LIMIT $1",
            "SELECT COUNT(*) + 1 AS rank, (SELECT COUNT(*) FROM garden_plants WHERE user_id = $1 AND harvested_at IS NOT NULL) AS score \
             FROM garden_plants p \
             WHERE user_id <> $1 AND harvested_at IS NOT NULL \
               AND (SELECT COUNT(*) FROM garden_plants WHERE user_id = $1 AND harvested_at IS NOT NULL) < \
                   (SELECT COUNT(*) FROM garden_plants WHERE user_id = p.user_id AND harvested_at IS NOT NULL)",
        )
    };

    let rows = state
        .db
        .orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [limit.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("get_garden_leaderboard {}: {e}", kind);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let mut entries = Vec::with_capacity(rows.len());
    for (rank, r) in (1i64..).zip(rows) {
        let tid: i64 = r.try_get("", "telegram_id").unwrap_or(0);
        let score: i64 = r.try_get("", "score").unwrap_or(0);
        entries.push(json!({
            "rank": rank,
            "display_name": anonymize_leaderboard_name(tid),
            "score": score,
            "is_you": tid == query.telegram_id,
        }));
    }

    let user_id = query.telegram_id.to_string();
    let user_rank = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            rank_sql,
            [user_id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("get_garden_leaderboard rank: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let user = user_rank.map(|r| {
        let rank: i64 = r.try_get("", "rank").unwrap_or(0);
        let score: i64 = r.try_get("", "score").unwrap_or(0);
        json!({
            "rank": if rank <= 0 { serde_json::Value::Null } else { rank.into() },
            "score": score,
            "display_name": anonymize_leaderboard_name(query.telegram_id),
        })
    });

    crate::metrics::garden_leaderboard_viewed(kind);

    Ok(Json(json!({
        "kind": kind,
        "entries": entries,
        "user": user,
    })))
}

#[derive(Debug, Deserialize)]
pub(crate) struct LogShareEventRequest {
    pub telegram_id: i64,
    pub channel: String,
    pub content_kind: String,
    pub content_id: String,
}

/// POST /api/garden/share-events — log a share for viral attribution.
///
/// The endpoint is authenticated (owner) but intentionally light; the
/// share itself happens via Telegram's native picker on the client.
async fn log_share_event(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<LogShareEventRequest>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(req.telegram_id)?;
    check_owner_lenient(&headers, &state, req.telegram_id, "garden")?;

    fn safe_label(s: &str, max: usize) -> bool {
        !s.is_empty()
            && s.len() <= max
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    }
    if !safe_label(&req.channel, 20)
        || !safe_label(&req.content_kind, 30)
        || req.content_id.is_empty()
        || req.content_id.len() > 200
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let content_kind = req.content_kind.clone();
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO share_events (id, telegram_id, channel, content_kind, content_id, shared_at) \
             VALUES ($1, $2, $3, $4, $5, to_timestamp($6 / 1000.0))",
            [
                id.into(),
                req.telegram_id.into(),
                req.channel.into(),
                req.content_kind.into(),
                req.content_id.clone().into(),
                now.into(),
            ],
        ))
        .await
        .map_err(|e| {
            tracing::error!("log_share_event: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    crate::metrics::share_event_logged(&content_kind);

    Ok(Json(json!({ "success": true })))
}

#[derive(Debug, Deserialize)]
pub(crate) struct AchievementsQuery {
    pub telegram_id: i64,
}

/// GET /api/garden/achievements — all garden achievements with unlock state.
async fn get_user_achievements(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<AchievementsQuery>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(query.telegram_id)?;
    check_owner_lenient(&headers, &state, query.telegram_id, "garden")?;

    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let rows = state
        .db
        .orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT \
                    a.id, a.name, a.description, a.icon, a.xp_reward, a.requirement, a.category, \
                    ua.unlocked_at, ua.notified \
             FROM achievements a \
             LEFT JOIN user_achievements ua ON ua.achievement_id = a.id AND ua.telegram_id = $1 \
             WHERE a.category = 'garden' \
             ORDER BY a.xp_reward ASC, a.id ASC",
            [query.telegram_id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("get_user_achievements: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let mut achievements = Vec::with_capacity(rows.len());
    let mut unnotified = Vec::new();
    for r in rows {
        let id: String = r.try_get("", "id").unwrap_or_default();
        let unlocked_at: Option<i64> = r
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "unlocked_at")
            .ok()
            .flatten()
            .map(|dt| dt.timestamp_millis());
        let notified: bool = r.try_get("", "notified").unwrap_or(true);
        if unlocked_at.is_some() && !notified {
            unnotified.push(id.clone());
        }
        achievements.push(json!({
            "id": id,
            "name": r.try_get::<String>("", "name").unwrap_or_default(),
            "description": r.try_get::<String>("", "description").unwrap_or_default(),
            "icon": r.try_get::<String>("", "icon").unwrap_or_default(),
            "xp_reward": r.try_get::<i32>("", "xp_reward").unwrap_or(0),
            "requirement": r.try_get::<String>("", "requirement").unwrap_or_default(),
            "category": r.try_get::<String>("", "category").unwrap_or_default(),
            "unlocked_at": unlocked_at,
            "notified": notified,
        }));
    }

    Ok(Json(json!({
        "achievements": achievements,
        "unnotified": unnotified,
    })))
}

/// POST /api/garden/achievements/notified — mark all garden achievement
/// unlocks as seen by the user.
async fn mark_achievements_notified(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<AchievementsQuery>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(req.telegram_id)?;
    check_owner_lenient(&headers, &state, req.telegram_id, "garden")?;

    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE user_achievements SET notified = true WHERE telegram_id = $1",
            [req.telegram_id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("mark_achievements_notified: {e}");
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
    crate::api::auth::check_owner_lenient(&headers, &state, req.telegram_id, "garden")?;
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
            "SELECT 1 FROM garden_plants WHERE user_id = $1 AND harvested_at IS NULL LIMIT 1",
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

    // B1: only seed a product that still exists AND is available in the live
    // catalog — the order JSON is a stale snapshot, so a since-deleted/hidden
    // strain would otherwise be planted as a "phantom" not in the menu.
    let live = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT EXISTS ( \
                 SELECT 1 FROM strains        WHERE id = $1 AND is_available = true \
                 UNION ALL SELECT 1 FROM sets           WHERE id = $1 AND is_available = true \
                 UNION ALL SELECT 1 FROM accessory_sets WHERE id = $1 AND is_available = true \
                 UNION ALL SELECT 1 FROM tea_sets       WHERE id = $1 AND is_available = true \
                 UNION ALL SELECT 1 FROM accessories    WHERE id = $1 AND is_available = true \
                 UNION ALL SELECT 1 FROM tea_products   WHERE id = $1 AND is_available = true \
             ) AS ok",
            [strain_id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("force_seed: live-check failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let is_live = live
        .and_then(|r| r.try_get::<bool>("", "ok").ok())
        .unwrap_or(false);
    if !is_live {
        tracing::info!(
            telegram_id = tid,
            "force_seed: skipped — seed product not in live catalog"
        );
        return Ok(Json(
            json!({ "success": false, "error": "product_not_in_menu" }),
        ));
    }

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

// ── B3: choose-any-product flow ───────────────────────────────────

/// Map a customer-facing catalog key to its table name. Static allow-list
/// (no user value reaches SQL as an identifier) — unknown → None.
fn garden_catalog_table(catalog: &str) -> Option<&'static str> {
    match catalog {
        "strain" => Some("strains"),
        "accessory" => Some("accessories"),
        "tea" => Some("tea_products"),
        "set" => Some("sets"),
        "accessory_set" => Some("accessory_sets"),
        "tea_set" => Some("tea_sets"),
        _ => None,
    }
}

/// GET /api/garden/products — every live, garden-eligible product the customer
/// can choose to grow a discount for, across all catalogs. Public (catalog data,
/// no user data), mirrors `/api/sets` resilience: degrade each source to empty.
async fn get_garden_products(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let sql = "\
        SELECT 'strain'        AS catalog, id, name, image_url, price_per_gram::float8 AS price FROM strains        WHERE is_available AND garden_eligible \
        UNION ALL SELECT 'accessory',     id, name, image_url, price::float8        FROM accessories    WHERE is_available AND garden_eligible \
        UNION ALL SELECT 'tea',           id, name, image_url, price::float8        FROM tea_products   WHERE is_available AND garden_eligible \
        UNION ALL SELECT 'set',           id, name, image_url, total_price::float8  FROM sets           WHERE is_available AND garden_eligible \
        UNION ALL SELECT 'accessory_set', id, name, image_url, total_price::float8  FROM accessory_sets WHERE is_available AND garden_eligible \
        UNION ALL SELECT 'tea_set',       id, name, image_url, total_price::float8  FROM tea_sets       WHERE is_available AND garden_eligible \
        ORDER BY catalog, name LIMIT 3000";
    let rows = state
        .db
        .orm
        .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await
        .map_err(|e| {
            tracing::error!("get_garden_products: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let products: Vec<Value> = rows
        .iter()
        .map(|r| {
            let price: f64 = crate::try_get_warn!(r, "price", 0.0);
            json!({
                "catalog": r.try_get::<String>("", "catalog").unwrap_or_default(),
                "id": r.try_get::<String>("", "id").unwrap_or_default(),
                "name": r.try_get::<String>("", "name").unwrap_or_default(),
                "image_url": r.try_get::<Option<String>>("", "image_url").ok().flatten(),
                "price": if price.is_finite() { price.max(0.0) } else { 0.0 },
            })
        })
        .collect();
    Ok(Json(json!({ "products": products })))
}

// ── B5: admin garden eligibility (opt products in/out of the chooser) ──

/// GET /api/garden/eligibility (admin) — every available product across all
/// catalogs with its current `garden_eligible` flag, so admins can toggle which
/// products customers may grow a discount for.
async fn get_garden_eligibility(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let sql = "\
        SELECT 'strain'        AS catalog, id, name, garden_eligible FROM strains        WHERE is_available \
        UNION ALL SELECT 'accessory',     id, name, garden_eligible FROM accessories    WHERE is_available \
        UNION ALL SELECT 'tea',           id, name, garden_eligible FROM tea_products   WHERE is_available \
        UNION ALL SELECT 'set',           id, name, garden_eligible FROM sets           WHERE is_available \
        UNION ALL SELECT 'accessory_set', id, name, garden_eligible FROM accessory_sets WHERE is_available \
        UNION ALL SELECT 'tea_set',       id, name, garden_eligible FROM tea_sets       WHERE is_available \
        ORDER BY catalog, name LIMIT 5000";
    let rows = state
        .db
        .orm
        .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await
        .map_err(|e| {
            tracing::error!("get_garden_eligibility: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let products: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "catalog": r.try_get::<String>("", "catalog").unwrap_or_default(),
                "id": r.try_get::<String>("", "id").unwrap_or_default(),
                "name": r.try_get::<String>("", "name").unwrap_or_default(),
                "garden_eligible": r.try_get::<bool>("", "garden_eligible").unwrap_or(true),
            })
        })
        .collect();
    Ok(Json(json!({ "products": products })))
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetEligibleRequest {
    pub catalog: String,
    pub product_id: String,
    pub eligible: bool,
}

/// PUT /api/garden/eligible (admin) — flip a product's garden eligibility.
async fn set_garden_eligible(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<SetEligibleRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    if req.product_id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let Some(table) = garden_catalog_table(&req.catalog) else {
        return Err(StatusCode::BAD_REQUEST);
    };
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            // `table` is from the static allow-list — safe to interpolate.
            format!("UPDATE {table} SET garden_eligible = $1 WHERE id = $2"),
            [req.eligible.into(), req.product_id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("set_garden_eligible: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "success": true })))
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChoosePlantRequest {
    pub telegram_id: i64,
    pub catalog: String,
    pub product_id: String,
}

/// POST /api/garden/plants/choose — the customer picks a live product to grow a
/// discount for. Snapshots the product's name + photo onto the plant's target_*
/// (the photo becomes the seed image). If an un-harvested plant already exists,
/// only its target product/strain fields are updated — watering progress is
/// preserved. The 24h post-harvest cooldown still blocks a new plant after a
/// recent harvest.
async fn choose_plant(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<ChoosePlantRequest>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(req.telegram_id)?;
    crate::api::auth::check_owner_lenient(&headers, &state, req.telegram_id, "garden")?;
    check_not_blocked(&state, req.telegram_id).await?;
    if req.product_id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let Some(table) = garden_catalog_table(&req.catalog) else {
        return Err(StatusCode::BAD_REQUEST);
    };
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    // Validate the product is live + garden-eligible, and snapshot name/photo.
    let row = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            // `table` is from the static allow-list above — safe to interpolate.
            format!(
                "SELECT name, image_url FROM {table} WHERE id = $1 AND is_available AND garden_eligible"
            ),
            [req.product_id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("choose_plant: lookup failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let Some(row) = row else {
        return Ok(Json(
            json!({ "success": false, "error": "product_not_available" }),
        ));
    };
    let name: String = row.try_get("", "name").unwrap_or_default();
    let image_url: Option<String> = row
        .try_get::<Option<String>>("", "image_url")
        .ok()
        .flatten();

    let user_id = req.telegram_id.to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let harvest_cooldown_floor = now.saturating_sub(garden::POST_HARVEST_COOLDOWN_MS);

    use sea_orm::TransactionTrait;
    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("choose_plant tx.begin: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let in_cooldown: bool = tx
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT EXISTS(SELECT 1 FROM garden_plants \
                 WHERE user_id = $1 AND harvested_at IS NOT NULL AND harvested_at > $2) AS c",
            [user_id.clone().into(), harvest_cooldown_floor.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("choose_plant cooldown check: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .and_then(|r| r.try_get::<bool>("", "c").ok())
        .unwrap_or(false);
    if in_cooldown {
        return Ok(Json(
            json!({ "success": false, "error": "harvest_cooldown" }),
        ));
    }

    // Try to update an existing un-harvested plant first: change WHAT the user is
    // growing without resetting water_count / current_stage / last_watered_at.
    let updated = tx
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE garden_plants \
             SET strain_id = $2, strain_name = $3, \
                 target_catalog = $4, target_product_id = $2, target_name = $3, target_image_url = $5, \
                 updated_at = $6 \
             WHERE user_id = $1 AND harvested_at IS NULL",
            [
                user_id.clone().into(),
                req.product_id.clone().into(),
                name.clone().into(),
                req.catalog.clone().into(),
                image_url.clone().into(),
                now.into(),
            ],
        ))
        .await
        .map_err(|e| {
            tracing::error!("choose_plant: update existing: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let plant_id = if updated.rows_affected() > 0 {
        // Read back the existing plant id for the response + log.
        let id_row = tx
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT id FROM garden_plants WHERE user_id = $1 AND harvested_at IS NULL",
                [user_id.clone().into()],
            ))
            .await
            .map_err(|e| {
                tracing::error!("choose_plant: read back id: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        id_row
            .and_then(|r| r.try_get::<String>("", "id").ok())
            .unwrap_or_else(|| "existing".to_string())
    } else {
        // No un-harvested plant exists — plant a fresh seed.
        let plant_id = uuid::Uuid::new_v4().to_string();
        let planted_at = now;
        tx.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO garden_plants \
                  (id, user_id, strain_id, strain_name, current_stage, planted_at, is_completed, water_count, \
                   target_catalog, target_product_id, target_name, target_image_url) \
             VALUES ($1, $2, $3, $4, 'seed', $5, false, 0, $6, $3, $4, $7)",
            [
                plant_id.clone().into(),
                user_id.into(),
                req.product_id.clone().into(),
                name.clone().into(),
                planted_at.into(),
                req.catalog.clone().into(),
                image_url.into(),
            ],
        ))
        .await
        .map_err(|e| {
            tracing::error!("choose_plant: insert failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        plant_id
    };

    tx.commit().await.map_err(|e| {
        tracing::error!("choose_plant commit: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::info!(
        telegram_id = req.telegram_id,
        plant_id = %plant_id,
        was_existing = updated.rows_affected() > 0,
        "choose_plant: chose {} {}",
        req.catalog,
        req.product_id
    );
    Ok(Json(json!({ "success": true, "plant_id": plant_id })))
}

/// POST /api/garden/plants/:id/reset — explicit "start from scratch". Resets
/// the current un-harvested plant to a seed (water_count=0, stage=Seed) while
/// keeping the same chosen target product. This is the ONLY supported way to
/// intentionally reset progress; choose_plant no longer does it.
async fn reset_plant(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};

    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("reset_plant tx.begin: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let row = tx
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT user_id, water_count, current_stage, is_completed, last_watered_at \
             FROM garden_plants WHERE id = $1 FOR UPDATE",
            [id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("reset_plant FOR UPDATE: {e}");
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
    crate::api::auth::check_owner_lenient(&headers, &state, tid, "garden")?;
    check_not_blocked(&state, tid).await?;

    let is_completed: bool = r.try_get("", "is_completed").unwrap_or(false);
    let harvested_at: Option<i64> = r.try_get("", "harvested_at").ok();
    let old_water_count: i32 = r.try_get("", "water_count").unwrap_or(0);
    let old_stage: String = r
        .try_get::<String>("", "current_stage")
        .unwrap_or_else(|_| "seed".into());

    if is_completed || harvested_at.is_some() {
        return Ok(Json(
            json!({ "success": false, "error": "Cannot reset a harvested plant" }),
        ));
    }

    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE garden_plants \
         SET water_count = 0, current_stage = 'seed', is_completed = false, last_watered_at = NULL, updated_at = $2 \
         WHERE id = $1 AND harvested_at IS NULL",
        [id.clone().into(), chrono::Utc::now().timestamp_millis().into()],
    ))
    .await
    .map_err(|e| {
        tracing::error!("reset_plant update: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await.map_err(|e| {
        tracing::error!("reset_plant commit: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::info!(
        plant_id = %id,
        user_id = %user_id,
        old_water_count = old_water_count,
        old_stage = %old_stage,
        "reset_plant: reset to seed"
    );

    Ok(Json(json!({ "success": true })))
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

    // Loop #18: achievement predicate tests.
    use super::GardenStats;

    #[test]
    fn achievement_first_water_requires_one_water() {
        assert!(super::garden_achievements_to_unlock(GardenStats {
            water_count: 1,
            harvest_count: 0,
            max_streak: 0,
        })
        .contains(&"garden_first_water"));
        assert!(!super::garden_achievements_to_unlock(GardenStats {
            water_count: 0,
            harvest_count: 0,
            max_streak: 0,
        })
        .contains(&"garden_first_water"));
    }

    #[test]
    fn achievement_streak_milestones_are_tiered() {
        let ids = super::garden_achievements_to_unlock(GardenStats {
            water_count: 5,
            harvest_count: 1,
            max_streak: 14,
        });
        assert!(ids.contains(&"garden_streak_3"));
        assert!(ids.contains(&"garden_streak_7"));
        assert!(ids.contains(&"garden_streak_14"));
        assert!(ids.contains(&"garden_zero_miss"));
    }

    #[test]
    fn achievement_harvest_milestones_stack() {
        let ids = super::garden_achievements_to_unlock(GardenStats {
            water_count: 10,
            harvest_count: 25,
            max_streak: 10,
        });
        assert!(ids.contains(&"garden_first_harvest"));
        assert!(ids.contains(&"garden_harvest_5"));
        assert!(ids.contains(&"garden_harvest_25"));
    }

    #[test]
    fn achievement_perfect_grow_needs_harvest_and_streak_13() {
        let without_harvest = super::garden_achievements_to_unlock(GardenStats {
            water_count: 13,
            harvest_count: 0,
            max_streak: 13,
        });
        assert!(!without_harvest.contains(&"garden_zero_miss"));
        let with_harvest = super::garden_achievements_to_unlock(GardenStats {
            water_count: 13,
            harvest_count: 1,
            max_streak: 13,
        });
        assert!(with_harvest.contains(&"garden_zero_miss"));
    }

    #[test]
    fn leaderboard_name_anonymizes() {
        assert_eq!(super::anonymize_leaderboard_name(123456789), "Grower #6789");
        assert_eq!(super::anonymize_leaderboard_name(42), "Grower #0042");
        assert_eq!(super::anonymize_leaderboard_name(-1), "Grower #9999");
    }
}
