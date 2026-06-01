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
    // Marketing flags (migration 028, TZ #2). Wire-compatible: clients that
    // don't know about these fields just ignore them.
    #[serde(default)]
    pub discount_percent: f64,
    #[serde(default)]
    pub sale_price: Option<f64>,
    #[serde(default)]
    pub sale_active: bool,
    /// RFC3339 string when set; None means "no expiry" or "not set".
    #[serde(default)]
    pub sale_until: Option<String>,
    #[serde(default)]
    pub is_best_seller: bool,
    #[serde(default)]
    pub is_new_arrival: bool,
    #[serde(default)]
    pub new_until: Option<String>,
    #[serde(default)]
    pub display_order: i32,
}

#[cfg(test)]
mod tests {
    use super::Strain;

    #[test]
    fn test_strain_serde_roundtrip() {
        let s = Strain {
            id: "uuid-1".into(),
            name: "OG Kush".into(),
            category: Some("Indica".into()),
            thc_percent: Some(22.5),
            cbd_percent: Some(0.1),
            effect: Some("Relax".into()),
            flavor_profile: Some("Earthy".into()),
            description: Some("Classic.".into()),
            price_per_gram: 350.0,
            available_grams: Some(100.0),
            image_url: Some("/uploads/abc.jpg".into()),
            video_url: None,
            is_available: true,
            is_strain_of_day: false,
            strain_of_day_discount: 0.0,
            name_en: Some("OG Kush EN".into()),
            description_en: None,
            effect_en: None,
            flavor_profile_en: None,
            strain_type_en: None,
            discount_percent: 0.0,
            sale_price: None,
            sale_active: false,
            sale_until: None,
            is_best_seller: false,
            is_new_arrival: false,
            new_until: None,
            display_order: 0,
        };
        let json = serde_json::to_value(&s).unwrap();
        let back: Strain = serde_json::from_value(json).unwrap();
        assert_eq!(back.id, "uuid-1");
        assert_eq!(back.price_per_gram, 350.0);
        assert!(back.is_available);
    }
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
            thc_percent: row
                .try_get::<_, f64>("thc_percent")
                .ok()
                .filter(|v| v.is_finite()),
            cbd_percent: row
                .try_get::<_, f64>("cbd_percent")
                .ok()
                .filter(|v| v.is_finite()),
            effect: row.try_get("effect").ok(),
            flavor_profile: row.try_get("flavor_profile").ok(),
            description: row.try_get("description").ok(),
            price_per_gram: {
                let v = row.try_get::<_, f64>("price_per_gram").unwrap_or(0.0);
                if v.is_finite() {
                    v.max(0.0)
                } else {
                    0.0
                }
            },
            available_grams: row
                .try_get::<_, f64>("available_grams")
                .ok()
                .filter(|v| v.is_finite()),
            image_url: row.try_get("image_url").ok(),
            video_url,
            is_available: row.try_get("is_available").unwrap_or(false),
            is_strain_of_day: row.try_get("is_strain_of_day").unwrap_or(false),
            strain_of_day_discount: {
                let v = row
                    .try_get::<_, f64>("strain_of_day_discount")
                    .unwrap_or(0.0);
                if v.is_finite() {
                    v.max(0.0)
                } else {
                    0.0
                }
            },
            // Bilingual EN fields (migration 016, nullable)
            name_en: row.try_get::<_, Option<String>>("name_en").ok().flatten(),
            description_en: row
                .try_get::<_, Option<String>>("description_en")
                .ok()
                .flatten(),
            effect_en: row.try_get::<_, Option<String>>("effect_en").ok().flatten(),
            flavor_profile_en: row
                .try_get::<_, Option<String>>("flavor_profile_en")
                .ok()
                .flatten(),
            strain_type_en: row
                .try_get::<_, Option<String>>("strain_type_en")
                .ok()
                .flatten(),
            // Marketing flags (migration 028, TZ #2). All optional in legacy
            // SELECTs — try_get falls back to default if the column wasn't in
            // the projection.
            discount_percent: {
                let v = row.try_get::<_, f64>("discount_percent").unwrap_or(0.0);
                if v.is_finite() {
                    v.max(0.0)
                } else {
                    0.0
                }
            },
            sale_price: row
                .try_get::<_, Option<f64>>("sale_price")
                .ok()
                .flatten()
                .filter(|v| v.is_finite()),
            sale_active: row.try_get("sale_active").unwrap_or(false),
            sale_until: row
                .try_get::<_, Option<chrono::DateTime<chrono::Utc>>>("sale_until")
                .ok()
                .flatten()
                .map(|t| t.to_rfc3339()),
            is_best_seller: row.try_get("is_best_seller").unwrap_or(false),
            is_new_arrival: row.try_get("is_new_arrival").unwrap_or(false),
            new_until: row
                .try_get::<_, Option<chrono::DateTime<chrono::Utc>>>("new_until")
                .ok()
                .flatten()
                .map(|t| t.to_rfc3339()),
            display_order: row.try_get("display_order").unwrap_or(0),
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
            thc_percent: row
                .try_get::<_, f64>("thc_percent")
                .ok()
                .filter(|v| v.is_finite()),
            price_per_gram: {
                let v = row.try_get::<_, f64>("price_per_gram").unwrap_or(0.0);
                if v.is_finite() {
                    v.max(0.0)
                } else {
                    0.0
                }
            },
            strain_of_day_discount: {
                let v = row
                    .try_get::<_, f64>("strain_of_day_discount")
                    .unwrap_or(0.0);
                if v.is_finite() {
                    v.max(0.0)
                } else {
                    0.0
                }
            },
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
    crate::db::entities::strain::Entity::find_by_id(id.to_string())
        .one(orm)
        .await
}
