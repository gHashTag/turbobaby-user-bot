//! SeaORM entity for `order_idempotency_keys` — Stripe/AWS-style replay
//! protection for `POST /api/orders` (cycle #57).
//!
//! Schema (migrations/029_order_idempotency_keys.sql):
//!   key          TEXT        PRIMARY KEY
//!   order_id     TEXT        NOT NULL
//!   telegram_id  BIGINT
//!   created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "order_idempotency_keys")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub key: String,
    pub order_id: String,
    pub telegram_id: Option<i64>,
    pub created_at: DateTimeWithTimeZone,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
