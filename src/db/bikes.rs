//! The bike catalog — wire shapes for the rental/sales catalog plus the
//! queries that read them.
//!
//! Schema: `migrations/077_bikes.sql` (families),
//! `078_bike_units.sql` (physical machines), `079_rental_terms.sql`
//! (`class_discounts` + `rental_terms`), `080_bike_service_records.sql`
//! (admin only). Every table is reached through its SeaORM entity under
//! `db::entities`.
//!
//! Replaces `db/strains.rs` (the cannabis catalog, whose table
//! `083_drop_cannabis_catalog.sql` removes), and two of that file's
//! constructs deliberately did NOT come across:
//!
//!   * the `clamp` closure. It mapped NaN/Inf to `0.0` and was applied to
//!     four money fields; for a price, `0` reads to a customer as FREE.
//!     Every money field here is `Option<f64>` filtered with
//!     `.is_finite()`, so a number the source never published stays absent
//!     and the client renders a dash (D9).
//!   * `try_get_warn!`. It is fail-open by design — a renamed column
//!     yields the default with a `tracing::warn` as the only evidence.
//!     Every read below goes through a typed SeaORM entity, so a renamed
//!     column is a query error, never a zero on a price tag.
//!
//! Absence is carried, not translated: `None` serialises as an explicit
//! JSON `null` (never omitted, never `0`, never an average and never a
//! "from" price), which is what the client turns into a dash.
//!
//! Every rate in `bikes` is the PUBLISHED tariff and is PRE class-discount
//! (D11) — showing it raw overstates every scooter by 33% and every
//! motorcycle by 18%. [`apply_class_discount`] does that last step and
//! answers `None` whenever either input is missing.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ──────────────────────────────────────────────────────────────────
// Vocabulary and query bounds
// ──────────────────────────────────────────────────────────────────

/// The `bike_units.status` value for a machine at base and rentable.
/// Mirrors the table's CHECK (status IN
/// ('available','rented','service','retired')).
pub(crate) const UNIT_STATUS_AVAILABLE: &str = "available";

/// Runaway guard on a catalog read, not pagination: the stocked fleet is
/// 14 families. Hard-coded const — no client-supplied number reaches a
/// `LIMIT`.
const FAMILY_QUERY_LIMIT: u64 = 500;

/// Runaway guard on one unit's service history (admin path).
const SERVICE_RECORD_LIMIT: u64 = 500;

// ──────────────────────────────────────────────────────────────────
// Wire shapes
// ──────────────────────────────────────────────────────────────────

/// A product family as the catalog serves it. One row of `bikes`; the
/// physical machines are counted separately (see [`BikeListing`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(unreachable_pub)] // Used as a field of pub structs in src/api/*.rs request/response bodies; pub(crate) would cascade.
pub struct Bike {
    pub id: String,
    /// Family key — `nmax-155`, `xmax-300-new`. This is what a cart rental
    /// line carries as `catalog_id` (D8).
    pub key: String,
    pub brand: String,
    pub model: String,
    /// `NEW 2023+` / `pre-2023`; `None` when the family has one variant.
    pub variant_label: Option<String>,
    /// `scooter` | `motorcycle` — the key into `class_discounts`.
    pub class: String,
    pub body: String,
    pub displacement_cc: i32,
    /// PRE class-discount published tariff, THB/day. `None` means the
    /// source publishes no rate for this family: render a dash and let a
    /// human quote it (D9/D11).
    pub base_rate_thb_day: Option<f64>,
    pub deposit_thb: Option<f64>,
    /// `None` for X-ADV 750 today — no monthly low-season price is
    /// published for it, and a dash is the honest rendering.
    pub monthly_low_season_thb: Option<f64>,
    /// Asking price when the family is offered for sale. Never derived
    /// from the internal per-unit purchase cost (D14).
    pub sale_price_thb: Option<f64>,
    /// `false` closes the family to NEW rentals — `click-125` today (D12).
    /// The contract still running on one of its units is unaffected.
    pub offered: bool,
    /// `true` puts the family on the forecourt. Orthogonal to
    /// `sale_price_thb`: a family can be for sale with no published asking
    /// price, which the detail screen renders as a dash plus the manager
    /// line rather than as a number nobody quoted (issue #11, D9/D11).
    pub for_sale: bool,
    pub description_ru: Option<String>,
    pub description_en: Option<String>,
    pub image_url: Option<String>,
    pub sort_order: i32,
}

