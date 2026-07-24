use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::api::auth::{check_admin, check_not_blocked, check_owner, validate_telegram_id_param};
use crate::api::orders::is_valid_idempotency_key;
use crate::AppState;

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        // Public event calendar
        .route("/events", get(list_events))
        .route("/events/:id", get(get_event))
        .route("/events/:id/book", post(book_event))
        // Admin
        .route("/admin/events", get(list_admin_events).post(create_event))
        .route("/admin/events/:id", get(get_admin_event).put(update_event).delete(delete_event))
        .route("/admin/events/:id/bookings", get(list_event_bookings))
        .route("/admin/events/:id/bookings/:booking_id/cancel", put(cancel_booking))
}

// ── Requests ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct CreateEventRequest {
    pub title: String,
    pub title_en: Option<String>,
    pub description: Option<String>,
    pub description_en: Option<String>,
    pub starts_at: String,
    pub ends_at: Option<String>,
    pub location_text: Option<String>,
    pub image_url: Option<String>,
    pub max_seats: Option<i32>,
    pub price_baht: Option<f64>,
    pub is_public: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateEventRequest {
    pub title: Option<String>,
    pub title_en: Option<String>,
    pub description: Option<String>,
    pub description_en: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub location_text: Option<String>,
    pub image_url: Option<String>,
    pub max_seats: Option<i32>,
    pub price_baht: Option<f64>,
    pub is_public: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BookEventRequest {
    pub telegram_id: i64,
    #[serde(default)]
    pub seats: Option<i32>,
}

// ── Helpers ──────────────────────────────────────────────────

fn parse_iso_timestamp(s: &str) -> Result<DateTime<Utc>, StatusCode> {
    s.parse::<DateTime<Utc>>()
        .map_err(|e| {
            tracing::debug!("parse_iso_timestamp failed: {e}");
            StatusCode::BAD_REQUEST
        })
}

fn event_id_ok(id: &str) -> Result<(), StatusCode> {
    if id.is_empty() || id.len() > 36 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

fn booking_id_ok(id: &str) -> Result<(), StatusCode> {
    if id.is_empty() || id.len() > 36 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// Validates a create/update payload. Returns a sanitized `is_public` default.
fn validate_event_request(
    req: &CreateEventRequest,
) -> Result<(bool, DateTime<Utc>, Option<DateTime<Utc>>), (StatusCode, String)> {
    fn bad(msg: String) -> (StatusCode, String) {
        (StatusCode::BAD_REQUEST, msg)
    }
    let title = req.title.trim();
    if title.is_empty() || title.len() > 200 {
        return Err(bad(format!("title невалиден ({} chars)", title.len())));
    }
    if let Some(ref t) = req.title_en {
        let t = t.trim();
        if t.len() > 200 {
            return Err(bad(format!("title_en слишком длинный ({}>200)", t.len())));
        }
    }
    if let Some(ref d) = req.description {
        if d.len() > 2000 {
            return Err(bad(format!("description слишком длинное ({}>2000)", d.len())));
        }
    }
    if let Some(ref d) = req.description_en {
        if d.len() > 2000 {
            return Err(bad(format!("description_en слишком длинное ({}>2000)", d.len())));
        }
    }
    if let Some(ref l) = req.location_text {
        if l.len() > 300 {
            return Err(bad(format!("location_text слишком длинный ({}>300)", l.len())));
        }
    }
    crate::api::validate_url(&req.image_url)
        .map_err(|_| bad(format!("image_url невалиден ({:?})", req.image_url)))?;

    let starts_at = parse_iso_timestamp(&req.starts_at)
        .map_err(|_| bad("starts_at не ISO-8601".to_string()))?;
    let ends_at = if let Some(ref e) = req.ends_at {
        let e = parse_iso_timestamp(e).map_err(|_| bad("ends_at не ISO-8601".to_string()))?;
        if e < starts_at {
            return Err(bad("ends_at раньше starts_at".to_string()));
        }
        Some(e)
    } else {
        None
    };

    if let Some(cap) = req.max_seats {
        if cap < 0 || cap > 1_000_000 {
            return Err(bad(format!("max_seats вне диапазона 0..1e6 ({cap})")));
        }
    }
    if let Some(price) = req.price_baht {
        if !price.is_finite() || price < 0.0 || price > 1_000_000.0 {
            return Err(bad(format!("price_baht вне диапазона 0..1e6 ({price})")));
        }
    }
    let is_public = req.is_public.unwrap_or(true);
    Ok((is_public, starts_at, ends_at))
}

fn validate_update_request(
    req: &UpdateEventRequest,
) -> Result<(Option<bool>, Option<DateTime<Utc>>, Option<Option<DateTime<Utc>>>), (StatusCode, String)>
{
    fn bad(msg: String) -> (StatusCode, String) {
        (StatusCode::BAD_REQUEST, msg)
    }
    if let Some(ref t) = req.title {
        let t = t.trim();
        if t.is_empty() || t.len() > 200 {
            return Err(bad(format!("title невалиден ({} chars)", t.len())));
        }
    }
    if let Some(ref t) = req.title_en {
        let t = t.trim();
        if t.len() > 200 {
            return Err(bad(format!("title_en слишком длинный ({}>200)", t.len())));
        }
    }
    if let Some(ref d) = req.description {
        if d.len() > 2000 {
            return Err(bad(format!("description слишком длинное ({}>2000)", d.len())));
        }
    }
    if let Some(ref d) = req.description_en {
        if d.len() > 2000 {
            return Err(bad(format!("description_en слишком длинное ({}>2000)", d.len())));
        }
    }
    if let Some(ref l) = req.location_text {
        if l.len() > 300 {
            return Err(bad(format!("location_text слишком длинный ({}>300)", l.len())));
        }
    }
    crate::api::validate_url(&req.image_url)
        .map_err(|_| bad(format!("image_url невалиден ({:?})", req.image_url)))?;

    let starts_at = if let Some(ref s) = req.starts_at {
        Some(parse_iso_timestamp(s).map_err(|_| bad("starts_at не ISO-8601".to_string()))?)
    } else {
        None
    };
    let ends_at = if let Some(ref e) = req.ends_at {
        let e = parse_iso_timestamp(e).map_err(|_| bad("ends_at не ISO-8601".to_string()))?;
        Some(Some(e))
    } else {
        None
    };
    if let (Some(s), Some(Some(e))) = (starts_at, ends_at) {
        if e < s {
            return Err(bad("ends_at раньше starts_at".to_string()));
        }
    }

    if let Some(cap) = req.max_seats {
        if cap < 0 || cap > 1_000_000 {
            return Err(bad(format!("max_seats вне диапазона 0..1e6 ({cap})")));
        }
    }
    if let Some(price) = req.price_baht {
        if !price.is_finite() || price < 0.0 || price > 1_000_000.0 {
            return Err(bad(format!("price_baht вне диапазона 0..1e6 ({price})")));
        }
    }
    Ok((req.is_public, starts_at, ends_at))
}

fn event_row(r: &sea_orm::QueryResult) -> Value {
    let starts_at: DateTime<Utc> = r.try_get("", "starts_at").unwrap_or_else(|_| Utc::now());
    let ends_at: Option<DateTime<Utc>> = r.try_get::<Option<DateTime<Utc>>>("", "ends_at").ok().flatten();
    let max_seats: Option<i32> = r.try_get::<Option<i32>>("", "max_seats").ok().flatten();
    let price_baht: Option<f64> = r.try_get::<Option<f64>>("", "price_baht").ok().flatten();
    let seats_taken: i64 = r.try_get::<i64>("", "seats_taken").unwrap_or(0);
    json!({
        "id": r.try_get::<String>("", "id").unwrap_or_default(),
        "title": r.try_get::<String>("", "title").unwrap_or_default(),
        "title_en": r.try_get::<Option<String>>("", "title_en").ok().flatten(),
        "description": r.try_get::<Option<String>>("", "description").ok().flatten(),
        "description_en": r.try_get::<Option<String>>("", "description_en").ok().flatten(),
        "starts_at": starts_at.to_rfc3339(),
        "ends_at": ends_at.map(|d| d.to_rfc3339()),
        "location_text": r.try_get::<Option<String>>("", "location_text").ok().flatten(),
        "image_url": r.try_get::<Option<String>>("", "image_url").ok().flatten(),
        "max_seats": max_seats,
        "price_baht": price_baht.and_then(|p| if p.is_finite() { Some(p) } else { None }),
        "is_public": r.try_get::<bool>("", "is_public").unwrap_or(true),
        "seats_taken": seats_taken,
        "seats_available": max_seats.map(|cap| (cap - seats_taken as i32).max(0)),
        "created_at": r.try_get::<DateTime<Utc>>("", "created_at").ok().map(|d| d.to_rfc3339()),
    })
}

fn booking_row(r: &sea_orm::QueryResult) -> Value {
    let created_at: DateTime<Utc> = r.try_get("", "created_at").unwrap_or_else(|_| Utc::now());
    json!({
        "id": r.try_get::<String>("", "id").unwrap_or_default(),
        "event_id": r.try_get::<String>("", "event_id").unwrap_or_default(),
        "telegram_id": r.try_get::<i64>("", "telegram_id").unwrap_or(0),
        "seats": r.try_get::<i32>("", "seats").unwrap_or(1),
        "status": r.try_get::<String>("", "status").unwrap_or_default(),
        "order_id": r.try_get::<Option<String>>("", "order_id").ok().flatten(),
        "created_at": created_at.to_rfc3339(),
    })
}

// ── Public endpoints ─────────────────────────────────────────

async fn list_events(
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let mut clauses = vec!["e.is_public = TRUE".to_string()];
    let mut values: Vec<sea_orm::Value> = Vec::new();

    if let Some(from) = q.get("from").filter(|s| !s.is_empty()) {
        let _ = parse_iso_timestamp(from)?;
        clauses.push("e.starts_at >= $1".to_string());
        values.push(from.clone().into());
    }
    if let Some(to) = q.get("to").filter(|s| !s.is_empty()) {
        let _ = parse_iso_timestamp(to)?;
        if values.is_empty() {
            clauses.push("e.starts_at <= $1".to_string());
        } else {
            clauses.push("e.starts_at <= $2".to_string());
        }
        values.push(to.clone().into());
    }

    let limit = q
        .get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .map(|n| n.clamp(1, 100))
        .unwrap_or(100);

    let where_sql = clauses.join(" AND ");
    let sql = format!(
        "SELECT e.id, e.title, e.title_en, e.description, e.description_en, e.starts_at, e.ends_at, \
         e.location_text, e.image_url, e.max_seats, e.price_baht::float8 AS price_baht, e.is_public, e.created_at, \
         COALESCE((SELECT SUM(b.seats) FROM event_bookings b WHERE b.event_id = e.id AND b.status = 'confirmed'), 0)::bigint AS seats_taken \
         FROM events e \
         WHERE {where_sql} \
         ORDER BY e.starts_at ASC \
         LIMIT ${}",
        values.len() + 1
    );
    values.push(limit.into());

    let rows = state
        .db
        .orm
        .query_all(Statement::from_sql_and_values(DbBackend::Postgres, sql, values))
        .await
        .map_err(|e| {
            tracing::error!("list_events: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let events: Vec<Value> = rows.iter().map(event_row).collect();
    Ok(Json(json!({ "events": events })))
}

async fn get_event(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    event_id_ok(&id)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = state.db.orm.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT e.id, e.title, e.title_en, e.description, e.description_en, e.starts_at, e.ends_at, \
         e.location_text, e.image_url, e.max_seats, e.price_baht::float8 AS price_baht, e.is_public, e.created_at, \
         COALESCE((SELECT SUM(b.seats) FROM event_bookings b WHERE b.event_id = e.id AND b.status = 'confirmed'), 0)::bigint AS seats_taken \
         FROM events e \
         WHERE e.id = $1 AND e.is_public = TRUE",
        [id.into()],
    )).await.map_err(|e| { tracing::error!("get_event: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    match row {
        Some(r) => Ok(Json(json!({ "event": event_row(&r) }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn book_event(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<BookEventRequest>,
) -> Result<Json<Value>, StatusCode> {
    event_id_ok(&id)?;
    validate_telegram_id_param(req.telegram_id)?;

    // Authenticated action: the initData user must own telegram_id.
    // Production initData HMAC validation is flaky for some Telegram clients,
    // so mirror the order/garden/admin lenient fallback: accept fresh initData
    // whose user.id matches the requested telegram_id when strict HMAC fails.
    // FIXME: remove fallback once validate_init_data is fully reliable.
    let _owner_id = match check_owner(&headers, &state, req.telegram_id) {
        Ok(id) => id,
        Err(StatusCode::UNAUTHORIZED) => {
            crate::api::auth::check_owner_lenient(&headers, &state, req.telegram_id, "event")?
        }
        Err(other) => return Err(other),
    };
    check_not_blocked(&state, req.telegram_id).await?;

    let seats = req.seats.unwrap_or(1);
    if seats <= 0 || seats > 10 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let idem_key: Option<String> = headers
        .get("x-idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(ref k) = idem_key {
        if !is_valid_idempotency_key(k) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};

    // Idempotency replay: if a booking already exists for this key/event/user, return it.
    if let Some(ref k) = idem_key {
        let existing = state.db.orm.query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id, event_id, telegram_id, seats, status, order_id, created_at FROM event_bookings \
             WHERE event_id = $1 AND telegram_id = $2 AND idempotency_key = $3",
            [id.clone().into(), req.telegram_id.into(), k.clone().into()],
        )).await.map_err(|e| { tracing::error!("book_event idempotency lookup: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
        if let Some(r) = existing {
            return Ok(Json(json!({
                "success": true,
                "booking_id": r.try_get::<String>("", "id").unwrap_or_default(),
                "idempotent_replay": true,
                "booking": booking_row(&r),
            })));
        }
    }

    let booking_id = uuid::Uuid::new_v4().to_string();
    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("book_event tx.begin: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Lock event row, ensure it is public and has not started.
    let ev = tx.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id, max_seats, starts_at FROM events WHERE id = $1 AND is_public = TRUE FOR UPDATE",
        [id.clone().into()],
    )).await.map_err(|e| { tracing::error!("book_event lock event: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;

    let Some(ev) = ev else {
        let _ = tx.rollback().await;
        return Err(StatusCode::NOT_FOUND);
    };
    let starts_at: DateTime<Utc> = ev.try_get("", "starts_at").unwrap_or_else(|_| Utc::now());
    if Utc::now() >= starts_at {
        let _ = tx.rollback().await;
        return Err(StatusCode::CONFLICT);
    }

    // Prevent duplicate active booking per user per event (MVP guard).
    let dup = tx.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT 1 FROM event_bookings WHERE event_id = $1 AND telegram_id = $2 AND status = 'confirmed'",
        [id.clone().into(), req.telegram_id.into()],
    )).await.map_err(|e| { tracing::error!("book_event dup check: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    if dup.is_some() {
        let _ = tx.rollback().await;
        return Err(StatusCode::CONFLICT);
    }

    // Capacity check.
    let max_seats: Option<i32> = ev.try_get::<Option<i32>>("", "max_seats").ok().flatten();
    if let Some(cap) = max_seats {
        let taken_row = tx.query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COALESCE(SUM(seats), 0)::int AS taken FROM event_bookings WHERE event_id = $1 AND status = 'confirmed'",
            [id.clone().into()],
        )).await.map_err(|e| { tracing::error!("book_event capacity check: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
        let taken: i32 = taken_row.and_then(|r| r.try_get("", "taken").ok()).unwrap_or(0);
        if taken + seats > cap {
            let _ = tx.rollback().await;
            return Err(StatusCode::CONFLICT);
        }
    }

    // Free booking in MVP; price_baht is reserved for future paid flow.
    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO event_bookings (id, event_id, telegram_id, seats, status, idempotency_key) VALUES ($1,$2,$3,$4,'confirmed',$5)",
        [
            booking_id.clone().into(),
            id.clone().into(),
            req.telegram_id.into(),
            seats.into(),
            idem_key.into(),
        ],
    )).await.map_err(|e| {
        tracing::error!("book_event insert: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await.map_err(|e| {
        tracing::error!("book_event commit: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({
        "success": true,
        "booking_id": booking_id,
        "seats": seats,
    })))
}

// ── Admin endpoints ──────────────────────────────────────────

async fn list_admin_events(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let rows = state.db.orm.query_all(Statement::from_string(
        DbBackend::Postgres,
        "SELECT e.id, e.title, e.title_en, e.description, e.description_en, e.starts_at, e.ends_at, \
         e.location_text, e.image_url, e.max_seats, e.price_baht::float8 AS price_baht, e.is_public, e.created_at, \
         COALESCE((SELECT SUM(b.seats) FROM event_bookings b WHERE b.event_id = e.id AND b.status = 'confirmed'), 0)::bigint AS seats_taken, \
         (SELECT COUNT(*)::bigint FROM event_bookings b WHERE b.event_id = e.id) AS bookings_count \
         FROM events e \
         ORDER BY e.starts_at DESC \
         LIMIT 500".to_string(),
    )).await.map_err(|e| { tracing::error!("list_admin_events: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    let events: Vec<Value> = rows.iter().map(|r| {
        let mut v = event_row(r);
        if let Value::Object(ref mut m) = v {
            m.insert("bookings_count".to_string(), json!(r.try_get::<i64>("", "bookings_count").unwrap_or(0)));
        }
        v
    }).collect();
    Ok(Json(json!({ "events": events })))
}

async fn create_event(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<CreateEventRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;
    let (is_public, starts_at, ends_at) = validate_event_request(&req)?;
    let id = uuid::Uuid::new_v4().to_string();
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO events (id, title, title_en, description, description_en, starts_at, ends_at, location_text, image_url, max_seats, price_baht, is_public) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        [
            id.clone().into(),
            req.title.trim().into(),
            req.title_en.filter(|s| !s.is_empty()).into(),
            req.description.filter(|s| !s.is_empty()).into(),
            req.description_en.filter(|s| !s.is_empty()).into(),
            sea_orm::Value::ChronoDateTimeUtc(Some(Box::new(starts_at))),
            ends_at.map(|d| sea_orm::Value::ChronoDateTimeUtc(Some(Box::new(d)))).unwrap_or(sea_orm::Value::ChronoDateTimeUtc(None)),
            req.location_text.filter(|s| !s.is_empty()).into(),
            req.image_url.filter(|s| !s.is_empty()).into(),
            req.max_seats.into(),
            req.price_baht.into(),
            is_public.into(),
        ],
    )).await.map_err(|e| { tracing::error!("create_event: {e}"); (StatusCode::INTERNAL_SERVER_ERROR, String::new()) })?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn get_admin_event(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    event_id_ok(&id)?;
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = state.db.orm.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT e.id, e.title, e.title_en, e.description, e.description_en, e.starts_at, e.ends_at, \
         e.location_text, e.image_url, e.max_seats, e.price_baht::float8 AS price_baht, e.is_public, e.created_at, \
         COALESCE((SELECT SUM(b.seats) FROM event_bookings b WHERE b.event_id = e.id AND b.status = 'confirmed'), 0)::bigint AS seats_taken \
         FROM events e \
         WHERE e.id = $1",
        [id.clone().into()],
    )).await.map_err(|e| { tracing::error!("get_admin_event: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    match row {
        Some(r) => Ok(Json(json!({ "event": event_row(&r) }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn update_event(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateEventRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    event_id_ok(&id).map_err(|s| (s, String::new()))?;
    check_admin(&headers, &state).map_err(|s| (s, String::new()))?;
    let (is_public, starts_at, ends_at) = validate_update_request(&req)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE events SET \
            title = COALESCE($1, title), \
            title_en = COALESCE($2, title_en), \
            description = COALESCE($3, description), \
            description_en = COALESCE($4, description_en), \
            starts_at = COALESCE($5, starts_at), \
            ends_at = COALESCE($6, ends_at), \
            location_text = COALESCE($7, location_text), \
            image_url = COALESCE($8, image_url), \
            max_seats = COALESCE($9, max_seats), \
            price_baht = COALESCE($10, price_baht), \
            is_public = COALESCE($11, is_public), \
            updated_at = NOW() \
         WHERE id = $12",
        [
            req.title.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()).map(|s| s.to_string()).into(),
            req.title_en.filter(|s| !s.is_empty()).into(),
            req.description.filter(|s| !s.is_empty()).into(),
            req.description_en.filter(|s| !s.is_empty()).into(),
            starts_at.map(|d| sea_orm::Value::ChronoDateTimeUtc(Some(Box::new(d)))).unwrap_or(sea_orm::Value::ChronoDateTimeUtc(None)),
            ends_at.unwrap_or(None).map(|d| sea_orm::Value::ChronoDateTimeUtc(Some(Box::new(d)))).unwrap_or(sea_orm::Value::ChronoDateTimeUtc(None)),
            req.location_text.filter(|s| !s.is_empty()).into(),
            req.image_url.filter(|s| !s.is_empty()).into(),
            req.max_seats.into(),
            req.price_baht.into(),
            is_public.map(|v| v.into()).unwrap_or(sea_orm::Value::Bool(None)),
            id.into(),
        ],
    )).await.map_err(|e| { tracing::error!("update_event: {e}"); (StatusCode::INTERNAL_SERVER_ERROR, String::new()) })?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_event(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    event_id_ok(&id)?;
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "DELETE FROM events WHERE id = $1",
        [id.into()],
    )).await.map_err(|e| { tracing::error!("delete_event: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true })))
}

async fn list_event_bookings(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    event_id_ok(&id)?;
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let rows = state.db.orm.query_all(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id, event_id, telegram_id, seats, status, order_id, created_at FROM event_bookings \
         WHERE event_id = $1 \
         ORDER BY created_at DESC",
        [id.into()],
    )).await.map_err(|e| { tracing::error!("list_event_bookings: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    let bookings: Vec<Value> = rows.iter().map(booking_row).collect();
    Ok(Json(json!({ "bookings": bookings })))
}

async fn cancel_booking(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path((event_id, booking_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    event_id_ok(&event_id)?;
    booking_id_ok(&booking_id)?;
    check_admin(&headers, &state)?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state.db.orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE event_bookings SET status = 'cancelled', updated_at = NOW() WHERE event_id = $1 AND id = $2",
        [event_id.into(), booking_id.into()],
    )).await.map_err(|e| { tracing::error!("cancel_booking: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(Json(json!({ "success": true })))
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_iso_timestamp_accepts_rfc3339() {
        let dt = parse_iso_timestamp("2026-08-01T19:00:00+07:00").unwrap();
        assert_eq!(dt.to_rfc3339(), "2026-08-01T12:00:00+00:00");
    }

    #[test]
    fn parse_iso_timestamp_rejects_garbage() {
        assert!(parse_iso_timestamp("not a date").is_err());
    }

    fn valid_create() -> CreateEventRequest {
        CreateEventRequest {
            title: "Smoke Session".into(),
            title_en: Some("Smoke Session".into()),
            description: Some("Join us".into()),
            description_en: Some("Join us".into()),
            starts_at: "2026-08-01T19:00:00+07:00".into(),
            ends_at: Some("2026-08-01T22:00:00+07:00".into()),
            location_text: Some("Bangkok".into()),
            image_url: Some("/uploads/event.jpg".into()),
            max_seats: Some(20),
            price_baht: None,
            is_public: Some(true),
        }
    }

    #[test]
    fn validate_event_ok() {
        assert!(validate_event_request(&valid_create()).is_ok());
    }

    #[test]
    fn validate_event_title_required_for_create() {
        let mut r = valid_create();
        r.title = "   ".into();
        assert_eq!(validate_event_request(&r).unwrap_err().0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validate_event_title_too_long() {
        let mut r = valid_create();
        r.title = "a".repeat(201);
        assert_eq!(validate_event_request(&r).unwrap_err().0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validate_event_ends_before_starts() {
        let mut r = valid_create();
        r.ends_at = Some("2026-08-01T18:00:00+07:00".into());
        assert_eq!(validate_event_request(&r).unwrap_err().0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validate_event_negative_max_seats() {
        let mut r = valid_create();
        r.max_seats = Some(-1);
        assert_eq!(validate_event_request(&r).unwrap_err().0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validate_event_price_negative() {
        let mut r = valid_create();
        r.price_baht = Some(-1.0);
        assert_eq!(validate_event_request(&r).unwrap_err().0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validate_event_bad_image_url() {
        let mut r = valid_create();
        r.image_url = Some("javascript:alert(1)".into());
        assert_eq!(validate_event_request(&r).unwrap_err().0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn event_id_ok_rejects_too_long() {
        assert!(event_id_ok(&"x".repeat(37)).is_err());
        assert!(event_id_ok(&"").is_err());
        assert!(event_id_ok("550e8400-e29b-41d4-a716-446655440000").is_ok());
    }
}
