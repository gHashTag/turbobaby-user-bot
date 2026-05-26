//! Garden API endpoints

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::api::auth::check_admin;
use crate::AppState;
use crate::trios::garden;

pub fn routes() -> Router<AppState> {
    Router::new()
        // User plants
        .route("/garden/plants", get(get_user_plants))
        .route("/garden/plants", post(plant_seed))
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
pub struct UserPlantsQuery {
    pub telegram_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct PlantSeedRequest {
    pub telegram_id: i64,
    pub strain_id: String,
    pub strain_name: String,
}

#[derive(Debug, Serialize)]
pub struct PlantResponse {
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
    pub water_count: u32,
    pub progress: u8,
    pub can_water: bool,
    pub next_water_at: i64,
}

#[derive(Debug, Serialize)]
pub struct RewardResponse {
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
    crate::api::auth::check_owner(&headers, &state, query.telegram_id)?;
    let client = state.db.pool.get().await
        .map_err(|e| {
            tracing::error!("Database connection error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let user_id = query.telegram_id.to_string();

    let rows = client.query(
        "SELECT id, user_id, strain_id, strain_name, current_stage, planted_at,
                is_completed, harvested_at, reward_claimed, water_count, last_watered_at
         FROM garden_plants
         WHERE user_id = $1
         ORDER BY planted_at DESC",
        &[&user_id],
    ).await.map_err(|e| {
        tracing::error!("Query error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let now = chrono::Utc::now().timestamp_millis();
    let plants: Vec<PlantResponse> = rows.iter().map(|r| {
        let plant = garden::Plant {
            id: r.get(0),
            user_id: r.get(1),
            strain_id: r.get(2),
            strain_name: r.get(3),
            current_stage: match r.get::<_, String>(4).as_str() {
                "seed" => garden::GrowthStage::Seed,
                "sprout" => garden::GrowthStage::Sprout,
                "first_leaf" => garden::GrowthStage::FirstLeaf,
                "young_bush" => garden::GrowthStage::YoungBush,
                "veg_start" => garden::GrowthStage::VegStart,
                "big_veg" => garden::GrowthStage::BigVeg,
                "pre_flower" => garden::GrowthStage::PreFlower,
                "small_buds" => garden::GrowthStage::SmallBuds,
                "big_buds" => garden::GrowthStage::BigBuds,
                "trimming" => garden::GrowthStage::Trimming,
                "curing" => garden::GrowthStage::Curing,
                "lab" => garden::GrowthStage::Lab,
                "delivery" => garden::GrowthStage::Delivery,
                _ => garden::GrowthStage::Final,
            },
            planted_at: r.get(5),
            is_completed: r.get(6),
            harvested_at: r.get(7),
            reward_claimed: r.get(8),
            water_count: r.get(9),
            last_watered_at: r.get(10),
        };

        let progress = garden::calculate_progress(&plant, now);
        PlantResponse {
            id: plant.id,
            user_id: plant.user_id,
            strain_id: plant.strain_id,
            strain_name: plant.strain_name,
            current_stage: format!("{:?}", plant.current_stage).to_lowercase(),
            stage_name: progress.stage_name,
            stage_emoji: progress.stage_emoji,
            planted_at: plant.planted_at,
            is_completed: plant.is_completed,
            harvested_at: plant.harvested_at,
            water_count: plant.water_count,
            progress: progress.total_progress,
            can_water: progress.can_water,
            next_water_at: progress.next_water_at,
        }
    }).collect();

    Ok(Json(json!({ "plants": plants })))
}

async fn plant_seed(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<PlantSeedRequest>,
) -> Result<Json<Value>, StatusCode> {
    crate::api::auth::check_owner(&headers, &state, req.telegram_id)?;
    let client = state.db.pool.get().await
        .map_err(|e| {
            tracing::error!("Database connection error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let user_id = req.telegram_id.to_string();

    let plant = garden::Plant::new(user_id.clone(), req.strain_id, req.strain_name);
    let plant_id = plant.id.clone();

    // Atomic insert: only succeeds if the user has no active plant.
    let inserted = client.execute(
        "INSERT INTO garden_plants (id, user_id, strain_id, strain_name, current_stage,
                                    planted_at, is_completed, water_count, last_watered_at)
         SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9
         WHERE NOT EXISTS (
             SELECT 1 FROM garden_plants WHERE user_id = $2 AND is_completed = false
         )",
        &[
            &plant.id,
            &plant.user_id,
            &plant.strain_id,
            &plant.strain_name,
            &format!("{:?}", plant.current_stage).to_lowercase(),
            &plant.planted_at,
            &plant.is_completed,
            &plant.water_count,
            &plant.last_watered_at,
        ],
    ).await.map_err(|e| {
        tracing::error!("Insert error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if inserted == 0 {
        return Ok(Json(json!({
            "success": false,
            "error": "You already have an active plant"
        })));
    }

    Ok(Json(json!({
        "success": true,
        "plant_id": plant_id
    })))
}

async fn water_plant(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await
        .map_err(|e| {
            tracing::error!("Database connection error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let now = chrono::Utc::now().timestamp_millis();

    // Get current plant
    let row = client.query_opt(
        "SELECT user_id, current_stage, is_completed, water_count, last_watered_at
         FROM garden_plants
         WHERE id = $1",
        &[&id],
    ).await.map_err(|e| {
        tracing::error!("Query error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let Some(r) = row else {
        return Ok(Json(json!({ "success": false, "error": "Plant not found" })));
    };

    let user_id: String = r.get(0);
    if let Ok(tid) = user_id.parse::<i64>() {
        crate::api::auth::check_owner(&headers, &state, tid)?;
    }

    let current_stage: String = r.get(1);
    let is_completed: bool = r.get(2);
    let water_count: i32 = r.get(3);
    let last_watered_at: Option<i64> = r.get(4);

    if is_completed {
        return Ok(Json(json!({ "success": false, "error": "Plant already completed" })));
    }

    // Atomic update with cooldown guard in WHERE clause to prevent race conditions
    let new_count = (water_count + 1) as u32;
    let new_stage = if let Some(stage) = garden::GrowthStage::from_index(new_count as usize) {
        format!("{:?}", stage).to_lowercase()
    } else {
        current_stage
    };
    let new_completed = new_count >= 13;
    let cooldown_ms = garden::WATER_COOLDOWN_MS as i64;
    let max_last_water = now - cooldown_ms;

    let rows = client.execute(
        "UPDATE garden_plants
         SET water_count = $1, current_stage = $2, is_completed = $3, last_watered_at = $4
         WHERE id = $5 AND (last_watered_at IS NULL OR last_watered_at <= $6)",
        &[&(new_count as i32), &new_stage, &new_completed, &now, &id, &max_last_water],
    ).await.map_err(|e| {
        tracing::error!("Update error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if rows == 0 {
        let next_water_at = last_watered_at.map(|t| t + garden::WATER_COOLDOWN_MS).unwrap_or(now);
        return Ok(Json(json!({
            "success": false,
            "error": "Cooldown active",
            "next_water_at": next_water_at
        })));
    }

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
    let mut client = state.db.pool.get().await
        .map_err(|e| {
            tracing::error!("Database connection error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Get plant
    let row = client.query_opt(
        "SELECT user_id, strain_id, strain_name, is_completed, harvested_at
         FROM garden_plants
         WHERE id = $1",
        &[&id],
    ).await.map_err(|e| {
        tracing::error!("Query error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let Some(r) = row else {
        return Ok(Json(json!({ "success": false, "error": "Plant not found" })));
    };

    let user_id: String = r.get(0);
    if let Ok(tid) = user_id.parse::<i64>() {
        crate::api::auth::check_owner(&headers, &state, tid)?;
    }

    let is_completed: bool = r.get(3);
    let harvested_at: Option<i64> = r.get(4);

    if !is_completed {
        return Ok(Json(json!({ "success": false, "error": "Plant not ready for harvest" })));
    }

    if harvested_at.is_some() {
        return Ok(Json(json!({ "success": false, "error": "Already harvested" })));
    }

    let now = chrono::Utc::now().timestamp_millis();
    let user_id: String = r.get(0);
    let strain_id: String = r.get(1);
    let strain_name: String = r.get(2);

    // Create reward
    let reward_id = uuid::Uuid::new_v4().to_string();

    // Start transaction
    let tx = client.transaction().await
        .map_err(|e| {
            tracing::error!("Transaction error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Read garden config inside transaction for consistency
    let config_row = tx.query_opt(
        "SELECT reward_discount_percent, reward_bonus_points, reward_expiration_days FROM garden_config WHERE id = 1",
        &[],
    ).await.map_err(|e| {
        tracing::error!("Config read error: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    })?;
    let (discount_percent, bonus_points, expiration_days) = match config_row {
        Some(r) => (r.get::<_, i32>(0), r.get::<_, i32>(1), r.get::<_, i32>(2)),
        None => (10, 100, 7),
    };
    let expires_at = now + (expiration_days as i64 * 24 * 60 * 60 * 1000);

    // Atomically mark plant as harvested — WHERE harvested_at IS NULL prevents race
    let rows = tx.execute(
        "UPDATE garden_plants SET harvested_at = $1, reward_claimed = true WHERE id = $2 AND harvested_at IS NULL",
        &[&now, &id],
    ).await.map_err(|e| {
        tracing::error!("Update plant error: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    })?;

    if rows == 0 {
        let _ = tx.rollback().await;
        return Ok(Json(json!({ "success": false, "error": "Already harvested" })));
    }

    // Create reward
    tx.execute(
        "INSERT INTO garden_rewards (id, plant_id, user_id, strain_id, strain_name,
                                    discount_percent, bonus_points, expires_at, is_used, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        &[
            &reward_id,
            &id,
            &user_id,
            &strain_id,
            &strain_name,
            &discount_percent,
            &bonus_points,
            &expires_at,
            &false,
            &now,
        ],
    ).await.map_err(|e| {
        tracing::error!("Insert reward error: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    })?;

    tx.commit().await.map_err(|e| {
        tracing::error!("Commit error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Notify admins about garden reward
    let bot = state.bot.clone();
    let config = state.config.clone();
    let notify_user_id = user_id.clone();
    let notify_strain = strain_name.clone();
    let notify_discount = discount_percent;
    let notify_bonus = bonus_points;
    tokio::spawn(async move {
        let text = format!(
            "\u{1F33F} Garden reward \u{0432}\u{044B}\u{0434}\u{0430}\u{043D}\n\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\u{1F194} {}\n\u{1F381} {} ({}% / {}pts)",
            notify_user_id, notify_strain, notify_discount, notify_bonus
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
pub struct UserRewardsQuery {
    pub telegram_id: i64,
}

async fn get_user_rewards(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<UserRewardsQuery>,
) -> Result<Json<Value>, StatusCode> {
    crate::api::auth::check_owner(&headers, &state, query.telegram_id)?;
    let client = state.db.pool.get().await
        .map_err(|e| {
            tracing::error!("Database connection error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let user_id = query.telegram_id.to_string();
    let now = chrono::Utc::now().timestamp_millis();

    let rows = client.query(
        "SELECT id, plant_id, strain_name, discount_percent, bonus_points, expires_at, is_used
         FROM garden_rewards
         WHERE user_id = $1
         ORDER BY created_at DESC",
        &[&user_id],
    ).await.map_err(|e| {
        tracing::error!("Query error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let rewards: Vec<RewardResponse> = rows.iter().map(|r| {
        let expires_at: i64 = r.get(5);
        let is_used: bool = r.get(6);
        RewardResponse {
            id: r.get(0),
            plant_id: r.get(1),
            strain_name: r.get(2),
            discount_percent: r.get::<_, i32>(3) as u32,
            bonus_points: r.get::<_, i32>(4) as u32,
            expires_at,
            is_used,
            is_active: !is_used && expires_at > now,
        }
    }).collect();

    Ok(Json(json!({ "rewards": rewards })))
}

async fn use_reward(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let mut client = state.db.pool.get().await
        .map_err(|e| {
            tracing::error!("Database connection error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let now = chrono::Utc::now().timestamp_millis();

    // Check reward (read-only, can stay outside tx for auth)
    let row = client.query_opt(
        "SELECT user_id, is_used, expires_at, discount_percent, bonus_points
         FROM garden_rewards
         WHERE id = $1",
        &[&id],
    ).await.map_err(|e| {
        tracing::error!("Query error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let Some(r) = row else {
        return Ok(Json(json!({ "success": false, "error": "Reward not found" })));
    };

    let user_id: String = r.get(0);
    if let Ok(tid) = user_id.parse::<i64>() {
        crate::api::auth::check_owner(&headers, &state, tid)?;
    }

    let is_used: bool = r.get(1);
    let expires_at: i64 = r.get(2);

    if is_used {
        return Ok(Json(json!({ "success": false, "error": "Reward already used" })));
    }

    if expires_at < now {
        return Ok(Json(json!({ "success": false, "error": "Reward expired" })));
    }

    let discount_percent: i32 = r.get(3);
    let bonus_points: i32 = r.get(4);

    // Atomically mark used and credit bonus inside a transaction
    let tx = client.transaction().await.map_err(|e| {
        tracing::error!("Transaction error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let rows = tx.execute(
        "UPDATE garden_rewards SET is_used = true WHERE id = $1 AND is_used = false",
        &[&id],
    ).await.map_err(|e| {
        tracing::error!("Mark reward used error: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    })?;

    if rows == 0 {
        let _ = tx.rollback().await;
        return Ok(Json(json!({ "success": false, "error": "Reward already used" })));
    }

    if let Ok(tid) = user_id.parse::<i64>() {
        let bonus_f64 = bonus_points as f64;
        tx.execute(
            "UPDATE loyalty_profiles SET bonus_balance = bonus_balance + $1 WHERE telegram_id = $2",
            &[&bonus_f64, &tid],
        ).await.map_err(|e| {
            tracing::error!("Credit bonus error: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        })?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("Commit error: {}", e);
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
pub struct ConfigUpdateRequest {
    pub is_enabled: Option<bool>,
    pub reward_discount_percent: Option<u32>,
    pub reward_bonus_points: Option<u32>,
    pub reward_expiration_days: Option<u32>,
}

async fn get_config(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await
        .map_err(|e| {
            tracing::error!("Database connection error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let row = client.query_opt(
        "SELECT is_enabled, reward_discount_percent, reward_bonus_points, reward_expiration_days
         FROM garden_config
         LIMIT 1",
        &[],
    ).await.map_err(|e| {
        tracing::error!("Query error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let config = match row {
        Some(r) => {
            garden::GameConfig {
                is_enabled: r.get(0),
                reward_discount_percent: r.get::<_, i32>(1) as u32,
                reward_bonus_points: r.get::<_, i32>(2) as u32,
                reward_expiration_days: r.get::<_, i32>(3) as u32,
            }
        }
        None => garden::GameConfig::default(),
    };

    Ok(Json(json!({
        "is_enabled": config.is_enabled,
        "reward_discount_percent": config.reward_discount_percent as f64,
        "reward_bonus_points": config.reward_bonus_points as f64,
        "reward_expiration_days": config.reward_expiration_days as f64,
    })))
}

async fn update_config(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<ConfigUpdateRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await
        .map_err(|e| {
            tracing::error!("Database connection error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // BUG-5: is_enabled в БД — BOOLEAN, раньше передавался i32 → type mismatch.
    if let Some(p) = req.reward_discount_percent {
        if p > 100 { return Err(StatusCode::BAD_REQUEST); }
    }
    if let Some(p) = req.reward_bonus_points {
        if p > 1_000_000 { return Err(StatusCode::BAD_REQUEST); }
    }
    if let Some(d) = req.reward_expiration_days {
        if d == 0 || d > 365 { return Err(StatusCode::BAD_REQUEST); }
    }
    let is_enabled = req.is_enabled;
    let reward_discount_percent = req.reward_discount_percent.map(|p| p as i32);
    let reward_bonus_points = req.reward_bonus_points.map(|p| p as i32);
    let reward_expiration_days = req.reward_expiration_days.map(|d| d as i32);
    client.execute(
        "UPDATE garden_config
         SET is_enabled = COALESCE($1, is_enabled),
             reward_discount_percent = COALESCE($2, reward_discount_percent),
             reward_bonus_points = COALESCE($3, reward_bonus_points),
             reward_expiration_days = COALESCE($4, reward_expiration_days),
             updated_at = NOW()
         WHERE id = 1",
        &[
            &is_enabled,
            &reward_discount_percent,
            &reward_bonus_points,
            &reward_expiration_days,
        ],
    ).await.map_err(|e| {
        tracing::error!("Update error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({ "success": true })))
}
