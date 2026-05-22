use axum::{
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    routing::{get, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;
use crate::api::auth::check_admin;
use crate::api::cache::{invalidate_strains, make_etag_header};
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
    // Bilingual EN fields (migration 016, all optional)
    pub name_en: Option<String>,
    pub description_en: Option<String>,
    pub effect_en: Option<String>,
    pub flavor_profile_en: Option<String>,
    pub strain_type_en: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/strains", get(get_strains).post(create_strain))
        .route("/strains/:id", get(get_strain).put(update_strain).delete(delete_strain))
        .route("/strains/:id/availability", put(toggle_availability))
        .route("/strains/strain-of-day", get(get_strains_of_day))
        .route("/strains/:id/strain-of-day", put(set_strain_of_day))
}

async fn get_strains(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<axum::response::Response, StatusCode> {
    // Use a fresh prepare() with a unique statement marker each call to
    // sidestep tokio-postgres' client-side prepared-statement cache, which
    // would otherwise keep returning SQLSTATE 0A000 "cached plan must not
    // change result type" after migration 012's ALTER TABLE ... TYPE.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let include_hidden = q.get("include_hidden").map(|v| v == "1" || v == "true").unwrap_or(false);
    let where_clause = if include_hidden { "" } else { "WHERE is_available = TRUE" };
    let sql = format!(
        "SELECT id, name, category, thc_percent, cbd_percent, effect, flavor_profile, description, price_per_gram, available_grams, image_url, is_available, is_strain_of_day, strain_of_day_discount, name_en, description_en, effect_en, flavor_profile_en, strain_type_en FROM strains {} ORDER BY name -- nonce={}",
        where_clause, nonce
    );
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("get_strains pool error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let rows = client.query(&sql, &[]).await.map_err(|e| {
        tracing::error!("get_strains query error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    tracing::info!("get_strains: returned {} rows", rows.len());

    let strains: Vec<Strain> = rows.iter().map(Strain::from_row).collect();
    let response_data = json!({ "strains": strains });
    let data_json = response_data.to_string();

    // Compute ETag and check conditional request
    let (changed, etag) = state.cache.has_changed("strains", &data_json).await;

    if !changed {
        if let Some(if_none_match) = headers.get("if-none-match") {
            if let Ok(if_none_match_str) = if_none_match.to_str() {
                if if_none_match_str == etag || if_none_match_str == format!("\"{}\"", etag) {
                    let mut response = axum::response::Response::new(axum::body::Body::empty());
                    *response.status_mut() = StatusCode::NOT_MODIFIED;
                    return Ok(response);
                }
            }
        }
    }

    let mut response = axum::response::Response::new(axum::body::Body::from(data_json));
    response.headers_mut().insert("etag", make_etag_header(&etag));
    response.headers_mut().insert("cache-control", HeaderValue::from_static("public, max-age=60"));
    response.headers_mut().insert("content-type", HeaderValue::from_static("application/json"));
    *response.status_mut() = StatusCode::OK;
    Ok(response)
}

async fn get_strain(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = client.query_opt(
        "SELECT id, name, category, thc_percent, cbd_percent, effect, flavor_profile, description, price_per_gram, available_grams, image_url, is_available, is_strain_of_day, strain_of_day_discount, name_en, description_en, effect_en, flavor_profile_en, strain_type_en FROM strains WHERE id = $1",
        &[&id],
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match row {
        Some(r) => Ok(Json(json!({ "strain": Strain::from_row(&r) }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn create_strain(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<CreateStrainRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let id = uuid::Uuid::new_v4().to_string();
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute(
        "INSERT INTO strains (id, name, category, thc_percent, cbd_percent, effect, flavor_profile, description, price_per_gram, available_grams, image_url, is_available, name_en, description_en, effect_en, flavor_profile_en, strain_type_en) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)",
        &[&id, &req.name, &req.category, &req.thc_percent, &req.cbd_percent, &req.effect, &req.flavor_profile, &req.description, &req.price_per_gram, &req.available_grams, &req.image_url, &true, &req.name_en, &req.description_en, &req.effect_en, &req.flavor_profile_en, &req.strain_type_en],
    ).await.map_err(|e| { tracing::error!("create_strain error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    invalidate_strains(&state.cache).await;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_strain(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(req): Json<CreateStrainRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("update_strain pool error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    client.execute(
        "UPDATE strains SET name=$1, category=$2, thc_percent=$3, cbd_percent=$4, effect=$5, flavor_profile=$6, description=$7, price_per_gram=$8, available_grams=$9, image_url=$10, name_en=$11, description_en=$12, effect_en=$13, flavor_profile_en=$14, strain_type_en=$15 WHERE id=$16",
        &[&req.name, &req.category, &req.thc_percent, &req.cbd_percent, &req.effect, &req.flavor_profile, &req.description, &req.price_per_gram, &req.available_grams, &req.image_url, &req.name_en, &req.description_en, &req.effect_en, &req.flavor_profile_en, &req.strain_type_en, &id],
    ).await.map_err(|e| {
        tracing::error!("update_strain SQL error for id={}: {:?}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    invalidate_strains(&state.cache).await;
    Ok(Json(json!({ "success": true })))
}

async fn delete_strain(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("delete_strain pool error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    client.execute("DELETE FROM strains WHERE id = $1", &[&id])
        .await.map_err(|e| { tracing::error!("delete_strain error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    invalidate_strains(&state.cache).await;
    Ok(Json(json!({ "success": true })))
}

async fn toggle_availability(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<Value>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    let available = body["is_available"].as_bool().unwrap_or(true);
    let client = state.db.pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client.execute("UPDATE strains SET is_available = $1 WHERE id = $2", &[&available, &id])
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    invalidate_strains(&state.cache).await;
    Ok(Json(json!({ "success": true })))
}

async fn get_strains_of_day(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let rows = state.db.get_strains_of_day().await.map_err(|e| {
        tracing::error!("get_strains_of_day error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "strains": rows })))
}

async fn set_strain_of_day(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<Value>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Err(code) = check_admin(&headers, &state) {
        return Err((code, Json(json!({ "error": "unauthorized" }))));
    }
    let enabled = body["is_strain_of_day"].as_bool().unwrap_or(true);
    let discount = body["discount"].as_f64().unwrap_or(10.0);
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("SOTD pool error: {:?}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("pool: {}", e) })))
    })?;
    if enabled {
        client.execute("UPDATE strains SET is_strain_of_day = true, strain_of_day_discount = $1, strain_of_day_set_at = NOW() WHERE id = $2", &[&discount, &id]).await.map_err(|e| {
            tracing::error!("SOTD update error for id={}: {:?}", id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("update: {}", e), "id": id })))
        })?;
    } else {
        client.execute("UPDATE strains SET is_strain_of_day = false WHERE id = $1", &[&id])
            .await.map_err(|e| {
                tracing::error!("SOTD disable error for id={}: {:?}", id, e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("disable: {}", e), "id": id })))
            })?;
    }
    invalidate_strains(&state.cache).await;
    Ok(Json(json!({ "success": true, "id": id })))
}

