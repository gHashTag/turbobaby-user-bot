use axum::{
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::auth::check_admin;
use crate::api::cache::{invalidate_strains, make_etag_header};
use crate::db::strains::Strain;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub(crate) struct CreateStrainRequest {
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

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/strains", get(get_strains).post(create_strain))
        .route(
            "/strains/:id",
            get(get_strain).put(update_strain).delete(delete_strain),
        )
        .route("/strains/:id/availability", put(toggle_availability))
        .route("/strains/strain-of-day", get(get_strains_of_day))
        .route("/strains/:id/strain-of-day", put(set_strain_of_day))
        // Cycle #133-C: bulk-toggle marketing flags. POST body lists
        // strain ids and which boolean flags to set (`Option<bool>`
        // each — `None` means leave untouched). Closes ТЗ #2 §3/§4
        // "Назначать одновременно несколько сортов".
        .route("/strains/bulk-marketing", post(bulk_set_marketing))
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

    let mut strains: Vec<Strain> = models.into_iter().map(Strain::from).collect();
    // Cycle #133-B + #136: TZ #2 section 5 — strip Sale / Best Seller /
    // New Arrival flags off customer-facing rows when the admin has
    // turned the global toggle on. Two inputs feed the decision:
    //   * env `HIDE_MARKETING_BADGES=1` — ops kill switch (#133-B)
    //   * DB `loyalty_config.marketing_badges_hidden` — admin self-
    //     service toggle via PUT /api/admin/marketing-display (#136)
    // Either being on hides the badges. SOTD stays — it's the headline
    // feature, not a promo. Admin endpoints (`include_hidden=1`)
    // bypass everything so admins still see real flag state.
    if !include_hidden && marketing_badges_hidden(&state).await {
        for s in strains.iter_mut() {
            mask_marketing_flags(s);
        }
    }
    let response_data = json!({ "strains": strains });
    let data_json = response_data.to_string();

    // The admin (`include_hidden`) variant is a DIFFERENT, auth-gated response
    // (hidden rows + real marketing flags). Cache it under a separate key so it
    // doesn't flap the public "strains" ETag, and mark it `private, no-store` so
    // a shared cache/CDN can never serve the admin payload to an unauthenticated
    // request for the same URL. The public variant stays shared-cacheable.
    let (cache_key, cache_control): (&str, &'static str) = if include_hidden {
        ("strains_admin", "private, no-store")
    } else {
        ("strains", "public, max-age=60")
    };

    // Compute ETag and check conditional request
    let (changed, etag) = state.cache.has_changed(cache_key, &data_json).await;

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
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static(cache_control));
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
/// Cycle #136: resolve the "hide marketing badges" decision by
/// OR-ing the env override (cycle #133-B) and the DB toggle. Env
/// stays so ops can flip it without touching the DB; DB makes admin
/// self-service possible without touching env / redeploying.
///
/// Reads `loyalty_config` row id=1. If the read fails, defaults to
/// `false` (display badges) so a DB hiccup never accidentally hides
/// promos.
async fn marketing_badges_hidden(state: &AppState) -> bool {
    if state.config.hide_marketing_badges {
        return true;
    }
    use crate::db::entities::loyalty_config;
    use sea_orm::EntityTrait;
    match loyalty_config::Entity::find_by_id(1)
        .one(&state.db.orm)
        .await
    {
        Ok(Some(m)) => m.marketing_badges_hidden,
        Ok(None) => false,
        Err(e) => {
            tracing::warn!("marketing_badges_hidden: loyalty_config read failed: {}", e);
            false
        }
    }
}

/// Cycle #133-B: zero out the three "promo" marketing flags so the
/// customer UI renders no Sale / Best Seller / New Arrival badges.
/// SOTD is kept intact — it's the headline daily feature, not a
/// time-limited promo.
fn mask_marketing_flags(s: &mut Strain) {
    s.sale_active = false;
    s.is_best_seller = false;
    s.is_new_arrival = false;
    s.sale_price = None;
    s.discount_percent = 0.0;
    s.sale_until = None;
    s.new_until = None;
}

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
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
    // Carousel cap: at most 3 featured strains. When enabling, reject if 3 OTHER
    // strains are already set — the owner must unset one first. Disabling is
    // always allowed. (The read side also limits to 3, but enforcing here keeps
    // the admin UI honest instead of silently dropping the 4th.)
    if enabled {
        let others = StrainEntity::find()
            .filter(StrainCol::IsStrainOfDay.eq(true))
            .filter(StrainCol::Id.ne(id.clone()))
            .count(&state.db.orm)
            .await
            .map_err(|e| {
                tracing::error!("SOTD cap count error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "count failed" })),
                )
            })?;
        if others >= 3 {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "sotd_limit",
                    "message": "Максимум 3 сорта дня. Снимите отметку с одного."
                })),
            ));
        }
    }
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

