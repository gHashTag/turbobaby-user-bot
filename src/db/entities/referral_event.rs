//! SeaORM entity for `referral_events` — append-only log of referral
//! relationships, with status lifecycle pending → confirmed/paid.
//!
//! Schema (migrations/007_referral_events.sql):
//!   id            UUID PRIMARY KEY DEFAULT gen_random_uuid()
//!   referrer_id   BIGINT NOT NULL
//!   referred_id   BIGINT NOT NULL UNIQUE        -- one event per invitee
//!   code          VARCHAR(20) NOT NULL
//!   bonus_paid    DOUBLE PRECISION NOT NULL DEFAULT 0
//!   status        VARCHAR(20) NOT NULL DEFAULT 'pending'  -- pending / confirmed / paid
//!   source        VARCHAR(50)
//!   created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
//!   confirmed_at  TIMESTAMPTZ

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "referral_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub referrer_id: i64,
    pub referred_id: i64,
    pub code: String,
    pub bonus_paid: f64,
    pub status: String,
    pub source: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub confirmed_at: Option<DateTimeWithTimeZone>,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
