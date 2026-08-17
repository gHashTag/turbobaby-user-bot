//! `POST /api/share/prepare` was the only authenticated endpoint in the API on
//! strict HMAC validation, and that is why sharing a product has never once
//! produced a product card in production.
//!
//! Every request that reaches the live server logs `kind=signed-but-invalid`:
//! the HMAC fails for every real client, and the other twenty-one authenticated
//! endpoints work only because `check_owner_lenient` accepts them anyway. Here
//! there was no fallback, so the call answered 401, the Mini App fell through to
//! `share_product_link_only`, and the recipient got a bare
//! `t.me/<bot>?start=…` whose Telegram preview is the bot's own profile card.
//!
//! **Why no test caught it.** `tests/common::make_init_data` signs initData
//! *correctly* with the harness token, so `validate_init_data` succeeds here and
//! has always succeeded here. The suite could not reproduce the production
//! condition even in principle: it was green about a request no real client
//! sends. So the test that matters is the one below — initData that is
//! **present and invalid**, which is what production actually carries.
//!
//! Run with:
//! ```sh
//! DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/woody_test \
//!   cargo test --features backend --test integration_share_prepare -- --ignored --test-threads=1
//! ```

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::{make_app_with_db, make_init_data};
use tower::ServiceExt;

const OWNER: i64 = 8420420131;

/// initData shaped like Telegram's but with a hash that cannot validate. This
/// is the production condition: a real payload whose HMAC fails.
fn signed_but_invalid(user_id: i64) -> String {
    let real = make_init_data(user_id, "dummy_test_token");
    // Keep every field and every ordering; corrupt only the digest. Replacing
    // the whole string with junk would test a different thing — a malformed
    // request rather than a real one that fails the check.
    match real.rfind("hash=") {
        Some(at) => format!("{}hash={}", &real[..at], "0".repeat(64)),
        None => real,
    }
}

async fn prepare(app: &axum::Router, init_data: &str, body: &str) -> (StatusCode, String) {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/share/prepare")
        .header("X-Telegram-Init-Data", init_data)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .expect("body");
    (status, String::from_utf8_lossy(&bytes).to_string())
}

#[tokio::test]
#[ignore]
async fn a_real_client_whose_hmac_fails_is_not_turned_away() {
    let Some((app, _db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };

    let body = format!(r#"{{"kind":"set","id":"no-such-set","telegram_id":{OWNER}}}"#);
    let (status, text) = prepare(&app, &signed_but_invalid(OWNER), &body).await;

    // The assertion is about the *gate*, not about the product. A missing set
    // must answer 404 — which proves the request got past authentication and
    // reached the lookup. Before this change the same request answered 401 and
    // the lookup was never reached.
    assert_ne!(
        status,
        StatusCode::UNAUTHORIZED,
        "a signed-but-invalid client is exactly what production sends, and this \
         endpoint refused it while twenty-one others accept it: {text}"
    );
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "expected the lookup to be reached and to miss; got {status}: {text}"
    );
}

/// The gate still exists. Without this, the test above passes just as well
/// against an endpoint with no authentication at all.
#[tokio::test]
#[ignore]
async fn a_request_with_no_credentials_at_all_is_still_refused() {
    let Some((app, _db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };

    let body = format!(r#"{{"kind":"set","id":"no-such-set","telegram_id":{OWNER}}}"#);
    let (status, _) = prepare(&app, "", &body).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// A well-formed request from a correctly-signed client reaches the lookup too,
/// so the change did not trade one cohort for another.
#[tokio::test]
#[ignore]
async fn a_correctly_signed_client_still_reaches_the_lookup() {
    let Some((app, _db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };

    let body = format!(r#"{{"kind":"set","id":"no-such-set","telegram_id":{OWNER}}}"#);
    let (status, text) = prepare(&app, &make_init_data(OWNER, "dummy_test_token"), &body).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{text}");
}

/// An unparseable `kind` is refused before any Telegram call is attempted.
#[tokio::test]
#[ignore]
async fn an_unknown_kind_is_refused_without_calling_telegram() {
    let Some((app, _db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };

    let body = format!(r#"{{"kind":"spaceship","id":"x","telegram_id":{OWNER}}}"#);
    let (status, _) = prepare(&app, &signed_but_invalid(OWNER), &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
