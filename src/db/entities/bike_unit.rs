/// SeaORM entity for the `bike_units` table — one row per PHYSICAL bike.
/// A customer books a family (`bikes`); the shop assigns the unit (D8).
///
/// Schema assembled from:
///   078_bike_units.sql — every column below lands together
///        (DECISIONS.md D1/D2)
///
/// Nothing here identifies a person or a vehicle registration: `unit_code`
/// is a shop-internal label, never a plate number, and the register's
/// renter names, debts, key codes, TAX and insurance dates stay out of the
/// repository entirely (D14).
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "bike_units")]
pub struct Model {
    /// VARCHAR(36) PRIMARY KEY DEFAULT gen_random_uuid()::text
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    // ── Family link (078_bike_units.sql) ─────────────────────────────
    /// REFERENCES bikes(id) ON DELETE CASCADE. The FK is enforced by
    /// Postgres and joined by hand — this repo models no SeaORM relations.
    pub bike_id: String,

    // ── Identity (078_bike_units.sql) ────────────────────────────────
    /// UNIQUE NOT NULL shop-internal code: `nmax-155-01`. NOT a plate.
    pub unit_code: String,
    pub model_year: Option<i32>,
    pub color: Option<String>,

    // ── Condition (078_bike_units.sql) ───────────────────────────────
    /// Kilometres travelled since TurboBaby acquired the unit — NOT a total
    /// odometer reading. One unit reads 2900 since purchase against 16433
    /// on the clock, so the two are not interchangeable and this column is
    /// never labelled "mileage" or "odometer" in any surface.
    pub km_since_purchase: Option<i32>,
    /// CHECK (status IN ('available','rented','service','retired')). The
    /// catalog counts `available` rows; it never infers availability from
    /// a flag on the family.
    pub status: String,

    // ── Timestamps (078_bike_units.sql) ──────────────────────────────
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
