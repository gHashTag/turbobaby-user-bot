use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "quest_places")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub category: String,
    pub lat: f64,
    pub lon: f64,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub is_available: bool,
    pub created_at: DateTimeWithTimeZone,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
