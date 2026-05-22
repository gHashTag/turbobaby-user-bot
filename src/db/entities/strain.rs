/// SeaORM entity for the `strains` table.
///
/// Schema assembled from:
///   001_initial.sql   — base columns
///   005_alter_strains_sotd.sql — is_strain_of_day, strain_of_day_discount, strain_of_day_set_at
///   016_bilingual_fields.sql   — *_en nullable TEXT columns
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "strains")]
pub struct Model {
    /// VARCHAR(36) PRIMARY KEY DEFAULT gen_random_uuid()::text
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    // ── Core fields (001_initial.sql) ────────────────────────────────
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
    pub created_at: Option<DateTimeWithTimeZone>,

    // ── Strain-of-day columns (005_alter_strains_sotd.sql) ───────────
    pub is_strain_of_day: bool,
    pub strain_of_day_discount: f64,
    pub strain_of_day_set_at: Option<DateTimeWithTimeZone>,

    // ── Bilingual EN fields (016_bilingual_fields.sql) ───────────────
    pub name_en: Option<String>,
    pub description_en: Option<String>,
    pub effect_en: Option<String>,
    pub flavor_profile_en: Option<String>,
    pub strain_type_en: Option<String>,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
