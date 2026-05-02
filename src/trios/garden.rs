//! Garden mechanics for Trios ecosystem

use serde::{Deserialize, Serialize};
use crate::trios::core::{Error, Result, Timestamp};
use crate::trios::validation::validate_water_count;

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
        let new_count = self.water_count + 1;
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

    pub fn harvest(&mut self) -> Result<PlantReward> {
        if !self.is_completed {
            return Err(Error::InvalidState(
                "Plant is not ready for harvest".to_string(),
            ));
        }
        self.harvested_at = Some(chrono::Utc::now().timestamp_millis());
        let now: Timestamp = chrono::Utc::now().timestamp_millis();
        Ok(PlantReward {
            id: format!("reward_{}", self.id),
            plant_id: self.id.clone(),
            user_id: self.user_id.clone(),
            strain_id: self.strain_id.clone(),
            strain_name: self.strain_name.clone(),
            discount_percent: 10,
            bonus_points: 100,
            expires_at: now.saturating_add(7 * 24 * 60 * 60 * 1000),
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
    let planted_at = plant.planted_at;
    let cooldown_ref = if last_watered > 0 {
        last_watered
    } else {
        planted_at
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

    PlantProgress {
        stage,
        stage_name: stage.name().to_string(),
        stage_emoji: stage.emoji().to_string(),
        progress: total_progress.min(100),
        total_progress: total_progress.min(100),
        time_to_next_stage,
        time_to_harvest: 0,
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
        let reward = plant.harvest().unwrap();
        assert_eq!(reward.strain_name, "OG Kush");
        assert_eq!(reward.discount_percent, 10);
        assert_eq!(reward.bonus_points, 100);
    }

    #[test]
    fn test_plant_harvest_not_ready() {
        let mut plant = Plant::new(
            "user123".to_string(),
            "strain456".to_string(),
            "OG Kush".to_string(),
        );
        assert!(plant.harvest().is_err());
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
        let now = 5000;
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
}
