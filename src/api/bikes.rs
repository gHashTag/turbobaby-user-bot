//! TurboBaby catalog API — bike families, their unit availability, and the
//! published rental terms. Issues #12 / #13.
//!
//! | route | serves |
//! | --- | --- |
//! | `GET /api/bikes` | offered families; filtered by `?available_only=true`, `?class=`, `?min_cc=`, `?max_cc=` — see [`CatalogFilter`] |
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
//! returns, and **if the door is silent the bot must not compute**. Issue #25
//! owns the door. So every family here carries
//!
//! ```text
//! "client_rate_thb_day": null,
//! "client_rate_source":  "unavailable"
//! ```
//!
//! produced by [`ask_door`] — the one function #25 replaces. `null` +
//! `"unavailable"` is a first-class state, not an error: the UI must render
//! "a human quotes this price" and emit no number at all.
//!
//! `base_rate_thb_day` and `class_discount` are also served, because the admin
//! surface and #25's divergence log both need the file's number. They are the
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

use crate::api::auth::check_admin;
use crate::api::cache::make_etag_header;
use crate::api::validate_url;
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
        // ── admin (every handler calls `check_admin` first) ──────────
        //
        // These are new, and what they replace is nothing: the admin fleet
        // screen has been issuing POST/PUT/DELETE against `/api/bikes` and
        // `/api/bikes/:id` since it was written, and no route has ever
        // matched them. Axum answered 405, the screen updated its own
        // in-memory cache first and only rolled back on failure, so the
        // toast said "✅ Добавлена!" and the row appeared — until a reload
        // fetched the truth from the database and it was gone.
        //
        // The paths are `/admin/bikes` rather than the `/bikes` the screen
        // was calling because a family the public catalog hides must not be
        // reachable by shaping a query against the public list; see
        // `db::bikes::list_all_families`. The screen is moved to match.
        .route(
            "/admin/bikes",
            get(admin_list_bikes).post(admin_create_bike),
        )
        .route(
            "/admin/bikes/:id",
            axum::routing::put(admin_update_bike).delete(admin_delete_bike),
        )
        .route(
            "/admin/bikes/:id/offered",
            axum::routing::put(admin_set_offered),
        )
        // ── the physical fleet ──────────────────────────────────────
        //
        // Same story one level down, and found by the gate rather than by
        // reading: the Units and Service tabs of the same screen have been
        // calling `/api/bike-units*` and `/api/admin/bike-service-records*`
        // since they were written, and neither path has ever been
        // registered anywhere. These did not even 405 — an unmatched path
        // falls through to the SPA fallback, which answers 200 with
        // `index.html`, so the list parse failed silently and every write
        // "succeeded" until a reload. `tests/ui_endpoints_exist.rs` is the
        // instrument; it is what stops the fourth instance.
        .route(
            "/admin/bike-units",
            get(admin_list_units).post(admin_create_unit),
        )
        .route(
            "/admin/bike-units/:id",
            axum::routing::put(admin_update_unit).delete(admin_delete_unit),
        )
        .route(
            "/admin/bike-units/:id/status",
            axum::routing::put(admin_set_unit_status),
        )
        .route(
            "/admin/bike-service-records",
            get(admin_list_service_records).post(admin_create_service_record),
        )
        .route(
            "/admin/bike-service-records/:id",
            axum::routing::delete(admin_delete_service_record),
        )
    // There is deliberately no `/admin/bikes/:id/for-sale` twin of the
    // `offered` toggle. `offered` has its own route because it is flipped
    // from the list row, where no form is open and a full-body PUT would
    // clobber whatever a stale card holds. The forecourt flag is edited on
    // the card next to the asking price it sits beside, so the card's PUT
    // already carries it — a second route would be an endpoint with no
    // caller, and this repository has enough of those.
}

// ── the door seam (D11) ──────────────────────────────────────────

/// `client_rate_source` when the number came from the owner's live sheet.
/// Reached by [`DoorAnswers::resolve`]; no family carries it today, because the
/// batch the handlers build is still empty — issue #25 fills it.
const CLIENT_RATE_FROM_DOOR: &str = "door";

/// `client_rate_source` when there is no client-facing number to give.
/// Covers both "the door is not wired yet" (now) and "the door was asked and
/// stayed silent" (after #25) — from the UI's side those are the same state:
/// say a human quotes this price, emit no number.
const CLIENT_RATE_UNAVAILABLE: &str = "unavailable";

/// What the door said about one family.
///
/// `Silent` deliberately collapses three door states — answered with nothing,
/// timed out, errored. That is not laziness: from the customer's side they are
/// one state, and the UI has no honest way to tell them apart. Splitting them
/// here would invite a screen that says "we could not reach the owner" for one
/// and something else for another, when D11's answer to all three is the same
/// sentence.
///
/// Neither variant is constructed outside the tests yet, because [`ask_door`]
/// still returns an empty batch — the `#[allow(dead_code)]` records that, and
/// #25 removes it by writing the door body. Declaring both states now is what
/// lets [`DoorAnswers::resolve`] be written and tested against the priced
/// branch years before a live sheet is reachable from the server.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DoorAnswer {
    Quoted(f64),
    Silent,
}

/// One door round-trip's worth of answers, keyed by family key.
///
/// The map exists so the `await` can be hoisted out of the per-family loop. A
/// key that is absent resolves to silence, which is what makes today's empty
/// map *correct* rather than a special case to be removed later.
#[derive(Debug, Clone, Default)]
pub(crate) struct DoorAnswers(HashMap<String, DoorAnswer>);

impl DoorAnswers {
    /// The client-facing per-day rate for one family, and where it came from.
    ///
    /// Never a fallback computation from `base_rate_thb_day` and
    /// `class_discount` — that is why this returns an `Option` rather than an
    /// `f64`, and why the lookup handle is the family key rather than a price.
    pub(crate) fn resolve(&self, family_key: &str) -> (Option<f64>, &'static str) {
        match self.0.get(family_key) {
            Some(DoorAnswer::Quoted(rate)) => (Some(*rate), CLIENT_RATE_FROM_DOOR),
            Some(DoorAnswer::Silent) | None => (None, CLIENT_RATE_UNAVAILABLE),
        }
    }
}

/// Ask the door for every family on one page, in one call.
///
/// **Issue #25 owns the body.** Today there is no door to ask, so this answers
/// with an empty map — every key resolves to silence, which is the same shape
/// the wired door must produce when the owner does not answer.
///
/// The batch signature is the point, and it is deliberate rather than
/// incidental. The measured door latency is 21.4 s
/// (`data/fleet_seed.json` → `measured_door_latency_s`), so the timeout path is
/// the *common* path, not the exceptional one. A per-family call would make
/// `GET /api/bikes` cost fourteen families × 21.4 s ≈ five minutes. The
/// contract #25 must keep:
///
/// * one call per request, covering every family on the page — never one per
///   card;
/// * one shared deadline for the whole batch;
/// * a deadline breach yields `Silent` for every unanswered key and **not** an
///   error to the caller. D11 forbids silence as firmly as invention — the
///   seed's `must_not_emit` has five entries and the fifth is "silence" — so an
///   unreachable door must still produce a catalog that says a human quotes the
///   price, not an error page that says nothing.
pub(crate) async fn ask_door(keys: &[&str]) -> DoorAnswers {
    let _ = keys;
    DoorAnswers::default()
}

// ── catalog filters ──────────────────────────────────────────────

/// The class and displacement filters `#8` names in its title.
///
/// They were missing until now, and the way they were missing is the point:
/// `list_bikes` read `available_only` and nothing else, and Axum hands unknown
/// query keys through without complaint. So `GET /api/bikes?class=motorcycle`
/// answered **200 with all thirteen families** — a caller that trusted the
/// parameter got the unfiltered catalog and no way to tell.
///
/// That is why an unrecognised *value* is a `400` here rather than a default.
/// `?class=motorcyle` is a typo, and the only honest answers to a typo are the
/// error or the whole catalog-with-a-warning; silently returning everything
/// under a 200 is the one answer that lies. Unknown *keys* are still ignored,
/// which is ordinary HTTP manners — the asymmetry is deliberate: a key the
/// server does not know may belong to someone else, but a value it does not
/// know was meant for it.
#[derive(Debug, Default, PartialEq)]
struct CatalogFilter {
    /// `scooter` | `motorcycle`, validated against [`BIKE_CLASSES`].
    ///
    /// Deliberately the *same* constant the admin write path checks, not a
    /// fresh copy of the two words: that constant is already pinned to the
    /// migration's CHECK constraint by
    /// `the_accepted_classes_are_the_ones_the_column_allows`, so reusing it
    /// buys this filter the same guard for free. A second copy would be the
    /// restated-list defect, and it would drift the first time a third class
    /// is added.
    class: Option<String>,
    /// Inclusive bounds on `bikes.displacement_cc`.
    min_cc: Option<i32>,
    max_cc: Option<i32>,
}

