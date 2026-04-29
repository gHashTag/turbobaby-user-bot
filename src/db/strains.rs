use serde::{Deserialize, Serialize};
use tokio_postgres::Row;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub is_strain_of_day: bool,
    pub strain_of_day_discount: f64,
}

impl Strain {
    pub fn from_row(row: &Row) -> Self {
        Self {
            id: row.get("id"),
            name: row.get("name"),
            category: row.get("category"),
            thc_percent: row.get("thc_percent"),
            cbd_percent: row.get("cbd_percent"),
            effect: row.get("effect"),
            flavor_profile: row.get("flavor_profile"),
            description: row.get("description"),
            price_per_gram: row.get("price_per_gram"),
            available_grams: row.get("available_grams"),
            image_url: row.get("image_url"),
            is_available: row.get("is_available"),
            is_strain_of_day: row.get("is_strain_of_day"),
            strain_of_day_discount: row.get("strain_of_day_discount"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrainOfDay {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    pub thc_percent: Option<f64>,
    pub price_per_gram: f64,
    pub strain_of_day_discount: f64,
    pub image_url: Option<String>,
}

impl StrainOfDay {
    pub fn from_row(row: &Row) -> Self {
        Self {
            id: row.get("id"),
            name: row.get("name"),
            category: row.get("category"),
            thc_percent: row.get("thc_percent"),
            price_per_gram: row.get("price_per_gram"),
            strain_of_day_discount: row.get("strain_of_day_discount"),
            image_url: row.get("image_url"),
        }
    }
}
