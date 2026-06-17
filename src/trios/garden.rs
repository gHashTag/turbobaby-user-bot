//! Garden mechanics for Trios ecosystem

use crate::trios::core::{Error, Result, Timestamp};
use crate::trios::validation::validate_water_count;
use serde::{Deserialize, Serialize};

/// Growth stage of a plant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrowthStage {
    Seed,
    Sprout,
    FirstLeaf,
    YoungBush,
    VegStart,
    BigVeg,
    PreFlower,
    SmallBuds,
    BigBuds,
    Trimming,
    Curing,
    Lab,
    Delivery,
    Final,
}

impl GrowthStage {
    pub const TOTAL_STAGES: usize = 14;

    pub fn name(&self) -> &'static str {
        match self {
            Self::Seed => "Семечка",
            Self::Sprout => "Росток",
            Self::FirstLeaf => "Первый лист",
            Self::YoungBush => "Молодой куст",
            Self::VegStart => "Начало веги",
            Self::BigVeg => "Большая вега",
            Self::PreFlower => "Предцвет",
            Self::SmallBuds => "Маленькие шишки",
            Self::BigBuds => "Большие шишки",
            Self::Trimming => "Тримминг",
            Self::Curing => "Пролечка",
            Self::Lab => "Лаборатория",
            Self::Delivery => "Доставка",
            Self::Final => "Финал",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Seed => "🌱",
            Self::Sprout => "🌿",
            Self::FirstLeaf => "🍃",
            Self::YoungBush => "🪴",
            Self::VegStart => "☘️",
            Self::BigVeg => "🌳",
            Self::PreFlower => "🌸",
            Self::SmallBuds => "🌾",
            Self::BigBuds => "🌼",
            Self::Trimming => "✂️",
            Self::Curing => "🏺",
            Self::Lab => "🔬",
            Self::Delivery => "📦",
            Self::Final => "🏆",
        }
    }

    pub fn index(&self) -> usize {
        *self as usize
    }

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Seed),
            1 => Some(Self::Sprout),
            2 => Some(Self::FirstLeaf),
            3 => Some(Self::YoungBush),
            4 => Some(Self::VegStart),
            5 => Some(Self::BigVeg),
            6 => Some(Self::PreFlower),
            7 => Some(Self::SmallBuds),
            8 => Some(Self::BigBuds),
            9 => Some(Self::Trimming),
            10 => Some(Self::Curing),
            11 => Some(Self::Lab),
            12 => Some(Self::Delivery),
            13 => Some(Self::Final),
            _ => None,
        }
    }

    /// Stable snake_case name for DB storage. Must match `from_db_name` and
    /// the `serde(rename_all = "snake_case")` contract exactly.
    pub fn db_name(&self) -> &'static str {
        match self {
            Self::Seed => "seed",
            Self::Sprout => "sprout",
            Self::FirstLeaf => "first_leaf",
            Self::YoungBush => "young_bush",
            Self::VegStart => "veg_start",
            Self::BigVeg => "big_veg",
            Self::PreFlower => "pre_flower",
            Self::SmallBuds => "small_buds",
            Self::BigBuds => "big_buds",
            Self::Trimming => "trimming",
            Self::Curing => "curing",
            Self::Lab => "lab",
            Self::Delivery => "delivery",
            Self::Final => "final",
        }
    }

    /// Parse from the DB snake_case name.
    pub fn from_db_name(name: &str) -> Option<Self> {
        match name {
            "seed" => Some(Self::Seed),
            "sprout" => Some(Self::Sprout),
            "first_leaf" => Some(Self::FirstLeaf),
            "young_bush" => Some(Self::YoungBush),
            "veg_start" => Some(Self::VegStart),
            "big_veg" => Some(Self::BigVeg),
            "pre_flower" => Some(Self::PreFlower),
            "small_buds" => Some(Self::SmallBuds),
            "big_buds" => Some(Self::BigBuds),
            "trimming" => Some(Self::Trimming),
            "curing" => Some(Self::Curing),
            "lab" => Some(Self::Lab),
            "delivery" => Some(Self::Delivery),
            "final" => Some(Self::Final),
            _ => None,
        }
    }
}

