//! SeaORM entity for `user_stars` — single server-side balance of the Stars (⭐)
//! internal currency shared between games and the Telegram shop.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_stars")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub telegram_id: i64,
    pub balance: i64,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
