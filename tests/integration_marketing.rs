//! Integration tests for ТЗ #2 marketing endpoints (cycle #167):
//!   * `POST /api/strains/bulk-marketing` — flip flags across many
//!     strains in one call (cycle #133-C).
//!   * `GET/PUT /api/admin/marketing-display` — admin self-service
//!     "hide all marketing badges" toggle (cycle #136).
//!
//! Closes the explicit deferred-coverage item from
//! `docs/TZ2_MARKETING.md`:
//!   > "No `cargo test` integration test for
//!   > `/api/strains/bulk-marketing`. The validator has seven unit
//!   > tests; the full POST-DB-GET round-trip is covered by manual
//!   > smoke only."
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
use sea_orm::{ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, Statement};
use serde_json::json;
use tower::ServiceExt;

use turbobaby_bot::api::auth::generate_admin_token;
use turbobaby_bot::db::entities::strain::{Column as StrainCol, Entity as StrainEntity};

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn bulk_marketing_flips_flags_and_persists() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let admin_token = generate_admin_token("test_password", "dummy_test_token");
    let suffix = rand_suffix();

    // Seed three strains, all with `is_best_seller = FALSE` (default).
    // Unique names per run avoid migration-008 seed collisions.
    let strain_ids: Vec<String> = (0..3).map(|_| uuid::Uuid::new_v4().to_string()).collect();
    for (i, id) in strain_ids.iter().enumerate() {
        let name = format!("integration-marketing-{}-{}", suffix, i);
        db.orm
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO strains (id, name, price_per_gram, is_available) \
                 VALUES ($1, $2, $3, TRUE)",
                [id.clone().into(), name.into(), 100.0_f64.into()],
            ))
            .await
            .expect("seed strain INSERT");
    }

    // POST bulk-marketing — flip is_best_seller=true for all three.
    let body = json!({
        "ids": strain_ids,
        "is_best_seller": true,
    });
    let request = Request::builder()
        .method("POST")
        .uri("/api/strains/bulk-marketing")
        .header("content-type", "application/json")
        .header("x-admin-token", &admin_token)
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let response = app.oneshot(request).await.expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let resp: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "bulk-marketing must succeed, body: {}",
        resp
    );
    assert_eq!(resp["success"], true);
    assert_eq!(
        resp["updated"], 3,
        "expected 3 rows updated, body: {}",
        resp
    );

    // DB assertion: each seeded strain now has is_best_seller = true,
    // is_new_arrival = false, sale_active = false (untouched flags
    // stay at their defaults).
    let rows = StrainEntity::find()
        .filter(StrainCol::Id.is_in(strain_ids.clone()))
        .all(&db.orm)
        .await
        .expect("strains query");
    assert_eq!(rows.len(), 3, "all three seeded strains must come back");
    for row in &rows {
        assert!(
            row.is_best_seller,
            "strain {} must be is_best_seller=true",
            row.id
        );
        assert!(!row.is_new_arrival, "is_new_arrival untouched → false");
        assert!(!row.sale_active, "sale_active untouched → false");
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn marketing_display_toggle_round_trips() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let admin_token = generate_admin_token("test_password", "dummy_test_token");

    // Read initial state. test_config sets HIDE_MARKETING_BADGES=false,
    // so env_override must be false. db_hidden could be anything from
    // a prior test run; we read + restore at the end.
    let initial = get_marketing_display(app.clone(), &admin_token).await;
    assert_eq!(initial.status, StatusCode::OK);
    assert_eq!(
        initial.body["env_override"], false,
        "test_config must have env_override = false"
    );
    let initial_db_hidden = initial.body["db_hidden"]
        .as_bool()
        .expect("db_hidden must be bool");

    // Flip to true (or back to false if it was already true).
    let target = !initial_db_hidden;
    let put_resp = put_marketing_display(app.clone(), &admin_token, target).await;
    assert_eq!(put_resp.status, StatusCode::OK, "PUT must succeed");

    // Read back — db_hidden should now equal `target`, hidden too
    // since env_override is false.
    let after = get_marketing_display(app.clone(), &admin_token).await;
    assert_eq!(after.status, StatusCode::OK);
    assert_eq!(
        after.body["db_hidden"], target,
        "db_hidden must reflect the PUT, got body: {}",
        after.body
    );
    assert_eq!(after.body["hidden"], target);

    // Cleanup: restore original state so this test is idempotent.
    let cleanup = put_marketing_display(app, &admin_token, initial_db_hidden).await;
    assert_eq!(cleanup.status, StatusCode::OK, "cleanup PUT must succeed");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn marketing_display_put_rejects_non_bool() {
    // `set_marketing_display` parses `body["hidden"].as_bool()` →
    // BAD_REQUEST on anything else. Negative-path coverage.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let admin_token = generate_admin_token("test_password", "dummy_test_token");

    for bad_body in [
        json!({"hidden": "true"}), // string, not bool
        json!({"hidden": 1}),      // int
        json!({}),                 // missing
        json!({"other": true}),    // wrong key
    ] {
        let resp = put_marketing_display_raw(app.clone(), &admin_token, &bad_body).await;
        assert_eq!(
            resp.status,
            StatusCode::BAD_REQUEST,
            "body {:?} must yield 400, got {}",
            bad_body,
            resp.status
        );
    }
}

// ── helpers ─────────────────────────────────────────────────────────

struct Resp {
    status: StatusCode,
    body: serde_json::Value,
}

async fn get_marketing_display(app: axum::Router, admin_token: &str) -> Resp {
    let request = Request::builder()
        .method("GET")
        .uri("/api/admin/marketing-display")
        .header("x-admin-token", admin_token)
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    Resp { status, body }
}

async fn put_marketing_display(app: axum::Router, admin_token: &str, hidden: bool) -> Resp {
    put_marketing_display_raw(app, admin_token, &json!({ "hidden": hidden })).await
}

async fn put_marketing_display_raw(
    app: axum::Router,
    admin_token: &str,
    body: &serde_json::Value,
) -> Resp {
    let request = Request::builder()
        .method("PUT")
        .uri("/api/admin/marketing-display")
        .header("content-type", "application/json")
        .header("x-admin-token", admin_token)
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap();
    let response = app.oneshot(request).await.expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    Resp { status, body }
}

fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}
