use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::error;

use crate::api::auth::{check_admin, check_not_blocked, check_owner, validate_telegram_id_param};
use crate::api::rate_limit::{
    check_and_record, client_ip_from_headers, new_store, SlidingWindowStore,
};
use crate::db::entities::bonus_transaction::{Column as BtCol, Entity as BtEntity};
use crate::db::orders::{BikeDeal, BikeLine, DepositForm, Order, OrderItem};
use crate::promptpay::{build_payload, svg_qr};
use crate::trios::pricing::{effective_accessory_price, effective_set_price, effective_tea_price};
use crate::AppState;
use sea_orm::{ColumnTrait, QueryFilter};
use std::collections::HashMap;
use std::time::Duration;

/// Cycle #105: per-IP rate-limit for anonymous order creation.
///
/// Authenticated orders (with `telegram_id`) already get serialised by
/// `pg_advisory_xact_lock($tid)` + a 1-min DB-driven `LIMIT 1` check
/// (line ~739 in `create_order`). Anonymous orders bypass both —
/// `tid.is_none()` means no advisory lock target and no DB rate-limit.
/// Without this guard an attacker could spray anonymous orders to fill
/// the `orders` table.
///
/// The `rate_limit_blocked("anon_order")` metric was declared in
/// `src/metrics.rs:39` at audit time but never wired (the existing
/// `client_ip_from_headers` + `check_and_record` were only used by
/// `api/upload.rs`). This static + the gate in `create_order` close
/// that gap.
///
/// Limit: 3 anonymous orders per minute per IP — generous for normal
/// browser-direct paths, tight enough to bound spray damage. 10k IPs
/// tracked at peak before the per-key eviction kicks in (~600KB).
static ANON_ORDER_RATE_LIMIT: std::sync::LazyLock<SlidingWindowStore> =
    std::sync::LazyLock::new(new_store);
const ANON_ORDER_RL_WINDOW: Duration = Duration::from_secs(60);
const ANON_ORDER_RL_MAX_ATTEMPTS: usize = 3;
const ANON_ORDER_RL_MAX_IPS: usize = 10_000;

#[derive(Debug, Deserialize)]
pub(crate) struct CreateOrderRequest {
    pub telegram_id: Option<i64>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub customer_telegram: Option<String>,
    pub items: Vec<OrderItem>,
    pub subtotal: f64,
    pub bonus_used: Option<f64>,
    /// Stars (⭐) the user wants to spend as an internal-currency discount.
    /// 1 Star = 1 THB. Server-side balance is debited atomically; the client
    /// only proposes the amount, it never sets the fiat discount itself.
    #[serde(default)]
    pub stars_used: Option<i64>,
    pub total: f64,
    pub shop_id: Option<String>,
    #[serde(default)]
    pub delivery_address: Option<String>,
    #[serde(default)]
    pub delivery_notes: Option<String>,
    // D5: `garden_reward_id` is gone with the garden mechanic. The field was
    // `#[serde(default)]`, and `CreateOrderRequest` does not deny unknown
    // fields, so a client still sending the key gets it ignored rather than a
    // 400 — which is what the old WASM checkout screen does until it is
    // rebuilt.
    /// Loop #7: explicit per-order age confirmation (20+). The server
    /// rejects the request unless `true` — client-only UI checks are not
    /// enough for compliance.
    #[serde(default)]
    pub age_confirmed: Option<bool>,
    /// Loop #7: chosen delivery zone id from the public `/api/delivery/zones`
    /// list. Stored on the order so the ETA/fee are authoritative on the
    /// server and the success screen can poll them.
    #[serde(default)]
    pub delivery_zone_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateOrderStatusRequest {
    pub status: String,
    #[allow(dead_code)]
    pub admin_telegram_id: Option<i64>,
}

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/orders", post(create_order))
        .route("/orders", get(get_orders))
        .route("/orders/:id", get(get_order))
        .route("/orders/:id/details", get(get_order_details))
        .route("/orders/:id/status", get(get_order_status))
        .route("/orders/:id/status", put(update_order_status))
        .route("/orders/:id/cancel", post(cancel_order))
        .route("/orders/:id/promptpay-qr", get(promptpay_qr))
        .route("/orders/user/:telegram_id", get(get_user_orders))
        .route("/delivery/zones", get(list_delivery_zones))
}

/// Sanitized payment inputs returned by [`validate_create_order`].
#[derive(Debug)]
struct ValidatedPayment {
    bonus_used: f64,
    stars_used: i64,
}