/// User's plant
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Plant {
    pub id: String,
    pub user_id: String,
    pub strain_id: String,
    pub strain_name: String,
    pub current_stage: GrowthStage,
    pub planted_at: Timestamp,
    pub is_completed: bool,
    pub harvested_at: Option<Timestamp>,
    pub reward_claimed: bool,
    pub water_count: u32,
    pub last_watered_at: Option<Timestamp>,
}

impl Plant {
    pub fn new(user_id: String, strain_id: String, strain_name: String) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: format!("plant_{}_{}", now, strain_id),
            user_id,
            strain_id,
            strain_name,
            current_stage: GrowthStage::Seed,
            planted_at: now,
            is_completed: false,
            harvested_at: None,
            reward_claimed: false,
            water_count: 0,
            last_watered_at: None,
        }
    }

    pub fn water(&mut self) -> Result<()> {
        if self.is_completed {
            return Err(Error::InvalidState(
                "Plant is already completed".to_string(),
            ));
        }
        let new_count = self
            .water_count
            .checked_add(1)
            .ok_or_else(|| Error::InvalidState("Water count overflow".to_string()))?;
        validate_water_count(new_count)?;
        self.water_count = new_count;
        self.last_watered_at = Some(chrono::Utc::now().timestamp_millis());

        // Update stage based on water count
        if let Some(stage) = GrowthStage::from_index(new_count as usize) {
            self.current_stage = stage;
        }

        if new_count >= 13 {
            self.is_completed = true;
        }

        Ok(())
    }

    pub fn harvest(&mut self, config: &GameConfig) -> Result<PlantReward> {
        if !self.is_completed {
            return Err(Error::InvalidState(
                "Plant is not ready for harvest".to_string(),
            ));
        }
        self.harvested_at = Some(chrono::Utc::now().timestamp_millis());
        let now: Timestamp = chrono::Utc::now().timestamp_millis();
        let expiration_ms = (config.reward_expiration_days as i64)
            .saturating_mul(24)
            .saturating_mul(60)
            .saturating_mul(60)
            .saturating_mul(1000);
        Ok(PlantReward {
            id: format!("reward_{}", self.id),
            plant_id: self.id.clone(),
            user_id: self.user_id.clone(),
            strain_id: self.strain_id.clone(),
            strain_name: self.strain_name.clone(),
            discount_percent: config.reward_discount_percent,
            bonus_points: config.reward_bonus_points,
            expires_at: now.saturating_add(expiration_ms),
            is_used: false,
            created_at: now,
        })
    }
}

/// Plant reward
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantReward {
    pub id: String,
    pub plant_id: String,
    pub user_id: String,
    pub strain_id: String,
    pub strain_name: String,
    pub discount_percent: u32,
    pub bonus_points: u32,
    pub expires_at: Timestamp,
    pub is_used: bool,
    pub created_at: Timestamp,
}

impl PlantReward {
    pub fn is_expired(&self, now: Timestamp) -> bool {
        self.expires_at < now
    }

    pub fn is_active(&self, now: Timestamp) -> bool {
        !self.is_used && !self.is_expired(now)
    }

    pub fn use_reward(&mut self) -> Result<()> {
        if self.is_used {
            return Err(Error::InvalidState("Reward already used".to_string()));
        }
        self.is_used = true;
        Ok(())
    }
}

/// Plant progress calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantProgress {
    pub stage: GrowthStage,
    pub stage_name: String,
    pub stage_emoji: String,
    pub progress: u8,       // 0-100% in current stage
    pub total_progress: u8, // 0-100% overall
    pub time_to_next_stage: i64,
    pub time_to_harvest: i64,
    pub is_ready_to_harvest: bool,
    pub stage_index: usize,
    pub can_water: bool,
    pub next_water_at: Timestamp,
}

