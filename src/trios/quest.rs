//! Quest data models for Woody Weed app

use serde::{Deserialize, Serialize};
use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_TITLE, T_SUBTITLE, T_POINT_1, T_POINT_2, T_POINT_3, T_POINT_4, T_POINT_5};

/// Unique quest identifier
pub type QuestId = String;

/// Unique location identifier
pub type LocationId = String;

/// Quest status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuestStatus {
    /// Quest not started
    NotStarted,
    /// Quest in progress
    InProgress,
    /// Quest completed
    Completed,
    /// Quest failed/abandoned
    Failed,
}

impl QuestStatus {
    /// All possible statuses
    pub fn all() -> &'static [QuestStatus] {
        &[
            QuestStatus::NotStarted,
            QuestStatus::InProgress,
            QuestStatus::Completed,
            QuestStatus::Failed,
        ]
    }

    /// Check if status is terminal
    pub fn is_terminal(&self) -> bool {
        matches!(self, QuestStatus::Completed | QuestStatus::Failed)
    }

    /// Check if status allows progress
    pub fn is_active(&self) -> bool {
        matches!(self, QuestStatus::InProgress)
    }
}

/// A location in the quest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestLocation {
    /// Unique location ID
    pub id: LocationId,

    /// Location name (i18n key)
    pub name_key: String,

    /// Location description (i18n key)
    pub description_key: String,

    /// Coordinates (optional)
    pub coordinates: Option<LocationCoordinates>,

    /// Required items to enter
    pub required_items: Vec<QuestItemId>,

    /// Items available at this location
    pub available_items: Vec<QuestItem>,
}

/// Geographic or map coordinates
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LocationCoordinates {
    pub x: f64,
    pub y: f64,
}

impl LocationCoordinates {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Distance to another coordinate
    pub fn distance_to(&self, other: &LocationCoordinates) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Unique item identifier
pub type QuestItemId = String;

/// An item in the quest system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestItem {
    /// Unique item ID
    pub id: QuestItemId,

    /// Item name (i18n key)
    pub name_key: String,

    /// Item description (i18n key)
    pub description_key: String,

    /// Item type
    pub item_type: QuestItemType,

    /// Whether item is consumable
    pub consumable: bool,
}

/// Type of quest item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestItemType {
    /// Key item for story progression
    Key,
    /// Collectible item
    Collectible,
    /// Usable tool
    Tool,
    /// Treasure/reward
    Treasure,
}

/// Quest reward
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestReward {
    /// Experience points
    pub xp: u32,

    /// Gold/currency
    pub gold: u64,

    /// Item rewards
    pub items: Vec<QuestItem>,
}

impl QuestReward {
    /// Empty reward
    pub fn empty() -> Self {
        Self {
            xp: 0,
            gold: 0,
            items: Vec::new(),
        }
    }

    /// Create a new reward
    pub fn new(xp: u32, gold: u64) -> Self {
        Self {
            xp,
            gold,
            items: Vec::new(),
        }
    }

    /// Add an item to the reward
    pub fn with_item(mut self, item: QuestItem) -> Self {
        self.items.push(item);
        self
    }

    /// Total reward value (for comparison)
    pub fn total_value(&self) -> u64 {
        self.gold + (self.xp as u64)
    }
}

impl Default for QuestReward {
    fn default() -> Self {
        Self::empty()
    }
}

/// A quest definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quest {
    /// Unique quest ID
    pub id: QuestId,

    /// Quest name (i18n key)
    pub name_key: String,

    /// Quest description (i18n key)
    pub description_key: String,

    /// Current status
    pub status: QuestStatus,

    /// Locations in this quest
    pub locations: Vec<QuestLocation>,

    /// Completion reward
    pub reward: QuestReward,

    /// Required quest IDs to unlock
    pub prerequisites: Vec<QuestId>,

    /// Quest metadata
    pub metadata: QuestMetadata,
}

/// Additional quest metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestMetadata {
    /// Created at timestamp
    pub created_at: u64,

    /// Updated at timestamp
    pub updated_at: u64,

    /// Difficulty level (1-10)
    pub difficulty: u8,

    /// Estimated duration in minutes
    pub estimated_duration_mins: u32,
}

impl Quest {
    /// Create a new quest
    pub fn new(
        id: QuestId,
        name_key: String,
        description_key: String,
        reward: QuestReward,
    ) -> Self {
        let now = chrono::Utc::now().timestamp() as u64;

        Self {
            id,
            name_key,
            description_key,
            status: QuestStatus::NotStarted,
            locations: Vec::new(),
            reward,
            prerequisites: Vec::new(),
            metadata: QuestMetadata {
                created_at: now,
                updated_at: now,
                difficulty: 1,
                estimated_duration_mins: 10,
            },
        }
    }

