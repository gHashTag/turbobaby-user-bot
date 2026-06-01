//! SeaORM entity for `block_history` — append-only audit log of every
//! state transition on `loyalty_profiles.is_blocked` (auto_block by
//! fraud detector, manual unblock by admin).
//!
//! Schema (migrations/031_block_history.sql):
//!   id              BIGSERIAL    PRIMARY KEY
//!   created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
//!   telegram_id     BIGINT       NOT NULL
//!   action          TEXT         NOT NULL    -- 'auto_block' | 'unblock'
//!   reason          TEXT
//!   actor_admin_id  BIGINT

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "block_history")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub created_at: DateTimeWithTimeZone,
    pub telegram_id: i64,
    pub action: String,
    pub reason: Option<String>,
    pub actor_admin_id: Option<i64>,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
