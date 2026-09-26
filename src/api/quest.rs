use crate::api::auth::{check_admin, check_not_blocked, validate_init_data};
use crate::trios::validation::clamp_finite_in_range;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        // 2026-09-21: a GET registration for the literal "/api/test" stood
        // here, its handler below. src/api/mod.rs:77 nests this router under
        // "/api", so it was served at /api/api/test while a request to
        // /api/test met the miss handler at src/api/mod.rs:128 -- unreachable
        // since written and fetched nowhere. Not respelled in a route shape:
        // parsers read this file as text. Guard: tests/api_document_pairing.rs.
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

// Cycle #148: `clamp_finite_in_range` moved to `src/trios/validation.rs`
// so other read paths can share the helper. The four callsites below
// import it via `use crate::trios::validation::clamp_finite_in_range;`
// at the top of this file.

// ── Quest Places ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct QuestPlaceRequest {
    pub name: String,
    pub category: Option<String>,
    pub lat: f64,
    pub lon: f64,
    pub description: Option<String>,
    pub image_url: Option<String>,
    #[allow(dead_code)]
    pub is_available: Option<bool>,
}

/// `GET /api/quest-places`, which no mounted screen reads. R2 (owner,
/// 2026-09-26, verbatim): «Закрыть для клиентов». A caller without admin proof
/// is answered exactly as an unmatched `/api` path is
/// (`crate::api::admin_or_missing_route`); an admin is served as before, and
/// no row is read or written differently.
async fn get_quest_places(
    headers: HeaderMap,
    uri: axum::http::Uri,
    State(state): State<AppState>,
) -> Result<Response, StatusCode> {
    if let Err(miss) = crate::api::admin_or_missing_route(&headers, &state, &uri) {
        return Ok(miss);
    }
    // Wave 3: полный row mapping. После миграции 020 lat/lon — DOUBLE PRECISION,
    // но оставляем ::float8 на SELECT для совместимости со старыми инстансами.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id, name, COALESCE(category,'location') AS category, \
                lat::float8 AS lat, lon::float8 AS lon, \
                COALESCE(description,'') AS description, \
                COALESCE(image_url,'') AS image_url, \
                COALESCE(is_available, true) AS is_available \
         FROM quest_places ORDER BY name LIMIT 2000",
        [],
    );
    let rows = state.db.orm.query_all(stmt).await.map_err(|e| {
        tracing::error!("get_quest_places sea-orm: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            // Cycle #147: range-clamp via `clamp_finite_in_range`,
            // not `.max(0)`. See helper doc for the Gulf-of-Guinea
            // bug context.
            let lat =
                clamp_finite_in_range(r.try_get::<f64>("", "lat").unwrap_or(0.0), -90.0, 90.0, 0.0);
            let lon = clamp_finite_in_range(
                r.try_get::<f64>("", "lon").unwrap_or(0.0),
                -180.0,
                180.0,
                0.0,
            );
            json!({
                "id":          r.try_get::<String>("", "id").unwrap_or_default(),
                "name":        r.try_get::<String>("", "name").unwrap_or_default(),
                "category":    r.try_get::<String>("", "category").unwrap_or_default(),
                "lat":         lat,
                "lon":         lon,
                "description": r.try_get::<String>("", "description").unwrap_or_default(),
                "image_url":   r.try_get::<String>("", "image_url").unwrap_or_default(),
                "is_available":r.try_get::<bool>("", "is_available").unwrap_or(true),
            })
        })
        .collect();

    Ok(Json(json!({ "quest_places": items })).into_response())
}

fn validate_quest_place_request(req: &QuestPlaceRequest) -> Result<(), StatusCode> {
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
    if !req.lat.is_finite() || !req.lon.is_finite() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.lat < -90.0 || req.lat > 90.0 || req.lon < -180.0 || req.lon > 180.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    crate::api::validate_url(&req.image_url)?;
    Ok(())
}

