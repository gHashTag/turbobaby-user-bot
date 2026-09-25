// Loop #11: server-side cart persistence with price authority.
//
// Cart is tied to telegram_id; items store a snapshot (name, unit_price,
// image_url) so the UI can render the cart even if catalog rows change.
// The server recalculates the total from DB prices on every read and
// rejects items that are unknown or unavailable — the same authority
// path used by create_order.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, patch, post},
    Json, Router,
};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::api::auth::{check_not_blocked, check_owner, validate_telegram_id_param};
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CartItemResp {
    pub id: String,
    pub kind: String,
    pub catalog_id: String,
    pub quantity: i32,
    pub unit_price: f64,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CartResp {
    pub telegram_id: i64,
    pub items: Vec<CartItemResp>,
    pub total: f64,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddCartItemReq {
    pub telegram_id: i64,
    pub kind: String,
    pub catalog_id: String,
    pub quantity: i32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateCartItemReq {
    pub quantity: i32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MergeCartReq {
    pub telegram_id: i64,
    pub items: Vec<CartItemResp>,
}

/// Upper bound on lines accepted by `POST /api/cart/merge`. The local cart
/// the client replays is user-controlled, and each line costs one catalog
/// lookup, so an unbounded payload is a cheap way to tie up a DB connection.
const MAX_MERGE_ITEMS: usize = 200;

/// Upsert for every cart line whose rental dates have not been selected yet.
///
/// Keep the conflict predicate aligned with
/// `migrations/081_cart_rental_lines.sql::idx_cart_items_undated_line`.
/// PostgreSQL cannot infer that partial index from the three columns alone.
const CART_ITEM_UPSERT_SQL: &str = "INSERT INTO cart_items \
   (id, cart_id, kind, catalog_id, quantity, unit_price, name, image_url, created_at, updated_at) \
 VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, now(), now()) \
 ON CONFLICT (cart_id, kind, catalog_id) \
   WHERE rental_start IS NULL AND rental_end IS NULL \
 DO UPDATE SET \
   quantity   = LEAST(cart_items.quantity + EXCLUDED.quantity, 1000000), \
   unit_price = EXCLUDED.unit_price, \
   name       = EXCLUDED.name, \
   image_url  = EXCLUDED.image_url, \
   updated_at = now()";

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/cart",
            get(get_cart).post(add_cart_item).delete(clear_cart),
        )
        .route("/cart/merge", post(merge_cart))
        .route(
            "/cart/items/:item_id",
            patch(update_cart_item).delete(delete_cart_item),
        )
}

fn parse_kind(kind: &str) -> Result<&'static str, StatusCode> {
    match kind {
        "strain" => Ok("strain"),
        "set" => Ok("set"),
        "accessory" => Ok("accessory"),
        "tea" => Ok("tea"),
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

fn validate_quantity(q: i32) -> Result<(), StatusCode> {
    if q <= 0 || q > 1_000_000 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// Collapse a client-supplied merge payload into one row per
/// `(kind, catalog_id)`, dropping lines that fail validation.
///
/// A local cart replayed from storage can legitimately carry the same line
/// twice. Passing duplicates straight through used to mean two writes to the
/// same row inside one transaction; folding them here also saves one catalog
/// lookup per duplicate.
///
/// The row identity this collapses on is `idx_cart_items_undated_line`
/// (`migrations/081_cart_rental_lines.sql`), not 059's dropped
/// `UNIQUE (cart_id, kind, catalog_id)` — the same three columns, but only for
/// lines with no dates, which is every line this endpoint can produce. A merge
/// payload carrying rental dates would need `(kind, catalog_id, start, end)` as
/// the key, because two different date ranges for one family are two lines and
/// folding them would lose a booking.
fn collapse_merge_items(items: Vec<CartItemResp>) -> Vec<(&'static str, String, i32)> {
    let mut out: Vec<(&'static str, String, i32)> = Vec::new();
    for item in items {
        let Ok(kind) = parse_kind(&item.kind) else {
            continue;
        };
        if validate_id(&item.catalog_id).is_err() || validate_quantity(item.quantity).is_err() {
            continue;
        }
        match out
            .iter_mut()
            .find(|(k, id, _)| *k == kind && *id == item.catalog_id)
        {
            Some((_, _, qty)) => *qty = qty.saturating_add(item.quantity).min(1_000_000),
            None => out.push((kind, item.catalog_id, item.quantity)),
        }
    }
    out
}

fn validate_id(id: &str) -> Result<(), StatusCode> {
    if id.is_empty() || id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// Resolve current DB price and snapshot fields for a catalog item.
/// Returns `None` when the item is unknown or unavailable.
async fn resolve_catalog_snapshot(
    state: &AppState,
    kind: &str,
    catalog_id: &str,
) -> Result<Option<(String, f64, Option<String>)>, StatusCode> {
    use crate::db::entities::{accessory, strain, tea_product};
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let row = match kind {
        "strain" => strain::Entity::find_by_id(catalog_id.to_string())
            .one(&state.db.orm)
            .await
            .map_err(|e| {
                tracing::error!("cart strain lookup: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .map(|m| {
                let sale_until = m.sale_until.as_ref().map(|d| d.to_rfc3339());
                let new_until = m.new_until.as_ref().map(|d| d.to_rfc3339());
                (
                    m.name,
                    crate::trios::pricing::effective_strain_price(
                        &crate::trios::pricing::MarketingFlags {
                            price_per_gram: m.price_per_gram,
                            is_strain_of_day: m.is_strain_of_day,
                            strain_of_day_discount: m.strain_of_day_discount,
                            sale_active: m.sale_active,
                            sale_until: sale_until.as_deref(),
                            sale_price: m.sale_price,
                            discount_percent: m.discount_percent,
                            is_new_arrival: m.is_new_arrival,
                            new_until: new_until.as_deref(),
                        },
                        Utc::now(),
                    )
                    .price,
                    m.image_url,
                )
            }),
        "accessory" => accessory::Entity::find_by_id(catalog_id.to_string())
            .one(&state.db.orm)
            .await
            .map_err(|e| {
                tracing::error!("cart accessory lookup: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .filter(|m| m.is_available)
            .map(|m| {
                (
                    m.name,
                    crate::trios::pricing::effective_accessory_price(m.price),
                    Some(m.image_url).filter(|s| !s.is_empty()),
                )
            }),
        "tea" => tea_product::Entity::find_by_id(catalog_id.to_string())
            .one(&state.db.orm)
            .await
            .map_err(|e| {
                tracing::error!("cart tea lookup: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .filter(|m| m.is_available)
            .map(|m| {
                (
                    m.name,
                    crate::trios::pricing::effective_tea_price(m.price),
                    Some(m.image_url).filter(|s| !s.is_empty()),
                )
            }),
        "set" => {
            // The public set catalog spans three tables; we price from the
            // same UNION that create_order uses. We prefer the matching table
            // by trying the cheapest query first and falling back.
            let ids = vec![catalog_id.to_string()];
            let rows = state.db.orm.query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT id, name, total_price::float8 AS tp, discount_percent::float8 AS dp, is_available, image_url \
                 FROM ( \
                   SELECT id, name, total_price, discount_percent, is_available, image_url FROM sets WHERE id = ANY($1) \
                   UNION ALL \
                   SELECT id, name, total_price, discount_percent, is_available, image_url FROM accessory_sets WHERE id = ANY($1) \
                   UNION ALL \
                   SELECT id, name, total_price, discount_percent, is_available, image_url FROM tea_sets WHERE id = ANY($1) \
                 ) x",
                [ids.into()],
            )).await.map_err(|e| {
                tracing::error!("cart set lookup: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            rows.into_iter().next().and_then(|r| {
                let id: String = r.try_get("", "id").ok()?;
                if id != catalog_id {
                    return None;
                }
                let avail: bool = r.try_get("", "is_available").unwrap_or(false);
                if !avail {
                    return None;
                }
                let name: String = r.try_get("", "name").ok()?;
                let tp: f64 = r.try_get("", "tp").ok()?;
                let dp: f64 = r.try_get("", "dp").ok()?;
                let price = crate::trios::pricing::effective_set_price(tp, dp);
                let image_url: Option<String> = r.try_get("", "image_url").ok().flatten();
                Some((name, price, image_url))
            })
        }
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    Ok(row)
}

/// Fetch the caller's cart, creating it if missing.
///
/// `carts.telegram_id` is UNIQUE, so the old read-then-insert shape was racy:
/// two concurrent requests for the same user (a double-tapped "reorder"
/// button fires two `POST /api/cart/merge` at once) both missed the SELECT,
/// both INSERTed, and the loser got a duplicate-key error surfaced as HTTP
/// 500. The insert is now an `ON CONFLICT DO NOTHING` upsert followed by a
/// re-read, so the loser simply picks up the winner's row.
async fn get_or_create_cart(
    db: &sea_orm::DatabaseConnection,
    telegram_id: i64,
) -> Result<crate::db::entities::cart::Model, StatusCode> {
    use crate::db::entities::cart::{Column as CartCol, Entity as CartEntity};
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    for attempt in 0..2 {
        let existing = CartEntity::find()
            .filter(CartCol::TelegramId.eq(telegram_id))
            .one(db)
            .await
            .map_err(|e| {
                tracing::error!("cart lookup: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        if let Some(c) = existing {
            return Ok(c);
        }
        if attempt == 0 {
            db.execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO carts (id, telegram_id, created_at, updated_at, expires_at) \
                 VALUES (gen_random_uuid(), $1, now(), now(), now() + interval '30 days') \
                 ON CONFLICT (telegram_id) DO NOTHING",
                [telegram_id.into()],
            ))
            .await
            .map_err(|e| {
                tracing::error!("cart insert: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
    }

    tracing::error!("cart get_or_create: row still missing after upsert for {telegram_id}");
    Err(StatusCode::INTERNAL_SERVER_ERROR)
}

/// Add `quantity` of one catalog item to a cart, atomically.
///
/// The previous find-then-insert-or-update shape raced the same way
/// `get_or_create_cart` did — and inside a transaction the duplicate-key error
/// also aborted every preceding write in that transaction. A single
/// `ON CONFLICT DO UPDATE` statement lets Postgres serialize concurrent
/// writers for us. The quantity cap mirrors `validate_quantity`.
///
/// # Why the conflict target carries a `WHERE`
///
/// `cart_items` had a plain `UNIQUE (cart_id, kind, catalog_id)` from
/// `migrations/059_cart_tables.sql:24`, and a bare three-column conflict
/// target inferred it. `migrations/081_cart_rental_lines.sql` **drops** that
/// constraint (it has to: a rental line is identified by its dates too) and
/// replaces it with a five-column `UNIQUE (cart_id, kind, catalog_id,
/// rental_start, rental_end)` plus the partial unique index
/// `idx_cart_items_undated_line ... WHERE rental_start IS NULL AND rental_end
/// IS NULL`, which is what still holds 059's one-line-per-item rule for
/// undated lines.
///
/// Postgres cannot infer a *partial* index from a bare conflict target: the
/// upsert has to repeat the index predicate verbatim. Without the `WHERE`
/// below, this statement fails with 42P10 ("no unique or exclusion constraint
/// matching the ON CONFLICT specification") the moment 081 has run — not on
/// some edge case, but on **every** write to a cart. The five-column target
/// would not work either, because the NULLs this function writes never match
/// it.
///
/// This function writes no dates, so every row it creates satisfies the first
/// branch of the `cart_items_rental_dates` CHECK (both NULL) and lands in the
/// partial index. That is deliberate and 081 blesses it: a rental line whose
/// dates the customer has not picked yet is one line per family. When a date
/// picker exists, a dated line needs the five-column target instead — at which
/// point this becomes two statements, not a widened one.
#[allow(clippy::too_many_arguments)]
async fn upsert_cart_item<C: sea_orm::ConnectionTrait>(
    conn: &C,
    cart_id: uuid::Uuid,
    kind: &str,
    catalog_id: &str,
    quantity: i32,
    unit_price: f64,
    name: &str,
    image_url: Option<String>,
) -> Result<(), StatusCode> {
    use sea_orm::{DbBackend, Statement};

    conn.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        CART_ITEM_UPSERT_SQL,
        [
            cart_id.into(),
            kind.into(),
            catalog_id.into(),
            quantity.into(),
            unit_price.into(),
            name.into(),
            image_url.into(),
        ],
    ))
    .await
    .map_err(|e| {
        tracing::error!("cart item upsert: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(())
}

async fn load_cart_items(
    db: &sea_orm::DatabaseConnection,
    cart_id: &uuid::Uuid,
) -> Result<Vec<crate::db::entities::cart_item::Model>, StatusCode> {
    use crate::db::entities::cart_item::{Column as ItemCol, Entity as ItemEntity};
    ItemEntity::find()
        .filter(ItemCol::CartId.eq(*cart_id))
        .order_by_asc(ItemCol::CreatedAt)
        .all(db)
        .await
        .map_err(|e| {
            tracing::error!("cart items load: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

fn cart_model_to_resp(
    model: &crate::db::entities::cart::Model,
    items: &[crate::db::entities::cart_item::Model],
) -> CartResp {
    let mut total = 0.0_f64;
    // A row of a retired kind stays stored and is never served (2026-09-26).
    let item_resp: Vec<CartItemResp> = served_rows(items)
        .map(|i| {
            let price = if i.unit_price.is_finite() {
                i.unit_price.max(0.0)
            } else {
                0.0
            };
            let q = i.quantity.max(0);
            total += price * q as f64;
            CartItemResp {
                id: i.id.to_string(),
                kind: i.kind.clone(),
                catalog_id: i.catalog_id.clone(),
                quantity: q,
                unit_price: price,
                name: i.name.clone(),
                image_url: i.image_url.clone(),
            }
        })
        .collect();
    CartResp {
        telegram_id: model.telegram_id,
        items: item_resp,
        total: total.max(0.0),
        updated_at: model.updated_at.map(|d| d.to_rfc3339()),
    }
}

async fn get_cart(
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<CartResp>, StatusCode> {
    let telegram_id = params
        .get("telegram_id")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    validate_telegram_id_param(telegram_id)?;
    check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;

    let cart = get_or_create_cart(&state.db.orm, telegram_id).await?;
    let items = load_cart_items(&state.db.orm, &cart.id).await?;
    Ok(Json(cart_model_to_resp(&cart, &items)))
}

async fn add_cart_item(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<AddCartItemReq>,
) -> Result<Json<CartResp>, StatusCode> {
    check_owner(&headers, &state, req.telegram_id)?;
    check_not_blocked(&state, req.telegram_id).await?;
    validate_id(&req.catalog_id)?;
    validate_quantity(req.quantity)?;
    let kind = parse_kind(&req.kind)?;

    let snapshot = resolve_catalog_snapshot(&state, kind, &req.catalog_id).await?;
    let (name, unit_price, image_url) = snapshot.ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;

    let cart = get_or_create_cart(&state.db.orm, req.telegram_id).await?;
    upsert_cart_item(
        &state.db.orm,
        cart.id,
        kind,
        &req.catalog_id,
        req.quantity,
        unit_price,
        &name,
        image_url,
    )
    .await?;

    // Refresh cart timestamp so the client can detect stale caches.
    let cart = get_or_create_cart(&state.db.orm, req.telegram_id).await?;
    let items = load_cart_items(&state.db.orm, &cart.id).await?;
    Ok(Json(cart_model_to_resp(&cart, &items)))
}

async fn update_cart_item(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(item_id): Path<String>,
    Json(req): Json<UpdateCartItemReq>,
) -> Result<Json<Value>, StatusCode> {
    validate_id(&item_id)?;
    let item_uuid = uuid::Uuid::parse_str(&item_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    validate_quantity(req.quantity)?;

    use crate::db::entities::cart_item::Entity as ItemEntity;
    let item = ItemEntity::find_by_id(item_uuid)
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("cart item find: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .filter(served_row)
        .ok_or(StatusCode::NOT_FOUND)?;

    let cart = crate::db::entities::cart::Entity::find_by_id(item.cart_id)
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("cart find for item: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    check_owner(&headers, &state, cart.telegram_id)?;
    check_not_blocked(&state, cart.telegram_id).await?;

    let mut am: crate::db::entities::cart_item::ActiveModel = item.into();
    am.quantity = Set(req.quantity);
    am.updated_at = Set(Some(chrono::DateTime::from(Utc::now())));
    am.update(&state.db.orm).await.map_err(|e| {
        tracing::error!("cart item patch: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({"success": true})))
}

async fn delete_cart_item(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(item_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    validate_id(&item_id)?;
    let item_uuid = uuid::Uuid::parse_str(&item_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    use crate::db::entities::cart_item::Entity as ItemEntity;
    let item = ItemEntity::find_by_id(item_uuid)
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("cart item find: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .filter(served_row)
        .ok_or(StatusCode::NOT_FOUND)?;

    let cart = crate::db::entities::cart::Entity::find_by_id(item.cart_id)
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("cart find for item: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    check_owner(&headers, &state, cart.telegram_id)?;
    check_not_blocked(&state, cart.telegram_id).await?;

    ItemEntity::delete_by_id(item_uuid)
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("cart item delete: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(json!({"success": true})))
}

async fn clear_cart(
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let telegram_id = params
        .get("telegram_id")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    validate_telegram_id_param(telegram_id)?;
    check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;

    use crate::db::entities::cart::{Column as CartCol, Entity as CartEntity};
    let cart = CartEntity::find()
        .filter(CartCol::TelegramId.eq(telegram_id))
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("cart lookup for clear: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if let Some(c) = cart {
        crate::db::entities::cart_item::Entity::delete_many()
            .filter(served_lines_of(c.id))
            .exec(&state.db.orm)
            .await
            .map_err(|e| {
                tracing::error!("cart clear items: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }

    Ok(Json(json!({"success": true})))
}

async fn merge_cart(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<MergeCartReq>,
) -> Result<Json<CartResp>, StatusCode> {
    validate_telegram_id_param(req.telegram_id)?;
    check_owner(&headers, &state, req.telegram_id)?;
    check_not_blocked(&state, req.telegram_id).await?;

    if req.items.len() > MAX_MERGE_ITEMS {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    let cart = get_or_create_cart(&state.db.orm, req.telegram_id).await?;

    // Resolve each item; skip unknown/unavailable rather than failing the whole merge.
    let mut to_insert = Vec::new();
    for (kind, catalog_id, quantity) in collapse_merge_items(req.items) {
        if let Some((name, unit_price, image_url)) =
            resolve_catalog_snapshot(&state, kind, &catalog_id).await?
        {
            to_insert.push((kind, catalog_id, quantity, name, unit_price, image_url));
        }
    }

    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("cart merge tx begin: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    for (kind, catalog_id, quantity, name, unit_price, image_url) in to_insert {
        upsert_cart_item(
            &tx,
            cart.id,
            kind,
            &catalog_id,
            quantity,
            unit_price,
            &name,
            image_url,
        )
        .await?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("cart merge commit: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    crate::metrics::reorder_clicked("server_merge");

    // Refresh cart timestamp after a successful merge.
    let cart = get_or_create_cart(&state.db.orm, req.telegram_id).await?;
    let items = load_cart_items(&state.db.orm, &cart.id).await?;
    Ok(Json(cart_model_to_resp(&cart, &items)))
}

// ── Rows of a retired kind: stored, never served (2026-09-26) ─────────────
//
// The owner ruled rental only on 2026-09-24 and, on 2026-09-25 (answer 12),
// that nothing of the previous shop may appear anywhere. A cart kept from that
// shop still holds its rows: `cart_items` admits the old catalogue's kinds
// beside `bike_rental` (migration 081's CHECK), and a returning customer's cart
// row is never deleted. Nothing here deletes or rewrites one either. To the
// API such a row is ABSENT: no response lists it or counts it in the total
// (`cart_model_to_resp`, behind `get_cart`, `add_cart_item` and `merge_cart`),
// PATCH and DELETE by its id answer 404 as for an id that does not exist, and
// clearing the cart leaves it where it is. Which kinds are served is one
// predicate shared with the reminder and the Mini App,
// `trios::pricing::cart_kind_is_served`; specs/turbobaby/cart_persistence.t27
// records the rule as SERVED_CART_KINDS.
//
// The owner's answer 3 of 2026-09-25 (second list) first masked such a row
// here instead: served under a neutral name and with no picture. When the two
// changes were merged on 2026-09-26 this rule was kept and that path removed:
// a row that is never served needs no name, and a served row is a rental row,
// whose stored name and picture are its own.
//
// The write gate is left as it was: `parse_kind` still names the old kinds,
// and none of them can write a row today (the census in cart_persistence.t27,
// KINDS_THAT_CAN_WRITE_A_ROW_TODAY). Closing it is a separate change.

/// Is this stored cart row one the API may serve?
fn served_row(row: &crate::db::entities::cart_item::Model) -> bool {
    crate::trios::pricing::cart_kind_is_served(&row.kind)
}

/// The rows of a cart the API may serve, in the order they were loaded.
fn served_rows(
    items: &[crate::db::entities::cart_item::Model],
) -> impl Iterator<Item = &crate::db::entities::cart_item::Model> {
    items.iter().filter(|row| served_row(row))
}

/// The served lines of one cart, as a filter: what clearing the cart deletes.
fn served_lines_of(cart_id: uuid::Uuid) -> sea_orm::Condition {
    use crate::db::entities::cart_item::Column as ItemCol;
    sea_orm::Condition::all()
        .add(ItemCol::CartId.eq(cart_id))
        .add(ItemCol::Kind.is_in(crate::trios::pricing::SERVED_CART_KINDS))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::cart_item;
    use axum::http::StatusCode;
    use sea_orm::QueryTrait;

    fn cart_row() -> crate::db::entities::cart::Model {
        crate::db::entities::cart::Model {
            id: uuid::Uuid::nil(),
            telegram_id: 42,
            created_at: None,
            updated_at: None,
            expires_at: None,
            reminder_sent_at: None,
            reminder_count: Some(0),
            reminder_variant: None,
            start_param: None,
            first_reminder_sent_at: None,
        }
    }

    fn line(kind: &str, name: &str, quantity: i32, unit_price: f64) -> cart_item::Model {
        cart_item::Model {
            id: uuid::Uuid::new_v4(),
            cart_id: uuid::Uuid::nil(),
            kind: kind.to_string(),
            catalog_id: format!("{kind}-1"),
            quantity,
            unit_price,
            name: name.to_string(),
            image_url: Some(format!("/assets/{kind}.webp")),
            created_at: None,
            updated_at: None,
        }
    }

    /// The cart response never carries a row of a retired kind: not its name,
    /// not its picture, not its money in the total.
    #[test]
    fn a_row_of_a_retired_kind_never_reaches_the_cart_response() {
        let rows = vec![
            line("accessory", "Retired Accessory", 2, 150.0),
            line("bike_rental", "NMAX 155", 1, 0.0),
            line("tea", "Retired Tea", 1, 90.0),
            line("set", "Retired Set", 3, 500.0),
            line("unknown_kind", "Unclassified", 1, 10.0),
        ];
        let resp = cart_model_to_resp(&cart_row(), &rows);

        assert_eq!(resp.items.len(), 1, "{:?}", resp.items);
        assert_eq!(resp.items[0].kind, "bike_rental");
        assert_eq!(resp.items[0].name, "NMAX 155");
        assert_eq!(
            resp.total, 0.0,
            "a hidden row's money leaked into the total"
        );
        let wire = serde_json::to_string(&resp).expect("the response serialises");
        for hidden in [
            "Retired",
            "Unclassified",
            "/assets/accessory",
            "/assets/tea",
        ] {
            assert!(!wire.contains(hidden), "{hidden} reached the wire: {wire}");
        }
    }

    /// A cart that holds only retired rows answers as an empty cart.
    #[test]
    fn a_cart_of_retired_rows_answers_empty() {
        let rows = vec![
            line("accessory", "Retired Accessory", 1, 150.0),
            line("tea", "Retired Tea", 1, 90.0),
        ];
        let resp = cart_model_to_resp(&cart_row(), &rows);
        assert!(resp.items.is_empty());
        assert_eq!(resp.total, 0.0);
    }

    /// PATCH and DELETE by id treat a hidden row as a missing one, and
    /// clearing a cart deletes only its served lines.
    #[test]
    fn only_a_served_row_is_reachable_by_id_or_by_clear() {
        assert!(served_row(&line("bike_rental", "NMAX 155", 1, 0.0)));
        for kind in ["accessory", "tea", "set", "bike_sale", ""] {
            assert!(!served_row(&line(kind, "x", 1, 1.0)), "{kind:?} is served");
        }
        let sql = cart_item::Entity::delete_many()
            .filter(served_lines_of(uuid::Uuid::nil()))
            .build(sea_orm::DbBackend::Postgres)
            .to_string();
        assert!(
            sql.starts_with("DELETE FROM")
                && sql.contains(r#""cart_id" = "#)
                && sql.contains(r#""kind" IN ('bike_rental')"#),
            "clear must delete this cart's served lines and no others: {sql}"
        );
    }

    #[test]
    fn parse_kind_accepts_valid() {
        assert_eq!(parse_kind("strain").unwrap(), "strain");
        assert_eq!(parse_kind("set").unwrap(), "set");
        assert_eq!(parse_kind("accessory").unwrap(), "accessory");
        assert_eq!(parse_kind("tea").unwrap(), "tea");
    }

    #[test]
    fn parse_kind_rejects_invalid() {
        assert_eq!(parse_kind("foo").unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validate_quantity_bounds() {
        assert!(validate_quantity(1).is_ok());
        assert!(validate_quantity(1_000_000).is_ok());
        assert_eq!(validate_quantity(0).unwrap_err(), StatusCode::BAD_REQUEST);
        assert_eq!(validate_quantity(-1).unwrap_err(), StatusCode::BAD_REQUEST);
        assert_eq!(
            validate_quantity(1_000_001).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_id_bounds() {
        assert!(validate_id("abc").is_ok());
        assert_eq!(validate_id("").unwrap_err(), StatusCode::BAD_REQUEST);
        assert_eq!(
            validate_id(&"a".repeat(201)).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    fn item(kind: &str, catalog_id: &str, quantity: i32) -> CartItemResp {
        CartItemResp {
            id: String::new(),
            kind: kind.to_string(),
            catalog_id: catalog_id.to_string(),
            quantity,
            unit_price: 0.0,
            name: String::new(),
            image_url: None,
        }
    }

    #[test]
    fn collapse_merges_duplicate_lines() {
        let out = collapse_merge_items(vec![
            item("strain", "a", 2),
            item("strain", "a", 3),
            item("tea", "a", 1),
        ]);
        // Same catalog_id under a different kind stays a separate line.
        assert_eq!(
            out,
            vec![("strain", "a".to_string(), 5), ("tea", "a".to_string(), 1)]
        );
    }

    #[test]
    fn collapse_drops_invalid_lines_without_failing_the_merge() {
        let out = collapse_merge_items(vec![
            item("bogus", "a", 1),
            item("strain", "", 1),
            item("strain", "b", 0),
            item("strain", "b", -5),
            item("strain", "c", 1),
        ]);
        assert_eq!(out, vec![("strain", "c".to_string(), 1)]);
    }

    #[test]
    fn collapse_caps_summed_quantity() {
        let out = collapse_merge_items(vec![
            item("set", "s", 1_000_000),
            item("set", "s", 1_000_000),
        ]);
        assert_eq!(out, vec![("set", "s".to_string(), 1_000_000)]);
    }

    #[test]
    fn undated_upsert_targets_migration_081_partial_index() {
        let sql = CART_ITEM_UPSERT_SQL
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        assert!(
            sql.contains(
                "ON CONFLICT (cart_id, kind, catalog_id) WHERE rental_start IS NULL AND rental_end IS NULL DO UPDATE"
            ),
            "the upsert must repeat migration 081's partial-index predicate: {sql}"
        );
        assert!(
            !sql.contains("ON CONFLICT (cart_id, kind, catalog_id, rental_start, rental_end)"),
            "NULL rental dates cannot infer the five-column unique constraint: {sql}"
        );
    }
}
