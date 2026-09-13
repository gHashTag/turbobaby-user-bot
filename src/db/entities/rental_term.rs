/// SeaORM entity for the `rental_terms` table — the published term-discount
/// BANDS: `week` 6-15%, `two_weeks` 15-25%, `month` 35-50%.
///
/// Schema assembled from:
///   079_rental_terms.sql — every column below lands together, seeded
///        in the same file (DECISIONS.md D1/D2)
///
/// A band is a RANGE, not a multiplier. The source publishes it as a range
/// and an exact quote comes from the door, so the two bounds are stored
/// separately and neither is averaged into a single number (D11).
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "rental_terms")]
pub struct Model {
    /// VARCHAR(36) PRIMARY KEY — no column DEFAULT: the three bands are
    /// seeded in the same migration and supply their own
    /// `gen_random_uuid()::text`.
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    // ── Band identity (079_rental_terms.sql) ─────────────────────────
    /// UNIQUE NOT NULL — `week` | `two_weeks` | `month`.
    pub band: String,

    // ── Day range (079_rental_terms.sql) ─────────────────────────────
    pub min_days: i32,
    /// NULL means open-ended: the `month` band has no upper bound, and the
    /// observed terms run 7, 30, 90, 120, 150 and 180 days.
    pub max_days: Option<i32>,

    // ── Published bounds (079_rental_terms.sql) ──────────────────────
    /// Fractions, not percentages: the `week` band is 0.06 to 0.15.
    pub discount_min: f64,
    pub discount_max: f64,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
