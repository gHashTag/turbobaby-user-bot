//! SeaORM entity for the `loyalty_config` singleton row.
//!
//! Schema:
//!   id INTEGER PRIMARY KEY DEFAULT 1               (001_initial.sql)
//!   config JSONB NOT NULL DEFAULT '{}'             (001_initial.sql)
//!   marketing_badges_hidden BOOLEAN NOT NULL DEFAULT FALSE
//!                                                  (032_loyalty_config_marketing_badges_hidden.sql)
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
    /// Cycle #136: TЗ #2 §5 — admin-controlled "hide all promo badges"
    /// flag. Toggled by `PUT /api/admin/marketing-display`. The
    /// `state.config.hide_marketing_badges` env override (cycle
    /// #133-B) still wins when set, as an ops kill switch.
    #[serde(default)]
    pub marketing_badges_hidden: bool,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