/// A family plus the availability the catalog needs.
///
/// The count is a separate type from [`Bike`] on purpose: `From<Model>`
/// cannot know it, and a defaulted `0` would read as "none left" on a
/// family with ten units at base. Here `0` can only come from a `COUNT`
/// that actually returned zero.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(unreachable_pub)] // Used as a field of pub structs in src/api/*.rs request/response bodies; pub(crate) would cascade.
pub struct BikeListing {
    #[serde(flatten)]
    pub bike: Bike,
    /// Units of this family whose status is `available`.
    pub units_available: i64,
}

/// One published class discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(unreachable_pub)] // Used as a field of pub structs in src/api/*.rs request/response bodies; pub(crate) would cascade.
pub struct ClassDiscount {
    /// `scooter` | `motorcycle`, matching `bikes.class`.
    pub class: String,
    /// Fraction of the base rate — scooter 0.25, motorcycle 0.15. `None`
    /// when the stored value is not a usable fraction; the caller must then
    /// quote no number at all rather than fall back to "no discount".
    pub discount: Option<f64>,
}

/// One published term-discount band. A RANGE, never collapsed to a single
/// multiplier: the source publishes bounds and an exact quote comes from
/// the door (D11).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(unreachable_pub)] // Used as a field of pub structs in src/api/*.rs request/response bodies; pub(crate) would cascade.
pub struct RentalTermBand {
    /// `week` | `two_weeks` | `month`.
    pub band: String,
    pub min_days: i32,
    /// `None` = open-ended upper bound (the `month` band).
    pub max_days: Option<i32>,
    /// Lower bound of the published band, as a fraction. `None` — together
    /// with `discount_max` — when the stored pair is not usable: the day
    /// range is still shown, the discount is not shown wrong.
    pub discount_min: Option<f64>,
    pub discount_max: Option<f64>,
}

/// One service line for one physical unit.
///
/// **ADMIN ONLY (D6).** Never served from a catalog endpoint: units read
/// `overdue` in the ops sheet today, and a public badge the shop cannot
/// keep accurate is the kind of number the data-honesty rule forbids.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(unreachable_pub)] // Written for the admin endpoints of epic #31; no caller yet.
pub struct BikeServiceRecord {
    pub id: String,
    pub bike_unit_id: String,
    pub service_type: String,
    /// Kilometres since purchase at the time of the reading — NOT an
    /// odometer value. One unit reads 2900 since purchase against 16433 on
    /// the clock, so the two are not interchangeable.
    pub current_km: Option<i32>,
    pub last_service_km: Option<i32>,
    pub interval_km: Option<i32>,
    pub next_km: Option<i32>,
    pub status: String,
    /// RFC3339 string, as the rest of the wire surface does timestamps.
    pub recorded_at: String,
}

// ──────────────────────────────────────────────────────────────────
// Entity → wire conversions
// ──────────────────────────────────────────────────────────────────

