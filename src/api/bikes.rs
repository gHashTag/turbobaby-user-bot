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
//! ## Money is nullable, and absent stays absent (DECISIONS.md D9)
//!
//! `base_rate_thb_day`, `deposit_thb`, `monthly_low_season_thb` and
//! `sale_price_thb` are `DOUBLE PRECISION` **NULL**-able columns. They are read
//! as `Option<f64>` and serialised as **explicit JSON `null`** — the key is
//! always present, never omitted. `#[serde(skip_serializing_if)]` is
//! deliberately not used: a missing key is what lets a downstream
//! `#[serde(default)]` turn an unknown price into `0.0`, which is the exact
//! defect D9 was written against.
//!
//! None of the three zero-manufacturing constructs D9 names is on this path:
//! no `clamp` closure, no `NOT NULL DEFAULT 0`, and no [`crate::try_get_warn!`].
//! The reader helpers below are the deliberate opposite of `try_get_warn!` —
//! they are *fail-closed*: an unreadable column is a 500 with the column named
//! in the log, not a silent default served to a customer.
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

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::api::cache::make_etag_header;
use crate::AppState;
use sea_orm::{ConnectionTrait, DbBackend, QueryResult, Statement};

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

// ── SQL ──────────────────────────────────────────────────────────

/// Every column a family response is built from.
///
/// Every cast is deliberate, and every one of them preserves NULL — a cast can
/// narrow a type but it can never manufacture a value:
///
/// * `::float8` survives a money column typed `numeric` rather than
///   `double precision`;
/// * `::int4` survives `smallint` or `bigint`;
/// * `::text` on `id`, `class` and `body` survives `uuid` or a Postgres enum.
///
/// Without them a type the reader did not predict is a 500 on a live catalog,
/// which is the one failure mode a read-only endpoint has no excuse for.
///
/// `"key"` is quoted because `key` is a keyword in some dialects; the column
/// itself is the unquoted lowercase `key`.
const FAMILY_COLUMNS: &str = "b.id::text AS id, b.\"key\", b.brand, b.model, b.variant_label, \
     b.class::text AS class, b.body::text AS body, \
     b.displacement_cc::int4 AS displacement_cc, \
     b.base_rate_thb_day::float8 AS base_rate_thb_day, \
     b.deposit_thb::float8 AS deposit_thb, \
     b.monthly_low_season_thb::float8 AS monthly_low_season_thb, \
     b.sale_price_thb::float8 AS sale_price_thb, \
     b.offered, b.description_ru, b.description_en, b.image_url, \
     b.sort_order::int4 AS sort_order, \
     cd.discount::float8 AS class_discount, \
     COALESCE(u.units_total, 0) AS units_total, \
     COALESCE(u.units_available, 0) AS units_available";

/// The family FROM clause.
///
/// Both joins are LEFT joins because both right-hand sides are legitimately
/// absent: a class with no published discount row, and a family with no units.
/// A family with a published tariff and zero units is a real state — the seed's
/// `price_list_only` entries (PCX 150, ADV 150, …) are exactly that — so unit
/// counts are counted, never assumed.
///
/// `units_total` excludes `retired`: a retired unit cannot be rented and
/// counting it would overstate the fleet.
const FAMILY_FROM: &str = "FROM bikes b \
     LEFT JOIN class_discounts cd ON cd.class = b.class \
     LEFT JOIN ( \
         SELECT bike_id, \
                COUNT(*) FILTER (WHERE status <> 'retired')  AS units_total, \
                COUNT(*) FILTER (WHERE status = 'available') AS units_available \
         FROM bike_units GROUP BY bike_id \
     ) u ON u.bike_id = b.id";

