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

use crate::api::auth::{check_admin, check_not_blocked, validate_telegram_id_param};
use crate::api::rate_limit::{
    check_and_record, client_ip_from_headers, new_store, SlidingWindowStore,
};
use crate::db::orders::{Order, OrderItem};
use crate::db::strains::Strain;
use crate::promptpay::{build_payload, svg_qr};
use crate::trios::pricing::{
    effective_accessory_price, effective_set_price, effective_strain_price, effective_tea_price,
    MarketingFlags,
};
use crate::AppState;
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
    /// B4: an optional garden reward to apply (product-scoped discount). The
    /// server loads the reward, computes the discount from DB prices, and
    /// verifies `total = subtotal - bonus_used - stars_used - garden_discount`
    /// — the client can't set the discount amount itself.
    #[serde(default)]
    pub garden_reward_id: Option<String>,
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
        .route("/orders/:id/status", put(update_order_status))
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
    // B4: when a garden reward is applied the total is further reduced by a
    // server-computed product discount, so the exact equality is deferred to
    // create_order (which knows the discount). Here we only require the claimed
    // total not to EXCEED the no-discount expected (a reward can only lower it).
    if req
        .garden_reward_id
        .as_deref()
        .is_some_and(|s| !s.is_empty())
    {
        if req.total > expected_total + 0.01 {
            return Err(StatusCode::BAD_REQUEST);
        }
    } else if (req.total - expected_total).abs() > 0.01 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(ValidatedPayment {
        bonus_used,
        stars_used,
    })
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

/// Outcome of the server-side strain-subtotal check (cycle #56). Distinct
/// variants so audit logs can tell stale-cart / typo / fraud apart.
///
/// Cycle #78: superseded in production by [`check_full_subtotal`] which
/// covers every catalog (strains + accessories + tea + sets). The
/// strain-only helper is kept as a focused unit-test target for the
/// strain-portion logic — useful when adding a new strain-pricing edge
/// case without paying the cost of building a full mixed-cart fixture.
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub(crate) enum SubtotalCheck {
    /// Server-computed strain portion matches the client-claimed subtotal
    /// within tolerance (strain-only orders) or fits within it (mixed orders).
    Ok,
    /// Item references an unknown `strain_id` — caller may be referencing a
    /// deleted strain, or fabricated the id outright.
    UnknownStrain(String),
    /// Mixed order: the strain portion alone *exceeds* the claimed subtotal,
    /// which is impossible unless the client lied about prices.
    StrainExceedsSubtotal { strain: f64, claimed: f64 },
    /// Strain-only order: server total disagrees with claimed by more than
    /// `tolerance`. `expected` and `claimed` go in audit logs but MUST NOT be
    /// echoed to the client (anti price-probing).
    StrainOnlyMismatch { claimed: f64, expected: f64 },
}

/// Server-authoritative price check for the strain portion of an order.
///
/// Pure helper — does no IO. Caller fetches the strain rows and assembles
/// the `strain_map`. Uses `trios::pricing::effective_strain_price` so the
/// precedence rules cannot drift from the customer-facing menu.
///
/// Semantics:
/// * Strain-only order (no `accessory_id` / `tea_id` / `set_id`): the server-
///   computed strain subtotal must equal `claimed_subtotal` within `tolerance`.
/// * Mixed order: only check that the strain portion alone does not exceed
///   the claimed subtotal. Accessory / tea / set price authority is a
///   separate cycle; until then, trust the client for those.
/// * Unknown strain id: short-circuit with `UnknownStrain`.
#[allow(dead_code)]
pub(crate) fn check_strain_subtotal(
    items: &[OrderItem],
    strain_map: &HashMap<&str, &Strain>,
    claimed_subtotal: f64,
    tolerance: f64,
    now: chrono::DateTime<chrono::Utc>,
) -> SubtotalCheck {
    let mut strain_sum = 0.0_f64;
    let mut has_non_strain = false;
    for item in items {
        if let Some(sid) = item.strain_id.as_deref() {
            let Some(strain) = strain_map.get(sid) else {
                return SubtotalCheck::UnknownStrain(sid.to_string());
            };
            let flags = MarketingFlags {
                price_per_gram: strain.price_per_gram,
                is_strain_of_day: strain.is_strain_of_day,
                strain_of_day_discount: strain.strain_of_day_discount,
                sale_active: strain.sale_active,
                sale_until: strain.sale_until.as_deref(),
                sale_price: strain.sale_price,
                discount_percent: strain.discount_percent,
                is_new_arrival: strain.is_new_arrival,
                new_until: strain.new_until.as_deref(),
            };
            let priced = effective_strain_price(&flags, now);
            let qty = if item.quantity.is_finite() {
                item.quantity.max(0.0)
            } else {
                0.0
            };
            strain_sum += priced.price * qty;
        } else if item.accessory_id.is_some() || item.tea_id.is_some() || item.set_id.is_some() {
            has_non_strain = true;
        }
    }
    if has_non_strain {
        if strain_sum > claimed_subtotal + tolerance {
            return SubtotalCheck::StrainExceedsSubtotal {
                strain: strain_sum,
                claimed: claimed_subtotal,
            };
        }
        return SubtotalCheck::Ok;
    }
    // Strain-only path — strict equality.
    if (strain_sum - claimed_subtotal).abs() > tolerance {
        return SubtotalCheck::StrainOnlyMismatch {
            claimed: claimed_subtotal,
            expected: strain_sum,
        };
    }
    SubtotalCheck::Ok
}

