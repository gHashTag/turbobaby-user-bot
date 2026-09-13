//! TurboBaby catalog API — bike families, their unit availability, and the
//! published rental terms. Issues #12 / #13.
//!
//! | route | serves |
//! | --- | --- |
//! | `GET /api/bikes` | offered families; `?available_only=true` narrows to families with at least one `available` unit |
//! | `GET /api/bikes/:key` | one family plus its unit rollup, recorded colours and model years |
//! | `GET /api/rental-terms` | published class discounts + term discount bands |
//!
//! Read-only. The admin write path is a separate issue, so this module never
//! invalidates an ETag key — [`crate::api::cache::ETagCache::has_changed`]
//! re-hashes the payload on every call, so a 304 is only ever served when the
//! bytes really are unchanged.
//!
//! ## One copy of every rule
//!
//! Every row here is read through [`crate::db::bikes`], never through SQL of
//! this module's own. That layer owns the queries, the `Option<f64>` money
//! mapping and the discount-usability test; this module owns the HTTP surface
//! — routes, query flags, caching, and the shape of the JSON. Re-implementing
//! either half here would give the catalog two copies of the same rule, free to
//! drift apart, which is the defect class D9 exists to prevent.
//!
//! The one query this module still issues itself is the per-family unit rollup
//! (colours, model years, counts by status), because `db::bikes` exposes an
//! `available` count and no unit list. It goes through the `bike_unit` SeaORM
//! entity, reading only the three columns the rollup may publish. See
//! [`load_units`]; a `db::bikes::list_units_for_family` is the better long-term
//! home for it.
//!
//! ## Money is nullable, and absent stays absent (DECISIONS.md D9)
//!
//! `base_rate_thb_day`, `deposit_thb`, `monthly_low_season_thb`,
//! `sale_price_thb` and `client_rate_thb_day` are read as `Option<f64>` and
//! serialised as **explicit JSON `null`** — the key is always present, never
//! omitted. `#[serde(skip_serializing_if)]` is deliberately used **nowhere** on
//! this path (nor on the structs in `db::bikes`): a missing key is what lets a
//! downstream `#[serde(default)]` turn an unknown price into `0.0`, which is
//! the exact defect D9 was written against. `class_discount`,
//! `discount_min`/`discount_max` and `max_days` follow the same rule.
//!
//! None of the three zero-manufacturing constructs D9 names is on this path:
//! no `clamp` closure, no `NOT NULL DEFAULT 0`, and no [`crate::try_get_warn!`]
//! — the latter is fail-open by design and would answer a customer with `0` on
//! a renamed column. The typed entity reads in `db::bikes` fail the query
//! instead, and this module maps that to a 500 rather than to a default.
//!
//! This module also does **not** re-filter the numbers `db::bikes` hands it.
//! The publishable range is asserted twice already — `CHECK (col IS NULL OR
//! col > 0)` in `migrations/077_bikes.sql`, and `.filter(|v| v.is_finite())` in
//! `Bike::from` — and a third private copy of the boundary here could only
//! disagree with them.
//!
//! ## The price door (DECISIONS.md D11) — a seam, not an implementation
//!
//! D11: the client-facing price is whatever the owner's live sheet (the "door")
//! returns, and **if the door is silent the bot must not compute**. Issue #7
//! owns the door. So every family here carries
//!
//! ```text
//! "client_rate_thb_day": null,
//! "client_rate_source":  "unavailable"
//! ```
//!
//! produced by [`client_rate`] — the one function #7 replaces. `null` +
//! `"unavailable"` is a first-class state, not an error: the UI must render
//! "a human quotes this price" and emit no number at all.
//!
//! `base_rate_thb_day` and `class_discount` are also served, because the admin
//! surface and #7's divergence log both need the file's number. They are the
//! **published pre-discount tariff**, not a client price: rendering
//! `base_rate_thb_day` to a customer overstates every scooter by 33%, and
//! multiplying it by `class_discount` is precisely the computation D11 forbids.
//! The only field a customer-facing price may come from is
//! `client_rate_thb_day`.
//!
//! That is why `db::bikes::apply_class_discount` and
//! `db::bikes::round_half_up_baht` are deliberately **not** called from this
//! module, even though they reproduce the owner's own quote sheet. They are the
//! door's arithmetic, to be applied to what the door returns — not a fallback
//! for its silence. Any number put in this payload will eventually be rendered,
//! so the payload carries no computed number at all.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::api::cache::make_etag_header;
use crate::db::bikes::{
    discount_for_class, find_family_by_key, list_class_discounts, list_offered_families,
    list_rental_term_bands, BikeListing, ClassDiscount, RentalTermBand, UNIT_STATUS_AVAILABLE,
};
use crate::AppState;

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/bikes", get(list_bikes))
        .route("/bikes/:key", get(get_bike))
        .route("/rental-terms", get(get_rental_terms))
}

