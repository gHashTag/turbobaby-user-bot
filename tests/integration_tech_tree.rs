//! Integration tests for the tech-tree endpoints.
//!
//! `src/api/tech_tree.rs` sat at 11.6% coverage. The interesting behaviour is
//! the dependency cascade in `complete_tech_node`: completing one node must
//! unlock exactly those locked nodes whose dependencies are *all* satisfied,
//! and no others. That is raw SQL with a nested `NOT EXISTS … unnest()`, the
//! kind of query that is easy to get subtly wrong and impossible to eyeball.
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
use serde_json::Value;
use tower::ServiceExt;

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
    woody_weed_bot::api::auth::generate_admin_token("test_password", "dummy_test_token")
}

async fn complete_node(app: axum::Router, id: &str, authed: bool) -> Resp {
    let mut builder = Request::builder()
        .method("POST")
        .uri(format!("/api/tech-tree/nodes/{id}/complete"));
    if authed {
        builder = builder.header("X-Admin-Token", admin_token());
    }
    send(app, builder.body(Body::empty()).unwrap()).await
}

/// Insert a node. `deps` are ids this node waits on.
async fn seed_node(
    db: &woody_weed_bot::db::Database,
    id: &str,
    status: &str,
    deps: &[&str],
) {
    let deps_literal = format!(
        "{{{}}}",
        deps.iter()
            .map(|d| format!("\"{d}\""))
            .collect::<Vec<_>>()
            .join(",")
    );
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tech_nodes \
             (id, name, description, category, icon, status, xp_required, xp_reward, \
              dependencies, unlocks, features, estimated_hours, priority) \
             VALUES ($1, $2, '', 'core', '🌱', $3, 0, 0, $4::text[], '{}'::text[], \
                     '{}'::text[], 0, 1)",
            [
                id.into(),
                format!("node {id}").into(),
                status.into(),
                deps_literal.into(),
            ],
        ))
        .await
        .expect("seed tech node INSERT");
}

async fn status_of(db: &woody_weed_bot::db::Database, id: &str) -> String {
    db.orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT status FROM tech_nodes WHERE id = $1",
            [id.into()],
        ))
        .await
        .expect("status query")
        .expect("node exists")
        .try_get::<String>("", "status")
        .expect("status column")
}

fn suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn completing_a_node_unlocks_only_nodes_whose_deps_are_all_satisfied() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let s = suffix();
    let (a, b) = (format!("tt-a-{s}"), format!("tt-b-{s}"));
    // `ready` waits only on A. `blocked` waits on A *and* B, and B stays
    // incomplete — so completing A must not unlock it.
    let (ready, blocked) = (format!("tt-ready-{s}"), format!("tt-blocked-{s}"));

    seed_node(&db, &a, "available", &[]).await;
    seed_node(&db, &b, "available", &[]).await;
    seed_node(&db, &ready, "locked", &[&a]).await;
    seed_node(&db, &blocked, "locked", &[&a, &b]).await;

    let resp = complete_node(app, &a, true).await;
    assert_eq!(resp.status, StatusCode::OK, "body: {}", resp.body);
    assert_eq!(resp.body["success"], true);

    assert_eq!(status_of(&db, &a).await, "completed");
    assert_eq!(
        status_of(&db, &ready).await,
        "available",
        "a node whose only dependency just completed must unlock"
    );
    assert_eq!(
        status_of(&db, &blocked).await,
        "locked",
        "a node with an unmet second dependency must stay locked — this is the \
         half of the cascade that is easy to get wrong"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_last_dependency_completes_a_chain() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let s = suffix();
    let (a, b) = (format!("tt2-a-{s}"), format!("tt2-b-{s}"));
    let target = format!("tt2-target-{s}");

    seed_node(&db, &a, "available", &[]).await;
    seed_node(&db, &b, "available", &[]).await;
    seed_node(&db, &target, "locked", &[&a, &b]).await;

    complete_node(app.clone(), &a, true).await;
    assert_eq!(
        status_of(&db, &target).await,
        "locked",
        "still waiting on the second dependency"
    );

    complete_node(app, &b, true).await;
    assert_eq!(
        status_of(&db, &target).await,
        "available",
        "with every dependency completed the node must unlock"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn completing_the_same_node_twice_is_a_reported_no_op() {
    // Double-tapping the admin button must not look like a failure, and must
    // not claim to have unlocked anything a second time.
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let s = suffix();
    let id = format!("tt3-{s}");
    seed_node(&db, &id, "available", &[]).await;

    assert_eq!(
        complete_node(app.clone(), &id, true).await.status,
        StatusCode::OK
    );
    let second = complete_node(app, &id, true).await;
    assert_eq!(second.status, StatusCode::OK, "body: {}", second.body);
    assert_eq!(second.body["success"], true);
    assert_eq!(second.body["unlocked"], 0);
    assert_eq!(second.body["note"], "already completed");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn completing_an_unknown_node_is_not_found() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let resp = complete_node(app, &format!("tt-missing-{}", suffix()), true).await;
    assert_eq!(
        resp.status,
        StatusCode::NOT_FOUND,
        "an unknown id must be 404, not a silent success"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn completing_a_node_requires_admin() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let s = suffix();
    let id = format!("tt4-{s}");
    seed_node(&db, &id, "available", &[]).await;

    let resp = complete_node(app, &id, false).await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        status_of(&db, &id).await,
        "available",
        "an unauthorised call must not change any state"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn listing_nodes_is_public_and_reports_a_consistent_total() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    seed_node(&db, &format!("tt5-{}", suffix()), "available", &[]).await;

    let resp = send(
        app,
        Request::builder()
            .uri("/api/tech-tree/nodes")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    let nodes = resp.body["nodes"].as_array().expect("nodes array");
    assert!(!nodes.is_empty(), "the seeded node must be listed");
    assert_eq!(
        resp.body["total"].as_u64().unwrap_or(0) as usize,
        nodes.len(),
        "`total` must match the array it describes"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn fetching_a_single_node_returns_it_or_404() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let s = suffix();
    let id = format!("tt6-{s}");
    seed_node(&db, &id, "available", &[]).await;

    let found = send(
        app.clone(),
        Request::builder()
            .uri(format!("/api/tech-tree/nodes/{id}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(found.status, StatusCode::OK, "body: {}", found.body);

    let missing = send(
        app,
        Request::builder()
            .uri(format!("/api/tech-tree/nodes/tt6-missing-{s}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn an_over_long_node_id_is_rejected_before_touching_the_database() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let resp = complete_node(app, &"x".repeat(201), true).await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
}
