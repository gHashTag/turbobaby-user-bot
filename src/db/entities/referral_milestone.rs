//! SeaORM entity for `referral_milestones` — idempotent awards when a referrer
//! reaches 1, 3, or 5 confirmed referrals.
//!
//! Schema (migrations/068_referral_milestones.sql):
//!   referrer_id   BIGINT NOT NULL
//!   milestone     INT NOT NULL            -- 1, 3, 5
//!   bonus_amount  DOUBLE PRECISION NOT NULL DEFAULT 0
//!   achieved_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
//!   PRIMARY KEY (referrer_id, milestone)

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "referral_milestones")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub referrer_id: i64,
    pub milestone: i32,
    pub bonus_amount: f64,
    pub achieved_at: DateTimeWithTimeZone,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
