//! Integration tests for the per-event attendee list.
//!
//! `event_bookings` stored only `telegram_id`, so the admin list of who booked
//! was a column of bare numbers — it identified nobody and reached nobody. The
//! handle is now captured from **validated** initData at booking time and
//! returned to the admin endpoint.
//!
//! Captured server-side on purpose: a client-supplied username would let
//! anyone book under someone else's handle, which is worse than no handle —
//! the owner would message the wrong person.
//!
//! Marked `#[ignore]` — runs with:
//!
//! ```sh
//! DATABASE_URL=postgres://... cargo test --features backend -- --ignored
//! ```

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::{json, Value};
use tower::ServiceExt;

const BOT_TOKEN: &str = "dummy_test_token";

struct Resp {
    status: StatusCode,
    body: Value,
}

async fn send(app: axum::Router, request: Request<Body>) -> Resp {
    let response = app.oneshot(request).await.expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    Resp { status, body }
}

fn admin_token() -> String {
    turbobaby_bot::api::auth::generate_admin_token("test_password", "dummy_test_token")
}

fn suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}

/// Seed a public event starting tomorrow with plenty of seats.
async fn seed_event(db: &turbobaby_bot::db::Database, seats: i32) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO events (id, title, description, starts_at, max_seats, is_public) \
             VALUES ($1, $2, 'attendee test', NOW() + INTERVAL '1 day', $3, TRUE)",
            [
                id.clone().into(),
                format!("attendee-event-{}", suffix()).into(),
                seats.into(),
            ],
        ))
        .await
        .expect("seed event INSERT");
    id
}