/// The ONLY transformation applied to a money column is
/// `.filter(|v| v.is_finite())`. A NaN or Inf that a manual SQL fix left
/// behind is reported as absent — because it is absent. It is never
/// clamped to `0.0`: that is what `strains.rs` did, and a price of 0 reads
/// as FREE (D9).
impl From<crate::db::entities::bike::Model> for Bike {
    fn from(m: crate::db::entities::bike::Model) -> Self {
        Self {
            id: m.id,
            key: m.key,
            brand: m.brand,
            model: m.model,
            variant_label: m.variant_label,
            class: m.class,
            body: m.body,
            displacement_cc: m.displacement_cc,
            base_rate_thb_day: m.base_rate_thb_day.filter(|v| v.is_finite()),
            deposit_thb: m.deposit_thb.filter(|v| v.is_finite()),
            monthly_low_season_thb: m.monthly_low_season_thb.filter(|v| v.is_finite()),
            sale_price_thb: m.sale_price_thb.filter(|v| v.is_finite()),
            offered: m.offered,
            for_sale: m.for_sale,
            description_ru: m.description_ru,
            description_en: m.description_en,
            image_url: m.image_url,
            sort_order: m.sort_order,
        }
    }
}

impl From<crate::db::entities::class_discount::Model> for ClassDiscount {
    fn from(m: crate::db::entities::class_discount::Model) -> Self {
        Self {
            class: m.class,
            discount: usable_discount(m.discount),
        }
    }
}

impl From<crate::db::entities::rental_term::Model> for RentalTermBand {
    fn from(m: crate::db::entities::rental_term::Model) -> Self {
        // A band is a pair. If either bound is unusable, or they arrive
        // inverted, neither is published — half a band is worse than none,
        // because the client would print the half it got as the whole rule.
        let bounds = match (
            usable_discount(m.discount_min),
            usable_discount(m.discount_max),
        ) {
            (Some(lo), Some(hi)) if lo <= hi => (Some(lo), Some(hi)),
            _ => (None, None),
        };
        Self {
            band: m.band,
            min_days: m.min_days,
            max_days: m.max_days,
            discount_min: bounds.0,
            discount_max: bounds.1,
        }
    }
}

impl From<crate::db::entities::bike_service_record::Model> for BikeServiceRecord {
    fn from(m: crate::db::entities::bike_service_record::Model) -> Self {
        Self {
            id: m.id,
            bike_unit_id: m.bike_unit_id,
            service_type: m.service_type,
            current_km: m.current_km,
            last_service_km: m.last_service_km,
            interval_km: m.interval_km,
            next_km: m.next_km,
            status: m.status,
            recorded_at: m.recorded_at.to_rfc3339(),
        }
    }
}

// ──────────────────────────────────────────────────────────────────
// Pure price helpers
// ──────────────────────────────────────────────────────────────────

/// A discount may touch a price only if it is a finite fraction in
/// `0.0..1.0` — the exact range the schema enforces
/// (`migrations/079_rental_terms.sql`: `CHECK (discount >= 0 AND
/// discount < 1)`). Anything else — NaN from a manual UPDATE, `1.5`,
/// `-0.2` — would turn a real tariff into a nonsense number, so it is
/// reported as absent and the price question goes to a human (D11).
///
/// `1.0` is excluded deliberately, and not just to mirror the CHECK: a
/// discount of 1 produces a rate of `0.0`, which is the one number this
/// module exists to keep off a price tag.
fn usable_discount(v: f64) -> Option<f64> {
    if v.is_finite() && (0.0..1.0).contains(&v) {
        Some(v)
    } else {
        None
    }
}

/// The discount published for `class`, or `None` when the class has no row
/// or its row holds an unusable number.
///
/// `None` is the caller's cue that no price may be computed — not a cue to
/// fall back to "no discount", which would quote the pre-discount tariff.
#[allow(unreachable_pub)] // Read by the catalog/pricing paths in src/api/*.rs and by integration tests as lib consumers.
pub fn discount_for_class(discounts: &[ClassDiscount], class: &str) -> Option<f64> {
    discounts
        .iter()
        .find(|d| d.class == class)
        .and_then(|d| d.discount)
}