/// Watering cooldown in milliseconds (5 minutes)
pub const WATER_COOLDOWN_MS: i64 = 5 * 60 * 1000;
/// Total water stages
pub const TOTAL_WATER_STAGES: usize = 14;
/// The water_count of a fully-grown plant (last valid index, 0-based).
pub const FINAL_WATER_COUNT: i32 = TOTAL_WATER_STAGES as i32 - 1; // 13

/// Outcome of validating a raw persisted `water_count` at the point of a
/// water action. The HTTP path (`api/garden.rs::water_plant`) advances a
/// plant with raw SQL instead of going through [`Plant::water`], so it needs
/// the same validation here. Pure + total so it is unit-testable without a DB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaterStep {
    /// Watering is allowed — persist these values.
    Advance {
        new_count: u32,
        stage: &'static str,
        completed: bool,
    },
    /// Plant is already at the final stage — friendly no-op rejection.
    AtFinalStage,
    /// Persisted count is outside the valid `0..=FINAL_WATER_COUNT` range
    /// (data corruption / bad migration). Callers must FAIL LOUD rather than
    /// silently clamp it (which would reset the plant to Seed and hide the rot).
    Corrupt,
}

/// Pure decision for "advance one watering" given the *persisted* count.
/// Mirrors [`Plant::water`]'s validation for the raw-SQL API boundary.
pub fn next_water_step(water_count: i32) -> WaterStep {
    if !(0..=FINAL_WATER_COUNT).contains(&water_count) {
        // < 0 or > 13 → corrupt persisted state. Do NOT clamp.
        return WaterStep::Corrupt;
    }
    if water_count == FINAL_WATER_COUNT {
        return WaterStep::AtFinalStage;
    }
    // water_count is now 0..=12, so +1 is 1..=13 — no overflow, always a
    // valid stage index.
    let new_count = (water_count + 1) as u32;
    let stage = GrowthStage::from_index(new_count as usize)
        .unwrap_or(GrowthStage::Final)
        .db_name();
    let completed = new_count as i32 >= FINAL_WATER_COUNT;
    WaterStep::Advance {
        new_count,
        stage,
        completed,
    }
}

/// A harvest reward is redeemable iff it has not been used and has not expired.
/// `expires_at` and `now` are epoch millis. Single source of truth for the
/// redemption gate — used by both the rewards list (`is_active`) and the
/// redeem path, which previously disagreed on the boundary (`> now` vs
/// `< now`, differing at exactly `expires_at == now`).
pub fn reward_is_active(is_used: bool, expires_at: i64, now: i64) -> bool {
    !is_used && expires_at > now
}

