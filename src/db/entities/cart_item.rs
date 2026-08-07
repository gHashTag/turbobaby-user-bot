/// SeaORM entity for the `cart_items` table (migration 059).
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cart_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: uuid::Uuid,
    pub cart_id: uuid::Uuid,
    pub kind: String,
    pub catalog_id: String,
    pub quantity: i32,
    pub unit_price: f64,
    pub name: String,
    pub image_url: Option<String>,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
