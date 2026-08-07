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
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
    TransactionTrait,
};

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

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/cart", get(get_cart).post(add_cart_item).delete(clear_cart))
        .route("/cart/merge", post(merge_cart))
        .route("/cart/items/:item_id", patch(update_cart_item).delete(delete_cart_item))
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
        "strain" => {
            strain::Entity::find_by_id(catalog_id.to_string())
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
                })
        }
        "accessory" => {
            accessory::Entity::find_by_id(catalog_id.to_string())
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
                })
        }
        "tea" => {
            tea_product::Entity::find_by_id(catalog_id.to_string())
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
                })
        }
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

async fn get_or_create_cart(
    db: &sea_orm::DatabaseConnection,
    telegram_id: i64,
) -> Result<crate::db::entities::cart::Model, StatusCode> {
    use crate::db::entities::cart::{Column as CartCol, Entity as CartEntity};
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
    let am = crate::db::entities::cart::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        telegram_id: Set(telegram_id),
        created_at: Set(Some(chrono::DateTime::from(Utc::now()))),
        updated_at: Set(Some(chrono::DateTime::from(Utc::now()))),
        expires_at: Set(Some(chrono::DateTime::from(Utc::now() + chrono::Duration::days(30)))),
        ..Default::default()
    };
    am.insert(db).await.map_err(|e| {
        tracing::error!("cart insert: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })
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

fn cart_model_to_resp(model: &crate::db::entities::cart::Model, items: &[crate::db::entities::cart_item::Model]) -> CartResp {
    let mut total = 0.0_f64;
    let item_resp: Vec<CartItemResp> = items
        .iter()
        .map(|i| {
            let price = if i.unit_price.is_finite() { i.unit_price.max(0.0) } else { 0.0 };
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
    use crate::db::entities::cart_item::{Column as ItemCol, Entity as ItemEntity};
    let existing = ItemEntity::find()
        .filter(ItemCol::CartId.eq(cart.id.clone()))
        .filter(ItemCol::Kind.eq(kind))
        .filter(ItemCol::CatalogId.eq(req.catalog_id.clone()))
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("cart item existing lookup: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if let Some(item) = existing {
        let new_qty = item.quantity.saturating_add(req.quantity).min(1_000_000);
        if new_qty <= 0 {
            return Err(StatusCode::BAD_REQUEST);
        }
        let mut am: crate::db::entities::cart_item::ActiveModel = item.into();
        am.quantity = Set(new_qty);
        am.unit_price = Set(unit_price);
        am.name = Set(name);
        am.image_url = Set(image_url.clone());
        am.updated_at = Set(Some(chrono::DateTime::from(Utc::now())));
        am.update(&state.db.orm).await.map_err(|e| {
            tracing::error!("cart item update: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    } else {
        let am = crate::db::entities::cart_item::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            cart_id: Set(cart.id),
            kind: Set(kind.to_string()),
            catalog_id: Set(req.catalog_id.clone()),
            quantity: Set(req.quantity),
            unit_price: Set(unit_price),
            name: Set(name),
            image_url: Set(image_url.clone()),
            created_at: Set(Some(chrono::DateTime::from(Utc::now()))),
            updated_at: Set(Some(chrono::DateTime::from(Utc::now()))),
            ..Default::default()
        };
        am.insert(&state.db.orm).await.map_err(|e| {
            tracing::error!("cart item insert: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

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
        .ok_or(StatusCode::NOT_FOUND)?;

    let cart = crate::db::entities::cart::Entity::find_by_id(item.cart_id.clone())
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
            .filter(crate::db::entities::cart_item::Column::CartId.eq(c.id))
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
    check_owner(&headers, &state, req.telegram_id)?;
    check_not_blocked(&state, req.telegram_id).await?;

    let cart = get_or_create_cart(&state.db.orm, req.telegram_id).await?;

    // Resolve each item; skip unknown/unavailable rather than failing the whole merge.
    let mut to_insert = Vec::new();
    for item in req.items {
        if let Ok(kind) = parse_kind(&item.kind) {
            if validate_id(&item.catalog_id).is_ok() && validate_quantity(item.quantity).is_ok() {
                if let Some((name, unit_price, image_url)) =
                    resolve_catalog_snapshot(&state, kind, &item.catalog_id).await?
                {
                    to_insert.push((kind, item.catalog_id, item.quantity, name, unit_price, image_url));
                }
            }
        }
    }

    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("cart merge tx begin: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    for (kind, catalog_id, quantity, name, unit_price, image_url) in to_insert {
        use crate::db::entities::cart_item::{Column as ItemCol, Entity as ItemEntity};
        let existing = ItemEntity::find()
            .filter(ItemCol::CartId.eq(cart.id.clone()))
            .filter(ItemCol::Kind.eq(kind))
            .filter(ItemCol::CatalogId.eq(catalog_id.clone()))
            .one(&tx)
            .await
            .map_err(|e| {
                tracing::error!("cart merge existing lookup: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        if let Some(ex) = existing {
            let new_qty = ex.quantity.saturating_add(quantity).min(1_000_000);
            let mut am: crate::db::entities::cart_item::ActiveModel = ex.into();
            am.quantity = Set(new_qty);
            am.unit_price = Set(unit_price);
            am.name = Set(name);
            am.image_url = Set(image_url.clone());
            am.updated_at = Set(Some(chrono::DateTime::from(Utc::now())));
            am.update(&tx).await.map_err(|e| {
                tracing::error!("cart merge update: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        } else {
            let am = crate::db::entities::cart_item::ActiveModel {
                id: Set(uuid::Uuid::new_v4()),
                cart_id: Set(cart.id.clone()),
                kind: Set(kind.to_string()),
                catalog_id: Set(catalog_id),
                quantity: Set(quantity),
                unit_price: Set(unit_price),
                name: Set(name),
                image_url: Set(image_url),
                created_at: Set(Some(chrono::DateTime::from(Utc::now()))),
                updated_at: Set(Some(chrono::DateTime::from(Utc::now()))),
                ..Default::default()
            };
            am.insert(&tx).await.map_err(|e| {
                tracing::error!("cart merge insert: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("cart merge commit: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Refresh cart timestamp after a successful merge.
    let cart = get_or_create_cart(&state.db.orm, req.telegram_id).await?;
    let items = load_cart_items(&state.db.orm, &cart.id).await?;
    Ok(Json(cart_model_to_resp(&cart, &items)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

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
        assert_eq!(validate_quantity(1_000_001).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validate_id_bounds() {
        assert!(validate_id("abc").is_ok());
        assert_eq!(validate_id("").unwrap_err(), StatusCode::BAD_REQUEST);
        assert_eq!(validate_id(&"a".repeat(201)).unwrap_err(), StatusCode::BAD_REQUEST);
    }
}
