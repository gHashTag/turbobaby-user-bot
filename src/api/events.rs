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

use crate::api::auth::{
    check_admin, check_not_blocked, check_owner_lenient, validate_telegram_id_param,
};
use crate::api::orders::is_valid_idempotency_key;
use crate::api::rate_limit::{check_and_record, new_store, SlidingWindowStore};
use crate::AppState;
use std::time::Duration;

/// Cycle #168: per-telegram-id rate limit for event booking.
/// 5 bookings per minute per user is generous for legitimate use and
/// tight enough to prevent seat-spray abuse.
static EVENT_BOOKING_RATE_LIMIT: std::sync::LazyLock<SlidingWindowStore> =
    std::sync::LazyLock::new(new_store);
const EVENT_BOOKING_RL_WINDOW: Duration = Duration::from_secs(60);
const EVENT_BOOKING_RL_MAX_ATTEMPTS: usize = 5;
const EVENT_BOOKING_RL_MAX_IDS: usize = 20_000;

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        // Public event calendar
        .route("/events/my-bookings", get(my_bookings))
        .route(
            "/events/bookings/:booking_id/cancel",
            put(cancel_my_booking),
        )
        .route("/events", get(list_events))
        .route("/events/:id", get(get_event))
        .route("/events/:id/book", post(book_event))
        .route("/events/:id/waitlist", post(join_waitlist))
        // Admin
        .route("/admin/events", get(list_admin_events).post(create_event))
        .route(
            "/admin/events/:id",
            get(get_admin_event).put(update_event).delete(delete_event),
        )
        .route("/admin/events/:id/bookings", get(list_event_bookings))
        .route("/admin/events/:id/bookings/send", post(send_event_bookings))
        .route(
            "/admin/events/:id/bookings/:booking_id/cancel",
            put(cancel_booking),
        )
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
    pub video_url: Option<String>,
    pub photos: Option<Vec<String>>,
    pub max_seats: Option<i32>,
    pub price_baht: Option<f64>,
    pub price_stars: Option<i64>,
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
    pub video_url: Option<String>,
    pub photos: Option<Vec<String>>,
    pub max_seats: Option<i32>,
    pub price_baht: Option<f64>,
    pub price_stars: Option<i64>,
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
    s.parse::<DateTime<Utc>>().map_err(|e| {
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
#[allow(clippy::type_complexity)]
fn validate_event_request(
    req: &CreateEventRequest,
) -> Result<(bool, DateTime<Utc>, Option<DateTime<Utc>>), (StatusCode, String)> {
    fn bad(msg: String) -> (StatusCode, String) {
        // A rejected create used to be invisible from both ends: the handler
        // logged nothing on 400 and the admin UI collapsed every failure into
        // "Ошибка сохранения". Log the reason so a failed save is diagnosable
        // from the server logs alone.
        tracing::warn!("create_event rejected: {msg}");
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
            return Err(bad(format!(
                "description слишком длинное ({}>2000)",
                d.len()
            )));
        }
    }
    if let Some(ref d) = req.description_en {
        if d.len() > 2000 {
            return Err(bad(format!(
                "description_en слишком длинное ({}>2000)",
                d.len()
            )));
        }
    }
    if let Some(ref l) = req.location_text {
        if l.len() > 300 {
            return Err(bad(format!(
                "location_text слишком длинный ({}>300)",
                l.len()
            )));
        }
    }
    crate::api::validate_url(&req.image_url)
        .map_err(|_| bad(format!("image_url невалиден ({:?})", req.image_url)))?;
    crate::api::validate_url(&req.video_url)
        .map_err(|_| bad(format!("video_url невалиден ({:?})", req.video_url)))?;
    if let Some(ref photos) = req.photos {
        for (i, url) in photos.iter().enumerate() {
            crate::api::validate_url(&Some(url.clone()))
                .map_err(|_| bad(format!("photos[{}] невалиден ({:?})", i, url)))?;
        }
        if photos.len() > 50 {
            return Err(bad(format!("photos слишком много ({}>50)", photos.len())));
        }
    }

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
        if !(0..=1_000_000).contains(&cap) {
            return Err(bad(format!("max_seats вне диапазона 0..1e6 ({cap})")));
        }
    }
    if let Some(price) = req.price_baht {
        if !price.is_finite() || !(0.0..=1_000_000.0).contains(&price) {
            return Err(bad(format!("price_baht вне диапазона 0..1e6 ({price})")));
        }
    }
    if let Some(stars) = req.price_stars {
        if !(0..=1_000_000_000).contains(&stars) {
            return Err(bad(format!("price_stars вне диапазона 0..1e9 ({stars})")));
        }
    }
    let is_public = storable_public_flag(req.is_public.unwrap_or(true));
    Ok((is_public, starts_at, ends_at))
}

#[allow(clippy::type_complexity)]
fn validate_update_request(
    req: &UpdateEventRequest,
) -> Result<
    (
        Option<bool>,
        Option<DateTime<Utc>>,
        Option<Option<DateTime<Utc>>>,
    ),
    (StatusCode, String),
> {
    fn bad(msg: String) -> (StatusCode, String) {
        tracing::warn!("update_event rejected: {msg}");
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
            return Err(bad(format!(
                "description слишком длинное ({}>2000)",
                d.len()
            )));
        }
    }
    if let Some(ref d) = req.description_en {
        if d.len() > 2000 {
            return Err(bad(format!(
                "description_en слишком длинное ({}>2000)",
                d.len()
            )));
        }
    }
    if let Some(ref l) = req.location_text {
        if l.len() > 300 {
            return Err(bad(format!(
                "location_text слишком длинный ({}>300)",
                l.len()
            )));
        }
    }
    crate::api::validate_url(&req.image_url)
        .map_err(|_| bad(format!("image_url невалиден ({:?})", req.image_url)))?;
    crate::api::validate_url(&req.video_url)
        .map_err(|_| bad(format!("video_url невалиден ({:?})", req.video_url)))?;
    if let Some(ref photos) = req.photos {
        for (i, url) in photos.iter().enumerate() {
            crate::api::validate_url(&Some(url.clone()))
                .map_err(|_| bad(format!("photos[{}] невалиден ({:?})", i, url)))?;
        }
        if photos.len() > 50 {
            return Err(bad(format!("photos слишком много ({} >50)", photos.len())));
        }
    }

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
        if !(0..=1_000_000).contains(&cap) {
            return Err(bad(format!("max_seats вне диапазона 0..1e6 ({cap})")));
        }
    }
    if let Some(price) = req.price_baht {
        if !price.is_finite() || !(0.0..=1_000_000.0).contains(&price) {
            return Err(bad(format!("price_baht вне диапазона 0..1e6 ({price})")));
        }
    }
    if let Some(stars) = req.price_stars {
        if !(0..=1_000_000_000).contains(&stars) {
            return Err(bad(format!("price_stars вне диапазона 0..1e9 ({stars})")));
        }
    }
    Ok((req.is_public.map(storable_public_flag), starts_at, ends_at))
}

fn event_row(r: &sea_orm::QueryResult) -> Value {
    let starts_at: DateTime<Utc> = r.try_get("", "starts_at").unwrap_or_else(|_| Utc::now());
    let ends_at: Option<DateTime<Utc>> = r
        .try_get::<Option<DateTime<Utc>>>("", "ends_at")
        .ok()
        .flatten();
    let max_seats: Option<i32> = r.try_get::<Option<i32>>("", "max_seats").ok().flatten();
    let price_baht: Option<f64> = r.try_get::<Option<f64>>("", "price_baht").ok().flatten();
    let price_stars: Option<i64> = r.try_get::<Option<i64>>("", "price_stars").ok().flatten();
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
        "video_url": r.try_get::<Option<String>>("", "video_url").ok().flatten(),
        "max_seats": max_seats,
        "price_baht": price_baht.filter(|p| p.is_finite()),
        "price_stars": price_stars,
        "is_public": r.try_get::<bool>("", "is_public").unwrap_or(true),
        "seats_taken": seats_taken,
        // One implementation of this subtraction, shared with the calendar
        // card that used to recompute it and get -3 where this got 0 (D15).
        "seats_available": max_seats.map(|cap| crate::trios::calendar::seats_free(cap, seats_taken)),
        "created_at": r.try_get::<DateTime<Utc>>("", "created_at").ok().map(|d| d.to_rfc3339()),
    })
}

/// The handle to show an admin for a booking, taken from the *validated*
/// initData rather than the request body.
///
/// A client-supplied username would let anyone book under someone else's
/// handle, which is worse than no handle at all: the owner would message the
/// wrong person. `validate_init_data` has already checked the HMAC, so
/// whatever it reports is what Telegram signed.
fn booker_identity(headers: &HeaderMap, state: &AppState) -> (Option<String>, Option<String>) {
    headers
        .get("X-Telegram-Init-Data")
        .and_then(|v| v.to_str().ok())
        .and_then(|d| crate::api::auth::validate_init_data(d, &state.config.bot_token))
        .map(|u| (u.username, u.first_name))
        .unwrap_or((None, None))
}