impl CatalogFilter {
    /// Parse, or name what was wrong with the request.
    ///
    /// The error strings are returned to the caller, so they say which
    /// parameter and which value — a bare `400` on a catalog read tells a
    /// client nothing it can act on.
    fn parse(q: &HashMap<String, String>) -> Result<Self, String> {
        let class = match q.get("class") {
            None => None,
            Some(raw) => {
                let lowered = raw.to_ascii_lowercase();
                if !BIKE_CLASSES.contains(&lowered.as_str()) {
                    return Err(format!(
                        "class must be one of {BIKE_CLASSES:?}, got {raw:?}"
                    ));
                }
                Some(lowered)
            }
        };

        let cc = |key: &str| -> Result<Option<i32>, String> {
            match q.get(key) {
                None => Ok(None),
                Some(raw) => raw
                    .parse::<i32>()
                    .map_err(|_| format!("{key} must be a whole number of cc, got {raw:?}"))
                    .and_then(|v| {
                        if v < 0 {
                            Err(format!("{key} must not be negative, got {v}"))
                        } else {
                            Ok(Some(v))
                        }
                    }),
            }
        };
        let min_cc = cc("min_cc")?;
        let max_cc = cc("max_cc")?;

        // An inverted range is a request that can only ever answer nothing.
        // Returning an empty list would be defensible; saying so is better,
        // because the empty list is indistinguishable from "we rent no bikes
        // in that band", which is a different and much worse thing to tell a
        // customer.
        if let (Some(lo), Some(hi)) = (min_cc, max_cc) {
            if lo > hi {
                return Err(format!(
                    "min_cc ({lo}) is above max_cc ({hi}); no displacement can satisfy both"
                ));
            }
        }

        Ok(Self {
            class,
            min_cc,
            max_cc,
        })
    }

    fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Takes the two fields rather than the whole `bikes` row on purpose: it
    /// makes the predicate testable without constructing a SeaORM `Model` with
    /// twenty-odd columns and two timestamps, none of which it reads. A test
    /// that needs a fixture that large tends not to get written.
    fn matches(&self, class: &str, displacement_cc: i32) -> bool {
        if let Some(ref want) = self.class {
            if !class.eq_ignore_ascii_case(want) {
                return false;
            }
        }
        if let Some(lo) = self.min_cc {
            if displacement_cc < lo {
                return false;
            }
        }
        if let Some(hi) = self.max_cc {
            if displacement_cc > hi {
                return false;
            }
        }
        true
    }

    /// The part of the ETag cache key this filter owns.
    ///
    /// The comment on `cache_key` below warns that a filtered catalog must not
    /// flap the unfiltered one's ETag. That warning was written when there was
    /// exactly one filter and two possible keys; with three more it has to be
    /// derived rather than spelled out, or the next filter added silently
    /// serves one selection's body under another's ETag.
    fn cache_suffix(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        format!(
            "|class={}|min={}|max={}",
            self.class.as_deref().unwrap_or("*"),
            self.min_cc
                .map(|v| v.to_string())
                .unwrap_or_else(|| "*".into()),
            self.max_cc
                .map(|v| v.to_string())
                .unwrap_or_else(|| "*".into()),
        )
    }
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

    // Parsed before the database is touched: a malformed request should cost
    // no query. The error is carried in the body rather than as a bare
    // `StatusCode`, because "400" alone on a catalog read tells a client
    // nothing it can fix.
    let filter = match CatalogFilter::parse(&q) {
        Ok(f) => f,
        Err(reason) => {
            tracing::debug!("list_bikes: rejecting query: {reason}");
            let mut response = axum::response::Response::new(axum::body::Body::from(
                json!({ "error": "bad_request", "detail": reason }).to_string(),
            ));
            response
                .headers_mut()
                .insert("content-type", HeaderValue::from_static("application/json"));
            *response.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(response);
        }
    };

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
    // Applied here rather than pushed into the SQL: the offered catalog is
    // thirteen rows, so the filter costs nothing in memory, and keeping it out
    // of `list_offered_families` leaves exactly one place that decides what
    // "offered" means. A `WHERE class = …` bolted onto that query would be the
    // second.
    let listings: Vec<_> = listings
        .into_iter()
        .filter(|l| filter.matches(&l.bike.class, l.bike.displacement_cc))
        .collect();
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

    // One door round-trip for the whole page, before the loop. The map below
    // stays synchronous precisely so a per-family `await` cannot be written
    // into it without the batch being taken apart first (D11, 21.4 s measured).
    let keys: Vec<&str> = listings.iter().map(|l| l.bike.key.as_str()).collect();
    let door = ask_door(&keys).await;

    let families = listings
        .iter()
        .map(|l| family_json(l, &discounts, &door))
        .collect::<Result<Vec<Value>, StatusCode>>()?;
    let body = json!({ "bikes": families }).to_string();

    // Separate keys: the filtered catalog must not flap the unfiltered one's
    // ETag, and vice versa.
    //
    // Observation for whoever wires the door (#25), recorded rather than acted
    // on: once a door number is in the body, this stops being a pure content
    // hash and becomes a freshness decision. A flapping door answer flaps the
    // ETag; a cached body serves a stale door price for up to the `max-age`
    // below. #25 does not specify which of those is wanted, so the behaviour
    // here is left exactly as it is rather than a policy being picked quietly.
    let cache_key = format!(
        "{}{}",
        if available_only {
            "bikes_available"
        } else {
            "bikes"
        },
        filter.cache_suffix()
    );
    let cache_key = cache_key.as_str();
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
    let door = ask_door(&[listing.bike.key.as_str()]).await;

