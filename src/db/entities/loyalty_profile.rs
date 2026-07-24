use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "loyalty_profiles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub telegram_id: i64,
    // total_spent / bonus_balance в БД могут быть NUMERIC или DOUBLE PRECISION.
    // SeaORM умеет читать оба варианта через Decimal либо f64 — берём f64 (sqlx сам кастит).
    pub total_spent: Option<f64>,
    pub bonus_balance: Option<f64>,
    pub tier: String,
    pub referral_code: Option<String>,
    pub referred_by: Option<i64>,
    pub referral_count: i32,
    pub first_purchase_at: Option<DateTimeWithTimeZone>,
    pub manager_telegram_id: Option<i64>,
    pub is_blocked: bool,
    pub age_verified: bool,
    pub date_of_birth: Option<chrono::NaiveDate>,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
