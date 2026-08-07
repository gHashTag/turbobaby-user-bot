//! Loop #19: garden invite deep-link referral flow integration test.
//!
//! Verifies that `POST /api/referrals/me/:telegram_id/garden-invite` creates a
//! pending `referral_events` row attributed to the garden source.

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::{make_app_with_db, make_init_data};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use tower::ServiceExt;

#[tokio::test]
#[ignore = "needs a throwaway DATABASE_URL and --features backend"]
async fn garden_invite_records_pending_referral() {
    let (app, db) = match make_app_with_db().await {
        Some(x) => x,
        None => return,
    };

    let referrer_id = 110001i64;
    let referred_id = 110002i64;
    let init_data = make_init_data(referred_id, "dummy_test_token");

    // Ensure a clean slate for the referred user.
    let _ = db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM referral_events WHERE referred_id = $1",
            [referred_id.into()],
        ))
        .await;

    let body = serde_json::json!({
        "referrer_id": referrer_id,
        "source": "utm_a",
    })
    .to_string();

    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/referrals/me/{}/garden-invite", referred_id))
        .header("content-type", "application/json")
        .header("x-telegram-init-data", init_data)
        .body(Body::from(body))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let row = db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT referrer_id, referred_id, status, source FROM referral_events WHERE referred_id = $1",
            [referred_id.into()],
        ))
        .await
        .unwrap()
        .expect("referral_event row should exist");

    assert_eq!(
        row.try_get::<i64>("", "referrer_id").unwrap(),
        referrer_id
    );
    assert_eq!(
        row.try_get::<i64>("", "referred_id").unwrap(),
        referred_id
    );
    assert_eq!(
        row.try_get::<String>("", "status").unwrap(),
        "pending"
    );
    assert_eq!(
        row.try_get::<Option<String>>("", "source").unwrap().as_deref(),
        Some("utm_a")
    );
}
