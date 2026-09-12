/// SeaORM entity for the `bikes` table — one row per product FAMILY, the
/// thing a customer actually chooses. The physical machines live in
/// `bike_units`; a family with ten units is still one row here.
///
/// Schema assembled from:
///   077_bikes.sql — every column below lands together (DECISIONS.md D1:
///        bike migrations start at 077 and stay contiguous; D2: 001-076
///        are never edited in place)
///
/// Money columns are NULLABLE on purpose (D9). An unpublished number stays
/// absent all the way to the client, which renders a dash — so there is no
/// `NOT NULL DEFAULT 0` here, and nothing in this table may be read through
/// `try_get_warn!` (fail-open: a renamed column would serve `0` to a
/// customer with only a `tracing::warn` as evidence).
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "bikes")]
pub struct Model {
    /// VARCHAR(36) PRIMARY KEY DEFAULT gen_random_uuid()::text
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    // ── Identity (077_bikes.sql) ─────────────────────────────────────
    /// UNIQUE NOT NULL family key: `nmax-155`, `xmax-300-new`. This is the
    /// value a cart rental line carries as `catalog_id` (D8).
    pub key: String,
    pub brand: String,
    pub model: String,
    /// `NEW 2023+` / `pre-2023`. NULL when the family has one variant.
    /// `xmax-300` and `xmax-300-new` are two families, not two trims: the
    /// tariff and the deposit differ.
    pub variant_label: Option<String>,

    // ── Classification (077_bikes.sql) ───────────────────────────────
    /// CHECK (class IN ('scooter','motorcycle')) — the key into
    /// `class_discounts`. X-ADV 750 is classed with the scooters because
    /// the published tariff classes it there, engine size notwithstanding.
    pub class: String,
    pub body: String,
    pub displacement_cc: i32,

    // ── Published money, all NULLABLE (077_bikes.sql, D9/D11) ────────
    /// PRE class-discount published tariff, THB/day. Rendering it as the
    /// client price without applying the class discount overstates every
    /// scooter by 33% and every motorcycle by 18%.
    pub base_rate_thb_day: Option<f64>,
    pub deposit_thb: Option<f64>,
    pub monthly_low_season_thb: Option<f64>,
    /// Asking price for a family offered for sale. Never derived from the
    /// internal per-unit purchase cost (D14).
    pub sale_price_thb: Option<f64>,

    // ── Catalog presentation (077_bikes.sql) ─────────────────────────
    /// FALSE closes a family to NEW rentals without cancelling a contract
    /// still running on one of its units — `click-125` today (D12).
    pub offered: bool,
    pub description_ru: Option<String>,
    pub description_en: Option<String>,
    pub image_url: Option<String>,
    pub sort_order: i32,

    // ── Timestamps (077_bikes.sql) ───────────────────────────────────
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
