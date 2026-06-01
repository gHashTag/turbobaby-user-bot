use axum::{
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    routing::{get, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::auth::check_admin;
use crate::api::cache::{invalidate_strains, make_etag_header};
use crate::db::strains::Strain;
use crate::AppState;

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
    // Marketing flags (migration 028, TZ #2). All optional so older clients
    // that don't set them keep working — server defaults each to 0/false/null.
    #[serde(default)]
    pub discount_percent: Option<f64>,
    #[serde(default)]
    pub sale_price: Option<f64>,
    #[serde(default)]
    pub sale_active: Option<bool>,
    /// RFC3339 timestamp string, or null/None to clear.
    #[serde(default)]
    pub sale_until: Option<String>,
    #[serde(default)]
    pub is_best_seller: Option<bool>,
    #[serde(default)]
    pub is_new_arrival: Option<bool>,
    #[serde(default)]
    pub new_until: Option<String>,
    #[serde(default)]
    pub display_order: Option<i32>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/strains", get(get_strains).post(create_strain))
        .route(
            "/strains/:id",
            get(get_strain).put(update_strain).delete(delete_strain),
        )
        .route("/strains/:id/availability", put(toggle_availability))
        .route("/strains/strain-of-day", get(get_strains_of_day))
        .route("/strains/:id/strain-of-day", put(set_strain_of_day))
}

async fn get_strains(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<axum::response::Response, StatusCode> {
    // Cycle #80: migrated from raw tokio_postgres to SeaORM. The previous
    // version had a defensive comment about a fresh prepare() per call to
    // dodge the SQLSTATE 0A000 stale-plan issue from migration 012's
    // ALTER TYPE; sqlx (which SeaORM uses) handles statement caching
    // differently and is not subject to that bug, so the workaround is
    // gone.
    let include_hidden = q
        .get("include_hidden")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);
    if include_hidden {
        crate::api::auth::check_admin(&headers, &state)?;
    }

    // TZ #2 priority ordering:
    //   group 1: Strain of the Day
    //   group 2: New arrivals still within `new_until`
    //   group 3: Best sellers
    //   group 4: Active sales still within `sale_until`
    //   group 5: everything else
    // Within a group: display_order ASC, then name ASC.
    use sea_orm::{ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect};
    let priority = sea_orm::sea_query::Expr::cust(
        "CASE \
         WHEN is_strain_of_day = TRUE THEN 1 \
         WHEN is_new_arrival = TRUE AND (new_until IS NULL OR new_until > NOW()) THEN 2 \
         WHEN is_best_seller = TRUE THEN 3 \
         WHEN sale_active = TRUE AND (sale_until IS NULL OR sale_until > NOW()) THEN 4 \
         ELSE 5 \
         END",
    );
    use crate::db::entities::strain::{Column as StrainCol, Entity as StrainEntity};
    let mut query = StrainEntity::find();
    if !include_hidden {
        query = query.filter(StrainCol::IsAvailable.eq(true));
    }
    let limit = if include_hidden { 5000 } else { 2000 };
    let models = query
        .order_by(priority, Order::Asc)
        .order_by(StrainCol::DisplayOrder, Order::Asc)
        .order_by(StrainCol::Name, Order::Asc)
        .limit(limit)
        .all(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("get_strains SeaORM error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    tracing::debug!("get_strains: returned {} rows", models.len());

    let strains: Vec<Strain> = models.into_iter().map(Strain::from).collect();
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
    response
        .headers_mut()
        .insert("etag", make_etag_header(&etag));
    response.headers_mut().insert(
        "cache-control",
        HeaderValue::from_static("public, max-age=60"),
    );
    response
        .headers_mut()
        .insert("content-type", HeaderValue::from_static("application/json"));
    *response.status_mut() = StatusCode::OK;
    Ok(response)
}

async fn get_strain(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Cycle #80: SeaORM migration. The raw SQL `WHERE id = $1 AND
    // is_available = TRUE` becomes `find_by_id + filter`. find_by_id
    // returns None if either the row is missing or it's hidden — either
    // way the API contract is 404, so we don't need to differentiate.
    use crate::db::entities::strain::{Column as StrainCol, Entity as StrainEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let model = StrainEntity::find_by_id(id.clone())
        .filter(StrainCol::IsAvailable.eq(true))
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("get_strain SeaORM error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    match model {
        Some(m) => Ok(Json(json!({ "strain": Strain::from(m) }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

fn validate_strain_request(req: &CreateStrainRequest) -> Result<(), StatusCode> {
    if req.name.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(ref c) = req.category {
        if c.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref e) = req.effect {
        if e.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref f) = req.flavor_profile {
        if f.len() > 1000 {
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
    if let Some(ref e) = req.effect_en {
        if e.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref f) = req.flavor_profile_en {
        if f.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref t) = req.strain_type_en {
        if t.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    crate::api::validate_url(&req.image_url)?;
    crate::api::validate_url(&req.video_url)?;
    if !req.price_per_gram.is_finite()
        || req.price_per_gram < 0.0
        || req.price_per_gram > 1_000_000.0
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(t) = req.thc_percent {
        if !t.is_finite() || !(0.0..=100.0).contains(&t) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(c) = req.cbd_percent {
        if !c.is_finite() || !(0.0..=100.0).contains(&c) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(a) = req.available_grams {
        if !a.is_finite() || !(0.0..=1_000_000.0).contains(&a) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    Ok(())
}

/// Parse RFC3339 timestamp string → `Option<chrono::DateTime<chrono::Utc>>`.
/// Empty or missing → None; malformed → Err(400 BAD_REQUEST). Used by the
/// admin TZ #2 marketing UI to set sale/new arrival expiry windows.
fn parse_marketing_until(
    s: &Option<String>,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, StatusCode> {
    match s.as_deref() {
        None | Some("") => Ok(None),
        Some(raw) => chrono::DateTime::parse_from_rfc3339(raw)
            .map(|dt| Some(dt.with_timezone(&chrono::Utc)))
            .map_err(|_| StatusCode::BAD_REQUEST),
    }
}

async fn create_strain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateStrainRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_strain_request(&req)?;
    let id = uuid::Uuid::new_v4().to_string();
    let sale_until = parse_marketing_until(&req.sale_until)?;
    let new_until = parse_marketing_until(&req.new_until)?;
    let discount_percent = req.discount_percent.unwrap_or(0.0);
    let sale_active = req.sale_active.unwrap_or(false);
    let is_best_seller = req.is_best_seller.unwrap_or(false);
    let is_new_arrival = req.is_new_arrival.unwrap_or(false);
    let display_order = req.display_order.unwrap_or(0);
    // Cycle #81: SeaORM insert via ActiveModel. The fields *not* Set here
    // (`created_at`, `is_strain_of_day`, `strain_of_day_discount`,
    // `strain_of_day_set_at`) get the DB DEFAULT — same as the raw SQL
    // which only listed the 26 editable columns and let the DB fill the
    // SOTD audit-only columns.
    use crate::db::entities::strain::{ActiveModel, Entity as StrainEntity};
    use sea_orm::{ActiveValue::Set, EntityTrait};
    let model = ActiveModel {
        id: Set(id.clone()),
        name: Set(req.name.clone()),
        category: Set(req.category.clone()),
        thc_percent: Set(req.thc_percent),
        cbd_percent: Set(req.cbd_percent),
        effect: Set(req.effect.clone()),
        flavor_profile: Set(req.flavor_profile.clone()),
        description: Set(req.description.clone()),
        price_per_gram: Set(req.price_per_gram),
        available_grams: Set(req.available_grams),
        image_url: Set(req.image_url.clone()),
        video_url: Set(req.video_url.clone()),
        is_available: Set(req.is_available.unwrap_or(true)),
        name_en: Set(req.name_en.clone()),
        description_en: Set(req.description_en.clone()),
        effect_en: Set(req.effect_en.clone()),
        flavor_profile_en: Set(req.flavor_profile_en.clone()),
        strain_type_en: Set(req.strain_type_en.clone()),
        discount_percent: Set(discount_percent),
        sale_price: Set(req.sale_price),
        sale_active: Set(sale_active),
        sale_until: Set(sale_until.map(|t| t.into())),
        is_best_seller: Set(is_best_seller),
        is_new_arrival: Set(is_new_arrival),
        new_until: Set(new_until.map(|t| t.into())),
        display_order: Set(display_order),
        ..Default::default()
    };
    StrainEntity::insert(model)
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("create_strain SeaORM error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    invalidate_strains(&state.cache).await;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_strain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<CreateStrainRequest>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    validate_strain_request(&req)?;
    let sale_until = parse_marketing_until(&req.sale_until)?;
    let new_until = parse_marketing_until(&req.new_until)?;
    let discount_percent = req.discount_percent.unwrap_or(0.0);
    let sale_active = req.sale_active.unwrap_or(false);
    let is_best_seller = req.is_best_seller.unwrap_or(false);
    let is_new_arrival = req.is_new_arrival.unwrap_or(false);
    let display_order = req.display_order.unwrap_or(0);
    // Cycle #81: SeaORM update via ActiveModel. Set every editable column
    // explicitly — unset (Default) columns are skipped, but the request
    // contract is "PUT replaces every field", so we Set them all. Convert
    // `Option<DateTime<Utc>>` to `Option<DateTimeWithTimeZone>` via `Into`.
    use crate::db::entities::strain::{ActiveModel, Column as StrainCol, Entity as StrainEntity};
    use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
    let model = ActiveModel {
        name: Set(req.name.clone()),
        category: Set(req.category.clone()),
        thc_percent: Set(req.thc_percent),
        cbd_percent: Set(req.cbd_percent),
        effect: Set(req.effect.clone()),
        flavor_profile: Set(req.flavor_profile.clone()),
        description: Set(req.description.clone()),
        price_per_gram: Set(req.price_per_gram),
        available_grams: Set(req.available_grams),
        image_url: Set(req.image_url.clone()),
        video_url: Set(req.video_url.clone()),
        name_en: Set(req.name_en.clone()),
        description_en: Set(req.description_en.clone()),
        effect_en: Set(req.effect_en.clone()),
        flavor_profile_en: Set(req.flavor_profile_en.clone()),
        strain_type_en: Set(req.strain_type_en.clone()),
        is_available: Set(req.is_available.unwrap_or(true)),
        discount_percent: Set(discount_percent),
        sale_price: Set(req.sale_price),
        sale_active: Set(sale_active),
        sale_until: Set(sale_until.map(|t| t.into())),
        is_best_seller: Set(is_best_seller),
        is_new_arrival: Set(is_new_arrival),
        new_until: Set(new_until.map(|t| t.into())),
        display_order: Set(display_order),
        ..Default::default()
    };
    let result = StrainEntity::update_many()
        .set(model)
        .filter(StrainCol::Id.eq(id.clone()))
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("update_strain SeaORM error for id={}: {:?}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected == 0 {
        tracing::warn!("update_strain: no rows affected for id={}", id);
        return Err(StatusCode::NOT_FOUND);
    }
    invalidate_strains(&state.cache).await;
    tracing::info!("update_strain: id={} updated successfully", id);
    Ok(Json(json!({ "success": true })))
}

async fn delete_strain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    // Cycle #81: SeaORM. `delete_by_id` returns a `DeleteResult` whose
    // `rows_affected` we deliberately ignore — the raw SQL was the same
    // (no NOT_FOUND for a missing row on DELETE).
    use crate::db::entities::strain::Entity as StrainEntity;
    use sea_orm::EntityTrait;
    StrainEntity::delete_by_id(id)
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("delete_strain SeaORM error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    invalidate_strains(&state.cache).await;
    Ok(Json(json!({ "success": true })))
}

async fn toggle_availability(
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
    // Cycle #81: SeaORM update_many. No-row tolerant (matches raw SQL).
    use crate::db::entities::strain::{Column as StrainCol, Entity as StrainEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    StrainEntity::update_many()
        .col_expr(
            StrainCol::IsAvailable,
            sea_orm::sea_query::Expr::value(available),
        )
        .filter(StrainCol::Id.eq(id))
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("toggle_availability SeaORM error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
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

async fn set_strain_of_day(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if id.len() > 200 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid id" })),
        ));
    }
    if let Err(code) = check_admin(&headers, &state) {
        return Err((code, Json(json!({ "error": "unauthorized" }))));
    }
    let enabled = crate::api::extract_bool(&body, "is_strain_of_day")
        .map_err(|e| (e, Json(json!({ "error": "invalid is_strain_of_day" }))))?;
    let discount = crate::api::extract_discount(&body)
        .map_err(|e| (e, Json(json!({ "error": "invalid discount" }))))?;
    // Cycle #81: SeaORM. Two branches:
    //   enable  — set is_strain_of_day=true, discount, set_at=NOW()
    //   disable — set is_strain_of_day=false (leave discount/set_at as audit history)
    use crate::db::entities::strain::{Column as StrainCol, Entity as StrainEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let result = if enabled {
        StrainEntity::update_many()
            .col_expr(
                StrainCol::IsStrainOfDay,
                sea_orm::sea_query::Expr::value(true),
            )
            .col_expr(
                StrainCol::StrainOfDayDiscount,
                sea_orm::sea_query::Expr::value(discount),
            )
            .col_expr(
                StrainCol::StrainOfDaySetAt,
                sea_orm::sea_query::Expr::cust("NOW()"),
            )
            .filter(StrainCol::Id.eq(id.clone()))
            .exec(&state.db.orm)
            .await
    } else {
        StrainEntity::update_many()
            .col_expr(
                StrainCol::IsStrainOfDay,
                sea_orm::sea_query::Expr::value(false),
            )
            .filter(StrainCol::Id.eq(id.clone()))
            .exec(&state.db.orm)
            .await
    };
    result.map_err(|e| {
        tracing::error!("SOTD SeaORM error for id={}: {:?}", id, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "update failed" })),
        )
    })?;
    invalidate_strains(&state.cache).await;
    Ok(Json(json!({ "success": true, "id": id })))
}

#[cfg(test)]
mod tests {
    use super::{validate_strain_request, CreateStrainRequest};
    use crate::api::{extract_bool, extract_discount};
    use axum::http::StatusCode;

    fn valid_strain() -> CreateStrainRequest {
        CreateStrainRequest {
            name: "OG Kush".into(),
            category: Some("Indica".into()),
            thc_percent: Some(20.0),
            cbd_percent: Some(0.5),
            effect: Some("Relax".into()),
            flavor_profile: Some("Earthy".into()),
            description: Some("Classic.".into()),
            price_per_gram: 100.0,
            available_grams: Some(50.0),
            image_url: Some("/uploads/kush.jpg".into()),
            video_url: None,
            is_available: Some(true),
            name_en: Some("OG Kush".into()),
            description_en: Some("Classic.".into()),
            effect_en: Some("Relax".into()),
            flavor_profile_en: Some("Earthy".into()),
            strain_type_en: Some("Indica".into()),
            // TZ #2 marketing fields default to None — tests check the
            // request validator, not the marketing precedence (that lives in
            // trios::pricing with its own table-driven tests).
            discount_percent: None,
            sale_price: None,
            sale_active: None,
            sale_until: None,
            is_best_seller: None,
            is_new_arrival: None,
            new_until: None,
            display_order: None,
        }
    }

    #[test]
    fn test_validate_strain_ok() {
        assert!(validate_strain_request(&valid_strain()).is_ok());
    }

    #[test]
    fn test_validate_strain_name_too_long() {
        let mut req = valid_strain();
        req.name = "a".repeat(201);
        assert_eq!(
            validate_strain_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_strain_price_negative() {
        let mut req = valid_strain();
        req.price_per_gram = -1.0;
        assert_eq!(
            validate_strain_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_strain_thc_out_of_range() {
        let mut req = valid_strain();
        req.thc_percent = Some(101.0);
        assert_eq!(
            validate_strain_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_strain_thc_nan() {
        let mut req = valid_strain();
        req.thc_percent = Some(f64::NAN);
        assert_eq!(
            validate_strain_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_strain_bad_image_url() {
        let mut req = valid_strain();
        req.image_url = Some("javascript:alert(1)".into());
        assert_eq!(
            validate_strain_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_strain_available_grams_too_high() {
        let mut req = valid_strain();
        req.available_grams = Some(2_000_000.0);
        assert_eq!(
            validate_strain_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_bool_true() {
        let body = serde_json::json!({"is_available": true});
        assert!(extract_bool(&body, "is_available").unwrap());
    }

    #[test]
    fn test_extract_bool_false() {
        let body = serde_json::json!({"is_available": false});
        assert!(!extract_bool(&body, "is_available").unwrap());
    }

    #[test]
    fn test_extract_bool_missing() {
        let body = serde_json::json!({});
        assert_eq!(
            extract_bool(&body, "is_available").unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_bool_not_bool() {
        let body = serde_json::json!({"is_available": "true"});
        assert_eq!(
            extract_bool(&body, "is_available").unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_discount_ok() {
        let body = serde_json::json!({"discount": 15.0});
        assert_eq!(extract_discount(&body).unwrap(), 15.0);
    }

    #[test]
    fn test_extract_discount_missing() {
        let body = serde_json::json!({});
        assert_eq!(
            extract_discount(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_discount_not_number() {
        let body = serde_json::json!({"discount": "ten"});
        assert_eq!(
            extract_discount(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_discount_negative() {
        let body = serde_json::json!({"discount": -1.0});
        assert_eq!(
            extract_discount(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_discount_too_high() {
        let body = serde_json::json!({"discount": 101.0});
        assert_eq!(
            extract_discount(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_discount_nan() {
        let body = serde_json::json!({"discount": f64::NAN});
        assert_eq!(
            extract_discount(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }
}
