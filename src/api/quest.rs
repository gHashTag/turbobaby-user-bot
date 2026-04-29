use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        // Quest Places
        .route("/quest-places", get(get_quest_places))
        .route("/quest-places", post(create_quest_place))
        .route("/quest-places/:id", put(update_quest_place))
        .route("/quest-places/:id", delete(delete_quest_place))
        // Treasure Hunts
        .route("/treasure-hunts", get(get_treasure_hunts))
        .route("/treasure-hunts", post(create_treasure_hunt))
        .route("/treasure-hunts/:id", put(update_treasure_hunt))
        .route("/treasure-hunts/:id", delete(delete_treasure_hunt))
        // Location Quest
        .route("/quest/locations", get(get_quest_locations))
        .route("/quest/locations", post(create_quest_location))
        .route("/quest/locations/:id", put(update_quest_location))
        .route("/quest/scan", post(scan_quest_qr))
}

// ── Quest Places ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct QuestPlaceRequest {
    pub name: String,
    pub category: Option<String>,
    pub lat: f64,
    pub lon: f64,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub is_available: Option<bool>,
}

async fn get_quest_places(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = client.query(
        "SELECT id, name, category, lat, lon, description, image_url, is_available FROM quest_places WHERE is_available = true ORDER BY name",
        &[],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let items: Vec<Value> = rows.iter().map(|r| json!({
        "id": r.try_get::<_, String>(0).unwrap_or_default(),
        "name": r.try_get::<_, String>(1).unwrap_or_default(),
        "category": r.try_get::<_, String>(2).unwrap_or_default(),
        "lat": r.try_get::<_, f64>(3).unwrap_or(0.0),
        "lon": r.try_get::<_, f64>(4).unwrap_or(0.0),
        "description": r.try_get::<_, String>(5).ok(),
        "image_url": r.try_get::<_, String>(6).ok(),
        "is_available": r.try_get::<_, bool>(7).unwrap_or(false),
    })).collect();
    Ok(Json(json!({ "quest_places": items })))
}

async fn create_quest_place(State(state): State<AppState>, Json(req): Json<QuestPlaceRequest>) -> Result<Json<Value>, StatusCode> {
    let id = uuid::Uuid::new_v4().to_string();
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "INSERT INTO quest_places (id, name, category, lat, lon, description, image_url) VALUES ($1,$2,$3,$4,$5,$6,$7)",
        &[&id, &req.name, &req.category.unwrap_or_else(|| "location".to_string()), &req.lat, &req.lon, &req.description, &req.image_url],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_quest_place(State(state): State<AppState>, Path(id): Path<String>, Json(req): Json<QuestPlaceRequest>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "UPDATE quest_places SET name=$1, category=$2, lat=$3, lon=$4, description=$5, image_url=$6 WHERE id=$7",
        &[&req.name, &req.category.unwrap_or_else(|| "location".to_string()), &req.lat, &req.lon, &req.description, &req.image_url, &id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_quest_place(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute("UPDATE quest_places SET is_available = false WHERE id = $1", &[&id])
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

// ── Treasure Hunts ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TreasureHuntRequest {
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub black_mark_title: String,
    pub black_mark_description: Option<String>,
    pub black_mark_image_url: Option<String>,
    pub is_active: Option<bool>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub start_lat: f64,
    pub start_lon: f64,
    pub start_name: String,
}

async fn get_treasure_hunts(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = client.query(
        "SELECT id, name, description, image_url, black_mark_title, black_mark_description, black_mark_image_url, is_active, starts_at, ends_at, start_lat, start_lon, start_name FROM treasure_hunts WHERE is_active = true ORDER BY created_at DESC",
        &[],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let items: Vec<Value> = rows.iter().map(|r| json!({
        "id": r.try_get::<_, String>(0).unwrap_or_default(),
        "name": r.try_get::<_, String>(1).unwrap_or_default(),
        "description": r.try_get::<_, String>(2).ok(),
        "image_url": r.try_get::<_, String>(3).ok(),
        "black_mark_title": r.try_get::<_, String>(4).unwrap_or_default(),
        "black_mark_description": r.try_get::<_, String>(5).ok(),
        "black_mark_image_url": r.try_get::<_, String>(6).ok(),
        "is_active": r.try_get::<_, bool>(7).unwrap_or(false),
        "starts_at": r.try_get::<_, String>(8).ok(),
        "ends_at": r.try_get::<_, String>(9).ok(),
        "start_lat": r.try_get::<_, f64>(10).unwrap_or(0.0),
        "start_lon": r.try_get::<_, f64>(11).unwrap_or(0.0),
        "start_name": r.try_get::<_, String>(12).unwrap_or_default(),
    })).collect();
    Ok(Json(json!({ "treasure_hunts": items })))
}

async fn create_treasure_hunt(State(state): State<AppState>, Json(req): Json<TreasureHuntRequest>) -> Result<Json<Value>, StatusCode> {
    let id = uuid::Uuid::new_v4().to_string();
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "INSERT INTO treasure_hunts (id, name, description, image_url, black_mark_title, black_mark_description, black_mark_image_url, start_lat, start_lon, start_name) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        &[&id, &req.name, &req.description, &req.image_url, &req.black_mark_title, &req.black_mark_description, &req.black_mark_image_url, &req.start_lat, &req.start_lon, &req.start_name],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_treasure_hunt(State(state): State<AppState>, Path(id): Path<String>, Json(req): Json<TreasureHuntRequest>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "UPDATE treasure_hunts SET name=$1, description=$2, image_url=$3, black_mark_title=$4, black_mark_description=$5, black_mark_image_url=$6, start_lat=$7, start_lon=$8, start_name=$9 WHERE id=$10",
        &[&req.name, &req.description, &req.image_url, &req.black_mark_title, &req.black_mark_description, &req.black_mark_image_url, &req.start_lat, &req.start_lon, &req.start_name, &id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_treasure_hunt(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute("UPDATE treasure_hunts SET is_active = false WHERE id = $1", &[&id])
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

// ── Location Quest Locations ──────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct QuestLocationParams {
    pub telegram_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct QuestLocationRequest {
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub map_url: Option<String>,
    pub is_active: Option<bool>,
    pub is_final: Option<bool>,
}

async fn get_quest_locations(
    State(state): State<AppState>,
    Query(_params): Query<QuestLocationParams>,
) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = client.query(
        "SELECT id, name, description, category, map_url, qr_token, is_active, is_final FROM location_quest_locations WHERE is_active = true ORDER BY id",
        &[],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let items: Vec<Value> = rows.iter().map(|r| json!({
        "id": r.get::<_, i32>(0),
        "name": r.get::<_, String>(1),
        "description": r.get::<_, String>(2),
        "category": r.get::<_, String>(3),
        "map_url": r.get::<_, String>(4),
        "qr_token": r.get::<_, String>(5),
        "is_active": r.get::<_, bool>(6),
        "is_final": r.get::<_, bool>(7),
    })).collect();
    Ok(Json(json!({ "locations": items })))
}

async fn create_quest_location(State(state): State<AppState>, Json(req): Json<QuestLocationRequest>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = client.query_one(
        "INSERT INTO location_quest_locations (name, description, category, map_url, is_final) VALUES ($1,$2,$3,$4,$5) RETURNING id",
        &[&req.name, &req.description.unwrap_or_default(), &req.category.unwrap_or_else(|| "location".to_string()), &req.map_url.unwrap_or_default(), &req.is_final.unwrap_or(false)],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true, "id": row.get::<_, i32>(0) })))
}

async fn update_quest_location(State(state): State<AppState>, Path(id): Path<i32>, Json(req): Json<QuestLocationRequest>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "UPDATE location_quest_locations SET name=$1, description=$2, category=$3, map_url=$4, is_final=$5 WHERE id=$6",
        &[&req.name, &req.description.unwrap_or_default(), &req.category.unwrap_or_else(|| "location".to_string()), &req.map_url.unwrap_or_default(), &req.is_final.unwrap_or(false), &id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn scan_quest_qr(State(state): State<AppState>, Json(body): Json<Value>) -> Result<Json<Value>, StatusCode> {
    let qr_token = body["qr_token"].as_str().unwrap_or("");
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = client.query_opt(
        "SELECT id, name, is_final FROM location_quest_locations WHERE qr_token = $1 AND is_active = true",
        &[&qr_token],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match row {
        Some(r) => Ok(Json(json!({
            "success": true,
            "location": {
                "id": r.get::<_, i32>(0),
                "name": r.get::<_, String>(1),
                "is_final": r.get::<_, bool>(2),
            }
        }))),
        None => Ok(Json(json!({ "success": false, "error": "Invalid QR token" }))),
    }
}
