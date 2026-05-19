use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post, put},
    Json, Router,
};
use std::collections::HashMap;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::AppState;
use crate::api::auth::check_admin;

pub fn routes() -> Router<AppState> {
    Router::new()
        // Accessories
        .route("/accessories", get(get_accessories))
        .route("/accessories", post(create_accessory))
        .route("/accessories/:id", get(get_accessory))
        .route("/accessories/:id", put(update_accessory))
        .route("/accessories/:id", delete(delete_accessory))
        .route("/accessories/:id/availability", put(toggle_accessory_availability))
        // Accessory Sets
        .route("/accessory-sets", get(get_accessory_sets))
        .route("/accessory-sets", post(create_accessory_set))
        .route("/accessory-sets/:id", put(update_accessory_set))
        .route("/accessory-sets/:id", delete(delete_accessory_set))
        // Tea Products
        .route("/tea-products", get(get_tea_products))
        .route("/tea-products", post(create_tea_product))
        .route("/tea-products/:id", get(get_tea_product))
        .route("/tea-products/:id", put(update_tea_product))
        .route("/tea-products/:id", delete(delete_tea_product))
        .route("/tea-products/:id/availability", put(toggle_tea_availability))
        // Tea Sets
        .route("/tea-sets", get(get_tea_sets))
        .route("/tea-sets", post(create_tea_set))
        .route("/tea-sets/:id", put(update_tea_set))
        .route("/tea-sets/:id", delete(delete_tea_set))
        // Sets
        .route("/sets", get(get_sets))
        .route("/sets", post(create_set))
        .route("/sets/:id", put(update_set))
        .route("/sets/:id", delete(delete_set))
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

fn accessory_row(r: &tokio_postgres::Row) -> Value {
    json!({
        "id": r.try_get::<_, String>(0).unwrap_or_default(),
        "name": r.try_get::<_, String>(1).unwrap_or_default(),
        "category": r.try_get::<_, String>(2).unwrap_or_default(),
        "description": r.try_get::<_, String>(3).unwrap_or_default(),
        "price": r.try_get::<_, f64>(4).unwrap_or(0.0),
        "stock": r.try_get::<_, i32>(5).unwrap_or(0),
        "image_url": r.try_get::<_, String>(6).unwrap_or_default(),
        "video_url": r.try_get::<_, String>(7).ok(),
        "is_available": r.try_get::<_, bool>(8).unwrap_or(false),
        "name_en": r.try_get::<_, Option<String>>(9).ok().flatten(),
        "description_en": r.try_get::<_, Option<String>>(10).ok().flatten(),
        "category_en": r.try_get::<_, Option<String>>(11).ok().flatten(),
    })
}

async fn get_accessories(State(state): State<AppState>, Query(q): Query<HashMap<String, String>>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let include_hidden = q.get("include_hidden").map(|v| v == "1" || v == "true").unwrap_or(false);
    let sql = if include_hidden {
        "SELECT id, name, category, description, price, stock, image_url, video_url, is_available, name_en, description_en, category_en FROM accessories ORDER BY name"
    } else {
        "SELECT id, name, category, description, price, stock, image_url, video_url, is_available, name_en, description_en, category_en FROM accessories WHERE is_available = TRUE ORDER BY name"
    };
    let rows = client.query(sql, &[]).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let items: Vec<Value> = rows.iter().map(accessory_row).collect();
    Ok(Json(json!({ "accessories": items })))
}

async fn toggle_accessory_availability(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<Value>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let available = body["is_available"].as_bool().unwrap_or(true);
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute("UPDATE accessories SET is_available = $1 WHERE id = $2", &[&available, &id])
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn get_accessory(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = client.query_opt(
        "SELECT id, name, category, description, price, stock, image_url, video_url, is_available, name_en, description_en, category_en FROM accessories WHERE id = $1",
        &[&id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match row {
        Some(r) => Ok(Json(json!({ "accessory": accessory_row(&r) }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn create_accessory(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<AccessoryRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let id = uuid::Uuid::new_v4().to_string();
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("create_accessory pool error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    client.execute(
        "INSERT INTO accessories (id, name, category, description, price, stock, image_url, video_url, is_available, name_en, description_en, category_en) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        &[&id, &req.name, &req.category.unwrap_or_default(), &req.description.unwrap_or_default(), &req.price, &req.stock.unwrap_or(0), &req.image_url.unwrap_or_default(), &req.video_url, &true, &req.name_en, &req.description_en, &req.category_en],
    ).await.map_err(|e| { tracing::error!("create_accessory error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_accessory(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(req): Json<AccessoryRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "UPDATE accessories SET name=$1, category=$2, description=$3, price=$4, stock=$5, image_url=$6, video_url=$7, name_en=$8, description_en=$9, category_en=$10 WHERE id=$11",
        &[&req.name, &req.category.unwrap_or_default(), &req.description.unwrap_or_default(), &req.price, &req.stock.unwrap_or(0), &req.image_url.unwrap_or_default(), &req.video_url, &req.name_en, &req.description_en, &req.category_en, &id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_accessory(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("delete_accessory pool error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    client.execute("DELETE FROM accessories WHERE id = $1", &[&id])
        .await.map_err(|e| { tracing::error!("delete_accessory error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
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
    #[allow(dead_code)]
    pub is_available: Option<bool>,
    pub is_deal_of_day: Option<bool>,
    // Bilingual EN fields (migration 016, all optional)
    pub name_en: Option<String>,
    pub description_en: Option<String>,
}

fn accessory_set_row(r: &tokio_postgres::Row) -> Value {
    let accessories: Vec<String> = r.try_get::<_, Vec<String>>(4)
        .unwrap_or_else(|_| vec![]);
    json!({
        "id": r.try_get::<_, String>(0).unwrap_or_default(),
        "name": r.try_get::<_, String>(1).unwrap_or_default(),
        "description": r.try_get::<_, String>(2).unwrap_or_default(),
        "icon": r.try_get::<_, String>(3).unwrap_or_default(),
        "accessories": accessories,
        "total_price": r.try_get::<_, f64>(5).unwrap_or(0.0),
        "discount_percent": r.try_get::<_, f64>(6).unwrap_or(0.0),
        "is_available": r.try_get::<_, bool>(7).unwrap_or(false),
        "is_deal_of_day": r.try_get::<_, bool>(8).unwrap_or(false),
        "name_en": r.try_get::<_, Option<String>>(9).ok().flatten(),
        "description_en": r.try_get::<_, Option<String>>(10).ok().flatten(),
    })
}

async fn get_accessory_sets(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = client.query(
        "SELECT id, name, description, icon, accessories, total_price, discount_percent, is_available, is_deal_of_day, name_en, description_en FROM accessory_sets WHERE is_available = TRUE ORDER BY name",
        &[],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let items: Vec<Value> = rows.iter().map(accessory_set_row).collect();
    Ok(Json(json!({ "accessory_sets": items })))
}

async fn create_accessory_set(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<AccessorySetRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let id = uuid::Uuid::new_v4().to_string();
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let accessories = req.accessories.unwrap_or_default();
    client.execute(
        "INSERT INTO accessory_sets (id, name, description, icon, accessories, total_price, discount_percent, is_deal_of_day, name_en, description_en) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        &[&id, &req.name, &req.description.unwrap_or_default(), &req.icon.unwrap_or_default(), &accessories, &req.total_price, &req.discount_percent.unwrap_or(0.0), &req.is_deal_of_day.unwrap_or(false), &req.name_en, &req.description_en],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_accessory_set(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(req): Json<AccessorySetRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let accessories = req.accessories.unwrap_or_default();
    client.execute(
        "UPDATE accessory_sets SET name=$1, description=$2, icon=$3, accessories=$4, total_price=$5, discount_percent=$6, is_deal_of_day=$7, name_en=$8, description_en=$9 WHERE id=$10",
        &[&req.name, &req.description.unwrap_or_default(), &req.icon.unwrap_or_default(), &accessories, &req.total_price, &req.discount_percent.unwrap_or(0.0), &req.is_deal_of_day.unwrap_or(false), &req.name_en, &req.description_en, &id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_accessory_set(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute("UPDATE accessory_sets SET is_available = false WHERE id = $1", &[&id])
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
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

fn tea_product_row(r: &tokio_postgres::Row) -> Value {
    json!({
        "id": r.try_get::<_, String>(0).unwrap_or_default(),
        "name": r.try_get::<_, String>(1).unwrap_or_default(),
        "subcategory": r.try_get::<_, String>(2).unwrap_or_default(),
        "description": r.try_get::<_, String>(3).unwrap_or_default(),
        "price": r.try_get::<_, f64>(4).unwrap_or(0.0),
        "stock": r.try_get::<_, i32>(5).unwrap_or(0),
        "image_url": r.try_get::<_, String>(6).unwrap_or_default(),
        "video_url": r.try_get::<_, String>(7).ok(),
        "is_available": r.try_get::<_, bool>(8).unwrap_or(false),
        "name_en": r.try_get::<_, Option<String>>(9).ok().flatten(),
        "description_en": r.try_get::<_, Option<String>>(10).ok().flatten(),
        "subcategory_en": r.try_get::<_, Option<String>>(11).ok().flatten(),
    })
}

async fn get_tea_products(State(state): State<AppState>, Query(q): Query<HashMap<String, String>>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let include_hidden = q.get("include_hidden").map(|v| v == "1" || v == "true").unwrap_or(false);
    let sql = if include_hidden {
        "SELECT id, name, subcategory, description, price, stock, image_url, video_url, is_available, name_en, description_en, subcategory_en FROM tea_products ORDER BY name"
    } else {
        "SELECT id, name, subcategory, description, price, stock, image_url, video_url, is_available, name_en, description_en, subcategory_en FROM tea_products WHERE is_available = TRUE ORDER BY name"
    };
    let rows = client.query(sql, &[]).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let items: Vec<Value> = rows.iter().map(tea_product_row).collect();
    // Return both keys for backwards-compat: admin expects `tea_products`, /tea page expects `products`.
    Ok(Json(json!({ "tea_products": items.clone(), "products": items })))
}

async fn toggle_tea_availability(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<Value>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let available = body["is_available"].as_bool().unwrap_or(true);
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute("UPDATE tea_products SET is_available = $1 WHERE id = $2", &[&available, &id])
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn get_tea_product(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = client.query_opt(
        "SELECT id, name, subcategory, description, price, stock, image_url, video_url, is_available, name_en, description_en, subcategory_en FROM tea_products WHERE id = $1",
        &[&id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match row {
        Some(r) => Ok(Json(json!({ "tea_product": tea_product_row(&r) }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn create_tea_product(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<TeaProductRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let id = uuid::Uuid::new_v4().to_string();
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("create_tea_product pool error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    client.execute(
        "INSERT INTO tea_products (id, name, subcategory, description, price, stock, image_url, video_url, is_available, name_en, description_en, subcategory_en) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        &[&id, &req.name, &req.subcategory.unwrap_or_else(|| "tea".to_string()), &req.description.unwrap_or_default(), &req.price, &req.stock.unwrap_or(0), &req.image_url.unwrap_or_default(), &req.video_url, &true, &req.name_en, &req.description_en, &req.subcategory_en],
    ).await.map_err(|e| { tracing::error!("create_tea_product error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_tea_product(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(req): Json<TeaProductRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "UPDATE tea_products SET name=$1, subcategory=$2, description=$3, price=$4, stock=$5, image_url=$6, video_url=$7, name_en=$8, description_en=$9, subcategory_en=$10 WHERE id=$11",
        &[&req.name, &req.subcategory.unwrap_or_else(|| "tea".to_string()), &req.description.unwrap_or_default(), &req.price, &req.stock.unwrap_or(0), &req.image_url.unwrap_or_default(), &req.video_url, &req.name_en, &req.description_en, &req.subcategory_en, &id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_tea_product(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("delete_tea_product pool error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    client.execute("DELETE FROM tea_products WHERE id = $1", &[&id])
        .await.map_err(|e| { tracing::error!("delete_tea_product error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
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
    #[allow(dead_code)]
    pub is_available: Option<bool>,
    // Bilingual EN fields (migration 016, all optional)
    pub name_en: Option<String>,
    pub description_en: Option<String>,
}

fn tea_set_row(r: &tokio_postgres::Row) -> Value {
    let tea_items: Vec<String> = r.try_get::<_, Vec<String>>(4)
        .unwrap_or_else(|_| vec![]);
    json!({
        "id": r.try_get::<_, String>(0).unwrap_or_default(),
        "name": r.try_get::<_, String>(1).unwrap_or_default(),
        "description": r.try_get::<_, String>(2).unwrap_or_default(),
        "icon": r.try_get::<_, String>(3).unwrap_or_default(),
        "items": tea_items,
        "total_price": r.try_get::<_, f64>(5).unwrap_or(0.0),
        "discount_percent": r.try_get::<_, f64>(6).unwrap_or(0.0),
        "is_available": r.try_get::<_, bool>(7).unwrap_or(false),
        "name_en": r.try_get::<_, Option<String>>(8).ok().flatten(),
        "description_en": r.try_get::<_, Option<String>>(9).ok().flatten(),
    })
}

async fn get_tea_sets(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = client.query(
        "SELECT id, name, description, icon, items, total_price, discount_percent, is_available, name_en, description_en FROM tea_sets WHERE is_available = TRUE ORDER BY name",
        &[],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let items: Vec<Value> = rows.iter().map(tea_set_row).collect();
    Ok(Json(json!({ "tea_sets": items })))
}

async fn create_tea_set(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<TeaSetRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let id = uuid::Uuid::new_v4().to_string();
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let tea_items = req.items.unwrap_or_default();
    client.execute(
        "INSERT INTO tea_sets (id, name, description, icon, items, total_price, discount_percent, name_en, description_en) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        &[&id, &req.name, &req.description.unwrap_or_default(), &req.icon.unwrap_or_default(), &tea_items, &req.total_price, &req.discount_percent.unwrap_or(0.0), &req.name_en, &req.description_en],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_tea_set(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(req): Json<TeaSetRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let tea_items = req.items.unwrap_or_default();
    client.execute(
        "UPDATE tea_sets SET name=$1, description=$2, icon=$3, items=$4, total_price=$5, discount_percent=$6, name_en=$7, description_en=$8 WHERE id=$9",
        &[&req.name, &req.description.unwrap_or_default(), &req.icon.unwrap_or_default(), &tea_items, &req.total_price, &req.discount_percent.unwrap_or(0.0), &req.name_en, &req.description_en, &id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_tea_set(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute("UPDATE tea_sets SET is_available = false WHERE id = $1", &[&id])
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

// ── Sets (combined from accessory_sets and tea_sets) ───────

#[derive(Debug, Deserialize)]
pub struct SetRequest {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub strain_ids: Option<Vec<String>>,
    pub accessory_ids: Option<Vec<String>>,
    pub total_price: f64,
    pub discount_percent: Option<f64>,
    #[allow(dead_code)]
    pub is_available: Option<bool>,
    pub is_deal_of_day: Option<bool>,
}

async fn get_sets(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get accessory sets — uppercase TRUE forces a new prepared statement
    // identity after schema ALTER TYPE invalidated the previous one.
    let accessory_sets = client.query(
        "SELECT id, name, description, icon, accessories, total_price, discount_percent, is_available, is_deal_of_day, name_en, description_en FROM accessory_sets WHERE is_available = TRUE",
        &[],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get tea sets
    let tea_sets = client.query(
        "SELECT id, name, description, icon, items, total_price, discount_percent, is_available, name_en, description_en FROM tea_sets WHERE is_available = TRUE",
        &[],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut items: Vec<serde_json::Value> = Vec::new();

    // Add accessory sets
    for r in accessory_sets.iter() {
        // Safe extraction of array column
        let accessories: Vec<String> = r.try_get::<_, Vec<String>>(4)
            .unwrap_or_else(|_| vec![]);
        items.push(json!({
            "id": r.try_get::<_, String>(0).unwrap_or_default(),
            "name": r.try_get::<_, String>(1).unwrap_or_default(),
            "description": r.try_get::<_, String>(2).unwrap_or_default(),
            "icon": r.try_get::<_, String>(3).unwrap_or_default(),
            "type": "accessory",
            "items": accessories,
            "total_price": r.try_get::<_, f64>(5).unwrap_or(0.0),
            "discount_percent": r.try_get::<_, f64>(6).unwrap_or(0.0),
            "is_available": r.try_get::<_, bool>(7).unwrap_or(false),
            "is_deal_of_day": r.try_get::<_, bool>(8).unwrap_or(false),
            "name_en": r.try_get::<_, Option<String>>(9).ok().flatten(),
            "description_en": r.try_get::<_, Option<String>>(10).ok().flatten(),
        }));
    }

    // Add tea sets
    for r in tea_sets.iter() {
        // Safe extraction of array column
        let tea_items: Vec<String> = r.try_get::<_, Vec<String>>(4)
            .unwrap_or_else(|_| vec![]);
        items.push(json!({
            "id": r.try_get::<_, String>(0).unwrap_or_default(),
            "name": r.try_get::<_, String>(1).unwrap_or_default(),
            "description": r.try_get::<_, String>(2).unwrap_or_default(),
            "icon": r.try_get::<_, String>(3).unwrap_or_default(),
            "type": "tea",
            "items": tea_items,
            "total_price": r.try_get::<_, f64>(5).unwrap_or(0.0),
            "discount_percent": r.try_get::<_, f64>(6).unwrap_or(0.0),
            "is_available": r.try_get::<_, bool>(7).unwrap_or(false),
            "is_deal_of_day": false,
            "name_en": r.try_get::<_, Option<String>>(8).ok().flatten(),
            "description_en": r.try_get::<_, Option<String>>(9).ok().flatten(),
        }));
    }

    Ok(Json(json!({ "sets": items })))
}

async fn create_set(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<SetRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let id = uuid::Uuid::new_v4().to_string();
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let strain_ids = req.strain_ids.unwrap_or_default();
    let accessory_ids = req.accessory_ids.unwrap_or_default();

    let mut items: Vec<String> = strain_ids;
    items.extend(accessory_ids);

    client.execute(
        "INSERT INTO accessory_sets (id, name, description, icon, accessories, total_price, discount_percent, is_deal_of_day) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        &[&id, &req.name, &req.description.unwrap_or_default(), &req.icon.unwrap_or_default(), &items, &req.total_price, &req.discount_percent.unwrap_or(0.0), &req.is_deal_of_day.unwrap_or(false)],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_set(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(req): Json<SetRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let strain_ids = req.strain_ids.unwrap_or_default();
    let accessory_ids = req.accessory_ids.unwrap_or_default();

    let mut items: Vec<String> = strain_ids;
    items.extend(accessory_ids);

    client.execute(
        "UPDATE accessory_sets SET name=$1, description=$2, icon=$3, accessories=$4, total_price=$5, discount_percent=$6, is_deal_of_day=$7 WHERE id=$8",
        &[&req.name, &req.description.unwrap_or_default(), &req.icon.unwrap_or_default(), &items, &req.total_price, &req.discount_percent.unwrap_or(0.0), &req.is_deal_of_day.unwrap_or(false), &id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_set(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute("UPDATE accessory_sets SET is_available = false WHERE id = $1", &[&id])
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}
