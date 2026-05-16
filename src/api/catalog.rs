use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post, put},
    Json, Router,
};
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
    })
}

async fn get_accessories(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // NOTE: small cosmetic change forces fresh prepared statement after schema alter
    let rows = client.query(
        "SELECT id, name, category, description, price, stock, image_url, video_url, is_available FROM accessories WHERE is_available = TRUE ORDER BY name",
        &[],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let items: Vec<Value> = rows.iter().map(accessory_row).collect();
    Ok(Json(json!({ "accessories": items })))
}

async fn get_accessory(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = client.query_opt(
        "SELECT id, name, category, description, price, stock, image_url, video_url, is_available FROM accessories WHERE id = $1",
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
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "INSERT INTO accessories (id, name, category, description, price, stock, image_url, video_url) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        &[&id, &req.name, &req.category.unwrap_or_default(), &req.description.unwrap_or_default(), &req.price, &req.stock.unwrap_or(0), &req.image_url.unwrap_or_default(), &req.video_url],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_accessory(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(req): Json<AccessoryRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "UPDATE accessories SET name=$1, category=$2, description=$3, price=$4, stock=$5, image_url=$6, video_url=$7 WHERE id=$8",
        &[&req.name, &req.category.unwrap_or_default(), &req.description.unwrap_or_default(), &req.price, &req.stock.unwrap_or(0), &req.image_url.unwrap_or_default(), &req.video_url, &id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_accessory(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute("UPDATE accessories SET is_available = false WHERE id = $1", &[&id])
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
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
}

async fn get_accessory_sets(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = client.query(
        "SELECT id, name, description, icon, accessories, total_price, discount_percent, is_available, is_deal_of_day FROM accessory_sets WHERE is_available = true ORDER BY name",
        &[],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let items: Vec<Value> = rows.iter().map(|r| {
        // Safe extraction of array column
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
        })
    }).collect();
    Ok(Json(json!({ "accessory_sets": items })))
}

async fn create_accessory_set(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<AccessorySetRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let id = uuid::Uuid::new_v4().to_string();
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let accessories = req.accessories.unwrap_or_default();
    client.execute(
        "INSERT INTO accessory_sets (id, name, description, icon, accessories, total_price, discount_percent, is_deal_of_day) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        &[&id, &req.name, &req.description.unwrap_or_default(), &req.icon.unwrap_or_default(), &accessories, &req.total_price, &req.discount_percent.unwrap_or(0.0), &req.is_deal_of_day.unwrap_or(false)],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_accessory_set(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(req): Json<AccessorySetRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let accessories = req.accessories.unwrap_or_default();
    client.execute(
        "UPDATE accessory_sets SET name=$1, description=$2, icon=$3, accessories=$4, total_price=$5, discount_percent=$6, is_deal_of_day=$7 WHERE id=$8",
        &[&req.name, &req.description.unwrap_or_default(), &req.icon.unwrap_or_default(), &accessories, &req.total_price, &req.discount_percent.unwrap_or(0.0), &req.is_deal_of_day.unwrap_or(false), &id],
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
    })
}

async fn get_tea_products(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = client.query(
        "SELECT id, name, subcategory, description, price, stock, image_url, video_url, is_available FROM tea_products WHERE is_available = true ORDER BY name",
        &[],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let items: Vec<Value> = rows.iter().map(tea_product_row).collect();
    Ok(Json(json!({ "tea_products": items })))
}

async fn get_tea_product(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = client.query_opt(
        "SELECT id, name, subcategory, description, price, stock, image_url, video_url, is_available FROM tea_products WHERE id = $1",
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
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "INSERT INTO tea_products (id, name, subcategory, description, price, stock, image_url, video_url) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        &[&id, &req.name, &req.subcategory.unwrap_or_else(|| "tea".to_string()), &req.description.unwrap_or_default(), &req.price, &req.stock.unwrap_or(0), &req.image_url.unwrap_or_default(), &req.video_url],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_tea_product(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(req): Json<TeaProductRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "UPDATE tea_products SET name=$1, subcategory=$2, description=$3, price=$4, stock=$5, image_url=$6, video_url=$7 WHERE id=$8",
        &[&req.name, &req.subcategory.unwrap_or_else(|| "tea".to_string()), &req.description.unwrap_or_default(), &req.price, &req.stock.unwrap_or(0), &req.image_url.unwrap_or_default(), &req.video_url, &id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_tea_product(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute("UPDATE tea_products SET is_available = false WHERE id = $1", &[&id])
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
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
}

async fn get_tea_sets(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = client.query(
        "SELECT id, name, description, icon, items, total_price, discount_percent, is_available FROM tea_sets WHERE is_available = true ORDER BY name",
        &[],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let items: Vec<Value> = rows.iter().map(|r| {
        // Safe extraction of array column
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
        })
    }).collect();
    Ok(Json(json!({ "tea_sets": items })))
}

async fn create_tea_set(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<TeaSetRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let id = uuid::Uuid::new_v4().to_string();
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let tea_items = req.items.unwrap_or_default();
    client.execute(
        "INSERT INTO tea_sets (id, name, description, icon, items, total_price, discount_percent) VALUES ($1,$2,$3,$4,$5,$6,$7)",
        &[&id, &req.name, &req.description.unwrap_or_default(), &req.icon.unwrap_or_default(), &tea_items, &req.total_price, &req.discount_percent.unwrap_or(0.0)],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_tea_set(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(req): Json<TeaSetRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let tea_items = req.items.unwrap_or_default();
    client.execute(
        "UPDATE tea_sets SET name=$1, description=$2, icon=$3, items=$4, total_price=$5, discount_percent=$6 WHERE id=$7",
        &[&req.name, &req.description.unwrap_or_default(), &req.icon.unwrap_or_default(), &tea_items, &req.total_price, &req.discount_percent.unwrap_or(0.0), &id],
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
        "SELECT id, name, description, icon, accessories, total_price, discount_percent, is_available, is_deal_of_day FROM accessory_sets WHERE is_available = TRUE",
        &[],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get tea sets
    let tea_sets = client.query(
        "SELECT id, name, description, icon, items, total_price, discount_percent, is_available FROM tea_sets WHERE is_available = TRUE",
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
