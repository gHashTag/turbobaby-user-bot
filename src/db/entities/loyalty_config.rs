//! SeaORM entity for the `loyalty_config` singleton row.
//!
//! Schema (from migrations/001_initial.sql):
//!   id INTEGER PRIMARY KEY DEFAULT 1
//!   config JSONB NOT NULL DEFAULT '{}'
//!
//! Holds the global loyalty configuration as a JSON blob: tier thresholds,
//! cashback percents, happy-hour window, referral bonus amount, etc. Always
//! a single row (id=1) — every read is `find_by_id(1)`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "loyalty_config")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    #[sea_orm(column_type = "Json")]
    pub config: serde_json::Value,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
