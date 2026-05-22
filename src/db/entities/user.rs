/// SeaORM entity for the `user_languages` table.
///
/// The project stores per-user data (language preference, name) in
/// `user_languages`.  There is no standalone `users` table — this entity
/// maps to the table that holds all user-level rows.
///
/// Columns as created in migration 001_initial.sql:
///   telegram_id  BIGINT PRIMARY KEY
///   language     VARCHAR(10)   NOT NULL DEFAULT 'en'
///   timezone     VARCHAR(50)   DEFAULT 'Asia/Bangkok'
///   first_name   TEXT
///   updated_at   TIMESTAMPTZ   DEFAULT NOW()
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_languages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub telegram_id: i64,
    pub language: String,
    pub timezone: Option<String>,
    pub first_name: Option<String>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