// ── the door seam (D11) ──────────────────────────────────────────

/// `client_rate_source` when the number came from the owner's live sheet.
/// Nothing sets it yet — issue #7 owns the door. Declared here so the UI
/// worker can branch on the final string today instead of guessing it later.
#[allow(dead_code)]
const CLIENT_RATE_FROM_DOOR: &str = "door";

/// `client_rate_source` when there is no client-facing number to give.
/// Covers both "the door is not wired yet" (now) and "the door was asked and
/// stayed silent" (after #7) — from the UI's side those are the same state:
/// say a human quotes this price, emit no number.
const CLIENT_RATE_UNAVAILABLE: &str = "unavailable";

/// The client-facing per-day rate for one family, and where it came from.
///
/// **Issue #7 owns the implementation.** Today there is no door to ask, so
/// this always answers "no number at all", which is the same shape the silent
/// door must produce later. When #7 wires the live sheet it replaces the body
/// of this function and keeps the `None` branch intact:
///
/// * door answered → `(Some(rate), CLIENT_RATE_FROM_DOOR)`
/// * door silent, timed out (measured at 21.4 s — the common path, D11) or
///   errored → `(None, CLIENT_RATE_UNAVAILABLE)`
///
/// What it must never become is a fallback computation from
/// `base_rate_thb_day` and `class_discount`. That is why this returns an
/// `Option` rather than an `f64`, and why the caller passes the family key
/// (the door's lookup handle) rather than a price.
fn client_rate(_family_key: &str) -> (Option<f64>, &'static str) {
    (None, CLIENT_RATE_UNAVAILABLE)
}

// ── handlers ─────────────────────────────────────────────────────

