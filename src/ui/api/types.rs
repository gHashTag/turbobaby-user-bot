// API Types — aligned with backend /api/strains JSON schema
use serde::{Deserialize, Serialize};

/// Strain product — matches backend StrainResponse exactly
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Strain {
    pub id: String,
    pub name: String,
    #[serde(rename = "category")]
    pub strain_type: StrainType,
    #[serde(rename = "thc_percent")]
    pub thc: Option<f64>,
    #[serde(rename = "cbd_percent")]
    pub cbd: Option<f64>,
    pub description: String,
    #[serde(rename = "price_per_gram")]
    pub price: f64,
    pub image_url: String,
    #[serde(default)]
    pub video_url: Option<String>,
    pub is_available: bool,
    #[serde(rename = "is_strain_of_day")]
    #[serde(default)]
    pub is_strain_of_day: bool,
    #[serde(default)]
    pub available_grams: Option<f64>,
    #[serde(default)]
    pub strain_of_day_discount: f64,
    #[serde(default)]
    pub effect: Option<String>,
    #[serde(default)]
    pub flavor_profile: Option<String>,
}

impl Default for Strain {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            strain_type: StrainType::Hybrid,
            thc: None,
            cbd: None,
            description: String::new(),
            price: 0.0,
            image_url: String::new(),
            video_url: None,
            is_available: false,
            is_strain_of_day: false,
            available_grams: None,
            strain_of_day_discount: 0.0,
            effect: None,
            flavor_profile: None,
        }
    }
}

impl Strain {
    /// Format THC for display, e.g. "24.0%"
    pub fn thc_display(&self) -> Option<String> {
        self.thc
            .filter(|v| v.is_finite())
            .map(|v| format!("{v:.1}%"))
    }
    /// Format CBD for display
    pub fn cbd_display(&self) -> Option<String> {
        self.cbd
            .filter(|v| v.is_finite())
            .map(|v| format!("{v:.1}%"))
    }
    /// Format price in baht
    pub fn price_display(&self) -> String {
        if self.price > 0.0 {
            format!("{:.0} ฿/g", self.price)
        } else {
            "Price on request".to_string()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StrainType {
    Sativa,
    Indica,
    Hybrid,
}

/// Accessory product
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Accessory {
    pub id: String,
    pub name: String,
    pub category: String,
    pub price: f64,
    pub image_url: String,
    pub is_available: bool,
}

/// Set (bundle of products) - matches backend /api/sets response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Set {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub icon: String,
    #[serde(rename = "type", default)]
    pub set_type: String,
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(rename = "total_price")]
    pub price: f64,
    #[serde(rename = "discount_percent")]
    pub discount: f64,
    pub is_available: bool,
    #[serde(rename = "is_deal_of_day", default)]
    pub is_deal_of_day: bool,
}

/// Tea product
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeaProduct {
    pub id: String,
    pub name: String,
    pub description: String,
    pub price: f64,
    pub image_url: String,
    pub is_available: bool,
}

/// Set (bundle of products)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductSet {
    pub id: String,
    pub name: String,
    pub description: String,
    pub price: f64,
    pub items: Vec<Strain>,
    pub image_url: String,
    pub discount: f64,
    pub is_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Order {
    pub id: String,
    pub telegram_id: i64,
    pub items: Vec<OrderItem>,
    pub total: f64,
    pub status: OrderStatus,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderItem {
    pub id: String,
    pub name: String,
    pub quantity: u32,
    pub price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Pending,
    Confirmed,
    Processing,
    Shipped,
    Delivered,
    Completed,
    Rejected,
    Ready,
    Cancelled,
}

/// Garden plant — matches backend PlantResponse exactly
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Plant {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
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
    Harvested,
}

/// Quest location
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuestLocation {
    pub id: String,
    pub name: String,
    pub description: String,
    pub latitude: f64,
    pub longitude: f64,
    pub reward_id: String,
    pub is_completed: bool,
    pub completed_at: Option<String>,
}

/// Loyalty profile
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoyaltyProfile {
    pub telegram_id: i64,
    pub points: i32,
    pub tier: LoyaltyTier,
    pub bonuses_used: u32,
    pub total_spent: f64,
}

// ── Admin Request Types ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccessoryRequest {
    pub name: String,
    pub category: Option<String>,
    pub description: Option<String>,
    pub price: f64,
    pub stock: Option<i32>,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub is_available: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeaProductRequest {
    pub name: String,
    pub subcategory: Option<String>,
    pub description: Option<String>,
    pub price: f64,
    pub stock: Option<i32>,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub is_available: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetRequest {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub strain_ids: Option<Vec<String>>,
    pub accessory_ids: Option<Vec<String>>,
    pub total_price: f64,
    pub discount_percent: Option<f64>,
    pub is_available: Option<bool>,
    pub is_deal_of_day: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccessorySetRequest {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub accessories: Option<Vec<String>>,
    pub total_price: f64,
    pub discount_percent: Option<f64>,
    pub is_available: Option<bool>,
    pub is_deal_of_day: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LoyaltyTier {
    None,
    Bronze,
    Brass,
    Silver,
    Gold,
}