async fn create_quest_place(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<QuestPlaceRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_quest_place_request(&req)?;
    let id = uuid::Uuid::new_v4().to_string();
    let category = req.category.unwrap_or_else(|| "location".to_string());
    let description = req.description.unwrap_or_default();
    let image_url = req.image_url.unwrap_or_default();
    // BUG-6 fix via SeaORM: в прод-схеме lat/lon могут быть NUMERIC (не DOUBLE PRECISION).
    // sqlx под капотом кастит f64 в numeric автоматически; то же для explicit ::float8 cast.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO quest_places (id, name, category, lat, lon, description, image_url) VALUES ($1,$2,$3,$4::float8,$5::float8,$6,$7)",
        [id.clone().into(), req.name.clone().into(), category.clone().into(), req.lat.into(), req.lon.into(), description.clone().into(), image_url.into()],
    );
    state.db.orm.execute(stmt).await.map_err(|e| {
        tracing::error!("create_quest_place sea-orm: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    crate::metrics::quest_created("place");

    let bot = state.bot.clone();
    let config = state.config.clone();
    let name = req.name.clone();
    let cat = category.clone();
    let lat = req.lat;
    let lon = req.lon;
    let desc = description.clone();
    tokio::spawn(async move {
        notify_quest_place_admins(&bot, &config, &name, &cat, lat, lon, &desc).await;
    });

    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_quest_place(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<QuestPlaceRequest>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    validate_quest_place_request(&req)?;
    // Wave 3: UPDATE через SeaORM с ::float8 castом — устраняет NUMERIC баг.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let category = req.category.unwrap_or_else(|| "location".to_string());
    let description = req.description.unwrap_or_default();
    let image_url = req.image_url.unwrap_or_default();
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE quest_places SET name=$1, category=$2, lat=$3::float8, lon=$4::float8, description=$5, image_url=$6 WHERE id=$7",
        [req.name.into(), category.into(), req.lat.into(), req.lon.into(), description.into(), image_url.into(), id.into()],
    );
    state.db.orm.execute(stmt).await.map_err(|e| {
        tracing::error!("update_quest_place sea-orm: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_quest_place(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE quest_places SET is_available = false WHERE id = $1",
        [id.into()],
    );
    state.db.orm.execute(stmt).await.map_err(|e| {
        tracing::error!("delete_quest_place sea-orm: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "success": true })))
}

// ── Treasure Hunts ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct TreasureHuntRequest {
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub black_mark_title: String,
    pub black_mark_description: Option<String>,
    pub black_mark_image_url: Option<String>,
    #[allow(dead_code)]
    pub is_active: Option<bool>,
    #[allow(dead_code)]
    pub starts_at: Option<String>,
    #[allow(dead_code)]
    pub ends_at: Option<String>,
    pub start_lat: f64,
    pub start_lon: f64,
    pub start_name: String,
}

/// `GET /api/treasure-hunts`, which no mounted screen reads. R2 (owner,
/// 2026-09-26, verbatim): «Закрыть для клиентов». Closed to customers exactly
/// as `get_quest_places` is.
async fn get_treasure_hunts(
    headers: HeaderMap,
    uri: axum::http::Uri,
    State(state): State<AppState>,
) -> Result<Response, StatusCode> {
    if let Err(miss) = crate::api::admin_or_missing_route(&headers, &state, &uri) {
        return Ok(miss);
    }
    // Cycle #93: SeaORM via Statement. SeaORM's `try_get("", "col")`
    // takes column *name*, not positional index — switch from `try_get(N)`
    // to `try_get("", "col_name")`.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let rows = state.db.orm.query_all(Statement::from_string(
        DbBackend::Postgres,
        "SELECT id, name, description, image_url, black_mark_title, black_mark_description, black_mark_image_url, is_active, starts_at, ends_at, start_lat::float8, start_lon::float8, start_name FROM treasure_hunts WHERE is_active = true ORDER BY created_at DESC LIMIT 2000".to_string(),
    )).await.map_err(|e| { tracing::error!("get_treasure_hunts: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            // Cycle #147: range-clamp via `clamp_finite_in_range`.
            let start_lat = clamp_finite_in_range(
                r.try_get::<f64>("", "start_lat").unwrap_or(0.0),
                -90.0,
                90.0,
                0.0,
            );
            let start_lon = clamp_finite_in_range(
                r.try_get::<f64>("", "start_lon").unwrap_or(0.0),
                -180.0,
                180.0,
                0.0,
            );
            json!({
                "id": r.try_get::<String>("", "id").unwrap_or_default(),
                "name": r.try_get::<String>("", "name").unwrap_or_default(),
                "description": r.try_get::<String>("", "description").ok(),
                "image_url": r.try_get::<String>("", "image_url").ok(),
                "black_mark_title": r.try_get::<String>("", "black_mark_title").unwrap_or_default(),
                "black_mark_description": r.try_get::<String>("", "black_mark_description").ok(),
                "black_mark_image_url": r.try_get::<String>("", "black_mark_image_url").ok(),
                "is_active": r.try_get::<bool>("", "is_active").unwrap_or(false),
                "starts_at": r.try_get::<String>("", "starts_at").ok(),
                "ends_at": r.try_get::<String>("", "ends_at").ok(),
                "start_lat": start_lat,
                "start_lon": start_lon,
                "start_name": r.try_get::<String>("", "start_name").unwrap_or_default(),
            })
        })
        .collect();
    Ok(Json(json!({ "treasure_hunts": items })).into_response())
}

fn validate_treasure_hunt_request(req: &TreasureHuntRequest) -> Result<(), StatusCode> {
    if req.name.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.black_mark_title.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.start_name.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(ref d) = req.description {
        if d.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref d) = req.black_mark_description {
        if d.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if !req.start_lat.is_finite() || !req.start_lon.is_finite() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.start_lat < -90.0
        || req.start_lat > 90.0
        || req.start_lon < -180.0
        || req.start_lon > 180.0
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    crate::api::validate_url(&req.image_url)?;
    crate::api::validate_url(&req.black_mark_image_url)?;

    // Cycle #154: timestamp format + ordering. Pre-cycle, `starts_at`
    // and `ends_at` were `Option<String>` passed straight to
    // PostgreSQL via `unwrap_or_default().into()`. Two consequences:
    //   1. Any malformed timestamp produced a PG 500 instead of a
    //      clean 400 at the boundary.
    //   2. `starts_at >= ends_at` was silently accepted — a hunt
    //      that ends before it starts has no active window, so the
    //      `is_active` flag is the only thing keeping it visible.
    //      Confusing and irreversible without admin SQL.
    // Empty string is treated as "not provided" (same as `None`)
    // because the existing `create_treasure_hunt` / `update_treasure_hunt`
    // do `unwrap_or_default()` and the empty-string → PG NULL
    // round-trip is the established convention here.
    let parse = |opt: &Option<String>| -> Result<Option<chrono::DateTime<chrono::FixedOffset>>, StatusCode> {
        match opt.as_deref() {
            None | Some("") => Ok(None),
            Some(s) => chrono::DateTime::parse_from_rfc3339(s)
                .map(Some)
                .map_err(|_| StatusCode::BAD_REQUEST),
        }
    };
    let start = parse(&req.starts_at)?;
    let end = parse(&req.ends_at)?;
    if let (Some(s), Some(e)) = (start, end) {
        if s >= e {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    Ok(())
}

async fn create_treasure_hunt(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<TreasureHuntRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_treasure_hunt_request(&req)?;
    let id = uuid::Uuid::new_v4().to_string();
    let description = req.description.unwrap_or_default();
    let image_url = req.image_url.unwrap_or_default();
    let bm_desc = req.black_mark_description.unwrap_or_default();
    let bm_image = req.black_mark_image_url.unwrap_or_default();
    let is_active = req.is_active.unwrap_or(true);
    // Cycle #155: bind `starts_at`/`ends_at` as Option<String>. The
    // pre-cycle `unwrap_or_default()` produced an empty-string for
    // None, which PostgreSQL TIMESTAMPTZ parsing rejected with a 500.
    // Treat None and `Some("")` identically (matches the cycle-#154
    // validator convention) and let sea_orm bind None → SQL NULL.
    let starts_at = req.starts_at.filter(|s| !s.is_empty());
    let ends_at = req.ends_at.filter(|s| !s.is_empty());
    // BUG-4 fix via SeaORM: start_lat/start_lon могут быть NUMERIC.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO treasure_hunts (id, name, description, image_url, black_mark_title, black_mark_description, black_mark_image_url, is_active, starts_at, ends_at, start_lat, start_lon, start_name) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11::float8,$12::float8,$13)",
        [id.clone().into(), req.name.clone().into(), description.clone().into(), image_url.into(), req.black_mark_title.into(), bm_desc.into(), bm_image.into(), is_active.into(), starts_at.into(), ends_at.into(), req.start_lat.into(), req.start_lon.into(), req.start_name.clone().into()],
    );
    state.db.orm.execute(stmt).await.map_err(|e| {
        tracing::error!("create_treasure_hunt sea-orm: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    crate::metrics::quest_created("treasure");

    let bot = state.bot.clone();
    let config = state.config.clone();
    let name = req.name.clone();
    let desc = description.clone();
    let start_name = req.start_name.clone();
    let start_lat = req.start_lat;
    let start_lon = req.start_lon;
    tokio::spawn(async move {
        notify_treasure_hunt_admins(
            &bot,
            &config,
            &name,
            &desc,
            &start_name,
            start_lat,
            start_lon,
        )
        .await;
    });

    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_treasure_hunt(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<TreasureHuntRequest>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    validate_treasure_hunt_request(&req)?;
    // Wave 3: UPDATE через SeaORM с ::float8 castом для start_lat/start_lon.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let description = req.description.unwrap_or_default();
    let image_url = req.image_url.unwrap_or_default();
    let bm_desc = req.black_mark_description.unwrap_or_default();
    let bm_image = req.black_mark_image_url.unwrap_or_default();
    let is_active = req.is_active.unwrap_or(true);
    // Cycle #155: mirror of the create_treasure_hunt fix — bind
    // Option<String> so None / empty → SQL NULL instead of an
    // empty-string that PG TIMESTAMPTZ rejects.
    let starts_at = req.starts_at.filter(|s| !s.is_empty());
    let ends_at = req.ends_at.filter(|s| !s.is_empty());
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE treasure_hunts SET name=$1, description=$2, image_url=$3, black_mark_title=$4, black_mark_description=$5, black_mark_image_url=$6, is_active=$7, starts_at=$8, ends_at=$9, start_lat=$10::float8, start_lon=$11::float8, start_name=$12 WHERE id=$13",
        [req.name.into(), description.into(), image_url.into(), req.black_mark_title.into(), bm_desc.into(), bm_image.into(), is_active.into(), starts_at.into(), ends_at.into(), req.start_lat.into(), req.start_lon.into(), req.start_name.into(), id.into()],
    );
    state.db.orm.execute(stmt).await.map_err(|e| {
        tracing::error!("update_treasure_hunt sea-orm: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_treasure_hunt(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE treasure_hunts SET is_active = false WHERE id = $1",
        [id.into()],
    );
    state.db.orm.execute(stmt).await.map_err(|e| {
        tracing::error!("delete_treasure_hunt sea-orm: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "success": true })))
}

// ── Location Quest Locations ──────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct QuestLocationParams {
    #[allow(dead_code)]
    pub telegram_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QuestLocationRequest {
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub map_url: Option<String>,
    #[allow(dead_code)]
    pub is_active: Option<bool>,
    pub is_final: Option<bool>,
}

async fn get_quest_locations(
    State(state): State<AppState>,
    Query(_params): Query<QuestLocationParams>,
) -> Result<Json<Value>, StatusCode> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let rows = state.db.orm.query_all(Statement::from_string(
        DbBackend::Postgres,
        "SELECT id, name, description, category, map_url, is_active, is_final FROM location_quest_locations WHERE is_active = true ORDER BY id LIMIT 2000".to_string(),
    )).await.map_err(|e| { tracing::error!("get_quest_locations: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            // id is a NOT NULL PK; a read error means real schema drift — fail
            // loud rather than emitting a fake id=0 list entry the client clicks.
            let id: i32 = r.try_get("", "id").map_err(|e| {
                tracing::error!("get_quest_locations: corrupt id: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            Ok(json!({
                "id": id,
                "name": r.try_get::<String>("", "name").unwrap_or_default(),
                "description": r.try_get::<String>("", "description").ok(),
                "category": r.try_get::<String>("", "category").ok(),
                "map_url": r.try_get::<String>("", "map_url").ok(),
                "is_active": r.try_get::<bool>("", "is_active").unwrap_or(true),
                "is_final": r.try_get::<bool>("", "is_final").unwrap_or(false),
            }))
        })
        .collect::<Result<Vec<Value>, StatusCode>>()?;
    Ok(Json(json!({ "locations": items })))
}

fn validate_quest_location_request(req: &QuestLocationRequest) -> Result<(), StatusCode> {
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
    crate::api::validate_url(&req.map_url)?;
    Ok(())
}

async fn create_quest_location(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<QuestLocationRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_quest_location_request(&req)?;
    // Cycle #93: SeaORM via Statement with `RETURNING id`. The id is
    // BIGSERIAL — auto-generated by Postgres, returned by the statement.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = state.db.orm.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO location_quest_locations (name, description, category, map_url, is_active, is_final) VALUES ($1,$2,$3,$4,$5,$6) RETURNING id",
        [
            req.name.into(),
            req.description.unwrap_or_default().into(),
            req.category.unwrap_or_else(|| "location".to_string()).into(),
            req.map_url.unwrap_or_default().into(),
            req.is_active.unwrap_or(true).into(),
            req.is_final.unwrap_or(false).into(),
        ],
    )).await.map_err(|e| { tracing::error!("create_quest_location: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?
        .ok_or_else(|| {
            tracing::error!("create_quest_location: RETURNING produced no row");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    // Fail loud: the freshly INSERTed `id` (NOT NULL PK from RETURNING) is the
    // identifier the client uses to reference the new location. A silent
    // `.unwrap_or(0)` would return `{"success": true, "id": 0}` on a read error
    // — a fabricated id the client would then act on. Propagate instead.
    let id: i32 = row.try_get("", "id").map_err(|e| {
        tracing::error!("create_quest_location: corrupt RETURNING id: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn update_quest_location(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<QuestLocationRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_quest_location_request(&req)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE location_quest_locations SET name=$1, description=$2, category=$3, map_url=$4, is_active=$5, is_final=$6 WHERE id=$7",
        [
            req.name.into(),
            req.description.unwrap_or_default().into(),
            req.category.unwrap_or_else(|| "location".to_string()).into(),
            req.map_url.unwrap_or_default().into(),
            req.is_active.unwrap_or(true).into(),
            req.is_final.unwrap_or(false).into(),
            id.into(),
        ],
    )).await.map_err(|e| { tracing::error!("update_quest_location: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true })))
}

fn extract_qr_token(body: &Value) -> Result<&str, StatusCode> {
    let token = body["qr_token"]
        .as_str()
        .or(body["code"].as_str())
        .unwrap_or("");
    if token.len() > 200 || token.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(token)
}

async fn scan_quest_qr(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // Require Telegram Mini App auth
    let init_data = headers
        .get("X-Telegram-Init-Data")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let user =
        validate_init_data(init_data, &state.config.bot_token).ok_or(StatusCode::UNAUTHORIZED)?;
    check_not_blocked(&state, user.id).await?;

    let qr_token = extract_qr_token(&body)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = state.db.orm.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id, name, is_final FROM location_quest_locations WHERE qr_token = $1 AND is_active = true LIMIT 1",
        [qr_token.into()],
    )).await.map_err(|e| { tracing::error!("scan_quest_qr: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    match row {
        Some(r) => {
            let location_name: String = r.try_get("", "name").unwrap_or_default();
            let is_final: bool = r.try_get("", "is_final").unwrap_or(false);
            crate::metrics::qr_scanned(is_final);
            // Notify admins about QR scan
            let bot = state.bot.clone();
            let config = state.config.clone();
            let loc_name = location_name.clone();
            let telegram_id = user.id;
            tokio::spawn(async move {
                let final_str = if is_final {
                    "\n\u{1F3C1} \u{0444}\u{0438}\u{043D}\u{0430}\u{043B}\u{044C}\u{043D}\u{0430}\u{044F} \u{0442}\u{043E}\u{0447}\u{043A}\u{0430}!"
                } else {
                    ""
                };
                let user_str = format!("\n\u{1F194} user: {}", telegram_id);
                let text = format!(
                    "\u{1F4F2} QR \u{043E}\u{0442}\u{0441}\u{043A}\u{0430}\u{043D}\u{0438}\u{0440}\u{043E}\u{0432}\u{0430}\u{043D}\n\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\u{1F4CD} {}{}{}" ,
                    html_escape(&loc_name), user_str, final_str
                );
                crate::notify::notify_admins(&bot, &config, &text).await;
            });
            // Fail loud on the location id the client receives + acts on.
            let id: i32 = r.try_get("", "id").map_err(|e| {
                tracing::error!("scan_quest_qr: corrupt location id: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            Ok(Json(json!({
                "success": true,
                "location": {
                    "id": id,
                    "name": location_name,
                    "is_final": is_final,
                }
            })))
        }
        None => Ok(Json(
            json!({ "success": false, "error": "Invalid QR token" }),
        )),
    }
}

// ── Admin Notifications ───────────────────────────────────────

use crate::util::html_escape;

async fn notify_quest_place_admins(
    bot: &teloxide::Bot,
    config: &crate::config::Config,
    name: &str,
    category: &str,
    lat: f64,
    lon: f64,
    description: &str,
) {
    use teloxide::prelude::*;

    let text = format!(
        "📍 Новое квест-место создано\n━━━━━━━━━━━━━━━━\n🏷 {}\n📂 {}\n🗺 {}, {}\n📝 {}",
        html_escape(name),
        html_escape(category),
        lat,
        lon,
        html_escape(description)
    );

    for admin_id in &config.admin_ids {
        // Cycle #150: parse_mode=Html so the html_escape'd `name`,
        // `category`, `description` above render `&lt;` as `<` (not
        // literal `&lt;`). Same fix shape as cycle #149's notify_admins.
        if let Err(e) = bot
            .send_message(teloxide::types::ChatId(*admin_id), &text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .await
        {
            tracing::warn!(
                "notify_quest_place_admins failed for admin_id={}: {}",
                admin_id,
                e
            );
        }
    }
}

async fn notify_treasure_hunt_admins(
    bot: &teloxide::Bot,
    config: &crate::config::Config,
    name: &str,
    description: &str,
    start_name: &str,
    start_lat: f64,
    start_lon: f64,
) {
    use teloxide::prelude::*;

    let text = format!(
        "🏴\u{200d}☠️ Новый treasure hunt создан\n━━━━━━━━━━━━━━━━\n🏷 {}\n📜 {}\n🗺 Старт: {} ({}, {})",
        html_escape(name), html_escape(description), html_escape(start_name), start_lat, start_lon
    );

    for admin_id in &config.admin_ids {
        // Cycle #150: parse_mode=Html — same as notify_quest_place_admins.
        if let Err(e) = bot
            .send_message(teloxide::types::ChatId(*admin_id), &text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .await
        {
            tracing::warn!(
                "notify_treasure_hunt_admins failed for admin_id={}: {}",
                admin_id,
                e
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        extract_qr_token, validate_quest_location_request, validate_quest_place_request,
        validate_treasure_hunt_request, QuestLocationRequest, QuestPlaceRequest,
        TreasureHuntRequest,
    };
    use axum::http::StatusCode;

    // Cycle #148: clamp_finite_in_range tests moved with the helper
    // to `src/trios/validation.rs`. See `crate::trios::validation::tests::clamp_*`.

    fn valid_quest_place() -> QuestPlaceRequest {
        QuestPlaceRequest {
            name: "Place".into(),
            category: Some("cat".into()),
            lat: 55.0,
            lon: 37.0,
            description: Some("desc".into()),
            image_url: Some("/uploads/place.jpg".into()),
            is_available: Some(true),
        }
    }

    #[test]
    fn test_validate_quest_place_ok() {
        assert!(validate_quest_place_request(&valid_quest_place()).is_ok());
    }

    #[test]
    fn test_validate_quest_place_name_too_long() {
        let mut req = valid_quest_place();
        req.name = "a".repeat(201);
        assert_eq!(
            validate_quest_place_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_quest_place_category_too_long() {
        let mut req = valid_quest_place();
        req.category = Some("a".repeat(201));
        assert_eq!(
            validate_quest_place_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_quest_place_description_too_long() {
        let mut req = valid_quest_place();
        req.description = Some("a".repeat(1001));
        assert_eq!(
            validate_quest_place_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_quest_place_lat_out_of_range() {
        let mut req = valid_quest_place();
        req.lat = 91.0;
        assert_eq!(
            validate_quest_place_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_quest_place_lon_out_of_range() {
        let mut req = valid_quest_place();
        req.lon = 181.0;
        assert_eq!(
            validate_quest_place_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_quest_place_bad_image_url() {
        let mut req = valid_quest_place();
        req.image_url = Some("javascript:alert(1)".into());
        assert_eq!(
            validate_quest_place_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    fn valid_treasure_hunt() -> TreasureHuntRequest {
        TreasureHuntRequest {
            name: "Hunt".into(),
            description: Some("desc".into()),
            image_url: Some("/uploads/hunt.jpg".into()),
            black_mark_title: "Mark".into(),
            black_mark_description: Some("bm desc".into()),
            black_mark_image_url: Some("/uploads/bm.jpg".into()),
            is_active: Some(true),
            starts_at: None,
            ends_at: None,
            start_lat: 55.0,
            start_lon: 37.0,
            start_name: "Start".into(),
        }
    }

    #[test]
    fn test_validate_treasure_hunt_ok() {
        assert!(validate_treasure_hunt_request(&valid_treasure_hunt()).is_ok());
    }

    #[test]
    fn test_validate_treasure_hunt_name_too_long() {
        let mut req = valid_treasure_hunt();
        req.name = "a".repeat(201);
        assert_eq!(
            validate_treasure_hunt_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_treasure_hunt_black_mark_title_too_long() {
        let mut req = valid_treasure_hunt();
        req.black_mark_title = "a".repeat(201);
        assert_eq!(
            validate_treasure_hunt_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_treasure_hunt_start_name_too_long() {
        let mut req = valid_treasure_hunt();
        req.start_name = "a".repeat(201);
        assert_eq!(
            validate_treasure_hunt_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_treasure_hunt_description_too_long() {
        let mut req = valid_treasure_hunt();
        req.description = Some("a".repeat(1001));
        assert_eq!(
            validate_treasure_hunt_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_treasure_hunt_start_lat_out_of_range() {
        let mut req = valid_treasure_hunt();
        req.start_lat = 91.0;
        assert_eq!(
            validate_treasure_hunt_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_treasure_hunt_bad_image_url() {
        let mut req = valid_treasure_hunt();
        req.image_url = Some("javascript:alert(1)".into());
        assert_eq!(
            validate_treasure_hunt_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    // ── treasure_hunt timestamp tests (cycle #154) ───────────────────

    #[test]
    fn test_validate_treasure_hunt_both_timestamps_none_ok() {
        // Pre-cycle baseline: both Option::None is valid.
        let mut req = valid_treasure_hunt();
        req.starts_at = None;
        req.ends_at = None;
        assert!(validate_treasure_hunt_request(&req).is_ok());
    }

    #[test]
    fn test_validate_treasure_hunt_proper_ordering_ok() {
        let mut req = valid_treasure_hunt();
        req.starts_at = Some("2026-06-01T00:00:00Z".into());
        req.ends_at = Some("2026-07-01T00:00:00Z".into());
        assert!(validate_treasure_hunt_request(&req).is_ok());
    }

    #[test]
    fn test_validate_treasure_hunt_start_after_end_rejected() {
        // The motivator: admin types end_date for `starts_at` and
        // start_date for `ends_at`. Pre-cycle the hunt was created
        // with no active window and only `is_active=true` kept it
        // visible.
        let mut req = valid_treasure_hunt();
        req.starts_at = Some("2026-07-01T00:00:00Z".into());
        req.ends_at = Some("2026-06-01T00:00:00Z".into());
        assert_eq!(
            validate_treasure_hunt_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_treasure_hunt_equal_timestamps_rejected() {
        // Equal start/end means the active window has zero duration.
        let mut req = valid_treasure_hunt();
        let same = "2026-06-01T00:00:00Z".to_string();
        req.starts_at = Some(same.clone());
        req.ends_at = Some(same);
        assert_eq!(
            validate_treasure_hunt_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_treasure_hunt_malformed_starts_at_rejected() {
        let mut req = valid_treasure_hunt();
        req.starts_at = Some("not-a-timestamp".into());
        req.ends_at = Some("2026-07-01T00:00:00Z".into());
        assert_eq!(
            validate_treasure_hunt_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_treasure_hunt_only_one_timestamp_ok() {
        // Either-or is fine: starts-only means "active from … forever";
        // ends-only means "active until …". Ordering check no-ops.
        let mut req = valid_treasure_hunt();
        req.starts_at = Some("2026-06-01T00:00:00Z".into());
        req.ends_at = None;
        assert!(validate_treasure_hunt_request(&req).is_ok());
        req.starts_at = None;
        req.ends_at = Some("2026-07-01T00:00:00Z".into());
        assert!(validate_treasure_hunt_request(&req).is_ok());
    }

    #[test]
    fn test_validate_treasure_hunt_empty_string_treated_as_none() {
        // Match the existing `unwrap_or_default()` convention in
        // `create_treasure_hunt` / `update_treasure_hunt`: empty
        // string round-trips to NULL.
        let mut req = valid_treasure_hunt();
        req.starts_at = Some("".into());
        req.ends_at = Some("".into());
        assert!(validate_treasure_hunt_request(&req).is_ok());
    }

    fn valid_quest_location() -> QuestLocationRequest {
        QuestLocationRequest {
            name: "Loc".into(),
            description: Some("desc".into()),
            category: Some("cat".into()),
            map_url: Some("/uploads/map.jpg".into()),
            is_active: Some(true),
            is_final: Some(false),
        }
    }

    #[test]
    fn test_validate_quest_location_ok() {
        assert!(validate_quest_location_request(&valid_quest_location()).is_ok());
    }

    #[test]
    fn test_validate_quest_location_name_too_long() {
        let mut req = valid_quest_location();
        req.name = "a".repeat(201);
        assert_eq!(
            validate_quest_location_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_quest_location_category_too_long() {
        let mut req = valid_quest_location();
        req.category = Some("a".repeat(201));
        assert_eq!(
            validate_quest_location_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_quest_location_description_too_long() {
        let mut req = valid_quest_location();
        req.description = Some("a".repeat(1001));
        assert_eq!(
            validate_quest_location_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_quest_location_bad_map_url() {
        let mut req = valid_quest_location();
        req.map_url = Some("javascript:alert(1)".into());
        assert_eq!(
            validate_quest_location_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_qr_token_from_qr_token() {
        let body = serde_json::json!({"qr_token": "abc123"});
        assert_eq!(extract_qr_token(&body).unwrap(), "abc123");
    }

    #[test]
    fn test_extract_qr_token_from_code() {
        let body = serde_json::json!({"code": "xyz789"});
        assert_eq!(extract_qr_token(&body).unwrap(), "xyz789");
    }

    #[test]
    fn test_extract_qr_token_qr_token_preferred() {
        let body = serde_json::json!({"qr_token": "aaa", "code": "bbb"});
        assert_eq!(extract_qr_token(&body).unwrap(), "aaa");
    }

    #[test]
    fn test_extract_qr_token_empty() {
        let body = serde_json::json!({"qr_token": ""});
        assert_eq!(
            extract_qr_token(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_qr_token_too_long() {
        let body = serde_json::json!({"qr_token": "a".repeat(201)});
        assert_eq!(
            extract_qr_token(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_qr_token_missing() {
        let body = serde_json::json!({"other": "value"});
        assert_eq!(
            extract_qr_token(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }
}

// The response types of the closed reads (R2, the owner's answer of 2026-09-26).
// Imported at the end of the file so that no line cited above moves.
use axum::response::{IntoResponse, Response};