fn booking_row(r: &sea_orm::QueryResult) -> Value {
    let created_at: DateTime<Utc> = r.try_get("", "created_at").unwrap_or_else(|_| Utc::now());
    json!({
        "id": r.try_get::<String>("", "id").unwrap_or_default(),
        "event_id": r.try_get::<String>("", "event_id").unwrap_or_default(),
        "telegram_id": r.try_get::<i64>("", "telegram_id").unwrap_or(0),
        "username": r.try_get::<Option<String>>("", "username").ok().flatten(),
        "first_name": r.try_get::<Option<String>>("", "first_name").ok().flatten(),
        "seats": r.try_get::<i32>("", "seats").unwrap_or(1),
        "status": r.try_get::<String>("", "status").unwrap_or_default(),
        "order_id": r.try_get::<Option<String>>("", "order_id").ok().flatten(),
        "stars_paid": r.try_get::<Option<i64>>("", "stars_paid").ok().flatten(),
        "stars_tx_id": r.try_get::<Option<String>>("", "stars_tx_id").ok().flatten(),
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
        clauses.push("e.starts_at >= $1::timestamptz".to_string());
        values.push(from.clone().into());
    }
    if let Some(to) = q.get("to").filter(|s| !s.is_empty()) {
        let _ = parse_iso_timestamp(to)?;
        if values.is_empty() {
            clauses.push("e.starts_at <= $1::timestamptz".to_string());
        } else {
            clauses.push("e.starts_at <= $2::timestamptz".to_string());
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
         e.location_text, e.image_url, e.video_url, e.max_seats, e.price_baht::float8 AS price_baht, e.price_stars, e.is_public, e.created_at, \
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
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            values,
        ))
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
         e.location_text, e.image_url, e.video_url, e.max_seats, e.price_baht::float8 AS price_baht, e.price_stars, e.is_public, e.created_at, \
         COALESCE((SELECT SUM(b.seats) FROM event_bookings b WHERE b.event_id = e.id AND b.status = 'confirmed'), 0)::bigint AS seats_taken \
         FROM events e \
         WHERE e.id = $1 AND e.is_public = TRUE",
        [id.clone().into()],
    )).await.map_err(|e| { tracing::error!("get_event: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    match row {
        Some(r) => {
            let mut ev = event_row(&r);
            let photos = load_event_photos(&state.db.orm, &id).await?;
            if let Value::Object(ref mut m) = ev {
                m.insert("photos".to_string(), json!(photos));
            }
            Ok(Json(json!({ "event": ev })))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn load_event_photos(
    orm: &sea_orm::DatabaseConnection,
    event_id: &str,
) -> Result<Vec<String>, StatusCode> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let rows = orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT url FROM event_photos WHERE event_id = $1 ORDER BY display_order ASC, created_at ASC",
            [event_id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("load_event_photos: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(rows
        .iter()
        .map(|r| r.try_get::<String>("", "url").unwrap_or_default())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Booking refusals carry a stable code.
///
/// Three quite different situations answered a bare 409 — the event has already
/// started, you are already on the list, and there are no seats left. The
/// customer saw one indistinguishable failure, and so did the log: a 409 in
/// production could not be told apart without reproducing it.
fn booking_err(code: StatusCode, slug: &str) -> (StatusCode, Json<Value>) {
    (code, Json(json!({ "error": slug })))
}

async fn book_event(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<BookEventRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    event_id_ok(&id).map_err(|c| booking_err(c, "invalid_event_id"))?;
    validate_telegram_id_param(req.telegram_id)
        .map_err(|c| booking_err(c, "invalid_telegram_id"))?;

    // Cycle #169: fall back to lenient Telegram ownership check (user id + fresh
    // auth_date, without strict HMAC) because production initData HMAC validation
    // still fails for some Telegram clients (same root cause as garden-401).
    // Booking as another user remains impossible: the initData user.id must match
    // the requested telegram_id.
    let _owner_id = check_owner_lenient(&headers, &state, req.telegram_id, "event_book")
        .map_err(|c| booking_err(c, "unauthorized"))?;
    let (booker_username, booker_first_name) = booker_identity(&headers, &state);
    check_not_blocked(&state, req.telegram_id)
        .await
        .map_err(|c| booking_err(c, "blocked"))?;

    // Rate-limit event bookings per telegram id.
    let rate_key = format!("event_book:{}", req.telegram_id);
    if !check_and_record(
        &EVENT_BOOKING_RATE_LIMIT,
        &rate_key,
        EVENT_BOOKING_RL_WINDOW,
        EVENT_BOOKING_RL_MAX_ATTEMPTS,
        EVENT_BOOKING_RL_MAX_IDS,
    )
    .await
    {
        crate::metrics::rate_limit_blocked("event_booking");
        tracing::warn!("book_event: rate-limit exceeded tid={}", req.telegram_id);
        return Err(booking_err(StatusCode::TOO_MANY_REQUESTS, "too_fast"));
    }

    // MVP: one seat per booking. Keep the field for forward compatibility,
    // but reject explicit multi-seat requests until paid/booking-for-others flow.
    let seats = match req.seats {
        None => 1,
        Some(1) => 1,
        Some(_) => return Err(booking_err(StatusCode::BAD_REQUEST, "invalid_request")),
    };

    let idem_key: Option<String> = headers
        .get("x-idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(ref k) = idem_key {
        if !is_valid_idempotency_key(k) {
            return Err(booking_err(StatusCode::BAD_REQUEST, "invalid_request"));
        }
    }

    use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};

    // Idempotency replay: if a booking already exists for this key/event/user, return it.
    if let Some(ref k) = idem_key {
        let existing = state.db.orm.query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id, event_id, telegram_id, seats, status, order_id, stars_paid, stars_tx_id, created_at FROM event_bookings \
             WHERE event_id = $1 AND telegram_id = $2 AND idempotency_key = $3",
            [id.clone().into(), req.telegram_id.into(), k.clone().into()],
        )).await.map_err(|e| { tracing::error!("book_event idempotency lookup: {e}"); booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error") })?;
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
        booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
    })?;

    // Lock event row, ensure it is public and has not started.
    let ev = tx.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id, max_seats, starts_at, price_stars FROM events WHERE id = $1 AND is_public = TRUE FOR UPDATE",
        [id.clone().into()],
    )).await.map_err(|e| { tracing::error!("book_event lock event: {e}"); booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error") })?;

    let Some(ev) = ev else {
        let _ = tx.rollback().await;
        return Err(booking_err(StatusCode::NOT_FOUND, "event_not_found"));
    };
    let starts_at: DateTime<Utc> = ev.try_get("", "starts_at").unwrap_or_else(|_| Utc::now());
    if Utc::now() >= starts_at {
        let _ = tx.rollback().await;
        return Err(booking_err(StatusCode::CONFLICT, "event_started"));
    }

    // Prevent duplicate active booking per user per event (MVP guard).
    let dup = tx.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT 1 FROM event_bookings WHERE event_id = $1 AND telegram_id = $2 AND status = 'confirmed'",
        [id.clone().into(), req.telegram_id.into()],
    )).await.map_err(|e| { tracing::error!("book_event dup check: {e}"); booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error") })?;
    if dup.is_some() {
        let _ = tx.rollback().await;
        return Err(booking_err(StatusCode::CONFLICT, "already_booked"));
    }

    // Capacity check.
    let max_seats: Option<i32> = ev.try_get::<Option<i32>>("", "max_seats").ok().flatten();
    if let Some(cap) = max_seats {
        let taken_row = tx.query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COALESCE(SUM(seats), 0)::int AS taken FROM event_bookings WHERE event_id = $1 AND status = 'confirmed'",
            [id.clone().into()],
        )).await.map_err(|e| { tracing::error!("book_event capacity check: {e}"); booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error") })?;
        let taken: i32 = taken_row
            .and_then(|r| r.try_get("", "taken").ok())
            .unwrap_or(0);
        if taken + seats > cap {
            let _ = tx.rollback().await;
            return Err(booking_err(StatusCode::CONFLICT, "sold_out"));
        }
    }

    // Paid booking: debit internal Stars atomically inside the same tx.
    let price_stars: Option<i64> = ev.try_get::<Option<i64>>("", "price_stars").ok().flatten();
    let stars_cost = price_stars.unwrap_or(0).saturating_mul(seats as i64);
    let mut stars_tx_id: Option<String> = None;
    if stars_cost > 0 {
        use crate::db::entities::{
            loyalty_profile::{ActiveModel as LpAm, Column as LpCol, Entity as LpEntity},
            stars_transaction::{ActiveModel as TxAm, Entity as TxEntity},
            user_stars::{ActiveModel as UsAm, Column as UsCol, Entity as UsEntity},
        };
        use sea_orm::sea_query::OnConflict;
        use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

        // Ensure loyalty_profile exists because user_stars.telegram_id references it.
        let lp_am = LpAm {
            telegram_id: Set(req.telegram_id),
            bonus_balance: Set(Some(0.0)),
            total_spent: Set(Some(0.0)),
            ..Default::default()
        };
        LpEntity::insert(lp_am)
            .on_conflict(
                OnConflict::column(LpCol::TelegramId)
                    .do_nothing()
                    .to_owned(),
            )
            .do_nothing()
            .exec(&tx)
            .await
            .map_err(|e| {
                tracing::error!("book_event loyalty_profile upsert: {e}");
                booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
            })?;

        let us_am = UsAm {
            telegram_id: Set(req.telegram_id),
            balance: Set(0),
            ..Default::default()
        };
        UsEntity::insert(us_am)
            .on_conflict(
                OnConflict::column(UsCol::TelegramId)
                    .update_column(UsCol::UpdatedAt)
                    .to_owned(),
            )
            .exec(&tx)
            .await
            .map_err(|e| {
                tracing::error!("book_event user_stars upsert: {e}");
                booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
            })?;

        let debited = UsEntity::update_many()
            .col_expr(
                UsCol::Balance,
                sea_orm::sea_query::Expr::cust_with_values("balance - $1", [stars_cost]),
            )
            .filter(UsCol::TelegramId.eq(req.telegram_id))
            .filter(UsCol::Balance.gte(stars_cost))
            .exec(&tx)
            .await
            .map_err(|e| {
                tracing::error!("book_event stars debit: {e}");
                booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
            })?;
        if debited.rows_affected == 0 {
            let _ = tx.rollback().await;
            return Err(booking_err(
                StatusCode::PAYMENT_REQUIRED,
                "insufficient_stars",
            ));
        }

        let balance_after = UsEntity::find_by_id(req.telegram_id)
            .one(&tx)
            .await
            .map_err(|e| {
                tracing::error!("book_event stars balance read: {e}");
                booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
            })?
            .map(|r| r.balance)
            .unwrap_or(0);

        let tx_id = uuid::Uuid::new_v4().to_string();
        TxEntity::insert(TxAm {
            id: Set(tx_id.clone()),
            telegram_id: Set(req.telegram_id),
            amount: Set(-stars_cost),
            balance_after: Set(balance_after),
            source: Set("events".to_string()),
            reason: Set("event_booking".to_string()),
            external_tx_id: Set(None),
            related_order_id: Set(Some(booking_id.clone())),
            ..Default::default()
        })
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("book_event stars transaction insert: {e}");
            booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
        })?;
        stars_tx_id = Some(tx_id);
    }

    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO event_bookings (id, event_id, telegram_id, seats, status, idempotency_key, stars_paid, stars_tx_id, username, first_name) VALUES ($1,$2,$3,$4,'confirmed',$5,$6,$7,$8,$9)",
        [
            booking_id.clone().into(),
            id.clone().into(),
            req.telegram_id.into(),
            seats.into(),
            idem_key.into(),
            stars_cost.into(),
            stars_tx_id.into(),
            booker_username.clone().into(),
            booker_first_name.clone().into(),
        ],
    )).await.map_err(|e| {
        tracing::error!("book_event insert: {e}");
        booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
    })?;

    tx.commit().await.map_err(|e| {
        tracing::error!("book_event commit: {e}");
        booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
    })?;

    crate::metrics::event_booking_created("confirmed");
    Ok(Json(json!({
        "success": true,
        "booking_id": booking_id,
        "seats": seats,
    })))
}