    /// Add a location to the quest
    pub fn with_location(mut self, location: QuestLocation) -> Self {
        self.locations.push(location);
        self
    }

    /// Add a prerequisite quest
    pub fn with_prerequisite(mut self, quest_id: QuestId) -> Self {
        self.prerequisites.push(quest_id);
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: QuestMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Check if quest can be started
    pub fn can_start(&self, completed_quests: &[QuestId]) -> bool {
        self.status == QuestStatus::NotStarted
            && self
                .prerequisites
                .iter()
                .all(|p| completed_quests.contains(p))
    }

    /// Get location by ID
    pub fn get_location(&self, id: &LocationId) -> Option<&QuestLocation> {
        self.locations.iter().find(|l| l.id == *id)
    }

    /// Start the quest
    pub fn start(&mut self) -> Result<(), QuestError> {
        if self.status != QuestStatus::NotStarted {
            return Err(QuestError::InvalidState {
                reason: format!("Cannot start quest in status {:?}", self.status),
            });
        }
        self.status = QuestStatus::InProgress;
        self.touch();
        Ok(())
    }

    /// Complete the quest
    pub fn complete(&mut self) -> Result<(), QuestError> {
        if self.status != QuestStatus::InProgress {
            return Err(QuestError::InvalidState {
                reason: format!("Cannot complete quest in status {:?}", self.status),
            });
        }
        self.status = QuestStatus::Completed;
        self.touch();
        Ok(())
    }

    /// Fail the quest
    pub fn fail(&mut self) -> Result<(), QuestError> {
        if self.status.is_terminal() {
            return Err(QuestError::InvalidState {
                reason: "Quest already in terminal state".to_string(),
            });
        }
        self.status = QuestStatus::Failed;
        self.touch();
        Ok(())
    }

    fn touch(&mut self) {
        self.metadata.updated_at = chrono::Utc::now().timestamp() as u64;
    }
}

/// Quest-specific error type
#[derive(Debug, thiserror::Error)]
pub enum QuestError {
    #[error("Invalid state: {reason}")]
    InvalidState { reason: String },
}

/// Quest checkpoint data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestCheckpoint {
    pub id: u8,
    pub title_key: &'static str,
    pub description_key: &'static str,
    pub reward_bat: u32,
}

impl QuestCheckpoint {
    pub fn new(id: u8, title_key: &'static str, description_key: &'static str) -> Self {
        Self {
            id,
            title_key,
            description_key,
            reward_bat: (id as u32) * 10,
        }
    }

    pub fn title(&self, lang: Lang) -> &str {
        t(lang, self.title_key)
    }

    pub fn description(&self, lang: Lang) -> &str {
        t(lang, self.description_key)
    }
}

/// Quest state data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestState {
    pub telegram_id: i64,
    pub current_checkpoint: u8,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
}

/// All available checkpoints for the location quest
pub fn get_checkpoints() -> Vec<QuestCheckpoint> {
    vec![
        QuestCheckpoint::new(
            1,
            T_POINT_1,
            "quest.checkpoint.1.description",
        ),
        QuestCheckpoint::new(
            2,
            T_POINT_2,
            "quest.checkpoint.2.description",
        ),
        QuestCheckpoint::new(
            3,
            T_POINT_3,
            "quest.checkpoint.3.description",
        ),
        QuestCheckpoint::new(
            4,
            T_POINT_4,
            "quest.checkpoint.4.description",
        ),
        QuestCheckpoint::new(
            5,
            T_POINT_5,
            "quest.checkpoint.5.description",
        ),
    ]
}