/// `GET /api/bikes` — the offered families, in catalog order.
///
/// `?available_only=true` keeps only families with at least one unit in status
/// `available`. Note this is availability *now*, not availability for a date
/// range; a dated check belongs to the booking path, not the catalog.
///
/// Families with `offered = false` (CLICK 125, D12) are never listed —
/// `db::bikes::list_offered_families` filters them — but
/// `GET /api/bikes/:key` still serves them, so a saved link can be answered
/// with the redirect to PCX 150 / ADV 150 / NMAX 155 instead of a 404. There is
/// deliberately no `?include_unoffered` flag: it would fork that rule into two
/// places.
async fn list_bikes(
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<axum::response::Response, StatusCode> {
    let available_only = query_flag(&q, "available_only");

    // `{e:#}` rather than the `{e}` used elsewhere in `api::*`: `db::bikes`
    // attaches a `.context()` to every query, and plain Display prints only
    // that context — "list_offered_families query" — throwing away the DbErr
    // underneath it, which is the half that says what actually broke.
    let listings = list_offered_families(&state.db.orm, available_only)
        .await
        .map_err(|e| {
            tracing::error!("list_bikes: {e:#}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    // One read of the discount ladder for the whole page, not one per card.
    let discounts = list_class_discounts(&state.db.orm).await.map_err(|e| {
        tracing::error!("list_bikes(class_discounts): {e:#}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    tracing::debug!(
        "list_bikes: {} families (available_only={})",
        listings.len(),
        available_only
    );

    let families = listings
        .iter()
        .map(|l| family_json(l, &discounts))
        .collect::<Result<Vec<Value>, StatusCode>>()?;
    let body = json!({ "bikes": families }).to_string();

    // Separate keys: the filtered catalog must not flap the unfiltered one's
    // ETag, and vice versa.
    let cache_key = if available_only {
        "bikes_available"
    } else {
        "bikes"
    };
    let (changed, etag) = state.cache.has_changed(cache_key, &body).await;
    if !changed {
        if let Some(if_none_match) = headers.get("if-none-match") {
            if let Ok(if_none_match_str) = if_none_match.to_str() {
                if if_none_match_str == etag || if_none_match_str == format!("\"{}\"", etag) {
                    let mut response = axum::response::Response::new(axum::body::Body::empty());
                    *response.status_mut() = StatusCode::NOT_MODIFIED;
                    return Ok(response);
                }
            }
        }
    }

    let mut response = axum::response::Response::new(axum::body::Body::from(body));
    response
        .headers_mut()
        .insert("etag", make_etag_header(&etag));
    response.headers_mut().insert(
        "cache-control",
        HeaderValue::from_static("public, max-age=60"),
    );
    response
        .headers_mut()
        .insert("content-type", HeaderValue::from_static("application/json"));
    *response.status_mut() = StatusCode::OK;
    Ok(response)
}

/// `GET /api/bikes/:key` — one family by its `bikes.key` (`nmax-155`,
/// `xmax-300-new`), which is also the cart's `catalog_id` (D8).
///
/// Serves unoffered families too, carrying `offered: false`, so the UI has
/// something to render for a link to CLICK 125 instead of a bare 404.
///
/// Units are reported as aggregates only — counts by status, the colours and
/// model years on record. No `unit_code`, no `km_since_purchase`, and nothing
/// from `bike_service_records`: service state is admin-only (D6), and a public
/// badge we cannot keep accurate is the class of number D9 forbids.
async fn get_bike(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if key.is_empty() || key.len() > MAX_KEY_LEN {
        return Err(StatusCode::BAD_REQUEST);
    }
    let listing = find_family_by_key(&state.db.orm, &key)
        .await
        .map_err(|e| {
            tracing::error!("get_bike({key}): {e:#}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    let discounts = list_class_discounts(&state.db.orm).await.map_err(|e| {
        tracing::error!("get_bike({key}, class_discounts): {e:#}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let units = load_units(&state, &listing.bike.id).await?;

    let bike = family_detail_json(&listing, &discounts, &units)?;
    Ok(Json(json!({ "bike": bike })))
}

/// `GET /api/rental-terms` — the published class discounts and term bands.
///
/// `term_bands` are **ranges**, not multipliers: `discount_min` /
/// `discount_max` bracket what the shop publishes for a term, and the number a
/// customer actually pays comes from the door (D11). Nothing here may be
/// multiplied out into a quote.
async fn get_rental_terms(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let class_discounts = list_class_discounts(&state.db.orm).await.map_err(|e| {
        tracing::error!("get_rental_terms(class_discounts): {e:#}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let term_bands = list_rental_term_bands(&state.db.orm).await.map_err(|e| {
        tracing::error!("get_rental_terms(rental_terms): {e:#}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(rental_terms_json(&class_discounts, &term_bands)?))
}

// ── row → JSON ───────────────────────────────────────────────────

/// Longest `bikes.key` the router will look up. The real keys are
/// `xmax-300-new` and friends; the bound only stops a pathological path
/// segment reaching the database.
const MAX_KEY_LEN: usize = 200;

/// Serialise a family for the catalog, then attach the two things the wire
/// shape carries and the table does not: the class discount published for its
/// class, and the door's answer.
///
/// The serialisation goes through `BikeListing`'s own `Serialize`, so the money
/// fields arrive exactly as `db::bikes` mapped them — `None` as an explicit
/// `null`, never a missing key and never `0`.
fn family_json(listing: &BikeListing, discounts: &[ClassDiscount]) -> Result<Value, StatusCode> {
    let mut value = serde_json::to_value(listing).map_err(|e| {
        tracing::error!(
            "bikes API: family {} not serialisable: {e}",
            listing.bike.key
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let obj = value.as_object_mut().ok_or_else(|| {
        // Unreachable while BikeListing is a struct; a 500 rather than a panic
        // if it ever stops being one.
        tracing::error!(
            "bikes API: family {} did not serialise to an object",
            listing.bike.key
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // The published class discount (scooter 0.25, motorcycle 0.15). `None`
    // when the class has no row or its row holds an unusable number — and
    // `null`, not `0.0`, because "unpublished" is not "full price".
    obj.insert(
        "class_discount".to_string(),
        json!(discount_for_class(discounts, &listing.bike.class)),
    );

    // ── the door (D11). The only field a customer price may come from. ──
    let (client_rate_thb_day, client_rate_source) = client_rate(&listing.bike.key);
    obj.insert(
        "client_rate_thb_day".to_string(),
        json!(client_rate_thb_day),
    );
    obj.insert("client_rate_source".to_string(), json!(client_rate_source));

    Ok(value)
}

/// One family plus its unit rollup — the `GET /api/bikes/:key` body.
///
/// Pure, so the contract is testable without a database.
fn family_detail_json(
    listing: &BikeListing,
    discounts: &[ClassDiscount],
    units: &[UnitRow],
) -> Result<Value, StatusCode> {
    let mut value = family_json(listing, discounts)?;
    let rollup = rollup_units(units);
    let obj = value.as_object_mut().ok_or_else(|| {
        tracing::error!("bikes API: family detail did not serialise to an object");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    // `units_available` arrives from `db::bikes` as a grouped COUNT, and the
    // rollup counts the same units in a second query, so under a concurrent
    // status change the two can disagree by a unit. The rollup wins: one
    // response must not contain two different answers to "how many are
    // available". These five keys are detail-only — the list has no unit query.
    obj.insert("units_total".to_string(), json!(rollup.total));
    obj.insert("units_available".to_string(), json!(rollup.available));
    obj.insert("unit_status_counts".to_string(), json!(rollup.by_status));
    obj.insert("colors".to_string(), json!(rollup.colors));
    obj.insert(
        "colors_available".to_string(),
        json!(rollup.colors_available),
    );
    obj.insert("model_years".to_string(), json!(rollup.model_years));
    Ok(value)
}

/// The `GET /api/rental-terms` body.
///
/// `class_discounts` is a **list** of `{class, discount}`, not an object keyed
/// by class: an unseeded table is then an empty list rather than an object whose
/// missing key a client could read as zero. Both discount fields may be `null`.
fn rental_terms_json(
    class_discounts: &[ClassDiscount],
    term_bands: &[RentalTermBand],
) -> Result<Value, StatusCode> {
    let classes = serde_json::to_value(class_discounts).map_err(|e| {
        tracing::error!("bikes API: class discounts not serialisable: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let bands = serde_json::to_value(term_bands).map_err(|e| {
        tracing::error!("bikes API: rental term bands not serialisable: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(json!({
        "class_discounts": classes,
        "term_bands": bands,
    }))
}

// ── units ────────────────────────────────────────────────────────

/// One physical unit, reduced to the three fields the public catalog may show.
///
/// `unit_code` and `km_since_purchase` are on the entity and are deliberately
/// not carried here, and nothing in `bike_service_records` is read at all: D6
/// keeps service state admin-only, `unit_code` is a shop-internal slot label,
/// and `km_since_purchase` is kilometres since TurboBaby bought the bike — not
/// an odometer — so there is no honest public label for it. Narrowing the row
/// at the boundary means the rollup below cannot leak a field by accident.
#[derive(Debug, Clone, PartialEq, Eq)]
struct UnitRow {
    status: String,
    color: Option<String>,
    model_year: Option<i32>,
}

/// Runaway guard on one family's units, not pagination: the rollup is
/// order-insensitive and the largest family in the fleet holds ten units.
const UNIT_QUERY_LIMIT: u64 = 1000;

/// Load one family's units through the `bike_unit` entity.
///
/// `bike_id` comes from the family row this request already read, so the id
/// never originates with the client. Typed entity columns rather than SQL of
/// our own: a renamed column is then a compile error here and a query error at
/// worst, never a default served on a price tag.
async fn load_units(state: &AppState, bike_id: &str) -> Result<Vec<UnitRow>, StatusCode> {
    use crate::db::entities::bike_unit::{Column as UnitCol, Entity as UnitEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
    let models = UnitEntity::find()
        .filter(UnitCol::BikeId.eq(bike_id))
        .order_by_asc(UnitCol::UnitCode)
        .limit(UNIT_QUERY_LIMIT)
        .all(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("load_units({bike_id}): {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(models
        .into_iter()
        .map(|m| UnitRow {
            status: m.status,
            color: m.color,
            model_year: m.model_year,
        })
        .collect())
}

/// What the public catalog says about a family's physical units.
#[derive(Debug, Default, PartialEq, Eq)]
struct UnitRollup {
    /// Units that could be rented at all — everything except `retired`.
    total: i64,
    /// Units in status `available` right now.
    available: i64,
    /// Count per status, including the statuses that are at zero. A count of 0
    /// here is a measurement, not a missing value: the rows are the complete
    /// set, so "no units in service" is something we genuinely know.
    by_status: BTreeMap<String, i64>,
    colors: Vec<String>,
    colors_available: Vec<String>,
    model_years: Vec<i32>,
}

/// The other three values of `bike_units.status`, mirroring the CHECK in
/// `migrations/078_bike_units.sql`. `available` is not restated — it comes from
/// `db::bikes::UNIT_STATUS_AVAILABLE`, so the count this module publishes and
/// the count `db::bikes` queries cannot disagree about what "available" means.
const UNIT_STATUS_RENTED: &str = "rented";
const UNIT_STATUS_SERVICE: &str = "service";
const UNIT_STATUS_RETIRED: &str = "retired";

/// The four statuses, pre-seeded into `by_status` so the UI never has to read a
/// missing key as a zero.
const UNIT_STATUSES: [&str; 4] = [
    UNIT_STATUS_AVAILABLE,
    UNIT_STATUS_RENTED,
    UNIT_STATUS_SERVICE,
    UNIT_STATUS_RETIRED,
];

fn rollup_units(units: &[UnitRow]) -> UnitRollup {
    let mut by_status: BTreeMap<String, i64> = UNIT_STATUSES
        .iter()
        .map(|s| ((*s).to_string(), 0_i64))
        .collect();
    let mut colors: BTreeSet<String> = BTreeSet::new();
    let mut colors_available: BTreeSet<String> = BTreeSet::new();
    let mut model_years: BTreeSet<i32> = BTreeSet::new();
    let mut total = 0_i64;
    let mut available = 0_i64;

    for u in units {
        // A status outside UNIT_STATUSES cannot pass the CHECK constraint, but
        // if one ever does it gets its own bucket rather than being folded into
        // a known one — a surprising status must be visible, not absorbed.
        *by_status.entry(u.status.clone()).or_insert(0) += 1;
        let retired = u.status == UNIT_STATUS_RETIRED;
        let is_available = u.status == UNIT_STATUS_AVAILABLE;
        if !retired {
            total += 1;
        }
        if is_available {
            available += 1;
        }
        // An empty or whitespace colour is an unrecorded colour, not a colour
        // named "". It is dropped rather than rendered — these lists are what
        // is on record, and they do not claim to be exhaustive.
        if let Some(color) = u
            .color
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .map(str::to_string)
        {
            if is_available {
                colors_available.insert(color.clone());
            }
            if !retired {
                colors.insert(color);
            }
        }
        if let Some(year) = u.model_year {
            if !retired {
                model_years.insert(year);
            }
        }
    }

    UnitRollup {
        total,
        available,
        by_status,
        colors: colors.into_iter().collect(),
        colors_available: colors_available.into_iter().collect(),
        model_years: model_years.into_iter().collect(),
    }
}

/// Boolean query flag, matching the `include_hidden` convention `api::catalog`
/// uses (`1` / `true`), plus any casing of `true` because a Mini App query
/// string is hand-assembled in several places.
fn query_flag(q: &HashMap<String, String>, key: &str) -> bool {
    q.get(key)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{
        client_rate, family_detail_json, family_json, query_flag, rental_terms_json, rollup_units,
        UnitRow, CLIENT_RATE_UNAVAILABLE, UNIT_STATUSES,
    };
    use crate::db::bikes::{Bike, BikeListing, ClassDiscount, RentalTermBand};
    use serde_json::{json, Value};
    use std::collections::HashMap;

    /// Every money field the wire shape carries. The rule under test is that
    /// each one is always PRESENT and `null` when absent — a missing key is
    /// what lets a `#[serde(default)]` downstream invent `0.0` (D9).
    const MONEY_FIELDS: [&str; 5] = [
        "base_rate_thb_day",
        "deposit_thb",
        "monthly_low_season_thb",
        "sale_price_thb",
        "client_rate_thb_day",
    ];

    /// Shaped like the seed's `nmax-155` row, with the money fields under the
    /// test's control.
    fn listing(key: &str, class: &str, base_rate_thb_day: Option<f64>) -> BikeListing {
        BikeListing {
            bike: Bike {
                id: "b0000000-0000-4000-8000-000000000001".to_string(),
                key: key.to_string(),
                brand: "Yamaha".to_string(),
                model: "NMAX 155".to_string(),
                variant_label: None,
                class: class.to_string(),
                body: "scooter".to_string(),
                displacement_cc: 155,
                base_rate_thb_day,
                deposit_thb: None,
                monthly_low_season_thb: None,
                sale_price_thb: None,
                offered: true,
                description_ru: None,
                description_en: None,
                image_url: None,
                sort_order: 10,
            },
            units_available: 5,
        }
    }

    fn obj_of(value: &Value) -> &serde_json::Map<String, Value> {
        value.as_object().expect("family serialises to an object")
    }

    fn scooter_ladder() -> Vec<ClassDiscount> {
        vec![
            ClassDiscount {
                class: "motorcycle".to_string(),
                discount: Some(0.15),
            },
            ClassDiscount {
                class: "scooter".to_string(),
                discount: Some(0.25),
            },
        ]
    }

    // ── D9: absent stays absent, and nothing becomes 0 ───────────

    #[test]
    fn absent_money_is_an_explicit_null_not_a_missing_key() {
        let value = family_json(&listing("x-adv-750", "motorcycle", None), &[]).expect("json");
        let obj = obj_of(&value);
        for field in MONEY_FIELDS {
            assert!(
                obj.contains_key(field),
                "`{field}` must be present so nothing downstream can default it"
            );
            assert_eq!(
                obj.get(field),
                Some(&Value::Null),
                "`{field}` must be null, never 0"
            );
        }
    }

    #[test]
    fn a_published_tariff_is_served_verbatim() {
        // No rounding, no discount, no currency conversion on the way out.
        let value = family_json(&listing("nmax-155", "scooter", Some(449.0)), &[]).expect("json");
        assert_eq!(obj_of(&value).get("base_rate_thb_day"), Some(&json!(449.0)));
    }

    #[test]
    fn an_unpublished_class_discount_is_null_not_zero() {
        // The ladder is seeded for scooter and motorcycle only. A family whose
        // class has no row must not be told "no discount" — 0.0 would assert
        // full price, a claim there is no source for (D11).
        let value = family_json(
            &listing("x-adv-750", "atv", Some(2788.0)),
            &scooter_ladder(),
        )
        .expect("json");
        assert_eq!(obj_of(&value).get("class_discount"), Some(&Value::Null));
    }

    #[test]
    fn the_published_class_discount_is_attached_from_the_ladder() {
        let value = family_json(
            &listing("nmax-155", "scooter", Some(939.0)),
            &scooter_ladder(),
        )
        .expect("json");
        assert_eq!(obj_of(&value).get("class_discount"), Some(&json!(0.25)));
    }

    // ── D11: the door seam ───────────────────────────────────────

    #[test]
    fn client_rate_is_no_number_until_the_door_is_wired() {
        // #7 replaces the body of client_rate. Until then the client-facing
        // price must be absent, NOT base_rate * (1 - class_discount).
        let (rate, source) = client_rate("nmax-155");
        assert_eq!(rate, None);
        assert_eq!(source, CLIENT_RATE_UNAVAILABLE);
    }

    #[test]
    fn a_family_with_both_inputs_still_serves_no_computed_price() {
        // 939 with a 25% class discount is the seed's reconciled row: the
        // owner's sheet quotes 704 and the arithmetic gives 704.25. Both
        // numbers are computable from this payload and NEITHER may be in it —
        // `db::bikes::apply_class_discount` exists for the door's answer, not
        // for its silence.
        let value = family_json(
            &listing("nmax-155", "scooter", Some(939.0)),
            &scooter_ladder(),
        )
        .expect("json");
        let obj = obj_of(&value);
        assert_eq!(obj.get("client_rate_thb_day"), Some(&Value::Null));
        assert_eq!(
            obj.get("client_rate_source"),
            Some(&json!(CLIENT_RATE_UNAVAILABLE))
        );
        for (name, field) in obj {
            if let Some(n) = field.as_f64() {
                assert!(
                    (n - 704.25).abs() > 1e-9 && (n - 704.0).abs() > 1e-9,
                    "`{name}` carries a computed client price: {n}"
                );
            }
        }
    }

    // ── D6 / D14: what the public catalog may not carry ──────────

    #[test]
    fn no_unit_or_service_field_reaches_the_wire() {
        let units = vec![
            unit(UNIT_STATUSES[0], Some("black"), Some(2020)),
            unit(UNIT_STATUSES[1], Some("green"), Some(2021)),
        ];
        let value = family_detail_json(
            &listing("nmax-155", "scooter", Some(939.0)),
            &scooter_ladder(),
            &units,
        )
        .expect("json");
        let obj = obj_of(&value);
        for forbidden in [
            "unit_code",
            "km_since_purchase",
            "current_km",
            "last_service_km",
            "interval_km",
            "next_km",
            "units",
            "plate",
            "renter",
        ] {
            assert!(
                !obj.contains_key(forbidden),
                "`{forbidden}` must never be in a public catalog body"
            );
        }
    }

    #[test]
    fn the_detail_body_carries_the_rollup_and_agrees_with_itself() {
        let units = vec![
            unit(UNIT_STATUSES[0], Some("black"), Some(2020)),
            unit(UNIT_STATUSES[0], Some("blue"), Some(2020)),
            unit(UNIT_STATUSES[1], Some("green"), Some(2021)),
            unit(UNIT_STATUSES[3], Some("red"), Some(2014)),
        ];
        let value =
            family_detail_json(&listing("nmax-155", "scooter", None), &[], &units).expect("json");
        let obj = obj_of(&value);
        // The fixture's `units_available` is 5 and the rollup says 2. One body
        // must not hold two answers to the same question.
        assert_eq!(obj.get("units_available"), Some(&json!(2)));
        assert_eq!(obj.get("units_total"), Some(&json!(3)));
        assert_eq!(
            obj.get("colors"),
            Some(&json!(["black", "blue", "green"])),
            "a retired unit's colour is not part of the family"
        );
        assert_eq!(obj.get("colors_available"), Some(&json!(["black", "blue"])));
        assert_eq!(obj.get("model_years"), Some(&json!([2020, 2021])));
        let counts = obj
            .get("unit_status_counts")
            .and_then(Value::as_object)
            .expect("unit_status_counts is an object");
        assert_eq!(counts.get("available"), Some(&json!(2)));
        assert_eq!(counts.get("service"), Some(&json!(0)));
    }

    #[test]
    fn an_unoffered_family_is_still_serialisable() {
        // D12: click-125 is closed to new rentals and the detail route serves
        // it anyway, so the UI can redirect instead of 404-ing.
        let mut l = listing("click-125", "scooter", Some(300.0));
        l.bike.offered = false;
        let value = family_json(&l, &scooter_ladder()).expect("json");
        assert_eq!(obj_of(&value).get("offered"), Some(&json!(false)));
    }

    // ── rental terms ─────────────────────────────────────────────

    #[test]
    fn rental_terms_publishes_bands_as_ranges_with_nullable_bounds() {
        let bands = vec![
            RentalTermBand {
                band: "week".to_string(),
                min_days: 7,
                max_days: Some(13),
                discount_min: Some(0.06),
                discount_max: Some(0.15),
            },
            RentalTermBand {
                band: "month".to_string(),
                min_days: 30,
                max_days: None,
                discount_min: None,
                discount_max: None,
            },
        ];
        let value = rental_terms_json(&scooter_ladder(), &bands).expect("json");
        let terms = value
            .get("term_bands")
            .and_then(Value::as_array)
            .expect("term_bands is an array");
        assert_eq!(terms.len(), 2);
        // Open-ended top band: max_days is null, and the key is present.
        let month = obj_of(&terms[1]);
        assert_eq!(month.get("max_days"), Some(&Value::Null));
        assert_eq!(month.get("min_days"), Some(&json!(30)));
        assert_eq!(month.get("discount_min"), Some(&Value::Null));
        assert_eq!(month.get("discount_max"), Some(&Value::Null));
        let week = obj_of(&terms[0]);
        assert_eq!(week.get("discount_min"), Some(&json!(0.06)));
        assert_eq!(week.get("discount_max"), Some(&json!(0.15)));
    }

    #[test]
    fn rental_terms_serves_class_discounts_as_a_list() {
        let value = rental_terms_json(&scooter_ladder(), &[]).expect("json");
        assert_eq!(
            value.get("class_discounts"),
            Some(&json!([
                { "class": "motorcycle", "discount": 0.15 },
                { "class": "scooter", "discount": 0.25 },
            ]))
        );
        // An unseeded rental_terms table is an empty list, not a made-up band.
        assert_eq!(value.get("term_bands"), Some(&json!([])));
    }

    #[test]
    fn rental_terms_of_an_unseeded_database_invents_nothing() {
        let value = rental_terms_json(&[], &[]).expect("json");
        assert_eq!(value.get("class_discounts"), Some(&json!([])));
        assert_eq!(value.get("term_bands"), Some(&json!([])));
    }

    // ── query_flag ───────────────────────────────────────────────

    fn q(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn query_flag_accepts_one_and_true() {
        assert!(query_flag(&q(&[("available_only", "1")]), "available_only"));
        assert!(query_flag(
            &q(&[("available_only", "true")]),
            "available_only"
        ));
        assert!(query_flag(
            &q(&[("available_only", "TRUE")]),
            "available_only"
        ));
    }

    #[test]
    fn query_flag_rejects_everything_else() {
        assert!(!query_flag(&q(&[]), "available_only"));
        assert!(!query_flag(
            &q(&[("available_only", "0")]),
            "available_only"
        ));
        assert!(!query_flag(
            &q(&[("available_only", "false")]),
            "available_only"
        ));
        assert!(!query_flag(&q(&[("available_only", "")]), "available_only"));
        assert!(!query_flag(
            &q(&[("available_only", "yes")]),
            "available_only"
        ));
    }

    // ── rollup_units ─────────────────────────────────────────────

    fn unit(status: &str, color: Option<&str>, year: Option<i32>) -> UnitRow {
        UnitRow {
            status: status.to_string(),
            color: color.map(str::to_string),
            model_year: year,
        }
    }

    #[test]
    fn the_four_statuses_match_the_check_constraint() {
        // 078_bike_units.sql: CHECK (status IN
        // ('available','rented','service','retired')).
        assert_eq!(
            UNIT_STATUSES,
            ["available", "rented", "service", "retired"],
            "UNIT_STATUSES drifted from the CHECK constraint"
        );
    }

    #[test]
    fn rollup_of_no_units_is_zero_across_every_status() {
        let r = rollup_units(&[]);
        assert_eq!(r.total, 0);
        assert_eq!(r.available, 0);
        // A family with a published tariff and no stock is a real state
        // (the seed's `price_list_only` families), so it must roll up
        // cleanly rather than look like an error.
        assert_eq!(r.by_status.get("available"), Some(&0));
        assert_eq!(r.by_status.get("rented"), Some(&0));
        assert_eq!(r.by_status.get("service"), Some(&0));
        assert_eq!(r.by_status.get("retired"), Some(&0));
        assert!(r.colors.is_empty());
        assert!(r.model_years.is_empty());
    }

    #[test]
    fn rollup_counts_available_and_excludes_retired_from_total() {
        // Shaped like the seed's nmax-155 row: 10 units, 5 out on rental.
        let mut units = Vec::new();
        for _ in 0..5 {
            units.push(unit("available", Some("black"), Some(2020)));
        }
        for _ in 0..5 {
            units.push(unit("rented", Some("green"), Some(2021)));
        }
        units.push(unit("retired", Some("red"), Some(2015)));
        let r = rollup_units(&units);
        assert_eq!(r.available, 5);
        assert_eq!(r.total, 10, "a retired unit is not part of the fleet");
        assert_eq!(r.by_status.get("retired"), Some(&1));
        assert_eq!(r.colors, vec!["black".to_string(), "green".to_string()]);
        assert_eq!(r.colors_available, vec!["black".to_string()]);
        assert_eq!(r.model_years, vec![2020, 2021]);
    }

    #[test]
    fn rollup_service_units_are_counted_but_not_available() {
        let r = rollup_units(&[
            unit("service", Some("grey"), Some(2019)),
            unit("available", Some("blue"), Some(2023)),
        ]);
        assert_eq!(r.total, 2);
        assert_eq!(r.available, 1);
        assert_eq!(r.by_status.get("service"), Some(&1));
        // The colour of a bike in the workshop is still a colour this family
        // comes in — but it is not a colour available today.
        assert_eq!(r.colors, vec!["blue".to_string(), "grey".to_string()]);
        assert_eq!(r.colors_available, vec!["blue".to_string()]);
    }

    #[test]
    fn rollup_drops_blank_colours_and_keeps_them_deduped_and_sorted() {
        let r = rollup_units(&[
            unit("available", Some("  black  "), None),
            unit("available", Some("black"), None),
            unit("available", Some(""), None),
            unit("available", Some("   "), None),
            unit("available", None, None),
            unit("available", Some("blue"), None),
        ]);
        assert_eq!(r.available, 6);
        assert_eq!(r.colors, vec!["black".to_string(), "blue".to_string()]);
        assert!(
            r.model_years.is_empty(),
            "an unrecorded year is absent, not a year"
        );
    }

    #[test]
    fn rollup_gives_an_unexpected_status_its_own_bucket() {
        // The CHECK constraint forbids this; if it ever happens the unit must
        // stay visible instead of being folded into a known status.
        let r = rollup_units(&[unit("impounded", Some("black"), Some(2022))]);
        assert_eq!(r.by_status.get("impounded"), Some(&1));
        assert_eq!(r.total, 1);
        assert_eq!(r.available, 0);
        assert_eq!(r.colors_available, Vec::<String>::new());
    }

    #[test]
    fn rollup_retired_only_family_has_no_fleet() {
        let r = rollup_units(&[
            unit("retired", Some("white"), Some(2016)),
            unit("retired", Some("white"), Some(2017)),
        ]);
        assert_eq!(r.total, 0);
        assert_eq!(r.available, 0);
        assert_eq!(r.by_status.get("retired"), Some(&2));
        assert!(r.colors.is_empty());
        assert!(r.model_years.is_empty());
    }
}