/// Validates a CreateOrderRequest. Returns the sanitized bonus_used and
/// stars_used on success.
fn validate_create_order(req: &CreateOrderRequest) -> Result<ValidatedPayment, StatusCode> {
    if let Some(ref name) = req.customer_name {
        if name.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref phone) = req.customer_phone {
        if phone.len() > 50 {
            return Err(StatusCode::BAD_REQUEST);
        }
        // Accept every shape a customer's own phone displays — local Thai
        // `081…`, Russian `8…`, IDD `0066…` — and reject only what cannot be
        // a phone number. Requiring a literal leading '+' here (and in the UI
        // gate that mirrors it) is what made the order button unclickable for
        // anyone typing their number the normal way.
        if crate::trios::store::normalize_phone(phone).is_none() {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref tg) = req.customer_telegram {
        if tg.len() > 100 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref shop_id) = req.shop_id {
        if shop_id.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    // Loop #7: explicit age confirmation is mandatory per order.
    if req.age_confirmed != Some(true) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    if let Some(ref zid) = req.delivery_zone_id {
        if zid.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if req.items.is_empty() || req.items.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.items.iter().any(|i| {
        i.strain_id.as_ref().is_some_and(|n| n.len() > 200)
            || i.strain_name.as_ref().is_some_and(|n| n.len() > 200)
            || i.accessory_id.as_ref().is_some_and(|n| n.len() > 200)
            || i.accessory_name.as_ref().is_some_and(|n| n.len() > 200)
            || i.tea_id.as_ref().is_some_and(|n| n.len() > 200)
            || i.tea_name.as_ref().is_some_and(|n| n.len() > 200)
            || i.set_id.as_ref().is_some_and(|n| n.len() > 200)
            || i.set_name.as_ref().is_some_and(|n| n.len() > 200)
    }) {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Cycle #152: upper bounds. Pre-cycle the per-item quantity was
    // only `> 0`, so a request with `quantity = 1e18` would pass the
    // boundary validator. The server-side price authority would then
    // recompute subtotal at the same inflated number (DB price ×
    // quantity), the comparison passes, and a `1e20 ฿` order lands
    // in the DB + admin chat. Cap per-item quantity at 10_000 (10kg
    // of weed is already implausible for a single line, but bulk-tea
    // orders could plausibly be hundreds of boxes — pick a domain
    // ceiling that's far above any real order). Cap subtotal/total
    // at 100M ฿ — the theoretical max for 100 items × 1M ฿/item.
    const MAX_ITEM_QUANTITY: f64 = 10_000.0;
    const MAX_ORDER_TOTAL: f64 = 100_000_000.0;
    if req
        .items
        .iter()
        .any(|i| !i.quantity.is_finite() || i.quantity <= 0.0 || i.quantity > MAX_ITEM_QUANTITY)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !req.total.is_finite() || req.total < 0.0 || req.total > MAX_ORDER_TOTAL {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !req.subtotal.is_finite() || req.subtotal < 0.0 || req.subtotal > MAX_ORDER_TOTAL {
        return Err(StatusCode::BAD_REQUEST);
    }
    let bonus_used = req.bonus_used.unwrap_or(0.0).max(0.0);
    if !bonus_used.is_finite() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if bonus_used > req.subtotal + 0.01 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let stars_used = req.stars_used.unwrap_or(0).max(0);
    const MAX_STARS_PER_TX: i64 = 1_000_000;
    if stars_used > MAX_STARS_PER_TX {
        return Err(StatusCode::BAD_REQUEST);
    }
    let stars_discount = stars_used as f64;
    if stars_discount > req.subtotal - bonus_used + 0.01 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let expected_total = (req.subtotal - bonus_used - stars_discount).max(0.0);
    // D5: the garden reward used to relax this into an inequality ("a reward
    // can only lower the total"), with the exact figure re-checked later in
    // `create_order`. With the mechanic gone the equality is strict again and
    // there is no second, looser gate anywhere behind it.
    if (req.total - expected_total).abs() > 0.01 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(ValidatedPayment {
        bonus_used,
        stars_used,
    })
}

/// Upper bound on units of one family on a single line.
///
/// The whole fleet is 37 units across 14 families and the largest family in
/// `data/fleet_seed.json` holds 10 (`nmax-155`), so 20 cannot refuse a real
/// booking while still rejecting the absurd. The generic `MAX_ITEM_QUANTITY`
/// of 10 000 above is a grams/boxes ceiling and means nothing for machines.
const MAX_BIKE_UNITS_PER_LINE: f64 = 20.0;

/// Upper bound on a rental term, in days. The seed's longest observed term is
/// 180 days; a year is the outer edge of plausible and keeps a typo'd date
/// (`2206-09-14`) from being stored as a 180-year booking.
const MAX_RENTAL_DAYS: i64 = 365;

/// Upper bound on any single quoted THB figure on a bike line (per-day rate,
/// deposit, sale price). The most expensive family rents at 2 788 ฿/day and
/// the largest published deposit is 25 000 ฿; a used bike sells for a few
/// hundred thousand. One million bounds all three without touching reality.
const MAX_QUOTED_THB: f64 = 1_000_000.0;

/// Shop-local today on the declared market's clock, used to reject a rental
/// that starts in the past.
///
/// `FixedOffset` rather than a tz database: the crate has no `chrono-tz`, and
/// the market profile records whether that reduction is legitimate
/// (`dst_observed`). This used to be a third copy of `7 * 3600`; the doc
/// comment it replaces said so itself — "which is also how `trios::promo` and
/// `trios::happy_hour` read the shop's clock" — and that sentence is the reason
/// D18's own list of these sites was two short.
///
/// `None` is handled rather than unwrapped (`clippy::unwrap_used`) by falling
/// back to the UTC date — which in the hour before local midnight is yesterday,
/// i.e. it errs towards accepting a booking, never towards refusing a valid one.
fn shop_today() -> chrono::NaiveDate {
    match crate::trios::market::MARKET.now() {
        Some(local) => local.date_naive(),
        None => chrono::Utc::now().date_naive(),
    }
}

/// Boundary validation for the bike half of every line — pure, so the rules
/// are unit-testable without a database. `today` is the shop-local date
/// ([`shop_today`] in production, a fixture in tests).
///
/// What it deliberately does NOT do: price anything. No rate is multiplied by
/// a span, no deposit is derived from a tariff. D11 gives the quote to the
/// door and D9 says an unquoted figure stays absent, so every money field
/// here is optional and only *bounded* when present.
fn validate_bike_lines(items: &[OrderItem], today: chrono::NaiveDate) -> Result<(), StatusCode> {
    // A quoted THB figure must be a real, positive number when it is present.
    // `None` is legitimate (the door was silent); `Some(0.0)` is not — a zero
    // rate reads as "free" and a zero deposit as "nothing owed" (D9).
    let quoted_ok = |v: Option<f64>| -> bool {
        match v {
            Some(x) => x.is_finite() && x > 0.0 && x <= MAX_QUOTED_THB,
            None => true,
        }
    };
    for item in items {
        let Some(BikeLine {
            bike_key,
            bike_name,
            deal,
        }) = item.bike.as_ref()
        else {
            continue;
        };
        if bike_key.is_empty() || bike_key.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
        if bike_name.as_ref().is_some_and(|n| n.len() > 200) {
            return Err(StatusCode::BAD_REQUEST);
        }
        // One line, one thing. A line carrying both a bike and a catalog id is
        // malformed: the subtotal would price it as a helmet while the
        // handover paperwork described a machine. Empty strings are treated as
        // absent — old clients send `""` where they mean `null`.
        let non_empty = |v: &Option<String>| v.as_deref().is_some_and(|s| !s.is_empty());
        if non_empty(&item.strain_id)
            || non_empty(&item.accessory_id)
            || non_empty(&item.tea_id)
            || non_empty(&item.set_id)
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        // Units, not grams: a line for 1.5 scooters cannot be handed over.
        // (`validate_create_order` has already ruled out non-finite and
        // non-positive quantities.)
        if item.quantity.fract() != 0.0 || item.quantity > MAX_BIKE_UNITS_PER_LINE {
            return Err(StatusCode::BAD_REQUEST);
        }
        // D7: a bike is handed over at the office or delivered; the legacy
        // drink vocabulary ("dine_in" / "takeaway") must not leak onto one.
        if let Some(f) = item.fulfillment.as_deref() {
            if !matches!(f, "delivery" | "pickup") {
                return Err(StatusCode::BAD_REQUEST);
            }
        }
        match deal {
            BikeDeal::BikeRental {
                rental_start,
                rental_end,
                rate_thb_day,
                deposit,
            } => {
                if rental_end < rental_start {
                    return Err(StatusCode::BAD_REQUEST);
                }
                // Both ends inclusive, same as `BikeDeal::span_days`.
                if (*rental_end - *rental_start).num_days() + 1 > MAX_RENTAL_DAYS {
                    return Err(StatusCode::BAD_REQUEST);
                }
                if rental_start < &today {
                    // Not malformed — a stale cart left open overnight lands
                    // here — so it is a 422, like the other "you cannot have
                    // this" refusals.
                    return Err(StatusCode::UNPROCESSABLE_ENTITY);
                }
                if !quoted_ok(*rate_thb_day) {
                    return Err(StatusCode::BAD_REQUEST);
                }
                match deposit {
                    Some(DepositForm::Money {
                        amount,
                        currency,
                        method,
                    }) => {
                        if !quoted_ok(*amount) {
                            return Err(StatusCode::BAD_REQUEST);
                        }
                        // "THB", or the USD/EUR equivalent the seed allows.
                        // CORRECTED 2026-09-24 (T27 C3): only the code's shape
                        // is bounded here. On the order path a FIGURE in any
                        // currency but THB is then refused (`deposit_refusal`,
                        // `DepositRefusal::NotComparable`) until the owner
                        // decides how a foreign deposit is recorded; only the
                        // money form with no figure passes in another currency.
                        if currency
                            .as_ref()
                            .is_some_and(|c| c.is_empty() || c.len() > 8)
                        {
                            return Err(StatusCode::BAD_REQUEST);
                        }
                        // How it must be returned: cash THB, bank transfer,
                        // USDT. Free text, because the door agrees the wording
                        // — bounded, not enumerated.
                        if method
                            .as_ref()
                            .is_some_and(|m| m.is_empty() || m.len() > 50)
                        {
                            return Err(StatusCode::BAD_REQUEST);
                        }
                    }
                    // The passport carries no amount by construction — that is
                    // the whole point of the enum.
                    Some(DepositForm::Passport) | None => {}
                }
            }
            BikeDeal::BikeSale { price_thb } if !quoted_ok(*price_thb) => {
                return Err(StatusCode::BAD_REQUEST);
            }
            // Retired 2026-09-24 (owner: rental only). Placed sale lines still read.
            BikeDeal::BikeSale { .. } => return Err(StatusCode::UNPROCESSABLE_ENTITY),
        }
    }
    Ok(())
}

/// Load the configured max share of an order that can be paid with bonus
/// balance.
///
/// A missing singleton and an unusable value are the same answer — the standing
/// default in `trios::loyalty` — so a DB glitch never opens the ceiling to
/// 100 %. Both the number and the range that decides "unusable" used to be
/// written out here, a third of the file away from the two other copies.
async fn load_max_bonus_usage_pct(orm: &sea_orm::DatabaseConnection) -> f64 {
    use crate::db::entities::loyalty_config::Entity as LcEntity;
    use sea_orm::EntityTrait;
    let Some(model) = (match LcEntity::find_by_id(1).one(orm).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("load_max_bonus_usage_pct: {e}");
            None
        }
    }) else {
        return crate::trios::loyalty::default_f64("max_bonus_usage_pct");
    };
    crate::trios::loyalty::max_bonus_usage_pct(&model.config)
}

/// True if `k` is a syntactically acceptable `X-Idempotency-Key` value.
/// Accepts 1-100 ASCII alphanumeric / `-` / `_` characters — fits UUIDs
/// (`xxxxxxxx-xxxx-...`), nanoids, and short random strings. Rejects spaces,
/// control bytes, slashes, quotes, and anything that could smuggle SQL or
/// header-injection. Defence is shallow but cheap, and a malformed key is
/// almost always a buggy client rather than a legitimate one.
pub(crate) fn is_valid_idempotency_key(k: &str) -> bool {
    !k.is_empty()
        && k.len() <= 100
        && k.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

// The strain-only `SubtotalCheck` / `check_strain_subtotal` pair (cycle #56,
// kept as a focused unit-test target after cycle #78 superseded it) left with
// the `strains` table: `083_drop_cannabis_catalog.sql` drops it, so there is
// no row for the helper to price and nothing for a test to fixture.

/// Catalog lookups for `check_full_subtotal` (cycle #58 / C). Built once per
/// order from the catalog SELECTs and handed in by reference.
///
/// The `strains` map went with the table (`083_drop_cannabis_catalog.sql`).
/// Accessories, tea and sets survive the rebrand and still price themselves
/// from the DB; bikes are not in here at all, because a bike line contributes
/// nothing to the authoritative subtotal — see [`check_full_subtotal`].
#[derive(Debug, Default)]
pub(crate) struct PriceCatalog<'a> {
    /// `id → (price, is_available)`.
    pub accessories: HashMap<&'a str, (f64, bool)>,
    pub tea_products: HashMap<&'a str, (f64, bool)>,
    /// `id → (total_price, discount_percent, is_available)` — same shape for
    /// `sets`, `accessory_sets`, and `tea_sets` (UUID primary keys don't
    /// collide across the three tables, so one map covers them all).
    pub sets: HashMap<&'a str, (f64, f64, bool)>,
}

/// Result of the full server-side price-authority check (cycle #58 / C).
/// Strict equality across every priced catalog combined.
#[derive(Debug, PartialEq)]
pub(crate) enum FullSubtotalCheck {
    Ok,
    /// `catalog` is one of `"accessories" | "tea_products" | "sets"` so audit
    /// logs can pinpoint which catalog the missing id belongs to. A missing
    /// bike family is reported by [`check_bike_lines`] with `"bikes"`, not
    /// from here — this helper is pure and never touches the fleet tables.
    UnknownItem {
        catalog: &'static str,
        id: String,
    },
    /// Item exists but `is_available = false` — admin turned it off since
    /// the customer last fetched the menu. Distinct from `UnknownItem` so
    /// the warn-vs-info severity in audit logs reflects fraud likelihood.
    Unavailable {
        catalog: &'static str,
        id: String,
    },
    /// Server-computed subtotal differs from client-claimed by more than
    /// `tolerance`. Internal-only — do **not** echo `expected` to the
    /// client (anti price-probing).
    Mismatch {
        claimed: f64,
        expected: f64,
    },
    /// Line item is missing every `*_id` and carries no bike — malformed
    /// payload.
    Malformed,
}

/// Server-authoritative subtotal check for every priced catalog: accessory,
/// tea product, set. Pure helper — caller fetches the rows in advance.
///
/// Routing precedence (first match per item):
///   1. `accessory_id`  → accessories
///   2. `tea_id`        → tea_products
///   3. `set_id`        → sets / accessory_sets / tea_sets (unified by UUID)
///   4. `bike`          → contributes **zero**, see below
///
/// A bike line is deliberately last so that a line carrying both a catalog id
/// and a bike is still priced by the catalog: putting the bike branch first
/// would let a client attach an empty rental to an accessory and get it for
/// nothing.
///
/// **Why a bike contributes zero.** A rental is not priced by this server at
/// all. The tariff in `data/fleet_seed.json` is pre-class-discount, the term
/// bands are ranges rather than multipliers, and D11 gives the number to the
/// door — so `rate_thb_day × days` is precisely the invented figure that is
/// forbidden. The deposit is returnable and is not revenue either. Both ride
/// along on the line as the record of what was agreed, and the money that
/// changes hands at the office is not this subtotal's business. The line is
/// therefore skipped rather than treated as `Malformed`, and a bike-only cart
/// legitimately claims a subtotal of 0.
///
/// Strict equality required: a mixed order with an unauthorised price on any
/// line item fails the check even if other lines compensate.
pub(crate) fn check_full_subtotal(
    items: &[OrderItem],
    catalog: &PriceCatalog<'_>,
    claimed_subtotal: f64,
    tolerance: f64,
) -> FullSubtotalCheck {
    let mut sum = 0.0_f64;
    for item in items {
        // Fail closed on a quantity that is not a number. The boundary
        // validator has already rejected those, but the old `else { 0.0 }`
        // here meant that if one ever slipped through, the line would price
        // itself at zero and a claimed subtotal of zero would be accepted.
        if !item.quantity.is_finite() || item.quantity < 0.0 {
            return FullSubtotalCheck::Malformed;
        }
        let qty = item.quantity;
        // An empty id string counts as absent, matching `validate_bike_lines`:
        // old clients send `""` where they mean `null`, and routing such a
        // line into the accessory branch would refuse it as an unknown item
        // with an empty id in the audit log. A non-bike line with nothing but
        // `""` still lands on `Malformed` below.
        let unit = if let Some(aid) = item.accessory_id.as_deref().filter(|s| !s.is_empty()) {
            let Some(&(price, avail)) = catalog.accessories.get(aid) else {
                return FullSubtotalCheck::UnknownItem {
                    catalog: "accessories",
                    id: aid.into(),
                };
            };
            if !avail {
                return FullSubtotalCheck::Unavailable {
                    catalog: "accessories",
                    id: aid.into(),
                };
            }
            effective_accessory_price(price)
        } else if let Some(tid) = item.tea_id.as_deref().filter(|s| !s.is_empty()) {
            let Some(&(price, avail)) = catalog.tea_products.get(tid) else {
                return FullSubtotalCheck::UnknownItem {
                    catalog: "tea_products",
                    id: tid.into(),
                };
            };
            if !avail {
                return FullSubtotalCheck::Unavailable {
                    catalog: "tea_products",
                    id: tid.into(),
                };
            }
            effective_tea_price(price)
        } else if let Some(sid) = item.set_id.as_deref().filter(|s| !s.is_empty()) {
            let Some(&(tp, dp, avail)) = catalog.sets.get(sid) else {
                return FullSubtotalCheck::UnknownItem {
                    catalog: "sets",
                    id: sid.into(),
                };
            };
            if !avail {
                return FullSubtotalCheck::Unavailable {
                    catalog: "sets",
                    id: sid.into(),
                };
            }
            effective_set_price(tp, dp)
        } else if item.bike.is_some() {
            // Rental and sale money is quoted and taken at the office; this
            // line adds nothing to the cart total. Zero here is not a price —
            // it is the absence of one from this subtotal's point of view.
            0.0
        } else {
            return FullSubtotalCheck::Malformed;
        };
        sum += unit * qty;
    }
    if (sum - claimed_subtotal).abs() > tolerance {
        return FullSubtotalCheck::Mismatch {
            claimed: claimed_subtotal,
            expected: sum,
        };
    }
    FullSubtotalCheck::Ok
}

// `item_matches_id` and `item_unit_price` (the B4 garden product-scoped
// discount) were only ever called from the garden-reward path in
// `create_order`. D5 removes the mechanic rather than repointing it, so both
// helpers and their tests go with it — under `-D warnings` a helper with no
// callers is a build failure, and leaving one behind would also leave the
// impression that some other code still computes a server-side discount.

/// Outcome of the fleet-side check for one bike line. Mirrors
/// [`FullSubtotalCheck`]'s vocabulary so `create_order` records the same fraud
/// codes for a fabricated bike family as for a fabricated accessory.
#[derive(Debug, PartialEq)]
pub(crate) enum BikeLineCheck {
    Ok,
    /// No `bikes` row has this `key` — a stale cart, or an invented family.
    UnknownFamily(String),
    /// The family exists but `offered = false`: closed to NEW rentals. Today
    /// that is `click-125` (D12), whose one unit is still out on a contract
    /// that predates the decision. A sale is not blocked by this flag — the
    /// column's documented meaning is about renting.
    NotOffered(String),
    /// A rental of a family that publishes no `base_rate_thb_day`. D9's
    /// amendment: a family with no published rate is not addable to a cart —
    /// the call refuses and the catalog says a human quotes this price (D11).
    /// Accepting it would put a rental with no rate anywhere in the system on
    /// the books.
    NoPublishedRate(String),
    /// A rental line's money deposit that the family's published
    /// `deposit_thb` does not back (T27 C3, #28 AC3/AC7). The reasons are
    /// [`DepositRefusal`]'s.
    DepositRefused(String, DepositRefusal),
}

/// Compare the rate the client says was quoted against the one the seed-derived
/// tariff implies, and log any divergence of a baht or more.
///
/// D11: "Every priced answer logs any divergence of 1 baht or more between the
/// door's number and the file's number", non-blocking. Here the client's
/// `rate_thb_day` IS the door's number (the owner quoted it in the chat that
/// produced the booking) and `base_rate_thb_day × (1 - class discount)` is the
/// file's. Neither is corrected to match the other: the quote stands, and the
/// log is for whoever reconciles the sheet later.
///
/// Pure, and it returns the pair it compared so a test can assert on it
/// without reading a log: `Some((quoted, from_file))` when both numbers exist
/// and differ by at least a baht.
pub(crate) fn rate_divergence(
    quoted_thb_day: Option<f64>,
    base_rate_thb_day: Option<f64>,
    class_discount: Option<f64>,
) -> Option<(f64, f64)> {
    use crate::db::bikes::{apply_class_discount, round_half_up_baht};
    let quoted = quoted_thb_day.filter(|v| v.is_finite())?;
    let from_file = round_half_up_baht(apply_class_discount(base_rate_thb_day, class_discount))?;
    if (quoted - from_file).abs() >= 1.0 {
        Some((quoted, from_file))
    } else {
        None
    }
}

/// Why a rental line's money deposit was refused (T27 C3, 2026-09-24).
///
/// The published figure is `bikes.deposit_thb`: a per-family attribute
/// looked up on the family key and never derived (D17), undiscounted (the
/// class and term discounts are RATE discounts). The figure in the order
/// body is input, never authority (#28 AC3/AC7). Until this check the body's
/// deposit was only bounded by [`validate_bike_lines`] and stored as sent, so
/// a lowered one was recorded rather than refused.
#[derive(Debug, PartialEq)]
pub(crate) enum DepositRefusal {
    /// Money on a family that publishes no deposit: `deposit_thb` is absent,
    /// or is not a finite positive figure. There is nothing to check the
    /// money against, and inventing a figure is the D9 defect.
    NotPublished,
    /// A figure in the published currency that is not the published figure.
    /// The owner's knowledge base lowers a deposit only through a manager,
    /// never through the order body.
    Differs { claimed: f64, published: f64 },
    /// A figure in another currency. Its equivalent is agreed by a human at
    /// the door and no exchange rate is published (D18), so the server has
    /// nothing to compare it with. The money form without a figure is still
    /// accepted.
    NotComparable { currency: String },
}

/// Server-side authority for the deposit of ONE rental line, against the
/// family's published `deposit_thb`. Pure; `None` means accepted.
///
/// * No deposit form, or the passport: accepted on any family. The passport
///   carries no amount by construction.
/// * Money on a family with no published deposit: refused.
/// * Money whose figure is not agreed yet (`amount: None`): accepted. That
///   is a dash, not a claim, and there is nothing to compare.
/// * A figure in the published currency, or with no currency named: must be
///   the published figure, exactly. The body may repeat the figure, never
///   name it. That is not a claim of whole-baht tiers: the column admits a
///   fraction and no tier check runs on it (turbobaby/catalog-api
///   `DEPOSIT_COLUMN_ADMITS_A_FRACTION`, `DEPOSIT_TIER_CHECK_IS_SHIPPED`).
/// * A figure in any other currency: refused, see
///   [`DepositRefusal::NotComparable`].
///
/// The figure is PER UNIT (2026-09-24). `deposit_thb` is one bike's deposit
/// and the line's `quantity` is not read, so a line of two units passes with
/// the one-bike figure and is refused with twice it. Whether a multi-unit
/// line should carry the line's total instead is the owner's question.
pub(crate) fn deposit_refusal(
    deposit: Option<&DepositForm>,
    published_deposit_thb: Option<f64>,
) -> Option<DepositRefusal> {
    let Some(DepositForm::Money {
        amount, currency, ..
    }) = deposit
    else {
        return None;
    };
    // The one filter that decides "usable" for a published figure (D15): an
    // absent, NaN, infinite, zero or negative column is no deposit at all.
    let Some(published) = crate::trios::pricing::published_money(published_deposit_thb) else {
        return Some(DepositRefusal::NotPublished);
    };
    // No figure agreed yet is a dash, not a claim: there is nothing to
    // compare, so the line is accepted.
    let claimed = (*amount)?;
    // The column is THB by name; the code comes from the declared market
    // profile rather than a literal (D18).
    let published_code = crate::trios::pricing::THB_MARKET.currency_code;
    if let Some(code) = currency.as_deref() {
        if !code.eq_ignore_ascii_case(published_code) {
            return Some(DepositRefusal::NotComparable {
                currency: code.to_string(),
            });
        }
    }
    if claimed != published {
        return Some(DepositRefusal::Differs { claimed, published });
    }
    None
}

/// [`deposit_refusal`] over every rental line of the family `key`; the first
/// refusal wins. Sale lines carry no deposit and are skipped.
pub(crate) fn rental_deposit_refusal(
    items: &[OrderItem],
    key: &str,
    published_deposit_thb: Option<f64>,
) -> Option<DepositRefusal> {
    items
        .iter()
        .filter_map(|item| item.bike.as_ref())
        .filter(|bike| bike.bike_key == key)
        .find_map(|bike| match &bike.deal {
            BikeDeal::BikeRental { deposit, .. } => {
                deposit_refusal(deposit.as_ref(), published_deposit_thb)
            }
            BikeDeal::BikeSale { .. } => None,
        })
}

/// Fleet-side authority for the bike lines of an order: the family must exist,
/// a rental must be of a family that is still offered and that publishes a
/// rate, a rental's money deposit must be the family's published one
/// ([`deposit_refusal`]), and any quoted rate is reconciled against the file.
///
/// One `find_family_by_key` per distinct family — a cart holds one or two, and
/// the whole fleet is 14 families, so this is cheaper and far clearer than
/// assembling another `ANY($1)` map. `class_discounts` is read once and only
/// when a rental line is actually present.
///
/// What this does NOT do is reserve anything. `units_available` counts the
/// units sitting at base *right now*, which says nothing about a booking three
/// weeks out, so a shortfall is logged for staff and never refuses the order —
/// refusing a legitimate future booking on a today-only count would be worse
/// than not checking. There is no reservation table in the schema yet.
async fn check_bike_lines(
    orm: &sea_orm::DatabaseConnection,
    items: &[OrderItem],
) -> Result<BikeLineCheck, sea_orm::DbErr> {
    use crate::db::bikes::{discount_for_class, find_family_by_key, list_class_discounts};

    // Units wanted per family, summed over the lines, and whether any line of
    // that family is a rental.
    let mut wanted: Vec<(&str, f64, bool)> = Vec::new();
    for item in items {
        let Some(bike) = item.bike.as_ref() else {
            continue;
        };
        let is_rental = bike.deal.rental_dates().is_some();
        match wanted.iter_mut().find(|(k, _, _)| *k == bike.bike_key) {
            Some(entry) => {
                entry.1 += item.quantity;
                entry.2 |= is_rental;
            }
            None => wanted.push((bike.bike_key.as_str(), item.quantity, is_rental)),
        }
    }
    if wanted.is_empty() {
        return Ok(BikeLineCheck::Ok);
    }

    let any_rental = wanted.iter().any(|(_, _, rental)| *rental);
    // An empty table is not "no discounts" — it is unseeded, and then
    // `discount_for_class` returns `None` and no file-side rate is computed.
    // That silences the divergence log; it never quotes the pre-discount
    // tariff, which would overstate a scooter by a third.
    let discounts = if any_rental {
        list_class_discounts(orm)
            .await
            .map_err(|e| sea_orm::DbErr::Custom(format!("list_class_discounts: {e}")))?
    } else {
        Vec::new()
    };

    let mut found: Vec<(&str, crate::db::bikes::BikeListing)> = Vec::new();
    for (key, units, is_rental) in &wanted {
        let listing = find_family_by_key(orm, key)
            .await
            .map_err(|e| sea_orm::DbErr::Custom(format!("find_family_by_key({key}): {e}")))?;
        let Some(listing) = listing else {
            return Ok(BikeLineCheck::UnknownFamily((*key).to_string()));
        };
        if *is_rental {
            if !listing.bike.offered {
                return Ok(BikeLineCheck::NotOffered((*key).to_string()));
            }
            if listing.bike.base_rate_thb_day.is_none() {
                return Ok(BikeLineCheck::NoPublishedRate((*key).to_string()));
            }
            if let Some(why) = rental_deposit_refusal(items, key, listing.bike.deposit_thb) {
                return Ok(BikeLineCheck::DepositRefused((*key).to_string(), why));
            }
        }
        if listing.units_available < units.ceil() as i64 {
            tracing::info!(
                bike_key = %key,
                units_requested = units,
                units_available = listing.units_available,
                "create_order: more units booked than are at base today — not a refusal, \
                 the availability count is point-in-time and carries no reservation"
            );
        }
        found.push((key, listing));
    }

    // Reconcile each quoted rate against the file, per line, reusing the rows
    // the gate above already read.
    for item in items {
        let Some(bike) = item.bike.as_ref() else {
            continue;
        };
        let BikeDeal::BikeRental { rate_thb_day, .. } = &bike.deal else {
            continue;
        };
        let Some((_, listing)) = found.iter().find(|(k, _)| *k == bike.bike_key) else {
            continue;
        };
        let discount = discount_for_class(&discounts, &listing.bike.class);
        if let Some((quoted, from_file)) =
            rate_divergence(*rate_thb_day, listing.bike.base_rate_thb_day, discount)
        {
            tracing::info!(
                bike_key = %bike.bike_key,
                quoted_thb_day = quoted,
                file_thb_day = from_file,
                "create_order: quoted rate diverges from the file by a baht or more (D11, non-blocking)"
            );
        }
    }
    Ok(BikeLineCheck::Ok)
}

async fn create_order(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(mut req): Json<CreateOrderRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Cycle #57: X-Idempotency-Key (Stripe/AWS-style replay protection).
    // Optional — old clients without the header keep working — but when
    // present, two POSTs with the same key produce one order and the
    // second call returns the original order_id. Closes the Two Generals
    // window where a network blip mid-response causes a duplicate retry.
    let idem_key: Option<String> = headers
        .get("x-idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(ref k) = idem_key {
        if !is_valid_idempotency_key(k) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Cycle #105: anonymous-order IP rate-limit. Authenticated orders
    // are protected later by per-tid advisory lock + DB-driven 1-min
    // window; anonymous orders had no abuse guard at all. Cheap and
    // in-memory, runs before any DB cost.
    if req.telegram_id.is_none() {
        let client_ip = client_ip_from_headers(&headers);
        if !check_and_record(
            &ANON_ORDER_RATE_LIMIT,
            &client_ip,
            ANON_ORDER_RL_WINDOW,
            ANON_ORDER_RL_MAX_ATTEMPTS,
            ANON_ORDER_RL_MAX_IPS,
        )
        .await
        {
            crate::metrics::rate_limit_blocked("anon_order");
            tracing::warn!("create_order: anon rate-limit exceeded ip={}", client_ip);
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    // If telegram_id is provided, verify ownership and blocked status.
    // check_owner now tries strict HMAC validation first, then falls back to
    // initData user-id + auth_date freshness. This matches the garden/events
    // policy and unblocks production Mini Apps where Telegram's initData HMAC
    // currently fails validation, while still preventing impersonation.
    if let Some(tid) = req.telegram_id {
        let _owner_id = crate::api::auth::check_owner(&headers, &state, tid)?;
        check_not_blocked(&state, tid).await?;
    }

    let ValidatedPayment {
        bonus_used,
        stars_used,
    } = validate_create_order(&req)?;

    // Loop #7 / #11: the delivery zone id (if supplied) must resolve to an
    // active DB zone. Rejecting here prevents a stale/malformed zone from
    // being stored and misleading the ETA on the success screen.
    if let Some(ref zid) = req.delivery_zone_id {
        if !zid.is_empty() {
            let zone_uuid = uuid::Uuid::parse_str(zid).map_err(|_| {
                tracing::info!(zone_id=%zid, "create_order: malformed delivery zone id");
                StatusCode::UNPROCESSABLE_ENTITY
            })?;
            use crate::db::entities::delivery_zone::{Column as ZoneCol, Entity as ZoneEntity};
            use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
            let exists = ZoneEntity::find()
                .filter(ZoneCol::Id.eq(zone_uuid))
                .filter(ZoneCol::IsActive.eq(true))
                .one(&state.db.orm)
                .await
                .map_err(|e| {
                    tracing::error!("create_order: delivery zone lookup failed: {e}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?
                .is_some();
            if !exists {
                tracing::info!(
                    telegram_id = req.telegram_id.unwrap_or(0),
                    zone_id = %zid,
                    "create_order: unknown or inactive delivery zone"
                );
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
        }
    }

    // D8: the bike half of every line, checked before any money is touched.
    // Pure and dateful — `shop_today()` is Phuket's date, not the server's.
    validate_bike_lines(&req.items, shop_today())?;

    // D9: a bike line's money lives in its `deal` and nowhere else. A per-day
    // rate parked in `unit_price` renders as a line total (`rate × units`) on
    // every screen that multiplies one by the other — an invented figure the
    // door never quoted. Clear the field rather than trusting the client not
    // to populate it; the stored order then says only what was agreed.
    for item in req.items.iter_mut() {
        if item.bike.is_some() {
            item.unit_price = None;
        }
    }

    // Cycle #58 / C: full server-side price authority across every priced
    // catalog (accessories + tea + sets). `trios::pricing` (cycle #55) keeps
    // the math identical to the customer-facing menu so legitimate orders
    // never get flagged as fraud.
    //
    // The strain lookup left with the table (`083_drop_cannabis_catalog.sql`);
    // a line that still carries a `strain_id` and nothing else now falls
    // through to `Malformed`, which is the honest answer — that product
    // cannot be sold any more.
    //
    // This block used to be skipped entirely when no catalog id was present,
    // which meant a cart of lines referencing nothing at all never reached
    // `check_full_subtotal` and its client-claimed subtotal was stored
    // unchecked. It now always runs: each SELECT is still conditional on its
    // own id list, so a bike-only cart costs no extra query and is required
    // to claim a subtotal of 0.
    let accessory_ids: Vec<String> = req
        .items
        .iter()
        .filter_map(|i| i.accessory_id.clone())
        .collect();
    let tea_ids: Vec<String> = req.items.iter().filter_map(|i| i.tea_id.clone()).collect();
    let set_ids: Vec<String> = req.items.iter().filter_map(|i| i.set_id.clone()).collect();
    {
        // Cycle #90: full SeaORM. The legacy `lookup_client = state.db.pool.get()`
        // is gone — accessory / tea / sets go through raw `Statement`
        // (pattern #15). This was the last `state.db.pool` usage in
        // api/orders.rs; after this cycle the orders endpoint is 100% off raw
        // Pool.
        use sea_orm::{ConnectionTrait, DbBackend, Statement};
        let mut catalog: PriceCatalog<'_> = PriceCatalog::default();
        // Accessories / tea_products — read-only price-auth lookups via raw
        // `Statement` (pattern #15). Generating full entities just for a
        // 3-column SELECT in a single call site isn't a useful trade —
        // the typed builder doesn't buy us much over `Statement` here.
        let acc_rows: Vec<(String, f64, bool)> = if !accessory_ids.is_empty() {
            let rows = state.db.orm
                .query_all(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "SELECT id, price::float8 AS price, is_available FROM accessories WHERE id = ANY($1)",
                    [accessory_ids.clone().into()],
                ))
                .await
                .map_err(|e| {
                    error!("price-auth accessory lookup: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            // Cycle #98: propagate `try_get` errors instead of
            // defaulting to 0.0. Per TRY_GET_AUDIT 🔴 — schema drift
            // (column rename, type change) would silently mark all
            // accessories as priced 0, letting fabricated orders pass
            // server-side authority. Propagating as 500 fails closed.
            rows.iter()
                .map(|r| {
                    Ok::<_, sea_orm::DbErr>((
                        r.try_get::<String>("", "id")?,
                        r.try_get::<f64>("", "price")?,
                        r.try_get::<bool>("", "is_available")?,
                    ))
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| {
                    error!("price-auth accessory parse: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?
        } else {
            Vec::new()
        };
        for (id, p, a) in &acc_rows {
            catalog.accessories.insert(id.as_str(), (*p, *a));
        }
        let tea_rows: Vec<(String, f64, bool)> = if !tea_ids.is_empty() {
            let rows = state.db.orm
                .query_all(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "SELECT id, price::float8 AS price, is_available FROM tea_products WHERE id = ANY($1)",
                    [tea_ids.clone().into()],
                ))
                .await
                .map_err(|e| {
                    error!("price-auth tea lookup: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            // Cycle #98: same propagation as accessories above — schema
            // drift fails closed instead of letting prices default to 0.
            rows.iter()
                .map(|r| {
                    Ok::<_, sea_orm::DbErr>((
                        r.try_get::<String>("", "id")?,
                        r.try_get::<f64>("", "price")?,
                        r.try_get::<bool>("", "is_available")?,
                    ))
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| {
                    error!("price-auth tea parse: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?
        } else {
            Vec::new()
        };
        for (id, p, a) in &tea_rows {
            catalog.tea_products.insert(id.as_str(), (*p, *a));
        }
        // Sets — UNION ALL across three tables (`sets`, `accessory_sets`,
        // `tea_sets`). Same `(total_price, discount_percent)` shape; UUID
        // PKs across the three don't collide. SeaORM 1.1 has no idiomatic
        // UNION builder, so raw `Statement` is the cleanest path.
        let set_rows: Vec<(String, f64, f64, bool)> = if !set_ids.is_empty() {
            let rows = state.db.orm.query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT id, total_price::float8 AS tp, discount_percent::float8 AS dp, is_available \
                 FROM sets WHERE id = ANY($1) \
                 UNION ALL \
                 SELECT id, total_price::float8 AS tp, discount_percent::float8 AS dp, is_available \
                 FROM accessory_sets WHERE id = ANY($1) \
                 UNION ALL \
                 SELECT id, total_price::float8 AS tp, discount_percent::float8 AS dp, is_available \
                 FROM tea_sets WHERE id = ANY($1)",
                [set_ids.clone().into()],
            )).await.map_err(|e| {
                error!("price-auth sets lookup: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            // Cycle #98: propagate parse errors on tp/dp too.
            rows.iter()
                .map(|r| {
                    Ok::<_, sea_orm::DbErr>((
                        r.try_get::<String>("", "id")?,
                        r.try_get::<f64>("", "tp")?,
                        r.try_get::<f64>("", "dp")?,
                        r.try_get::<bool>("", "is_available")?,
                    ))
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| {
                    error!("price-auth sets parse: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?
        } else {
            Vec::new()
        };
        for (id, tp, dp, a) in &set_rows {
            catalog.sets.insert(id.as_str(), (*tp, *dp, *a));
        }

        match check_full_subtotal(&req.items, &catalog, req.subtotal, 0.01) {
            FullSubtotalCheck::Ok => {}
            FullSubtotalCheck::UnknownItem { catalog: cat, id } => {
                tracing::warn!(
                    telegram_id = req.telegram_id.unwrap_or(0),
                    missing_catalog = cat,
                    missing_id = %id,
                    "create_order: order references missing catalog item"
                );
                // Cycle #59: persist for /engage admin panel. Best-effort —
                // we don't fail the reject path if the audit insert blips.
                if let Err(e) = crate::db::orders::record_fraud_event(
                    &state.db.orm,
                    req.telegram_id,
                    crate::db::orders::FRAUD_CODE_UNKNOWN_ITEM,
                    Some(cat),
                    Some(&id),
                    None,
                    None,
                )
                .await
                {
                    tracing::warn!("fraud_event audit insert failed: {}", e);
                }
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
            FullSubtotalCheck::Unavailable { catalog: cat, id } => {
                // Stale cart vs admin turning the item off — log at info, not
                // warn, so fraud alerts don't drown in the everyday case.
                tracing::info!(
                    telegram_id = req.telegram_id.unwrap_or(0),
                    unavailable_catalog = cat,
                    unavailable_id = %id,
                    "create_order: order references item marked unavailable"
                );
                if let Err(e) = crate::db::orders::record_fraud_event(
                    &state.db.orm,
                    req.telegram_id,
                    crate::db::orders::FRAUD_CODE_UNAVAILABLE,
                    Some(cat),
                    Some(&id),
                    None,
                    None,
                )
                .await
                {
                    tracing::warn!("fraud_event audit insert failed: {}", e);
                }
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
            FullSubtotalCheck::Mismatch { claimed, expected } => {
                tracing::warn!(
                    telegram_id = req.telegram_id.unwrap_or(0),
                    claimed_subtotal = claimed,
                    expected_subtotal = expected,
                    items = req.items.len(),
                    "create_order: subtotal mismatch — possible client tampering"
                );
                if let Err(e) = crate::db::orders::record_fraud_event(
                    &state.db.orm,
                    req.telegram_id,
                    crate::db::orders::FRAUD_CODE_SUBTOTAL_MISMATCH,
                    None,
                    None,
                    Some(claimed),
                    Some(expected),
                )
                .await
                {
                    tracing::warn!("fraud_event audit insert failed: {}", e);
                }
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
            FullSubtotalCheck::Malformed => {
                tracing::warn!(
                    telegram_id = req.telegram_id.unwrap_or(0),
                    items = req.items.len(),
                    "create_order: malformed line item — no *_id field set"
                );
                if let Err(e) = crate::db::orders::record_fraud_event(
                    &state.db.orm,
                    req.telegram_id,
                    crate::db::orders::FRAUD_CODE_MALFORMED,
                    None,
                    None,
                    None,
                    None,
                )
                .await
                {
                    tracing::warn!("fraud_event audit insert failed: {}", e);
                }
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
        }
    }

    // D11/D12: the fleet half of price authority. A bike line adds nothing to
    // the subtotal, so `check_full_subtotal` cannot police it — this does. It
    // refuses unknown families, rentals of a family the door has closed,
    // rentals of a family with no published rate (there is no number to quote,
    // so the cart cannot hold one), and a rental whose money deposit is not the
    // family's published deposit (T27 C3). A quoted rate that disagrees with
    // the file is logged, never corrected: the door is authoritative, and
    // logging must not block the answer.
    match check_bike_lines(&state.db.orm, &req.items).await {
        Ok(BikeLineCheck::Ok) => {}
        Ok(BikeLineCheck::UnknownFamily(key)) => {
            tracing::warn!(
                telegram_id = req.telegram_id.unwrap_or(0),
                bike_key = %key,
                "create_order: order references a bike family that does not exist"
            );
            if let Err(e) = crate::db::orders::record_fraud_event(
                &state.db.orm,
                req.telegram_id,
                crate::db::orders::FRAUD_CODE_UNKNOWN_ITEM,
                Some("bikes"),
                Some(&key),
                None,
                None,
            )
            .await
            {
                tracing::warn!("fraud_event audit insert failed: {}", e);
            }
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
        Ok(BikeLineCheck::NotOffered(key)) => {
            // The everyday case (click-125 is closed to new rentals while the
            // existing rental runs), so info, not warn.
            tracing::info!(
                telegram_id = req.telegram_id.unwrap_or(0),
                bike_key = %key,
                "create_order: rental requested for a family not offered"
            );
            if let Err(e) = crate::db::orders::record_fraud_event(
                &state.db.orm,
                req.telegram_id,
                crate::db::orders::FRAUD_CODE_UNAVAILABLE,
                Some("bikes"),
                Some(&key),
                None,
                None,
            )
            .await
            {
                tracing::warn!("fraud_event audit insert failed: {}", e);
            }
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
        Ok(BikeLineCheck::NoPublishedRate(key)) => {
            // Not fraud, and not an empty cart line either: the shop has no
            // published day rate for this family, so a human quotes it. No
            // fraud event — the client did nothing wrong.
            tracing::info!(
                telegram_id = req.telegram_id.unwrap_or(0),
                bike_key = %key,
                "create_order: rental requested for a family with no published rate"
            );
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
        Ok(BikeLineCheck::DepositRefused(key, why)) => {
            // T27 C3: the deposit on a rental line is the family's published
            // figure or no figure at all. No fraud event: a cart left open
            // across a tariff edit lands here as honestly as a lowered figure
            // does, and no fraud code names this refusal.
            tracing::warn!(
                telegram_id = req.telegram_id.unwrap_or(0),
                bike_key = %key,
                refusal = ?why,
                "create_order: rental deposit is not backed by the family's published deposit"
            );
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
        Err(e) => {
            error!("create_order: bike line check failed: {e}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    // Loop #14: enforce the configured max_bonus_usage_pct ceiling at the
    // trust boundary. The client can send any bonus_used ≤ subtotal, but the
    // business rule caps how much of an order can be paid with bonus balance.
    let max_bonus_pct = load_max_bonus_usage_pct(&state.db.orm).await;
    let max_bonus_allowed = (req.subtotal * max_bonus_pct / 100.0).max(0.0);
    if bonus_used > max_bonus_allowed + 0.01 {
        tracing::warn!(
            telegram_id = req.telegram_id.unwrap_or(0),
            bonus_used,
            max_bonus_pct,
            max_bonus_allowed,
            subtotal = req.subtotal,
            "create_order: bonus_used exceeds configured max_bonus_usage_pct"
        );
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    // D5: the garden reward's total re-check lived here. It only ran when a
    // reward was applied; the unconditional identity
    // `total == subtotal - bonus_used - stars_used` is now enforced for every
    // order by `validate_create_order`, so nothing is lost by its removal.

    let id = uuid::Uuid::new_v4().to_string();
    let items_json = serde_json::to_value(&req.items).map_err(|e| {
        error!("items serialization failed: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    // Cycle #87: SeaORM transaction. 7-statement tx mixes entity API
    // (loyalty UPDATE, order INSERT, idempotency_keys CRUD) with raw
    // `Statement::from_sql_and_values` for Postgres advisory locks
    // (`pg_advisory_xact_lock`) — there's no entity model for those,
    // they're session-scoped primitives that release on commit/rollback.
    use crate::db::entities::{
        loyalty_profile::{Column as LpCol, Entity as LoyaltyProfileEntity},
        order::{ActiveModel as OrderAm, Entity as OrderEntity},
        order_idempotency_key::{ActiveModel as IdemAm, Entity as IdemEntity},
        stars_transaction::{ActiveModel as StarsTxAm, Entity as StarsTxEntity},
        user_stars::{ActiveModel as UsAm, Column as UsCol, Entity as UsEntity},
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{
        ActiveValue::Set, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
        Statement, TransactionTrait,
    };
    let tx = state.db.orm.begin().await.map_err(|e| {
        error!("create_order tx.begin error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Idempotency check (must run BEFORE the 1-min rate limit). Two parallel
    // POSTs with the same key serialise on the per-key advisory lock; the
    // loser then sees the existing row and replays the cached order_id
    // instead of being told "you're rate-limited".
    if let Some(ref k) = idem_key {
        tx.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtext($1)::bigint)",
            [k.clone().into()],
        ))
        .await
        .map_err(|e| {
            error!("idempotency lock: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let existing = IdemEntity::find_by_id(k.clone())
            .one(&tx)
            .await
            .map_err(|e| {
                error!("idempotency SELECT: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        if let Some(row) = existing {
            let existing_id = row.order_id;
            if let Err(e) = tx.commit().await {
                tracing::error!("idempotency replay commit: {}", e);
            }
            tracing::info!(
                order_id = %existing_id,
                "create_order: idempotent replay"
            );
            return Ok(Json(json!({
                "success": true,
                "order_id": existing_id,
                "idempotent_replay": true
            })));
        }
    }

    // Serialize order creation per user to close the rate-limit race window.
    if let Some(tid) = req.telegram_id {
        tx.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock($1)",
            [tid.into()],
        ))
        .await
        .map_err(|e| {
            error!("advisory lock error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    // Rate-limit inside tx to close the race window. The 1-minute window
    // and the `LIMIT 1` make this a small bounded scan; no need for an
    // entity helper.
    if let Some(tid) = req.telegram_id {
        let recent = tx
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT 1 AS one FROM orders WHERE telegram_id = $1 AND created_at > NOW() - INTERVAL '1 minute' LIMIT 1",
                [tid.into()],
            ))
            .await
            .map_err(|e| {
                error!("rate-limit check error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        if recent.is_some() {
            // tx drops → auto-rollback
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    // Atomic bonus deduction: UPDATE with built-in balance guard
    // (pattern #12 in memory/seaorm-patterns.md).
    if bonus_used > 0.0 {
        if let Some(tid) = req.telegram_id {
            let deducted = LoyaltyProfileEntity::update_many()
                .col_expr(
                    LpCol::BonusBalance,
                    sea_orm::sea_query::Expr::cust_with_values(
                        "GREATEST(0, bonus_balance - $1)",
                        [bonus_used],
                    ),
                )
                .filter(LpCol::TelegramId.eq(tid))
                .filter(LpCol::BonusBalance.gte(bonus_used))
                .exec(&tx)
                .await
                .map_err(|e| {
                    error!("bonus deduction error: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            if deducted.rows_affected == 0 {
                // tx drops → auto-rollback
                return Err(StatusCode::BAD_REQUEST);
            }
        } else {
            // tx drops → auto-rollback
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Atomic Stars deduction + ledger entry (cycle #172). The debit is
    // idempotent by virtue of running inside the order idempotency tx.
    if stars_used > 0 {
        let Some(tid) = req.telegram_id else {
            return Err(StatusCode::BAD_REQUEST);
        };
        // user_stars FKs to loyalty_profiles; seed both idempotently so a
        // brand-new gamer who earned Stars before ever ordering still succeeds.
        let lp_seed = crate::db::entities::loyalty_profile::ActiveModel {
            telegram_id: Set(tid),
            bonus_balance: Set(Some(0.0)),
            total_spent: Set(Some(0.0)),
            ..Default::default()
        };
        LoyaltyProfileEntity::insert(lp_seed)
            .on_conflict(
                OnConflict::column(crate::db::entities::loyalty_profile::Column::TelegramId)
                    .do_nothing()
                    .to_owned(),
            )
            .do_nothing()
            .exec(&tx)
            .await
            .map_err(|e| {
                error!("stars deduction loyalty seed error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        UsEntity::insert(UsAm {
            telegram_id: Set(tid),
            balance: Set(0),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::column(UsCol::TelegramId)
                .update_column(UsCol::UpdatedAt)
                .to_owned(),
        )
        .exec(&tx)
        .await
        .map_err(|e| {
            error!("stars deduction user_stars seed error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let debited = UsEntity::update_many()
            .col_expr(
                UsCol::Balance,
                sea_orm::sea_query::Expr::cust_with_values("balance - $1", [stars_used]),
            )
            .filter(UsCol::TelegramId.eq(tid))
            .filter(UsCol::Balance.gte(stars_used))
            .exec(&tx)
            .await
            .map_err(|e| {
                error!("stars deduction error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        if debited.rows_affected == 0 {
            // Not enough Stars — fail loud so the UI can prompt the user.
            return Err(StatusCode::PAYMENT_REQUIRED);
        }

        let balance_after = UsEntity::find_by_id(tid)
            .one(&tx)
            .await
            .map_err(|e| {
                error!("stars deduction balance read: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .map(|r| r.balance)
            .unwrap_or(0);
        let stars_tx_id = uuid::Uuid::new_v4().to_string();
        StarsTxEntity::insert(StarsTxAm {
            id: Set(stars_tx_id),
            telegram_id: Set(tid),
            amount: Set(-stars_used),
            balance_after: Set(balance_after),
            source: Set("plot".to_string()),
            reason: Set("purchase".to_string()),
            external_tx_id: Set(None),
            related_order_id: Set(Some(id.clone())),
            ..Default::default()
        })
        .exec(&tx)
        .await
        .map_err(|e| {
            error!("stars transaction insert error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    // Order INSERT via ActiveModel.
    let order_am = OrderAm {
        id: Set(id.clone()),
        telegram_id: Set(req.telegram_id),
        customer_name: Set(req.customer_name.clone()),
        // Store E.164, not what was typed: the courier calls this number and
        // admin search matches on it, so `081…` and `+6681…` must not become
        // two different customers.
        customer_phone: Set(req
            .customer_phone
            .as_deref()
            .and_then(crate::trios::store::normalize_phone)
            .or_else(|| req.customer_phone.clone())),
        customer_telegram: Set(req.customer_telegram.clone()),
        items: Set(items_json.clone()),
        subtotal: Set(req.subtotal),
        bonus_used: Set(bonus_used),
        stars_used: Set(stars_used),
        total: Set(req.total),
        status: Set("pending".to_string()),
        shop_id: Set(req.shop_id.clone()),
        delivery_address: Set(req.delivery_address.clone()),
        delivery_notes: Set(req.delivery_notes.clone()),
        age_confirmed: Set(req.age_confirmed.unwrap_or(false)),
        delivery_zone_id: Set(req.delivery_zone_id.clone()),
        ..Default::default()
    };
    OrderEntity::insert(order_am).exec(&tx).await.map_err(|e| {
        error!("create_order insert error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // D5: the `UPDATE garden_rewards SET is_used = true` that consumed the
    // applied reward inside this transaction is gone with the mechanic.

    // Record the idempotency key inside the same tx so retries after this
    // commit see the cached order_id. The earlier advisory lock guarantees
    // no other tx can hold a different (key, order_id) for this `k`.
    if let Some(ref k) = idem_key {
        let idem_am = IdemAm {
            key: Set(k.clone()),
            order_id: Set(id.clone()),
            telegram_id: Set(req.telegram_id),
            ..Default::default()
        };
        IdemEntity::insert(idem_am).exec(&tx).await.map_err(|e| {
            error!("idempotency INSERT: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    if let Err(e) = tx.commit().await {
        error!("create_order commit error: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    crate::metrics::order_created();

    let bot = state.bot.clone();
    let config = state.config.clone();
    let order_id = id.clone();
    tokio::spawn(async move {
        // The typed items, not `items_json`: the admin card has to show a
        // rental's dates, quoted rate and deposit form, and reading those back
        // out of an untyped `Value` is how the old card ended up printing
        // `unwrap_or(0.0)` for every number it could not find.
        notify_admins(
            &bot,
            &config,
            &order_id,
            &req.customer_name,
            &req.customer_telegram,
            &req.items,
            req.subtotal,
            bonus_used,
            stars_used,
            req.total,
        )
        .await;
    });

    Ok(Json(json!({ "success": true, "order_id": id })))
}

use crate::util::html_escape;

/// D9: a THB figure for the admin card, or an em dash when there is none.
///
/// The dash is the whole point. `0` on a deposit line would tell the person at
/// the door that nothing is owed; the dash tells them the number has not been
/// computed yet and that they are the one who decides it (D11).
fn thb_or_dash(v: Option<f64>) -> String {
    match crate::db::orders::finite_money(v) {
        Some(x) => format!("{x} ฿"),
        None => "—".into(),
    }
}

/// One line of the admin order card.
///
/// A bike line reads as the machine, the units, the term and what was agreed;
/// everything else keeps the short catalog form. No figure is invented: the
/// per-day rate is printed as quoted and never multiplied by the span, because
/// the bands in `data/fleet_seed.json` are ranges and the door owns the total
/// (D11).
fn admin_item_line(item: &OrderItem) -> String {
    let qty = if item.quantity.is_finite() {
        item.quantity
    } else {
        0.0
    };
    if let Some(bike) = item.bike.as_ref() {
        let name = bike
            .bike_name
            .as_deref()
            .filter(|n| !n.is_empty())
            .unwrap_or(bike.bike_key.as_str());
        let handover = match item.fulfillment.as_deref() {
            Some("delivery") => " 🛻 доставка",
            Some("pickup") => " 🏠 самовывоз",
            _ => "",
        };
        match &bike.deal {
            BikeDeal::BikeRental {
                rental_start,
                rental_end,
                rate_thb_day,
                deposit,
            } => {
                let deposit_text = match deposit {
                    Some(DepositForm::Passport) => "паспорт".to_string(),
                    Some(DepositForm::Money {
                        amount,
                        currency,
                        method,
                    }) => {
                        let cur = currency.as_deref().filter(|c| !c.is_empty()).unwrap_or("฿");
                        let figure = match crate::db::orders::finite_money(*amount) {
                            Some(x) => format!("{x} {}", html_escape(cur)),
                            None => "—".into(),
                        };
                        match method.as_deref().filter(|m| !m.is_empty()) {
                            Some(m) => format!("{figure} ({})", html_escape(m)),
                            None => figure,
                        }
                    }
                    // Not "no deposit": not yet agreed.
                    None => "—".into(),
                };
                // `span_days` is read back to staff, never multiplied by the
                // rate — see its doc comment.
                let span_text = match bike.deal.span_days() {
                    Some(d) => format!("{d} дн."),
                    None => "—".into(),
                };
                format!(
                    "  • 🏍 {} × {} шт{}\n     аренда {} → {} ({}), ставка/день: {}, залог: {}",
                    html_escape(name),
                    qty,
                    handover,
                    rental_start,
                    rental_end,
                    span_text,
                    thb_or_dash(*rate_thb_day),
                    deposit_text,
                )
            }
            BikeDeal::BikeSale { price_thb } => format!(
                "  • 🏍 {} × {} шт{}\n     продажа, цена: {}",
                html_escape(name),
                qty,
                handover,
                thb_or_dash(*price_thb),
            ),
        }
    } else {
        let name = item
            .accessory_name
            .as_deref()
            .or(item.tea_name.as_deref())
            .or(item.set_name.as_deref())
            .or(item.strain_name.as_deref())
            .filter(|n| !n.is_empty())
            .unwrap_or("?");
        // D7: `fulfillment`, two l's. The drink vocabulary is kept for the
        // catalog lines that still use it; `validate_bike_lines` keeps it off
        // a machine.
        let fulfillment = match item.fulfillment.as_deref() {
            Some("dine_in") => " 🍽 на месте",
            Some("takeaway") => " 🥡 с собой",
            Some("delivery") => " 🛻 доставка",
            Some("pickup") => " 🏠 самовывоз",
            _ => "",
        };
        format!("  • {} × {}{}", html_escape(name), qty, fulfillment)
    }
}

#[allow(clippy::too_many_arguments)]
async fn notify_admins(
    bot: &teloxide::Bot,
    config: &crate::config::Config,
    order_id: &str,
    customer_name: &Option<String>,
    customer_telegram: &Option<String>,
    items: &[OrderItem],
    subtotal: f64,
    bonus_used: f64,
    stars_used: i64,
    total: f64,
) {
    use teloxide::prelude::*;
    use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

    // The old card hard-coded `× {}g` on every line — grams, for a rebrand
    // whose unit is a machine. The unit now comes from the line itself.
    let items_text = items
        .iter()
        .map(admin_item_line)
        .collect::<Vec<_>>()
        .join("\n");

    let source = customer_telegram
        .as_ref()
        .map(|t| format!("@{}", html_escape(t)))
        .or_else(|| customer_name.as_ref().map(|n| html_escape(n)))
        .unwrap_or_else(|| "Anonymous".into());

    let stars_line = if stars_used > 0 {
        format!("\n⭐ Stars: -{} ฿", stars_used)
    } else {
        String::new()
    };
    let text = format!(
        "🚨 <b>New Order!</b>\n━━━━━━━━━━━━━━━━\n👤 {}\n📦 Items:\n{}\n━━━━━━━━━━━━━━━━\n💰 Subtotal: {} ฿\n🎁 Bonus: -{} ฿{}\n💳 Total: {} ฿\n🔖 #{}",
        source, items_text, subtotal, bonus_used, stars_line, total, html_escape(&order_id[order_id.len().saturating_sub(6)..])
    );

    let btns = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("✅ Confirm", format!("confirm_{}", order_id)),
        InlineKeyboardButton::callback("❌ Reject", format!("reject_{}", order_id)),
    ]]);

    for admin_id in &config.admin_ids {
        if let Err(e) = bot
            .send_message(teloxide::types::ChatId(*admin_id), &text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(btns.clone())
            .await
        {
            tracing::warn!(
                "notify_admins (order) failed for admin_id={}: {}",
                admin_id,
                e
            );
        }
    }
}

async fn get_orders(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(100)
        .clamp(1, 500);
    let offset = params
        .get("offset")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    // Cycle #85 Part 1: SeaORM. The legacy SQL cast `subtotal::float8`
    // etc. — sqlx auto-coerces NUMERIC↔f64 so the cast goes away.
    use crate::db::entities::order::{Column as OrderCol, Entity as OrderEntity};
    use sea_orm::{EntityTrait, Order as SortOrder, QueryOrder, QuerySelect};
    let models = OrderEntity::find()
        .order_by(OrderCol::CreatedAt, SortOrder::Desc)
        .limit(limit as u64)
        .offset(offset as u64)
        .all(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("get_orders SeaORM error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let orders: Vec<Order> = models.into_iter().map(Order::from).collect();
    Ok(Json(json!({ "orders": orders })))
}

async fn get_order(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    // Cycle #85 Part 1: SeaORM `find_by_id`.
    use crate::db::entities::order::Entity as OrderEntity;
    use sea_orm::EntityTrait;
    let model = OrderEntity::find_by_id(id)
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("get_order SeaORM error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    match model {
        Some(m) => Ok(Json(json!({ "order": Order::from(m) }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Public order-status endpoint used by the success screen. Returns only
/// non-sensitive fields (status + delivery zone/ETA) and requires the caller
/// to prove ownership of the order via Telegram initData.
async fn get_order_status(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let tid = params
        .get("telegram_id")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    validate_telegram_id_param(tid)?;
    let _owner_id = check_owner(&headers, &state, tid)?;

    use crate::db::entities::order::Entity as OrderEntity;
    use sea_orm::EntityTrait;
    let model = OrderEntity::find_by_id(id)
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("get_order_status SeaORM error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let Some(model) = model else {
        return Err(StatusCode::NOT_FOUND);
    };
    if model.telegram_id != Some(tid) {
        return Err(StatusCode::NOT_FOUND);
    }

    let zone = if let Some(ref zid) = model.delivery_zone_id {
        if let Ok(zone_uuid) = uuid::Uuid::parse_str(zid) {
            use crate::db::entities::delivery_zone::{Column as ZoneCol, Entity as ZoneEntity};
            use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
            ZoneEntity::find()
                .filter(ZoneCol::Id.eq(zone_uuid))
                .filter(ZoneCol::IsActive.eq(true))
                .one(&state.db.orm)
                .await
                .map_err(|e| {
                    tracing::error!("get_order_status: delivery zone lookup failed: {e}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?
        } else {
            None
        }
    } else {
        None
    };

    Ok(Json(json!({
        "order_id": model.id,
        "status": model.status,
        "delivery_zone_id": model.delivery_zone_id,
        "delivery_zone_name": zone.as_ref().map(|z| z.name.clone()),
        "min_eta_minutes": zone.as_ref().and_then(crate::delivery::zone_eta_min),
        "max_eta_minutes": zone.as_ref().and_then(crate::delivery::zone_eta_max),
        // D9: a fee that is not a finite number serialises as JSON `null` and
        // renders as a dash. The old `else { 0.0 }` told the customer that
        // delivery to this zone was free. The fees are the owner's Phuket
        // table (migration 087, owner 2026-09-24). No ETA is published, so
        // both minute fields above are `null` and nothing may fill them in.
        "delivery_fee_baht": zone.as_ref().and_then(|z| crate::db::orders::finite_money(Some(z.fee))),
    })))
}

/// Public order details endpoint used by the order detail screen. Returns the
/// full order row and requires the caller to prove ownership via Telegram
/// initData. The admin-only [`get_order`] endpoint is kept unchanged.
async fn get_order_details(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let tid = params
        .get("telegram_id")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    validate_telegram_id_param(tid)?;
    let _owner_id = check_owner(&headers, &state, tid)?;

    use crate::db::entities::order::Entity as OrderEntity;
    use sea_orm::EntityTrait;
    let order_id = id.clone();
    let model = OrderEntity::find_by_id(order_id)
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("get_order_details SeaORM error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let Some(model) = model else {
        return Err(StatusCode::NOT_FOUND);
    };
    if model.telegram_id != Some(tid) {
        return Err(StatusCode::NOT_FOUND);
    }

    // Loop #14: surface cashback earned on this order so the success screen
    // and order detail can reinforce the retention value immediately.
    let cashback_credited = BtEntity::find()
        .filter(BtCol::RelatedOrderId.eq(id.clone()))
        .filter(BtCol::TxType.eq("order_cashback"))
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("get_order_details cashback lookup: {e:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map(|tx| tx.amount);

    let mut order_json = serde_json::to_value(Order::for_customer(model)).map_err(|e| {
        tracing::error!("get_order_details serialize: {e:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    if let Some(amount) = cashback_credited {
        order_json["cashback_credited"] = json!(amount);
    }

    Ok(Json(json!({ "order": order_json })))
}

pub(crate) fn validate_update_order_status(id: &str, status: &str) -> Result<(), StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if status.len() > 50 {
        return Err(StatusCode::BAD_REQUEST);
    }
    const VALID_STATUSES: &[&str] = &[
        "pending",
        "confirmed",
        "preparing",
        "ready",
        "out_for_delivery",
        "delivered",
        "completed", // legacy alias for delivered
        "rejected",
        "cancelled",
    ];
    if !VALID_STATUSES.contains(&status) {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

async fn update_order_status(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateOrderStatusRequest>,
) -> Result<Json<Value>, StatusCode> {
    validate_update_order_status(&id, &req.status)?;
    check_admin(&headers, &state)?;
    // Cycle #88: full SeaORM. Three branches:
    //   1. status==completed → delegate to complete_order_and_update_loyalty (SeaORM tx).
    //   2. status==rejected → SeaORM tx that FOR UPDATE-locks the order, refunds
    //      bonus_used if the order was still live, then flips status.
    //   3. anything else → simple update_many with NOT_FOUND.
    // The initial "is this order in a terminal state already?" guard reads
    // the row outside the tx (it'd be a wasted lock for the 99% case where
    // the status is pending/confirmed).
    use crate::db::entities::{
        loyalty_profile::{ActiveModel as LpAm, Column as LpCol, Entity as LpEntity},
        order::{Column as OrderCol, Entity as OrderEntity},
        stars_transaction::{ActiveModel as StarsTxAm, Entity as StarsTxEntity},
        user_stars::{ActiveModel as UsAm, Column as UsCol, Entity as UsEntity},
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{
        ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QuerySelect, TransactionTrait,
    };

    let current = OrderEntity::find_by_id(id.clone())
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("update_order_status read: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let Some(current) = current else {
        return Err(StatusCode::NOT_FOUND);
    };
    let cur_status = current.status.as_str();
    let is_terminal = matches!(
        cur_status,
        "completed" | "delivered" | "rejected" | "cancelled"
    );
    if is_terminal && req.status != cur_status {
        return Err(StatusCode::BAD_REQUEST);
    }

    if req.status == "completed" || req.status == "delivered" {
        // Cycle #171: `complete_order_and_update_loyalty` is now the
        // canonical completion point — loyalty tier (cycle #88) +
        // garden seed (cycle #168) + referral bonus (cycle #170
        // added it duplicated here; #171 lifted into the function).
        // This call site owns no completion side effects. Future
        // completion paths get the full chain for free.
        if let Err(e) = crate::db::orders::complete_order_and_update_loyalty(
            &state.db.orm,
            &id,
            state.config.referral_welcome_bonus,
        )
        .await
        {
            tracing::error!(
                "update_order_status: complete_order_and_update_loyalty error: {}",
                e
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    } else if req.status == "rejected" {
        // Reject path: refund bonus_used if any, then flip status. The
        // FOR UPDATE on the order row serialises against concurrent admin
        // actions and against the create_order tx's idempotency path.
        let tx = state.db.orm.begin().await.map_err(|e| {
            tracing::error!("reject tx.begin: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let locked = OrderEntity::find_by_id(id.clone())
            .lock_exclusive()
            .one(&tx)
            .await
            .map_err(|e| {
                tracing::error!("reject FOR UPDATE read: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        if let Some(o) = locked {
            // Refund only if the order isn't already in a terminal state.
            // (cycle #76 `should_refund_bonus` helper lives in bot/callbacks
            // for the duplicate refund path there.)
            if !matches!(
                o.status.as_str(),
                "rejected" | "completed" | "delivered" | "cancelled"
            ) {
                // D9: a refund of an unreadable figure cannot be silently
                // rounded to nothing. `bonus_used` is a `NOT NULL DEFAULT 0`
                // column on the frozen migration 001, so absent is not
                // expressible here and the refund must still skip — but it
                // now says so, loudly, instead of looking like an order that
                // used no bonus.
                let bonus = match crate::db::orders::finite_money(Some(o.bonus_used)) {
                    Some(b) => b,
                    None => {
                        tracing::error!(
                            order_id = %o.id,
                            bonus_used = o.bonus_used,
                            "reject: bonus_used is not a finite non-negative number; \
                             no bonus refunded — needs a manual adjustment"
                        );
                        0.0
                    }
                };
                if bonus > 0.0 {
                    if let Some(tid) = o.telegram_id {
                        // Ensure loyalty_profile exists (idempotent).
                        let lp_seed = LpAm {
                            telegram_id: Set(tid),
                            bonus_balance: Set(Some(0.0)),
                            total_spent: Set(Some(0.0)),
                            ..Default::default()
                        };
                        LpEntity::insert(lp_seed)
                            .on_conflict(
                                OnConflict::column(LpCol::TelegramId)
                                    .do_nothing()
                                    .to_owned(),
                            )
                            .do_nothing()
                            .exec(&tx)
                            .await
                            .map_err(|e| {
                                tracing::error!("reject loyalty seed: {:?}", e);
                                StatusCode::INTERNAL_SERVER_ERROR
                            })?;
                        // Bump bonus_balance (column-expr; pattern #11).
                        LpEntity::update_many()
                            .col_expr(
                                LpCol::BonusBalance,
                                sea_orm::sea_query::Expr::cust_with_values(
                                    "bonus_balance + $1",
                                    [bonus],
                                ),
                            )
                            .filter(LpCol::TelegramId.eq(tid))
                            .exec(&tx)
                            .await
                            .map_err(|e| {
                                tracing::error!("reject bonus refund: {:?}", e);
                                StatusCode::INTERNAL_SERVER_ERROR
                            })?;
                    }
                }
                // Refund Stars that were debited at checkout (cycle #172).
                let stars = o.stars_used.max(0);
                if stars > 0 {
                    if let Some(tid) = o.telegram_id {
                        UsEntity::insert(UsAm {
                            telegram_id: Set(tid),
                            balance: Set(0),
                            ..Default::default()
                        })
                        .on_conflict(
                            OnConflict::column(UsCol::TelegramId)
                                .update_column(UsCol::UpdatedAt)
                                .to_owned(),
                        )
                        .exec(&tx)
                        .await
                        .map_err(|e| {
                            tracing::error!("reject stars refund seed: {:?}", e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;
                        UsEntity::update_many()
                            .col_expr(
                                UsCol::Balance,
                                sea_orm::sea_query::Expr::cust_with_values("balance + $1", [stars]),
                            )
                            .filter(UsCol::TelegramId.eq(tid))
                            .exec(&tx)
                            .await
                            .map_err(|e| {
                                tracing::error!("reject stars refund: {:?}", e);
                                StatusCode::INTERNAL_SERVER_ERROR
                            })?;
                        let balance_after = UsEntity::find_by_id(tid)
                            .one(&tx)
                            .await
                            .map_err(|e| {
                                tracing::error!("reject stars balance read: {:?}", e);
                                StatusCode::INTERNAL_SERVER_ERROR
                            })?
                            .map(|r| r.balance)
                            .unwrap_or(0);
                        StarsTxEntity::insert(StarsTxAm {
                            id: Set(uuid::Uuid::new_v4().to_string()),
                            telegram_id: Set(tid),
                            amount: Set(stars),
                            balance_after: Set(balance_after),
                            source: Set("plot".to_string()),
                            reason: Set("refund".to_string()),
                            external_tx_id: Set(None),
                            related_order_id: Set(Some(id.clone())),
                            ..Default::default()
                        })
                        .exec(&tx)
                        .await
                        .map_err(|e| {
                            tracing::error!("reject stars transaction: {:?}", e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;
                    }
                }
            }
        }
        // Flip status (no-op tolerant: if the row vanished between the
        // initial read and the tx, return 404).
        let updated = OrderEntity::update_many()
            .col_expr(
                OrderCol::Status,
                sea_orm::sea_query::Expr::value(req.status.clone()),
            )
            .filter(OrderCol::Id.eq(id.clone()))
            .exec(&tx)
            .await
            .map_err(|e| {
                tracing::error!("reject status update: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        if updated.rows_affected == 0 {
            // tx drops → auto-rollback.
            return Err(StatusCode::NOT_FOUND);
        }
        tx.commit().await.map_err(|e| {
            tracing::error!("reject commit: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    } else {
        // Non-terminal transition: simple update_many.
        let updated = OrderEntity::update_many()
            .col_expr(
                OrderCol::Status,
                sea_orm::sea_query::Expr::value(req.status.clone()),
            )
            .filter(OrderCol::Id.eq(id.clone()))
            .exec(&state.db.orm)
            .await
            .map_err(|e| {
                tracing::error!("status update: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        if updated.rows_affected == 0 {
            return Err(StatusCode::NOT_FOUND);
        }
    }

    crate::metrics::order_status_changed(&req.status);

    // Cycle #9: push the new status to the customer for every milestone,
    // including non-terminal transitions (preparing/ready/out_for_delivery).
    if let Some(customer_tid) = current.telegram_id {
        let bot = state.bot.clone();
        let db = state.db.clone();
        let config = state.config.clone();
        let status_for_notify = req.status.clone();
        let order_id_for_notify = id.clone();
        tokio::spawn(async move {
            crate::bot::notify::notify_order_status(
                &bot,
                &db,
                &config,
                customer_tid,
                &order_id_for_notify,
                &status_for_notify,
                None,
            )
            .await;
        });
    }

    Ok(Json(json!({ "success": true })))
}

/// Cycle #94: customer self-service cancellation. Only allowed while the
/// order is still `pending`. Refunds any bonus/stars spent and notifies the
/// customer so the UI can update live.
async fn cancel_order(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let tid = params
        .get("telegram_id")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    validate_telegram_id_param(tid)?;
    let _owner_id = check_owner(&headers, &state, tid)?;
    check_not_blocked(&state, tid).await?;

    use crate::db::entities::{
        loyalty_profile::{ActiveModel as LpAm, Column as LpCol, Entity as LpEntity},
        order::{Column as OrderCol, Entity as OrderEntity},
        stars_transaction::{ActiveModel as StarsTxAm, Entity as StarsTxEntity},
        user_stars::{ActiveModel as UsAm, Column as UsCol, Entity as UsEntity},
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{
        ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QuerySelect, TransactionTrait,
    };

    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("cancel_order tx.begin: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let locked = OrderEntity::find_by_id(id.clone())
        .lock_exclusive()
        .one(&tx)
        .await
        .map_err(|e| {
            tracing::error!("cancel_order FOR UPDATE read: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let Some(o) = locked else {
        return Err(StatusCode::NOT_FOUND);
    };
    if o.telegram_id != Some(tid) {
        return Err(StatusCode::NOT_FOUND);
    }
    if o.status != "pending" {
        return Err(StatusCode::CONFLICT);
    }

    // D9: same as the reject path — skipping the refund is forced by the
    // `NOT NULL DEFAULT 0` column, but it is recorded rather than hidden.
    let bonus = match crate::db::orders::finite_money(Some(o.bonus_used)) {
        Some(b) => b,
        None => {
            tracing::error!(
                order_id = %o.id,
                bonus_used = o.bonus_used,
                "cancel_order: bonus_used is not a finite non-negative number; \
                 no bonus refunded — needs a manual adjustment"
            );
            0.0
        }
    };
    if bonus > 0.0 {
        let lp_seed = LpAm {
            telegram_id: Set(tid),
            bonus_balance: Set(Some(0.0)),
            total_spent: Set(Some(0.0)),
            ..Default::default()
        };
        LpEntity::insert(lp_seed)
            .on_conflict(
                OnConflict::column(LpCol::TelegramId)
                    .do_nothing()
                    .to_owned(),
            )
            .do_nothing()
            .exec(&tx)
            .await
            .map_err(|e| {
                tracing::error!("cancel_order loyalty seed: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        LpEntity::update_many()
            .col_expr(
                LpCol::BonusBalance,
                sea_orm::sea_query::Expr::cust_with_values("bonus_balance + $1", [bonus]),
            )
            .filter(LpCol::TelegramId.eq(tid))
            .exec(&tx)
            .await
            .map_err(|e| {
                tracing::error!("cancel_order bonus refund: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }
    let stars = o.stars_used.max(0);
    if stars > 0 {
        UsEntity::insert(UsAm {
            telegram_id: Set(tid),
            balance: Set(0),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::column(UsCol::TelegramId)
                .update_column(UsCol::UpdatedAt)
                .to_owned(),
        )
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("cancel_order stars seed: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        UsEntity::update_many()
            .col_expr(
                UsCol::Balance,
                sea_orm::sea_query::Expr::cust_with_values("balance + $1", [stars]),
            )
            .filter(UsCol::TelegramId.eq(tid))
            .exec(&tx)
            .await
            .map_err(|e| {
                tracing::error!("cancel_order stars refund: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        let balance_after = UsEntity::find_by_id(tid)
            .one(&tx)
            .await
            .map_err(|e| {
                tracing::error!("cancel_order stars balance read: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .map(|r| r.balance)
            .unwrap_or(0);
        StarsTxEntity::insert(StarsTxAm {
            id: Set(uuid::Uuid::new_v4().to_string()),
            telegram_id: Set(tid),
            amount: Set(stars),
            balance_after: Set(balance_after),
            source: Set("plot".to_string()),
            reason: Set("cancel".to_string()),
            external_tx_id: Set(None),
            related_order_id: Set(Some(id.clone())),
            ..Default::default()
        })
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("cancel_order stars transaction: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    let updated = OrderEntity::update_many()
        .col_expr(
            OrderCol::Status,
            sea_orm::sea_query::Expr::value("cancelled"),
        )
        .filter(OrderCol::Id.eq(id.clone()))
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("cancel_order status update: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if updated.rows_affected == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    tx.commit().await.map_err(|e| {
        tracing::error!("cancel_order commit: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let bot = state.bot.clone();
    let config = state.config.clone();
    let db = state.db.clone();
    let order_id = id.clone();
    tokio::spawn(async move {
        crate::bot::notify::notify_order_status(
            &bot,
            &db,
            &config,
            tid,
            &order_id,
            "cancelled",
            None,
        )
        .await;
    });

    crate::metrics::order_cancelled_by_user();

    Ok(Json(json!({ "success": true, "status": "cancelled" })))
}

async fn get_user_orders(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    crate::api::auth::check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;
    // Cycle #85 Part 1: SeaORM filter + order + limit.
    use crate::db::entities::order::{Column as OrderCol, Entity as OrderEntity};
    use sea_orm::{
        ColumnTrait, EntityTrait, Order as SortOrder, QueryFilter, QueryOrder, QuerySelect,
    };
    let models = OrderEntity::find()
        .filter(OrderCol::TelegramId.eq(telegram_id))
        .order_by(OrderCol::CreatedAt, SortOrder::Desc)
        .limit(50)
        .all(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("get_user_orders SeaORM error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let orders: Vec<Order> = models.into_iter().map(Order::for_customer).collect();
    Ok(Json(json!({ "orders": orders })))
}

/// Public delivery zones and fees; the ETA pair is `null` unless a row stores
/// one. No auth — used by the customer checkout and order tracker.
async fn list_delivery_zones(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    use crate::db::entities::delivery_zone::{Column as ZoneCol, Entity as ZoneEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
    let models = ZoneEntity::find()
        .filter(ZoneCol::IsActive.eq(true))
        .order_by_asc(ZoneCol::SortOrder)
        .order_by_asc(ZoneCol::Name)
        .all(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("list_delivery_zones: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let zones: Vec<Value> = models
        .into_iter()
        .map(|z| {
            json!({
                "id": z.id,
                "name": z.name,
                "name_en": z.name_en,
                "min_eta_minutes": crate::delivery::zone_eta_min(&z),
                "max_eta_minutes": crate::delivery::zone_eta_max(&z),
                // D9: `null`, not `0` — see `get_order_status`. A zone whose
                // fee is undocumented renders as a dash and the customer is
                // told a human quotes it, rather than being shown free
                // delivery to the far end of the island.
                "delivery_fee_baht": crate::db::orders::finite_money(Some(z.fee)),
            })
        })
        .collect();
    Ok(Json(json!({ "zones": zones })))
}

/// Admin-only SVG QR code for PromptPay payment of an order. Falls back to
/// a text QR if no merchant ID is configured.
async fn promptpay_qr(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    use crate::db::entities::order::Entity as OrderEntity;
    use sea_orm::EntityTrait;
    let model = OrderEntity::find_by_id(id.clone())
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("promptpay_qr order lookup: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let order = model.ok_or(StatusCode::NOT_FOUND)?;
    let total = promptpay_qr_amount(&id, order.total)?;
    let payload = build_payload(&state.config.promptpay, total, &id);
    let svg = svg_qr(&payload).map_err(|e| {
        tracing::error!("promptpay_qr render: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/svg+xml")], svg).into_response())
}

/// The amount a PromptPay QR may carry for an order whose stored total is
/// `total`, or the refusal.
///
/// D9: a QR is a payment instruction, so there is no honest fallback. The
/// old `else { 0.0 }` built a scannable 0-baht code for an order whose
/// total was NaN — the customer would have paid nothing and both sides
/// would have believed the bill was settled. Refuse loudly instead.
///
/// T27 D4 (2026-09-24): the same code was still drawn for a total of exactly
/// zero, because [`finite_money`](crate::db::orders::finite_money) keeps a
/// zero. A zero total is a real figure — a bike-only order stores 0 because
/// the rental is not this server's sum ([`check_full_subtotal`]), and an
/// order the discounts paid in full floors at 0 — but there is nothing to
/// collect, and a code that "settles" 0 baht while the rental money is still
/// owed is the defect above with a finite number in it. So a total that is
/// not strictly positive gets no QR: an unusable stored figure stays the
/// server's own fault (500), a zero is refused as the caller's request (422).
///
/// CORRECTED 2026-09-24 (review of D4): "strictly positive" was read off the
/// stored figure, and the QR does not carry the stored figure. It prints the
/// total at two decimals (`{:.2}` in `amount_tlv` and `fallback_payload`,
/// `src/promptpay.rs`), and `create_order` stores the body's total whenever
/// [`validate_create_order`] finds it within a hundredth of the expected one,
/// so a bike-only order may store 0.004: that was drawn as a scannable "0.00".
/// The test is therefore on the satang the code would carry: under one
/// satang after rounding is the zero case (422). One satang (0.01) still
/// gets its QR: it is a real amount the same tolerance can store, and the
/// smallest the code can carry.
fn promptpay_qr_amount(order_id: &str, total: f64) -> Result<f64, StatusCode> {
    let Some(total) = crate::db::orders::finite_money(Some(total)) else {
        tracing::error!(
            order_id = %order_id,
            total = total,
            "promptpay_qr: order total is not a finite non-negative number; refusing to render a QR"
        );
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    if (total * 100.0).round() < 1.0 {
        tracing::info!(
            order_id = %order_id,
            total = total,
            "promptpay_qr: order total rounds to zero satang; there is nothing to collect, refusing to render a QR"
        );
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::{
        admin_item_line, check_and_record, check_full_subtotal, deposit_refusal,
        is_valid_idempotency_key, new_store, promptpay_qr_amount, rate_divergence,
        rental_deposit_refusal, thb_or_dash, validate_bike_lines, validate_create_order,
        validate_update_order_status, CreateOrderRequest, DepositRefusal, FullSubtotalCheck,
        PriceCatalog, ANON_ORDER_RL_MAX_ATTEMPTS, ANON_ORDER_RL_MAX_IPS, ANON_ORDER_RL_WINDOW,
        MAX_BIKE_UNITS_PER_LINE, MAX_RENTAL_DAYS,
    };

    // ── Idempotency-key validator (cycle #57) ────────────────────────────

    #[test]
    fn idempotency_key_accepts_uuid_v4_shape() {
        assert!(is_valid_idempotency_key(
            "550e8400-e29b-41d4-a716-446655440000"
        ));
    }

    #[test]
    fn idempotency_key_accepts_nanoid_style() {
        assert!(is_valid_idempotency_key("V1StGXR8_Z5jdHi6B-myT"));
    }

    #[test]
    fn idempotency_key_rejects_empty() {
        assert!(!is_valid_idempotency_key(""));
    }

    #[test]
    fn idempotency_key_rejects_too_long() {
        assert!(!is_valid_idempotency_key(&"a".repeat(101)));
    }

    #[test]
    fn idempotency_key_rejects_whitespace_inside() {
        assert!(!is_valid_idempotency_key("has space"));
    }

    #[test]
    fn idempotency_key_rejects_control_bytes() {
        assert!(!is_valid_idempotency_key("line\nbreak"));
        assert!(!is_valid_idempotency_key("tab\there"));
    }

    #[test]
    fn idempotency_key_rejects_header_or_sql_injection_chars() {
        assert!(!is_valid_idempotency_key("path/inject"));
        assert!(!is_valid_idempotency_key("sql'inject"));
        assert!(!is_valid_idempotency_key("hdr:inject"));
        assert!(!is_valid_idempotency_key("\"quoted\""));
    }

    use crate::db::orders::{BikeDeal, BikeLine, DepositForm, OrderItem};
    use axum::http::StatusCode;

    /// A legacy (pre-rebrand) line: it still carries a `strain_id`, which is
    /// all the validator tests need. It is deliberately unpriceable now — the
    /// `strains` table is gone, so `check_full_subtotal` reports it as
    /// `Malformed`, which is the honest answer for a product that cannot be
    /// sold any more.
    fn strain_item(id: &str, qty: f64) -> OrderItem {
        OrderItem {
            strain_id: Some(id.into()),
            strain_name: Some(id.into()),
            accessory_id: None,
            accessory_name: None,
            tea_id: None,
            tea_name: None,
            set_id: None,
            set_name: None,
            quantity: qty,
            unit_price: None,
            is_set: None,
            is_accessory: None,
            is_tea: None,
            is_tea_set: None,
            fulfillment: None,
            bike: None,
        }
    }

    /// A bike line. `deal` carries the whole rental-or-sale distinction, so
    /// one fixture covers both.
    fn bike_item(key: &str, units: f64, deal: BikeDeal) -> OrderItem {
        OrderItem {
            strain_id: None,
            strain_name: None,
            accessory_id: None,
            accessory_name: None,
            tea_id: None,
            tea_name: None,
            set_id: None,
            set_name: None,
            quantity: units,
            unit_price: None,
            is_set: None,
            is_accessory: None,
            is_tea: None,
            is_tea_set: None,
            fulfillment: None,
            bike: Some(BikeLine {
                bike_key: key.into(),
                bike_name: None,
                deal,
            }),
        }
    }

    fn day(y: i32, m: u32, d: u32) -> chrono::NaiveDate {
        match chrono::NaiveDate::from_ymd_opt(y, m, d) {
            Some(d) => d,
            None => panic!("test fixture has an impossible date {y}-{m}-{d}"),
        }
    }

    /// A 7-day rental starting the day after `today`, with the figures the
    /// door quoted for `nmax-155` in `data/fleet_seed.json`.
    fn rental(today: chrono::NaiveDate) -> BikeDeal {
        BikeDeal::BikeRental {
            rental_start: today + chrono::Duration::days(1),
            rental_end: today + chrono::Duration::days(7),
            rate_thb_day: Some(449.0),
            deposit: Some(DepositForm::Money {
                amount: Some(3000.0),
                currency: Some("THB".into()),
                method: Some("cash THB".into()),
            }),
        }
    }

    // The six `check_strain_subtotal` tests that stood here (exact match,
    // mismatch, unknown strain, the two mixed-cart cases, and the two
    // sale-window cases) went with the helper and the `strains` table. The
    // sale-precedence behaviour they were really guarding lives in
    // `trios::pricing` and is still covered for accessories, tea and sets by
    // the `full_check_*` tests below.

    // ── check_full_subtotal — every priced catalog (cycle #58 / C) ──────────

    fn accessory_item_with(id: &str, qty: f64) -> OrderItem {
        OrderItem {
            strain_id: None,
            strain_name: None,
            accessory_id: Some(id.into()),
            accessory_name: Some(id.into()),
            tea_id: None,
            tea_name: None,
            set_id: None,
            set_name: None,
            quantity: qty,
            unit_price: None,
            is_set: None,
            is_accessory: Some(true),
            is_tea: None,
            is_tea_set: None,
            fulfillment: None,
            bike: None,
        }
    }

    fn tea_item_with(id: &str, qty: f64) -> OrderItem {
        OrderItem {
            strain_id: None,
            strain_name: None,
            accessory_id: None,
            accessory_name: None,
            tea_id: Some(id.into()),
            tea_name: Some(id.into()),
            set_id: None,
            set_name: None,
            quantity: qty,
            unit_price: None,
            is_set: None,
            is_accessory: None,
            is_tea: Some(true),
            is_tea_set: None,
            fulfillment: None,
            bike: None,
        }
    }

    fn set_item_with(id: &str, qty: f64) -> OrderItem {
        OrderItem {
            strain_id: None,
            strain_name: None,
            accessory_id: None,
            accessory_name: None,
            tea_id: None,
            tea_name: None,
            set_id: Some(id.into()),
            set_name: Some(id.into()),
            quantity: qty,
            unit_price: None,
            is_set: Some(true),
            is_accessory: None,
            is_tea: None,
            is_tea_set: None,
            fulfillment: None,
            bike: None,
        }
    }

    #[test]
    fn full_check_sums_every_priced_catalog() {
        let mut cat = PriceCatalog::default();
        cat.accessories.insert("a1", (250.0, true));
        cat.tea_products.insert("t1", (80.0, true));
        cat.sets.insert("set1", (1000.0, 10.0, true)); // 900
        let items = vec![
            accessory_item_with("a1", 1.0), // 250
            tea_item_with("t1", 3.0),       // 240
            set_item_with("set1", 1.0),     // 900
        ];
        // 250 + 240 + 900 = 1390
        assert_eq!(
            check_full_subtotal(&items, &cat, 1390.0, 0.01),
            FullSubtotalCheck::Ok
        );
    }

    // The `item_unit_price` / `item_matches_id` tests and the two garden
    // money-path tests (`validate_order_garden_reward_relaxes_total_downward_only`,
    // `garden_discount_arithmetic`) went with the B4 mechanic under D5. The
    // strict total identity the first of them relaxed is now asserted
    // unconditionally by `test_validate_total_mismatch`.

    // ── Bike lines contribute nothing to the subtotal (D11) ─────────────

    #[test]
    fn full_check_bike_only_cart_claims_zero() {
        // A rental is quoted at the door. Nothing about it belongs in the
        // cart subtotal, so a bike-only cart legitimately claims 0 — and a
        // claim of the day rate, or of rate × days, is a mismatch.
        let cat = PriceCatalog::default();
        let items = vec![bike_item("nmax-155", 1.0, rental(day(2026, 9, 12)))];
        assert_eq!(
            check_full_subtotal(&items, &cat, 0.0, 0.01),
            FullSubtotalCheck::Ok
        );
        match check_full_subtotal(&items, &cat, 449.0 * 7.0, 0.01) {
            FullSubtotalCheck::Mismatch { claimed, expected } => {
                assert!((claimed - 3143.0).abs() < 1e-9);
                assert!(expected.abs() < 1e-9);
            }
            other => panic!("expected Mismatch, got {other:?}"),
        }
    }

    #[test]
    fn full_check_bike_line_does_not_zero_out_an_accessory() {
        // The attack the branch ordering exists to stop: attach an empty
        // rental to a 250-baht helmet and claim the line is free. The
        // accessory branch is checked first, so the catalog still prices it.
        let mut cat = PriceCatalog::default();
        cat.accessories.insert("a1", (250.0, true));
        let mut item = accessory_item_with("a1", 1.0);
        item.bike = Some(BikeLine {
            bike_key: "nmax-155".into(),
            bike_name: None,
            deal: BikeDeal::BikeSale { price_thb: None },
        });
        match check_full_subtotal(&[item], &cat, 0.0, 0.01) {
            FullSubtotalCheck::Mismatch { expected, .. } => {
                assert!((expected - 250.0).abs() < 1e-9);
            }
            other => panic!("expected Mismatch, got {other:?}"),
        }
    }

    #[test]
    fn full_check_empty_id_string_is_not_an_unknown_item() {
        // Old clients send `""` where they mean `null`. Routing that into the
        // accessory branch refused the order with an empty id in the audit
        // log; a bike line carrying one is priced as a bike.
        let cat = PriceCatalog::default();
        let mut item = bike_item("nmax-155", 1.0, BikeDeal::BikeSale { price_thb: None });
        item.accessory_id = Some(String::new());
        assert_eq!(
            check_full_subtotal(&[item], &cat, 0.0, 0.01),
            FullSubtotalCheck::Ok
        );
    }

    #[test]
    fn full_check_fails_closed_on_a_nan_quantity() {
        // The boundary validator rejects these, but if one ever slipped past,
        // the old `else { 0.0 }` priced the line at zero and accepted a
        // claimed subtotal of zero with it.
        let mut cat = PriceCatalog::default();
        cat.accessories.insert("a1", (250.0, true));
        let items = vec![accessory_item_with("a1", f64::NAN)];
        assert_eq!(
            check_full_subtotal(&items, &cat, 0.0, 0.01),
            FullSubtotalCheck::Malformed
        );
    }

    #[test]
    fn full_check_flags_unavailable_accessory() {
        let mut cat = PriceCatalog::default();
        cat.accessories.insert("a1", (250.0, false));
        let items = vec![accessory_item_with("a1", 1.0)];
        match check_full_subtotal(&items, &cat, 250.0, 0.01) {
            FullSubtotalCheck::Unavailable { catalog: c, id } => {
                assert_eq!(c, "accessories");
                assert_eq!(id, "a1");
            }
            other => panic!("expected Unavailable(accessories), got {:?}", other),
        }
    }

    #[test]
    fn full_check_flags_unknown_set() {
        let cat = PriceCatalog::default();
        let items = vec![set_item_with("missing", 1.0)];
        match check_full_subtotal(&items, &cat, 999.0, 0.01) {
            FullSubtotalCheck::UnknownItem { catalog: c, id } => {
                assert_eq!(c, "sets");
                assert_eq!(id, "missing");
            }
            other => panic!("expected UnknownItem(sets), got {:?}", other),
        }
    }

    #[test]
    fn full_check_mismatch_reports_expected_and_claimed() {
        let mut cat = PriceCatalog::default();
        cat.accessories.insert("a1", (250.0, true));
        let items = vec![accessory_item_with("a1", 1.0)];
        // Client claims 1 baht
        match check_full_subtotal(&items, &cat, 1.0, 0.01) {
            FullSubtotalCheck::Mismatch { claimed, expected } => {
                assert!((claimed - 1.0).abs() < 1e-9);
                assert!((expected - 250.0).abs() < 1e-9);
            }
            other => panic!("expected Mismatch, got {:?}", other),
        }
    }

    #[test]
    fn full_check_malformed_when_no_ids_and_no_bike() {
        let cat = PriceCatalog::default();
        let mut item = strain_item("s1", 1.0);
        item.strain_id = None; // every *_id is None and there is no bike
        match check_full_subtotal(&[item], &cat, 0.0, 0.01) {
            FullSubtotalCheck::Malformed => {}
            other => panic!("expected Malformed, got {:?}", other),
        }
    }

    #[test]
    fn full_check_malformed_for_a_line_that_still_names_a_strain() {
        // `083_drop_cannabis_catalog.sql` took the table. A cart line that
        // still carries only a `strain_id` cannot be priced and must not be
        // silently treated as free.
        let cat = PriceCatalog::default();
        match check_full_subtotal(&[strain_item("s1", 1.0)], &cat, 0.0, 0.01) {
            FullSubtotalCheck::Malformed => {}
            other => panic!("expected Malformed, got {:?}", other),
        }
    }

    fn valid_req() -> CreateOrderRequest {
        CreateOrderRequest {
            telegram_id: Some(1),
            customer_name: Some("Alice".into()),
            customer_phone: Some("+12345".into()),
            customer_telegram: Some("alice".into()),
            items: vec![OrderItem {
                strain_id: Some("s1".into()),
                strain_name: Some("Legacy line".into()),
                accessory_id: None,
                accessory_name: None,
                tea_id: None,
                tea_name: None,
                set_id: None,
                set_name: None,
                quantity: 1.0,
                unit_price: None,
                is_set: None,
                is_accessory: None,
                is_tea: None,
                is_tea_set: None,
                fulfillment: None,
                bike: None,
            }],
            subtotal: 100.0,
            bonus_used: Some(10.0),
            stars_used: None,
            total: 90.0,
            shop_id: None,
            delivery_address: Some("123 Test Lane".into()),
            delivery_notes: None,
            age_confirmed: Some(true),
            delivery_zone_id: Some("thongsala".into()),
        }
    }

    #[test]
    fn test_validate_ok() {
        let req = valid_req();
        assert_eq!(validate_create_order(&req).unwrap().bonus_used, 10.0);
    }

    #[test]
    fn test_validate_no_bonus() {
        let mut req = valid_req();
        req.bonus_used = None;
        req.total = 100.0;
        assert_eq!(validate_create_order(&req).unwrap().bonus_used, 0.0);
    }

    #[test]
    fn test_validate_empty_items() {
        let mut req = valid_req();
        req.items = vec![];
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_too_many_items() {
        let mut req = valid_req();
        req.items = (0..101)
            .map(|_| OrderItem {
                strain_id: Some("s".into()),
                strain_name: Some("X".into()),
                accessory_id: None,
                accessory_name: None,
                tea_id: None,
                tea_name: None,
                set_id: None,
                set_name: None,
                quantity: 1.0,
                unit_price: None,
                is_set: None,
                is_accessory: None,
                is_tea: None,
                is_tea_set: None,
                fulfillment: None,
                bike: None,
            })
            .collect();
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_accepts_the_phone_shapes_customers_actually_type() {
        // Server and UI share `normalize_phone`, so anything the customer can
        // now get past the order button must also pass here. Before this the
        // server demanded a literal leading '+'.
        for phone in [
            "0812345678",
            "081 234 5678",
            "081-234-5678",
            "+66812345678",
            "+66 81 234 5678",
            "0066812345678",
            "8 999 123-45-67",
        ] {
            let mut req = valid_req();
            req.customer_phone = Some(phone.to_string());
            assert!(
                validate_create_order(&req).is_ok(),
                "{phone} should be accepted"
            );
        }
    }

    #[test]
    fn validate_still_rejects_non_phone_input() {
        for phone in ["", "abc", "12", "+1"] {
            let mut req = valid_req();
            req.customer_phone = Some(phone.to_string());
            assert_eq!(
                validate_create_order(&req).unwrap_err(),
                StatusCode::BAD_REQUEST,
                "{phone:?} should be rejected"
            );
        }
    }

    #[test]
    fn validate_allows_pickup_order_without_address() {
        // An in-store order carries no delivery address. The server must not
        // be the thing that blocks it.
        let mut req = valid_req();
        req.delivery_address = None;
        assert!(validate_create_order(&req).is_ok());
    }

    #[test]
    fn validate_requires_explicit_age_confirmation() {
        // Compliance gate: never inferred, never defaulted.
        for age in [None, Some(false)] {
            let mut req = valid_req();
            req.age_confirmed = age;
            assert_eq!(
                validate_create_order(&req).unwrap_err(),
                StatusCode::UNPROCESSABLE_ENTITY,
                "age_confirmed={age:?} must not create an order"
            );
        }
    }

    #[test]
    fn validate_rejects_empty_cart() {
        let mut req = valid_req();
        req.items = Vec::new();
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_name_too_long() {
        let mut req = valid_req();
        req.customer_name = Some("a".repeat(201));
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_phone_too_long() {
        let mut req = valid_req();
        req.customer_phone = Some("a".repeat(51));
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_telegram_too_long() {
        let mut req = valid_req();
        req.customer_telegram = Some("a".repeat(101));
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_shop_id_too_long() {
        let mut req = valid_req();
        req.shop_id = Some("a".repeat(201));
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_strain_id_too_long() {
        let mut req = valid_req();
        req.items[0].strain_id = Some("a".repeat(201));
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_negative_total() {
        let mut req = valid_req();
        req.total = -1.0;
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_nan_total() {
        let mut req = valid_req();
        req.total = f64::NAN;
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_negative_quantity() {
        let mut req = valid_req();
        req.items[0].quantity = -1.0;
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_bonus_exceeds_subtotal() {
        let mut req = valid_req();
        req.bonus_used = Some(101.0);
        req.total = -1.0; // will fail before math check, but let's set valid total
        req.total = 0.0;
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_total_mismatch() {
        let mut req = valid_req();
        req.total = 95.0; // expected 90.0
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_total_tolerance() {
        let mut req = valid_req();
        req.total = 90.009; // within 0.01 of expected 90.0
        assert_eq!(validate_create_order(&req).unwrap().bonus_used, 10.0);
    }

    #[test]
    fn test_validate_update_order_status_ok() {
        assert!(validate_update_order_status("abc123", "confirmed").is_ok());
    }

    #[test]
    fn test_validate_update_order_status_id_too_long() {
        assert_eq!(
            validate_update_order_status(&"a".repeat(201), "confirmed").unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_update_order_status_too_long() {
        assert_eq!(
            validate_update_order_status("abc", &"a".repeat(51)).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_update_order_status_invalid() {
        assert_eq!(
            validate_update_order_status("abc", "shipped").unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    /// Cycle #105: verifies the per-IP anonymous-order rate limit
    /// actually enforces the configured cap. Uses an isolated store so
    /// the global `ANON_ORDER_RATE_LIMIT` isn't perturbed by test order.
    #[tokio::test]
    async fn anon_order_rate_limit_blocks_after_max_attempts() {
        let store = new_store();
        let ip = "203.0.113.42";
        // First N attempts pass.
        for i in 0..ANON_ORDER_RL_MAX_ATTEMPTS {
            assert!(
                check_and_record(
                    &store,
                    ip,
                    ANON_ORDER_RL_WINDOW,
                    ANON_ORDER_RL_MAX_ATTEMPTS,
                    ANON_ORDER_RL_MAX_IPS,
                )
                .await,
                "attempt {} should pass under the cap",
                i
            );
        }
        // (N+1)th is blocked.
        assert!(
            !check_and_record(
                &store,
                ip,
                ANON_ORDER_RL_WINDOW,
                ANON_ORDER_RL_MAX_ATTEMPTS,
                ANON_ORDER_RL_MAX_IPS,
            )
            .await,
            "attempt past the cap should be rate-limited"
        );
    }

    #[tokio::test]
    async fn anon_order_rate_limit_isolates_per_ip() {
        let store = new_store();
        // Exhaust IP-A.
        for _ in 0..ANON_ORDER_RL_MAX_ATTEMPTS {
            assert!(
                check_and_record(
                    &store,
                    "203.0.113.1",
                    ANON_ORDER_RL_WINDOW,
                    ANON_ORDER_RL_MAX_ATTEMPTS,
                    ANON_ORDER_RL_MAX_IPS,
                )
                .await
            );
        }
        assert!(
            !check_and_record(
                &store,
                "203.0.113.1",
                ANON_ORDER_RL_WINDOW,
                ANON_ORDER_RL_MAX_ATTEMPTS,
                ANON_ORDER_RL_MAX_IPS,
            )
            .await
        );
        // IP-B is unaffected.
        assert!(
            check_and_record(
                &store,
                "203.0.113.2",
                ANON_ORDER_RL_WINDOW,
                ANON_ORDER_RL_MAX_ATTEMPTS,
                ANON_ORDER_RL_MAX_IPS,
            )
            .await
        );
    }

    // ── validate_create_order upper-bound tests (cycle #152) ─────────────

    fn valid_order_request() -> CreateOrderRequest {
        CreateOrderRequest {
            telegram_id: Some(42),
            customer_name: None,
            customer_phone: Some("+12345".into()),
            customer_telegram: None,
            items: vec![strain_item("s1", 1.0)],
            subtotal: 100.0,
            bonus_used: Some(0.0),
            stars_used: None,
            total: 100.0,
            shop_id: None,
            delivery_address: None,
            delivery_notes: None,
            age_confirmed: Some(true),
            delivery_zone_id: None,
        }
    }

    #[test]
    fn validate_create_order_baseline_ok() {
        assert!(validate_create_order(&valid_order_request()).is_ok());
    }

    #[test]
    fn validate_create_order_rejects_quantity_above_max() {
        let mut req = valid_order_request();
        req.items[0].quantity = 10_000.01;
        req.subtotal = 1_000_001.0;
        req.total = 1_000_001.0;
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_create_order_accepts_quantity_at_max() {
        let mut req = valid_order_request();
        req.items[0].quantity = 10_000.0;
        // subtotal/total don't matter here — the helper only checks
        // their bounds and the (subtotal - bonus = total) cross-check.
        req.subtotal = 100.0;
        req.total = 100.0;
        assert!(validate_create_order(&req).is_ok());
    }

    #[test]
    fn validate_create_order_rejects_subtotal_above_max() {
        let mut req = valid_order_request();
        req.subtotal = 100_000_000.01;
        req.total = 100_000_000.01;
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_create_order_rejects_total_above_max() {
        let mut req = valid_order_request();
        // Force just total above the cap while keeping subtotal valid.
        // The (subtotal - bonus ≈ total) cross-check fires before the
        // upper-bound check if the inequality isn't satisfied, so make
        // bonus_used absorb the gap.
        req.subtotal = 100.0;
        req.bonus_used = Some(0.0);
        req.total = 100_000_000.01;
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_create_order_rejects_inflated_quantity_attack() {
        // The cycle-#152 motivator: a malicious client sends
        // `quantity = 1e18` matched by a `subtotal` that the
        // server-side price-authority check could be tricked into
        // accepting. Boundary validator catches it.
        let mut req = valid_order_request();
        req.items[0].quantity = 1e18;
        req.subtotal = 1e20;
        req.total = 1e20;
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    // ── validate_bike_lines (issues #14 / #15) ───────────────────────────

    /// A fixed "today" so the date rules are deterministic. Every rental
    /// fixture is expressed relative to it.
    fn today() -> chrono::NaiveDate {
        day(2026, 9, 13)
    }

    #[test]
    fn bike_lines_accept_a_plain_rental() {
        let items = vec![bike_item("nmax-155", 2.0, rental(today()))];
        assert!(validate_bike_lines(&items, today()).is_ok());
    }

    #[test]
    fn bike_lines_accept_a_rental_starting_today() {
        // Walk-in: the customer is at the counter now. The boundary must not
        // be the thing that refuses same-day hire.
        let items = vec![bike_item(
            "nmax-155",
            1.0,
            BikeDeal::BikeRental {
                rental_start: today(),
                rental_end: today(),
                rate_thb_day: Some(449.0),
                deposit: Some(DepositForm::Passport),
            },
        )];
        assert!(validate_bike_lines(&items, today()).is_ok());
    }

    #[test]
    fn bike_lines_accept_absent_money() {
        // D9/D11: the door was silent. That is a legitimate order — a human
        // quotes it — and must not be turned into a number here.
        let items = vec![
            bike_item(
                "xadv-750",
                1.0,
                BikeDeal::BikeRental {
                    rental_start: today() + chrono::Duration::days(2),
                    rental_end: today() + chrono::Duration::days(9),
                    rate_thb_day: None,
                    deposit: None,
                },
            ),
            // A sale line stood here until 2026-09-24: see bike_sale_lines_are_retired.
        ];
        assert!(validate_bike_lines(&items, today()).is_ok());
    }

    #[test]
    fn bike_lines_reject_zero_money() {
        // `Some(0.0)` is the failure D9 exists to stop: a zero rate reads as
        // "free" and a zero deposit as "nothing owed". Absence is `None`.
        let zero_rate = vec![bike_item(
            "nmax-155",
            1.0,
            BikeDeal::BikeRental {
                rental_start: today(),
                rental_end: today(),
                rate_thb_day: Some(0.0),
                deposit: None,
            },
        )];
        assert_eq!(
            validate_bike_lines(&zero_rate, today()).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
        let zero_deposit = vec![bike_item(
            "nmax-155",
            1.0,
            BikeDeal::BikeRental {
                rental_start: today(),
                rental_end: today(),
                rate_thb_day: Some(449.0),
                deposit: Some(DepositForm::Money {
                    amount: Some(0.0),
                    currency: Some("THB".into()),
                    method: Some("cash THB".into()),
                }),
            },
        )];
        assert_eq!(
            validate_bike_lines(&zero_deposit, today()).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
        let zero_sale = vec![bike_item(
            "cb-650r",
            1.0,
            BikeDeal::BikeSale {
                price_thb: Some(0.0),
            },
        )];
        assert_eq!(
            validate_bike_lines(&zero_sale, today()).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn bike_lines_reject_nan_and_absurd_money() {
        for bad in [f64::NAN, f64::INFINITY, -1.0, 1_000_000.01] {
            let items = vec![bike_item(
                "cb-650r",
                1.0,
                BikeDeal::BikeSale {
                    price_thb: Some(bad),
                },
            )];
            assert_eq!(
                validate_bike_lines(&items, today()).unwrap_err(),
                StatusCode::BAD_REQUEST,
                "{bad} should be rejected"
            );
        }
    }

    #[test]
    fn bike_lines_reject_a_rental_that_started_in_the_past() {
        // A cart left open overnight, not a malformed payload — 422.
        let items = vec![bike_item(
            "nmax-155",
            1.0,
            BikeDeal::BikeRental {
                rental_start: today() - chrono::Duration::days(1),
                rental_end: today() + chrono::Duration::days(3),
                rate_thb_day: Some(449.0),
                deposit: Some(DepositForm::Passport),
            },
        )];
        assert_eq!(
            validate_bike_lines(&items, today()).unwrap_err(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[test]
    fn bike_lines_reject_backwards_and_overlong_terms() {
        let backwards = vec![bike_item(
            "nmax-155",
            1.0,
            BikeDeal::BikeRental {
                rental_start: today() + chrono::Duration::days(5),
                rental_end: today() + chrono::Duration::days(2),
                rate_thb_day: None,
                deposit: None,
            },
        )];
        assert_eq!(
            validate_bike_lines(&backwards, today()).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
        // A typo'd year must not become a 180-year booking. Both ends are
        // inclusive, so MAX_RENTAL_DAYS - 1 days apart is exactly the cap.
        let at_cap = vec![bike_item(
            "nmax-155",
            1.0,
            BikeDeal::BikeRental {
                rental_start: today(),
                rental_end: today() + chrono::Duration::days(MAX_RENTAL_DAYS - 1),
                rate_thb_day: None,
                deposit: None,
            },
        )];
        assert!(validate_bike_lines(&at_cap, today()).is_ok());
        let over_cap = vec![bike_item(
            "nmax-155",
            1.0,
            BikeDeal::BikeRental {
                rental_start: today(),
                rental_end: today() + chrono::Duration::days(MAX_RENTAL_DAYS),
                rate_thb_day: None,
                deposit: None,
            },
        )];
        assert_eq!(
            validate_bike_lines(&over_cap, today()).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn bike_lines_reject_fractional_and_absurd_unit_counts() {
        let fractional = vec![bike_item("nmax-155", 1.5, rental(today()))];
        assert_eq!(
            validate_bike_lines(&fractional, today()).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
        let at_cap = vec![bike_item(
            "nmax-155",
            MAX_BIKE_UNITS_PER_LINE,
            rental(today()),
        )];
        assert!(validate_bike_lines(&at_cap, today()).is_ok());
        let over_cap = vec![bike_item(
            "nmax-155",
            MAX_BIKE_UNITS_PER_LINE + 1.0,
            rental(today()),
        )];
        assert_eq!(
            validate_bike_lines(&over_cap, today()).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn bike_lines_reject_the_drink_fulfillment_vocabulary() {
        // D7: a machine is picked up or delivered. "dine_in" on a bike line
        // means the cart was built by the wrong screen.
        for bad in ["dine_in", "takeaway", "", "teleport"] {
            let mut items = vec![bike_item("nmax-155", 1.0, rental(today()))];
            items[0].fulfillment = Some(bad.into());
            assert_eq!(
                validate_bike_lines(&items, today()).unwrap_err(),
                StatusCode::BAD_REQUEST,
                "fulfillment={bad:?} should be rejected on a bike line"
            );
        }
        for good in ["delivery", "pickup"] {
            let mut items = vec![bike_item("nmax-155", 1.0, rental(today()))];
            items[0].fulfillment = Some(good.into());
            assert!(
                validate_bike_lines(&items, today()).is_ok(),
                "fulfillment={good:?} should be accepted"
            );
        }
    }

    #[test]
    fn bike_lines_reject_a_line_that_is_also_a_catalog_item() {
        let mut items = vec![bike_item("nmax-155", 1.0, rental(today()))];
        items[0].accessory_id = Some("helmet-1".into());
        assert_eq!(
            validate_bike_lines(&items, today()).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
        // An empty string is absence, not a catalog item.
        items[0].accessory_id = Some(String::new());
        assert!(validate_bike_lines(&items, today()).is_ok());
    }

    #[test]
    fn bike_lines_require_a_family_key() {
        let mut items = vec![bike_item("", 1.0, rental(today()))];
        assert_eq!(
            validate_bike_lines(&items, today()).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
        if let Some(b) = items[0].bike.as_mut() {
            b.bike_key = "a".repeat(201);
        }
        assert_eq!(
            validate_bike_lines(&items, today()).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn bike_lines_ignore_legacy_lines() {
        // The `items` JSONB still holds pre-rebrand lines with no `bike` at
        // all. They are none of this validator's business.
        let items = vec![strain_item("s1", 2.5), accessory_item_with("a1", 1.0)];
        assert!(validate_bike_lines(&items, today()).is_ok());
    }

    // ── D11 divergence logging ───────────────────────────────────────────

    #[test]
    fn rate_divergence_compares_quote_against_the_discounted_file_rate() {
        // `nmax-155` publishes 449 ฿/day PRE-discount and is a scooter (25 %),
        // so the file implies 449 × 0.75 = 336.75 → 337 ฿. A door quote of the
        // pre-discount figure diverges by 112 ฿ and must be logged.
        assert_eq!(
            rate_divergence(Some(449.0), Some(449.0), Some(0.25)),
            Some((449.0, 337.0))
        );
        // The discounted figure agrees, and so does a sub-baht difference.
        assert_eq!(rate_divergence(Some(337.0), Some(449.0), Some(0.25)), None);
        assert_eq!(rate_divergence(Some(337.4), Some(449.0), Some(0.25)), None);
    }

    #[test]
    fn rate_divergence_stays_silent_when_either_number_is_absent() {
        // Nothing to reconcile, and nothing that may be invented: an unseeded
        // discount table must never make the pre-discount tariff the
        // "expected" number.
        assert_eq!(rate_divergence(None, Some(449.0), Some(0.25)), None);
        assert_eq!(rate_divergence(Some(449.0), None, Some(0.25)), None);
        assert_eq!(rate_divergence(Some(449.0), Some(449.0), None), None);
        assert_eq!(
            rate_divergence(Some(f64::NAN), Some(449.0), Some(0.25)),
            None
        );
    }

    // ── The admin card (D9) ──────────────────────────────────────────────

    #[test]
    fn thb_or_dash_never_shows_zero_for_an_absent_figure() {
        assert_eq!(thb_or_dash(Some(449.0)), "449 ฿");
        assert_eq!(thb_or_dash(None), "—");
        assert_eq!(thb_or_dash(Some(f64::NAN)), "—");
        assert_eq!(thb_or_dash(Some(-1.0)), "—");
        // A genuine zero is still a figure someone agreed; it is the *absent*
        // one that must not read as zero.
        assert_eq!(thb_or_dash(Some(0.0)), "0 ฿");
    }

    #[test]
    fn admin_card_renders_a_rental_without_inventing_a_total() {
        let line = admin_item_line(&bike_item("nmax-155", 2.0, rental(today())));
        assert!(line.contains("nmax-155"), "{line}");
        assert!(line.contains("2026-09-14"), "{line}");
        assert!(line.contains("2026-09-20"), "{line}");
        assert!(line.contains("7 дн."), "{line}");
        assert!(line.contains("449 ฿"), "{line}");
        assert!(line.contains("3000 THB"), "{line}");
        assert!(line.contains("cash THB"), "{line}");
        // The forbidden number: 449 × 7 = 3143.
        assert!(!line.contains("3143"), "{line}");
        // And no grams, whatever the rebrand left behind.
        assert!(!line.contains('g'), "{line}");
    }

    #[test]
    fn admin_card_dashes_an_unquoted_rental_and_names_the_passport() {
        let line = admin_item_line(&bike_item(
            "xadv-750",
            1.0,
            BikeDeal::BikeRental {
                rental_start: today(),
                rental_end: today(),
                rate_thb_day: None,
                deposit: Some(DepositForm::Passport),
            },
        ));
        assert!(line.contains("ставка/день: —"), "{line}");
        assert!(line.contains("залог: паспорт"), "{line}");
        // The dash, not a figure: nothing about this rental is priced yet.
        assert!(!line.contains('฿'), "no invented figure: {line}");
    }

    #[test]
    fn admin_card_dashes_a_deposit_that_is_not_agreed_yet() {
        // `None` is "not agreed yet", never "nothing owed".
        let line = admin_item_line(&bike_item(
            "nmax-155",
            1.0,
            BikeDeal::BikeRental {
                rental_start: today(),
                rental_end: today(),
                rate_thb_day: Some(449.0),
                deposit: None,
            },
        ));
        assert!(line.contains("залог: —"), "{line}");
    }

    #[test]
    fn admin_card_renders_a_sale_and_falls_back_to_the_family_key() {
        let line = admin_item_line(&bike_item(
            "cb-650r",
            1.0,
            BikeDeal::BikeSale { price_thb: None },
        ));
        assert!(line.contains("продажа"), "{line}");
        assert!(line.contains("цена: —"), "{line}");
        assert!(line.contains("cb-650r"), "{line}");
    }

    #[test]
    fn admin_card_keeps_the_short_form_for_a_catalog_line() {
        let line = admin_item_line(&accessory_item_with("a1", 3.0));
        assert_eq!(line, "  • a1 × 3");
    }

    // ── PromptPay QR amount (T27 D4, 2026-09-24) ─────────────────────────

    #[test]
    fn a_zero_total_is_refused_a_promptpay_qr_rather_than_drawn_as_one() {
        // A bike-only order stores subtotal 0 and total 0 (the rental is not
        // this server's sum), and an order the discounts paid in full floors at
        // 0. Either way there is nothing to collect: a scannable code for 0
        // baht is a payment instruction that settles nothing. A 4xx, never an
        // SVG.
        assert_eq!(
            promptpay_qr_amount("o1", 0.0),
            Err(StatusCode::UNPROCESSABLE_ENTITY)
        );
        assert_eq!(
            promptpay_qr_amount("o1", -0.0),
            Err(StatusCode::UNPROCESSABLE_ENTITY)
        );
        // Review fix: a total the create path can store (within a hundredth of
        // an expected 0) that the code would print as "0.00".
        assert_eq!(
            promptpay_qr_amount("o1", 0.004),
            Err(StatusCode::UNPROCESSABLE_ENTITY)
        );
    }

    #[test]
    fn the_refusal_follows_the_two_decimals_the_code_prints() {
        // The QR prints `{:.2}` (src/promptpay.rs), so "nothing to collect" is
        // decided on that text, not on the stored figure. Half a satang rounds
        // up to one, and is printed as one.
        for total in [0.0, 0.001, 0.004, 0.0049, 0.005, 0.006, 0.01, 0.1, 449.0] {
            assert_eq!(
                promptpay_qr_amount("o1", total).is_err(),
                format!("{total:.2}") == "0.00",
                "{total}"
            );
        }
    }

    #[test]
    fn a_positive_total_gets_its_qr_and_an_unusable_one_stays_a_server_fault() {
        assert_eq!(promptpay_qr_amount("o1", 449.0), Ok(449.0));
        // One satang is a real amount the create path's hundredth can store,
        // and the smallest the code can carry, so it keeps its QR.
        assert_eq!(promptpay_qr_amount("o1", 0.01), Ok(0.01));
        // Nobody computed these: the stored row is broken, which is the
        // server's fault and not the caller's (unchanged by D4).
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
            assert_eq!(
                promptpay_qr_amount("o1", bad),
                Err(StatusCode::INTERNAL_SERVER_ERROR),
                "{bad}"
            );
        }
    }

    // ── Rental deposit authority (T27 C3, 2026-09-24) ────────────────────

    fn money(amount: Option<f64>, currency: Option<&str>) -> DepositForm {
        DepositForm::Money {
            amount,
            currency: currency.map(str::to_string),
            method: Some("cash THB".into()),
        }
    }

    /// A rental of `key` starting tomorrow whose deposit is `deposit`.
    fn rental_with(key: &str, deposit: Option<DepositForm>) -> OrderItem {
        bike_item(
            key,
            1.0,
            BikeDeal::BikeRental {
                rental_start: today() + chrono::Duration::days(1),
                rental_end: today() + chrono::Duration::days(7),
                rate_thb_day: Some(449.0),
                deposit,
            },
        )
    }

    #[test]
    fn a_money_deposit_must_be_the_familys_published_deposit() {
        // nmax-155 publishes 3000 (data/fleet_seed.json; the `rental` fixture).
        let published = Some(3000.0);
        let refused = |amount: f64, currency: Option<&str>| {
            deposit_refusal(Some(&money(Some(amount), currency)), published)
        };
        assert_eq!(refused(3000.0, Some("THB")), None);
        // The lowered deposit #28 describes: bounded, and until now stored.
        assert_eq!(
            refused(2000.0, Some("THB")),
            Some(DepositRefusal::Differs {
                claimed: 2000.0,
                published: 3000.0
            })
        );
        // A raised one is no better: the figure is not the body's to name.
        assert_eq!(
            refused(3500.0, Some("THB")),
            Some(DepositRefusal::Differs {
                claimed: 3500.0,
                published: 3000.0
            })
        );
        // No currency named reads as the published one, so only the
        // published figure passes; the ISO code is not case-sensitive.
        assert_eq!(refused(3000.0, None), None);
        assert_eq!(
            refused(2999.0, None),
            Some(DepositRefusal::Differs {
                claimed: 2999.0,
                published: 3000.0
            })
        );
        assert_eq!(refused(3000.0, Some("thb")), None);
    }

    #[test]
    fn a_money_deposit_on_a_family_that_publishes_none_is_refused() {
        // click-125 publishes no deposit. An unusable stored figure is the
        // same absence, never a deposit of zero (D9).
        for published in [None, Some(0.0), Some(-1.0), Some(f64::NAN)] {
            assert_eq!(
                deposit_refusal(Some(&money(Some(2000.0), Some("THB"))), published),
                Some(DepositRefusal::NotPublished),
                "{published:?}"
            );
            assert_eq!(
                deposit_refusal(Some(&money(None, None)), published),
                Some(DepositRefusal::NotPublished),
                "{published:?}"
            );
        }
    }

    #[test]
    fn the_passport_and_a_figure_not_agreed_yet_are_still_accepted() {
        for published in [Some(3000.0), None] {
            assert_eq!(
                deposit_refusal(Some(&DepositForm::Passport), published),
                None
            );
            assert_eq!(deposit_refusal(None, published), None);
        }
        // Money with no figure yet states a form, not an amount.
        assert_eq!(
            deposit_refusal(Some(&money(None, Some("THB"))), Some(3000.0)),
            None
        );
        assert_eq!(
            deposit_refusal(Some(&money(None, Some("USD"))), Some(3000.0)),
            None
        );
    }

    #[test]
    fn a_foreign_currency_figure_cannot_be_compared_and_is_refused() {
        // No exchange rate is published (D18); the equivalent is agreed at
        // the door, so a figure in another currency has no authority here.
        assert_eq!(
            deposit_refusal(Some(&money(Some(100.0), Some("USD"))), Some(3000.0)),
            Some(DepositRefusal::NotComparable {
                currency: "USD".into()
            })
        );
    }

    #[test]
    fn every_rental_line_of_the_family_is_held_to_its_deposit() {
        let agreed = || rental_with("nmax-155", Some(money(Some(3000.0), Some("THB"))));
        let lowered = rental_with("nmax-155", Some(money(Some(1000.0), Some("THB"))));
        // Another family's line is judged against its own figure, not this one.
        let other = || rental_with("xmax-300", Some(money(Some(1000.0), Some("THB"))));
        let sale = || bike_item("nmax-155", 1.0, BikeDeal::BikeSale { price_thb: None });
        assert_eq!(
            rental_deposit_refusal(&[agreed(), sale(), other()], "nmax-155", Some(3000.0)),
            None
        );
        assert_eq!(
            rental_deposit_refusal(&[agreed(), lowered, other()], "nmax-155", Some(3000.0)),
            Some(DepositRefusal::Differs {
                claimed: 1000.0,
                published: 3000.0
            })
        );
        // The passport stands on a family that publishes no deposit.
        assert_eq!(
            rental_deposit_refusal(
                &[rental_with("nmax-155", Some(DepositForm::Passport))],
                "nmax-155",
                None
            ),
            None
        );
    }

    #[test]
    fn a_multi_unit_line_carries_the_per_unit_deposit_not_the_line_total() {
        // Documents the rule as shipped (2026-09-24), not an owner decision:
        // `deposit_thb` is one bike's deposit, `quantity` is not read, so a
        // line of two units passes with the one-bike figure and is refused with
        // twice it. Per line or per unit is an open owner question.
        let two_units = |figure: f64| {
            bike_item(
                "nmax-155",
                2.0,
                BikeDeal::BikeRental {
                    rental_start: today() + chrono::Duration::days(1),
                    rental_end: today() + chrono::Duration::days(7),
                    rate_thb_day: Some(449.0),
                    deposit: Some(money(Some(figure), Some("THB"))),
                },
            )
        };
        assert_eq!(
            rental_deposit_refusal(&[two_units(3000.0)], "nmax-155", Some(3000.0)),
            None
        );
        assert_eq!(
            rental_deposit_refusal(&[two_units(6000.0)], "nmax-155", Some(3000.0)),
            Some(DepositRefusal::Differs {
                claimed: 6000.0,
                published: 3000.0
            })
        );
    }

    /// Owner decision 2026-09-24 (rental only, Phuket only): bike sales are
    /// retired from every customer surface, so a NEW sale line is refused at
    /// the boundary -- 422, like the other "you cannot have this" refusals,
    /// with no new sentence. A malformed price is still malformed first (the
    /// 400 tests above are unchanged), and `BikeDeal::BikeSale` stays in the
    /// vocabulary because placed orders carry it.
    #[test]
    fn bike_sale_lines_are_retired() {
        for price in [None, Some(250_000.0)] {
            let items = vec![bike_item(
                "cb-650r",
                1.0,
                BikeDeal::BikeSale { price_thb: price },
            )];
            assert_eq!(
                validate_bike_lines(&items, today()).unwrap_err(),
                StatusCode::UNPROCESSABLE_ENTITY,
                "a sale line priced {price:?} was admitted"
            );
        }
        // A sale line cannot ride along with a rental either.
        let mixed = vec![
            bike_item(
                "xadv-750",
                1.0,
                BikeDeal::BikeRental {
                    rental_start: today() + chrono::Duration::days(2),
                    rental_end: today() + chrono::Duration::days(9),
                    rate_thb_day: None,
                    deposit: None,
                },
            ),
            bike_item("cb-650r", 1.0, BikeDeal::BikeSale { price_thb: None }),
        ];
        assert_eq!(
            validate_bike_lines(&mixed, today()).unwrap_err(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        // The rental alone is still admitted with its money absent (D9/D11).
        assert!(validate_bike_lines(&mixed[..1], today()).is_ok());
    }
}
