use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Strain {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    pub thc_percent: Option<f64>,
    pub cbd_percent: Option<f64>,
    pub effect: Option<String>,
    pub flavor_profile: Option<String>,
    pub description: Option<String>,
    pub price_per_gram: f64,
    pub available_grams: Option<f64>,
    pub image_url: Option<String>,
    pub is_available: bool,
    pub is_strain_of_day: bool,  // Changed from Option<bool> - DB has NOT NULL DEFAULT false
    pub strain_of_day_discount: f64,  // Changed from Option<f64> - DB has NOT NULL DEFAULT 0
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StrainOfDay {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    pub thc_percent: Option<f64>,
    pub price_per_gram: f64,
    pub strain_of_day_discount: f64,  // Changed from Option<f64> - DB has NOT NULL DEFAULT 0
    pub image_url: Option<String>,
}