/// Apply a published class discount to a PRE-discount base rate.
///
/// `None` in, `None` out — and never `0.0`:
///   * no published rate → `None` (the family has no tariff to discount)
///   * no usable class discount → `None` (quoting the pre-discount number
///     overstates a scooter by 33%; D11 forbids inventing the rest)
///   * a non-finite product → `None`
///
/// The arithmetic is `base * (1 - discount)`, which reproduces the owner's
/// own quote sheet on all six rows the seed reconciles — 939 → 704.25
/// against a quoted 704, 998 → 748.50 against 749, 690 → 517.50 against
/// 518. [`round_half_up_baht`] is the last step to the printed number.
#[allow(unreachable_pub)] // Read by the catalog/pricing paths in src/api/*.rs and by integration tests as lib consumers.
pub fn apply_class_discount(base_rate_thb_day: Option<f64>, discount: Option<f64>) -> Option<f64> {
    let base = base_rate_thb_day.filter(|v| v.is_finite())?;
    let discount = discount.and_then(usable_discount)?;
    let rate = base * (1.0 - discount);
    if rate.is_finite() {
        Some(rate)
    } else {
        None
    }
}

/// Round a THB figure to whole baht, half away from zero — the rule the
/// owner's quote sheet uses (704.25 → 704, 748.50 → 749, 517.50 → 518,
/// 592.50 → 593, 336.75 → 337). `None` stays `None`; a non-finite input is
/// absent, not zero.
#[allow(unreachable_pub)] // Read by the catalog/pricing paths in src/api/*.rs and by integration tests as lib consumers.
pub fn round_half_up_baht(thb: Option<f64>) -> Option<f64> {
    thb.filter(|v| v.is_finite()).map(|v| v.round())
}

// ──────────────────────────────────────────────────────────────────
// Queries
// ──────────────────────────────────────────────────────────────────