    let bike = family_detail_json(&listing, &discounts, &units, &door)?;
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
fn family_json(
    listing: &BikeListing,
    discounts: &[ClassDiscount],
    door: &DoorAnswers,
) -> Result<Value, StatusCode> {
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
    //
    // A family closed to new rentals is never priced, even if the door answers
    // for it (D12, #25's fifth criterion). CLICK 125 is the measured case: the
    // seasonal grid still carries 187/day for it, and that figure is stale —
    // not a tariff. The family stays serialisable and the detail endpoint stays
    // 200, because the PCX 150 / ADV 150 / NMAX 155 redirect needs that page to
    // exist. Never bookable and never priced is not the same as hidden.
    let (client_rate_thb_day, client_rate_source) = if listing.bike.offered {
        door.resolve(&listing.bike.key)
    } else {
        (None, CLIENT_RATE_UNAVAILABLE)
    };
    obj.insert(
        "client_rate_thb_day".to_string(),
        json!(client_rate_thb_day),
    );
    obj.insert("client_rate_source".to_string(), json!(client_rate_source));

    // D11's divergence log: when the door does quote, record how far its number
    // sits from what the file implies, and carry on. The comparator is the one
    // the order path already uses — one implementation, not a catalog twin
    // (D15) — and it is deliberately **non-blocking**: neither number is
    // corrected to match the other, the answer still returns, and the log is
    // for whoever reconciles the owner's sheet later.
    if let Some(rate) = client_rate_thb_day {
        if let Some((quoted, from_file)) = crate::api::orders::rate_divergence(
            Some(rate),
            listing.bike.base_rate_thb_day,
            discount_for_class(discounts, &listing.bike.class),
        ) {
            tracing::info!(
                family = %listing.bike.key,
                quoted,
                from_file,
                "D11 door/file divergence"
            );
        }
    }

    Ok(value)
}

/// One family plus its unit rollup — the `GET /api/bikes/:key` body.
///
/// Pure, so the contract is testable without a database.
fn family_detail_json(
    listing: &BikeListing,
    discounts: &[ClassDiscount],
    units: &[UnitRow],
    door: &DoorAnswers,
) -> Result<Value, StatusCode> {
    let mut value = family_json(listing, discounts, door)?;
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

// ──────────────────────────────────────────────────────────────────
// Admin write surface
// ──────────────────────────────────────────────────────────────────
//
// Every handler below opens with `check_admin`. That is the whole gate:
// there is no per-handler role, and none of these paths is reachable
// without it.

/// A family as the admin screen submits it, for create and for update alike.
///
/// One struct for both because the screen sends the same fields either way,
/// and two structs would be the restated-list defect with a type system
/// attached — a field added to the create form and forgotten on the edit form
/// is exactly the drift that costs an afternoon.
///
/// Money is `Option<f64>` and stays `Option` all the way to the column (D9).
/// `None` means the shop has not published that number; it is never written
/// as `0`, because a `0` deposit reads as "no deposit required" and a `0`
/// sale price reads as "free".
/// `key` is optional because it is write-once. It is required on create and
/// ignored on update: `bikes.key` is the `catalog_id` a cart rental line
/// carries (D8), so renaming a family orphans every live cart that holds one.
/// The edit card does not offer it as a field; the server does not accept it
/// as one either, so the rule holds even against a hand-made request.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct BikeWriteRequest {
    #[serde(default)]
    pub key: Option<String>,
    pub brand: String,
    pub model: String,
    #[serde(default)]
    pub variant_label: Option<String>,
    pub class: String,
    pub body: String,
    pub displacement_cc: i32,
    #[serde(default)]
    pub base_rate_thb_day: Option<f64>,
    #[serde(default)]
    pub deposit_thb: Option<f64>,
    #[serde(default)]
    pub monthly_low_season_thb: Option<f64>,
    #[serde(default)]
    pub sale_price_thb: Option<f64>,
    #[serde(default = "default_true")]
    pub offered: bool,
    /// Defaults to `false`, not to "keep whatever is stored": a body that
    /// omits the flag is a body written before the flag existed, and the
    /// safe reading of silence is that the bike is not on the forecourt.
    #[serde(default)]
    pub for_sale: bool,
    #[serde(default)]
    pub sort_order: Option<i32>,
    #[serde(default)]
    pub description_ru: Option<String>,
    #[serde(default)]
    pub description_en: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
}

fn default_true() -> bool {
    true
}

/// The two values `bikes.class` may hold, mirroring the CHECK constraint in
/// `077_bikes.sql`.
///
/// Checked here so a typo comes back as a 400 naming the field rather than as
/// a 500 carrying a Postgres constraint name. The list is duplicated from SQL,
/// which is the restated-list defect — so it is pinned by
/// `the_accepted_classes_are_the_ones_the_column_allows`, which reads the
/// migration.
const BIKE_CLASSES: [&str; 2] = ["scooter", "motorcycle"];

/// Largest engine the form will accept, in cc. The fleet's biggest is the
/// X-ADV 750; the bound exists to catch a slipped decimal point, not to
/// express an opinion about motorcycles.
const MAX_DISPLACEMENT_CC: i32 = 3000;

/// Longest free-text field the form will accept. Descriptions are a
/// paragraph, not a document.
const MAX_TEXT_LEN: usize = 4000;

/// What is wrong with a submitted family, in words the admin screen shows.
///
/// A string, not a code: the screen already renders the response body
/// verbatim under the save button (`src/ui/screens/admin_screen.rs` reads
/// `r.text()` on a non-success status), so the most useful thing this can
/// return is a sentence. "HTTP 400" tells the owner nothing about which of
/// seventeen fields he got wrong.
type WriteRejection = String;

/// Check and normalise a submitted family.
///
/// Pure, so the whole contract is testable without a database — which matters
/// more than usual here, because the alternative is discovering the rules by
/// watching Postgres reject things in production.
///
/// Normalisation is deliberately narrow: whitespace is trimmed, and an empty
/// optional string becomes `None`. Nothing else is rewritten. In particular a
/// money value is never rounded, clamped or defaulted — an unusable number is
/// rejected outright rather than quietly turned into a different number that
/// a customer then sees (D9/D11).
pub(crate) fn validate_bike_write(
    req: &BikeWriteRequest,
) -> Result<BikeWriteRequest, WriteRejection> {
    fn required(label: &str, v: &str, max: usize) -> Result<String, WriteRejection> {
        let t = v.trim();
        if t.is_empty() {
            return Err(format!("Поле «{label}» обязательно"));
        }
        if t.chars().count() > max {
            return Err(format!("Поле «{label}» длиннее {max} символов"));
        }
        Ok(t.to_string())
    }
    fn optional(
        label: &str,
        v: &Option<String>,
        max: usize,
    ) -> Result<Option<String>, WriteRejection> {
        let Some(t) = v.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        if t.chars().count() > max {
            return Err(format!("Поле «{label}» длиннее {max} символов"));
        }
        Ok(Some(t.to_string()))
    }
    // A money field is either absent or a real, non-negative number. NaN and
    // infinity are rejected rather than filtered to `None` the way the read
    // path filters them: on the way out, a stored NaN is damage to be survived;
    // on the way in, it is a bug to be refused before it becomes that damage.
    fn money(label: &str, v: Option<f64>) -> Result<Option<f64>, WriteRejection> {
        match v {
            None => Ok(None),
            Some(n) if !n.is_finite() => Err(format!("Поле «{label}»: не число")),
            Some(n) if n < 0.0 => Err(format!("Поле «{label}»: отрицательная цена")),
            Some(n) => Ok(Some(n)),
        }
    }

    // The key is a URL path segment (`GET /api/bikes/:key`) and a cart line's
    // `catalog_id` (D8). Restricting it to the shape the seeded keys already
    // have — `xmax-300-new` — keeps both of those honest without needing an
    // escaping rule anywhere downstream. Absent is allowed here and refused by
    // the create handler, which is the only caller that needs one.
    let key = match optional("ключ", &req.key, MAX_KEY_LEN)? {
        None => None,
        Some(k) => {
            if !k
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err("Ключ: только строчные латинские буквы, цифры и дефис".to_string());
            }
            Some(k)
        }
    };

    let class = required("класс", &req.class, 32)?;
    if !BIKE_CLASSES.contains(&class.as_str()) {
        return Err(format!(
            "Класс должен быть одним из: {}",
            BIKE_CLASSES.join(", ")
        ));
    }

    if req.displacement_cc < 0 || req.displacement_cc > MAX_DISPLACEMENT_CC {
        return Err(format!(
            "Объём двигателя: от 0 до {MAX_DISPLACEMENT_CC} см³"
        ));
    }

    let image_url = optional("фото", &req.image_url, 2048)?;
    validate_url(&image_url).map_err(|_| "Ссылка на фото: недопустимый адрес".to_string())?;

    Ok(BikeWriteRequest {
        key,
        brand: required("бренд", &req.brand, 200)?,
        model: required("модель", &req.model, 200)?,
        variant_label: optional("вариант", &req.variant_label, 200)?,
        class,
        body: required("тип", &req.body, 64)?,
        displacement_cc: req.displacement_cc,
        base_rate_thb_day: money("цена в день", req.base_rate_thb_day)?,
        deposit_thb: money("депозит", req.deposit_thb)?,
        monthly_low_season_thb: money("цена за месяц", req.monthly_low_season_thb)?,
        sale_price_thb: money("цена продажи", req.sale_price_thb)?,
        offered: req.offered,
        for_sale: req.for_sale,
        sort_order: req.sort_order,
        description_ru: optional("описание", &req.description_ru, MAX_TEXT_LEN)?,
        description_en: optional("описание (EN)", &req.description_en, MAX_TEXT_LEN)?,
        image_url,
    })
}

