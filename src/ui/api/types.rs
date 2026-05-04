// API Types - Reuse from backend trios module where possible
use serde::{Deserialize, Serialize};

/// Strain product
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
    pub description: Option<String>,
    pub effect: Option<String>,
    pub flavor_profile: Option<String>,
    pub image_url: String,
    #[serde(rename = "price_per_gram")]
    pub price: f64,
    pub available_grams: Option<f64>,
    pub is_available: bool,
    #[serde(default)]
    pub is_strain_of_day: bool,
    #[serde(default)]
    pub strain_of_day_discount: f64,
}

impl Default for Strain {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            strain_type: StrainType::Hybrid,
            thc: None,
            cbd: None,
            description: None,
            effect: None,
            flavor_profile: None,
            image_url: String::new(),
            price: 0.0,
            available_grams: None,
            is_available: false,
            is_strain_of_day: false,
            strain_of_day_discount: 0.0,
        }
    }
}

/// Wrapper matching the API response `{"strains": [...]}`
#[derive(Debug, Deserialize)]
pub struct GetStrainsResponse {
    pub strains: Vec<Strain>,
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

/// Set (bundle of products)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Set {
    pub id: String,
    pub name: String,
    pub description: String,
    pub price: f64,
    pub discount: f64,
    pub image_url: String,
    pub is_available: bool,
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
    Processing,
    Shipped,
    Delivered,
    Cancelled,
}

/// Garden plant
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Plant {
    pub id: String,
    pub telegram_id: i64,
    pub strain_id: String,
    pub strain_name: String,
    pub growth_stage: GrowthStage,
    pub progress: u8,
    pub planted_at: String,
    pub watered_at: Option<String>,
    pub is_ready_for_harvest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GrowthStage {
    Seed,
    Sprout,
    SmallPlant,
    MediumPlant,
    LargePlant,
    Mature,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LoyaltyTier {
    Brass,
    Silver,
    Gold,
}
