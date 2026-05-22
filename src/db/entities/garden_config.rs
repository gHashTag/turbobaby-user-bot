use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "garden_config")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub is_enabled: bool,
    pub reward_discount_percent: i32,
    pub reward_bonus_points: i32,
    pub reward_expiration_days: i32,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