/// Cycle #133-C: bulk marketing-flag toggle.
///
/// Request body:
///   {
///     "ids":              ["uuid1", "uuid2", …],   // required, 1..=500
///     "is_best_seller":   true | false | null,     // optional — null means "leave alone"
///     "is_new_arrival":   true | false | null,     // optional
///     "sale_active":      true | false | null,     // optional
///   }
///
/// Returns `{"success": true, "updated": N}` where N is the number of
/// strain rows touched (`rows_affected`). All three flag fields are
/// `Option<bool>` so the admin UI can toggle exactly one at a time
/// (leave the others null) instead of having to round-trip the
/// current values.
///
/// Closes ТЗ #2 §3/§4 "Назначать одновременно несколько сортов" —
/// pre-#133-C the only path was N parallel `PUT /api/strains/:id`
/// calls from the admin client.
async fn bulk_set_marketing(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BulkMarketingRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&headers, &state).map_err(|c| (c, Json(json!({ "error": "unauthorized" }))))?;
    validate_bulk_marketing(&req)
        .map_err(|msg| (StatusCode::BAD_REQUEST, Json(json!({ "error": msg }))))?;

    use crate::db::entities::strain::{Column as StrainCol, Entity as StrainEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    // Build the update_many with only the fields the caller actually
    // set. Three Options × one .col_expr() each when Some(_).
    let mut q = StrainEntity::update_many().filter(StrainCol::Id.is_in(req.ids.clone()));
    if let Some(v) = req.is_best_seller {
        q = q.col_expr(StrainCol::IsBestSeller, sea_orm::sea_query::Expr::value(v));
    }
    if let Some(v) = req.is_new_arrival {
        q = q.col_expr(StrainCol::IsNewArrival, sea_orm::sea_query::Expr::value(v));
    }
    if let Some(v) = req.sale_active {
        q = q.col_expr(StrainCol::SaleActive, sea_orm::sea_query::Expr::value(v));
    }
    let result = q.exec(&state.db.orm).await.map_err(|e| {
        tracing::error!("bulk_set_marketing SeaORM error: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "db error" })),
        )
    })?;
    invalidate_strains(&state.cache).await;
    Ok(Json(
        json!({ "success": true, "updated": result.rows_affected }),
    ))
}

#[derive(Debug, Deserialize)]
struct BulkMarketingRequest {
    ids: Vec<String>,
    #[serde(default)]
    is_best_seller: Option<bool>,
    #[serde(default)]
    is_new_arrival: Option<bool>,
    #[serde(default)]
    sale_active: Option<bool>,
}

