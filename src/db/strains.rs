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
    pub video_url: Option<String>,
    pub is_available: bool,
    pub is_strain_of_day: bool,
    pub strain_of_day_discount: f64,
    // Bilingual EN fields (nullable, migration 016)
    pub name_en: Option<String>,
    pub description_en: Option<String>,
    pub effect_en: Option<String>,
    pub flavor_profile_en: Option<String>,
    pub strain_type_en: Option<String>,
}

impl Strain {
    pub fn from_row(row: &Row) -> Self {
        let id: String = row.try_get("id").unwrap_or_default();
        let name: String = row.try_get("name").unwrap_or_default();
        let video_url: Option<String> = row.try_get("video_url").ok();
        Self {
            id,
            name,
            category: row.try_get("category").ok(),
            thc_percent: row.try_get("thc_percent").ok(),
            cbd_percent: row.try_get("cbd_percent").ok(),
            effect: row.try_get("effect").ok(),
            flavor_profile: row.try_get("flavor_profile").ok(),
            description: row.try_get("description").ok(),
            price_per_gram: row.try_get("price_per_gram").unwrap_or(0.0),
            available_grams: row.try_get("available_grams").ok(),
            image_url: row.try_get("image_url").ok(),
            video_url,
            is_available: row.try_get("is_available").unwrap_or(false),
            is_strain_of_day: row.try_get("is_strain_of_day").unwrap_or(false),
            strain_of_day_discount: row.try_get("strain_of_day_discount").unwrap_or(0.0),
            // Bilingual EN fields (migration 016, nullable)
            name_en: row.try_get::<_, Option<String>>("name_en").ok().flatten(),
            description_en: row.try_get::<_, Option<String>>("description_en").ok().flatten(),
            effect_en: row.try_get::<_, Option<String>>("effect_en").ok().flatten(),
            flavor_profile_en: row.try_get::<_, Option<String>>("flavor_profile_en").ok().flatten(),
            strain_type_en: row.try_get::<_, Option<String>>("strain_type_en").ok().flatten(),
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
            id: row.try_get("id").unwrap_or_default(),
            name: row.try_get("name").unwrap_or_default(),
            category: row.try_get("category").ok(),
            thc_percent: row.try_get("thc_percent").ok(),
            price_per_gram: row.try_get("price_per_gram").unwrap_or(0.0),
            strain_of_day_discount: row.try_get("strain_of_day_discount").unwrap_or(0.0),
            image_url: row.try_get("image_url").ok(),
        }
    }
}

// Wave 5: SeaORM Entity API path. Старый tokio-postgres код выше — будем удалять в Wave 6+.

/// Returns all strains via SeaORM Entity API.
#[allow(dead_code)]
pub async fn get_all_strains_seaorm(
    orm: &sea_orm::DatabaseConnection,
) -> Result<Vec<crate::db::entities::strain::Model>, sea_orm::DbErr> {
    use sea_orm::EntityTrait;
    crate::db::entities::strain::Entity::find().all(orm).await
}

/// Returns a single strain by its UUID string primary key via SeaORM.
#[allow(dead_code)]
pub async fn get_strain_by_id_seaorm(
    orm: &sea_orm::DatabaseConnection,
    id: &str,
) -> Result<Option<crate::db::entities::strain::Model>, sea_orm::DbErr> {
    use sea_orm::EntityTrait;
    crate::db::entities::strain::Entity::find_by_id(id.to_string()).one(orm).await
}
