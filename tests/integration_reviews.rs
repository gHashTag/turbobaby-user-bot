//! Integration tests for Variant C community endpoints:
//! reviews, lab certificates, and LINE broadcast admin surface.

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sea_orm::{ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, Statement};
use tower::ServiceExt;

use woody_weed_bot::db::entities::strain_review::{Column as ReviewCol, Entity as ReviewEntity};

fn admin_headers() -> (String, &'static str, &'static str) {
    (
        common::make_init_data(42, "dummy_test_token"),
        "test_password",
        "42",
    )
}

async fn seed_strain(db: &woody_weed_bot::db::Database, strain_id: &str, name: &str) {
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO strains (id, name, price_per_gram, is_available) VALUES ($1, $2, 100.0, TRUE)",
            [strain_id.into(), name.into()],
        ))
        .await
        .expect("seed strain");
}

async fn seed_delivered_order(
    db: &woody_weed_bot::db::Database,
    order_id: &str,
    telegram_id: i64,
    strain_id: &str,
    strain_name: &str,
) {
    let items = serde_json::json!([{
        "strain_id": strain_id,
        "strain_name": strain_name,
        "quantity": 1.0,
    }]);
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO orders (id, telegram_id, items, subtotal, total, status, created_at) \
             VALUES ($1, $2, $3::jsonb, 100.0, 100.0, 'delivered', NOW())",
            [
                order_id.into(),
                telegram_id.into(),
                items.to_string().into(),
            ],
        ))
        .await
        .expect("seed delivered order");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn list_reviews_returns_empty_for_unknown_strain() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration reviews test");
        return;
    };
    let _ = db
        .orm
        .execute_unprepared("TRUNCATE strain_reviews, lab_certificates, orders")
        .await;

    let strain_id = uuid::Uuid::new_v4().to_string();
    seed_strain(&db, &strain_id, "Ghost OG").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/reviews?strain_id={}", strain_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("list reviews");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    assert_eq!(body["reviews"].as_array().unwrap().len(), 0);
    assert!(body["average_rating"].is_null());
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn user_can_create_review_for_delivered_order() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration reviews test");
        return;
    };
    let _ = db
        .orm
        .execute_unprepared("TRUNCATE strain_reviews, lab_certificates, orders, strains")
        .await;

    let user_id: i64 = 1001;
    let strain_id = uuid::Uuid::new_v4().to_string();
    let order_id = uuid::Uuid::new_v4().to_string();
    seed_strain(&db, &strain_id, "Ghost OG").await;
    seed_delivered_order(&db, &order_id, user_id, &strain_id, "Ghost OG").await;

    let init_data = common::make_init_data(user_id, "dummy_test_token");
    let body = serde_json::json!({
        "order_id": order_id,
        "strain_id": strain_id,
        "rating": 5,
        "comment": "Fire strain 🔥",
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/reviews")
                .method("POST")
                .header("X-Telegram-Init-Data", &init_data)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .expect("create review");

    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let resp: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    eprintln!("create review status={status:?} body={resp}");
    assert_eq!(status, StatusCode::OK, "create review should succeed");
    assert_eq!(resp["rating"], 5);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/reviews?strain_id={}", strain_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("list reviews");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    let reviews = body["reviews"].as_array().unwrap();
    assert_eq!(reviews.len(), 1);
    assert_eq!(body["average_rating"].as_f64().unwrap(), 5.0);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn duplicate_review_for_same_order_strain_is_rejected() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration reviews test");
        return;
    };
    let _ = db
        .orm
        .execute_unprepared("TRUNCATE strain_reviews, lab_certificates, orders, strains")
        .await;

    let user_id: i64 = 1002;
    let strain_id = uuid::Uuid::new_v4().to_string();
    let order_id = uuid::Uuid::new_v4().to_string();
    seed_strain(&db, &strain_id, "Duplicate Test").await;
    seed_delivered_order(&db, &order_id, user_id, &strain_id, "Duplicate Test").await;

    let init_data = common::make_init_data(user_id, "dummy_test_token");
    let body = serde_json::json!({
        "order_id": order_id,
        "strain_id": strain_id,
        "rating": 4,
        "comment": "First",
    });

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/reviews")
                .method("POST")
                .header("X-Telegram-Init-Data", &init_data)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .expect("first review");
    assert_eq!(first.status(), StatusCode::OK);

    let body2 = serde_json::json!({
        "order_id": order_id,
        "strain_id": strain_id,
        "rating": 3,
        "comment": "Second",
    });
    let second = app
        .oneshot(
            Request::builder()
                .uri("/api/reviews")
                .method("POST")
                .header("X-Telegram-Init-Data", &init_data)
                .header("content-type", "application/json")
                .body(Body::from(body2.to_string()))
                .unwrap(),
        )
        .await
        .expect("duplicate review");

    assert_eq!(second.status(), StatusCode::CONFLICT);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn review_for_pending_order_is_rejected() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration reviews test");
        return;
    };
    let _ = db
        .orm
        .execute_unprepared("TRUNCATE strain_reviews, lab_certificates, orders, strains")
        .await;

    let user_id: i64 = 1003;
    let strain_id = uuid::Uuid::new_v4().to_string();
    let order_id = uuid::Uuid::new_v4().to_string();
    seed_strain(&db, &strain_id, "Pending Test").await;

    let items = serde_json::json!([{
        "strain_id": strain_id,
        "strain_name": "Pending Test",
        "quantity": 1.0,
    }]);
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO orders (id, telegram_id, items, subtotal, total, status, created_at) \
             VALUES ($1, $2, $3::jsonb, 100.0, 100.0, 'pending', NOW())",
            [
                order_id.clone().into(),
                user_id.into(),
                items.to_string().into(),
            ],
        ))
        .await
        .expect("seed pending order");

    let init_data = common::make_init_data(user_id, "dummy_test_token");
    let body = serde_json::json!({
        "order_id": order_id,
        "strain_id": strain_id,
        "rating": 5,
        "comment": "Too early",
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reviews")
                .method("POST")
                .header("X-Telegram-Init-Data", &init_data)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .expect("review pending order");

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn admin_can_moderate_review() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration reviews test");
        return;
    };
    let _ = db
        .orm
        .execute_unprepared("TRUNCATE strain_reviews, lab_certificates, orders, strains")
        .await;

    let user_id: i64 = 1004;
    let strain_id = uuid::Uuid::new_v4().to_string();
    let order_id = uuid::Uuid::new_v4().to_string();
    seed_strain(&db, &strain_id, "Mod Test").await;
    seed_delivered_order(&db, &order_id, user_id, &strain_id, "Mod Test").await;

    let init_data = common::make_init_data(user_id, "dummy_test_token");
    let body = serde_json::json!({
        "order_id": order_id,
        "strain_id": strain_id,
        "rating": 2,
        "comment": "Meh",
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/reviews")
                .method("POST")
                .header("X-Telegram-Init-Data", &init_data)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .expect("create review");
    assert_eq!(response.status(), StatusCode::OK);

    // Default approved=true; find id, then unapprove.
    let review_id = ReviewEntity::find()
        .filter(ReviewCol::OrderId.eq(&order_id))
        .one(&db.orm)
        .await
        .expect("find review")
        .expect("review exists")
        .id;

    let (admin_init, admin_token, admin_telegram_id) = admin_headers();
    let body = serde_json::json!({ "approved": false });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/reviews/{}/moderate", review_id))
                .method("POST")
                .header("X-Telegram-Init-Data", admin_init)
                .header("X-Admin-Token", admin_token)
                .header("X-Admin-Telegram-Id", admin_telegram_id)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .expect("moderate review");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    assert_eq!(body["approved"], false);

    // Public list should now hide the review.
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/reviews?strain_id={}", strain_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("list reviews after moderation");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    assert!(body["reviews"].as_array().unwrap().is_empty());
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn admin_can_create_and_list_lab_cert() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration reviews test");
        return;
    };
    let _ = db
        .orm
        .execute_unprepared("TRUNCATE strain_reviews, lab_certificates, orders, strains")
        .await;

    let strain_id = uuid::Uuid::new_v4().to_string();
    seed_strain(&db, &strain_id, "Lab Test").await;

    let (admin_init, admin_token, admin_telegram_id) = admin_headers();
    let body = serde_json::json!({
        "certificate_url": "/uploads/lab-1.pdf",
        "tested_at": "2026-07-20",
        "thc_percent": 24.5,
        "cbd_percent": 1.2,
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/strains/{}/lab-cert", strain_id))
                .method("POST")
                .header("X-Telegram-Init-Data", admin_init.clone())
                .header("X-Admin-Token", admin_token)
                .header("X-Admin-Telegram-Id", admin_telegram_id)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .expect("create lab cert");

    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/strains/{}/lab-certs", strain_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("list lab certs");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    let certs = body["lab_certificates"].as_array().unwrap();
    assert_eq!(certs.len(), 1);
    assert_eq!(certs[0]["thc_percent"], 24.5);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn telegram_broadcast_with_no_users_returns_success() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping integration reviews test");
        return;
    };
    let _ = db
        .orm
        .execute_unprepared(
            "TRUNCATE user_languages, loyalty_profiles, strain_reviews, lab_certificates, orders, strains",
        )
        .await;

    let (admin_init, admin_token, admin_telegram_id) = admin_headers();
    let body = serde_json::json!({ "text": "Hello Telegram friends" });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/broadcast")
                .method("POST")
                .header("X-Telegram-Init-Data", admin_init)
                .header("X-Admin-Token", admin_token)
                .header("X-Admin-Telegram-Id", admin_telegram_id)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .expect("telegram broadcast");

    // The broadcast handler returns a structured success response; with the
    // dummy test bot every send fails at the Telegram API layer, so sent=0
    // and failed equals the number of unique recipients found in the test DB.
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("valid json");
    assert_eq!(json["success"], true);
    let recipients = json["recipients"].as_u64().expect("recipients count");
    let sent = json["sent"].as_u64().unwrap_or(0);
    let failed = json["failed"].as_u64().unwrap_or(0);
    assert_eq!(recipients, sent + failed);
}
