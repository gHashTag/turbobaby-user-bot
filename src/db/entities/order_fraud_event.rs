//! SeaORM entity for `order_fraud_events` — append-only audit log written
//! on every `POST /api/orders` rejection (subtotal_mismatch, unknown_item,
//! unavailable, malformed).
//!
//! Schema (migrations/030_order_fraud_events.sql):
//!   id                BIGSERIAL    PRIMARY KEY
//!   created_at        TIMESTAMPTZ  NOT NULL DEFAULT NOW()
//!   telegram_id       BIGINT
//!   code              TEXT         NOT NULL
//!   catalog           TEXT
//!   item_id           TEXT
//!   claimed_subtotal  DOUBLE PRECISION
//!   expected_subtotal DOUBLE PRECISION

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "order_fraud_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub created_at: DateTimeWithTimeZone,
    pub telegram_id: Option<i64>,
    pub code: String,
    pub catalog: Option<String>,
    pub item_id: Option<String>,
    pub claimed_subtotal: Option<f64>,
    pub expected_subtotal: Option<f64>,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