/// `available` unit counts per family, keyed by `bikes.id`.
///
/// One grouped query rather than one per family — the catalog list would
/// otherwise fire a round trip per card. A family with no `available` unit
/// is simply absent from the map, which callers read as 0.
async fn available_unit_counts(
    orm: &sea_orm::DatabaseConnection,
) -> Result<std::collections::HashMap<String, i64>> {
    use crate::db::entities::bike_unit::{Column as UnitCol, Entity as UnitEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
    let rows: Vec<(String, i64)> = UnitEntity::find()
        .select_only()
        .column(UnitCol::BikeId)
        .column_as(UnitCol::Id.count(), "units_available")
        .filter(UnitCol::Status.eq(UNIT_STATUS_AVAILABLE))
        .group_by(UnitCol::BikeId)
        .into_tuple()
        .all(orm)
        .await
        .context("available_unit_counts query")?;
    Ok(rows.into_iter().collect())
}

/// `available` unit count for one family.
async fn available_units_for_family(
    orm: &sea_orm::DatabaseConnection,
    bike_id: &str,
) -> Result<i64> {
    use crate::db::entities::bike_unit::{Column as UnitCol, Entity as UnitEntity};
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
    let n = UnitEntity::find()
        .filter(UnitCol::BikeId.eq(bike_id))
        .filter(UnitCol::Status.eq(UNIT_STATUS_AVAILABLE))
        .count(orm)
        .await
        .context("available_units_for_family count")?;
    // `count()` hands back u64; the whole fleet is 37 units, so the
    // conversion can only fail on a value that cannot exist. Saturating
    // keeps the arm honest without an unwrap.
    Ok(i64::try_from(n).unwrap_or(i64::MAX))
}

/// Every family the shop currently offers, in catalog order.
///
/// `available_only = true` keeps only families with at least one unit at
/// base — a filter on a real count, not on a flag someone has to remember
/// to flip.
///
/// Families with `offered = false` never appear here (D12 closes
/// `click-125` to new rentals). [`find_family_by_key`] still returns them
/// so a stale deep link can be answered honestly instead of 404-ing.
#[allow(unreachable_pub)] // Read by the catalog endpoints in src/api/*.rs and by integration tests as lib consumers.
pub async fn list_offered_families(
    orm: &sea_orm::DatabaseConnection,
    available_only: bool,
) -> Result<Vec<BikeListing>> {
    use crate::db::entities::bike::{Column as BikeCol, Entity as BikeEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
    let models = BikeEntity::find()
        .filter(BikeCol::Offered.eq(true))
        .order_by_asc(BikeCol::SortOrder)
        .order_by_asc(BikeCol::DisplacementCc)
        .order_by_asc(BikeCol::Key)
        .limit(FAMILY_QUERY_LIMIT)
        .all(orm)
        .await
        .context("list_offered_families query")?;

    let counts = available_unit_counts(orm).await?;
    let mut listings = Vec::with_capacity(models.len());
    for m in models {
        let units_available = counts.get(&m.id).copied().unwrap_or(0);
        if available_only && units_available == 0 {
            continue;
        }
        listings.push(BikeListing {
            bike: Bike::from(m),
            units_available,
        });
    }
    Ok(listings)
}

/// One family by its `key`, with its `available` unit count.
///
/// Returns unoffered families too — the caller reads `bike.offered` and
/// decides. For `click-125` that means saying it is not rented for now and
/// pointing at NMAX 155 (D12, since 2026-09-24), which is more use to a
/// customer than a 404.
#[allow(unreachable_pub)] // Read by the catalog endpoints in src/api/*.rs and by integration tests as lib consumers.
pub async fn find_family_by_key(
    orm: &sea_orm::DatabaseConnection,
    key: &str,
) -> Result<Option<BikeListing>> {
    use crate::db::entities::bike::{Column as BikeCol, Entity as BikeEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let Some(m) = BikeEntity::find()
        .filter(BikeCol::Key.eq(key))
        .one(orm)
        .await
        .context("find_family_by_key query")?
    else {
        return Ok(None);
    };
    let units_available = available_units_for_family(orm, &m.id).await?;
    Ok(Some(BikeListing {
        bike: Bike::from(m),
        units_available,
    }))
}

/// Every family, offered or not, with its `available` unit count.
///
/// **ADMIN ONLY.** The public catalog must keep using
/// [`list_offered_families`], which has exactly one rule — `offered = true`
/// is what customers see (D12). This is a second function rather than a flag
/// on that one so the customer-facing rule has no parameter that can be got
/// wrong: there is no request an anonymous caller can shape that reaches an
/// unoffered family through the public list.
///
/// The admin needs the opposite view, because `click-125` is invisible on the
/// public catalog and re-opening it from a screen that cannot show it is not
/// possible.
#[allow(unreachable_pub)] // Read by the admin endpoints in src/api/bikes.rs and by integration tests as lib consumers.
pub async fn list_all_families(orm: &sea_orm::DatabaseConnection) -> Result<Vec<BikeListing>> {
    use crate::db::entities::bike::{Column as BikeCol, Entity as BikeEntity};
    use sea_orm::{EntityTrait, QueryOrder, QuerySelect};
    let models = BikeEntity::find()
        .order_by_asc(BikeCol::SortOrder)
        .order_by_asc(BikeCol::DisplacementCc)
        .order_by_asc(BikeCol::Key)
        .limit(FAMILY_QUERY_LIMIT)
        .all(orm)
        .await
        .context("list_all_families query")?;
    let counts = available_unit_counts(orm).await?;
    Ok(models
        .into_iter()
        .map(|m| {
            let units_available = counts.get(&m.id).copied().unwrap_or(0);
            BikeListing {
                bike: Bike::from(m),
                units_available,
            }
        })
        .collect())
}

/// How many physical units belong to a family, whatever their status.
///
/// Distinct from [`available_units_for_family`] on purpose: this one counts
/// machines that exist, not machines that can be rented today. Deleting a
/// family cascades to `bike_units` (`078_bike_units.sql`), so the delete path
/// asks this question rather than the availability one — a family whose units
/// are all out on rent has zero available and everything to lose.
#[allow(unreachable_pub)] // Read by the admin endpoints in src/api/bikes.rs and by integration tests as lib consumers.
pub async fn count_units_for_family(
    orm: &sea_orm::DatabaseConnection,
    bike_id: &str,
) -> Result<u64> {
    use crate::db::entities::bike_unit::{Column as UnitCol, Entity as UnitEntity};
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
    UnitEntity::find()
        .filter(UnitCol::BikeId.eq(bike_id))
        .count(orm)
        .await
        .context("count_units_for_family query")
}

/// The published class discounts, one row per class.
///
/// An empty result is not "no discounts" — it is an unseeded table, and a
/// caller that finds its class missing must not compute a price.
#[allow(unreachable_pub)] // Read by the catalog/pricing paths in src/api/*.rs and by integration tests as lib consumers.
pub async fn list_class_discounts(orm: &sea_orm::DatabaseConnection) -> Result<Vec<ClassDiscount>> {
    use crate::db::entities::class_discount::{Column as DiscountCol, Entity as DiscountEntity};
    use sea_orm::{EntityTrait, QueryOrder};
    let models = DiscountEntity::find()
        .order_by_asc(DiscountCol::Class)
        .all(orm)
        .await
        .context("list_class_discounts query")?;
    Ok(models.into_iter().map(ClassDiscount::from).collect())
}

/// The published term-discount bands, shortest term first.
#[allow(unreachable_pub)] // Read by the catalog/pricing paths in src/api/*.rs and by integration tests as lib consumers.
pub async fn list_rental_term_bands(
    orm: &sea_orm::DatabaseConnection,
) -> Result<Vec<RentalTermBand>> {
    use crate::db::entities::rental_term::{Column as TermCol, Entity as TermEntity};
    use sea_orm::{EntityTrait, QueryOrder};
    let models = TermEntity::find()
        .order_by_asc(TermCol::MinDays)
        .all(orm)
        .await
        .context("list_rental_term_bands query")?;
    Ok(models.into_iter().map(RentalTermBand::from).collect())
}

/// Service history for one physical unit, newest first.
///
/// **ADMIN ONLY (D6)** — no catalog endpoint may call this. The caller is
/// responsible for the admin gate; this function does not check it.
#[allow(unreachable_pub)] // Written for the admin endpoints of epic #31; no caller yet.
#[expect(dead_code)] // Fires the build when the fleet-admin epic wires a caller, forcing this attribute off.
pub async fn list_service_records_for_unit(
    orm: &sea_orm::DatabaseConnection,
    bike_unit_id: &str,
) -> Result<Vec<BikeServiceRecord>> {
    use crate::db::entities::bike_service_record::{Column as ServiceCol, Entity as ServiceEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
    let models = ServiceEntity::find()
        .filter(ServiceCol::BikeUnitId.eq(bike_unit_id))
        .order_by_desc(ServiceCol::RecordedAt)
        .limit(SERVICE_RECORD_LIMIT)
        .all(orm)
        .await
        .context("list_service_records_for_unit query")?;
    Ok(models.into_iter().map(BikeServiceRecord::from).collect())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_class_discount, discount_for_class, round_half_up_baht, Bike, BikeListing,
        ClassDiscount, RentalTermBand,
    };

    fn ts() -> sea_orm::prelude::DateTimeWithTimeZone {
        chrono::Utc::now().into()
    }

    fn bike_model(
        base: Option<f64>,
        deposit: Option<f64>,
        monthly: Option<f64>,
        sale: Option<f64>,
    ) -> crate::db::entities::bike::Model {
        crate::db::entities::bike::Model {
            id: "uuid-1".into(),
            key: "nmax-155".into(),
            brand: "Yamaha".into(),
            model: "NMAX 155".into(),
            variant_label: None,
            class: "scooter".into(),
            body: "scooter".into(),
            displacement_cc: 155,
            base_rate_thb_day: base,
            deposit_thb: deposit,
            monthly_low_season_thb: monthly,
            sale_price_thb: sale,
            offered: true,
            for_sale: false,
            description_ru: None,
            description_en: None,
            image_url: None,
            sort_order: 0,
            created_at: ts(),
            updated_at: ts(),
        }
    }

    #[test]
    fn test_bike_serde_roundtrip() {
        let b = Bike::from(bike_model(Some(449.0), Some(3000.0), Some(5000.0), None));
        let json = serde_json::to_value(&b).unwrap();
        let back: Bike = serde_json::from_value(json).unwrap();
        assert_eq!(back.key, "nmax-155");
        assert_eq!(back.base_rate_thb_day, Some(449.0));
        assert_eq!(back.sale_price_thb, None);
    }

    /// The defect D9 exists to prevent: an absent price must reach the
    /// client as `null`, never as `0`, and the field must be present so the
    /// client knows to draw a dash.
    #[test]
    fn test_absent_money_serialises_as_explicit_null() {
        let b = Bike::from(bike_model(None, None, None, None));
        let json = serde_json::to_value(&b).unwrap();
        for field in [
            "base_rate_thb_day",
            "deposit_thb",
            "monthly_low_season_thb",
            "sale_price_thb",
        ] {
            assert_eq!(
                json.get(field),
                Some(&serde_json::Value::Null),
                "{field} must serialise as an explicit null"
            );
        }
    }

    /// `strains.rs` clamped NaN/Inf to 0.0 on four money fields. Absent
    /// must stay absent instead.
    #[test]
    fn test_non_finite_money_becomes_absent_not_zero() {
        let b = Bike::from(bike_model(
            Some(f64::NAN),
            Some(f64::INFINITY),
            Some(f64::NEG_INFINITY),
            Some(f64::NAN),
        ));
        assert_eq!(b.base_rate_thb_day, None);
        assert_eq!(b.deposit_thb, None);
        assert_eq!(b.monthly_low_season_thb, None);
        assert_eq!(b.sale_price_thb, None);
    }

    #[test]
    fn test_listing_flattens_family_and_carries_count() {
        let listing = BikeListing {
            bike: Bike::from(bike_model(Some(449.0), None, None, None)),
            units_available: 5,
        };
        let json = serde_json::to_value(&listing).unwrap();
        assert_eq!(json.get("key").and_then(|v| v.as_str()), Some("nmax-155"));
        assert_eq!(
            json.get("units_available").and_then(|v| v.as_i64()),
            Some(5)
        );
    }

    /// All six rows of `data/fleet_seed.json` → `reconciliation.checks`:
    /// base × 0.75 equals the owner's own per-day grid figure once rounded
    /// half-up. This is the test that says the arithmetic matches the door.
    #[test]
    fn test_class_discount_reproduces_owner_grid() {
        let scooter = Some(0.25);
        let cases = [
            (939.0, 704.25, 704.0),
            (998.0, 748.50, 749.0),
            (690.0, 517.50, 518.0),
            (790.0, 592.50, 593.0),
            (449.0, 336.75, 337.0),
            (2788.0, 2091.00, 2091.0),
        ];
        for (base, exact, printed) in cases {
            let rate = apply_class_discount(Some(base), scooter);
            assert_eq!(rate, Some(exact), "base {base} discounted");
            assert_eq!(
                round_half_up_baht(rate),
                Some(printed),
                "base {base} printed"
            );
        }
    }

    #[test]
    fn test_class_discount_motorcycle_band() {
        // XSR 155: 590 pre-discount, motorcycle -15%.
        assert_eq!(apply_class_discount(Some(590.0), Some(0.15)), Some(501.5));
    }

    /// None in, None out — the whole point of D9. No branch may produce 0.
    #[test]
    fn test_apply_class_discount_absence_propagates() {
        assert_eq!(apply_class_discount(None, Some(0.25)), None);
        assert_eq!(apply_class_discount(Some(449.0), None), None);
        assert_eq!(apply_class_discount(None, None), None);
        assert_eq!(apply_class_discount(Some(f64::NAN), Some(0.25)), None);
        assert_eq!(apply_class_discount(Some(f64::INFINITY), Some(0.25)), None);
        assert_eq!(apply_class_discount(Some(449.0), Some(f64::NAN)), None);
    }

    /// A discount outside the schema's `[0, 1)` would invert, inflate or
    /// zero the price. Refuse to compute rather than publish the nonsense.
    #[test]
    fn test_out_of_range_discount_refuses_to_compute() {
        assert_eq!(apply_class_discount(Some(449.0), Some(1.5)), None);
        assert_eq!(apply_class_discount(Some(449.0), Some(-0.2)), None);
        // 1.0 is rejected: it is outside the schema's CHECK, and it is the
        // one input that would make this function return a rate of 0.0.
        assert_eq!(apply_class_discount(Some(449.0), Some(1.0)), None);
        // 0.0 is legal — "the class is published with no discount" — and
        // must pass the tariff through untouched rather than blank it.
        assert_eq!(apply_class_discount(Some(449.0), Some(0.0)), Some(449.0));
    }

    #[test]
    fn test_discount_for_class_missing_row_is_none() {
        let discounts = vec![
            ClassDiscount {
                class: "scooter".into(),
                discount: Some(0.25),
            },
            ClassDiscount {
                class: "motorcycle".into(),
                discount: None,
            },
        ];
        assert_eq!(discount_for_class(&discounts, "scooter"), Some(0.25));
        // Row present but unusable, and row absent entirely: both are
        // "unknown", and both must stop a computed price.
        assert_eq!(discount_for_class(&discounts, "motorcycle"), None);
        assert_eq!(discount_for_class(&discounts, "quadbike"), None);
        assert_eq!(discount_for_class(&[], "scooter"), None);
    }

    #[test]
    fn test_round_half_up_baht_keeps_absence() {
        assert_eq!(round_half_up_baht(None), None);
        assert_eq!(round_half_up_baht(Some(f64::NAN)), None);
        assert_eq!(round_half_up_baht(Some(0.5)), Some(1.0));
    }

    fn term_model(band: &str, lo: f64, hi: f64) -> crate::db::entities::rental_term::Model {
        crate::db::entities::rental_term::Model {
            id: "uuid-term".into(),
            band: band.into(),
            min_days: 7,
            max_days: Some(13),
            discount_min: lo,
            discount_max: hi,
        }
    }

    #[test]
    fn test_rental_term_band_published_bounds() {
        let b = RentalTermBand::from(term_model("week", 0.06, 0.15));
        assert_eq!(b.band, "week");
        assert_eq!(b.discount_min, Some(0.06));
        assert_eq!(b.discount_max, Some(0.15));
        assert_eq!(b.max_days, Some(13));
    }

    /// Half a band is worse than none: the client would print the half it
    /// received as the whole rule.
    #[test]
    fn test_rental_term_band_drops_both_bounds_when_one_is_unusable() {
        let b = RentalTermBand::from(term_model("month", 0.35, f64::NAN));
        assert_eq!(b.discount_min, None);
        assert_eq!(b.discount_max, None);
        // The day range survives — it is not a money field.
        assert_eq!(b.min_days, 7);

        let inverted = RentalTermBand::from(term_model("two_weeks", 0.25, 0.15));
        assert_eq!(inverted.discount_min, None);
        assert_eq!(inverted.discount_max, None);
    }

    #[test]
    fn test_class_discount_from_model_filters_nonsense() {
        let sane = ClassDiscount::from(crate::db::entities::class_discount::Model {
            class: "scooter".into(),
            discount: 0.25,
        });
        assert_eq!(sane.discount, Some(0.25));
        let broken = ClassDiscount::from(crate::db::entities::class_discount::Model {
            class: "motorcycle".into(),
            discount: f64::NAN,
        });
        assert_eq!(broken.discount, None);
    }
}
