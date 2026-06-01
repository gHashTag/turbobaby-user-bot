use crate::api::auth::check_admin;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn routes() -> Router<AppState> {
    Router::new()
        // Accessories
        .route("/accessories", get(get_accessories))
        .route("/accessories", post(create_accessory))
        .route("/accessories/:id", get(get_accessory))
        .route("/accessories/:id", put(update_accessory))
        .route("/accessories/:id", delete(delete_accessory))
        .route(
            "/accessories/:id/availability",
            put(toggle_accessory_availability),
        )
        // Accessory Sets
        .route("/accessory-sets", get(get_accessory_sets))
        .route("/accessory-sets", post(create_accessory_set))
        .route("/accessory-sets/:id", put(update_accessory_set))
        .route("/accessory-sets/:id", delete(delete_accessory_set))
        .route(
            "/accessory-sets/:id/availability",
            put(toggle_accessory_set_availability),
        )
        // Tea Products
        .route("/tea-products", get(get_tea_products))
        .route("/tea-products", post(create_tea_product))
        .route("/tea-products/:id", get(get_tea_product))
        .route("/tea-products/:id", put(update_tea_product))
        .route("/tea-products/:id", delete(delete_tea_product))
        .route(
            "/tea-products/:id/availability",
            put(toggle_tea_availability),
        )
        // Tea Sets
        .route("/tea-sets", get(get_tea_sets))
        .route("/tea-sets", post(create_tea_set))
        .route("/tea-sets/:id", put(update_tea_set))
        .route("/tea-sets/:id", delete(delete_tea_set))
        .route(
            "/tea-sets/:id/availability",
            put(toggle_tea_set_availability),
        )
        // Sets (strain sets)
        .route("/sets", get(get_sets))
        .route("/sets", post(create_set))
        .route("/sets/:id", put(update_set))
        .route("/sets/:id", delete(delete_set))
        .route("/sets/:id/availability", put(toggle_set_availability))
}

// ── Accessories ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AccessoryRequest {
    pub name: String,
    pub category: Option<String>,
    pub description: Option<String>,
    pub price: f64,
    pub stock: Option<i32>,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    #[allow(dead_code)]
    pub is_available: Option<bool>,
    // Bilingual EN fields (migration 016, all optional)
    pub name_en: Option<String>,
    pub description_en: Option<String>,
    pub category_en: Option<String>,
}

fn accessory_row(r: &sea_orm::QueryResult) -> Value {
    let price = {
        let v: f64 = crate::try_get_warn!(r, "price", 0.0);
        if v.is_finite() {
            v.max(0.0)
        } else {
            0.0
        }
    };
    json!({
        "id": r.try_get::<String>("", "id").unwrap_or_default(),
        "name": r.try_get::<String>("", "name").unwrap_or_default(),
        "category": r.try_get::<String>("", "category").unwrap_or_default(),
        "description": r.try_get::<String>("", "description").unwrap_or_default(),
        "price": price,
        "stock": r.try_get::<i32>("", "stock").unwrap_or(0),
        "image_url": r.try_get::<String>("", "image_url").unwrap_or_default(),
        "video_url": r.try_get::<String>("", "video_url").ok(),
        "is_available": r.try_get::<bool>("", "is_available").unwrap_or(false),
        "name_en": r.try_get::<Option<String>>("", "name_en").ok().flatten(),
        "description_en": r.try_get::<Option<String>>("", "description_en").ok().flatten(),
        "category_en": r.try_get::<Option<String>>("", "category_en").ok().flatten(),
    })
}

