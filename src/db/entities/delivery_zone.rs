/// SeaORM entity for the `delivery_zones` table (migration 060).
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "delivery_zones")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: uuid::Uuid,
    pub name: String,
    pub name_en: Option<String>,
    pub fee: f64,
    pub min_order: f64,
    pub eta_min: i32,
    pub eta_max: i32,
    pub is_active: bool,
    pub sort_order: i32,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
