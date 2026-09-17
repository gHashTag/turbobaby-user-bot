//! An unmatched `/api/...` must 404 — it must not inherit the SPA fallback.
//!
//! This is `#48`. Before the fix, `GET /api/bike-units` on production answered
//! **200 with `index.html`**, because `api::router()` nests `/api` inside an
//! app whose top-level fallback serves the SPA. A path is a string, not a
//! symbol: nothing in the compiler, `dead_code` or clippy can see that a route
//! was never registered, so the miss only ever surfaced at runtime — as a
//! `serde_json::from_str` failing on `<!DOCTYPE html>`, a screen stuck on its
//! error branch, and (the writes being optimistic-first) a green success toast
//! over a row that had never been saved. Nine calls in the fleet admin screen
//! lived that way from the day they were written.
//!
//! What this file pins is deliberately the *composition*, not the inner router
//! in isolation. `api::router()` on its own would 404 an unknown path with or
//! without the fix — axum's default fallback does that much — so a test that
//! asked only the inner router would pass while production still served HTML.
//! That is the vacuous-test shape this repo keeps finding, and it is exactly
//! the shape that would have missed the original defect. The behaviour that
//! actually matters is version-dependent axum semantics: whether an inner
//! router's own fallback survives being nested inside an outer router that
//! later sets its own. So the test builds that sandwich and asks it.
//!
//! The end-to-end proof is a `curl` against the deployment; this is the gate
//! that keeps it true, and that fails loudly if an axum upgrade changes the
//! nesting rule underneath us.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use tower::ServiceExt;

/// Stands in for `serve_dist`: the top-level fallback that serves the SPA.
async fn spa() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html")],
        "<!DOCTYPE html><html><body>app</body></html>",
    )
}

/// Stands in for `api_not_found` in `src/api/mod.rs`.
async fn api_not_found(uri: axum::http::Uri) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        format!(
            "{{\"error\":\"not_found\",\"detail\":\"no API route matches {}\"}}",
            uri.path()
        ),
    )
}

/// The same sandwich `main.rs` builds: an API router nested under `/api`,
/// merged into an app, with the SPA fallback applied last.
fn app() -> Router {
    let api = Router::new()
        .route("/bikes", get(|| async { "{\"bikes\":[]}" }))
        .fallback(api_not_found);

    Router::new()
        .merge(
            Router::new()
                .route("/health", get(|| async { "ok" }))
                .nest("/api", api),
        )
        .route("/catalog", get(spa))
        .fallback(spa)
}

async fn call(path: &str) -> (StatusCode, String) {
    let res = app()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .expect("router should answer");
    let status = res.status();
    let ct = res
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap_or_default().to_string())
        .unwrap_or_default();
    (status, ct)
}

#[tokio::test]
async fn an_unmatched_api_path_is_a_404_and_not_the_spa() {
    let (status, ct) = call("/api/definitely-not-a-route").await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an /api path with no route answered {status} — a caller cannot tell \
         a missing endpoint from an empty one, which is how #48 hid nine of them"
    );
    assert!(
        !ct.contains("text/html"),
        "an /api miss answered content-type {ct:?}; HTML here is the defect \
         itself — the caller parses it as JSON and fails silently"
    );
}

#[tokio::test]
async fn the_old_dead_path_from_48_is_among_them() {
    // The literal path the fleet admin screen used to call, kept as a named
    // case so the regression has the same shape as the incident.
    let (status, ct) = call("/api/bike-units").await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "/api/bike-units answered {status}"
    );
    assert!(!ct.contains("text/html"), "/api/bike-units answered {ct:?}");
}

#[tokio::test]
async fn the_test_is_not_vacuous_the_spa_fallback_is_really_there() {
    // Without this, every assertion above would pass on a router that simply
    // had no fallback at all, and the gate would prove nothing about the
    // arrangement production actually runs.
    let (status, ct) = call("/some/client/route").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the outer SPA fallback is missing from this fixture, so the /api \
         assertions are testing nothing"
    );
    assert!(
        ct.contains("text/html"),
        "the outer fallback did not serve HTML ({ct:?}); the fixture no longer \
         reproduces the composition it is meant to guard"
    );
}

#[tokio::test]
async fn real_api_routes_and_health_still_answer() {
    let (status, _) = call("/api/bikes").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the /api fallback swallowed a real route"
    );

    let (status, _) = call("/health").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "/health is outside /api and must be untouched"
    );

    let (status, ct) = call("/catalog").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "an explicit SPA route must still serve the app"
    );
    assert!(
        ct.contains("text/html"),
        "an SPA route served {ct:?} instead of HTML"
    );
}