async fn get_accessories(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    // Cycle #95: SeaORM via Statement (pattern #15) — same shape repeated
    // across all 5 catalog tables.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let include_hidden = q
        .get("include_hidden")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);
    if include_hidden {
        crate::api::auth::check_admin(&headers, &state)?;
    }
    let sql = if include_hidden {
        "SELECT id, name, category, description, price::float8 AS price, stock, image_url, video_url, is_available, name_en, description_en, category_en FROM accessories ORDER BY name LIMIT 5000"
    } else {
        "SELECT id, name, category, description, price::float8 AS price, stock, image_url, video_url, is_available, name_en, description_en, category_en FROM accessories WHERE is_available = TRUE ORDER BY name LIMIT 2000"
    };
    let rows = state
        .db
        .orm
        .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await
        .map_err(|e| {
            tracing::error!("get_accessories: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let items: Vec<Value> = rows.iter().map(accessory_row).collect();
    Ok(Json(json!({ "accessories": items })))
}

async fn toggle_accessory_availability(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    let available = crate::api::extract_bool(&body, "is_available")?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE accessories SET is_available = $1 WHERE id = $2",
            [available.into(), id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("toggle_accessory_availability: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "success": true })))
}

async fn get_accessory(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = state.db.orm.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id, name, category, description, price::float8 AS price, stock, image_url, video_url, is_available, name_en, description_en, category_en FROM accessories WHERE id = $1 AND is_available = TRUE",
        [id.into()],
    )).await.map_err(|e| { tracing::error!("get_accessory: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    match row {
        Some(r) => Ok(Json(json!({ "accessory": accessory_row(&r) }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

fn validate_accessory_request(req: &AccessoryRequest) -> Result<(), StatusCode> {
    if req.name.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(ref c) = req.category {
        if c.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref d) = req.description {
        if d.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref n) = req.name_en {
        if n.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref d) = req.description_en {
        if d.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref c) = req.category_en {
        if c.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if !req.price.is_finite() || req.price < 0.0 || req.price > 1_000_000.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    crate::api::validate_url(&req.image_url)?;
    crate::api::validate_url(&req.video_url)?;
    Ok(())
}

async fn create_accessory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AccessoryRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_accessory_request(&req)?;
    let id = uuid::Uuid::new_v4().to_string();
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO accessories (id, name, category, description, price, stock, image_url, video_url, is_available, name_en, description_en, category_en) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        [
            id.clone().into(),
            req.name.into(),
            req.category.unwrap_or_default().into(),
            req.description.unwrap_or_default().into(),
            req.price.into(),
            req.stock.unwrap_or(0).into(),
            req.image_url.unwrap_or_default().into(),
            req.video_url.into(),
            true.into(),
            req.name_en.into(),
            req.description_en.into(),
            req.category_en.into(),
        ],
    )).await.map_err(|e| { tracing::error!("create_accessory: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_accessory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<AccessoryRequest>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    validate_accessory_request(&req)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE accessories SET name=$1, category=$2, description=$3, price=$4, stock=$5, image_url=$6, video_url=$7, name_en=$8, description_en=$9, category_en=$10, is_available=$11 WHERE id=$12",
        [
            req.name.into(),
            req.category.unwrap_or_default().into(),
            req.description.unwrap_or_default().into(),
            req.price.into(),
            req.stock.unwrap_or(0).into(),
            req.image_url.unwrap_or_default().into(),
            req.video_url.into(),
            req.name_en.into(),
            req.description_en.into(),
            req.category_en.into(),
            req.is_available.unwrap_or(true).into(),
            id.into(),
        ],
    )).await.map_err(|e| { tracing::error!("update_accessory: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_accessory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM accessories WHERE id = $1",
            [id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("delete_accessory: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "success": true })))
}

// ── Accessory Sets ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AccessorySetRequest {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub accessories: Option<Vec<String>>,
    pub total_price: f64,
    pub discount_percent: Option<f64>,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    #[allow(dead_code)]
    pub is_available: Option<bool>,
    pub is_deal_of_day: Option<bool>,
    // Bilingual EN fields (migration 016, all optional)
    pub name_en: Option<String>,
    pub description_en: Option<String>,
}

fn accessory_set_row(r: &sea_orm::QueryResult) -> Value {
    let accessories: Vec<String> = r
        .try_get::<Vec<String>>("", "accessories")
        .unwrap_or_default();
    let total_price = {
        let v: f64 = crate::try_get_warn!(r, "total_price", 0.0);
        if v.is_finite() {
            v.max(0.0)
        } else {
            0.0
        }
    };
    let discount_percent = {
        let v: f64 = crate::try_get_warn!(r, "discount_percent", 0.0);
        if v.is_finite() {
            v.max(0.0)
        } else {
            0.0
        }
    };
    json!({
        "id": r.try_get::<String>("", "id").unwrap_or_default(),
        "name": r.try_get::<String>("", "name").unwrap_or_default(),
        "description": r.try_get::<String>("", "description").unwrap_or_default(),
        "icon": r.try_get::<String>("", "icon").unwrap_or_default(),
        "accessories": accessories,
        "total_price": total_price,
        "discount_percent": discount_percent,
        "is_available": r.try_get::<bool>("", "is_available").unwrap_or(false),
        "is_deal_of_day": r.try_get::<bool>("", "is_deal_of_day").unwrap_or(false),
        "name_en": r.try_get::<Option<String>>("", "name_en").ok().flatten(),
        "description_en": r.try_get::<Option<String>>("", "description_en").ok().flatten(),
        "image_url": r.try_get::<Option<String>>("", "image_url").ok().flatten(),
        "video_url": r.try_get::<Option<String>>("", "video_url").ok().flatten(),
    })
}

async fn get_accessory_sets(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let include_hidden = q
        .get("include_hidden")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);
    if include_hidden {
        crate::api::auth::check_admin(&headers, &state)?;
    }
    let sql = if include_hidden {
        "SELECT id, name, description, icon, accessories, total_price::float8 AS total_price, discount_percent::float8 AS discount_percent, is_available, is_deal_of_day, name_en, description_en, image_url, video_url FROM accessory_sets ORDER BY name LIMIT 5000"
    } else {
        "SELECT id, name, description, icon, accessories, total_price::float8 AS total_price, discount_percent::float8 AS discount_percent, is_available, is_deal_of_day, name_en, description_en, image_url, video_url FROM accessory_sets WHERE is_available = TRUE ORDER BY name LIMIT 2000"
    };
    let rows = state
        .db
        .orm
        .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await
        .map_err(|e| {
            tracing::error!("get_accessory_sets: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let items: Vec<Value> = rows.iter().map(accessory_set_row).collect();
    Ok(Json(json!({ "accessory_sets": items })))
}

fn validate_accessory_set_request(req: &AccessorySetRequest) -> Result<(), StatusCode> {
    if req.name.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(ref d) = req.description {
        if d.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref i) = req.icon {
        if i.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref n) = req.name_en {
        if n.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref d) = req.description_en {
        if d.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if !req.total_price.is_finite() || req.total_price < 0.0 || req.total_price > 1_000_000.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(d) = req.discount_percent {
        if !d.is_finite() || !(0.0..=100.0).contains(&d) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    crate::api::validate_url(&req.image_url)?;
    crate::api::validate_url(&req.video_url)?;
    Ok(())
}

async fn create_accessory_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AccessorySetRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_accessory_set_request(&req)?;
    let id = uuid::Uuid::new_v4().to_string();
    let accessories = req.accessories.unwrap_or_default();
    if accessories.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO accessory_sets (id, name, description, icon, accessories, total_price, discount_percent, is_deal_of_day, name_en, description_en, image_url, video_url) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        [
            id.clone().into(),
            req.name.into(),
            req.description.unwrap_or_default().into(),
            req.icon.unwrap_or_default().into(),
            accessories.into(),
            req.total_price.into(),
            req.discount_percent.unwrap_or(0.0).into(),
            req.is_deal_of_day.unwrap_or(false).into(),
            req.name_en.into(),
            req.description_en.into(),
            req.image_url.into(),
            req.video_url.into(),
        ],
    )).await.map_err(|e| { tracing::error!("create_accessory_set: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_accessory_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<AccessorySetRequest>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    validate_accessory_set_request(&req)?;
    let accessories = req.accessories.unwrap_or_default();
    if accessories.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE accessory_sets SET name=$1, description=$2, icon=$3, accessories=$4, total_price=$5, discount_percent=$6, is_deal_of_day=$7, name_en=$8, description_en=$9, image_url=$10, video_url=$11, is_available=$12 WHERE id=$13",
        [
            req.name.into(),
            req.description.unwrap_or_default().into(),
            req.icon.unwrap_or_default().into(),
            accessories.into(),
            req.total_price.into(),
            req.discount_percent.unwrap_or(0.0).into(),
            req.is_deal_of_day.unwrap_or(false).into(),
            req.name_en.into(),
            req.description_en.into(),
            req.image_url.into(),
            req.video_url.into(),
            req.is_available.unwrap_or(true).into(),
            id.into(),
        ],
    )).await.map_err(|e| { tracing::error!("update_accessory_set: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_accessory_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM accessory_sets WHERE id = $1",
            [id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("delete_accessory_set: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "success": true })))
}

async fn toggle_accessory_set_availability(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    let available = crate::api::extract_bool(&body, "is_available")?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE accessory_sets SET is_available = $1 WHERE id = $2",
            [available.into(), id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("toggle_accessory_set_availability: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "success": true })))
}

// ── Tea Products ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TeaProductRequest {
    pub name: String,
    pub subcategory: Option<String>,
    pub description: Option<String>,
    pub price: f64,
    pub stock: Option<i32>,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    #[allow(dead_code)]
    pub is_available: Option<bool>,
    // Bilingual EN fields (migration 016, all optional)
    pub name_en: Option<String>,
    pub description_en: Option<String>,
    pub subcategory_en: Option<String>,
}

fn tea_product_row(r: &sea_orm::QueryResult) -> Value {
    let price = {
        let v: f64 = crate::try_get_warn!(r, "price", 0.0);
        if v.is_finite() {
            v.max(0.0)
        } else {
            0.0
        }
    };
    json!({
        "id": r.try_get::<String>("", "id").unwrap_or_default(),
        "name": r.try_get::<String>("", "name").unwrap_or_default(),
        "subcategory": r.try_get::<String>("", "subcategory").unwrap_or_default(),
        "description": r.try_get::<String>("", "description").unwrap_or_default(),
        "price": price,
        "stock": r.try_get::<i32>("", "stock").unwrap_or(0),
        "image_url": r.try_get::<String>("", "image_url").unwrap_or_default(),
        "video_url": r.try_get::<String>("", "video_url").ok(),
        "is_available": r.try_get::<bool>("", "is_available").unwrap_or(false),
        "name_en": r.try_get::<Option<String>>("", "name_en").ok().flatten(),
        "description_en": r.try_get::<Option<String>>("", "description_en").ok().flatten(),
        "subcategory_en": r.try_get::<Option<String>>("", "subcategory_en").ok().flatten(),
    })
}

async fn get_tea_products(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let include_hidden = q
        .get("include_hidden")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);
    if include_hidden {
        crate::api::auth::check_admin(&headers, &state)?;
    }
    let sql = if include_hidden {
        "SELECT id, name, subcategory, description, price::float8 AS price, stock, image_url, video_url, is_available, name_en, description_en, subcategory_en FROM tea_products ORDER BY name LIMIT 5000"
    } else {
        "SELECT id, name, subcategory, description, price::float8 AS price, stock, image_url, video_url, is_available, name_en, description_en, subcategory_en FROM tea_products WHERE is_available = TRUE ORDER BY name LIMIT 2000"
    };
    let rows = state
        .db
        .orm
        .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await
        .map_err(|e| {
            tracing::error!("get_tea_products: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let items: Vec<Value> = rows.iter().map(tea_product_row).collect();
    // Return both keys for backwards-compat: admin expects `tea_products`, /tea page expects `products`.
    Ok(Json(
        json!({ "tea_products": items.clone(), "products": items }),
    ))
}

async fn toggle_tea_availability(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    let available = crate::api::extract_bool(&body, "is_available")?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE tea_products SET is_available = $1 WHERE id = $2",
            [available.into(), id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("toggle_tea_availability: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "success": true })))
}

async fn get_tea_product(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = state.db.orm.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id, name, subcategory, description, price::float8 AS price, stock, image_url, video_url, is_available, name_en, description_en, subcategory_en FROM tea_products WHERE id = $1 AND is_available = TRUE",
        [id.into()],
    )).await.map_err(|e| { tracing::error!("get_tea_product: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    match row {
        Some(r) => Ok(Json(json!({ "tea_product": tea_product_row(&r) }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

fn validate_tea_product_request(req: &TeaProductRequest) -> Result<(), StatusCode> {
    if req.name.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(ref s) = req.subcategory {
        if s.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref d) = req.description {
        if d.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref n) = req.name_en {
        if n.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref d) = req.description_en {
        if d.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref s) = req.subcategory_en {
        if s.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if !req.price.is_finite() || req.price < 0.0 || req.price > 1_000_000.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    crate::api::validate_url(&req.image_url)?;
    crate::api::validate_url(&req.video_url)?;
    Ok(())
}

async fn create_tea_product(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<TeaProductRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_tea_product_request(&req)?;
    let id = uuid::Uuid::new_v4().to_string();
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO tea_products (id, name, subcategory, description, price, stock, image_url, video_url, is_available, name_en, description_en, subcategory_en) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        [
            id.clone().into(),
            req.name.into(),
            req.subcategory.unwrap_or_else(|| "tea".to_string()).into(),
            req.description.unwrap_or_default().into(),
            req.price.into(),
            req.stock.unwrap_or(0).into(),
            req.image_url.unwrap_or_default().into(),
            req.video_url.into(),
            true.into(),
            req.name_en.into(),
            req.description_en.into(),
            req.subcategory_en.into(),
        ],
    )).await.map_err(|e| { tracing::error!("create_tea_product: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_tea_product(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<TeaProductRequest>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    validate_tea_product_request(&req)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE tea_products SET name=$1, subcategory=$2, description=$3, price=$4, stock=$5, image_url=$6, video_url=$7, name_en=$8, description_en=$9, subcategory_en=$10, is_available=$11 WHERE id=$12",
        [
            req.name.into(),
            req.subcategory.unwrap_or_else(|| "tea".to_string()).into(),
            req.description.unwrap_or_default().into(),
            req.price.into(),
            req.stock.unwrap_or(0).into(),
            req.image_url.unwrap_or_default().into(),
            req.video_url.into(),
            req.name_en.into(),
            req.description_en.into(),
            req.subcategory_en.into(),
            req.is_available.unwrap_or(true).into(),
            id.into(),
        ],
    )).await.map_err(|e| { tracing::error!("update_tea_product: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_tea_product(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM tea_products WHERE id = $1",
            [id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("delete_tea_product: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "success": true })))
}

// ── Tea Sets ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TeaSetRequest {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub items: Option<Vec<String>>,
    pub total_price: f64,
    pub discount_percent: Option<f64>,
    pub video_url: Option<String>,
    #[allow(dead_code)]
    pub is_available: Option<bool>,
    // Bilingual EN fields (migration 016, all optional)
    pub name_en: Option<String>,
    pub description_en: Option<String>,
}

fn tea_set_row(r: &sea_orm::QueryResult) -> Value {
    let tea_items: Vec<String> = r.try_get::<Vec<String>>("", "items").unwrap_or_default();
    let total_price: f64 = crate::try_get_warn!(r, "total_price", 0.0);
    let total_price = if total_price.is_finite() {
        total_price.max(0.0)
    } else {
        0.0
    };
    let discount_percent: f64 = crate::try_get_warn!(r, "discount_percent", 0.0);
    let discount_percent = if discount_percent.is_finite() {
        discount_percent.max(0.0)
    } else {
        0.0
    };
    json!({
        "id": r.try_get::<String>("", "id").unwrap_or_default(),
        "name": r.try_get::<String>("", "name").unwrap_or_default(),
        "description": r.try_get::<String>("", "description").unwrap_or_default(),
        "icon": r.try_get::<String>("", "icon").unwrap_or_default(),
        "items": tea_items,
        "total_price": total_price,
        "discount_percent": discount_percent,
        "is_available": r.try_get::<bool>("", "is_available").unwrap_or(false),
        "name_en": r.try_get::<Option<String>>("", "name_en").ok().flatten(),
        "description_en": r.try_get::<Option<String>>("", "description_en").ok().flatten(),
        "video_url": r.try_get::<Option<String>>("", "video_url").ok().flatten(),
    })
}

async fn get_tea_sets(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let include_hidden = q
        .get("include_hidden")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);
    if include_hidden {
        crate::api::auth::check_admin(&headers, &state)?;
    }
    let sql = if include_hidden {
        "SELECT id, name, description, icon, items, total_price::float8 AS total_price, discount_percent::float8 AS discount_percent, is_available, name_en, description_en, video_url FROM tea_sets ORDER BY name LIMIT 5000"
    } else {
        "SELECT id, name, description, icon, items, total_price::float8 AS total_price, discount_percent::float8 AS discount_percent, is_available, name_en, description_en, video_url FROM tea_sets WHERE is_available = TRUE ORDER BY name LIMIT 2000"
    };
    let rows = state
        .db
        .orm
        .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await
        .map_err(|e| {
            tracing::error!("get_tea_sets: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let items: Vec<Value> = rows.iter().map(tea_set_row).collect();
    Ok(Json(json!({ "tea_sets": items })))
}

fn validate_tea_set_request(req: &TeaSetRequest) -> Result<(), StatusCode> {
    if req.name.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(ref d) = req.description {
        if d.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref i) = req.icon {
        if i.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref n) = req.name_en {
        if n.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref d) = req.description_en {
        if d.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if !req.total_price.is_finite() || req.total_price < 0.0 || req.total_price > 1_000_000.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(d) = req.discount_percent {
        if !d.is_finite() || !(0.0..=100.0).contains(&d) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    crate::api::validate_url(&req.video_url)?;
    Ok(())
}

async fn create_tea_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<TeaSetRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_tea_set_request(&req)?;
    let id = uuid::Uuid::new_v4().to_string();
    let tea_items = req.items.unwrap_or_default();
    if tea_items.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO tea_sets (id, name, description, icon, items, total_price, discount_percent, name_en, description_en, video_url) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        [
            id.clone().into(),
            req.name.into(),
            req.description.unwrap_or_default().into(),
            req.icon.unwrap_or_default().into(),
            tea_items.into(),
            req.total_price.into(),
            req.discount_percent.unwrap_or(0.0).into(),
            req.name_en.into(),
            req.description_en.into(),
            req.video_url.into(),
        ],
    )).await.map_err(|e| { tracing::error!("create_tea_set: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_tea_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<TeaSetRequest>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    validate_tea_set_request(&req)?;
    let tea_items = req.items.unwrap_or_default();
    if tea_items.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE tea_sets SET name=$1, description=$2, icon=$3, items=$4, total_price=$5, discount_percent=$6, name_en=$7, description_en=$8, video_url=$9, is_available=$10 WHERE id=$11",
        [
            req.name.into(),
            req.description.unwrap_or_default().into(),
            req.icon.unwrap_or_default().into(),
            tea_items.into(),
            req.total_price.into(),
            req.discount_percent.unwrap_or(0.0).into(),
            req.name_en.into(),
            req.description_en.into(),
            req.video_url.into(),
            req.is_available.unwrap_or(true).into(),
            id.into(),
        ],
    )).await.map_err(|e| { tracing::error!("update_tea_set: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_tea_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM tea_sets WHERE id = $1",
            [id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("delete_tea_set: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "success": true })))
}

async fn toggle_tea_set_availability(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    let available = crate::api::extract_bool(&body, "is_available")?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE tea_sets SET is_available = $1 WHERE id = $2",
            [available.into(), id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("toggle_tea_set_availability: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "success": true })))
}

// ── Sets (strain sets from `sets` table) ─────────────────────

#[derive(Debug, Deserialize)]
pub struct SetRequest {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub strain_ids: Option<Vec<String>>,
    pub accessory_ids: Option<Vec<String>>,
    pub total_price: f64,
    pub discount_percent: Option<f64>,
    pub video_url: Option<String>,
    #[allow(dead_code)]
    pub is_available: Option<bool>,
    pub is_deal_of_day: Option<bool>,
}

fn set_row(r: &sea_orm::QueryResult) -> Value {
    let strain_ids: Vec<String> = r
        .try_get::<Vec<String>>("", "strain_ids")
        .unwrap_or_default();
    let accessory_ids: Vec<String> = r
        .try_get::<Vec<String>>("", "accessory_ids")
        .unwrap_or_default();
    let total_price: f64 = crate::try_get_warn!(r, "total_price", 0.0);
    let total_price = if total_price.is_finite() {
        total_price.max(0.0)
    } else {
        0.0
    };
    let discount_percent: f64 = crate::try_get_warn!(r, "discount_percent", 0.0);
    let discount_percent = if discount_percent.is_finite() {
        discount_percent.max(0.0)
    } else {
        0.0
    };
    json!({
        "id": r.try_get::<String>("", "id").unwrap_or_default(),
        "name": r.try_get::<String>("", "name").unwrap_or_default(),
        "description": r.try_get::<String>("", "description").unwrap_or_default(),
        "icon": r.try_get::<String>("", "icon").unwrap_or_default(),
        "strain_ids": strain_ids,
        "accessory_ids": accessory_ids,
        "total_price": total_price,
        "discount_percent": discount_percent,
        "is_available": r.try_get::<bool>("", "is_available").unwrap_or(false),
        "is_deal_of_day": r.try_get::<bool>("", "is_deal_of_day").unwrap_or(false),
        "video_url": r.try_get::<Option<String>>("", "video_url").ok().flatten(),
    })
}

async fn get_sets(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let include_hidden = q
        .get("include_hidden")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);
    if include_hidden {
        crate::api::auth::check_admin(&headers, &state)?;
    }

    if include_hidden {
        // Use a single compatible query that works with or without accessory_ids column.
        let rows = state.db.orm.query_all(Statement::from_string(
            DbBackend::Postgres,
            "SELECT id, name, description, icon, strain_ids, COALESCE(accessory_ids, NULL::text[]) AS accessory_ids, total_price::float8 AS total_price, discount_percent::float8 AS discount_percent, is_available, COALESCE(is_deal_of_day, false) AS is_deal_of_day, video_url FROM sets ORDER BY name LIMIT 5000".to_string(),
        )).await.map_err(|e| { tracing::error!("get_sets admin: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
        let items: Vec<Value> = rows.iter().map(set_row).collect();
        return Ok(Json(json!({ "sets": items })));
    }

    // Public mode: combine accessory_sets + tea_sets (backwards compat)
    let accessory_sets = state.db.orm.query_all(Statement::from_string(
        DbBackend::Postgres,
        "SELECT id, name, description, icon, accessories, total_price::float8 AS total_price, discount_percent::float8 AS discount_percent, is_available, is_deal_of_day, name_en, description_en, image_url, video_url FROM accessory_sets WHERE is_available = TRUE LIMIT 2000".to_string(),
    )).await.map_err(|e| { tracing::error!("get_sets accessory_sets: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;

    let tea_sets = state.db.orm.query_all(Statement::from_string(
        DbBackend::Postgres,
        "SELECT id, name, description, icon, items, total_price::float8 AS total_price, discount_percent::float8 AS discount_percent, is_available, name_en, description_en, image_url, video_url FROM tea_sets WHERE is_available = TRUE LIMIT 2000".to_string(),
    )).await.map_err(|e| { tracing::error!("get_sets tea_sets: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;

    let mut items: Vec<serde_json::Value> = Vec::new();

    for r in accessory_sets.iter() {
        let accessories: Vec<String> = r
            .try_get::<Vec<String>>("", "accessories")
            .unwrap_or_default();
        let total_price: f64 = crate::try_get_warn!(r, "total_price", 0.0);
        let total_price = if total_price.is_finite() {
            total_price.max(0.0)
        } else {
            0.0
        };
        let discount_percent: f64 = crate::try_get_warn!(r, "discount_percent", 0.0);
        let discount_percent = if discount_percent.is_finite() {
            discount_percent.max(0.0)
        } else {
            0.0
        };
        items.push(json!({
            "id": r.try_get::<String>("", "id").unwrap_or_default(),
            "name": r.try_get::<String>("", "name").unwrap_or_default(),
            "description": r.try_get::<String>("", "description").unwrap_or_default(),
            "icon": r.try_get::<String>("", "icon").unwrap_or_default(),
            "type": "accessory",
            "items": accessories,
            "total_price": total_price,
            "discount_percent": discount_percent,
            "is_available": r.try_get::<bool>("", "is_available").unwrap_or(false),
            "is_deal_of_day": r.try_get::<bool>("", "is_deal_of_day").unwrap_or(false),
            "name_en": r.try_get::<Option<String>>("", "name_en").ok().flatten(),
            "description_en": r.try_get::<Option<String>>("", "description_en").ok().flatten(),
            "image_url": r.try_get::<Option<String>>("", "image_url").ok().flatten(),
            "video_url": r.try_get::<Option<String>>("", "video_url").ok().flatten(),
        }));
    }

    for r in tea_sets.iter() {
        let tea_items: Vec<String> = r.try_get::<Vec<String>>("", "items").unwrap_or_default();
        let total_price: f64 = crate::try_get_warn!(r, "total_price", 0.0);
        let total_price = if total_price.is_finite() {
            total_price.max(0.0)
        } else {
            0.0
        };
        let discount_percent: f64 = crate::try_get_warn!(r, "discount_percent", 0.0);
        let discount_percent = if discount_percent.is_finite() {
            discount_percent.max(0.0)
        } else {
            0.0
        };
        items.push(json!({
            "id": r.try_get::<String>("", "id").unwrap_or_default(),
            "name": r.try_get::<String>("", "name").unwrap_or_default(),
            "description": r.try_get::<String>("", "description").unwrap_or_default(),
            "icon": r.try_get::<String>("", "icon").unwrap_or_default(),
            "type": "tea",
            "items": tea_items,
            "total_price": total_price,
            "discount_percent": discount_percent,
            "is_available": r.try_get::<bool>("", "is_available").unwrap_or(false),
            "is_deal_of_day": false,
            "name_en": r.try_get::<Option<String>>("", "name_en").ok().flatten(),
            "description_en": r.try_get::<Option<String>>("", "description_en").ok().flatten(),
            "image_url": r.try_get::<Option<String>>("", "image_url").ok().flatten(),
            "video_url": r.try_get::<Option<String>>("", "video_url").ok().flatten(),
        }));
    }

    Ok(Json(json!({ "sets": items })))
}

fn validate_set_request(req: &SetRequest) -> Result<(), StatusCode> {
    if req.name.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(ref d) = req.description {
        if d.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref i) = req.icon {
        if i.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if !req.total_price.is_finite() || req.total_price < 0.0 || req.total_price > 1_000_000.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(d) = req.discount_percent {
        if !d.is_finite() || !(0.0..=100.0).contains(&d) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    crate::api::validate_url(&req.video_url)?;
    Ok(())
}

async fn create_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SetRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_set_request(&req)?;
    let id = uuid::Uuid::new_v4().to_string();
    let strain_ids = req.strain_ids.unwrap_or_default();
    let accessory_ids = req.accessory_ids.unwrap_or_default();
    if strain_ids.len() > 100 || accessory_ids.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO sets (id, name, description, icon, strain_ids, accessory_ids, total_price, discount_percent, is_deal_of_day, video_url) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        [
            id.clone().into(),
            req.name.into(),
            req.description.unwrap_or_default().into(),
            req.icon.unwrap_or_default().into(),
            strain_ids.into(),
            accessory_ids.into(),
            req.total_price.into(),
            req.discount_percent.unwrap_or(0.0).into(),
            req.is_deal_of_day.unwrap_or(false).into(),
            req.video_url.into(),
        ],
    )).await.map_err(|e| { tracing::error!("create_set: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<SetRequest>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    validate_set_request(&req)?;
    let strain_ids = req.strain_ids.unwrap_or_default();
    let accessory_ids = req.accessory_ids.unwrap_or_default();
    if strain_ids.len() > 100 || accessory_ids.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE sets SET name=$1, description=$2, icon=$3, strain_ids=$4, accessory_ids=$5, total_price=$6, discount_percent=$7, is_deal_of_day=$8, video_url=$9, is_available=$10 WHERE id=$11",
        [
            req.name.into(),
            req.description.unwrap_or_default().into(),
            req.icon.unwrap_or_default().into(),
            strain_ids.into(),
            accessory_ids.into(),
            req.total_price.into(),
            req.discount_percent.unwrap_or(0.0).into(),
            req.is_deal_of_day.unwrap_or(false).into(),
            req.video_url.into(),
            req.is_available.unwrap_or(true).into(),
            id.into(),
        ],
    )).await.map_err(|e| { tracing::error!("update_set: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM sets WHERE id = $1",
            [id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("delete_set: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "success": true })))
}

async fn toggle_set_availability(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    let available = crate::api::extract_bool(&body, "is_available")?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE sets SET is_available = $1 WHERE id = $2",
            [available.into(), id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("toggle_set_availability: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "success": true })))
}

#[cfg(test)]
mod tests {
    use super::{
        validate_accessory_request, validate_accessory_set_request, validate_set_request,
        validate_tea_product_request, validate_tea_set_request, AccessoryRequest,
        AccessorySetRequest, SetRequest, TeaProductRequest, TeaSetRequest,
    };
    use axum::http::StatusCode;

    fn valid_accessory() -> AccessoryRequest {
        AccessoryRequest {
            name: "Pipe".into(),
            category: Some("Glass".into()),
            description: Some("Nice pipe".into()),
            price: 100.0,
            stock: Some(10),
            image_url: Some("/uploads/pipe.jpg".into()),
            video_url: None,
            is_available: Some(true),
            name_en: Some("Pipe".into()),
            description_en: Some("Nice pipe".into()),
            category_en: Some("Glass".into()),
        }
    }

    #[test]
    fn test_validate_accessory_ok() {
        assert!(validate_accessory_request(&valid_accessory()).is_ok());
    }

    #[test]
    fn test_validate_accessory_name_too_long() {
        let mut req = valid_accessory();
        req.name = "a".repeat(201);
        assert_eq!(
            validate_accessory_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_accessory_category_too_long() {
        let mut req = valid_accessory();
        req.category = Some("a".repeat(201));
        assert_eq!(
            validate_accessory_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_accessory_description_too_long() {
        let mut req = valid_accessory();
        req.description = Some("a".repeat(1001));
        assert_eq!(
            validate_accessory_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_accessory_price_negative() {
        let mut req = valid_accessory();
        req.price = -1.0;
        assert_eq!(
            validate_accessory_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_accessory_price_too_high() {
        let mut req = valid_accessory();
        req.price = 2_000_000.0;
        assert_eq!(
            validate_accessory_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_accessory_price_nan() {
        let mut req = valid_accessory();
        req.price = f64::NAN;
        assert_eq!(
            validate_accessory_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_accessory_bad_image_url() {
        let mut req = valid_accessory();
        req.image_url = Some("javascript:alert(1)".into());
        assert_eq!(
            validate_accessory_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    fn valid_tea() -> TeaProductRequest {
        TeaProductRequest {
            name: "Green Tea".into(),
            subcategory: Some("Herbal".into()),
            description: Some("Fresh".into()),
            price: 50.0,
            stock: Some(5),
            image_url: None,
            video_url: None,
            is_available: Some(true),
            name_en: Some("Green Tea".into()),
            description_en: Some("Fresh".into()),
            subcategory_en: Some("Herbal".into()),
        }
    }

    #[test]
    fn test_validate_tea_ok() {
        assert!(validate_tea_product_request(&valid_tea()).is_ok());
    }

    #[test]
    fn test_validate_tea_name_too_long() {
        let mut req = valid_tea();
        req.name = "a".repeat(201);
        assert_eq!(
            validate_tea_product_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_tea_subcategory_too_long() {
        let mut req = valid_tea();
        req.subcategory = Some("a".repeat(201));
        assert_eq!(
            validate_tea_product_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_tea_price_negative() {
        let mut req = valid_tea();
        req.price = -0.01;
        assert_eq!(
            validate_tea_product_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    fn valid_accessory_set() -> AccessorySetRequest {
        AccessorySetRequest {
            name: "Set".into(),
            description: Some("desc".into()),
            icon: Some("icon".into()),
            accessories: Some(vec!["a1".into()]),
            total_price: 100.0,
            discount_percent: Some(10.0),
            image_url: Some("/uploads/set.jpg".into()),
            video_url: None,
            is_available: Some(true),
            is_deal_of_day: Some(false),
            name_en: Some("Set".into()),
            description_en: Some("desc".into()),
        }
    }

    #[test]
    fn test_validate_accessory_set_ok() {
        assert!(validate_accessory_set_request(&valid_accessory_set()).is_ok());
    }

    #[test]
    fn test_validate_accessory_set_name_too_long() {
        let mut req = valid_accessory_set();
        req.name = "a".repeat(201);
        assert_eq!(
            validate_accessory_set_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_accessory_set_total_price_negative() {
        let mut req = valid_accessory_set();
        req.total_price = -1.0;
        assert_eq!(
            validate_accessory_set_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_accessory_set_discount_too_high() {
        let mut req = valid_accessory_set();
        req.discount_percent = Some(101.0);
        assert_eq!(
            validate_accessory_set_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    fn valid_tea_set() -> TeaSetRequest {
        TeaSetRequest {
            name: "Tea Set".into(),
            description: Some("desc".into()),
            icon: Some("icon".into()),
            items: Some(vec!["t1".into()]),
            total_price: 100.0,
            discount_percent: Some(10.0),
            video_url: None,
            is_available: Some(true),
            name_en: Some("Tea Set".into()),
            description_en: Some("desc".into()),
        }
    }

    #[test]
    fn test_validate_tea_set_ok() {
        assert!(validate_tea_set_request(&valid_tea_set()).is_ok());
    }

    #[test]
    fn test_validate_tea_set_name_too_long() {
        let mut req = valid_tea_set();
        req.name = "a".repeat(201);
        assert_eq!(
            validate_tea_set_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_tea_set_total_price_nan() {
        let mut req = valid_tea_set();
        req.total_price = f64::NAN;
        assert_eq!(
            validate_tea_set_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    fn valid_set() -> SetRequest {
        SetRequest {
            name: "Strain Set".into(),
            description: Some("desc".into()),
            icon: Some("icon".into()),
            strain_ids: Some(vec!["s1".into()]),
            accessory_ids: Some(vec!["a1".into()]),
            total_price: 100.0,
            discount_percent: Some(10.0),
            video_url: None,
            is_available: Some(true),
            is_deal_of_day: Some(false),
        }
    }

    #[test]
    fn test_validate_set_ok() {
        assert!(validate_set_request(&valid_set()).is_ok());
    }

    #[test]
    fn test_validate_set_name_too_long() {
        let mut req = valid_set();
        req.name = "a".repeat(201);
        assert_eq!(
            validate_set_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_set_total_price_too_high() {
        let mut req = valid_set();
        req.total_price = 2_000_000.0;
        assert_eq!(
            validate_set_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }
}