/// initData carrying a username, signed with the real test token so strict
/// validation accepts it.
fn init_data_with_username(user_id: i64, username: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::{Digest, Sha256};

    let user_json = format!(r#"{{"id":{user_id},"first_name":"Тест","username":"{username}"}}"#);
    let auth_date = chrono::Utc::now().timestamp();
    // Data-check string: sorted `key=value` pairs joined by \n, hash excluded.
    let check = format!("auth_date={auth_date}\nuser={user_json}",);
    let mut secret = Hmac::<Sha256>::new_from_slice(b"WebAppData").expect("hmac key");
    secret.update(BOT_TOKEN.as_bytes());
    let secret_key = secret.finalize().into_bytes();

    let mut mac = Hmac::<Sha256>::new_from_slice(&secret_key).expect("hmac key");
    mac.update(check.as_bytes());
    let hash = mac.finalize().into_bytes();
    let hash_hex = hash.iter().fold(String::new(), |mut acc, b| {
        use std::fmt::Write;
        let _ = write!(acc, "{b:02x}");
        acc
    });
    let _ = Sha256::new(); // keep the Digest import honest across sha2 versions

    format!(
        "auth_date={auth_date}&user={}&hash={hash_hex}",
        urlencoding::encode(&user_json)
    )
}

async fn book(app: axum::Router, event_id: &str, tid: i64, init_data: &str) -> Resp {
    send(
        app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/events/{event_id}/book"))
            .header("content-type", "application/json")
            .header("X-Telegram-Init-Data", init_data)
            .header("X-Idempotency-Key", uuid::Uuid::new_v4().to_string())
            .body(Body::from(
                serde_json::to_vec(&json!({ "telegram_id": tid, "seats": 1 })).unwrap(),
            ))
            .unwrap(),
    )
    .await
}

async fn attendees(app: axum::Router, event_id: &str) -> Resp {
    send(
        app,
        Request::builder()
            .uri(format!("/api/admin/events/{event_id}/bookings"))
            .header("X-Admin-Token", admin_token())
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn a_booking_records_the_bookers_telegram_handle() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let event_id = seed_event(&db, 10).await;
    let tid = 999_600_000 + (suffix() as i64 % 100_000);
    let handle = format!("guest{}", suffix());

    let booked = book(
        app.clone(),
        &event_id,
        tid,
        &init_data_with_username(tid, &handle),
    )
    .await;
    assert_eq!(booked.status, StatusCode::OK, "body: {}", booked.body);

    let list = attendees(app, &event_id).await;
    assert_eq!(list.status, StatusCode::OK, "body: {}", list.body);
    let rows = list.body["bookings"].as_array().expect("bookings array");
    assert_eq!(rows.len(), 1, "one booking expected: {}", list.body);
    assert_eq!(
        rows[0]["username"].as_str(),
        Some(handle.as_str()),
        "the admin list must carry the handle, not just an id: {}",
        list.body
    );
    assert_eq!(rows[0]["telegram_id"].as_i64(), Some(tid));
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn every_attendee_appears_in_the_event_list() {
    // The owner asked for a per-event list; three people booking must produce
    // three rows on that event and nothing on another.
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let event_id = seed_event(&db, 10).await;
    let other_event = seed_event(&db, 10).await;
    let base = 999_610_000 + (suffix() as i64 % 100_000);

    let mut handles = Vec::new();
    for i in 0..3 {
        let tid = base + i;
        let handle = format!("guest{}_{i}", suffix());
        let resp = book(
            app.clone(),
            &event_id,
            tid,
            &init_data_with_username(tid, &handle),
        )
        .await;
        assert_eq!(resp.status, StatusCode::OK, "booking {i}: {}", resp.body);
        handles.push(handle);
    }

    let list = attendees(app.clone(), &event_id).await;
    let got: Vec<String> = list.body["bookings"]
        .as_array()
        .expect("bookings array")
        .iter()
        .filter_map(|b| b["username"].as_str().map(String::from))
        .collect();
    for h in &handles {
        assert!(
            got.contains(h),
            "{h} missing from the attendee list: {got:?}"
        );
    }

    let empty = attendees(app, &other_event).await;
    assert!(
        empty.body["bookings"]
            .as_array()
            .expect("bookings array")
            .is_empty(),
        "bookings must be scoped to their own event"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn a_booker_without_a_username_still_appears() {
    // Plenty of Telegram accounts have no @handle. They must still show up —
    // silently dropping them would understate the guest count.
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let event_id = seed_event(&db, 10).await;
    let tid = 999_620_000 + (suffix() as i64 % 100_000);

    let resp = book(
        app.clone(),
        &event_id,
        tid,
        &common::make_init_data(tid, BOT_TOKEN),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK, "body: {}", resp.body);

    let list = attendees(app, &event_id).await;
    let rows = list.body["bookings"].as_array().expect("bookings array");
    assert_eq!(rows.len(), 1, "the booking must still be listed");
    assert_eq!(rows[0]["telegram_id"].as_i64(), Some(tid));
    assert!(
        rows[0]["username"].is_null(),
        "no handle should be invented for a user who has none: {}",
        list.body
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_handle_comes_from_init_data_not_the_request_body() {
    // A client-supplied username would let anyone book under someone else's
    // handle and send the owner to the wrong person.
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let event_id = seed_event(&db, 10).await;
    let tid = 999_630_000 + (suffix() as i64 % 100_000);
    let real = format!("real{}", suffix());

    let resp = send(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/events/{event_id}/book"))
            .header("content-type", "application/json")
            .header("X-Telegram-Init-Data", init_data_with_username(tid, &real))
            .header("X-Idempotency-Key", uuid::Uuid::new_v4().to_string())
            .body(Body::from(
                serde_json::to_vec(&json!({
                    "telegram_id": tid,
                    "seats": 1,
                    "username": "impersonated_owner",
                }))
                .unwrap(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK, "body: {}", resp.body);

    let list = attendees(app, &event_id).await;
    let rows = list.body["bookings"].as_array().expect("bookings array");
    assert_eq!(
        rows[0]["username"].as_str(),
        Some(real.as_str()),
        "the signed handle must win over anything in the body"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_attendee_list_is_admin_only() {
    // It is a list of customers' contact handles.
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let event_id = seed_event(&db, 10).await;

    let resp = send(
        app,
        Request::builder()
            .uri(format!("/api/admin/events/{event_id}/bookings"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert!(
        matches!(
            resp.status,
            StatusCode::UNAUTHORIZED | StatusCode::TOO_MANY_REQUESTS
        ),
        "the attendee list must not be readable without admin auth (got {})",
        resp.status
    );
    assert!(
        resp.body["bookings"].is_null(),
        "no attendee data may leak to an unauthenticated caller"
    );
}

/// The "send the guest list to Telegram" path exists because a guest without
/// an @username cannot be opened from the Mini App at all. The send itself
/// needs a live Telegram token, so this asserts the parts that can be checked
/// without one: who is allowed to ask, and for what.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn sending_the_guest_list_is_admin_only_and_scoped_to_a_real_event() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let event_id = seed_event(&db, 10).await;

    let anon = send(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/admin/events/{event_id}/bookings/send"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert!(
        matches!(
            anon.status,
            StatusCode::UNAUTHORIZED | StatusCode::TOO_MANY_REQUESTS
        ),
        "an unauthenticated caller must not make the bot send anything (got {})",
        anon.status
    );

    // Admin, but an event that does not exist: 404 rather than an empty
    // message sent into the owner's chat.
    let missing = send(
        app,
        Request::builder()
            .method("POST")
            .uri(format!(
                "/api/admin/events/{}/bookings/send",
                uuid::Uuid::new_v4()
            ))
            .header("X-Admin-Token", admin_token())
            // 42 is the only admin in the test config (tests/common/mod.rs).
            .header("X-Admin-Telegram-Id", "42")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

/// The password-auth path reports admin id 0, so the target chat comes from a
/// header. That header must be checked against the admin list, or anyone
/// holding the admin password could make the bot message arbitrary chats.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_guest_list_cannot_be_sent_to_a_non_admin_chat() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let event_id = seed_event(&db, 10).await;

    let resp = send(
        app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/admin/events/{event_id}/bookings/send"))
            .header("X-Admin-Token", admin_token())
            .header("X-Admin-Telegram-Id", "999999999")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(
        resp.status,
        StatusCode::FORBIDDEN,
        "a non-admin chat id must be refused, not messaged"
    );
}

/// A refusal has to say which refusal it is.
///
/// Three unrelated situations answered a bare 409 — the event has already
/// started, you are already on the list, and there are no seats left. The
/// customer saw one indistinguishable failure and so did the log; a 409 in
/// production could not be told apart without reproducing it.
#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn booking_refusals_carry_a_distinct_reason() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };

    // Already booked.
    let event_id = seed_event(&db, 10).await;
    let tid = 999_640_000 + (suffix() as i64 % 100_000);
    let init = init_data_with_username(tid, &format!("dup{}", suffix()));
    assert_eq!(
        book(app.clone(), &event_id, tid, &init).await.status,
        StatusCode::OK
    );
    let dup = book(app.clone(), &event_id, tid, &init).await;
    assert_eq!(dup.status, StatusCode::CONFLICT);
    assert_eq!(
        dup.body["error"], "already_booked",
        "a repeat booking must say so: {}",
        dup.body
    );

    // Sold out: a one-seat event taken by someone else.
    let small = seed_event(&db, 1).await;
    let first = 999_650_000 + (suffix() as i64 % 100_000);
    let second = first + 1;
    assert_eq!(
        book(
            app.clone(),
            &small,
            first,
            &init_data_with_username(first, &format!("one{}", suffix()))
        )
        .await
        .status,
        StatusCode::OK
    );
    let full = book(
        app.clone(),
        &small,
        second,
        &init_data_with_username(second, &format!("two{}", suffix())),
    )
    .await;
    assert_eq!(full.status, StatusCode::CONFLICT);
    assert_eq!(
        full.body["error"], "sold_out",
        "a full event must say so rather than look like a duplicate: {}",
        full.body
    );

    // Already started — seeded in the past.
    let past = uuid::Uuid::new_v4().to_string();
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO events (id, title, description, starts_at, max_seats, is_public) \
             VALUES ($1, $2, 'past', NOW() - INTERVAL '1 hour', 10, TRUE)",
            [past.clone().into(), format!("past-{}", suffix()).into()],
        ))
        .await
        .expect("seed past event");
    let late_tid = 999_660_000 + (suffix() as i64 % 100_000);
    let late = book(
        app,
        &past,
        late_tid,
        &init_data_with_username(late_tid, &format!("late{}", suffix())),
    )
    .await;
    assert_eq!(late.status, StatusCode::CONFLICT);
    assert_eq!(
        late.body["error"], "event_started",
        "a finished event must say so, not look full: {}",
        late.body
    );
}
