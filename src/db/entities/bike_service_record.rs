/// SeaORM entity for the `bike_service_records` table — oil, gear, ABS and
/// air-filter service state per PHYSICAL unit. Replaces `lab_certificates`
/// (DECISIONS.md D6), which is the one cannabis table with a real
/// motorbike counterpart rather than a forced analogy.
///
/// Schema assembled from:
///   080_bike_service_records.sql — every column below lands together;
///        the table ships EMPTY, there is no seed (DECISIONS.md D1/D2)
///
/// **ADMIN ONLY.** Service state is never surfaced in the public catalog:
/// units read `overdue` in the ops sheet today, and a public "serviced"
/// badge the shop cannot keep accurate is exactly the kind of number the
/// data-honesty rule forbids. The only reader is the admin path in
/// `db::bikes::list_service_records_for_unit`.
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "bike_service_records")]
pub struct Model {
    /// VARCHAR(36) PRIMARY KEY — no column DEFAULT: rows arrive from the
    /// admin surface, which supplies the id.
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    // ── Unit link (080_bike_service_records.sql) ─────────────────────
    /// REFERENCES bike_units(id) ON DELETE CASCADE, joined by hand.
    pub bike_unit_id: String,

    // ── Service line (080_bike_service_records.sql) ──────────────────
    /// `oil`, `gear_oil`, `abs`, `air_filter` — the ops sheet's own
    /// vocabulary, kept as TEXT because it grows without a migration.
    pub service_type: String,
    /// Kilometres since purchase at the time of the reading — NOT an
    /// odometer value (see `bike_unit::Model::km_since_purchase`).
    pub current_km: Option<i32>,
    pub last_service_km: Option<i32>,
    pub interval_km: Option<i32>,
    pub next_km: Option<i32>,
    /// The ops sheet's verdict for this line, e.g. `ok` / `overdue`.
    pub status: String,

    // ── Timestamp (080_bike_service_records.sql) ─────────────────────
    pub recorded_at: DateTimeWithTimeZone,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