/// Get default location quest
pub fn get_location_quest() -> Quest {
    Quest::new(
        "woody-island-quest".to_string(),
        T_TITLE.to_string(),
        T_SUBTITLE.to_string(),
        QuestReward::new(500, 100),
    )
    .with_metadata(QuestMetadata {
        created_at: 0,
        updated_at: 0,
        difficulty: 3,
        estimated_duration_mins: 60,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quest_status_all() {
        let all = QuestStatus::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn test_quest_status_is_terminal() {
        assert!(QuestStatus::Completed.is_terminal());
        assert!(QuestStatus::Failed.is_terminal());
        assert!(!QuestStatus::InProgress.is_terminal());
        assert!(!QuestStatus::NotStarted.is_terminal());
    }

    #[test]
    fn test_location_coordinates_distance() {
        let c1 = LocationCoordinates::new(0.0, 0.0);
        let c2 = LocationCoordinates::new(3.0, 4.0);
        assert_eq!(c1.distance_to(&c2), 5.0);
    }

    #[test]
    fn test_quest_reward_empty() {
        let reward = QuestReward::empty();
        assert_eq!(reward.total_value(), 0);
    }

    #[test]
    fn test_quest_reward_with_item() {
        let item = QuestItem {
            id: "item-1".to_string(),
            name_key: "item.name".to_string(),
            description_key: "item.desc".to_string(),
            item_type: QuestItemType::Key,
            consumable: false,
        };
        let reward = QuestReward::new(100, 50).with_item(item);
        assert_eq!(reward.total_value(), 150);
        assert_eq!(reward.items.len(), 1);
    }

    #[test]
    fn test_quest_new() {
        let quest = Quest::new(
            "quest-1".to_string(),
            "quest.name".to_string(),
            "quest.desc".to_string(),
            QuestReward::new(100, 50),
        );
        assert_eq!(quest.id, "quest-1");
        assert_eq!(quest.status, QuestStatus::NotStarted);
        assert_eq!(quest.locations.len(), 0);
    }

    #[test]
    fn test_quest_start() {
        let mut quest = Quest::new(
            "quest-1".to_string(),
            "quest.name".to_string(),
            "quest.desc".to_string(),
            QuestReward::empty(),
        );
        assert!(quest.start().is_ok());
        assert_eq!(quest.status, QuestStatus::InProgress);
    }

    #[test]
    fn test_quest_start_already_started() {
        let mut quest = Quest::new(
            "quest-1".to_string(),
            "quest.name".to_string(),
            "quest.desc".to_string(),
            QuestReward::empty(),
        );
        quest.start().unwrap();
        assert!(quest.start().is_err());
    }

    #[test]
    fn test_quest_complete() {
        let mut quest = Quest::new(
            "quest-1".to_string(),
            "quest.name".to_string(),
            "quest.desc".to_string(),
            QuestReward::empty(),
        );
        quest.start().unwrap();
        assert!(quest.complete().is_ok());
        assert_eq!(quest.status, QuestStatus::Completed);
    }

    #[test]
    fn test_quest_complete_without_start() {
        let mut quest = Quest::new(
            "quest-1".to_string(),
            "quest.name".to_string(),
            "quest.desc".to_string(),
            QuestReward::empty(),
        );
        assert!(quest.complete().is_err());
    }

    #[test]
    fn test_quest_can_start() {
        let quest = Quest::new(
            "quest-1".to_string(),
            "quest.name".to_string(),
            "quest.desc".to_string(),
            QuestReward::empty(),
        );
        assert!(quest.can_start(&[]));

        let quest_with_prereq = Quest::new(
            "quest-2".to_string(),
            "quest.name".to_string(),
            "quest.desc".to_string(),
            QuestReward::empty(),
        )
        .with_prerequisite("quest-1".to_string());
        assert!(!quest_with_prereq.can_start(&[]));
        assert!(quest_with_prereq.can_start(&["quest-1".to_string()]));
    }

    #[test]
    fn test_quest_get_location() {
        let location = QuestLocation {
            id: "loc-1".to_string(),
            name_key: "loc.name".to_string(),
            description_key: "loc.desc".to_string(),
            coordinates: None,
            required_items: vec![],
            available_items: vec![],
        };

        let quest = Quest::new(
            "quest-1".to_string(),
            "quest.name".to_string(),
            "quest.desc".to_string(),
            QuestReward::empty(),
        )
        .with_location(location);

        assert!(quest.get_location(&"loc-1".to_string()).is_some());
        assert!(quest.get_location(&"loc-2".to_string()).is_none());
    }

    #[test]
    fn test_quest_fail() {
        let mut quest = Quest::new(
            "quest-1".to_string(),
            "quest.name".to_string(),
            "quest.desc".to_string(),
            QuestReward::empty(),
        );
        quest.start().unwrap();
        assert!(quest.fail().is_ok());
        assert_eq!(quest.status, QuestStatus::Failed);
    }

    #[test]
    fn test_quest_fail_already_terminal() {
        let mut quest = Quest::new(
            "quest-1".to_string(),
            "quest.name".to_string(),
            "quest.desc".to_string(),
            QuestReward::empty(),
        );
        quest.start().unwrap();
        quest.complete().unwrap();
        assert!(quest.fail().is_err());
    }

    #[test]
    fn test_checkpoint_new() {
        let cp = QuestCheckpoint::new(1, "title", "desc");
        assert_eq!(cp.id, 1);
        assert_eq!(cp.reward_bat, 10);
    }

    #[test]
    fn test_get_all_checkpoints() {
        let checkpoints = get_checkpoints();
        assert_eq!(checkpoints.len(), 5);
    }

    #[test]
    fn test_get_location_quest() {
        let quest = get_location_quest();
        assert_eq!(quest.id, "woody-island-quest");
        assert_eq!(quest.metadata.difficulty, 3);
    }
}