/// Calculate plant progress
pub fn calculate_progress(plant: &Plant, now: Timestamp) -> PlantProgress {
    if plant.is_completed {
        let final_stage = GrowthStage::Final;
        return PlantProgress {
            stage: final_stage,
            stage_name: final_stage.name().to_string(),
            stage_emoji: final_stage.emoji().to_string(),
            progress: 100,
            total_progress: 100,
            time_to_next_stage: 0,
            time_to_harvest: 0,
            is_ready_to_harvest: true,
            stage_index: final_stage.index(),
            can_water: false,
            next_water_at: 0,
        };
    }

    let water_count = plant.water_count as usize;
    let stage_index = water_count.min(TOTAL_WATER_STAGES - 1);
    let stage = GrowthStage::from_index(stage_index).unwrap_or(GrowthStage::Seed);
    let total_progress = ((water_count as f64 / (TOTAL_WATER_STAGES - 1) as f64) * 100.0) as u8;
    let is_ready_to_harvest = water_count >= TOTAL_WATER_STAGES - 1;

    // Calculate watering cooldown
    let last_watered = plant.last_watered_at.unwrap_or(0);
    let cooldown_ref = if last_watered > 0 {
        last_watered
    } else {
        0 // New plant: no cooldown, can water immediately
    };
    let next_water_at = if cooldown_ref > 0 {
        cooldown_ref.saturating_add(WATER_COOLDOWN_MS)
    } else {
        0
    };
    let can_water =
        !is_ready_to_harvest && (cooldown_ref == 0 && last_watered == 0 || now >= next_water_at);
    let time_to_next_stage = if can_water {
        0
    } else {
        next_water_at.saturating_sub(now)
    };
    let time_to_harvest = if is_ready_to_harvest {
        0
    } else {
        // `saturating_sub`: the `is_ready_to_harvest` guard above already keeps
        // us out of this branch when `water_count >= TOTAL_WATER_STAGES - 1`,
        // but compute the remaining stages defensively so an over-grown /
        // corrupt count (validate_water_count allows up to 100) can never
        // underflow this usize subtraction. Matches the saturating idiom used
        // for the cooldown math above.
        ((TOTAL_WATER_STAGES - 1).saturating_sub(water_count) as i64)
            .saturating_mul(WATER_COOLDOWN_MS)
    };

    PlantProgress {
        stage,
        stage_name: stage.name().to_string(),
        stage_emoji: stage.emoji().to_string(),
        progress: total_progress.min(100),
        total_progress: total_progress.min(100),
        time_to_next_stage,
        time_to_harvest,
        is_ready_to_harvest,
        stage_index,
        can_water,
        next_water_at,
    }
}

/// Game configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub is_enabled: bool,
    pub reward_discount_percent: u32,
    pub reward_bonus_points: u32,
    pub reward_expiration_days: u32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            is_enabled: true,
            reward_discount_percent: 10,
            reward_bonus_points: 100,
            reward_expiration_days: 7,
        }
    }
}

/// Reward config values sanitized into the valid domain, plus whether any value
/// was out of range (so the caller can log the corruption LOUDLY).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SanitizedRewardConfig {
    pub discount_percent: i32,
    pub bonus_points: i32,
    pub expiration_days: i32,
    /// true when at least one input was outside its valid range and got clamped.
    pub corrupted: bool,
}

/// Clamp garden reward config — read from `garden_config` at harvest time — into
/// the SAME domain the admin update validator enforces (discount 0..=100, bonus
/// 0..=1_000_000, days 1..=365).
///
/// The admin API accepts only `u32` (`ConfigUpdateRequest`), so a negative value
/// here cannot arrive through the API — it means DB corruption or a bad
/// migration. That matters because `harvest_plant` writes `bonus_points` into a
/// `garden_rewards` row that `use_reward` later CREDITS to `bonus_balance`
/// (`bonus_balance + $1`): a negative would silently *subtract* from a user's
/// balance, and a negative discount would invert into a surcharge. `get_config`
/// already clamps `.max(0)` for display — this gives the harvest (financial)
/// path the same defense, and reports `corrupted` so the caller logs it instead
/// of swallowing it silently (fail loud, degrade safe). See
/// [[fail-loud-not-silent-clamp]] / [[triage-subagent-finding-then-harden]].
pub fn sanitize_reward_config(
    discount_percent: i32,
    bonus_points: i32,
    expiration_days: i32,
) -> SanitizedRewardConfig {
    let d = discount_percent.clamp(0, 100);
    let b = bonus_points.clamp(0, 1_000_000);
    let days = expiration_days.clamp(1, 365);
    SanitizedRewardConfig {
        discount_percent: d,
        bonus_points: b,
        expiration_days: days,
        corrupted: d != discount_percent || b != bonus_points || days != expiration_days,
    }
}

/// Filter active rewards
pub fn filter_active_rewards(rewards: &[PlantReward], now: Timestamp) -> Vec<&PlantReward> {
    rewards.iter().filter(|r| r.is_active(now)).collect()
}

/// Count active rewards
pub fn count_active_rewards(rewards: &[PlantReward], now: Timestamp) -> usize {
    filter_active_rewards(rewards, now).len()
}