/// Build the list query for the two boolean request flags.
///
/// Pure and string-assembled, which is safe here because both inputs are
/// `bool` — no request text ever reaches the SQL. The one caller that takes a
/// value from the client (`GET /api/bikes/:key`) uses a bound `$1` parameter
/// instead.
fn list_sql(include_unoffered: bool, available_only: bool) -> String {
    let mut sql = format!("SELECT {} {}", FAMILY_COLUMNS, FAMILY_FROM);
    let mut conditions: Vec<&str> = Vec::new();
    if !include_unoffered {
        conditions.push("b.offered = TRUE");
    }
    if available_only {
        conditions.push("COALESCE(u.units_available, 0) > 0");
    }
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    // Cheapest first within the shop's own ordering: `sort_order` is the
    // owner's hand-ranking, then displacement so a browse reads small-to-big,
    // then a stable tiebreak so two requests never disagree.
    sql.push_str(
        " ORDER BY b.sort_order ASC, b.displacement_cc ASC, b.brand ASC, b.model ASC, b.\"key\" ASC \
          LIMIT 2000",
    );
    sql
}

// ── handlers ─────────────────────────────────────────────────────

/// `GET /api/bikes` — the offered families.
///
/// * `?available_only=true` — only families with at least one unit in status
///   `available`. Note this is availability *now*, not availability for a date
///   range; a dated check belongs to the booking path, not the catalog.
/// * `?include_unoffered=true` — also families with `offered = FALSE`
///   (CLICK 125, D12). Deliberately **not** admin-gated: "we do not rent this
///   one now" is a public fact the FAQ already gives out, and
///   `GET /api/bikes/:key` serves the same row unauthenticated so the UI can
///   render the redirect to PCX 150 / ADV 150 / NMAX 155. It is a separate
///   ETag key so it cannot flap the default catalog's ETag.
async fn list_bikes(
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<axum::response::Response, StatusCode> {
    let available_only = query_flag(&q, "available_only");
    let include_unoffered = query_flag(&q, "include_unoffered");
    let sql = list_sql(include_unoffered, available_only);

    let rows = state
        .db
        .orm
        .query_all(Statement::from_string(DbBackend::Postgres, sql))
        .await
        .map_err(|e| {
            tracing::error!("list_bikes: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    tracing::debug!(
        "list_bikes: {} families (available_only={}, include_unoffered={})",
        rows.len(),
        available_only,
        include_unoffered
    );

    let families = rows
        .iter()
        .map(family_json)
        .collect::<Result<Vec<Value>, StatusCode>>()?;
    let body = json!({ "bikes": families }).to_string();

    let cache_key = match (include_unoffered, available_only) {
        (false, false) => "bikes",
        (false, true) => "bikes_available",
        (true, false) => "bikes_all",
        (true, true) => "bikes_all_available",
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
/// model years on record. No `unit_code`, and nothing from
/// `bike_service_records`: service state is admin-only (D6), and a public
/// badge we cannot keep accurate is the class of number D9 forbids.
async fn get_bike(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if key.is_empty() || key.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let sql = format!(
        "SELECT {} {} WHERE b.\"key\" = $1",
        FAMILY_COLUMNS, FAMILY_FROM
    );
    let row = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            &sql,
            [key.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("get_bike({key}): {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let Some(row) = row else {
        return Err(StatusCode::NOT_FOUND);
    };

    let mut family = family_json(&row)?;
    let rollup = rollup_units(&load_units(&state, &key).await?);

    if let Some(obj) = family.as_object_mut() {
        // The rollup and the aggregate join read `bike_units` in two separate
        // queries, so under a concurrent status change they can disagree by a
        // unit. The rollup wins here: one response must not contain two
        // different answers to "how many are available".
        obj.insert("units_total".to_string(), json!(rollup.total));
        obj.insert("units_available".to_string(), json!(rollup.available));
        obj.insert("unit_status_counts".to_string(), json!(rollup.by_status));
        obj.insert("colors".to_string(), json!(rollup.colors));
        obj.insert(
            "colors_available".to_string(),
            json!(rollup.colors_available),
        );
        obj.insert("model_years".to_string(), json!(rollup.model_years));
    }
    Ok(Json(json!({ "bike": family })))
}

/// `GET /api/rental-terms` — the published class discounts and term bands.
///
/// `term_bands` are **ranges**, not multipliers: `discount_min` / `discount_max`
/// bracket what the shop publishes for a term, and the number a customer
/// actually pays comes from the door (D11). Nothing here may be multiplied out
/// into a quote.
async fn get_rental_terms(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let class_rows = state
        .db
        .orm
        .query_all(Statement::from_string(
            DbBackend::Postgres,
            "SELECT class::text AS class, discount::float8 AS discount \
             FROM class_discounts ORDER BY class ASC"
                .to_string(),
        ))
        .await
        .map_err(|e| {
            tracing::error!("get_rental_terms(class_discounts): {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let mut class_discounts = serde_json::Map::new();
    for r in &class_rows {
        class_discounts.insert(text(r, "class")?, json!(fraction(r, "discount")?));
    }

    let band_rows = state
        .db
        .orm
        .query_all(Statement::from_string(
            DbBackend::Postgres,
            "SELECT band::text AS band, \
                    min_days::int4 AS min_days, \
                    max_days::int4 AS max_days, \
                    discount_min::float8 AS discount_min, \
                    discount_max::float8 AS discount_max \
             FROM rental_terms ORDER BY min_days ASC, band ASC"
                .to_string(),
        ))
        .await
        .map_err(|e| {
            tracing::error!("get_rental_terms(rental_terms): {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let mut term_bands: Vec<Value> = Vec::with_capacity(band_rows.len());
    for r in &band_rows {
        term_bands.push(json!({
            "band": text(r, "band")?,
            "min_days": int(r, "min_days")?,
            // Open-ended top band (`month` and up) has no max_days.
            "max_days": int_opt(r, "max_days")?,
            "discount_min": fraction(r, "discount_min")?,
            "discount_max": fraction(r, "discount_max")?,
        }));
    }

    Ok(Json(json!({
        "class_discounts": class_discounts,
        "term_bands": term_bands,
    })))
}

// ── row → JSON ───────────────────────────────────────────────────

fn family_json(r: &QueryResult) -> Result<Value, StatusCode> {
    let key = text(r, "key")?;
    let (client_rate_thb_day, client_rate_source) = client_rate(&key);
    Ok(json!({
        "id": text(r, "id")?,
        "key": key,
        "brand": text(r, "brand")?,
        "model": text(r, "model")?,
        // NULL for a family with a single generation ("NMAX 155"); set where
        // the shop sells two tariffs under one model name ("NEW 2023+").
        "variant_label": text_opt(r, "variant_label")?,
        "class": text(r, "class")?,
        "body": text(r, "body")?,
        "displacement_cc": int(r, "displacement_cc")?,
        // ── published tariff (PRE class-discount). NOT a client price. ──
        "base_rate_thb_day": money(r, "base_rate_thb_day")?,
        "class_discount": fraction(r, "class_discount")?,
        "deposit_thb": money(r, "deposit_thb")?,
        "monthly_low_season_thb": money(r, "monthly_low_season_thb")?,
        "sale_price_thb": money(r, "sale_price_thb")?,
        // ── the door (D11). The only field a customer price may come from. ──
        "client_rate_thb_day": client_rate_thb_day,
        "client_rate_source": client_rate_source,
        "offered": flag(r, "offered")?,
        "description_ru": text_opt(r, "description_ru")?,
        "description_en": text_opt(r, "description_en")?,
        "image_url": text_opt(r, "image_url")?,
        "sort_order": int(r, "sort_order")?,
        "units_total": count(r, "units_total")?,
        "units_available": count(r, "units_available")?,
    }))
}

/// One physical unit, reduced to the three fields the public catalog may show.
///
/// `unit_code`, `km_since_purchase` and everything in `bike_service_records`
/// are read by neither this struct nor its query: D6 keeps service state
/// admin-only, and `km_since_purchase` is kilometres since TurboBaby bought
/// the bike — not an odometer — so there is no honest public label for it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct UnitRow {
    status: String,
    color: Option<String>,
    model_year: Option<i32>,
}

/// Load one family's units, keyed on the family `key` rather than on the id
/// read back out of the family row: the subselect compares `bike_units.bike_id`
/// against `bikes.id` — the same column, so the same type, whatever that type
/// is — while the only bound parameter is the `key` text the router already
/// handed us.
///
/// The query reads only the three columns the rollup needs, so it depends on
/// no column this module does not already serve. `LIMIT` is a runaway guard,
/// not pagination: the rollup is order-insensitive and the largest family in
/// the fleet holds ten units.
async fn load_units(state: &AppState, family_key: &str) -> Result<Vec<UnitRow>, StatusCode> {
    let rows = state
        .db
        .orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT status::text AS status, color, model_year::int4 AS model_year \
             FROM bike_units \
             WHERE bike_id = (SELECT id FROM bikes WHERE \"key\" = $1) \
             ORDER BY status ASC LIMIT 1000",
            [family_key.to_string().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("load_units({family_key}): {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    rows.iter().map(unit_row).collect()
}

fn unit_row(r: &QueryResult) -> Result<UnitRow, StatusCode> {
    Ok(UnitRow {
        status: text(r, "status")?,
        color: text_opt(r, "color")?,
        model_year: int_opt(r, "model_year")?,
    })
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

/// The four statuses `bike_units.status` is constrained to. Pre-seeded into
/// `by_status` so the UI never has to read a missing key as a zero.
const UNIT_STATUSES: [&str; 4] = ["available", "rented", "service", "retired"];

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
        let retired = u.status == UNIT_STATUSES[3];
        let is_available = u.status == UNIT_STATUSES[0];
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

// ── fail-closed column readers ───────────────────────────────────
//
// Every column these read is named in the SELECT list above, so a renamed
// column fails the query itself. What is left is a type mismatch, and these
// map it to a 500 with the column in the log. That is the deliberate opposite
// of `try_get_warn!`, which is fail-open by design: it would answer a customer
// with `0` / `false` / `""` and leave a log line as the only evidence.

fn read_error(column: &str, e: &sea_orm::DbErr) -> StatusCode {
    tracing::error!("bikes API: column `{}` unreadable: {}", column, e);
    StatusCode::INTERNAL_SERVER_ERROR
}

fn text(r: &QueryResult, column: &'static str) -> Result<String, StatusCode> {
    r.try_get::<String>("", column)
        .map_err(|e| read_error(column, &e))
}

fn text_opt(r: &QueryResult, column: &'static str) -> Result<Option<String>, StatusCode> {
    r.try_get::<Option<String>>("", column)
        .map_err(|e| read_error(column, &e))
}

fn int(r: &QueryResult, column: &'static str) -> Result<i32, StatusCode> {
    r.try_get::<i32>("", column)
        .map_err(|e| read_error(column, &e))
}

fn int_opt(r: &QueryResult, column: &'static str) -> Result<Option<i32>, StatusCode> {
    r.try_get::<Option<i32>>("", column)
        .map_err(|e| read_error(column, &e))
}

/// `COUNT(*)` is `bigint`.
fn count(r: &QueryResult, column: &'static str) -> Result<i64, StatusCode> {
    r.try_get::<i64>("", column)
        .map_err(|e| read_error(column, &e))
}

fn flag(r: &QueryResult, column: &'static str) -> Result<bool, StatusCode> {
    r.try_get::<bool>("", column)
        .map_err(|e| read_error(column, &e))
}

/// Read a nullable money column (D9). SQL NULL stays `None` and serialises as
/// JSON `null`, which the UI renders as a dash.
fn money(r: &QueryResult, column: &'static str) -> Result<Option<f64>, StatusCode> {
    let raw = r
        .try_get::<Option<f64>>("", column)
        .map_err(|e| read_error(column, &e))?;
    let out = publishable_money(raw);
    if raw.is_some() && out.is_none() {
        // Loud, because a stored value we refuse to publish is a data defect,
        // not a missing price. The customer still sees a dash — the log is for
        // us, never a substitute for correct output.
        tracing::error!(
            "bikes API: column `{}` holds unpublishable {:?}; serving null",
            column,
            raw
        );
    }
    Ok(out)
}

/// Read a nullable discount fraction. `class_discounts.discount` and
/// `rental_terms.discount_min/max` are NOT NULL in the schema, but the LEFT
/// JOIN in [`FAMILY_FROM`] yields NULL for a class with no published row —
/// and that is "unpublished", not "no discount". `0.0` would assert full
/// price, a claim we have no source for.
fn fraction(r: &QueryResult, column: &'static str) -> Result<Option<f64>, StatusCode> {
    let raw = r
        .try_get::<Option<f64>>("", column)
        .map_err(|e| read_error(column, &e))?;
    let out = publishable_fraction(raw);
    if raw.is_some() && out.is_none() {
        tracing::error!(
            "bikes API: column `{}` holds unpublishable discount {:?}; serving null",
            column,
            raw
        );
    }
    Ok(out)
}

/// What a money column is allowed to become in a response.
///
/// | stored | served | why |
/// | --- | --- | --- |
/// | NULL | `None` | the source publishes nothing (D9) |
/// | finite, `>= 0` | verbatim | including `0.0` — the DB asserts it and this function does not rewrite asserted data. Guarding *absent* against becoming `0` is what D9 asks for, and an absent value never reaches here as `Some`. |
/// | negative | `None` | no published tariff, deposit or sale price is negative; a dash beats a wrong number |
/// | NaN / ±Inf | `None` | the `clamp` closure D9 names turns these into a confident `0.0`, i.e. FREE. Absent is the honest rendering. |
fn publishable_money(raw: Option<f64>) -> Option<f64> {
    match raw {
        Some(v) if v.is_finite() && v >= 0.0 => Some(v),
        _ => None,
    }
}

/// A discount is a fraction in `0.0..=1.0` (0.25 = the 25% scooter class
/// discount). Anything outside that, or non-finite, is not a discount we can
/// publish.
fn publishable_fraction(raw: Option<f64>) -> Option<f64> {
    match raw {
        Some(v) if v.is_finite() && (0.0..=1.0).contains(&v) => Some(v),
        _ => None,
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
        client_rate, list_sql, publishable_fraction, publishable_money, query_flag, rollup_units,
        UnitRow, CLIENT_RATE_UNAVAILABLE, FAMILY_COLUMNS,
    };
    use std::collections::HashMap;

    // ── D9: absent stays absent, and nothing becomes 0 ───────────

    #[test]
    fn absent_money_stays_absent() {
        // The whole point: NULL must not become 0.0 anywhere on this path.
        assert_eq!(publishable_money(None), None);
    }

    #[test]
    fn published_money_passes_through_verbatim() {
        // NMAX 155's published base tariff, unrounded and unadjusted.
        assert_eq!(publishable_money(Some(449.0)), Some(449.0));
        assert_eq!(publishable_money(Some(2788.0)), Some(2788.0));
    }

    #[test]
    fn stored_zero_is_not_rewritten() {
        // A stored 0 is a value the DB asserts. D9's rule is that an *absent*
        // number must not become 0 — not that a present 0 must vanish.
        assert_eq!(publishable_money(Some(0.0)), Some(0.0));
    }

    #[test]
    fn non_finite_money_is_absent_not_zero() {
        // This is the `clamp` closure's failure mode (NaN -> 0.0 -> "FREE").
        assert_eq!(publishable_money(Some(f64::NAN)), None);
        assert_eq!(publishable_money(Some(f64::INFINITY)), None);
        assert_eq!(publishable_money(Some(f64::NEG_INFINITY)), None);
    }

    #[test]
    fn negative_money_is_absent() {
        assert_eq!(publishable_money(Some(-1.0)), None);
    }

    #[test]
    fn published_fractions_pass_through() {
        assert_eq!(publishable_fraction(Some(0.25)), Some(0.25)); // scooter
        assert_eq!(publishable_fraction(Some(0.15)), Some(0.15)); // motorcycle
        assert_eq!(publishable_fraction(Some(0.0)), Some(0.0));
        assert_eq!(publishable_fraction(Some(1.0)), Some(1.0));
    }

    #[test]
    fn out_of_range_or_non_finite_fraction_is_absent() {
        assert_eq!(publishable_fraction(None), None);
        assert_eq!(publishable_fraction(Some(1.5)), None);
        assert_eq!(publishable_fraction(Some(-0.1)), None);
        assert_eq!(publishable_fraction(Some(f64::NAN)), None);
        assert_eq!(publishable_fraction(Some(25.0)), None); // percent, not fraction
    }

    #[test]
    fn money_columns_are_read_as_nullable_doubles() {
        // Guards the SELECT list against losing a ::float8 cast (which would
        // turn a `numeric` column into a read error) or an alias (which would
        // turn it into a missing key downstream).
        for column in [
            "base_rate_thb_day",
            "deposit_thb",
            "monthly_low_season_thb",
            "sale_price_thb",
        ] {
            let needle = format!("b.{column}::float8 AS {column}");
            assert!(
                FAMILY_COLUMNS.contains(&needle),
                "FAMILY_COLUMNS lost `{needle}`"
            );
        }
    }

    #[test]
    fn reader_types_are_pinned_by_casts() {
        // `id` may be uuid, `class`/`body` may be enums, the integers may be
        // smallint. Each reader asks for one concrete Rust type, so the cast
        // is what keeps an unpredicted column type from 500-ing the catalog.
        for needle in [
            "b.id::text AS id",
            "b.class::text AS class",
            "b.body::text AS body",
            "b.displacement_cc::int4 AS displacement_cc",
            "b.sort_order::int4 AS sort_order",
            "cd.discount::float8 AS class_discount",
        ] {
            assert!(
                FAMILY_COLUMNS.contains(needle),
                "FAMILY_COLUMNS lost `{needle}`"
            );
        }
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

    // ── list_sql ─────────────────────────────────────────────────

    #[test]
    fn list_sql_default_is_offered_only() {
        let sql = list_sql(false, false);
        assert!(sql.contains("b.offered = TRUE"), "{sql}");
        assert!(!sql.contains("units_available, 0) > 0"), "{sql}");
    }

    #[test]
    fn list_sql_available_only_adds_the_stock_filter() {
        let sql = list_sql(false, true);
        assert!(sql.contains("b.offered = TRUE"), "{sql}");
        assert!(sql.contains("COALESCE(u.units_available, 0) > 0"), "{sql}");
        assert!(sql.contains(" AND "), "{sql}");
    }

    #[test]
    fn list_sql_include_unoffered_drops_only_the_offered_filter() {
        let sql = list_sql(true, false);
        assert!(!sql.contains("b.offered = TRUE"), "{sql}");
        // ...and still selects the column, so the client can see WHY.
        assert!(sql.contains("b.offered,"), "{sql}");
        assert!(!sql.contains(" WHERE "), "{sql}");
    }

    #[test]
    fn list_sql_include_unoffered_and_available_only_combine() {
        let sql = list_sql(true, true);
        assert!(sql.contains("COALESCE(u.units_available, 0) > 0"), "{sql}");
        assert!(!sql.contains("b.offered = TRUE"), "{sql}");
    }

    #[test]
    fn list_sql_every_variant_is_bounded_and_ordered() {
        for (unoffered, available) in [(false, false), (false, true), (true, false), (true, true)] {
            let sql = list_sql(unoffered, available);
            assert!(sql.contains("LIMIT 2000"), "{sql}");
            assert!(sql.contains("ORDER BY b.sort_order ASC"), "{sql}");
            assert!(sql.contains("FROM bikes b"), "{sql}");
            assert!(sql.contains("class_discounts"), "{sql}");
            assert!(sql.contains("FROM bike_units"), "{sql}");
            // No request text is interpolated — the only inputs are booleans.
            assert!(!sql.contains('$'), "{sql}");
        }
    }

    #[test]
    fn list_sql_counts_exclude_retired_units() {
        let sql = list_sql(false, false);
        assert!(
            sql.contains("COUNT(*) FILTER (WHERE status <> 'retired')"),
            "{sql}"
        );
        assert!(
            sql.contains("COUNT(*) FILTER (WHERE status = 'available')"),
            "{sql}"
        );
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
