//! R1 and R2, the owner's answers of 2026-09-26, on a real PostgreSQL.
//!
//! * R1, verbatim «Только для админа»: `GET /api/loyalty/leaderboard` (first
//!   names, total spend, tier) answers an admin only; anyone else gets 401.
//! * R2, verbatim «Закрыть для клиентов»: `GET /api/quest-places`,
//!   `GET /api/treasure-hunts` and `GET /api/loyalty/config`, which no mounted
//!   screen reads, answer a caller without admin proof exactly as an unmatched
//!   `/api` path does; an admin is served the rows as before.
//!
//! Every unauthenticated request carries an address of its own, so the admin
//! limiter (10 failures per address per 5 minutes) never trips mid-test.
//! `#[ignore]`d: run only against a private, throwaway PostgreSQL:
//! ```sh
//! DATABASE_URL=postgres://postgres@127.0.0.1:<port>/<fresh db> \
//!   cargo test --features backend --test integration_closed_reads -- --ignored --test-threads=1
//! ```

#![cfg(feature = "backend")]
#![allow(clippy::panic, clippy::expect_used)]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::make_app_with_db;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::Value;
use tower::ServiceExt;

const BOT_TOKEN: &str = "dummy_test_token";

struct Answer {
    status: StatusCode,
    content_type: Option<String>,
    body: Value,
}

async fn get(app: &axum::Router, uri: &str, admin: bool, from: &str) -> Answer {
    send(app, Method::GET, uri, admin, from, None).await
}

async fn send(
    app: &axum::Router,
    method: Method,
    uri: &str,
    admin: bool,
    from: &str,
    body: Option<Value>,
) -> Answer {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("X-Forwarded-For", from);
    if admin {
        req = req.header(
            "X-Admin-Token",
            turbobaby_bot::api::auth::generate_admin_token("test_password", BOT_TOKEN),
        );
    }
    let req = match body {
        Some(body) => req
            .header("content-type", "application/json")
            .body(Body::from(body.to_string())),
        None => req.body(Body::empty()),
    }
    .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    let status = resp.status();
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .expect("body");
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    Answer {
        status,
        content_type,
        body,
    }
}

#[tokio::test]
#[ignore]
async fn r1_the_loyalty_leaderboard_answers_an_admin_only() {
    let Some((app, _db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };
    let anonymous = get(&app, "/api/loyalty/leaderboard", false, "10.61.0.1").await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    let admin = get(&app, "/api/loyalty/leaderboard", true, "10.61.0.2").await;
    assert_eq!(admin.status, StatusCode::OK, "{}", admin.body);
    assert!(admin.body["leaderboard"].is_array(), "{}", admin.body);
}

#[tokio::test]
#[ignore]
async fn r2_the_closed_reads_answer_like_a_missing_route_and_serve_an_admin() {
    let Some((app, db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };
    let marker = format!("closed-read-{}", uuid::Uuid::new_v4());
    for sql in [
        "INSERT INTO quest_places (name) VALUES ($1)",
        "INSERT INTO treasure_hunts (name, is_active) VALUES ($1, true)",
    ] {
        db.orm
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                [marker.clone().into()],
            ))
            .await
            .expect(sql);
    }

    // A real miss beside each path: its answer, with only the path changed.
    for (n, (closed, sibling, list)) in [
        ("/quest-places", "/quest-placez", Some("quest_places")),
        ("/treasure-hunts", "/treasure-huntz", Some("treasure_hunts")),
        ("/loyalty/config", "/loyalty/config/nothing", None),
    ]
    .into_iter()
    .enumerate()
    {
        let miss = get(
            &app,
            &format!("/api{sibling}"),
            false,
            &format!("10.62.{n}.1"),
        )
        .await;
        assert_eq!(miss.status, StatusCode::NOT_FOUND);
        let shut = get(
            &app,
            &format!("/api{closed}"),
            false,
            &format!("10.62.{n}.2"),
        )
        .await;
        assert_eq!(
            shut.status,
            StatusCode::NOT_FOUND,
            "{closed}: {}",
            shut.body
        );
        assert_eq!(shut.content_type, miss.content_type, "{closed}");
        assert_eq!(
            shut.body.to_string(),
            miss.body.to_string().replace(sibling, closed),
            "{closed} does not answer like a missing route"
        );
        assert_eq!(shut.body["error"], "not_found");
        assert_eq!(
            shut.body["detail"],
            format!("no API route matches {closed}")
        );

        let served = get(
            &app,
            &format!("/api{closed}"),
            true,
            &format!("10.62.{n}.3"),
        )
        .await;
        assert_eq!(served.status, StatusCode::OK, "{closed}: {}", served.body);
        match list {
            Some(key) => assert!(
                served.body[key]
                    .as_array()
                    .expect("a list")
                    .iter()
                    .any(|row| row["name"] == marker.as_str()),
                "{closed} lost its rows for the admin: {}",
                served.body
            ),
            None => assert!(served.body.get("config").is_some(), "{}", served.body),
        }
    }

    // The admin writes on those paths are unchanged: still 401 without proof.
    let write = send(
        &app,
        Method::POST,
        "/api/quest-places",
        false,
        "10.63.0.1",
        Some(serde_json::json!({"name": "x", "lat": 0.0, "lon": 0.0})),
    )
    .await;
    assert_eq!(write.status, StatusCode::UNAUTHORIZED);
}
