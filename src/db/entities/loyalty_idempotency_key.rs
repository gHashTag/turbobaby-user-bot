//! SeaORM entity for `loyalty_idempotency_keys` — replay protection for
//! the loyalty admin endpoints (cycle #158 phase 1).
//!
//! Schema (migrations/033_loyalty_idempotency_keys.sql):
//!   key          TEXT        PRIMARY KEY
//!   tx_id        TEXT        NOT NULL  -- bonus_transactions.id of the original request
//!   telegram_id  BIGINT      NOT NULL
//!   created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
//!
//! Phase 2 (next cycle) wires this into `add_bonus` / `use_bonus` handlers.
//! Until then this entity is intentionally unreferenced — see the
//! `loyalty_idempotency_key` entry in
//! `src/db/mod.rs::entity_wiring_tests::ALLOWED_UNWIRED_ENTITIES`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "loyalty_idempotency_keys")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub key: String,
    pub tx_id: String,
    pub telegram_id: i64,
    pub created_at: DateTimeWithTimeZone,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
