use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::error;

use crate::api::auth::{check_admin, check_not_blocked, validate_telegram_id_param};
use crate::db::orders::{Order, OrderItem};
use crate::db::strains::Strain;
use crate::trios::pricing::{
    effective_accessory_price, effective_set_price, effective_strain_price, effective_tea_price,
    MarketingFlags,
};
use crate::AppState;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct CreateOrderRequest {
    pub telegram_id: Option<i64>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub customer_telegram: Option<String>,
    pub items: Vec<OrderItem>,
    pub subtotal: f64,
    pub bonus_used: Option<f64>,
    pub total: f64,
    pub shop_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrderStatusRequest {
    pub status: String,
    #[allow(dead_code)]
    pub admin_telegram_id: Option<i64>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/orders", post(create_order))
        .route("/orders", get(get_orders))
        .route("/orders/:id", get(get_order))
        .route("/orders/:id/status", put(update_order_status))
        .route("/orders/user/:telegram_id", get(get_user_orders))
}

/// Validates a CreateOrderRequest. Returns the sanitized bonus_used on success.
fn validate_create_order(req: &CreateOrderRequest) -> Result<f64, StatusCode> {
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
    if req
        .items
        .iter()
        .any(|i| !i.quantity.is_finite() || i.quantity <= 0.0)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !req.total.is_finite() || req.total < 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !req.subtotal.is_finite() || req.subtotal < 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let bonus_used = req.bonus_used.unwrap_or(0.0).max(0.0);
    if !bonus_used.is_finite() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if bonus_used > req.subtotal + 0.01 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let expected_total = (req.subtotal - bonus_used).max(0.0);
    if (req.total - expected_total).abs() > 0.01 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(bonus_used)
}

/// True if `k` is a syntactically acceptable `X-Idempotency-Key` value.
/// Accepts 1-100 ASCII alphanumeric / `-` / `_` characters — fits UUIDs
/// (`xxxxxxxx-xxxx-...`), nanoids, and short random strings. Rejects spaces,
/// control bytes, slashes, quotes, and anything that could smuggle SQL or
/// header-injection. Defence is shallow but cheap, and a malformed key is
/// almost always a buggy client rather than a legitimate one.
pub fn is_valid_idempotency_key(k: &str) -> bool {
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
pub enum SubtotalCheck {
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
pub fn check_strain_subtotal(
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
pub struct PriceCatalog<'a> {
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
pub enum FullSubtotalCheck {
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
pub fn check_full_subtotal(
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

    // If telegram_id is provided, verify ownership and blocked status.
    if let Some(tid) = req.telegram_id {
        crate::api::auth::check_owner(&headers, &state, tid)?;
        check_not_blocked(&state, tid).await?;
    }

    let bonus_used = validate_create_order(&req)?;

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
        let lookup_client = state.db.pool.get().await.map_err(|e| {
            error!("price-auth pool error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let mut catalog: PriceCatalog<'_> = PriceCatalog::default();
        // Strains — full SELECT to pick up all marketing flags (cycle #56).
        let strains: Vec<Strain> =
            if !strain_ids.is_empty() {
                let rows = lookup_client.query(
                "SELECT id, name, category, thc_percent::float8, cbd_percent::float8, effect, \
                    flavor_profile, description, price_per_gram::float8, available_grams::float8, \
                    image_url, video_url, is_available, is_strain_of_day, \
                    strain_of_day_discount::float8, name_en, description_en, effect_en, \
                    flavor_profile_en, strain_type_en, discount_percent::float8, \
                    sale_price::float8, sale_active, sale_until, is_best_seller, \
                    is_new_arrival, new_until, display_order \
                 FROM strains WHERE id = ANY($1)",
                &[&strain_ids],
            ).await.map_err(|e| {
                error!("price-auth strain lookup: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
                rows.iter().map(Strain::from_row).collect()
            } else {
                Vec::new()
            };
        for s in &strains {
            catalog.strains.insert(s.id.as_str(), s);
        }
        // Accessories — flat (price, is_available).
        let acc_rows: Vec<(String, f64, bool)> = if !accessory_ids.is_empty() {
            let rows = lookup_client.query(
                "SELECT id, price::float8 AS price, is_available FROM accessories WHERE id = ANY($1)",
                &[&accessory_ids],
            ).await.map_err(|e| {
                error!("price-auth accessory lookup: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            rows.iter()
                .map(|r| {
                    (
                        r.try_get::<_, String>("id").unwrap_or_default(),
                        r.try_get::<_, f64>("price").unwrap_or(0.0),
                        r.try_get::<_, bool>("is_available").unwrap_or(false),
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        for (id, p, a) in &acc_rows {
            catalog.accessories.insert(id.as_str(), (*p, *a));
        }
        // Tea products — same schema as accessories.
        let tea_rows: Vec<(String, f64, bool)> = if !tea_ids.is_empty() {
            let rows = lookup_client.query(
                "SELECT id, price::float8 AS price, is_available FROM tea_products WHERE id = ANY($1)",
                &[&tea_ids],
            ).await.map_err(|e| {
                error!("price-auth tea lookup: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            rows.iter()
                .map(|r| {
                    (
                        r.try_get::<_, String>("id").unwrap_or_default(),
                        r.try_get::<_, f64>("price").unwrap_or(0.0),
                        r.try_get::<_, bool>("is_available").unwrap_or(false),
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        for (id, p, a) in &tea_rows {
            catalog.tea_products.insert(id.as_str(), (*p, *a));
        }
        // Sets — UNION across three tables (`sets`, `accessory_sets`,
        // `tea_sets`). Same `(total_price, discount_percent)` shape; UUID
        // PKs across the three don't collide.
        let set_rows: Vec<(String, f64, f64, bool)> = if !set_ids.is_empty() {
            let rows = lookup_client.query(
                "SELECT id, total_price::float8 AS tp, discount_percent::float8 AS dp, is_available \
                 FROM sets WHERE id = ANY($1) \
                 UNION ALL \
                 SELECT id, total_price::float8 AS tp, discount_percent::float8 AS dp, is_available \
                 FROM accessory_sets WHERE id = ANY($1) \
                 UNION ALL \
                 SELECT id, total_price::float8 AS tp, discount_percent::float8 AS dp, is_available \
                 FROM tea_sets WHERE id = ANY($1)",
                &[&set_ids],
            ).await.map_err(|e| {
                error!("price-auth sets lookup: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            rows.iter()
                .map(|r| {
                    (
                        r.try_get::<_, String>("id").unwrap_or_default(),
                        r.try_get::<_, f64>("tp").unwrap_or(0.0),
                        r.try_get::<_, f64>("dp").unwrap_or(0.0),
                        r.try_get::<_, bool>("is_available").unwrap_or(false),
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        for (id, tp, dp, a) in &set_rows {
            catalog.sets.insert(id.as_str(), (*tp, *dp, *a));
        }

        match check_full_subtotal(&req.items, &catalog, req.subtotal, 0.01, chrono::Utc::now()) {
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
                    &state.db.pool,
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
                    &state.db.pool,
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
                    &state.db.pool,
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
                    &state.db.pool,
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

    let id = uuid::Uuid::new_v4().to_string();
    let items_json = serde_json::to_value(&req.items).map_err(|e| {
        error!("items serialization failed: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    // Atomic transaction: rate-limit check, bonus deduction, and insert order together.
    let mut client = state.db.pool.get().await.map_err(|e| {
        error!("create_order pool error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let tx = client.transaction().await.map_err(|e| {
        error!("create_order tx error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Idempotency check (must run BEFORE the 1-min rate limit). Two parallel
    // POSTs with the same key serialise on the per-key advisory lock; the
    // loser then sees the existing row and replays the cached order_id
    // instead of being told "you're rate-limited".
    if let Some(ref k) = idem_key {
        tx.execute("SELECT pg_advisory_xact_lock(hashtext($1)::bigint)", &[k])
            .await
            .map_err(|e| {
                error!("idempotency lock: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        let row = tx
            .query_opt(
                "SELECT order_id FROM order_idempotency_keys WHERE key = $1",
                &[k],
            )
            .await
            .map_err(|e| {
                error!("idempotency SELECT: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        if let Some(row) = row {
            let existing_id: String = row.get(0);
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
        tx.execute("SELECT pg_advisory_xact_lock($1)", &[&tid])
            .await
            .map_err(|e| {
                error!("advisory lock error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }

    // Rate-limit inside tx to close the race window.
    if let Some(tid) = req.telegram_id {
        let recent = tx.query_opt(
            "SELECT 1 FROM orders WHERE telegram_id = $1 AND created_at > NOW() - INTERVAL '1 minute' LIMIT 1",
            &[&tid],
        ).await.map_err(|e| { error!("rate-limit check error: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
        if recent.is_some() {
            if let Err(e) = tx.rollback().await {
                tracing::error!("create_order rollback error: {}", e);
            }
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    // Atomic bonus deduction: UPDATE with built-in balance guard.
    if bonus_used > 0.0 {
        if let Some(tid) = req.telegram_id {
            let deducted = tx.execute(
                "UPDATE loyalty_profiles SET bonus_balance = GREATEST(0, bonus_balance - $1) WHERE telegram_id = $2 AND bonus_balance >= $1",
                &[&bonus_used, &tid],
            ).await.map_err(|e| { error!("bonus deduction error: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
            if deducted == 0 {
                if let Err(e) = tx.rollback().await {
                    tracing::error!("create_order rollback error: {}", e);
                }
                return Err(StatusCode::BAD_REQUEST);
            }
        } else {
            if let Err(e) = tx.rollback().await {
                tracing::error!("create_order rollback error: {}", e);
            }
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    tx.execute(
        "INSERT INTO orders (id, telegram_id, customer_name, customer_phone, customer_telegram, items, subtotal, bonus_used, total, status, shop_id) VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7::float8, $8::float8, $9::float8, 'pending', $10)",
        &[&id, &req.telegram_id, &req.customer_name, &req.customer_phone, &req.customer_telegram, &items_json, &req.subtotal, &bonus_used, &req.total, &req.shop_id],
    ).await.map_err(|e| { error!("create_order insert error: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    // Record the idempotency key inside the same tx so retries after this
    // commit see the cached order_id. The earlier advisory lock guarantees
    // no other tx can hold a different (key, order_id) for this `k`.
    if let Some(ref k) = idem_key {
        tx.execute(
            "INSERT INTO order_idempotency_keys (key, order_id, telegram_id) VALUES ($1, $2, $3)",
            &[k, &id, &req.telegram_id],
        )
        .await
        .map_err(|e| {
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
                    format!("  • {} × {}g", html_escape(name), qty)
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

    let text = format!(
        "🚨 <b>New Order!</b>\n━━━━━━━━━━━━━━━━\n👤 {}\n📦 Items:\n{}\n━━━━━━━━━━━━━━━━\n💰 Subtotal: {} ฿\n🎁 Bonus: -{} ฿\n💳 Total: {} ฿\n🔖 #{}",
        source, items_text, subtotal, bonus_used, total, html_escape(&order_id[order_id.len().saturating_sub(6)..])
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
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("DB error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let rows = client.query(
        "SELECT id, telegram_id, customer_name, customer_phone, customer_telegram, items, subtotal::float8, bonus_used::float8, total::float8, status, shop_id, created_at FROM orders ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        &[&limit, &offset],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let orders: Vec<Order> = rows.iter().map(Order::from_row).collect();
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
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("DB error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let row = client.query_opt(
        "SELECT id, telegram_id, customer_name, customer_phone, customer_telegram, items, subtotal::float8, bonus_used::float8, total::float8, status, shop_id, created_at FROM orders WHERE id = $1",
        &[&id],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    match row {
        Some(r) => Ok(Json(json!({ "order": Order::from_row(&r) }))),
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
        "completed",
        "rejected",
        "ready",
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
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("DB error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let current_status: Option<String> = client
        .query_opt("SELECT status FROM orders WHERE id = $1", &[&id])
        .await
        .map_err(|e| {
            tracing::error!("DB error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map(|r| r.try_get("status").unwrap_or_default());
    match current_status {
        Some(ref current)
            if (current == "completed" || current == "rejected" || current == "cancelled")
                && req.status != *current =>
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        None => return Err(StatusCode::NOT_FOUND),
        _ => {}
    }

    if req.status == "completed" {
        if let Err(e) =
            crate::db::orders::complete_order_and_update_loyalty(&state.db.pool, &id).await
        {
            tracing::error!(
                "update_order_status: complete_order_and_update_loyalty error: {}",
                e
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    } else {
        let mut client = state.db.pool.get().await.map_err(|e| {
            tracing::error!("DB error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        if req.status == "rejected" {
            let tx = client.transaction().await.map_err(|e| {
                tracing::error!("DB tx error: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            if let Some(r) = tx.query_opt("SELECT telegram_id, bonus_used::float8, status FROM orders WHERE id = $1 FOR UPDATE", &[&id]).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })? {
                let current_status: String = r.try_get("status").unwrap_or_default();
                if current_status != "rejected" && current_status != "completed" {
                    let bonus_raw: f64 = r.try_get::<_, f64>("bonus_used").unwrap_or(0.0);
                    let bonus = if bonus_raw.is_finite() { bonus_raw.max(0.0) } else { 0.0 };
                    let tid: Option<i64> = r.try_get("telegram_id").ok().flatten();
                    if bonus > 0.0 {
                        if let Some(tid) = tid {
                            tx.execute(
                                "INSERT INTO loyalty_profiles (telegram_id, bonus_balance, total_spent) VALUES ($1, 0, 0) ON CONFLICT (telegram_id) DO NOTHING",
                                &[&tid],
                            ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
                            tx.execute(
                                "UPDATE loyalty_profiles SET bonus_balance = bonus_balance + $1 WHERE telegram_id = $2",
                                &[&bonus, &tid],
                            ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
                        }
                    }
                }
            }
            let rows = tx
                .execute(
                    "UPDATE orders SET status = $1 WHERE id = $2",
                    &[&req.status, &id],
                )
                .await
                .map_err(|e| {
                    tracing::error!("DB error: {:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            if rows == 0 {
                if let Err(e) = tx.rollback().await {
                    tracing::error!("update_order_status rollback error: {:?}", e);
                }
                return Err(StatusCode::NOT_FOUND);
            }
            tx.commit().await.map_err(|e| {
                tracing::error!("DB commit error: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        } else {
            let rows = client
                .execute(
                    "UPDATE orders SET status = $1 WHERE id = $2",
                    &[&req.status, &id],
                )
                .await
                .map_err(|e| {
                    tracing::error!("DB error: {:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            if rows == 0 {
                return Err(StatusCode::NOT_FOUND);
            }
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
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("DB error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let rows = client.query(
        "SELECT id, telegram_id, customer_name, customer_phone, customer_telegram, items, subtotal::float8, bonus_used::float8, total::float8, status, shop_id, created_at FROM orders WHERE telegram_id = $1 ORDER BY created_at DESC LIMIT 50",
        &[&telegram_id],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(
        json!({ "orders": rows.iter().map(Order::from_row).collect::<Vec<_>>() }),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        check_full_subtotal, check_strain_subtotal, is_valid_idempotency_key,
        validate_create_order, validate_update_order_status, CreateOrderRequest, FullSubtotalCheck,
        PriceCatalog, SubtotalCheck,
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
            is_set: None,
            is_accessory: None,
            is_tea: None,
            is_tea_set: None,
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
            is_set: None,
            is_accessory: Some(true),
            is_tea: None,
            is_tea_set: None,
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
            is_set: None,
            is_accessory: Some(true),
            is_tea: None,
            is_tea_set: None,
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
            is_set: None,
            is_accessory: None,
            is_tea: Some(true),
            is_tea_set: None,
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
            is_set: Some(true),
            is_accessory: None,
            is_tea: None,
            is_tea_set: None,
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
                is_set: None,
                is_accessory: None,
                is_tea: None,
                is_tea_set: None,
            }],
            subtotal: 100.0,
            bonus_used: Some(10.0),
            total: 90.0,
            shop_id: None,
        }
    }

    #[test]
    fn test_validate_ok() {
        let req = valid_req();
        assert_eq!(validate_create_order(&req).unwrap(), 10.0);
    }

    #[test]
    fn test_validate_no_bonus() {
        let mut req = valid_req();
        req.bonus_used = None;
        req.total = 100.0;
        assert_eq!(validate_create_order(&req).unwrap(), 0.0);
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
                is_set: None,
                is_accessory: None,
                is_tea: None,
                is_tea_set: None,
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
        assert_eq!(validate_create_order(&req).unwrap(), 10.0);
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
}
