use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "treasure_hunts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub black_mark_title: String,
    pub black_mark_description: Option<String>,
    pub black_mark_image_url: Option<String>,
    pub is_active: bool,
    pub starts_at: Option<DateTimeWithTimeZone>,
    pub ends_at: Option<DateTimeWithTimeZone>,
    pub start_lat: f64,
    pub start_lon: f64,
    pub start_name: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
