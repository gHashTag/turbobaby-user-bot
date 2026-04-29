use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::AppState;
use crate::db::strains::Strain;

#[derive(Debug, Deserialize)]
pub struct CreateStrainRequest {
    pub name: String,
    pub category: Option<String>,
    pub thc_percent: Option<f64>,
    pub cbd_percent: Option<f64>,
    pub effect: Option<String>,
    pub flavor_profile: Option<String>,
    pub description: Option<String>,
    pub price_per_gram: f64,
    pub available_grams: Option<f64>,
    pub image_url: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/strains", get(get_strains))
        .route("/strains", post(create_strain))
        .route("/strains/:id", get(get_strain))
        .route("/strains/:id", put(update_strain))
        .route("/strains/:id", delete(delete_strain))
        .route("/strains/:id/availability", put(toggle_availability))
        .route("/strains/strain-of-day", get(get_strains_of_day))
        .route("/strains/:id/strain-of-day", put(set_strain_of_day))
}

async fn get_strains(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let rows = sqlx::query_as!(Strain,
        "SELECT id, name, category, thc_percent, cbd_percent, effect, flavor_profile, description,
                price_per_gram, available_grams, image_url, is_available,
                is_strain_of_day, strain_of_day_discount FROM strains
         WHERE is_available = true ORDER BY name"
    )
    .fetch_all(&state.db.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "strains": rows })))
}

async fn get_strain(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let row = sqlx::query_as!(Strain,
        "SELECT id, name, category, thc_percent, cbd_percent, effect, flavor_profile, description,
                price_per_gram, available_grams, image_url, is_available,
                is_strain_of_day, strain_of_day_discount FROM strains WHERE id = $1",
        id
    )
    .fetch_optional(&state.db.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(json!({ "strain": row })))
}

async fn create_strain(
    State(state): State<AppState>,
    Json(req): Json<CreateStrainRequest>,
) -> Result<Json<Value>, StatusCode> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query!(
        "INSERT INTO strains (id, name, category, thc_percent, cbd_percent, effect, flavor_profile, description, price_per_gram, available_grams, image_url)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        id, req.name, req.category, req.thc_percent, req.cbd_percent, req.effect,
        req.flavor_profile, req.description, req.price_per_gram, req.available_grams, req.image_url
    )
    .execute(&state.db.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_strain(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CreateStrainRequest>,
) -> Result<Json<Value>, StatusCode> {
    sqlx::query!(
        "UPDATE strains SET name=$1, category=$2, thc_percent=$3, cbd_percent=$4, effect=$5,
         flavor_profile=$6, description=$7, price_per_gram=$8, available_grams=$9, image_url=$10
         WHERE id=$11",
        req.name, req.category, req.thc_percent, req.cbd_percent, req.effect,
        req.flavor_profile, req.description, req.price_per_gram, req.available_grams, req.image_url, id
    )
    .execute(&state.db.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_strain(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    sqlx::query!("UPDATE strains SET is_available = false WHERE id = $1", id)
        .execute(&state.db.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn toggle_availability(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let available = body["is_available"].as_bool().unwrap_or(true);
    sqlx::query!("UPDATE strains SET is_available = $1 WHERE id = $2", available, id)
        .execute(&state.db.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "success": true })))
}

async fn get_strains_of_day(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let rows = state.db.get_strains_of_day().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "strains": rows })))
}

async fn set_strain_of_day(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let enabled = body["is_strain_of_day"].as_bool().unwrap_or(true);
    let discount = body["discount"].as_f64().unwrap_or(10.0);
    if enabled {
        sqlx::query!(
            "UPDATE strains SET is_strain_of_day = true, strain_of_day_discount = $1, strain_of_day_set_at = NOW() WHERE id = $2",
            discount, id
        )
        .execute(&state.db.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    } else {
        sqlx::query!("UPDATE strains SET is_strain_of_day = false WHERE id = $1", id)
            .execute(&state.db.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    Ok(Json(json!({ "success": true })))
}
