//! SeaORM entity for `bonus_transactions` — append-only ledger of every
//! bonus credit / debit (referral payouts, garden harvest, manual admin
//! grants, etc).
//!
//! Schema (from migrations/001_initial.sql + 020_force_double_precision):
//!   id              VARCHAR(36) PRIMARY KEY    -- UUID v4 string
//!   telegram_id     BIGINT NOT NULL
//!   amount          DOUBLE PRECISION NOT NULL  -- positive = credit, negative = debit
//!   tx_type         VARCHAR(50) NOT NULL       -- "referral_bonus", "garden_harvest", etc.
//!   description     TEXT
//!   related_order_id VARCHAR(36)
//!   created_at      TIMESTAMPTZ DEFAULT NOW()

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "bonus_transactions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub telegram_id: i64,
    pub amount: f64,
    pub tx_type: String,
    pub description: Option<String>,
    pub related_order_id: Option<String>,
    pub created_at: Option<DateTimeWithTimeZone>,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
