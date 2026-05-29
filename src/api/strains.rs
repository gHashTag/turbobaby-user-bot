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
    pub video_url: Option<String>,
    #[allow(dead_code)]
    pub is_available: Option<bool>,
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
    let include_hidden = q.get("include_hidden").map(|v| v == "1" || v == "true").unwrap_or(false);
    if include_hidden {
        crate::api::auth::check_admin(&headers, &state)?;
    }
    let sql = if include_hidden {
        "SELECT id, name, category, thc_percent::float8, cbd_percent::float8, effect, flavor_profile, description, price_per_gram::float8, available_grams::float8, image_url, video_url, is_available, is_strain_of_day, strain_of_day_discount::float8, name_en, description_en, effect_en, flavor_profile_en, strain_type_en FROM strains ORDER BY name LIMIT 5000"
    } else {
        "SELECT id, name, category, thc_percent::float8, cbd_percent::float8, effect, flavor_profile, description, price_per_gram::float8, available_grams::float8, image_url, video_url, is_available, is_strain_of_day, strain_of_day_discount::float8, name_en, description_en, effect_en, flavor_profile_en, strain_type_en FROM strains WHERE is_available = TRUE ORDER BY name LIMIT 2000"
    };
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("get_strains pool error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let rows = client.query(sql, &[]).await.map_err(|e| {
        tracing::error!("get_strains query error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    tracing::debug!("get_strains: returned {} rows", rows.len());

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
    if id.len() > 200 { return Err(StatusCode::BAD_REQUEST); }
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let row = client.query_opt(
        "SELECT id, name, category, thc_percent::float8, cbd_percent::float8, effect, flavor_profile, description, price_per_gram::float8, available_grams::float8, image_url, video_url, is_available, is_strain_of_day, strain_of_day_discount::float8, name_en, description_en, effect_en, flavor_profile_en, strain_type_en FROM strains WHERE id = $1 AND is_available = TRUE",
        &[&id],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    match row {
        Some(r) => Ok(Json(json!({ "strain": Strain::from_row(&r) }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn create_strain(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<CreateStrainRequest>) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    if req.name.len() > 200 { return Err(StatusCode::BAD_REQUEST); }
    if let Some(ref c) = req.category { if c.len() > 200 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref e) = req.effect { if e.len() > 1000 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref f) = req.flavor_profile { if f.len() > 1000 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref d) = req.description { if d.len() > 1000 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref n) = req.name_en { if n.len() > 200 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref d) = req.description_en { if d.len() > 1000 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref e) = req.effect_en { if e.len() > 1000 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref f) = req.flavor_profile_en { if f.len() > 1000 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref t) = req.strain_type_en { if t.len() > 200 { return Err(StatusCode::BAD_REQUEST); } }
    crate::api::validate_url(&req.image_url)?;
    crate::api::validate_url(&req.video_url)?;
    if !req.price_per_gram.is_finite() || req.price_per_gram < 0.0 || req.price_per_gram > 1_000_000.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(t) = req.thc_percent { if !t.is_finite() || t < 0.0 || t > 100.0 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(c) = req.cbd_percent { if !c.is_finite() || c < 0.0 || c > 100.0 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(a) = req.available_grams { if !a.is_finite() || a < 0.0 || a > 1_000_000.0 { return Err(StatusCode::BAD_REQUEST); } }
    let id = uuid::Uuid::new_v4().to_string();
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    client.execute(
        "INSERT INTO strains (id, name, category, thc_percent, cbd_percent, effect, flavor_profile, description, price_per_gram, available_grams, image_url, video_url, is_available, name_en, description_en, effect_en, flavor_profile_en, strain_type_en) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)",
        &[&id, &req.name, &req.category, &req.thc_percent, &req.cbd_percent, &req.effect, &req.flavor_profile, &req.description, &req.price_per_gram, &req.available_grams, &req.image_url, &req.video_url, &req.is_available.unwrap_or(true), &req.name_en, &req.description_en, &req.effect_en, &req.flavor_profile_en, &req.strain_type_en],
    ).await.map_err(|e| { tracing::error!("create_strain error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    invalidate_strains(&state.cache).await;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_strain(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(req): Json<CreateStrainRequest>) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 { return Err(StatusCode::BAD_REQUEST); }
    check_admin(&headers, &state)?;
    if req.name.len() > 200 { return Err(StatusCode::BAD_REQUEST); }
    if let Some(ref c) = req.category { if c.len() > 200 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref e) = req.effect { if e.len() > 1000 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref f) = req.flavor_profile { if f.len() > 1000 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref d) = req.description { if d.len() > 1000 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref n) = req.name_en { if n.len() > 200 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref d) = req.description_en { if d.len() > 1000 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref e) = req.effect_en { if e.len() > 1000 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref f) = req.flavor_profile_en { if f.len() > 1000 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(ref t) = req.strain_type_en { if t.len() > 200 { return Err(StatusCode::BAD_REQUEST); } }
    crate::api::validate_url(&req.image_url)?;
    crate::api::validate_url(&req.video_url)?;
    if !req.price_per_gram.is_finite() || req.price_per_gram < 0.0 || req.price_per_gram > 1_000_000.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(t) = req.thc_percent { if !t.is_finite() || t < 0.0 || t > 100.0 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(c) = req.cbd_percent { if !c.is_finite() || c < 0.0 || c > 100.0 { return Err(StatusCode::BAD_REQUEST); } }
    if let Some(a) = req.available_grams { if !a.is_finite() || a < 0.0 || a > 1_000_000.0 { return Err(StatusCode::BAD_REQUEST); } }
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("update_strain pool error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let rows = client.execute(
        "UPDATE strains SET name=$1, category=$2, thc_percent=$3, cbd_percent=$4, effect=$5, flavor_profile=$6, description=$7, price_per_gram=$8, available_grams=$9, image_url=$10, video_url=$11, name_en=$12, description_en=$13, effect_en=$14, flavor_profile_en=$15, strain_type_en=$16, is_available=$17 WHERE id=$18",
        &[&req.name, &req.category, &req.thc_percent, &req.cbd_percent, &req.effect, &req.flavor_profile, &req.description, &req.price_per_gram, &req.available_grams, &req.image_url, &req.video_url, &req.name_en, &req.description_en, &req.effect_en, &req.flavor_profile_en, &req.strain_type_en, &req.is_available.unwrap_or(true), &id],
    ).await.map_err(|e| {
        tracing::error!("update_strain SQL error for id={}: {:?}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    if rows == 0 {
        tracing::warn!("update_strain: no rows affected for id={}", id);
        return Err(StatusCode::NOT_FOUND);
    }
    invalidate_strains(&state.cache).await;
    tracing::info!("update_strain: id={} updated successfully", id);
    Ok(Json(json!({ "success": true })))
}

async fn delete_strain(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 { return Err(StatusCode::BAD_REQUEST); }
    check_admin(&headers, &state)?;
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("delete_strain pool error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    client.execute("DELETE FROM strains WHERE id = $1", &[&id])
        .await.map_err(|e| { tracing::error!("delete_strain error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    invalidate_strains(&state.cache).await;
    Ok(Json(json!({ "success": true })))
}

async fn toggle_availability(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<Value>) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 { return Err(StatusCode::BAD_REQUEST); }
    check_admin(&headers, &state)?;
    let available = body["is_available"].as_bool().unwrap_or(true);
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    client.execute("UPDATE strains SET is_available = $1 WHERE id = $2", &[&available, &id])
        .await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
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
    if id.len() > 200 { return Err((StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid id" })))); }
    if let Err(code) = check_admin(&headers, &state) {
        return Err((code, Json(json!({ "error": "unauthorized" }))));
    }
    let enabled = body["is_strain_of_day"].as_bool().unwrap_or(true);
    let discount = body["discount"].as_f64().unwrap_or(10.0);
    if !discount.is_finite() || discount < 0.0 || discount > 100.0 {
        return Err((StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid discount" }))));
    }
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("SOTD pool error: {:?}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "database error" })))
    })?;
    if enabled {
        client.execute("UPDATE strains SET is_strain_of_day = true, strain_of_day_discount = $1, strain_of_day_set_at = NOW() WHERE id = $2", &[&discount, &id]).await.map_err(|e| {
            tracing::error!("SOTD update error for id={}: {:?}", id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "update failed" })))
        })?;
    } else {
        client.execute("UPDATE strains SET is_strain_of_day = false WHERE id = $1", &[&id])
            .await.map_err(|e| {
                tracing::error!("SOTD disable error for id={}: {:?}", id, e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "disable failed" })))
            })?;
    }
    invalidate_strains(&state.cache).await;
    Ok(Json(json!({ "success": true, "id": id })))
}

