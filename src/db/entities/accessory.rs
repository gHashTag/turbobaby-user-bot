/// SeaORM entity for the `accessories` table (migration 002_catalog.sql).
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "accessories")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub price: f64,
    pub stock: i32,
    pub image_url: String,
    pub video_url: Option<String>,
    pub is_available: bool,
    pub created_at: Option<DateTimeWithTimeZone>,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