/// `GET /api/admin/bikes` — every family, offered or not.
///
/// No ETag and no cache key, unlike the public list: this is the screen the
/// owner edits from, and a 304 served off a body that predates his last save
/// is how an edit appears to have been lost.
async fn admin_list_bikes(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;
    let listings = crate::db::bikes::list_all_families(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("admin_list_bikes: {e:#}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?;
    let discounts = list_class_discounts(&state.db.orm).await.map_err(|e| {
        tracing::error!("admin_list_bikes(class_discounts): {e:#}");
        (StatusCode::INTERNAL_SERVER_ERROR, String::new())
    })?;
    // The admin screen shows the same cards the catalog does, so it gets the
    // same shape — including `client_rate_*`, so the owner can see what the
    // door is actually quoting rather than what the file says (D11).
    let keys: Vec<&str> = listings.iter().map(|l| l.bike.key.as_str()).collect();
    let door = ask_door(&keys).await;
    let families = listings
        .iter()
        .map(|l| family_json(l, &discounts, &door))
        .collect::<Result<Vec<Value>, StatusCode>>()
        .map_err(|s| (s, String::new()))?;
    Ok(Json(json!({ "bikes": families })))
}

/// `POST /api/admin/bikes` — add a family.
async fn admin_create_bike(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<BikeWriteRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;
    let req = validate_bike_write(&req).map_err(|m| (StatusCode::BAD_REQUEST, m))?;
    let key = req.key.clone().ok_or((
        StatusCode::BAD_REQUEST,
        "Поле «ключ» обязательно".to_string(),
    ))?;

    use crate::db::entities::bike::{ActiveModel, Entity as BikeEntity};
    use sea_orm::{ActiveValue::Set, EntityTrait};

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().into();
    let model = ActiveModel {
        id: Set(id.clone()),
        key: Set(key.clone()),
        brand: Set(req.brand),
        model: Set(req.model),
        variant_label: Set(req.variant_label),
        class: Set(req.class),
        body: Set(req.body),
        displacement_cc: Set(req.displacement_cc),
        base_rate_thb_day: Set(req.base_rate_thb_day),
        deposit_thb: Set(req.deposit_thb),
        monthly_low_season_thb: Set(req.monthly_low_season_thb),
        sale_price_thb: Set(req.sale_price_thb),
        offered: Set(req.offered),
        for_sale: Set(req.for_sale),
        description_ru: Set(req.description_ru),
        description_en: Set(req.description_en),
        image_url: Set(req.image_url),
        sort_order: Set(req.sort_order.unwrap_or(0)),
        created_at: Set(now),
        updated_at: Set(now),
    };
    BikeEntity::insert(model)
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            // `bikes.key` is UNIQUE. A duplicate is the owner adding a family
            // twice, which is an ordinary mistake and deserves a sentence, not
            // a 500 and a log line he cannot read.
            let text = e.to_string();
            if text.contains("duplicate key") || text.contains("unique constraint") {
                return (
                    StatusCode::CONFLICT,
                    format!("Модель с ключом «{key}» уже есть"),
                );
            }
            tracing::error!("admin_create_bike: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?;
    tracing::info!(bike_id = %id, key = %key, "admin: family created");
    Ok(Json(json!({ "id": id })))
}

/// `PUT /api/admin/bikes/:id` — replace a family's editable fields.
async fn admin_update_bike(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<BikeWriteRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;
    let req = validate_bike_write(&req).map_err(|m| (StatusCode::BAD_REQUEST, m))?;

    use crate::db::entities::bike::{ActiveModel, Entity as BikeEntity};
    use sea_orm::{ActiveValue::Set, EntityTrait};

    let existing = BikeEntity::find_by_id(id.clone())
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("admin_update_bike(find): {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?
        .ok_or((StatusCode::NOT_FOUND, "Модель не найдена".to_string()))?;

    let key = existing.key.clone();
    let mut model: ActiveModel = existing.into();
    // `model.key` is deliberately NOT written. See `BikeWriteRequest::key`:
    // the key is a live cart's `catalog_id`, and a rename here would leave
    // those lines pointing at a family that no longer answers under that name.
    model.brand = Set(req.brand);
    model.model = Set(req.model);
    model.variant_label = Set(req.variant_label);
    model.class = Set(req.class);
    model.body = Set(req.body);
    model.displacement_cc = Set(req.displacement_cc);
    model.base_rate_thb_day = Set(req.base_rate_thb_day);
    model.deposit_thb = Set(req.deposit_thb);
    model.monthly_low_season_thb = Set(req.monthly_low_season_thb);
    model.sale_price_thb = Set(req.sale_price_thb);
    model.offered = Set(req.offered);
    model.for_sale = Set(req.for_sale);
    model.description_ru = Set(req.description_ru);
    model.description_en = Set(req.description_en);
    model.image_url = Set(req.image_url);
    if let Some(order) = req.sort_order {
        model.sort_order = Set(order);
    }
    model.updated_at = Set(chrono::Utc::now().into());
    BikeEntity::update(model)
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("admin_update_bike: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?;
    tracing::info!(bike_id = %id, key = %key, "admin: family updated");
    Ok(Json(json!({ "ok": true })))
}

/// `PUT /api/admin/bikes/:id/offered` — open or close a family to new rentals.
///
/// Its own route because it is the one edit the owner makes from the list
/// without opening the card, and because it must not require him to re-submit
/// seventeen other fields to flip one boolean — a full-body update used as a
/// toggle is how an unrelated field gets clobbered by a stale form.
async fn admin_set_offered(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let offered = crate::api::extract_bool(&body, "offered").map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Ожидалось поле offered".to_string(),
        )
    })?;
    set_family_flag(headers, state, &id, FamilyFlag::Offered, offered).await
}

/// Which single boolean [`set_family_flag`] is writing.
///
/// A one-variant enum today. It exists rather than a bare `bool` parameter
/// because the next flag on this table — and `bikes` has already grown one in
/// `086` — should extend a match rather than add a second copy of the
/// find-modify-save body below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FamilyFlag {
    Offered,
}

/// Find a family, move one boolean, save. Shared so a second toggle is a
/// match arm rather than a second transaction written slightly differently.
async fn set_family_flag(
    headers: HeaderMap,
    state: AppState,
    id: &str,
    flag: FamilyFlag,
    value: bool,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;

    use crate::db::entities::bike::{ActiveModel, Entity as BikeEntity};
    use sea_orm::{ActiveValue::Set, EntityTrait};

    let existing = BikeEntity::find_by_id(id.to_string())
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("set_family_flag(find): {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?
        .ok_or((StatusCode::NOT_FOUND, "Модель не найдена".to_string()))?;

    let key = existing.key.clone();
    let mut model: ActiveModel = existing.into();
    match flag {
        FamilyFlag::Offered => model.offered = Set(value),
    }
    model.updated_at = Set(chrono::Utc::now().into());
    BikeEntity::update(model)
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("set_family_flag: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?;
    tracing::info!(bike_id = %id, key = %key, ?flag, value, "admin: family flag set");
    Ok(Json(json!({ "ok": true })))
}

/// `DELETE /api/admin/bikes/:id` — remove a family that has no machines.
///
/// Refuses while any `bike_units` row points at it. The FK is
/// `ON DELETE CASCADE` (`078_bike_units.sql`), so without this check one tap
/// on the list screen destroys every unit of the family and, with them, the
/// per-unit service history `080_bike_service_records.sql` hangs off — records
/// of work that was actually done to a physical machine, which no amount of
/// re-adding the family brings back.
///
/// The owner's usual intent here is "stop renting this", and that is
/// `offered = false`, which is reversible and one route away. This path is for
/// a family added by mistake.
async fn admin_delete_bike(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;

    let units = crate::db::bikes::count_units_for_family(&state.db.orm, &id)
        .await
        .map_err(|e| {
            tracing::error!("admin_delete_bike(count): {e:#}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?;
    if units > 0 {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "У модели {units} шт. в парке — удаление удалит и их, и историю обслуживания. \
                 Сначала удалите байки, либо снимите модель с аренды."
            ),
        ));
    }

    use crate::db::entities::bike::Entity as BikeEntity;
    use sea_orm::EntityTrait;
    let res = BikeEntity::delete_by_id(id.clone())
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("admin_delete_bike: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?;
    if res.rows_affected == 0 {
        return Err((StatusCode::NOT_FOUND, "Модель не найдена".to_string()));
    }
    tracing::info!(bike_id = %id, "admin: family deleted");
    Ok(Json(json!({ "ok": true })))
}

// ── the physical fleet: units and service records ────────────────
//
// The same defect as the family routes above, found the same way and left
// behind when they were fixed: the admin screen's Units tab and Service tab
// have been issuing GET/POST/PUT/DELETE against `/api/bike-units` and
// `/api/admin/bike-service-records` since they were written, and no route has
// ever answered either path. They do not even 405 — an unmatched path falls
// through to the SPA fallback, which returns 200 and `index.html`, so the
// screens' `serde_json::from_str` quietly failed and the list simply rendered
// empty. Every write was optimistic-first, so a unit appeared in the table,
// the haptic buzzed success, and a reload showed it had never existed.
//
// `tests/ui_endpoints_exist.rs` is what turned that up, and is what stops the
// third instance.
//
// The paths are `/admin/*` for the same reason the family routes are: a
// physical machine is not public. The catalog publishes counts derived from
// `bike_units` (`rollup_units`) and nothing else — not codes, not colours per
// machine, not kilometres — and an endpoint that lists the rows themselves
// must not be reachable without the admin gate.

/// The write shape for one physical machine, shared by create and update.
///
/// `id`, `created_at` and `updated_at` are deliberately absent: the first is a
/// path parameter or a fresh UUID, the other two are the server's to stamp.
#[derive(Debug, serde::Deserialize)]
struct UnitWriteRequest {
    bike_id: String,
    unit_code: String,
    #[serde(default)]
    model_year: Option<i32>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    km_since_purchase: Option<i32>,
    status: String,
}

/// Validate a unit write, returning the owner's sentence rather than a code.
///
/// The three checks mirror constraints `078_bike_units.sql` already enforces.
/// They are restated here not to replace the database but to answer in Russian:
/// a CHECK violation arrives as a 500 and a log line the owner cannot read,
/// and "статус ... неизвестен" is the difference between a fixable mistake and
/// a screen that seems broken.
fn validate_unit_write(req: &UnitWriteRequest) -> Result<(), String> {
    let code = req.unit_code.trim();
    if code.is_empty() {
        return Err("Код байка обязателен".to_string());
    }
    if code.chars().count() > 64 {
        return Err("Код байка длиннее 64 символов".to_string());
    }
    if req.bike_id.trim().is_empty() {
        return Err("Выберите модель".to_string());
    }
    if !UNIT_STATUSES.contains(&req.status.as_str()) {
        return Err(format!(
            "Статус «{}» неизвестен — допустимы: {}",
            req.status,
            UNIT_STATUSES.join(", ")
        ));
    }
    // 078's own CHECK. A new machine legitimately reads 0, so 0 passes and
    // NULL still means "not recorded" (D9) — neither is rewritten into the
    // other here.
    if req.km_since_purchase.is_some_and(|km| km < 0) {
        return Err("Пробег с покупки не может быть отрицательным".to_string());
    }
    Ok(())
}

/// A duplicate `unit_code` is an ordinary mistake — the owner adding the same
/// machine twice — and deserves a sentence, not a 500.
fn unit_write_error(e: &sea_orm::DbErr, code: &str, context: &'static str) -> (StatusCode, String) {
    let text = e.to_string();
    if text.contains("duplicate key") || text.contains("unique constraint") {
        return (
            StatusCode::CONFLICT,
            format!("Байк с кодом «{code}» уже есть"),
        );
    }
    tracing::error!("{context}: {e}");
    (StatusCode::INTERNAL_SERVER_ERROR, String::new())
}

/// `GET /api/admin/bike-units` — every physical machine, retired ones included.
///
/// Unlike the catalog's rollup this does not hide `retired` rows: the owner's
/// reason for opening this tab is usually to look at exactly those.
async fn admin_list_units(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;

    use crate::db::entities::bike_unit::{Column as UnitCol, Entity as UnitEntity};
    use sea_orm::{EntityTrait, QueryOrder};
    let units = UnitEntity::find()
        .order_by_asc(UnitCol::UnitCode)
        .all(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("admin_list_units: {e:#}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?;

    // Built by hand rather than by serializing the entity, so that adding a
    // column to `bike_units` cannot publish it to a screen by accident — which
    // is the shape D14 cares about most on this table.
    let units: Vec<Value> = units
        .into_iter()
        .map(|u| {
            json!({
                "id": u.id,
                "bike_id": u.bike_id,
                "unit_code": u.unit_code,
                "model_year": u.model_year,
                "color": u.color,
                "km_since_purchase": u.km_since_purchase,
                "status": u.status,
            })
        })
        .collect();
    Ok(Json(json!({ "units": units })))
}

/// `POST /api/admin/bike-units` — add one physical machine.
async fn admin_create_unit(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<UnitWriteRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;
    validate_unit_write(&req).map_err(|m| (StatusCode::BAD_REQUEST, m))?;

    use crate::db::entities::bike_unit::{ActiveModel, Entity as UnitEntity};
    use sea_orm::{ActiveValue::Set, EntityTrait};

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().into();
    let code = req.unit_code.trim().to_string();
    let model = ActiveModel {
        id: Set(id.clone()),
        bike_id: Set(req.bike_id.trim().to_string()),
        unit_code: Set(code.clone()),
        model_year: Set(req.model_year),
        color: Set(req.color),
        km_since_purchase: Set(req.km_since_purchase),
        status: Set(req.status),
        created_at: Set(now),
        updated_at: Set(now),
    };
    UnitEntity::insert(model)
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            // The FK to `bikes(id)` fires here when the screen's family picker
            // holds a family somebody deleted in another tab.
            if e.to_string().contains("foreign key") {
                return (
                    StatusCode::BAD_REQUEST,
                    "Модель не найдена — обновите страницу".to_string(),
                );
            }
            unit_write_error(&e, &code, "admin_create_unit")
        })?;
    tracing::info!(unit_id = %id, unit_code = %code, "admin: unit created");
    Ok(Json(json!({ "id": id })))
}

/// `PUT /api/admin/bike-units/:id` — replace one machine's editable fields.
async fn admin_update_unit(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UnitWriteRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;
    validate_unit_write(&req).map_err(|m| (StatusCode::BAD_REQUEST, m))?;

    use crate::db::entities::bike_unit::{ActiveModel, Entity as UnitEntity};
    use sea_orm::{ActiveValue::Set, EntityTrait};

    let existing = UnitEntity::find_by_id(id.clone())
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("admin_update_unit(find): {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?
        .ok_or((StatusCode::NOT_FOUND, "Байк не найден".to_string()))?;

    let code = req.unit_code.trim().to_string();
    let mut model: ActiveModel = existing.into();
    model.bike_id = Set(req.bike_id.trim().to_string());
    model.unit_code = Set(code.clone());
    model.model_year = Set(req.model_year);
    model.color = Set(req.color);
    model.km_since_purchase = Set(req.km_since_purchase);
    model.status = Set(req.status);
    model.updated_at = Set(chrono::Utc::now().into());
    UnitEntity::update(model)
        .exec(&state.db.orm)
        .await
        .map_err(|e| unit_write_error(&e, &code, "admin_update_unit"))?;
    tracing::info!(unit_id = %id, unit_code = %code, "admin: unit updated");
    Ok(Json(json!({ "ok": true })))
}

/// `PUT /api/admin/bike-units/:id/status` — move one machine between
/// available / rented / service / retired.
///
/// Its own route for the reason `admin_set_offered` has one: this is the edit
/// made from the list row, where no form is open, and a full-body PUT used as a
/// one-field toggle is how a stale card clobbers a colour or a kilometre count
/// somebody else just fixed.
async fn admin_set_unit_status(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;

    let status = body
        .get("status")
        .and_then(Value::as_str)
        .ok_or((
            StatusCode::BAD_REQUEST,
            "Поле status обязательно".to_string(),
        ))?
        .to_string();
    if !UNIT_STATUSES.contains(&status.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Статус «{status}» неизвестен — допустимы: {}",
                UNIT_STATUSES.join(", ")
            ),
        ));
    }

    use crate::db::entities::bike_unit::{ActiveModel, Entity as UnitEntity};
    use sea_orm::{ActiveValue::Set, EntityTrait};

    let existing = UnitEntity::find_by_id(id.clone())
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("admin_set_unit_status(find): {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?
        .ok_or((StatusCode::NOT_FOUND, "Байк не найден".to_string()))?;

    let code = existing.unit_code.clone();
    let mut model: ActiveModel = existing.into();
    model.status = Set(status.clone());
    model.updated_at = Set(chrono::Utc::now().into());
    UnitEntity::update(model)
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("admin_set_unit_status: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?;
    tracing::info!(unit_id = %id, unit_code = %code, %status, "admin: unit status set");
    Ok(Json(json!({ "ok": true })))
}

/// `DELETE /api/admin/bike-units/:id` — remove one physical machine.
///
/// The FK from `bike_service_records` is `ON DELETE CASCADE`, so this also
/// destroys every record of work done to that machine. Unlike a family, a unit
/// has no reversible "stop renting it" flag to offer instead — `retired` is
/// that, and the sentence below says so, because the owner reaching for the bin
/// icon on a sold machine almost always wants `retired` and not this.
async fn admin_delete_unit(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;

    use crate::db::entities::bike_service_record::{Column as ServiceCol, Entity as ServiceEntity};
    use crate::db::entities::bike_unit::Entity as UnitEntity;
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

    let records = ServiceEntity::find()
        .filter(ServiceCol::BikeUnitId.eq(id.clone()))
        .count(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("admin_delete_unit(count records): {e:#}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?;
    if records > 0 {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "У байка {records} записей обслуживания — удаление сотрёт и их. \
                 Если байк продан или списан, поставьте статус «Выведен»."
            ),
        ));
    }

    let res = UnitEntity::delete_by_id(id.clone())
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("admin_delete_unit: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?;
    if res.rows_affected == 0 {
        return Err((StatusCode::NOT_FOUND, "Байк не найден".to_string()));
    }
    tracing::info!(unit_id = %id, "admin: unit deleted");
    Ok(Json(json!({ "ok": true })))
}

/// The write shape for one service line.
#[derive(Debug, serde::Deserialize)]
struct ServiceWriteRequest {
    bike_unit_id: String,
    service_type: String,
    #[serde(default)]
    current_km: Option<i32>,
    #[serde(default)]
    last_service_km: Option<i32>,
    #[serde(default)]
    interval_km: Option<i32>,
    /// The ops sheet keeps this as its own column rather than deriving it from
    /// `last_service_km + interval_km`, and so does the screen. Nothing here
    /// computes it: a derived number the owner did not type would silently
    /// disagree with the sheet he is copying from.
    #[serde(default)]
    next_km: Option<i32>,
    status: String,
}

/// `ok` | `due` | `overdue`, as `080_bike_service_records.sql` documents and the
/// admin screen's dropdown offers.
///
/// Unlike `bike_units.status` this is NOT a database CHECK — 080 leaves the
/// column free TEXT so the ops vocabulary can grow without a migration. The
/// validation therefore lives only here, and it is worth having: the screen
/// colours rows by these three words, and a fourth would render as no colour
/// at all with nothing to explain why.
const SERVICE_STATUSES: [&str; 3] = ["ok", "due", "overdue"];

fn validate_service_write(req: &ServiceWriteRequest) -> Result<(), String> {
    if req.bike_unit_id.trim().is_empty() {
        return Err("Выберите байк".to_string());
    }
    let kind = req.service_type.trim();
    if kind.is_empty() {
        return Err("Вид обслуживания обязателен".to_string());
    }
    if kind.chars().count() > 64 {
        return Err("Вид обслуживания длиннее 64 символов".to_string());
    }
    if !SERVICE_STATUSES.contains(&req.status.as_str()) {
        return Err(format!(
            "Статус «{}» неизвестен — допустимы: {}",
            req.status,
            SERVICE_STATUSES.join(", ")
        ));
    }
    for (label, km) in [
        ("Текущий пробег", req.current_km),
        ("Пробег прошлого ТО", req.last_service_km),
        ("Интервал", req.interval_km),
        ("Следующее ТО", req.next_km),
    ] {
        if km.is_some_and(|v| v < 0) {
            return Err(format!("{label} не может быть отрицательным"));
        }
    }
    Ok(())
}

/// `GET /api/admin/bike-service-records` — the whole service log, newest first.
///
/// **ADMIN ONLY (D6).** No catalog surface may read this: units read `overdue`
/// today, and a public "serviced" badge the shop cannot keep accurate is
/// precisely the number the data-honesty rule forbids.
async fn admin_list_service_records(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;

    use crate::db::entities::bike_service_record::{Column as ServiceCol, Entity as ServiceEntity};
    use sea_orm::{EntityTrait, QueryOrder};
    let records = ServiceEntity::find()
        .order_by_desc(ServiceCol::RecordedAt)
        .all(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("admin_list_service_records: {e:#}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?;

    let records: Vec<Value> = records
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "bike_unit_id": r.bike_unit_id,
                "service_type": r.service_type,
                "current_km": r.current_km,
                "last_service_km": r.last_service_km,
                "interval_km": r.interval_km,
                "next_km": r.next_km,
                "status": r.status,
                "recorded_at": r.recorded_at.to_rfc3339(),
            })
        })
        .collect();
    Ok(Json(json!({ "records": records })))
}

/// `POST /api/admin/bike-service-records` — log one piece of work.
///
/// Returns `recorded_at` as well as `id`: the screen inserts its row
/// optimistically with no timestamp and patches both from this response, so
/// the stamp the list sorts by is the server's, not the phone's clock.
async fn admin_create_service_record(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<ServiceWriteRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;
    validate_service_write(&req).map_err(|m| (StatusCode::BAD_REQUEST, m))?;

    use crate::db::entities::bike_service_record::{ActiveModel, Entity as ServiceEntity};
    use sea_orm::{ActiveValue::Set, EntityTrait};

    let id = uuid::Uuid::new_v4().to_string();
    let recorded_at: chrono::DateTime<chrono::FixedOffset> = chrono::Utc::now().into();
    let model = ActiveModel {
        id: Set(id.clone()),
        bike_unit_id: Set(req.bike_unit_id.trim().to_string()),
        service_type: Set(req.service_type.trim().to_string()),
        current_km: Set(req.current_km),
        last_service_km: Set(req.last_service_km),
        interval_km: Set(req.interval_km),
        next_km: Set(req.next_km),
        status: Set(req.status),
        recorded_at: Set(recorded_at),
    };
    ServiceEntity::insert(model)
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            if e.to_string().contains("foreign key") {
                return (
                    StatusCode::BAD_REQUEST,
                    "Байк не найден — обновите страницу".to_string(),
                );
            }
            tracing::error!("admin_create_service_record: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?;
    tracing::info!(record_id = %id, "admin: service record created");
    Ok(Json(json!({
        "id": id,
        "recorded_at": recorded_at.to_rfc3339(),
    })))
}

/// `DELETE /api/admin/bike-service-records/:id` — remove a mistyped line.
///
/// No confirmation gate of its own beyond the screen's modal: unlike a unit or
/// a family, a service record has nothing hanging off it, so this deletes one
/// row and only one.
async fn admin_delete_service_record(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;

    use crate::db::entities::bike_service_record::Entity as ServiceEntity;
    use sea_orm::EntityTrait;
    let res = ServiceEntity::delete_by_id(id.clone())
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("admin_delete_service_record: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?;
    if res.rows_affected == 0 {
        return Err((StatusCode::NOT_FOUND, "Запись не найдена".to_string()));
    }
    tracing::info!(record_id = %id, "admin: service record deleted");
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::{
        family_detail_json, family_json, query_flag, rental_terms_json, rollup_units,
        validate_bike_write, BikeWriteRequest, CatalogFilter, DoorAnswer, DoorAnswers, UnitRow,
        BIKE_CLASSES, CLIENT_RATE_FROM_DOOR, CLIENT_RATE_UNAVAILABLE, UNIT_STATUSES,
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
                for_sale: false,
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
        let value = family_json(
            &listing("x-adv-750", "motorcycle", None),
            &[],
            &silent_door(),
        )
        .expect("json");
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
        let value = family_json(
            &listing("nmax-155", "scooter", Some(449.0)),
            &[],
            &silent_door(),
        )
        .expect("json");
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
            &silent_door(),
        )
        .expect("json");
        assert_eq!(obj_of(&value).get("class_discount"), Some(&Value::Null));
    }

    #[test]
    fn the_published_class_discount_is_attached_from_the_ladder() {
        let value = family_json(
            &listing("nmax-155", "scooter", Some(939.0)),
            &scooter_ladder(),
            &silent_door(),
        )
        .expect("json");
        assert_eq!(obj_of(&value).get("class_discount"), Some(&json!(0.25)));
    }

    // ── D11: the door seam ───────────────────────────────────────

    /// The door as it stands today, and as it stands whenever the owner does
    /// not answer: an empty batch. Every key resolves to silence.
    fn silent_door() -> DoorAnswers {
        DoorAnswers::default()
    }

    /// A door that quoted one family. This is what the tests below need and the
    /// reason `DoorAnswers` carries a map rather than a bare "wired" flag —
    /// the priced branch has to be reachable from a test years before the live
    /// sheet is reachable from the server.
    fn door_quoting(key: &str, thb: f64) -> DoorAnswers {
        let mut answers = HashMap::new();
        answers.insert(key.to_string(), DoorAnswer::Quoted(thb));
        DoorAnswers(answers)
    }

    #[test]
    fn a_silent_door_yields_no_number_and_the_unavailable_source() {
        // The client-facing price is absent, NOT base_rate * (1 - discount).
        assert_eq!(
            silent_door().resolve("nmax-155"),
            (None, CLIENT_RATE_UNAVAILABLE)
        );
        assert_eq!(
            door_quoting("nmax-155", 337.0).resolve("nmax-155"),
            (Some(337.0), CLIENT_RATE_FROM_DOOR)
        );
        // Absent-means-silent. This is the assertion that makes today's empty
        // batch *correct* rather than a stub waiting to be special-cased: a
        // family the door did not answer for is in exactly the same state as a
        // family it answered nothing for.
        assert_eq!(
            door_quoting("nmax-155", 337.0).resolve("pcx-160"),
            (None, CLIENT_RATE_UNAVAILABLE)
        );
    }

    #[test]
    fn a_family_with_both_inputs_still_serves_no_computed_price() {
        // 939 with a 25% class discount is the seed's reconciled row: the
        // owner's sheet quotes 704 and the arithmetic gives 704.25. Both
        // numbers are computable from this payload and NEITHER may be in it
        // while the door is silent — `db::bikes::apply_class_discount` exists
        // for the door's answer, not for its silence.
        let value = family_json(
            &listing("nmax-155", "scooter", Some(939.0)),
            &scooter_ladder(),
            &silent_door(),
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

    #[test]
    fn a_door_quote_of_the_computable_number_reaches_only_the_client_rate_field() {
        // The sibling above is the sharpest anti-computation gate in the repo
        // and also its sharpest trap: 704 is a number the *door* may legitimately
        // quote for this family, so a body-wide ban on 704 would go red on a
        // correct answer. The rule D11 actually carries is narrower — no
        // computed price when the source is not the door — so the ban is stated
        // per-source rather than relaxed.
        let value = family_json(
            &listing("nmax-155", "scooter", Some(939.0)),
            &scooter_ladder(),
            &door_quoting("nmax-155", 704.0),
        )
        .expect("json");
        let obj = obj_of(&value);
        assert_eq!(obj.get("client_rate_thb_day"), Some(&json!(704.0)));
        assert_eq!(
            obj.get("client_rate_source"),
            Some(&json!(CLIENT_RATE_FROM_DOOR))
        );
        for (name, field) in obj {
            if name == "client_rate_thb_day" {
                continue;
            }
            if let Some(n) = field.as_f64() {
                assert!(
                    (n - 704.25).abs() > 1e-9 && (n - 704.0).abs() > 1e-9,
                    "`{name}` carries the door's price in a field that is not the door's: {n}"
                );
            }
        }
    }

    #[test]
    fn a_diverging_door_quote_still_reaches_the_wire() {
        // The file implies 449 x 0.75 = 336.75 -> 337; the door says 449. That
        // is a 112-baht divergence, far over the >= 1 baht threshold, so the
        // log fires. D11 is explicit that neither number is corrected to match
        // the other and the answer still returns — this is the half of #25's
        // third criterion that nothing in the tree covered.
        let value = family_json(
            &listing("nmax-155", "scooter", Some(449.0)),
            &scooter_ladder(),
            &door_quoting("nmax-155", 449.0),
        )
        .expect("a divergence must never fail the request");
        let obj = obj_of(&value);
        assert_eq!(obj.get("client_rate_thb_day"), Some(&json!(449.0)));
        assert_eq!(
            obj.get("client_rate_source"),
            Some(&json!(CLIENT_RATE_FROM_DOOR))
        );
        // And the file's own number is still served verbatim beside it, for the
        // admin surface and for whoever reconciles the sheet.
        assert_eq!(obj.get("base_rate_thb_day"), Some(&json!(449.0)));
    }

    #[test]
    fn click_125_never_carries_a_client_rate_even_when_the_door_answers() {
        // D12 plus #25's fifth criterion. CLICK 125 is closed to new rentals;
        // the seasonal grid's 187/day for it is stale, not a tariff. A family
        // that is not offered is not priced, whatever the door says.
        let mut closed = listing("click-125", "scooter", Some(249.0));
        closed.bike.offered = false;
        let value = family_json(
            &closed,
            &scooter_ladder(),
            &door_quoting("click-125", 187.0),
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
                    (n - 187.0).abs() > 1e-9,
                    "`{name}` carries a price for a family closed to new rentals: {n}"
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
            &silent_door(),
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
        let value = family_detail_json(
            &listing("nmax-155", "scooter", None),
            &[],
            &units,
            &silent_door(),
        )
        .expect("json");
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
        let value = family_json(&l, &scooter_ladder(), &silent_door()).expect("json");
        assert_eq!(obj_of(&value).get("offered"), Some(&json!(false)));
    }

    #[test]
    fn the_wire_always_carries_for_sale() {
        // The UI's `Bike` struct has declared `for_sale: Option<bool>` since
        // the sale block was written, and until `086_bikes_for_sale.sql` there
        // was no column behind it — so serde filled it with `None` on every
        // response and `for_sale == Some(true)` in `bike_detail.rs` could
        // never be true. A struct field with no producer is indistinguishable
        // from a working one at the call site, which is exactly why this
        // asserts key PRESENCE and not just a value: the failure mode being
        // guarded is an absent key reading as a legitimate "we don't sell it".
        let mut l = listing("xmax-300-new", "motorcycle", Some(900.0));
        let value = family_json(&l, &scooter_ladder(), &silent_door()).expect("json");
        assert_eq!(obj_of(&value).get("for_sale"), Some(&json!(false)));

        l.bike.for_sale = true;
        let value = family_json(&l, &scooter_ladder(), &silent_door()).expect("json");
        assert_eq!(obj_of(&value).get("for_sale"), Some(&json!(true)));
        // …and with no published asking price, which is the state D9/D11
        // require to stay expressible: "we sell this one, ask us the price".
        assert_eq!(obj_of(&value).get("sale_price_thb"), Some(&json!(null)));
    }

    // ── admin write surface ──────────────────────────────────────

    fn write_req() -> BikeWriteRequest {
        BikeWriteRequest {
            key: Some("nmax-155".to_string()),
            brand: "Yamaha".to_string(),
            model: "NMAX 155".to_string(),
            variant_label: None,
            class: "scooter".to_string(),
            body: "scooter".to_string(),
            displacement_cc: 155,
            base_rate_thb_day: Some(400.0),
            deposit_thb: None,
            monthly_low_season_thb: None,
            sale_price_thb: None,
            offered: true,
            for_sale: false,
            sort_order: None,
            description_ru: None,
            description_en: None,
            image_url: None,
        }
    }

    #[test]
    fn the_accepted_classes_are_the_ones_the_column_allows() {
        // `BIKE_CLASSES` is a second copy of the CHECK constraint, and a
        // second copy of a list is the defect this repository keeps finding.
        // So it is read back out of the migration rather than retyped: adding
        // a third class in SQL and forgetting it here now fails the build's
        // test step instead of producing 500s that name a Postgres constraint.
        let sql = include_str!("../../migrations/077_bikes.sql");
        let clause = sql
            .lines()
            .find(|l| l.contains("CHECK (class IN ("))
            .expect("077_bikes.sql constrains `class`");
        for class in BIKE_CLASSES {
            assert!(
                clause.contains(&format!("'{class}'")),
                "`{class}` is accepted by the API and not by the column: {clause}"
            );
        }
        // …and the other direction, which is the one that actually bites: a
        // class the column allows but the API refuses is a family the owner
        // cannot add at all.
        let in_sql = clause
            .split_once("CHECK (class IN (")
            .map(|(_, rest)| rest)
            .and_then(|rest| rest.split_once("))"))
            .map(|(inner, _)| inner)
            .expect("the CHECK clause closes on the same line");
        let sql_classes: Vec<String> = in_sql
            .split(',')
            .map(|s| s.trim().trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(sql_classes.len(), BIKE_CLASSES.len(), "class list drifted");
        for class in &sql_classes {
            assert!(
                BIKE_CLASSES.contains(&class.as_str()),
                "the column allows `{class}` and the API rejects it"
            );
        }
    }

    #[test]
    fn a_valid_family_passes_and_is_trimmed() {
        let mut r = write_req();
        r.brand = "  Yamaha  ".to_string();
        r.description_ru = Some("   ".to_string());
        let out = validate_bike_write(&r).expect("valid");
        assert_eq!(out.brand, "Yamaha");
        // Whitespace-only optional text becomes absent, not an empty string:
        // D9 draws the line at "absent stays absent", and `Some("")` is a
        // published empty description, which renders as a blank paragraph.
        assert_eq!(out.description_ru, None);
    }

    #[test]
    fn an_unknown_class_is_refused_by_name() {
        let mut r = write_req();
        r.class = "quadbike".to_string();
        let err = validate_bike_write(&r).expect_err("rejected");
        assert!(
            err.contains("scooter"),
            "the message names the options: {err}"
        );
    }

    #[test]
    fn a_key_that_is_not_a_url_segment_is_refused() {
        for bad in ["NMAX 155", "nmax/155", "nmax_155", "nmax?155"] {
            let mut r = write_req();
            r.key = Some(bad.to_string());
            assert!(
                validate_bike_write(&r).is_err(),
                "`{bad}` must not become a `/api/bikes/:key` path segment"
            );
        }
    }

    #[test]
    fn an_absent_key_validates_and_is_the_update_shape() {
        // The edit card sends no key, because a family's key is a live cart's
        // `catalog_id` (D8) and renaming it orphans those lines. Validation
        // must therefore accept a body without one — the create handler is
        // where the requirement lives, and only there.
        let mut r = write_req();
        r.key = None;
        assert_eq!(validate_bike_write(&r).expect("valid").key, None);
        r.key = Some("   ".to_string());
        assert_eq!(
            validate_bike_write(&r).expect("valid").key,
            None,
            "a whitespace key is absent, not a family named ' '"
        );
    }

    #[test]
    fn an_unusable_price_is_refused_rather_than_stored() {
        // The read path filters NaN to `None` because a stored NaN is damage
        // to survive. The write path must not do that: filtering here would
        // accept the submission, drop the number silently, and leave the owner
        // looking at a card that says a dash while his form said 400.
        for bad in [f64::NAN, f64::INFINITY, -1.0] {
            let mut r = write_req();
            r.base_rate_thb_day = Some(bad);
            assert!(validate_bike_write(&r).is_err(), "{bad} must be refused");
        }
        let mut r = write_req();
        r.deposit_thb = None;
        assert_eq!(
            validate_bike_write(&r).expect("valid").deposit_thb,
            None,
            "an absent price stays absent — never 0 (D9)"
        );
    }

    #[test]
    fn a_family_can_be_for_sale_with_no_asking_price() {
        // The state issue #11 exists to make expressible: on the forecourt,
        // price on request. If validation ever starts requiring a price
        // alongside the flag, the shop is forced to invent a number (D11).
        let mut r = write_req();
        r.for_sale = true;
        r.sale_price_thb = None;
        let out = validate_bike_write(&r).expect("for sale without a price is valid");
        assert!(out.for_sale);
        assert_eq!(out.sale_price_thb, None);
    }

    #[test]
    fn a_javascript_url_never_reaches_the_image_field() {
        let mut r = write_req();
        r.image_url = Some("javascript:alert(1)".to_string());
        assert!(validate_bike_write(&r).is_err());
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

    // ── CatalogFilter ────────────────────────────────────────────
    //
    // `#8` names class and displacement filters in its title. Before this,
    // `GET /api/bikes` read `available_only` and nothing else, and because
    // Axum hands unknown query keys straight through, `?class=motorcycle`
    // returned all thirteen families under a `200` — measured against
    // production. That is the worst of the three possible answers: a client
    // that trusts the parameter shows scooters on a motorcycle page and has no
    // way to know it was ignored.

    #[test]
    fn an_absent_query_filters_nothing() {
        let f = CatalogFilter::parse(&q(&[])).expect("no params is a valid request");
        assert!(f.is_empty());
        assert!(f.matches("scooter", 125));
        assert!(f.matches("motorcycle", 750));
        assert_eq!(
            f.cache_suffix(),
            "",
            "an empty filter owns the unfiltered key"
        );
    }

    #[test]
    fn available_only_alone_is_not_a_catalog_filter() {
        // It selects on fleet state, not on the family's own columns, and it
        // is applied in SQL. If it leaked into the suffix, the two filters
        // would be spelled into the cache key twice.
        let f = CatalogFilter::parse(&q(&[("available_only", "true")]))
            .expect("available_only is handled elsewhere, not rejected here");
        assert!(f.is_empty());
    }

    #[test]
    fn class_is_matched_case_insensitively() {
        let f = CatalogFilter::parse(&q(&[("class", "Motorcycle")])).expect("valid class");
        assert!(f.matches("motorcycle", 750));
        assert!(!f.matches("scooter", 125));
    }

    #[test]
    fn an_unknown_class_is_named_rather_than_ignored() {
        // The whole point of the change: a typo must not read as "no filter".
        let err = CatalogFilter::parse(&q(&[("class", "moped")]))
            .expect_err("`moped` is not a value this column can hold");
        assert!(
            err.contains("moped"),
            "the error repeats what was sent: {err}"
        );
        assert!(err.contains("class"), "and names the parameter: {err}");
    }

    #[test]
    fn the_filter_accepts_exactly_the_classes_the_column_allows() {
        // Third copy of the closed domain — but not a fourth *list*: it walks
        // `BIKE_CLASSES`, which `the_accepted_classes_are_the_ones_the_column_allows`
        // already pins to the migration's CHECK constraint.
        for class in BIKE_CLASSES {
            assert!(
                CatalogFilter::parse(&q(&[("class", class)])).is_ok(),
                "{class} is a legal value of bikes.class but the filter refused it"
            );
        }
    }

    #[test]
    fn displacement_bounds_are_inclusive_at_both_ends() {
        // A customer asking for 150cc-and-up means to see the 150.
        let f = CatalogFilter::parse(&q(&[("min_cc", "150"), ("max_cc", "155")]))
            .expect("a valid band");
        assert!(f.matches("scooter", 150), "the floor is part of the band");
        assert!(f.matches("scooter", 155), "so is the ceiling");
        assert!(!f.matches("scooter", 149));
        assert!(!f.matches("scooter", 156));
    }

    #[test]
    fn a_non_numeric_displacement_is_rejected() {
        let err =
            CatalogFilter::parse(&q(&[("min_cc", "150cc")])).expect_err("`150cc` is not a number");
        assert!(err.contains("min_cc"), "{err}");
    }

    #[test]
    fn an_inverted_band_is_refused_rather_than_answered_with_nothing() {
        // An empty list here would be indistinguishable from "we rent no bikes
        // in that band", which is a different and much worse thing to say.
        let err = CatalogFilter::parse(&q(&[("min_cc", "400"), ("max_cc", "125")]))
            .expect_err("no displacement is both ≥400 and ≤125");
        assert!(err.contains("400") && err.contains("125"), "{err}");
    }

    #[test]
    fn distinct_selections_get_distinct_cache_keys() {
        // The ETag guard: two different bodies must never share a key, or one
        // selection is served under the other's `304`.
        let keys: Vec<String> = [
            vec![],
            vec![("class", "scooter")],
            vec![("class", "motorcycle")],
            vec![("min_cc", "150")],
            vec![("max_cc", "150")],
            vec![("class", "scooter"), ("min_cc", "150")],
        ]
        .into_iter()
        .map(|pairs| {
            CatalogFilter::parse(&q(&pairs))
                .expect("all six are valid")
                .cache_suffix()
        })
        .collect();

        let unique: std::collections::HashSet<&String> = keys.iter().collect();
        assert_eq!(
            unique.len(),
            keys.len(),
            "two selections collapsed to one cache key: {keys:?}"
        );
    }
}