/// Pure validator for the bulk request. Returns the same human-readable
/// reasons the handler surfaces as a JSON `error` field.
fn validate_bulk_marketing(req: &BulkMarketingRequest) -> Result<(), &'static str> {
    if req.ids.is_empty() {
        return Err("ids must be non-empty");
    }
    if req.ids.len() > 500 {
        return Err("ids must not exceed 500 entries per request");
    }
    for id in &req.ids {
        if id.is_empty() || id.len() > 200 {
            return Err("each id must be 1..=200 chars");
        }
    }
    if req.is_best_seller.is_none() && req.is_new_arrival.is_none() && req.sale_active.is_none() {
        return Err("at least one of is_best_seller/is_new_arrival/sale_active must be set");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{mask_marketing_flags, validate_strain_request, CreateStrainRequest};
    use crate::api::{extract_bool, extract_discount};
    use crate::db::strains::Strain;
    use axum::http::StatusCode;

    // ── mask_marketing_flags (cycle #133-B) ──────────────────────────

    fn strain_with_all_flags_on() -> Strain {
        Strain {
            id: "id".into(),
            name: "x".into(),
            category: Some("Hybrid".into()),
            thc_percent: Some(20.0),
            cbd_percent: Some(1.0),
            effect: None,
            flavor_profile: None,
            description: None,
            price_per_gram: 100.0,
            available_grams: None,
            image_url: None,
            video_url: None,
            is_available: true,
            is_strain_of_day: true,
            strain_of_day_discount: 15.0,
            name_en: None,
            description_en: None,
            effect_en: None,
            flavor_profile_en: None,
            strain_type_en: None,
            discount_percent: 25.0,
            sale_price: Some(75.0),
            sale_active: true,
            sale_until: Some("2099-01-01T00:00:00Z".into()),
            is_best_seller: true,
            is_new_arrival: true,
            new_until: Some("2099-01-01T00:00:00Z".into()),
            display_order: 0,
        }
    }

    // ── validate_bulk_marketing (cycle #133-C) ───────────────────────

    fn br(ids: Vec<&str>) -> super::BulkMarketingRequest {
        super::BulkMarketingRequest {
            ids: ids.into_iter().map(String::from).collect(),
            is_best_seller: Some(true),
            is_new_arrival: None,
            sale_active: None,
        }
    }

    #[test]
    fn bulk_validator_rejects_empty_ids() {
        let r = br(vec![]);
        assert_eq!(
            super::validate_bulk_marketing(&r),
            Err("ids must be non-empty")
        );
    }

    #[test]
    fn bulk_validator_rejects_too_many_ids() {
        let mut r = br(vec!["id"]);
        r.ids = (0..501).map(|i| format!("id-{}", i)).collect();
        assert!(super::validate_bulk_marketing(&r)
            .err()
            .map(|s| s.contains("500"))
            .unwrap_or(false));
    }

    #[test]
    fn bulk_validator_rejects_empty_id_entry() {
        let r = br(vec!["", "ok"]);
        assert_eq!(
            super::validate_bulk_marketing(&r),
            Err("each id must be 1..=200 chars")
        );
    }

    #[test]
    fn bulk_validator_rejects_oversized_id_entry() {
        let mut r = br(vec!["x"]);
        r.ids = vec!["a".repeat(201)];
        assert!(super::validate_bulk_marketing(&r).is_err());
    }

    #[test]
    fn bulk_validator_rejects_all_flags_none() {
        let mut r = br(vec!["id"]);
        r.is_best_seller = None;
        r.is_new_arrival = None;
        r.sale_active = None;
        assert!(super::validate_bulk_marketing(&r)
            .err()
            .map(|s| s.contains("at least one"))
            .unwrap_or(false));
    }

    #[test]
    fn bulk_validator_accepts_one_flag_set() {
        // is_best_seller defaults to Some(true) in br()
        let r = br(vec!["id-1", "id-2"]);
        assert!(super::validate_bulk_marketing(&r).is_ok());
    }

    #[test]
    fn bulk_validator_accepts_false_flag() {
        // Setting a flag to false is a valid action (turn off).
        let mut r = br(vec!["id"]);
        r.is_best_seller = Some(false);
        assert!(super::validate_bulk_marketing(&r).is_ok());
    }

    #[test]
    fn mask_zeroes_promo_flags_keeps_sotd() {
        let mut s = strain_with_all_flags_on();
        mask_marketing_flags(&mut s);
        // Promo flags zeroed
        assert!(!s.sale_active);
        assert!(!s.is_best_seller);
        assert!(!s.is_new_arrival);
        assert_eq!(s.sale_price, None);
        assert_eq!(s.discount_percent, 0.0);
        assert_eq!(s.sale_until, None);
        assert_eq!(s.new_until, None);
        // SOTD stays — headline feature, not a promo.
        assert!(s.is_strain_of_day);
        assert_eq!(s.strain_of_day_discount, 15.0);
        // Other fields untouched.
        assert_eq!(s.id, "id");
        assert_eq!(s.price_per_gram, 100.0);
    }

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
