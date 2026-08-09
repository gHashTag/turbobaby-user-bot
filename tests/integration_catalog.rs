//! Integration tests for the catalog CRUD endpoints.
//!
//! `src/api/catalog.rs` sat at 26.8% coverage across four near-identical
//! product families (accessories, accessory sets, tea products, tea sets) plus
//! strain sets. Near-identical is exactly the shape where one family quietly
//! drifts from the others — a missing admin check or a missing visibility
//! filter on one of five endpoints looks like nothing at all until it is
//! exploited.
//!
//! These tests therefore run the same contract over every family in a table
//! rather than testing one and assuming the rest.
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
use serde_json::{json, Value};
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

fn suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}

/// One product family: its collection path, the JSON key its list comes back
/// under, and a minimal valid create body.
struct Family {
    path: &'static str,
    list_key: &'static str,
    price_field: &'static str,
}

const FAMILIES: [Family; 4] = [
    Family {
        path: "accessories",
        list_key: "accessories",
        price_field: "price",
    },
    Family {
        path: "accessory-sets",
        list_key: "accessory_sets",
        price_field: "total_price",
    },
    Family {
        path: "tea-products",
        list_key: "tea_products",
        price_field: "price",
    },
    Family {
        path: "tea-sets",
        list_key: "tea_sets",
        price_field: "total_price",
    },
];

fn create_body(f: &Family, name: &str, price: f64) -> Value {
    let mut body = json!({ "name": name, "description": "test item" });
    body[f.price_field] = json!(price);
    body
}

async fn create(app: axum::Router, f: &Family, body: &Value, authed: bool) -> Resp {
    let mut builder = Request::builder()
        .method("POST")
        .uri(format!("/api/{}", f.path))
        .header("content-type", "application/json");
    if authed {
        builder = builder.header("X-Admin-Token", admin_token());
    }
    send(
        app,
        builder
            .body(Body::from(serde_json::to_vec(body).unwrap()))
            .unwrap(),
    )
    .await
}

async fn set_available(app: axum::Router, f: &Family, id: &str, available: bool) -> Resp {
    send(
        app,
        Request::builder()
            .method("PUT")
            .uri(format!("/api/{}/{}/availability", f.path, id))
            .header("content-type", "application/json")
            .header("X-Admin-Token", admin_token())
            .body(Body::from(
                serde_json::to_vec(&json!({ "is_available": available })).unwrap(),
            ))
            .unwrap(),
    )
    .await
}