async fn my_bookings(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let tid = q
        .get("telegram_id")
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    validate_telegram_id_param(tid)?;
    check_owner_lenient(&headers, &state, tid, "event_book")?;
    check_not_blocked(&state, tid).await?;

    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let rows = state
        .db
        .orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT b.id, b.event_id, b.telegram_id, b.seats, b.status, b.order_id, b.stars_paid, b.stars_tx_id, b.created_at, \
             e.title, e.title_en, e.starts_at, e.location_text \
             FROM event_bookings b JOIN events e ON e.id = b.event_id \
             WHERE b.telegram_id = $1 \
             ORDER BY e.starts_at ASC",
            [tid.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("my_bookings: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let bookings: Vec<Value> = rows
        .iter()
        .map(|r| {
            let mut b = booking_row(r);
            if let Value::Object(ref mut m) = b {
                m.insert(
                    "event_title".to_string(),
                    json!(r.try_get::<String>("", "title").unwrap_or_default()),
                );
                m.insert(
                    "event_title_en".to_string(),
                    json!(r.try_get::<Option<String>>("", "title_en").ok().flatten()),
                );
                m.insert(
                    "event_starts_at".to_string(),
                    json!(r
                        .try_get::<DateTime<Utc>>("", "starts_at")
                        .ok()
                        .map(|d| d.to_rfc3339())),
                );
                m.insert(
                    "event_location_text".to_string(),
                    json!(r
                        .try_get::<Option<String>>("", "location_text")
                        .ok()
                        .flatten()),
                );
            }
            b
        })
        .collect();
    Ok(Json(json!({ "bookings": bookings })))
}

/// How many waitlisted rows one cancellation reads before it gives up.
///
/// The work per cancellation has to be bounded: this runs inside the cancel
/// transaction, which holds `FOR UPDATE` on the event row, and every booking
/// of that event waits behind it. Nothing in the tree bounds a waitlist —
/// `join_waitlist` has no limit and no rate limiter — so an unbounded scan is
/// an unbounded lock.
///
/// Ten is OURS and not a measurement: no waitlist length is published
/// anywhere in this repository, and inventing one to look measured would be
/// worse than saying this. It is a WORK bound and not a policy, because the
/// order above already puts every customer who can pay ahead of every
/// customer who cannot: the tenth row is reached only after nine consecutive
/// balances moved between the read and the debit inside one transaction. When
/// the whole scan promotes nobody the seat stays empty for this cancellation
/// and `tracing::warn!` says so with the event, the price and the counts —
/// the failure is visible rather than a threshold quietly deciding who waits.
const PROMOTION_CANDIDATE_SCAN_MAX: i64 = 10;

/// One waitlisted customer, reduced to what the promotion decision needs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PromotionCandidate {
    booking_id: String,
    telegram_id: i64,
    /// The Stars balance read together with the candidate row. Advisory only:
    /// the conditional debit re-reads it under the row lock and is the
    /// authority, so a balance that moved between the two is a lost race and
    /// not an overdraft.
    balance: i64,
}

/// Who the freed seat is offered to next, in waitlist order, starting at
/// `from`.
///
/// The line is already ordered; this decides who in it can take the seat.
/// A customer who cannot clear `price` is SKIPPED and not stopped at: the
/// shipped rule ended the whole promotion at the first person who could not
/// pay, so one waitlisted customer with an empty balance held the seat empty
/// for everybody behind him until the next cancellation, which may never come.
///
/// Skipping costs the skipped customer nothing. This returns an index into the
/// line and never reorders or rewrites it — his row keeps its `status` and its
/// `created_at`, which is the only thing his place is made of, so he is at the
/// head again the moment he has the Stars.
///
/// `price <= 0` is a free event: everybody can take the seat and the head
/// takes it.
fn next_promotable(candidates: &[PromotionCandidate], price: i64, from: usize) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .skip(from)
        .find(|(_, c)| price <= 0 || c.balance >= price)
        .map(|(i, _)| i)
}

