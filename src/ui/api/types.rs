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

    // ── TZ #2 marketing flags (cycle #131) ───────────────────────────
    // Server-side `db::strains::Strain` serialises these on every
    // /api/strains response; the wire type just wasn't reading them
    // until now. All `#[serde(default)]` so older API responses keep
    // deserialising.
    #[serde(default)]
    pub discount_percent: f64,
    #[serde(default)]
    pub sale_price: Option<f64>,
    #[serde(default)]
    pub sale_active: bool,
    #[serde(default)]
    pub sale_until: Option<String>,
    #[serde(default)]
    pub is_best_seller: bool,
    #[serde(default)]
    pub is_new_arrival: bool,
    #[serde(default)]
    pub new_until: Option<String>,
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
            discount_percent: 0.0,
            sale_price: None,
            sale_active: false,
            sale_until: None,
            is_best_seller: false,
            is_new_arrival: false,
            new_until: None,
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

/// Calendar event — matches backend /api/events JSON schema
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub title_en: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub description_en: Option<String>,
    pub starts_at: String,
    #[serde(default)]
    pub ends_at: Option<String>,
    #[serde(default)]
    pub location_text: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub video_url: Option<String>,
    #[serde(default)]
    pub photos: Vec<String>,
    #[serde(default)]
    pub max_seats: Option<i32>,
    #[serde(default)]
    pub price_baht: Option<f64>,
    #[serde(default)]
    pub price_stars: Option<i64>,
    #[serde(default)]
    pub is_public: bool,
    #[serde(default)]
    pub seats_taken: i64,
    #[serde(default)]
    pub seats_available: Option<i32>,
    #[serde(default)]
    pub created_at: Option<String>,
}

impl Event {
    /// Localized display title.
    pub fn display_title(&self) -> String {
        crate::ui::lang::localized(&self.title, self.title_en.as_deref())
    }

    /// Localized description, if any.
    pub fn display_description(&self) -> Option<String> {
        self.description
            .as_deref()
            .map(|d| crate::ui::lang::localized(d, self.description_en.as_deref()))
    }

    /// True when the event has a finite capacity and all seats are taken.
    pub fn is_sold_out(&self) -> bool {
        matches!(
            (self.max_seats, self.seats_available),
            (Some(cap), Some(available)) if available <= 0 && cap > 0
        )
    }
}

/// User's event booking — matches backend /api/events/:id/book response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventBooking {
    pub id: String,
    pub event_id: String,
    pub telegram_id: i64,
    pub seats: i32,
    pub status: String,
    #[serde(default)]
    pub order_id: Option<String>,
    #[serde(default)]
    pub stars_paid: Option<i64>,
    #[serde(default)]
    pub stars_tx_id: Option<String>,
    pub created_at: String,
}

/// Body for creating an event booking.
#[derive(Debug, Serialize)]
pub struct BookEventRequest {
    pub telegram_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seats: Option<i32>,
}

// ── Variant C: Reviews / Lab Certificates / LINE broadcast ─────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Review {
    pub id: String,
    pub telegram_id: i64,
    pub strain_id: String,
    pub order_id: String,
    pub rating: i32,
    pub comment: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewsList {
    pub reviews: Vec<Review>,
    pub average_rating: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LabCertificate {
    pub id: String,
    pub strain_id: String,
    #[serde(default)]
    pub certificate_url: Option<String>,
    #[serde(default)]
    pub tested_at: Option<String>,
    pub thc_percent: Option<f64>,
    pub cbd_percent: Option<f64>,
    pub uploaded_by_telegram_id: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LabCertificatesList {
    pub lab_certificates: Vec<LabCertificate>,
}

#[derive(Debug, Serialize)]
pub struct CreateReviewRequest {
    pub order_id: String,
    pub strain_id: String,
    pub rating: i32,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
}

#[derive(Debug, Serialize)]
pub struct BroadcastRequest {
    pub text: String,
    pub photo_url: Option<String>,
    pub product: Option<BroadcastProduct>,
    pub button_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BroadcastProduct {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub image_url: Option<String>,
}

/// Delivery zone with ETA / fee — matches backend `/api/delivery/zones`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeliveryZone {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub name_en: Option<String>,
    pub min_eta_minutes: u32,
    pub max_eta_minutes: u32,
    pub delivery_fee_baht: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeliveryZonesResponse {
    pub zones: Vec<DeliveryZone>,
}

// ── Loop #11: server-side cart wire types ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerCartItem {
    pub id: String,
    pub kind: String,
    pub catalog_id: String,
    #[serde(default)]
    pub quantity: i32,
    #[serde(default)]
    pub unit_price: f64,
    pub name: String,
    #[serde(default)]
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerCart {
    pub telegram_id: i64,
    #[serde(default)]
    pub items: Vec<ServerCartItem>,
    #[serde(default)]
    pub total: f64,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddServerCartItem {
    pub telegram_id: i64,
    pub kind: String,
    pub catalog_id: String,
    #[serde(default)]
    pub quantity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCartMerge {
    pub telegram_id: i64,
    #[serde(default)]
    pub items: Vec<ServerCartItem>,
}