async fn list_public(app: axum::Router, f: &Family) -> Resp {
    send(
        app,
        Request::builder()
            .uri(format!("/api/{}", f.path))
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

fn contains_name(body: &Value, list_key: &str, name: &str) -> bool {
    body[list_key]
        .as_array()
        .map(|items| items.iter().any(|i| i["name"] == name))
        .unwrap_or(false)
}

// ── Authorisation ────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn creating_a_product_requires_admin_in_every_family() {
    // One family missing its check_admin would let anyone add items to the
    // shop. Asserting per family is the point — they are copy-pasted code.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let s = suffix();

    for f in &FAMILIES {
        let body = create_body(f, &format!("anon-{}-{s}", f.path), 100.0);
        let resp = create(app.clone(), f, &body, false).await;
        assert_eq!(
            resp.status,
            StatusCode::UNAUTHORIZED,
            "{} must refuse an unauthenticated create",
            f.path
        );
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn an_unauthenticated_create_writes_nothing() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let s = suffix();

    for f in &FAMILIES {
        let name = format!("ghost-{}-{s}", f.path);
        create(app.clone(), f, &create_body(f, &name, 100.0), false).await;
        let list = list_public(app.clone(), f).await;
        assert!(
            !contains_name(&list.body, f.list_key, &name),
            "{} leaked a refused create into the catalog",
            f.path
        );
    }
}

// ── Visibility ───────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn taking_an_item_off_sale_hides_it_from_the_public_list() {
    // The same asymmetry between a write flag and a read filter is what broke
    // strain-of-day. Assert both directions, for every family.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let s = suffix();

    for f in &FAMILIES {
        let name = format!("vis-{}-{s}", f.path);
        let created = create(app.clone(), f, &create_body(f, &name, 250.0), true).await;
        assert_eq!(
            created.status,
            StatusCode::OK,
            "{} create failed: {}",
            f.path,
            created.body
        );
        let id = created.body["id"].as_str().expect("created id").to_string();

        assert!(
            contains_name(&list_public(app.clone(), f).await.body, f.list_key, &name),
            "{} must list a freshly created item",
            f.path
        );

        assert_eq!(
            set_available(app.clone(), f, &id, false).await.status,
            StatusCode::OK
        );
        assert!(
            !contains_name(&list_public(app.clone(), f).await.body, f.list_key, &name),
            "{} still shows an item taken off sale",
            f.path
        );

        assert_eq!(
            set_available(app.clone(), f, &id, true).await.status,
            StatusCode::OK
        );
        assert!(
            contains_name(&list_public(app.clone(), f).await.body, f.list_key, &name),
            "{} did not bring the item back when put on sale again",
            f.path
        );
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_admin_listing_shows_hidden_items_but_only_to_an_admin() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let s = suffix();

    for f in &FAMILIES {
        let name = format!("hidden-{}-{s}", f.path);
        let created = create(app.clone(), f, &create_body(f, &name, 10.0), true).await;
        let id = created.body["id"].as_str().expect("created id").to_string();
        set_available(app.clone(), f, &id, false).await;

        // Anonymous caller asking for hidden rows must be refused outright,
        // not quietly served the public list.
        let anon = send(
            app.clone(),
            Request::builder()
                .uri(format!("/api/{}?include_hidden=1", f.path))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        // 401 normally; 429 once the auth-failure limiter trips, since this
        // loop makes several rejected calls from one client. Either is a
        // refusal — what must never happen is a 200 carrying hidden rows.
        assert!(
            matches!(
                anon.status,
                StatusCode::UNAUTHORIZED | StatusCode::TOO_MANY_REQUESTS
            ),
            "{} served include_hidden without auth (status {})",
            f.path,
            anon.status
        );
        assert!(
            !contains_name(&anon.body, f.list_key, &name),
            "{} leaked an off-sale item to an anonymous caller",
            f.path
        );

        let admin = send(
            app.clone(),
            Request::builder()
                .uri(format!("/api/{}?include_hidden=1", f.path))
                .header("X-Admin-Token", admin_token())
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(admin.status, StatusCode::OK);
        assert!(
            contains_name(&admin.body, f.list_key, &name),
            "{} admin listing must include the off-sale item",
            f.path
        );
    }
}

// ── Validation ───────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn impossible_prices_are_refused_in_every_family() {
    // A negative or NaN price reaching the DB would poison every order total
    // computed from it.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let s = suffix();

    for f in &FAMILIES {
        for price in [-1.0_f64, 1_000_001.0] {
            let body = create_body(f, &format!("badprice-{}-{s}", f.path), price);
            let resp = create(app.clone(), f, &body, true).await;
            assert_eq!(
                resp.status,
                StatusCode::BAD_REQUEST,
                "{} accepted price {price}",
                f.path
            );
        }
        // Zero is legitimate — freebies and gifts exist.
        let ok = create(
            app.clone(),
            f,
            &create_body(f, &format!("free-{}-{s}", f.path), 0.0),
            true,
        )
        .await;
        assert_eq!(
            ok.status,
            StatusCode::OK,
            "{} rejected a zero price: {}",
            f.path,
            ok.body
        );
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn an_over_long_name_is_refused() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };

    for f in &FAMILIES {
        let body = create_body(f, &"n".repeat(201), 10.0);
        let resp = create(app.clone(), f, &body, true).await;
        assert_eq!(
            resp.status,
            StatusCode::BAD_REQUEST,
            "{} accepted a 201-character name",
            f.path
        );
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn an_over_long_id_is_refused_before_the_database() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let long = "x".repeat(201);

    for f in &FAMILIES {
        let resp = set_available(app.clone(), f, &long, false).await;
        assert_eq!(
            resp.status,
            StatusCode::BAD_REQUEST,
            "{} accepted a 201-character id",
            f.path
        );
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn a_malformed_availability_body_is_refused() {
    // `is_available` missing or of the wrong type must not be read as `false`
    // and silently pull an item from the shop.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let s = suffix();
    let f = &FAMILIES[0];

    let created = create(
        app.clone(),
        f,
        &create_body(f, &format!("malformed-{s}"), 50.0),
        true,
    )
    .await;
    let id = created.body["id"].as_str().expect("created id").to_string();

    for body in [json!({}), json!({ "is_available": "yes" })] {
        let resp = send(
            app.clone(),
            Request::builder()
                .method("PUT")
                .uri(format!("/api/{}/{}/availability", f.path, id))
                .header("content-type", "application/json")
                .header("X-Admin-Token", admin_token())
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await;
        assert_eq!(
            resp.status,
            StatusCode::BAD_REQUEST,
            "body {body} must be refused"
        );
    }

    assert!(
        contains_name(
            &list_public(app, f).await.body,
            f.list_key,
            &format!("malformed-{s}")
        ),
        "a refused availability call must leave the item on sale"
    );
}

// ── Strain sets ──────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_public_sets_endpoint_always_answers() {
    // `get_sets` deliberately degrades each source to an empty list on error
    // rather than 500'ing the whole page — a real incident twice in 2026-06.
    // Whatever else changes, this endpoint must not fail.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let resp = send(
        app,
        Request::builder()
            .uri("/api/sets")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert!(
        resp.body["sets"].is_array(),
        "sets must always be an array, got {}",
        resp.body
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn creating_a_strain_set_requires_admin() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let resp = send(
        app,
        Request::builder()
            .method("POST")
            .uri("/api/sets")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&json!({
                    "name": format!("anon-set-{}", suffix()),
                    "total_price": 100.0,
                    "strain_ids": [],
                }))
                .unwrap(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
}
