//! SeaORM entity for `notification_queue` — persisted outbound Telegram
//! notifications with at-least-once delivery.
//!
//! Schema (migrations/067_notification_queue.sql):
//!   id            UUID PRIMARY KEY DEFAULT gen_random_uuid()
//!   telegram_id   BIGINT NOT NULL
//!   kind          VARCHAR(50) NOT NULL  -- friend_joined / friend_ordered / milestone (others held)
//!   payload       JSONB NOT NULL DEFAULT '{}'
//!   scheduled_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
//!   processed_at  TIMESTAMPTZ
//!   attempts      INT NOT NULL DEFAULT 0
//!   created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "notification_queue")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub telegram_id: i64,
    pub kind: String,
    #[sea_orm(column_type = "Json")]
    pub payload: serde_json::Value,
    pub scheduled_at: DateTimeWithTimeZone,
    pub processed_at: Option<DateTimeWithTimeZone>,
    pub attempts: i32,
    pub created_at: DateTimeWithTimeZone,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