/// Get active (growing) plants
pub fn filter_active_plants(plants: &[Plant]) -> Vec<&Plant> {
    plants.iter().filter(|p| !p.is_completed).collect()
}

/// Get completed plants
pub fn filter_completed_plants(plants: &[Plant]) -> Vec<&Plant> {
    plants.iter().filter(|p| p.is_completed).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reward_is_active_boundary_and_states() {
        let now = 1_000_000i64;
        // Active: not used, expiry strictly in the future.
        assert!(reward_is_active(false, now + 1, now));
        // Exactly at expiry → NOT active (boundary unified across both paths).
        assert!(!reward_is_active(false, now, now));
        // Past expiry → not active.
        assert!(!reward_is_active(false, now - 1, now));
        // Used → never active, even if not expired.
        assert!(!reward_is_active(true, now + 10_000, now));
    }

    #[test]
    fn test_sanitize_reward_config_passes_valid_values_unchanged() {
        let s = sanitize_reward_config(10, 100, 7);
        assert_eq!(s.discount_percent, 10);
        assert_eq!(s.bonus_points, 100);
        assert_eq!(s.expiration_days, 7);
        assert!(!s.corrupted);
        // Domain boundaries are valid, not corrupt.
        let edge = sanitize_reward_config(100, 1_000_000, 365);
        assert_eq!(edge.discount_percent, 100);
        assert_eq!(edge.bonus_points, 1_000_000);
        assert_eq!(edge.expiration_days, 365);
        assert!(!edge.corrupted);
        let edge_lo = sanitize_reward_config(0, 0, 1);
        assert!(!edge_lo.corrupted);
    }

    #[test]
    fn test_sanitize_reward_config_negative_bonus_cannot_subtract_from_balance() {
        // A negative bonus from a tampered config would otherwise SUBTRACT from a
        // user's bonus_balance in use_reward — clamp to 0 and flag corruption.
        let s = sanitize_reward_config(-50, -200, -3);
        assert_eq!(s.discount_percent, 0);
        assert_eq!(
            s.bonus_points, 0,
            "negative bonus must never credit-negative"
        );
        assert_eq!(s.expiration_days, 1);
        assert!(s.corrupted);
    }

    #[test]
    fn test_sanitize_reward_config_clamps_over_range_and_flags() {
        let s = sanitize_reward_config(150, 9_999_999, 4000);
        assert_eq!(s.discount_percent, 100);
        assert_eq!(s.bonus_points, 1_000_000);
        assert_eq!(s.expiration_days, 365);
        assert!(s.corrupted);
        // i32 extremes must not panic and must clamp.
        let ext = sanitize_reward_config(i32::MIN, i32::MAX, i32::MIN);
        assert_eq!(ext.discount_percent, 0);
        assert_eq!(ext.bonus_points, 1_000_000);
        assert_eq!(ext.expiration_days, 1);
        assert!(ext.corrupted);
    }

    #[test]
    fn test_next_water_step_advances_from_seed() {
        match next_water_step(0) {
            WaterStep::Advance {
                new_count,
                stage,
                completed,
            } => {
                assert_eq!(new_count, 1);
                assert_eq!(stage, "sprout");
                assert!(!completed);
            }
            other => panic!("expected Advance, got {other:?}"),
        }
    }

    #[test]
    fn test_next_water_step_last_advance_completes() {
        // 12 → 13 is the final watering; must mark completed.
        match next_water_step(FINAL_WATER_COUNT - 1) {
            WaterStep::Advance {
                new_count,
                stage,
                completed,
            } => {
                assert_eq!(new_count, FINAL_WATER_COUNT as u32);
                assert_eq!(stage, "final");
                assert!(completed);
            }
            other => panic!("expected completing Advance, got {other:?}"),
        }
    }

    #[test]
    fn test_next_water_step_at_final_stage_is_noop() {
        assert_eq!(next_water_step(FINAL_WATER_COUNT), WaterStep::AtFinalStage);
    }

    #[test]
    fn test_next_water_step_negative_is_corrupt_not_clamped() {
        // Regression: the API used to `.max(0)` a negative count, silently
        // resetting the plant to Seed. Corruption must be rejected, not hidden.
        assert_eq!(next_water_step(-1), WaterStep::Corrupt);
        assert_eq!(next_water_step(-5), WaterStep::Corrupt);
        assert_eq!(next_water_step(i32::MIN), WaterStep::Corrupt);
    }

    #[test]
    fn test_next_water_step_above_final_is_corrupt_not_final() {
        // Out-of-range high counts are corruption, distinct from the friendly
        // AtFinalStage no-op at exactly 13.
        assert_eq!(next_water_step(FINAL_WATER_COUNT + 1), WaterStep::Corrupt);
        assert_eq!(next_water_step(50), WaterStep::Corrupt);
        assert_eq!(next_water_step(i32::MAX), WaterStep::Corrupt);
    }

    #[test]
    fn test_next_water_step_every_valid_count_maps_to_a_real_stage() {
        // For all advanceable counts (0..=12) the produced stage must be a
        // real, round-trippable db_name — never the unwrap_or(Final) fallback
        // masking an out-of-range index.
        for wc in 0..FINAL_WATER_COUNT {
            match next_water_step(wc) {
                WaterStep::Advance { stage, .. } => {
                    assert!(
                        GrowthStage::from_db_name(stage).is_some(),
                        "count {wc} produced unknown stage {stage:?}"
                    );
                }
                other => panic!("count {wc} should Advance, got {other:?}"),
            }
        }
    }

    #[test]
    fn test_plant_new() {
        let plant = Plant::new(
            "user123".to_string(),
            "strain456".to_string(),
            "OG Kush".to_string(),
        );
        assert_eq!(plant.current_stage, GrowthStage::Seed);
        assert_eq!(plant.water_count, 0);
        assert!(!plant.is_completed);
    }

    #[test]
    fn test_plant_water() {
        let mut plant = Plant::new(
            "user123".to_string(),
            "strain456".to_string(),
            "OG Kush".to_string(),
        );
        plant.water().unwrap();
        assert_eq!(plant.water_count, 1);
        assert_eq!(plant.current_stage, GrowthStage::Sprout);
        plant.water().unwrap();
        assert_eq!(plant.water_count, 2);
        assert_eq!(plant.current_stage, GrowthStage::FirstLeaf);
    }

    #[test]
    fn test_plant_water_max() {
        let mut plant = Plant::new(
            "user123".to_string(),
            "strain456".to_string(),
            "OG Kush".to_string(),
        );
        plant.water_count = 100;
        assert!(plant.water().is_err());
    }

    #[test]
    fn test_plant_harvest() {
        let mut plant = Plant::new(
            "user123".to_string(),
            "strain456".to_string(),
            "OG Kush".to_string(),
        );
        plant.is_completed = true;
        plant.water_count = 13;
        let config = GameConfig::default();
        let reward = plant.harvest(&config).unwrap();
        assert_eq!(reward.strain_name, "OG Kush");
        assert_eq!(reward.discount_percent, config.reward_discount_percent);
        assert_eq!(reward.bonus_points, config.reward_bonus_points);
    }

    #[test]
    fn test_plant_harvest_not_ready() {
        let mut plant = Plant::new(
            "user123".to_string(),
            "strain456".to_string(),
            "OG Kush".to_string(),
        );
        let config = GameConfig::default();
        assert!(plant.harvest(&config).is_err());
    }

    #[test]
    fn test_growth_stage_names() {
        assert_eq!(GrowthStage::Seed.name(), "Семечка");
        assert_eq!(GrowthStage::Final.name(), "Финал");
    }

    #[test]
    fn test_growth_stage_emoji() {
        assert_eq!(GrowthStage::Seed.emoji(), "🌱");
        assert_eq!(GrowthStage::Final.emoji(), "🏆");
    }

    #[test]
    fn test_growth_stage_index() {
        assert_eq!(GrowthStage::Seed.index(), 0);
        assert_eq!(GrowthStage::Final.index(), 13);
    }

    #[test]
    fn test_growth_stage_from_index() {
        assert_eq!(GrowthStage::from_index(0), Some(GrowthStage::Seed));
        assert_eq!(GrowthStage::from_index(13), Some(GrowthStage::Final));
        assert_eq!(GrowthStage::from_index(14), None);
    }

    #[test]
    fn test_calculate_progress_new() {
        let plant = Plant::new(
            "user123".to_string(),
            "strain456".to_string(),
            "OG Kush".to_string(),
        );
        let now = chrono::Utc::now().timestamp_millis();
        let progress = calculate_progress(&plant, now);
        assert_eq!(progress.stage, GrowthStage::Seed);
        assert_eq!(progress.total_progress, 0);
        assert!(progress.can_water);
    }

    #[test]
    fn test_calculate_progress_overgrown_water_count_no_underflow() {
        // Regression: an over-grown / corrupt water_count (> TOTAL_WATER_STAGES-1;
        // validate_water_count allows up to 100) must NOT underflow the usize
        // subtraction in time_to_harvest. The is_ready_to_harvest guard already
        // keeps us out of that branch, but saturating_sub makes it safe even if
        // the guard is ever refactored.
        let mut plant = Plant::new(
            "user123".to_string(),
            "strain456".to_string(),
            "OG Kush".to_string(),
        );
        plant.water_count = 50; // way past the 13-stage cap
        let now = chrono::Utc::now().timestamp_millis();
        let progress = calculate_progress(&plant, now); // must not panic
        assert!(progress.is_ready_to_harvest);
        assert_eq!(progress.time_to_harvest, 0);
        assert_eq!(progress.stage_index, TOTAL_WATER_STAGES - 1); // clamped to Final
    }

    #[test]
    fn test_calculate_progress_watered() {
        let mut plant = Plant::new(
            "user123".to_string(),
            "strain456".to_string(),
            "OG Kush".to_string(),
        );
        plant.water().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let progress = calculate_progress(&plant, now);
        assert_eq!(progress.stage, GrowthStage::Sprout);
        assert_eq!(progress.total_progress, 7); // ~1/13
    }

    #[test]
    fn test_calculate_progress_completed() {
        let mut plant = Plant::new(
            "user123".to_string(),
            "strain456".to_string(),
            "OG Kush".to_string(),
        );
        plant.is_completed = true;
        let now = chrono::Utc::now().timestamp_millis();
        let progress = calculate_progress(&plant, now);
        assert_eq!(progress.stage, GrowthStage::Final);
        assert_eq!(progress.total_progress, 100);
        assert!(progress.is_ready_to_harvest);
    }

    #[test]
    fn test_plant_reward_is_active() {
        let _now = 5000;
        let reward = PlantReward {
            id: "reward1".to_string(),
            plant_id: "plant1".to_string(),
            user_id: "user1".to_string(),
            strain_id: "strain1".to_string(),
            strain_name: "Test".to_string(),
            discount_percent: 10,
            bonus_points: 100,
            expires_at: 10000,
            is_used: false,
            created_at: 0,
        };
        assert!(reward.is_active(5000));
        assert!(!reward.is_active(15000));
    }

    #[test]
    fn test_plant_reward_use() {
        let mut reward = PlantReward {
            id: "reward1".to_string(),
            plant_id: "plant1".to_string(),
            user_id: "user1".to_string(),
            strain_id: "strain1".to_string(),
            strain_name: "Test".to_string(),
            discount_percent: 10,
            bonus_points: 100,
            expires_at: 10000,
            is_used: false,
            created_at: 0,
        };
        reward.use_reward().unwrap();
        assert!(reward.is_used);
        assert!(reward.use_reward().is_err());
    }

    #[test]
    fn test_filter_active_plants() {
        let plant1 = Plant::new(
            "user1".to_string(),
            "strain1".to_string(),
            "Test".to_string(),
        );
        let mut plant2 = Plant::new(
            "user1".to_string(),
            "strain2".to_string(),
            "Test".to_string(),
        );
        plant2.is_completed = true;

        let plants = vec![plant1, plant2];
        let active = filter_active_plants(&plants);
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn test_filter_active_rewards() {
        let now = 5000;
        let reward1 = PlantReward {
            id: "r1".to_string(),
            plant_id: "p1".to_string(),
            user_id: "u1".to_string(),
            strain_id: "s1".to_string(),
            strain_name: "Test".to_string(),
            discount_percent: 10,
            bonus_points: 100,
            expires_at: 10000,
            is_used: false,
            created_at: 0,
        };
        let reward2 = PlantReward {
            id: "r2".to_string(),
            plant_id: "p2".to_string(),
            user_id: "u1".to_string(),
            strain_id: "s2".to_string(),
            strain_name: "Test".to_string(),
            discount_percent: 10,
            bonus_points: 100,
            expires_at: 1000,
            is_used: false,
            created_at: 0,
        };

        let rewards = vec![reward1, reward2];
        let active = filter_active_rewards(&rewards, now);
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn test_game_config_default() {
        let config = GameConfig::default();
        assert!(config.is_enabled);
        assert_eq!(config.reward_discount_percent, 10);
        assert_eq!(config.reward_bonus_points, 100);
    }

    #[test]
    fn test_growth_stage_db_name_roundtrip() {
        let stages = [
            GrowthStage::Seed,
            GrowthStage::Sprout,
            GrowthStage::FirstLeaf,
            GrowthStage::YoungBush,
            GrowthStage::VegStart,
            GrowthStage::BigVeg,
            GrowthStage::PreFlower,
            GrowthStage::SmallBuds,
            GrowthStage::BigBuds,
            GrowthStage::Trimming,
            GrowthStage::Curing,
            GrowthStage::Lab,
            GrowthStage::Delivery,
            GrowthStage::Final,
        ];
        for stage in stages {
            let name = stage.db_name();
            let parsed = GrowthStage::from_db_name(name).unwrap();
            assert_eq!(stage, parsed, "db_name roundtrip failed for {:?}", stage);
        }
    }

    #[test]
    fn test_growth_stage_db_name_snake_case() {
        assert_eq!(GrowthStage::FirstLeaf.db_name(), "first_leaf");
        assert_eq!(GrowthStage::YoungBush.db_name(), "young_bush");
        assert_eq!(GrowthStage::PreFlower.db_name(), "pre_flower");
        assert_eq!(GrowthStage::SmallBuds.db_name(), "small_buds");
        assert_eq!(GrowthStage::BigBuds.db_name(), "big_buds");
    }

    #[test]
    fn test_growth_stage_from_db_name_unknown() {
        assert_eq!(GrowthStage::from_db_name("unknown"), None);
        assert_eq!(GrowthStage::from_db_name("firstleaf"), None); // no underscore
    }

    #[test]
    fn test_calculate_progress_time_to_harvest() {
        let mut plant = Plant::new(
            "user123".to_string(),
            "strain456".to_string(),
            "OG Kush".to_string(),
        );
        let now = chrono::Utc::now().timestamp_millis();
        let progress = calculate_progress(&plant, now);
        // 13 stages total, water_count=0 → 13 waters remaining
        assert_eq!(
            progress.time_to_harvest,
            13 * WATER_COOLDOWN_MS,
            "time_to_harvest should account for remaining waters"
        );

        plant.water_count = 10;
        let progress = calculate_progress(&plant, now);
        assert_eq!(progress.time_to_harvest, 3 * WATER_COOLDOWN_MS);
    }
}