/// Catalog lookups for `check_full_subtotal` (cycle #58 / C). Built once per
/// order from the four catalog SELECTs and handed in by reference.
#[derive(Debug, Default)]
pub(crate) struct PriceCatalog<'a> {
    pub strains: HashMap<&'a str, &'a Strain>,
    /// `id → (price, is_available)`.
    pub accessories: HashMap<&'a str, (f64, bool)>,
    pub tea_products: HashMap<&'a str, (f64, bool)>,
    /// `id → (total_price, discount_percent, is_available)` — same shape for
    /// `sets`, `accessory_sets`, and `tea_sets` (UUID primary keys don't
    /// collide across the three tables, so one map covers them all).
    pub sets: HashMap<&'a str, (f64, f64, bool)>,
}

/// Result of the full server-side price-authority check (cycle #58 / C).
/// Strict equality across all four catalogs combined.
#[derive(Debug, PartialEq)]
pub(crate) enum FullSubtotalCheck {
    Ok,
    /// `catalog` is one of `"strains" | "accessories" | "tea_products" | "sets"`
    /// so audit logs can pinpoint which catalog the missing id belongs to.
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
    /// Line item is missing every `*_id` — malformed payload.
    Malformed,
}

/// Server-authoritative subtotal check for every catalog: strain, accessory,
/// tea product, set. Pure helper — caller fetches the rows in advance.
///
/// Routing precedence (first match per item):
///   1. `strain_id`     → strains (uses `trios::pricing::effective_strain_price`)
///   2. `accessory_id`  → accessories
///   3. `tea_id`        → tea_products
///   4. `set_id`        → sets / accessory_sets / tea_sets (unified by UUID)
///
/// Strict equality required: a mixed order with an unauthorised price on any
/// line item fails the check even if other lines compensate.
pub(crate) fn check_full_subtotal(
    items: &[OrderItem],
    catalog: &PriceCatalog<'_>,
    claimed_subtotal: f64,
    tolerance: f64,
    now: chrono::DateTime<chrono::Utc>,
) -> FullSubtotalCheck {
    let mut sum = 0.0_f64;
    for item in items {
        let qty = if item.quantity.is_finite() {
            item.quantity.max(0.0)
        } else {
            0.0
        };
        let unit = if let Some(sid) = item.strain_id.as_deref() {
            let Some(strain) = catalog.strains.get(sid) else {
                return FullSubtotalCheck::UnknownItem {
                    catalog: "strains",
                    id: sid.into(),
                };
            };
            if !strain.is_available {
                return FullSubtotalCheck::Unavailable {
                    catalog: "strains",
                    id: sid.into(),
                };
            }
            let flags = MarketingFlags {
                price_per_gram: strain.price_per_gram,
                is_strain_of_day: strain.is_strain_of_day,
                strain_of_day_discount: strain.strain_of_day_discount,
                sale_active: strain.sale_active,
                sale_until: strain.sale_until.as_deref(),
                sale_price: strain.sale_price,
                discount_percent: strain.discount_percent,
                is_new_arrival: strain.is_new_arrival,
                new_until: strain.new_until.as_deref(),
            };
            effective_strain_price(&flags, now).price
        } else if let Some(aid) = item.accessory_id.as_deref() {
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
        } else if let Some(tid) = item.tea_id.as_deref() {
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
        } else if let Some(sid) = item.set_id.as_deref() {
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

/// True if `item` references catalog product `id` in any of its `*_id` fields.
pub(crate) fn item_matches_id(item: &OrderItem, id: &str) -> bool {
    item.strain_id.as_deref() == Some(id)
        || item.accessory_id.as_deref() == Some(id)
        || item.tea_id.as_deref() == Some(id)
        || item.set_id.as_deref() == Some(id)
}

/// Server-authoritative UNIT price for one order item — same catalog + effective
/// price logic as `check_full_subtotal`, but per-item. `None` if the product is
/// unknown or unavailable. Used by the B4 garden discount so the reduction is
/// computed server-side (never trusting a client-sent amount).
pub(crate) fn item_unit_price(
    item: &OrderItem,
    catalog: &PriceCatalog<'_>,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<f64> {
    if let Some(sid) = item.strain_id.as_deref() {
        let strain = catalog.strains.get(sid)?;
        if !strain.is_available {
            return None;
        }
        let flags = MarketingFlags {
            price_per_gram: strain.price_per_gram,
            is_strain_of_day: strain.is_strain_of_day,
            strain_of_day_discount: strain.strain_of_day_discount,
            sale_active: strain.sale_active,
            sale_until: strain.sale_until.as_deref(),
            sale_price: strain.sale_price,
            discount_percent: strain.discount_percent,
            is_new_arrival: strain.is_new_arrival,
            new_until: strain.new_until.as_deref(),
        };
        Some(effective_strain_price(&flags, now).price)
    } else if let Some(aid) = item.accessory_id.as_deref() {
        let &(price, avail) = catalog.accessories.get(aid)?;
        if !avail {
            return None;
        }
        Some(effective_accessory_price(price))
    } else if let Some(tid) = item.tea_id.as_deref() {
        let &(price, avail) = catalog.tea_products.get(tid)?;
        if !avail {
            return None;
        }
        Some(effective_tea_price(price))
    } else if let Some(sid) = item.set_id.as_deref() {
        let &(tp, dp, avail) = catalog.sets.get(sid)?;
        if !avail {
            return None;
        }
        Some(effective_set_price(tp, dp))
    } else {
        None
    }
}

async fn create_order(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<CreateOrderRequest>,
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

    // B4: load + validate an applied garden reward (product-scoped discount).
    // Returns (reward_id, target_product_id, percent) on success. Every failure
    // path is a 422 (the client claimed a reward it can't use) — never a silent
    // accept. The discount AMOUNT is computed later from DB prices, not here.
    let garden_reward: Option<(String, String, u32)> = {
        use sea_orm::{ConnectionTrait, DbBackend, Statement};
        if let Some(rid) = req.garden_reward_id.as_deref().filter(|s| !s.is_empty()) {
            if rid.len() > 200 {
                return Err(StatusCode::BAD_REQUEST);
            }
            let Some(uid) = req.telegram_id else {
                // Rewards belong to an authenticated user; anon can't apply one.
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            };
            let now_ms = chrono::Utc::now().timestamp_millis();
            let row = state
                .db
                .orm
                .query_one(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "SELECT discount_percent, target_product_id, scope, is_used, expires_at, user_id \
                     FROM garden_rewards WHERE id = $1",
                    [rid.into()],
                ))
                .await
                .map_err(|e| {
                    error!("create_order: garden reward lookup failed: {e}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            let Some(row) = row else {
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            };
            let r_user: String = row.try_get("", "user_id").unwrap_or_default();
            let is_used: bool = row.try_get("", "is_used").unwrap_or(true);
            let expires_at: i64 = row.try_get("", "expires_at").unwrap_or(0);
            let scope: String = row.try_get("", "scope").unwrap_or_default();
            let target: Option<String> = row
                .try_get::<Option<String>>("", "target_product_id")
                .ok()
                .flatten();
            let pct: i32 = row.try_get("", "discount_percent").unwrap_or(0);
            if r_user != uid.to_string()
                || is_used
                || expires_at <= now_ms
                || scope != "product"
                || target.is_none()
            {
                tracing::info!(
                    telegram_id = uid,
                    "create_order: garden reward not applicable (used/expired/scope/owner)"
                );
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
            let target = target.unwrap();
            // The reward's target product must actually be in the cart.
            if !req.items.iter().any(|i| item_matches_id(i, &target)) {
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
            Some((rid.to_string(), target, pct.clamp(0, 100) as u32))
        } else {
            None
        }
    };
    let mut garden_discount = 0.0_f64;

    // Cycle #58 / C: full server-side price authority across every catalog
    // (strains + accessories + tea + sets). Cycle #56 covered strains only;
    // this closes the remaining mixed-order trust path. `trios::pricing`
    // (cycle #55) keeps the math identical to the customer-facing menu so
    // legitimate orders never get flagged as fraud.
    let strain_ids: Vec<String> = req
        .items
        .iter()
        .filter_map(|i| i.strain_id.clone())
        .collect();
    let accessory_ids: Vec<String> = req
        .items
        .iter()
        .filter_map(|i| i.accessory_id.clone())
        .collect();
    let tea_ids: Vec<String> = req.items.iter().filter_map(|i| i.tea_id.clone()).collect();
    let set_ids: Vec<String> = req.items.iter().filter_map(|i| i.set_id.clone()).collect();
    if !strain_ids.is_empty()
        || !accessory_ids.is_empty()
        || !tea_ids.is_empty()
        || !set_ids.is_empty()
    {
        // Cycle #90: full SeaORM. The legacy `lookup_client = state.db.pool.get()`
        // is gone — strains go through the typed entity, accessory / tea /
        // sets go through raw `Statement` (pattern #15). This was the last
        // `state.db.pool` usage in api/orders.rs; after this cycle the
        // orders endpoint is 100% off raw Pool.
        use sea_orm::{ConnectionTrait, DbBackend, Statement};
        let mut catalog: PriceCatalog<'_> = PriceCatalog::default();
        let strains: Vec<Strain> = if !strain_ids.is_empty() {
            use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
            let models = crate::db::entities::strain::Entity::find()
                .filter(crate::db::entities::strain::Column::Id.is_in(strain_ids.clone()))
                .all(&state.db.orm)
                .await
                .map_err(|e| {
                    error!("price-auth strain lookup (SeaORM): {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            models.into_iter().map(Strain::from).collect()
        } else {
            Vec::new()
        };
        for s in &strains {
            catalog.strains.insert(s.id.as_str(), s);
        }
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

        match check_full_subtotal(&req.items, &catalog, req.subtotal, 0.01, chrono::Utc::now()) {
            FullSubtotalCheck::Ok => {
                // B4: compute the garden product-scoped discount from server-side
                // prices (the catalog is live here). Applied to the target line's
                // unit × quantity. If the target product can't be priced, reject.
                if let Some((_, ref target, pct)) = garden_reward {
                    let now_dt = chrono::Utc::now();
                    if let Some(ti) = req.items.iter().find(|i| item_matches_id(i, target)) {
                        match item_unit_price(ti, &catalog, now_dt) {
                            Some(unit) => {
                                let qty = if ti.quantity.is_finite() {
                                    ti.quantity.max(0.0)
                                } else {
                                    0.0
                                };
                                garden_discount = (unit * qty * (pct as f64) / 100.0).max(0.0);
                            }
                            None => return Err(StatusCode::UNPROCESSABLE_ENTITY),
                        }
                    }
                }
            }
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

    // B4: with a garden reward applied, the authoritative total is
    // subtotal - bonus_used - stars_used - (server-computed) garden_discount.
    // Verify the client's claimed total matches; a mismatch = tampering → reject + audit.
    if garden_reward.is_some() {
        let stars_discount = stars_used as f64;
        let expected = (req.subtotal - bonus_used - stars_discount - garden_discount).max(0.0);
        if (req.total - expected).abs() > 0.01 {
            tracing::warn!(
                telegram_id = req.telegram_id.unwrap_or(0),
                claimed_total = req.total,
                expected_total = expected,
                garden_discount,
                "create_order: garden-discount total mismatch — possible tampering"
            );
            if let Err(e) = crate::db::orders::record_fraud_event(
                &state.db.orm,
                req.telegram_id,
                crate::db::orders::FRAUD_CODE_SUBTOTAL_MISMATCH,
                None,
                None,
                Some(req.total),
                Some(expected),
            )
            .await
            {
                tracing::warn!("fraud_event audit insert failed: {}", e);
            }
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

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
        customer_phone: Set(req.customer_phone.clone()),
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
        ..Default::default()
    };
    OrderEntity::insert(order_am).exec(&tx).await.map_err(|e| {
        error!("create_order insert error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // B4: consume the applied garden reward atomically with the order. The
    // `WHERE is_used = false` makes it race-safe + idempotent: if a concurrent
    // order already used it, 0 rows → abort (tx drops → full rollback) so the
    // discount can never be applied twice.
    if let Some((ref rid, _, _)) = garden_reward {
        let used = tx
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "UPDATE garden_rewards SET is_used = true WHERE id = $1 AND is_used = false",
                [rid.clone().into()],
            ))
            .await
            .map_err(|e| {
                error!("create_order: mark garden reward used: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        if used.rows_affected() == 0 {
            return Err(StatusCode::CONFLICT);
        }
    }

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
    let items_v = items_json.clone();
    tokio::spawn(async move {
        notify_admins(
            &bot,
            &config,
            &order_id,
            &req.customer_name,
            &req.customer_telegram,
            &items_v,
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

#[allow(clippy::too_many_arguments)]
async fn notify_admins(
    bot: &teloxide::Bot,
    config: &crate::config::Config,
    order_id: &str,
    customer_name: &Option<String>,
    customer_telegram: &Option<String>,
    items: &Value,
    subtotal: f64,
    bonus_used: f64,
    stars_used: i64,
    total: f64,
) {
    use teloxide::prelude::*;
    use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

    let items_text = items
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|item| {
                    let name = item["strain_name"]
                        .as_str()
                        .or(item["accessory_name"].as_str())
                        .or(item["tea_name"].as_str())
                        .or(item["set_name"].as_str())
                        .unwrap_or("?");
                    let qty = item["quantity"].as_f64().unwrap_or(0.0);
                    // A3: show how each drink should be served.
                    let fulfillment = match item["fulfillment"].as_str() {
                        Some("dine_in") => " 🍽 на месте",
                        Some("takeaway") => " 🥡 с собой",
                        _ => "",
                    };
                    format!("  • {} × {}g{}", html_escape(name), qty, fulfillment)
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

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
        if let Err(e) =
            crate::db::orders::complete_order_and_update_loyalty(&state.db.orm, &id).await
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
                let bonus = if o.bonus_used.is_finite() {
                    o.bonus_used.max(0.0)
                } else {
                    0.0
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
    Ok(Json(json!({ "success": true })))
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
    let orders: Vec<Order> = models.into_iter().map(Order::from).collect();
    Ok(Json(json!({ "orders": orders })))
}

/// Public delivery zones + ETA/fee ranges. No auth — used by the customer
/// checkout and order tracker.
async fn list_delivery_zones(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "zones": state.config.delivery_zones.zones }))
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
    let total = if order.total.is_finite() {
        order.total
    } else {
        0.0
    };
    let payload = build_payload(&state.config.promptpay, total, &id);
    let svg = svg_qr(&payload).map_err(|e| {
        tracing::error!("promptpay_qr render: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/svg+xml")], svg).into_response())
}

#[cfg(test)]
mod tests {
    use super::{
        check_and_record, check_full_subtotal, check_strain_subtotal, is_valid_idempotency_key,
        item_matches_id, item_unit_price, new_store, validate_create_order,
        validate_update_order_status, CreateOrderRequest, FullSubtotalCheck, PriceCatalog,
        SubtotalCheck, ANON_ORDER_RL_MAX_ATTEMPTS, ANON_ORDER_RL_MAX_IPS, ANON_ORDER_RL_WINDOW,
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

    use crate::db::orders::OrderItem;
    use crate::db::strains::Strain;
    use axum::http::StatusCode;
    use std::collections::HashMap;

    fn strain(id: &str, price: f64) -> Strain {
        Strain {
            id: id.into(),
            name: id.into(),
            category: None,
            thc_percent: None,
            cbd_percent: None,
            effect: None,
            flavor_profile: None,
            description: None,
            price_per_gram: price,
            available_grams: Some(100.0),
            image_url: None,
            video_url: None,
            is_available: true,
            is_strain_of_day: false,
            strain_of_day_discount: 0.0,
            name_en: None,
            description_en: None,
            effect_en: None,
            flavor_profile_en: None,
            strain_type_en: None,
            discount_percent: 0.0,
            sale_price: None,
            sale_active: false,
            sale_until: None,
            is_best_seller: false,
            is_new_arrival: false,
            new_until: None,
            display_order: 0,
        }
    }

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
        }
    }

    fn accessory_item() -> OrderItem {
        OrderItem {
            strain_id: None,
            strain_name: None,
            accessory_id: Some("acc-1".into()),
            accessory_name: Some("Grinder".into()),
            tea_id: None,
            tea_name: None,
            set_id: None,
            set_name: None,
            quantity: 1.0,
            unit_price: None,
            is_set: None,
            is_accessory: Some(true),
            is_tea: None,
            is_tea_set: None,
            fulfillment: None,
        }
    }

    fn now_utc() -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }

    #[test]
    fn subtotal_check_strain_only_ok_at_exact_match() {
        let s = strain("s1", 100.0);
        let mut map = HashMap::new();
        map.insert(s.id.as_str(), &s);
        // 100 * 2.5 = 250
        let items = vec![strain_item("s1", 2.5)];
        assert_eq!(
            check_strain_subtotal(&items, &map, 250.0, 0.01, now_utc()),
            SubtotalCheck::Ok
        );
    }

    #[test]
    fn subtotal_check_strain_only_mismatch_flags_fraud() {
        let s = strain("s1", 350.0);
        let mut map = HashMap::new();
        map.insert(s.id.as_str(), &s);
        // Client lies: 1 baht for a strain worth 350.
        let items = vec![strain_item("s1", 1.0)];
        match check_strain_subtotal(&items, &map, 1.0, 0.01, now_utc()) {
            SubtotalCheck::StrainOnlyMismatch { claimed, expected } => {
                assert!((claimed - 1.0).abs() < 1e-9);
                assert!((expected - 350.0).abs() < 1e-9);
            }
            other => panic!("expected StrainOnlyMismatch, got {:?}", other),
        }
    }

    #[test]
    fn subtotal_check_unknown_strain_short_circuits() {
        let map: HashMap<&str, &Strain> = HashMap::new();
        let items = vec![strain_item("missing-id", 1.0)];
        assert_eq!(
            check_strain_subtotal(&items, &map, 999.0, 0.01, now_utc()),
            SubtotalCheck::UnknownStrain("missing-id".into())
        );
    }

    #[test]
    fn subtotal_check_mixed_within_subtotal_ok() {
        // Strain portion: 100. Accessory adds 50 (we trust the client for
        // non-strain in this cycle). Claimed subtotal: 150 is fine.
        let s = strain("s1", 100.0);
        let mut map = HashMap::new();
        map.insert(s.id.as_str(), &s);
        let items = vec![strain_item("s1", 1.0), accessory_item()];
        assert_eq!(
            check_strain_subtotal(&items, &map, 150.0, 0.01, now_utc()),
            SubtotalCheck::Ok
        );
    }

    #[test]
    fn subtotal_check_mixed_strain_exceeds_claimed_flags() {
        // Strain alone is 200 but client claimed total 100 — impossible.
        let s = strain("s1", 100.0);
        let mut map = HashMap::new();
        map.insert(s.id.as_str(), &s);
        let items = vec![strain_item("s1", 2.0), accessory_item()];
        match check_strain_subtotal(&items, &map, 100.0, 0.01, now_utc()) {
            SubtotalCheck::StrainExceedsSubtotal { strain, claimed } => {
                assert!((strain - 200.0).abs() < 1e-9);
                assert!((claimed - 100.0).abs() < 1e-9);
            }
            other => panic!("expected StrainExceedsSubtotal, got {:?}", other),
        }
    }

    #[test]
    fn subtotal_check_honors_sale_discount_from_pricing_module() {
        // Sanity that the helper actually uses `trios::pricing` precedence:
        // sale_active + discount 50% on a 200-baht strain should produce
        // an expected 100-baht subtotal at qty 1.
        let mut s = strain("s1", 200.0);
        s.sale_active = true;
        s.discount_percent = 50.0;
        let mut map = HashMap::new();
        map.insert(s.id.as_str(), &s);
        let items = vec![strain_item("s1", 1.0)];
        assert_eq!(
            check_strain_subtotal(&items, &map, 100.0, 0.01, now_utc()),
            SubtotalCheck::Ok
        );
    }

    #[test]
    fn subtotal_check_expired_sale_falls_back_to_base() {
        // Client tries to claim sale_price after sale_until expired — server
        // must charge base price.
        let in_past = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let mut s = strain("s1", 200.0);
        s.sale_active = true;
        s.sale_until = Some(in_past);
        s.discount_percent = 50.0;
        let mut map = HashMap::new();
        map.insert(s.id.as_str(), &s);
        let items = vec![strain_item("s1", 1.0)];
        // Client thinks they got the discount: subtotal=100
        match check_strain_subtotal(&items, &map, 100.0, 0.01, now_utc()) {
            SubtotalCheck::StrainOnlyMismatch { claimed, expected } => {
                assert!((claimed - 100.0).abs() < 1e-9);
                assert!((expected - 200.0).abs() < 1e-9);
            }
            other => panic!(
                "expected StrainOnlyMismatch from expired sale, got {:?}",
                other
            ),
        }
    }

    // ── check_full_subtotal — every catalog (cycle #58 / C) ──────────

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
        }
    }

    #[test]
    fn full_check_sums_all_four_catalogs() {
        let s = strain("s1", 100.0);
        let mut cat = PriceCatalog::default();
        cat.strains.insert(s.id.as_str(), &s);
        cat.accessories.insert("a1", (250.0, true));
        cat.tea_products.insert("t1", (80.0, true));
        cat.sets.insert("set1", (1000.0, 10.0, true)); // 900
        let items = vec![
            strain_item("s1", 2.0),         // 200
            accessory_item_with("a1", 1.0), // 250
            tea_item_with("t1", 3.0),       // 240
            set_item_with("set1", 1.0),     // 900
        ];
        // 200 + 250 + 240 + 900 = 1590
        assert_eq!(
            check_full_subtotal(&items, &cat, 1590.0, 0.01, now_utc()),
            FullSubtotalCheck::Ok
        );
    }

    // ── B4: garden product-scoped discount money path ──────────────
    #[test]
    fn item_unit_price_per_catalog() {
        let s = strain("s1", 100.0);
        let mut cat = PriceCatalog::default();
        cat.strains.insert(s.id.as_str(), &s);
        cat.accessories.insert("a1", (250.0, true));
        cat.tea_products.insert("t1", (80.0, true));
        cat.sets.insert("set1", (1000.0, 10.0, true)); // 900 effective
        assert_eq!(
            item_unit_price(&strain_item("s1", 2.0), &cat, now_utc()),
            Some(100.0)
        );
        assert_eq!(
            item_unit_price(&accessory_item_with("a1", 1.0), &cat, now_utc()),
            Some(250.0)
        );
        assert_eq!(
            item_unit_price(&tea_item_with("t1", 1.0), &cat, now_utc()),
            Some(80.0)
        );
        assert_eq!(
            item_unit_price(&set_item_with("set1", 1.0), &cat, now_utc()),
            Some(900.0)
        );
        // Unknown + unavailable → None (so the discount path rejects).
        assert_eq!(
            item_unit_price(&accessory_item_with("nope", 1.0), &cat, now_utc()),
            None
        );
        cat.accessories.insert("a2", (250.0, false));
        assert_eq!(
            item_unit_price(&accessory_item_with("a2", 1.0), &cat, now_utc()),
            None
        );
    }

    #[test]
    fn item_matches_id_checks_all_id_fields() {
        assert!(item_matches_id(&strain_item("s1", 1.0), "s1"));
        assert!(item_matches_id(&accessory_item_with("a1", 1.0), "a1"));
        assert!(item_matches_id(&tea_item_with("t1", 1.0), "t1"));
        assert!(item_matches_id(&set_item_with("set1", 1.0), "set1"));
        assert!(!item_matches_id(&strain_item("s1", 1.0), "other"));
    }

    #[test]
    fn validate_order_garden_reward_relaxes_total_downward_only() {
        let mut req = valid_req();
        req.subtotal = 200.0;
        req.bonus_used = Some(0.0);
        // With a reward, a LOWER total (the discount) is allowed; the exact
        // amount is verified later in create_order against DB prices.
        req.garden_reward_id = Some("rw1".into());
        req.total = 150.0;
        assert!(validate_create_order(&req).is_ok());
        // But the total can never EXCEED subtotal - bonus (a reward only lowers).
        req.total = 250.0;
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
        // Without a reward, the total must be exact.
        req.garden_reward_id = None;
        req.total = 150.0;
        assert_eq!(
            validate_create_order(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
        req.total = 200.0;
        assert!(validate_create_order(&req).is_ok());
    }

    // The end-to-end discount arithmetic: discount = unit × qty × pct/100, and
    // expected_total = subtotal - bonus - discount.
    #[test]
    fn garden_discount_arithmetic() {
        let s = strain("s1", 100.0);
        let mut cat = PriceCatalog::default();
        cat.strains.insert(s.id.as_str(), &s);
        let target = strain_item("s1", 2.0); // 2g × 100 = 200 line total
        let unit = item_unit_price(&target, &cat, now_utc()).unwrap();
        let pct = 20u32;
        let discount = (unit * 2.0 * (pct as f64) / 100.0).max(0.0);
        assert_eq!(discount, 40.0); // 20% of 200
        let subtotal = 200.0;
        let bonus = 0.0;
        let expected_total = (subtotal - bonus - discount).max(0.0);
        assert_eq!(expected_total, 160.0);
    }

    #[test]
    fn full_check_flags_unavailable_accessory() {
        let mut cat = PriceCatalog::default();
        cat.accessories.insert("a1", (250.0, false));
        let items = vec![accessory_item_with("a1", 1.0)];
        match check_full_subtotal(&items, &cat, 250.0, 0.01, now_utc()) {
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
        match check_full_subtotal(&items, &cat, 999.0, 0.01, now_utc()) {
            FullSubtotalCheck::UnknownItem { catalog: c, id } => {
                assert_eq!(c, "sets");
                assert_eq!(id, "missing");
            }
            other => panic!("expected UnknownItem(sets), got {:?}", other),
        }
    }

    #[test]
    fn full_check_strain_precedence_wins_over_accessory_id() {
        // Defensive: if a malicious client sets BOTH `strain_id` and
        // `accessory_id` on the same item to shadow an expensive strain
        // with a cheap accessory, the strain path must win.
        let s = strain("s1", 1000.0);
        let mut cat = PriceCatalog::default();
        cat.strains.insert(s.id.as_str(), &s);
        cat.accessories.insert("a1", (1.0, true));
        let mut item = strain_item("s1", 1.0);
        item.accessory_id = Some("a1".into());
        assert_eq!(
            check_full_subtotal(&[item], &cat, 1000.0, 0.01, now_utc()),
            FullSubtotalCheck::Ok
        );
    }

    #[test]
    fn full_check_mismatch_reports_expected_and_claimed() {
        let mut cat = PriceCatalog::default();
        cat.accessories.insert("a1", (250.0, true));
        let items = vec![accessory_item_with("a1", 1.0)];
        // Client claims 1 baht
        match check_full_subtotal(&items, &cat, 1.0, 0.01, now_utc()) {
            FullSubtotalCheck::Mismatch { claimed, expected } => {
                assert!((claimed - 1.0).abs() < 1e-9);
                assert!((expected - 250.0).abs() < 1e-9);
            }
            other => panic!("expected Mismatch, got {:?}", other),
        }
    }

    #[test]
    fn full_check_malformed_when_no_ids() {
        let cat = PriceCatalog::default();
        let mut item = strain_item("s1", 1.0);
        item.strain_id = None; // every *_id is None now
        match check_full_subtotal(&[item], &cat, 0.0, 0.01, now_utc()) {
            FullSubtotalCheck::Malformed => {}
            other => panic!("expected Malformed, got {:?}", other),
        }
    }

    fn valid_req() -> CreateOrderRequest {
        CreateOrderRequest {
            telegram_id: Some(1),
            customer_name: Some("Alice".into()),
            customer_phone: Some("+123".into()),
            customer_telegram: Some("alice".into()),
            items: vec![OrderItem {
                strain_id: Some("s1".into()),
                strain_name: Some("Indica".into()),
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
            }],
            subtotal: 100.0,
            bonus_used: Some(10.0),
            stars_used: None,
            total: 90.0,
            shop_id: None,
            garden_reward_id: None,
            delivery_address: Some("123 Test Lane".into()),
            delivery_notes: None,
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
            })
            .collect();
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
            customer_phone: None,
            customer_telegram: None,
            items: vec![strain_item("s1", 1.0)],
            subtotal: 100.0,
            bonus_used: Some(0.0),
            stars_used: None,
            total: 100.0,
            shop_id: None,
            garden_reward_id: None,
            delivery_address: None,
            delivery_notes: None,
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
}