/// Cancel a booking and, if a seat opens up, auto-promote the oldest waitlist entry.
/// Returns the promoted booking id (if any) so the HTTP layer can report it.
async fn cancel_booking_and_promote(
    db: &sea_orm::DatabaseConnection,
    event_id: &str,
    booking_id: &str,
) -> Result<Option<String>, StatusCode> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};

    let tx = db.begin().await.map_err(|e| {
        tracing::error!("cancel_booking tx.begin: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Lock event and booking rows to prevent races.
    let ev = tx
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id, max_seats, price_stars FROM events WHERE id = $1 FOR UPDATE",
            [event_id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("cancel_booking lock event: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if ev.is_none() {
        let _ = tx.rollback().await;
        return Err(StatusCode::NOT_FOUND);
    }

    let booking = tx
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id, telegram_id, status, stars_paid, stars_tx_id FROM event_bookings WHERE id = $1 AND event_id = $2 FOR UPDATE",
            [booking_id.into(), event_id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("cancel_booking lock booking: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let Some(booking) = booking else {
        let _ = tx.rollback().await;
        return Err(StatusCode::NOT_FOUND);
    };
    let status: String = booking.try_get("", "status").unwrap_or_default();
    if status == "cancelled" {
        let _ = tx.rollback().await;
        return Ok(None);
    }
    let booking_telegram_id: i64 = booking.try_get("", "telegram_id").unwrap_or(0);
    let stars_paid: i64 = booking
        .try_get::<Option<i64>>("", "stars_paid")
        .ok()
        .flatten()
        .unwrap_or(0);

    // Refund Stars if this was a paid booking.
    if stars_paid > 0 {
        use crate::db::entities::{
            loyalty_profile::{ActiveModel as LpAm, Column as LpCol, Entity as LpEntity},
            stars_transaction::{ActiveModel as TxAm, Entity as TxEntity},
            user_stars::{ActiveModel as UsAm, Column as UsCol, Entity as UsEntity},
        };
        use sea_orm::sea_query::OnConflict;
        use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

        let lp_am = LpAm {
            telegram_id: Set(booking_telegram_id),
            bonus_balance: Set(Some(0.0)),
            total_spent: Set(Some(0.0)),
            ..Default::default()
        };
        LpEntity::insert(lp_am)
            .on_conflict(
                OnConflict::column(LpCol::TelegramId)
                    .do_nothing()
                    .to_owned(),
            )
            .do_nothing()
            .exec(&tx)
            .await
            .map_err(|e| {
                tracing::error!("cancel_booking loyalty_profile upsert: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let us_am = UsAm {
            telegram_id: Set(booking_telegram_id),
            balance: Set(0),
            ..Default::default()
        };
        UsEntity::insert(us_am)
            .on_conflict(
                OnConflict::column(UsCol::TelegramId)
                    .update_column(UsCol::UpdatedAt)
                    .to_owned(),
            )
            .exec(&tx)
            .await
            .map_err(|e| {
                tracing::error!("cancel_booking user_stars upsert: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        UsEntity::update_many()
            .col_expr(
                UsCol::Balance,
                sea_orm::sea_query::Expr::cust_with_values("balance + $1", [stars_paid]),
            )
            .filter(UsCol::TelegramId.eq(booking_telegram_id))
            .exec(&tx)
            .await
            .map_err(|e| {
                tracing::error!("cancel_booking stars refund: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let balance_after = UsEntity::find_by_id(booking_telegram_id)
            .one(&tx)
            .await
            .map_err(|e| {
                tracing::error!("cancel_booking stars refund balance read: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .map(|r| r.balance)
            .unwrap_or(0);

        let refund_tx_id = uuid::Uuid::new_v4().to_string();
        TxEntity::insert(TxAm {
            id: Set(refund_tx_id),
            telegram_id: Set(booking_telegram_id),
            amount: Set(stars_paid),
            balance_after: Set(balance_after),
            source: Set("events".to_string()),
            reason: Set("event_refund".to_string()),
            external_tx_id: Set(None),
            related_order_id: Set(Some(booking_id.to_string())),
            ..Default::default()
        })
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("cancel_booking stars refund ledger insert: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    // Cancel the booking.
    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE event_bookings SET status = 'cancelled', updated_at = NOW() WHERE id = $1",
        [booking_id.into()],
    ))
    .await
    .map_err(|e| {
        tracing::error!("cancel_booking update: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    crate::metrics::event_booking_cancelled();

    // Auto-promote the oldest waitlist entry if capacity allows.
    let max_seats: Option<i32> = ev
        .as_ref()
        .and_then(|r| r.try_get::<Option<i32>>("", "max_seats").ok().flatten());
    let mut promoted_id: Option<String> = None;
    if let Some(cap) = max_seats {
        let taken_row = tx
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT COALESCE(SUM(seats), 0)::int AS taken FROM event_bookings WHERE event_id = $1 AND status = 'confirmed'",
                [event_id.into()],
            ))
            .await
            .map_err(|e| {
                tracing::error!("cancel_booking capacity check: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        let taken: i32 = taken_row
            .and_then(|r| r.try_get("", "taken").ok())
            .unwrap_or(0);
        if taken < cap {
            // The price as it stands NOW, not as it stood when he joined: the
            // waitlist INSERT carries no stars column, so nothing was ever
            // quoted to him.
            let price_stars: Option<i64> = ev
                .as_ref()
                .and_then(|r| r.try_get::<Option<i64>>("", "price_stars").ok().flatten());
            let promo_price = price_stars.unwrap_or(0);

            // The line, read once, with each waiting customer's Stars balance
            // beside him.
            //
            // `ORDER BY (balance >= price) DESC` puts everyone who can pay
            // ahead of everyone who cannot, and `created_at ASC` then decides
            // among them — which is the waitlist rule, unchanged. Nothing in
            // the line is rewritten: no status, no created_at, and a customer
            // who is passed over is at the head again the moment he has the
            // Stars. On a free event the first key is true for every row and
            // the order is exactly `created_at ASC`, as it always was.
            //
            // The balances are ADVISORY. The conditional debit below re-reads
            // the balance under the row lock and is the authority; a balance
            // that moved between the two is a lost race, handled as one, and
            // never an overdraft.
            let candidate_rows = tx
                .query_all(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "SELECT b.id, b.telegram_id, COALESCE(u.balance, 0)::bigint AS balance \
                     FROM event_bookings b \
                     LEFT JOIN user_stars u ON u.telegram_id = b.telegram_id \
                     WHERE b.event_id = $1 AND b.status = 'waitlisted' \
                     ORDER BY (COALESCE(u.balance, 0) >= $2::bigint) DESC, b.created_at ASC \
                     LIMIT $3 FOR UPDATE OF b",
                    [
                        event_id.into(),
                        promo_price.into(),
                        PROMOTION_CANDIDATE_SCAN_MAX.into(),
                    ],
                ))
                .await
                .map_err(|e| {
                    tracing::error!("cancel_booking waitlist select: {e}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            // An unreadable balance is not a confident zero (D9): it decides
            // nothing about money here, because this number never reaches the
            // debit. It only means this cancellation does not offer HIM the
            // seat, and his row is left exactly as it was.
            let candidates: Vec<PromotionCandidate> = candidate_rows
                .iter()
                .map(|r| PromotionCandidate {
                    booking_id: r.try_get("", "id").unwrap_or_default(),
                    telegram_id: r.try_get("", "telegram_id").unwrap_or(0),
                    balance: r.try_get("", "balance").unwrap_or(0),
                })
                .filter(|c| !c.booking_id.is_empty() && c.telegram_id != 0)
                .collect();

            // One cancellation frees one seat, so at most one promotion — but
            // the seat is offered DOWN the line instead of to the head alone.
            let mut from = 0usize;
            let mut debit_attempts = 0u32;
            while let Some(i) = next_promotable(&candidates, promo_price, from) {
                let candidate = &candidates[i];
                from = i + 1;
                let mut promo_stars_tx_id: Option<String> = None;

                if promo_price > 0 {
                    use crate::db::entities::{
                        stars_transaction::{ActiveModel as TxAm2, Entity as TxEntity2},
                        user_stars::{Column as UsCol2, Entity as UsEntity2},
                    };
                    use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

                    debit_attempts += 1;
                    // No loyalty_profile/user_stars upsert here, unlike the
                    // booking door: a candidate reaches this line only because
                    // the read found him a balance of at least `promo_price`,
                    // which is positive, so his `user_stars` row exists. The
                    // upsert would have written a row for people we then skip.
                    let debited = UsEntity2::update_many()
                        .col_expr(
                            UsCol2::Balance,
                            sea_orm::sea_query::Expr::cust_with_values(
                                "balance - $1",
                                [promo_price],
                            ),
                        )
                        .filter(UsCol2::TelegramId.eq(candidate.telegram_id))
                        .filter(UsCol2::Balance.gte(promo_price))
                        .exec(&tx)
                        .await
                        .map_err(|e| {
                            tracing::error!("cancel_booking promote stars debit: {e}");
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;
                    if debited.rows_affected == 0 {
                        // His balance moved between the read and the lock, or
                        // his balance row is gone. Named rather than silent,
                        // because this is the one way the seat can still miss
                        // someone the read said could take it.
                        tracing::warn!(
                            "cancel_booking: waitlist candidate {} could not be debited {} stars on event {}, offering the seat to the next one",
                            candidate.booking_id,
                            promo_price,
                            event_id
                        );
                        continue;
                    }

                    let balance_after = UsEntity2::find_by_id(candidate.telegram_id)
                        .one(&tx)
                        .await
                        .map_err(|e| {
                            tracing::error!("cancel_booking promote balance read: {e}");
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?
                        .map(|r| r.balance)
                        .unwrap_or(0);
                    let promo_tx_id = uuid::Uuid::new_v4().to_string();
                    TxEntity2::insert(TxAm2 {
                        id: Set(promo_tx_id.clone()),
                        telegram_id: Set(candidate.telegram_id),
                        amount: Set(-promo_price),
                        balance_after: Set(balance_after),
                        source: Set("events".to_string()),
                        reason: Set("event_booking".to_string()),
                        external_tx_id: Set(None),
                        related_order_id: Set(Some(candidate.booking_id.clone())),
                        ..Default::default()
                    })
                    .exec(&tx)
                    .await
                    .map_err(|e| {
                        tracing::error!("cancel_booking promote ledger insert: {e}");
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;
                    promo_stars_tx_id = Some(promo_tx_id);
                }

                tx.execute(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "UPDATE event_bookings SET status = 'confirmed', updated_at = NOW(), stars_paid = $1, stars_tx_id = $2 WHERE id = $3",
                    [
                        promo_price.into(),
                        promo_stars_tx_id.into(),
                        candidate.booking_id.clone().into(),
                    ],
                ))
                .await
                .map_err(|e| {
                    tracing::error!("cancel_booking promote update: {e}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
                crate::metrics::event_booking_created("confirmed");
                crate::metrics::event_waitlist_promoted();
                promoted_id = Some(candidate.booking_id.clone());
                break;
            }

            // A line that exists and got nobody is the failure this scan can
            // still have, and it says so with its numbers instead of leaving a
            // freed seat unexplained. An empty line is not that: there was
            // nobody to promote.
            if promoted_id.is_none() && !candidates.is_empty() {
                tracing::warn!(
                    "cancel_booking: seat freed on event {} stayed empty - {} waitlisted row(s) read (scan cap {}), {} debit attempt(s), price {} stars",
                    event_id,
                    candidates.len(),
                    PROMOTION_CANDIDATE_SCAN_MAX,
                    debit_attempts,
                    promo_price
                );
            }
        }
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("cancel_booking commit: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(promoted_id)
}

async fn cancel_my_booking(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(booking_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    booking_id_ok(&booking_id)?;
    let tid = q
        .get("telegram_id")
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    validate_telegram_id_param(tid)?;
    check_owner_lenient(&headers, &state, tid, "event_book")?;

    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT event_id, telegram_id FROM event_bookings WHERE id = $1",
            [booking_id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("cancel_my_booking lookup: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let (event_id, owner): (String, i64) = row
        .map(|r| {
            (
                r.try_get("", "event_id").unwrap_or_default(),
                r.try_get("", "telegram_id").unwrap_or(0),
            )
        })
        .unwrap_or_default();
    if owner != tid {
        return Err(StatusCode::FORBIDDEN);
    }
    if event_id.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    let promoted_id = cancel_booking_and_promote(&state.db.orm, &event_id, &booking_id).await?;
    let mut resp = json!({ "success": true });
    if let Some(id) = promoted_id {
        resp["promoted_booking_id"] = id.into();
    }
    Ok(Json(resp))
}

async fn join_waitlist(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<BookEventRequest>,
) -> Result<Json<Value>, StatusCode> {
    event_id_ok(&id)?;
    validate_telegram_id_param(req.telegram_id)?;
    check_owner_lenient(&headers, &state, req.telegram_id, "event_waitlist")?;
    let (booker_username, booker_first_name) = booker_identity(&headers, &state);
    check_not_blocked(&state, req.telegram_id).await?;

    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    // Only allow waitlist if the event is public and capacity is actually full.
    let ev = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id, max_seats FROM events WHERE id = $1 AND is_public = TRUE",
            [id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("join_waitlist lookup: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let Some(ev) = ev else {
        return Err(StatusCode::NOT_FOUND);
    };
    let max_seats: Option<i32> = ev.try_get::<Option<i32>>("", "max_seats").ok().flatten();
    let cap = max_seats.unwrap_or(0);
    if cap <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let taken_row = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COALESCE(SUM(seats), 0)::int AS taken FROM event_bookings WHERE event_id = $1 AND status = 'confirmed'",
            [id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("join_waitlist capacity check: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let taken: i32 = taken_row
        .and_then(|r| r.try_get("", "taken").ok())
        .unwrap_or(0);
    if taken < cap {
        return Err(StatusCode::CONFLICT); // still has seats
    }

    // Idempotency: one waitlist entry per user per event.
    let dup = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT 1 FROM event_bookings WHERE event_id = $1 AND telegram_id = $2 AND status IN ('confirmed','waitlisted')",
            [id.clone().into(), req.telegram_id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("join_waitlist dup check: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if dup.is_some() {
        return Err(StatusCode::CONFLICT);
    }

    let booking_id = uuid::Uuid::new_v4().to_string();
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO event_bookings (id, event_id, telegram_id, seats, status, username, first_name) VALUES ($1,$2,$3,1,'waitlisted',$4,$5)",
            [
                booking_id.clone().into(),
                id.into(),
                req.telegram_id.into(),
                booker_username.into(),
                booker_first_name.into(),
            ],
        ))
        .await
        .map_err(|e| {
            tracing::error!("join_waitlist insert: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    crate::metrics::event_booking_created("waitlisted");
    Ok(Json(
        json!({ "success": true, "booking_id": booking_id, "status": "waitlisted" }),
    ))
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
         e.location_text, e.image_url, e.video_url, e.max_seats, e.price_baht::float8 AS price_baht, e.price_stars, e.is_public, e.created_at, \
         COALESCE((SELECT SUM(b.seats) FROM event_bookings b WHERE b.event_id = e.id AND b.status = 'confirmed'), 0)::bigint AS seats_taken, \
         (SELECT COUNT(*)::bigint FROM event_bookings b WHERE b.event_id = e.id) AS bookings_count \
         FROM events e \
         ORDER BY e.starts_at DESC \
         LIMIT 500".to_string(),
    )).await.map_err(|e| { tracing::error!("list_admin_events: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    let events: Vec<Value> = rows
        .iter()
        .map(|r| {
            let mut v = event_row(r);
            if let Value::Object(ref mut m) = v {
                m.insert(
                    "bookings_count".to_string(),
                    json!(r.try_get::<i64>("", "bookings_count").unwrap_or(0)),
                );
            }
            v
        })
        .collect();
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
    use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};
    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("create_event tx.begin: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, String::new())
    })?;
    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO events (id, title, title_en, description, description_en, starts_at, ends_at, location_text, image_url, video_url, max_seats, price_baht, price_stars, is_public) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
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
            req.video_url.filter(|s| !s.is_empty()).into(),
            req.max_seats.into(),
            req.price_baht.into(),
            req.price_stars.into(),
            is_public.into(),
        ],
    )).await.map_err(|e| {
        tracing::error!("create_event: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, String::new())
    })?;
    if let Err(e) = replace_event_photos(&tx, &id, req.photos.as_deref().unwrap_or(&[])).await {
        let _ = tx.rollback().await;
        return Err(e);
    }
    tx.commit().await.map_err(|e| {
        tracing::error!("create_event tx.commit: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, String::new())
    })?;
    Ok(Json(json!({ "success": true, "id": id })))
}

async fn replace_event_photos(
    db: &impl sea_orm::ConnectionTrait,
    event_id: &str,
    photos: &[String],
) -> Result<(), (StatusCode, String)> {
    use sea_orm::{DbBackend, Statement};
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "DELETE FROM event_photos WHERE event_id = $1",
        [event_id.into()],
    ))
    .await
    .map_err(|e| {
        tracing::error!("replace_event_photos delete: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, String::new())
    })?;
    for (i, url) in photos.iter().enumerate() {
        let url = url.trim();
        if url.is_empty() {
            continue;
        }
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO event_photos (id, event_id, url, display_order) VALUES ($1,$2,$3,$4)",
            [
                uuid::Uuid::new_v4().to_string().into(),
                event_id.into(),
                url.into(),
                (i as i32).into(),
            ],
        ))
        .await
        .map_err(|e| {
            tracing::error!("replace_event_photos insert: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        })?;
    }
    Ok(())
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
         e.location_text, e.image_url, e.video_url, e.max_seats, e.price_baht::float8 AS price_baht, e.price_stars, e.is_public, e.created_at, \
         COALESCE((SELECT SUM(b.seats) FROM event_bookings b WHERE b.event_id = e.id AND b.status = 'confirmed'), 0)::bigint AS seats_taken \
         FROM events e \
         WHERE e.id = $1",
        [id.clone().into()],
    )).await.map_err(|e| { tracing::error!("get_admin_event: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    match row {
        Some(r) => {
            let mut ev = event_row(&r);
            let photos = load_event_photos(&state.db.orm, &id).await?;
            if let Value::Object(ref mut m) = ev {
                m.insert("photos".to_string(), json!(photos));
            }
            Ok(Json(json!({ "event": ev })))
        }
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
    use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};
    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("update_event tx.begin: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, String::new())
    })?;
    tx.execute(Statement::from_sql_and_values(
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
            video_url = COALESCE($9, video_url), \
            max_seats = COALESCE($10, max_seats), \
            price_baht = COALESCE($11, price_baht), \
            price_stars = COALESCE($12, price_stars), \
            is_public = COALESCE($13, is_public), \
            updated_at = NOW() \
         WHERE id = $14",
        [
            req.title
                .as_deref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .into(),
            req.title_en.filter(|s| !s.is_empty()).into(),
            req.description.filter(|s| !s.is_empty()).into(),
            req.description_en.filter(|s| !s.is_empty()).into(),
            starts_at
                .map(|d| sea_orm::Value::ChronoDateTimeUtc(Some(Box::new(d))))
                .unwrap_or(sea_orm::Value::ChronoDateTimeUtc(None)),
            ends_at
                .unwrap_or(None)
                .map(|d| sea_orm::Value::ChronoDateTimeUtc(Some(Box::new(d))))
                .unwrap_or(sea_orm::Value::ChronoDateTimeUtc(None)),
            req.location_text.filter(|s| !s.is_empty()).into(),
            req.image_url.filter(|s| !s.is_empty()).into(),
            req.video_url.filter(|s| !s.is_empty()).into(),
            req.max_seats.into(),
            req.price_baht.into(),
            req.price_stars.into(),
            is_public
                .map(|v| v.into())
                .unwrap_or(sea_orm::Value::Bool(None)),
            id.clone().into(),
        ],
    ))
    .await
    .map_err(|e| {
        tracing::error!("update_event: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, String::new())
    })?;
    if let Some(ref photos) = req.photos {
        if let Err(e) = replace_event_photos(&tx, &id, photos).await {
            let _ = tx.rollback().await;
            return Err(e);
        }
    }
    tx.commit().await.map_err(|e| {
        tracing::error!("update_event tx.commit: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, String::new())
    })?;
    Ok(Json(json!({ "success": true })))
}

/// What a delete is allowed to do about the money the event is still holding.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DeleteVerdict {
    /// Nothing paid is attached; the cascade destroys no money.
    Allowed,
    /// The event still holds paid bookings and the caller acknowledged
    /// nothing. The two numbers exist so the refusal can say how many people
    /// and how many Stars, rather than being one more indistinguishable
    /// failure.
    HoldsPaidBookings { bookings: i64, stars: i64 },
    /// The caller named a count and it is not the count this transaction
    /// measured. All three numbers travel — the two the server stands behind
    /// and the one the caller sent — so the answer shows the gap instead of
    /// only asserting that there is one.
    AcknowledgementIsStale {
        bookings: i64,
        stars: i64,
        acknowledged: i64,
    },
    /// The caller named exactly the count the server measured, and it is not
    /// zero: the delete goes through and these paid bookings go with it. Still
    /// no refund — the two numbers are here so the log line and the answer can
    /// say what was given up.
    AbandonsPaidBookings { bookings: i64, stars: i64 },
}

/// The query parameter that opens the delete on an event still holding paid
/// bookings: `DELETE /api/admin/events/:id?abandon_paid_bookings=3`.
///
/// Named, and not a flag. `?force=1` would be a word about the delete; this is
/// a word about the BOOKINGS, and the number in it is the thing being given
/// up. A client that cannot say how many paid bookings it is destroying is a
/// client that does not know how many there are, and it gets the refusal.
const ABANDON_PAID_BOOKINGS_PARAM: &str = "abandon_paid_bookings";

/// A delete must not silently destroy paid state.
///
/// `migrations/043_events_booking.sql:29` declares `ON DELETE CASCADE` on
/// `event_bookings.event_id`, so `DELETE FROM events` takes every booking row
/// with it — including the rows that record `stars_paid`. The only code that
/// returns Stars is the refund block inside `cancel_booking_and_promote`, and
/// a delete never enters it. The customer was left with a debit in
/// `stars_transactions` (that table has no foreign key to the booking, so the
/// ledger survives) pointing at an event, a seat and a booking that no longer
/// exist, and nothing in the tree would ever pay it back.
///
/// So the delete REFUSES while paid bookings are attached. Refusing is the
/// reversible half of the choice: nothing is destroyed by it, and the owner
/// already has a tested path that returns the Stars one booking at a time.
/// Refunding from inside the delete would have to reuse
/// `cancel_booking_and_promote`, whose second half PROMOTES a waitlisted
/// customer — charging him for a seat at an event that is about to vanish.
///
/// AND IT REFUSES WITH A DOOR, which is the half added on 2026-09-21. A
/// refusal with no way through is its own defect: an event that already
/// happened holds its paid bookings for ever, so a finished evening with three
/// paid seats was a row nobody — the owner included — could delete at all. The
/// door is `acknowledged`: the caller names how many paid bookings he is
/// giving up, and it has to equal the count this transaction measured under
/// the row lock.
///
/// Three properties, each doing work:
///
/// * **Refusal stays the default.** `None` is every caller that says nothing,
///   which is every client written before the parameter existed.
/// * **The acknowledgement is explicit, and it is a NUMBER rather than a
///   flag.** A flag can be set by a client with no idea what it is destroying;
///   a count can only be sent by one that has read the same event this
///   transaction just locked.
/// * **The server’s count is the authority.** A stale admin screen replaying
///   an old click carries the old number, and a seat paid for in between makes
///   that number wrong, so the delete refuses and hands back what it measured.
///   Zero is not exempt: `Some(0)` against three paid bookings is the same
///   false statement as any other wrong number, and it is the one an empty
///   form field would send.
///
/// The cost of that strictness is named rather than hidden: a caller whose
/// count is stale in the HARMLESS direction — he says three, they were all
/// cancelled and refunded, the server measures zero — is refused too, and has
/// to re-read the event. That is one round trip, against an exception that
/// would have to be reasoned about every time this function is read.
///
/// Neither road refunds anything. The door decides who may destroy the rows,
/// not whether the Stars come back.
fn delete_verdict(paid_bookings: i64, stars_held: i64, acknowledged: Option<i64>) -> DeleteVerdict {
    match acknowledged {
        // A count travelled and it is not this event’s. Whatever the caller
        // read, it was not what is in front of this transaction.
        Some(n) if n != paid_bookings => DeleteVerdict::AcknowledgementIsStale {
            bookings: paid_bookings,
            stars: stars_held,
            acknowledged: n,
        },
        // It matches, and there is something to give up.
        Some(_) if paid_bookings > 0 => DeleteVerdict::AbandonsPaidBookings {
            bookings: paid_bookings,
            stars: stars_held,
        },
        // It matches at zero: an ordinary delete that happens to carry the
        // parameter. Nothing is abandoned, so nothing is announced.
        Some(_) => DeleteVerdict::Allowed,
        None if paid_bookings > 0 => DeleteVerdict::HoldsPaidBookings {
            bookings: paid_bookings,
            stars: stars_held,
        },
        None => DeleteVerdict::Allowed,
    }
}

async fn delete_event(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    event_id_ok(&id).map_err(|c| booking_err(c, "invalid_event_id"))?;
    check_admin(&headers, &state).map_err(|c| booking_err(c, "unauthorized"))?;

    // The acknowledgement, read before anything is locked. Absent is the
    // default and means no: only a value that PARSES and MATCHES opens the
    // door, and `delete_verdict` below owns the matching.
    //
    // A present value that is not a number is answered 400 and not read as
    // absent, and above all not read as zero (D9). Both silent readings turn a
    // typo into a sentence about the money: as absent it becomes the ordinary
    // refusal on an event whose count the caller did name, and as zero it
    // becomes a claim that nothing is at stake.
    let acknowledged: Option<i64> = match q.get(ABANDON_PAID_BOOKINGS_PARAM).map(|s| s.trim()) {
        None => None,
        Some(raw) => match raw.parse::<i64>() {
            Ok(n) => Some(n),
            Err(_) => {
                return Err(booking_err(
                    StatusCode::BAD_REQUEST,
                    "invalid_abandon_paid_bookings",
                ))
            }
        },
    };

    use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};

    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("delete_event tx.begin: {e}");
        booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
    })?;

    // Lock the event row the way the booking door and the cancel path both do,
    // so a seat cannot be sold — or a waitlisted customer charged and promoted
    // — between the count below and the DELETE. A row that is not there locks
    // nothing and counts nothing, and the DELETE is then the no-op this
    // endpoint has always answered for an id that does not exist.
    tx.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id FROM events WHERE id = $1 FOR UPDATE",
        [id.clone().into()],
    ))
    .await
    .map_err(|e| {
        tracing::error!("delete_event lock event: {e}");
        booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
    })?;

    // What the cascade would take with the event. A row still counts as
    // holding Stars unless it was CANCELLED: cancelling is the path that gave
    // them back, and it is the only one.
    let held = tx
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS paid_bookings, COALESCE(SUM(stars_paid), 0)::bigint AS stars_held \
             FROM event_bookings \
             WHERE event_id = $1 AND status <> 'cancelled' AND COALESCE(stars_paid, 0) > 0",
            [id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("delete_event paid booking count: {e}");
            booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
        })?;
    // A count that cannot be read is not zero (D9). Reading it as zero here
    // would open the delete on exactly the failure the count exists to catch,
    // so an unreadable count refuses the same way a database error does.
    let (paid_bookings, stars_held): (i64, i64) = match held {
        Some(ref r) => match (r.try_get("", "paid_bookings"), r.try_get("", "stars_held")) {
            (Ok(n), Ok(s)) => (n, s),
            _ => {
                let _ = tx.rollback().await;
                tracing::error!("delete_event: paid booking count unreadable for event {id}");
                return Err(booking_err(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "server_error",
                ));
            }
        },
        None => {
            let _ = tx.rollback().await;
            tracing::error!("delete_event: paid booking count returned no row for event {id}");
            return Err(booking_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "server_error",
            ));
        }
    };

    // One decision, taken in one place. Both refusals roll back and answer 409
    // with every number the caller needs to act; both ways through fall out of
    // the match and reach the DELETE below.
    let abandoned = match delete_verdict(paid_bookings, stars_held, acknowledged) {
        DeleteVerdict::Allowed => None,
        DeleteVerdict::AbandonsPaidBookings { bookings, stars } => {
            // The event, the people and the money, written BEFORE the DELETE:
            // the rows that recorded them are about to stop existing, and a
            // crash before the commit must not take the record with them.
            tracing::warn!(
                "delete_event: about to delete event {id} with {bookings} paid booking(s) worth {stars} stars abandoned, acknowledged by the caller; no Stars are returned"
            );
            Some((bookings, stars))
        }
        DeleteVerdict::HoldsPaidBookings { bookings, stars } => {
            // Both numbers travel to the caller. A bare 409 would be one more
            // indistinguishable failure, and the owner's next move depends on
            // how many people he has to refund first.
            let _ = tx.rollback().await;
            tracing::warn!(
                "delete_event refused: event {id} still holds {bookings} paid booking(s) worth {stars} stars"
            );
            return Err((
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "paid_bookings_exist",
                    "paid_bookings": bookings,
                    "stars_held": stars,
                })),
            ));
        }
        DeleteVerdict::AcknowledgementIsStale {
            bookings,
            stars,
            acknowledged,
        } => {
            // The caller answered for a different event than the one under the
            // lock. The measured count goes back so the next attempt can be
            // made against what is actually there.
            let _ = tx.rollback().await;
            tracing::warn!(
                "delete_event refused: event {id} holds {bookings} paid booking(s) worth {stars} stars, caller acknowledged {acknowledged}"
            );
            return Err((
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "acknowledgement_is_stale",
                    "paid_bookings": bookings,
                    "stars_held": stars,
                    "acknowledged": acknowledged,
                })),
            ));
        }
    };

    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "DELETE FROM events WHERE id = $1",
        [id.into()],
    ))
    .await
    .map_err(|e| {
        tracing::error!("delete_event: {e}");
        booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
    })?;
    tx.commit().await.map_err(|e| {
        tracing::error!("delete_event tx.commit: {e}");
        booking_err(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
    })?;
    // The ordinary delete answers what it always answered. The one that took
    // paid bookings with it says so in the body too: the caller asked for this,
    // and the receipt is the last place those two numbers exist.
    match abandoned {
        None => Ok(Json(json!({ "success": true }))),
        Some((bookings, stars)) => Ok(Json(json!({
            "success": true,
            "abandoned_paid_bookings": bookings,
            "stars_abandoned": stars,
        }))),
    }
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
        "SELECT id, event_id, telegram_id, username, first_name, seats, status, order_id, stars_paid, stars_tx_id, created_at FROM event_bookings \
         WHERE event_id = $1 \
         ORDER BY created_at DESC",
        [id.into()],
    )).await.map_err(|e| { tracing::error!("list_event_bookings: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    let bookings: Vec<Value> = rows.iter().map(booking_row).collect();
    Ok(Json(json!({ "bookings": bookings })))
}

/// `POST /api/admin/events/:id/bookings/send` — deliver the guest list to the
/// admin as a Telegram message.
///
/// Exists because the Mini App cannot open a person who has no @username: the
/// webview turns `tg://user?id=…` into an "Open link?" prompt that then does
/// nothing. In a *bot message* the same URL is a real inline mention, so every
/// guest becomes tappable regardless of whether they have a handle.
async fn send_event_bookings(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    event_id_ok(&id)?;
    let admin_id = check_admin(&headers, &state)?;

    // The password-auth path reports 0, so fall back to the header — but only
    // for an id that is actually an admin. Without that check, anyone holding
    // the admin password could use this endpoint to send messages to arbitrary
    // chats.
    let target = if admin_id > 0 {
        admin_id
    } else {
        let claimed = headers
            .get("X-Admin-Telegram-Id")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok())
            .ok_or(StatusCode::BAD_REQUEST)?;
        if !state.config.admin_ids.contains(&claimed) {
            return Err(StatusCode::FORBIDDEN);
        }
        claimed
    };

    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let title = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT title FROM events WHERE id = $1",
            [id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("send_event_bookings title: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .and_then(|r| r.try_get::<String>("", "title").ok())
        .ok_or(StatusCode::NOT_FOUND)?;

    let rows = state
        .db
        .orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT telegram_id, username, first_name, seats FROM event_bookings \
             WHERE event_id = $1 AND status = 'confirmed' \
             ORDER BY created_at ASC",
            [id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("send_event_bookings rows: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let attendees: Vec<crate::trios::attendees::Attendee> = rows
        .iter()
        .map(|r| crate::trios::attendees::Attendee {
            telegram_id: r.try_get::<i64>("", "telegram_id").unwrap_or(0),
            username: r.try_get::<Option<String>>("", "username").ok().flatten(),
            first_name: r.try_get::<Option<String>>("", "first_name").ok().flatten(),
            seats: r.try_get::<i32>("", "seats").unwrap_or(1),
        })
        .collect();
    let count = attendees.len();
    let text = crate::trios::attendees::format_attendee_message(&title, &attendees);

    use teloxide::payloads::SendMessageSetters;
    use teloxide::prelude::Requester;
    use teloxide::types::ChatId;
    state
        .bot
        .send_message(ChatId(target), text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .await
        .map_err(|e| {
            tracing::error!("send_event_bookings send: {e}");
            StatusCode::BAD_GATEWAY
        })?;

    Ok(Json(json!({ "success": true, "sent": count })))
}

async fn cancel_booking(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path((event_id, booking_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    event_id_ok(&event_id)?;
    booking_id_ok(&booking_id)?;
    check_admin(&headers, &state)?;
    let promoted_id = cancel_booking_and_promote(&state.db.orm, &event_id, &booking_id).await?;
    let mut resp = json!({ "success": true });
    if let Some(id) = promoted_id {
        resp["promoted_booking_id"] = id.into();
    }
    Ok(Json(resp))
}

// ── Reminders ──────────────────────────────────────────────────

fn build_event_reminder_body(
    title: &str,
    starts_at: chrono::DateTime<chrono::Utc>,
    location_text: Option<&str>,
) -> String {
    // The declared market's wall clock (D18). Described as "local time" rather
    // than named: customers read a timezone label as the event address, and the
    // market's zone name is not the venue.
    //
    // Was `starts_at + chrono::Duration::hours(7)` — a value typed UTC while
    // holding local time. `%d.%m.%Y %H:%M` never asks for the offset, which is
    // why the lie kept producing correct-looking output. This site is one of
    // the two D18's own list of offset literals missed.
    let starts_local = match crate::trios::market::MARKET.at(starts_at) {
        Some(local) => local,
        None => starts_at.fixed_offset(),
    };
    let starts_text = format!(
        "{} по местному времени",
        starts_local.format("%d.%m.%Y %H:%M")
    );
    let location = location_text
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Место проведения уточняется");
    crate::trios::i18n::tf(
        crate::trios::core::Lang::Russian,
        crate::trios::i18n::T_EVENTS_REMINDER_BODY,
        &[title.to_string(), starts_text, location.to_string()],
    )
}

/// A3: send one reminder per confirmed booking for events starting
/// within the next `hours` window and not already reminded.
/// Returns the number of successfully delivered reminders.
#[allow(dead_code)]
pub(crate) async fn send_event_reminders(
    orm: &sea_orm::DatabaseConnection,
    bot: &teloxide::Bot,
    hours: i64,
) -> Result<usize, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let rows = orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT b.id AS booking_id, b.telegram_id, e.id AS event_id, e.title, \
                    e.starts_at, e.location_text \
             FROM event_bookings b \
             JOIN events e ON e.id = b.event_id \
             WHERE b.status = 'confirmed' \
               AND b.reminder_sent_at IS NULL \
               AND e.starts_at > NOW() \
               AND e.starts_at <= NOW() + ($1 * INTERVAL '1 hour') \
             ORDER BY e.starts_at ASC",
            [hours.into()],
        ))
        .await?;

    let mut sent = 0usize;
    for r in rows {
        let booking_id: String = r.try_get("", "booking_id").unwrap_or_default();
        let telegram_id: i64 = r.try_get("", "telegram_id").unwrap_or(0);
        let title: String = r.try_get("", "title").unwrap_or_default();
        let starts_at: chrono::DateTime<chrono::Utc> = r
            .try_get("", "starts_at")
            .unwrap_or_else(|_| chrono::Utc::now());
        let location_text = r
            .try_get::<Option<String>>("", "location_text")
            .unwrap_or(None);
        let body = build_event_reminder_body(&title, starts_at, location_text.as_deref());

        // Best-effort send; failures are logged but don't break the sweep.
        use teloxide::prelude::Requester;
        let deliver = async {
            bot.send_message(teloxide::types::ChatId(telegram_id), &body)
                .await?;
            Ok::<(), teloxide::RequestError>(())
        };
        if let Err(e) = deliver.await {
            tracing::warn!(
                "event reminder send failed booking={} tid={}: {}",
                booking_id,
                telegram_id,
                e
            );
            continue;
        }

        orm.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE event_bookings SET reminder_sent_at = NOW() WHERE id = $1",
            [booking_id.into()],
        ))
        .await?;
        sent += 1;
    }

    Ok(sent)
}

/// Spawn a background loop that sends event reminders every `interval_secs`.
/// Gated to server builds (not WASM).
#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub(crate) fn spawn_event_reminder_loop(
    orm: sea_orm::DatabaseConnection,
    bot: std::sync::Arc<teloxide::Bot>,
    hours: i64,
    interval_secs: u64,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        interval.tick().await; // discard cold-start tick
        loop {
            interval.tick().await;
            match send_event_reminders(&orm, &bot, hours).await {
                // silent-tick: deliberate, not an omission -- ticks every 300s (spawn_event_reminder_loop(.., 24, 300)); `sent` increments only after a successful Telegram send, so Ok(0) can mean every send FAILED.
                // Reporting "nothing due" here would announce health where there may be
                // total delivery failure, which is worse than the silence it replaced.
                Ok(0) => {}
                Ok(n) => tracing::info!("event reminders: sent {} reminder(s)", n),
                Err(e) => tracing::warn!("event reminders sweep failed: {}", e),
            }
        }
    });
}

/// Whether the admin API may store an event as public. `false` since the
/// owner's ruling of 2026-09-24 (rental only, Phuket only), which retired
/// events from every customer surface.
///
/// Every customer read of an event -- the calendar, one event, a booking, a
/// waitlist join -- filters `is_public = TRUE`. A new event defaulted to public
/// (migration 043's `DEFAULT TRUE` agrees) and an edit could re-publish a
/// hidden one; both now store the requested flag ANDed with this constant.
/// Rows already public are hidden by migration 088 (085 hid the ones it found),
/// so once 088 has run the customer reads answer empty, while the routes, the
/// rows, the admin records, the bookings and the refund paths all stay.
/// Flipping it back to `true` restores the old write path; nothing was deleted.
pub(crate) const EVENTS_PUBLISHABLE: bool = false;

/// The `is_public` an admin write may store: the requested flag, and never
/// `true` while [`EVENTS_PUBLISHABLE`] is false. Used on create (where an
/// omitted flag still reads as a request for public) and on edit (where an
/// omitted flag leaves the stored one alone).
fn storable_public_flag(requested: bool) -> bool {
    requested && EVENTS_PUBLISHABLE
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_reminder_uses_the_real_location_not_the_timezone_name() {
        let starts_at = chrono::DateTime::parse_from_rfc3339("2026-08-28T13:00:00Z")
            .expect("valid timestamp")
            .with_timezone(&chrono::Utc);
        let body = build_event_reminder_body(
            "SUNSET RIDE NIGHT",
            starts_at,
            Some("TurboBaby, Kamala Phuket"),
        );

        assert!(body.contains("28.08.2026 20:00"));
        assert!(body.contains("TurboBaby, Kamala Phuket"));
        assert!(body.contains("по местному времени"));
        assert!(!body.contains("Bangkok"));
    }

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
            video_url: None,
            photos: None,
            max_seats: Some(20),
            price_baht: None,
            price_stars: None,
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
        assert_eq!(
            validate_event_request(&r).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_event_title_too_long() {
        let mut r = valid_create();
        r.title = "a".repeat(201);
        assert_eq!(
            validate_event_request(&r).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_event_ends_before_starts() {
        let mut r = valid_create();
        r.ends_at = Some("2026-08-01T18:00:00+07:00".into());
        assert_eq!(
            validate_event_request(&r).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_event_negative_max_seats() {
        let mut r = valid_create();
        r.max_seats = Some(-1);
        assert_eq!(
            validate_event_request(&r).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_event_price_negative() {
        let mut r = valid_create();
        r.price_baht = Some(-1.0);
        assert_eq!(
            validate_event_request(&r).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_event_price_stars_negative() {
        let mut r = valid_create();
        r.price_stars = Some(-1);
        assert_eq!(
            validate_event_request(&r).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_event_price_stars_too_large() {
        let mut r = valid_create();
        r.price_stars = Some(1_000_000_001);
        assert_eq!(
            validate_event_request(&r).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_event_bad_image_url() {
        let mut r = valid_create();
        r.image_url = Some("javascript:alert(1)".into());
        assert_eq!(
            validate_event_request(&r).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn event_id_ok_rejects_too_long() {
        assert!(event_id_ok(&"x".repeat(37)).is_err());
        assert!(event_id_ok(&"").is_err());
        assert!(event_id_ok("550e8400-e29b-41d4-a716-446655440000").is_ok());
    }

    // ── The waitlist: a freed seat must reach someone ────────────────
    //
    // No telegram id here is a person: 1, 2, 3 are line positions written as
    // the column's type and nothing else (D14).

    /// A line position: the booking id, the id the debit would target, and the
    /// balance read with the row.
    fn waiting(booking_id: &str, telegram_id: i64, balance: i64) -> PromotionCandidate {
        PromotionCandidate {
            booking_id: booking_id.to_string(),
            telegram_id,
            balance,
        }
    }

    /// The head-of-line block. Three people wait for a 500-Star seat and the
    /// first cannot pay; the seat must reach the second, not stay empty.
    #[test]
    fn a_freed_seat_passes_the_head_who_cannot_pay() {
        let line = [
            waiting("a", 1, 0),
            waiting("b", 2, 500),
            waiting("c", 3, 900),
        ];
        assert_eq!(
            next_promotable(&line, 500, 0),
            Some(1),
            "the seat stopped at the head instead of reaching the next payer"
        );
    }

    /// Skipping must not cost the skipped customer his place: the decision is
    /// an index into the line, and the line it was given comes back unchanged.
    #[test]
    fn a_skipped_customer_keeps_his_place_in_the_line() {
        let line = [waiting("a", 1, 0), waiting("b", 2, 0), waiting("c", 3, 900)];
        let before = line.clone();
        assert_eq!(next_promotable(&line, 500, 0), Some(2));
        assert_eq!(line, before, "the line was reordered or rewritten");
    }

    /// A lost race resumes at the next candidate rather than at the head: the
    /// debit is the authority, and when it refuses the seat moves on.
    #[test]
    fn the_scan_resumes_after_a_candidate_falls_through() {
        let line = [
            waiting("a", 1, 900),
            waiting("b", 2, 0),
            waiting("c", 3, 900),
        ];
        assert_eq!(next_promotable(&line, 500, 1), Some(2));
    }

    /// Nobody in the line can pay: there is no promotion, and the caller is
    /// the one that says so in the log.
    #[test]
    fn a_line_of_people_who_cannot_pay_promotes_nobody() {
        let line = [waiting("a", 1, 0), waiting("b", 2, 499)];
        assert_eq!(next_promotable(&line, 500, 0), None);
        assert_eq!(next_promotable(&[], 500, 0), None);
    }

    /// A free event charges nobody, so the head takes the seat whatever his
    /// balance. Guard rather than proof: this held before the skip was added
    /// and must keep holding.
    #[test]
    fn a_free_event_promotes_the_head_whatever_his_balance() {
        let line = [waiting("a", 1, 0), waiting("b", 2, 900)];
        assert_eq!(next_promotable(&line, 0, 0), Some(0));
    }

    // ── Delete: paid bookings are not the delete's to destroy ─────────

    /// The cascade at migrations/043_events_booking.sql:29 takes paid bookings
    /// with the event and no Stars come back, so the delete refuses and says
    /// how many and how much.
    #[test]
    fn a_delete_refuses_while_the_event_holds_paid_bookings() {
        assert_eq!(
            delete_verdict(3, 1500, None),
            DeleteVerdict::HoldsPaidBookings {
                bookings: 3,
                stars: 1500
            },
            "the delete destroyed paid bookings with no refund"
        );
        assert_eq!(
            delete_verdict(1, 500, None),
            DeleteVerdict::HoldsPaidBookings {
                bookings: 1,
                stars: 500
            }
        );
    }

    /// An event nobody paid for deletes as it always did: a free event, an
    /// empty one, and one whose bookings were all cancelled and refunded
    /// already all count zero paid rows.
    #[test]
    fn a_delete_with_nothing_paid_still_goes_through() {
        assert_eq!(delete_verdict(0, 0, None), DeleteVerdict::Allowed);
    }

    /// The way through, and the reason it exists: an event that already
    /// happened holds paid bookings for ever, so a refusal with no door is a
    /// row nobody can ever remove. The owner opens the door by NAMING what he
    /// gives up -- the count the server just measured -- and the delete goes
    /// through, still refunding nothing.
    #[test]
    fn an_owner_who_names_the_count_may_abandon_it() {
        assert_eq!(
            delete_verdict(3, 1500, Some(3)),
            DeleteVerdict::AbandonsPaidBookings {
                bookings: 3,
                stars: 1500
            },
            "an acknowledgement matching the count the server measured was refused"
        );
    }

    /// The stale-screen case, which is why the count travels at all. The admin
    /// list said one paid booking; two more were paid while the confirm dialog
    /// sat open. Replaying that click must not take the two rows nobody has
    /// seen -- the server's number is the authority and the answer carries it.
    #[test]
    fn a_stale_screen_cannot_abandon_a_booking_it_never_saw() {
        assert_eq!(
            delete_verdict(3, 1500, Some(1)),
            DeleteVerdict::AcknowledgementIsStale {
                bookings: 3,
                stars: 1500,
                acknowledged: 1
            },
            "a count from an older screen opened the delete on rows it did not name"
        );
    }

    /// Zero is not a skeleton key. `abandon_paid_bookings=0` on an event that
    /// holds three is the same false statement as any other wrong number, and
    /// it is the one a client would send by accident -- an empty field, a
    /// default, a number that was true before the seats sold.
    #[test]
    fn acknowledging_zero_does_not_open_a_paid_event() {
        assert_eq!(
            delete_verdict(3, 1500, Some(0)),
            DeleteVerdict::AcknowledgementIsStale {
                bookings: 3,
                stars: 1500,
                acknowledged: 0
            }
        );
    }

    /// Nothing paid and nothing claimed: the ordinary delete, unchanged, and
    /// the acknowledgement is not something it has to carry.
    #[test]
    fn an_acknowledgement_is_not_required_when_nothing_was_paid() {
        assert_eq!(delete_verdict(0, 0, Some(0)), DeleteVerdict::Allowed);
        assert_eq!(
            delete_verdict(0, 0, Some(2)),
            DeleteVerdict::AcknowledgementIsStale {
                bookings: 0,
                stars: 0,
                acknowledged: 2
            },
            "a count that is false about this event was accepted as harmless"
        );
    }

    /// Owner decision 2026-09-24 (rental only, Phuket only): events are retired
    /// from every customer surface. Every customer read filters `is_public`,
    /// and migrations 085 and 088 hide the rows already public, so what could
    /// still put an event in front of a customer is this write: a new event
    /// defaulted to public and an edit could re-publish a hidden one. Neither can.
    #[test]
    fn a_new_event_is_never_stored_public_while_events_are_retired() {
        let mut omitted = valid_create();
        omitted.is_public = None;
        assert!(!validate_event_request(&omitted).expect("valid create").0);
        // `valid_create` asks for a public event outright.
        assert!(
            !validate_event_request(&valid_create())
                .expect("valid create")
                .0
        );

        let update = |body: Value| -> Option<bool> {
            let req: UpdateEventRequest = serde_json::from_value(body).expect("update body");
            validate_update_request(&req).expect("valid update").0
        };
        assert_eq!(update(json!({ "is_public": true })), Some(false));
        assert_eq!(update(json!({ "is_public": false })), Some(false));
        // An edit that does not mention the flag leaves the stored one alone.
        assert_eq!(update(json!({})), None);
    }
}
