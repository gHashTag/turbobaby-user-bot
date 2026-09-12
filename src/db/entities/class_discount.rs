/// SeaORM entity for the `class_discounts` table — the published discount
/// applied to a class's PRE-discount base tariff: `scooter` 0.25,
/// `motorcycle` 0.15.
///
/// Schema assembled from:
///   079_rental_terms.sql — both columns land together, seeded in the
///        same file (DECISIONS.md D1/D2)
///
/// Two rows, keyed by class, rather than a constant in Rust: the owner
/// publishes these numbers and they have already moved once. A missing row
/// is not "no discount" — it means the discount is unknown, and D11 forbids
/// computing a client price from an unknown (see `db::bikes`).
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "class_discounts")]
pub struct Model {
    /// TEXT PRIMARY KEY with CHECK (class IN ('scooter','motorcycle')),
    /// mirroring the CHECK on `bikes.class` so the two tables cannot
    /// disagree about which classes exist.
    #[sea_orm(primary_key, auto_increment = false)]
    pub class: String,

    // ── Published discount (079_rental_terms.sql) ────────────────────
    /// Fraction of the base rate, not a percentage: 0.25 means -25%.
    /// CHECK (discount >= 0 AND discount < 1) — 1.0 is unstorable, which
    /// is also what keeps a discounted rate from ever reaching 0.
    pub discount: f64,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
