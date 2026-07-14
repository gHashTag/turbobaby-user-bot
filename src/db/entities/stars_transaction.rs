//! SeaORM entity for `stars_transactions` — append-only ledger of every
//! Stars credit (from games) and debit (from shop purchases).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "stars_transactions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub telegram_id: i64,
    pub amount: i64,
    pub balance_after: i64,
    pub source: String,
    pub reason: String,
    pub external_tx_id: Option<String>,
    pub related_order_id: Option<String>,
    pub created_at: Option<DateTimeWithTimeZone>,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
