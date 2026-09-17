//! Loop #20: garden invitee progress panel integration test.
//!
//! Verifies that `GET /api/referrals/me/:telegram_id/invitees` returns
//! referred users with their order status.

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::{make_app_with_db, make_init_data};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use tower::ServiceExt;

#[tokio::test]
#[ignore = "needs a throwaway DATABASE_URL and --features backend"]
async fn invitees_returns_referred_progress() {
    let (app, db) = match make_app_with_db().await {
        Some(x) => x,
        None => return,
    };

    let referrer_id = 130001i64;
    let referred_id = 130002i64;
    let referrer_init = make_init_data(referrer_id, "dummy_test_token");

    // Clean slate.
    let _ = db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM referral_events WHERE referrer_id = $1 OR referred_id = $2",
            [referrer_id.into(), referred_id.into()],
        ))
        .await;
    let _ = db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM orders WHERE telegram_id = $1",
            [referred_id.into()],
        ))
        .await;
    let _ = db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM garden_plants WHERE user_id = $1",
            [referred_id.to_string().into()],
        ))
        .await;

    // Seed referrer code so the referral_event has a code.
    let _ = db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO loyalty_profiles (telegram_id, referral_code) VALUES ($1, 'LOOP20A') ON CONFLICT (telegram_id) DO UPDATE SET referral_code = EXCLUDED.referral_code",
            [referrer_id.into()],
        ))
        .await;

    // Insert a pending referral event.
    let _ = db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO referral_events (referrer_id, referred_id, code, status, source) VALUES ($1, $2, 'LOOP20A', 'pending', 'utm_a')",
            [referrer_id.into(), referred_id.into()],
        ))
        .await;

    // Give the referred user a first name.
    let _ = db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO user_languages (telegram_id, language, first_name) VALUES ($1, 'en', 'Alex') ON CONFLICT (telegram_id) DO UPDATE SET first_name = EXCLUDED.first_name",
            [referred_id.into()],
        ))
        .await;
    // A `garden_plants` row was seeded here to give the invitee a streak of 3.
    // Migration 083 drops that table, so the insert could only fail — and this
    // fixture discards insert errors with `let _ =`, so it would have failed in
    // silence while the assertion below blamed the endpoint.

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/referrals/me/{}/invitees", referrer_id))
        .header("x-telegram-init-data", referrer_init)
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    if status != StatusCode::OK {
        eprintln!(
            "invitees response: {} {}",
            status,
            String::from_utf8_lossy(&body)
        );
    }
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["count"].as_i64(), Some(1));
    let invitees = json["invitees"].as_array().expect("invitees array");
    let first = &invitees[0];
    assert_eq!(first["status"].as_str(), Some("pending"));
    assert!(
        first.get("streak").is_none(),
        "the invitee row must not carry a garden streak: {first}"
    );
    assert_eq!(first["has_ordered"].as_bool(), Some(false));
    assert!(first["display_name"].as_str().unwrap().contains("Alex"));
    assert_eq!(first["source"].as_str(), Some("utm_a"));
}
