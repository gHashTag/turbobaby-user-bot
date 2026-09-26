pub(crate) mod admin;
// Integration tests in `tests/*.rs` reach in as external consumers
// of the lib crate, so these two must stay `pub mod`. The bin crate
// (src/main.rs has private `mod api;`) still sees them as
// unreachable, hence the targeted allow.
#[allow(unreachable_pub)]
pub mod auth;
#[allow(unreachable_pub)]
pub mod cache;
// The bike catalog: families, units and the two published discount tables.
// It replaces `strains`, which is gone with its migrations-083 tables.
pub(crate) mod bikes;
pub(crate) mod cart;
pub(crate) mod catalog;
pub(crate) mod client_errors;
pub(crate) mod debug;
pub(crate) mod events;
pub(crate) mod game;
pub(crate) mod happy_hour;
pub(crate) mod loyalty;
#[cfg(feature = "utoipa")]
pub(crate) mod openapi;
pub(crate) mod orders;
pub(crate) mod quest;
pub(crate) mod rate_limit;
pub(crate) mod referrals;
pub(crate) mod share;
pub(crate) mod stars;
pub(crate) mod tech_tree;
pub(crate) mod upload;
pub(crate) mod users;

use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::{extract::DefaultBodyLimit, response::Json, routing::get, Router};
use sea_orm::{ConnectionTrait, Statement};
use serde_json::{json, Value};

/// Extract a required boolean field from a JSON body.
pub(crate) fn extract_bool(body: &Value, key: &str) -> Result<bool, StatusCode> {
    body.get(key)
        .and_then(|v| v.as_bool())
        .ok_or(StatusCode::BAD_REQUEST)
}

/// Validates that a URL is either empty/None or starts with an allowed scheme.
/// Allowed: http://, https://, /, data:image/, data:video/
pub(crate) fn validate_url(url: &Option<String>) -> Result<(), StatusCode> {
    if let Some(ref u) = url {
        if u.is_empty() {
            return Ok(());
        }
        if u.len() > 2048 {
            return Err(StatusCode::BAD_REQUEST);
        }
        // Block protocol-relative URLs (//evil.com/...)
        if u.starts_with("//") {
            return Err(StatusCode::BAD_REQUEST);
        }
        let allowed = u.starts_with("http://")
            || u.starts_with("https://")
            || u.starts_with('/')
            || u.starts_with("data:image/")
            || u.starts_with("data:video/");
        if !allowed {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    Ok(())
}

#[allow(unreachable_pub)] // Top-level entry called from main.rs's Axum boot — pub(crate) is the same in practice but loses the "API entry" signal at the call site.
pub fn router(state: crate::AppState) -> Router {
    Router::<AppState>::new()
        .route("/health", get(health_handler))
        .nest("/api", api_routes())
        .with_state(state)
}

fn api_routes() -> Router<AppState> {
    let mut router = Router::<AppState>::new()
        .route("/ping", get(ping_handler))
        .merge(debug::routes())
        .merge(orders::routes())
        .merge(bikes::routes())
        .merge(loyalty::routes())
        .merge(admin::routes())
        .merge(catalog::routes())
        .merge(events::routes())
        .merge(quest::routes())
        .merge(happy_hour::routes())
        .merge(game::routes())
        .merge(client_errors::routes())
        .merge(referrals::routes())
        .merge(stars::routes())
        .merge(tech_tree::routes())
        .merge(users::routes())
        .merge(cart::routes())
        .merge(share::routes());

    // Apply 2MB body limit to all non-upload routes.
    // Upload routes are merged AFTER this layer so their own 110MB limit remains effective.
    router = router.layer(DefaultBodyLimit::max(2 * 1024 * 1024));
    router = router.merge(upload::routes());

    // An unmatched `/api/...` path must 404, and it must say so in JSON.
    //
    // Without this it inherited the *outer* router's SPA fallback and answered
    // **200 with `index.html`**. A path is a string, not a symbol: no compiler,
    // no `dead_code` lint and no clippy pass can see that a route was never
    // registered. So the miss surfaced at runtime as a `serde_json::from_str`
    // failing on `<!DOCTYPE html>`, the screen's error branch rendering for
    // ever, and — because the writes are optimistic-first — a green "✅"
    // toast over a row that had never been saved. That is exactly how nine
    // calls in the fleet admin screen went to endpoints nobody had written.
    //
    // Set last, after every merge: a fallback set before a `merge` loses to
    // the merged router's own.
    router.fallback(api_not_found)
}

/// The 404 body for `/api/*`, in the shape every other API error uses.
///
/// JSON rather than an empty body so that a caller which hits this by mistake
/// reads a sentence instead of guessing — the whole point of the change is
/// that a missing route stops being silent.
async fn api_not_found(uri: axum::http::Uri) -> (StatusCode, Json<Value>) {
    // Built by `missing_route` (end of this file) since 2026-09-26, the one
    // builder of this answer. R2's closed reads (`admin_or_missing_route`)
    // answer a caller without admin proof with it too (owner: «Закрыть для
    // клиентов»), so a closed read and a real miss are one answer, byte for
    // byte, and cannot drift apart. The body keeps its old length, seven
    // lines, so that no line cited below it moves.
    missing_route(uri.path())
}

/// `/api/ping` — dumb liveness probe. Returns 200 as long as the
/// process is alive. Use this for "should the orchestrator restart
/// me?" decisions — restart only on no-response / non-200.
async fn ping_handler() -> Json<Value> {
    Json(json!({"status": "ok", "service": "turbobaby-bot"}))
}

/// `/health` — readiness probe. Returns 200 only when the backend
/// can actually serve traffic (DB reachable). Returns 503 on DB
/// failure so the orchestrator stops routing requests to a container
/// whose DB connection has gone away.
///
/// Cycle #117: prior to this, `/health` was a copy of `/ping` — Railway
/// would mark the container healthy even when the DB was unreachable,
/// silently routing customer traffic into 500s. The `SELECT 1` ping
/// uses the same SeaORM connection pool the app uses, so a pool
/// exhaustion or DB outage surfaces as a 503 here too.
async fn health_handler(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    match state
        .db
        .orm
        .execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT 1".to_string(),
        ))
        .await
    {
        Ok(_) => Ok(Json(
            json!({"status": "ok", "service": "turbobaby-bot", "db": "ok"}),
        )),
        Err(e) => {
            tracing::warn!("/health: DB ping failed: {}", e);
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_bool, validate_url};
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn validate_url_accepts_none() {
        assert!(validate_url(&None).is_ok());
    }

    #[test]
    fn validate_url_accepts_empty() {
        assert!(validate_url(&Some("".to_string())).is_ok());
    }

    #[test]
    fn validate_url_accepts_http() {
        assert!(validate_url(&Some("http://example.com/img.jpg".to_string())).is_ok());
    }

    #[test]
    fn validate_url_accepts_https() {
        assert!(validate_url(&Some("https://example.com/img.jpg".to_string())).is_ok());
    }

    #[test]
    fn validate_url_accepts_relative() {
        assert!(validate_url(&Some("/uploads/abc.jpg".to_string())).is_ok());
    }

    #[test]
    fn validate_url_accepts_data_image() {
        assert!(validate_url(&Some("data:image/png;base64,iVBORw0KGgo=".to_string())).is_ok());
    }

    #[test]
    fn validate_url_accepts_data_video() {
        assert!(validate_url(&Some("data:video/mp4;base64,AAAA".to_string())).is_ok());
    }

    #[test]
    fn validate_url_rejects_javascript() {
        assert_eq!(
            validate_url(&Some("javascript:alert(1)".to_string())).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_url_rejects_data_text_html() {
        assert_eq!(
            validate_url(&Some(
                "data:text/html,<script>alert(1)</script>".to_string()
            ))
            .unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_url_rejects_ftp() {
        assert_eq!(
            validate_url(&Some("ftp://evil.com/file.jpg".to_string())).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_url_rejects_protocol_relative() {
        assert_eq!(
            validate_url(&Some("//evil.com/img.jpg".to_string())).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_url_rejects_too_long() {
        let long = "https://x.com/".to_string() + &"a".repeat(3000);
        assert_eq!(
            validate_url(&Some(long)).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_bool_true() {
        assert!(extract_bool(&json!({"k": true}), "k").unwrap());
    }

    #[test]
    fn test_extract_bool_false() {
        assert!(!extract_bool(&json!({"k": false}), "k").unwrap());
    }

    #[test]
    fn test_extract_bool_missing() {
        assert_eq!(
            extract_bool(&json!({}), "k").unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_bool_not_bool() {
        assert_eq!(
            extract_bool(&json!({"k": "true"}), "k").unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }
}

/// Cycle #143: defensive test catching the "added a routes file but
/// forgot to wire it" bug class.
///
/// Cycle #143 audit discovered a 244-line file in `src/api/` that
/// defined `pub fn routes()` for high-score persistence but was
/// orphaned since cycle #91 — `pub mod` was missing from this file,
/// so it never compiled. The orphan-table defense in
/// `src/db/mod.rs::orphan_table_tests` missed the companion table
/// because its name still appeared as a string literal in the
/// uncompiled file, fooling the textual reference check. Cycle #144
/// deleted the file and migrated the table name to ALLOWED_ORPHANS.
///
/// This test walks `src/api/*.rs`, finds every file with
/// `pub fn routes(`, and asserts each is both `pub mod`'d and
/// `.merge()`'d in this file (`src/api/mod.rs`). Modules pending a
/// wire-or-delete decision go in `ALLOWED_UNWIRED_ROUTES` with
/// rationale.
#[cfg(test)]
mod route_wiring_tests {
    /// Modules with `pub fn routes()` deferred from production wiring.
    /// Each entry must name who deferred it and why.
    ///
    /// Cycle #144 emptied this list — the prior entry was deleted
    /// along with its file, and its companion table was migrated to
    /// `src/db/mod.rs::ALLOWED_ORPHANS`. New entries should be rare
    /// and short-lived; prefer wiring or deleting over allowlisting.
    const ALLOWED_UNWIRED_ROUTES: &[&str] = &[];

    fn module_name_from_path(path: &std::path::Path) -> Option<String> {
        let stem = path.file_stem()?.to_str()?;
        if stem == "mod" {
            return None;
        }
        Some(stem.to_string())
    }

    /// Strip `//` line comments so a sample like `.merge(game::routes())`
    /// inside the allowlist rationale doesn't fool the textual contains
    /// check. (Block `/* … */` comments aren't used in this file; if
    /// that changes, broaden this.)
    fn strip_line_comments(src: &str) -> String {
        let mut out = String::with_capacity(src.len());
        for line in src.lines() {
            let cut = match line.find("//") {
                Some(i) => &line[..i],
                None => line,
            };
            out.push_str(cut);
            out.push('\n');
        }
        out
    }

    fn modules_with_pub_fn_routes() -> Vec<String> {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let api_dir = std::path::Path::new(manifest).join("src/api");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&api_dir)
            .expect("src/api readable")
            .flatten()
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            let Some(module) = module_name_from_path(&path) else {
                continue;
            };
            let Ok(src) = std::fs::read_to_string(&path) else {
                continue;
            };
            // Match any visibility on the declaration, not just bare `pub`.
            //
            // This test was added on 2026-06-02 looking for `pub fn routes(`.
            // On 2026-06-06, `baf77cc` ("tighten 59 pub items to pub(crate)")
            // rewrote every one of them to `pub(crate) fn routes(`, and the
            // needle stopped matching anything at all. The list went empty,
            // both assertions below were then over empty vectors, and the gate
            // reported success for three months while checking nothing — the
            // exact bug class it was written to catch, now in the catcher.
            // Hence `routes_fns_are_found` below: a gate whose input can go to
            // zero silently is not a gate.
            let has_routes = src.lines().any(|l| {
                let l = l.trim_start();
                l.starts_with("fn routes(")
                    || l.starts_with("pub fn routes(")
                    || (l.starts_with("pub(") && l.contains(") fn routes("))
            });
            if has_routes {
                out.push(module);
            }
        }
        out
    }

    /// The guard the original test lacked.
    ///
    /// `modules_with_pub_fn_routes` locates its subjects by matching source
    /// text, so any change to how a `routes()` function is declared can empty
    /// it — and an empty subject list makes
    /// `every_routes_fn_is_declared_and_merged` pass unconditionally. Pinning a
    /// floor means the next such refactor fails here, loudly, instead of
    /// quietly switching the wiring check off.
    ///
    /// The floor is deliberately well under the real count (19 at the time of
    /// writing) so that deleting a module is not a test failure; only losing
    /// the ability to see modules at all is.
    #[test]
    fn routes_fns_are_found() {
        let found = modules_with_pub_fn_routes();
        assert!(
            found.len() >= 10,
            "only {} src/api/*.rs modules were recognised as declaring `routes()`: {:?}\n\
             The recogniser in `modules_with_pub_fn_routes` has gone blind — fix its \
             pattern rather than this floor. Every module merged in `api_routes()` \
             must be visible to it, or the wiring check silently covers nothing.",
            found.len(),
            found,
        );
    }

    #[test]
    fn every_routes_fn_is_declared_and_merged() {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let mod_path = std::path::Path::new(manifest).join("src/api/mod.rs");
        let raw = std::fs::read_to_string(&mod_path).expect("read src/api/mod.rs");
        let mod_src = strip_line_comments(&raw);

        let mut missing_pub_mod = Vec::new();
        let mut missing_merge = Vec::new();

        for module in modules_with_pub_fn_routes() {
            if ALLOWED_UNWIRED_ROUTES.contains(&module.as_str()) {
                continue;
            }
            // Match the declaration at any visibility.
            //
            // This needle carried the same defect as the `routes()` one above,
            // and it survived the first repair: unblinding `routes()` handed
            // this assertion 19 subjects for the first time since 2026-06-06,
            // and it then failed all 19 — because `baf77cc` had rewritten
            // `pub mod orders;` to `pub(crate) mod orders;` in the same sweep.
            // The gate was blind twice over, and fixing only the half that
            // produced the subject list turned three months of false green into
            // 19 false red. Both needles have to know about visibility, so the
            // declaration is recognised the same way in both places.
            let declares_module = |vis_prefix: &str| {
                mod_src
                    .lines()
                    .map(str::trim_start)
                    .any(|l| l.starts_with(&format!("{vis_prefix}mod {module};")))
            };
            let has_pub_mod = declares_module("")
                || declares_module("pub ")
                || mod_src
                    .lines()
                    .map(str::trim_start)
                    .any(|l| l.starts_with("pub(") && l.contains(&format!(") mod {module};")));
            let merge_needle = format!(".merge({}::routes())", module);
            if !has_pub_mod {
                missing_pub_mod.push(module.clone());
            }
            if !mod_src.contains(&merge_needle) {
                missing_merge.push(module);
            }
        }

        assert!(
            missing_pub_mod.is_empty(),
            "src/api/*.rs files declare `routes()` but have no `mod` declaration in src/api/mod.rs: {:?}\n\
             Either add `pub(crate) mod <name>;` to src/api/mod.rs or add to ALLOWED_UNWIRED_ROUTES with rationale.",
            missing_pub_mod,
        );

        assert!(
            missing_merge.is_empty(),
            "src/api/*.rs files have `pub fn routes()` but are not `.merge()`'d into api_routes(): {:?}\n\
             Either add `.merge(<name>::routes())` in api_routes() or add to ALLOWED_UNWIRED_ROUTES with rationale.",
            missing_merge,
        );
    }

    #[test]
    fn allowlist_does_not_contain_wired_modules() {
        // Stale allowlist entries shouldn't accumulate. If a module is
        // wired into api_routes(), removing it from the allowlist
        // should be part of the same change.
        let manifest = env!("CARGO_MANIFEST_DIR");
        let mod_path = std::path::Path::new(manifest).join("src/api/mod.rs");
        let raw = std::fs::read_to_string(&mod_path).expect("read src/api/mod.rs");
        let mod_src = strip_line_comments(&raw);

        let stale: Vec<&&str> = ALLOWED_UNWIRED_ROUTES
            .iter()
            .filter(|name| {
                let merge_needle = format!(".merge({}::routes())", name);
                mod_src.contains(&merge_needle)
            })
            .collect();

        assert!(
            stale.is_empty(),
            "ALLOWED_UNWIRED_ROUTES contains modules that are now merged in api_routes(): {:?}\n\
             Remove these entries from the allowlist.",
            stale,
        );
    }
}

// The closed reads (R2), the owner's answer of 2026-09-26. Appended so that no
// line cited above moves.

use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};

/// The missing-route answer for `path`: `404` and
/// `{"error":"not_found","detail":"no API route matches <path>"}`. `path` is
/// the path as the nested `/api` router sees it (prefix stripped), which is
/// what `api_not_found` and the closed reads both hand it.
pub(crate) fn missing_route(path: &str) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "detail": format!("no API route matches {}", path),
        })),
    )
}

/// R2 (owner, 2026-09-26, verbatim): «Закрыть для клиентов». A read no
/// mounted screen uses answers a caller without admin proof exactly as an
/// unmatched path does: ANY `check_admin` refusal (401, or 429 once the admin
/// limiter trips) becomes [`missing_route`] for the request's own path. The
/// route stays registered, so nobody gets a 405, and a failed attempt still
/// counts toward the admin limiter, so the route is no password oracle. An
/// admin's id comes back as `check_admin` returns it (0 for the password
/// token).
// The `Err` IS the answer the handler returns, once per request, so boxing it
// would buy nothing but a `*` at the three call sites.
#[allow(clippy::result_large_err)]
pub(crate) fn admin_or_missing_route(
    headers: &HeaderMap,
    state: &AppState,
    uri: &axum::http::Uri,
) -> Result<i64, Response> {
    crate::api::auth::check_admin(headers, state)
        .map_err(|_| missing_route(uri.path()).into_response())
}

#[cfg(test)]
mod missing_route_tests {
    use super::{api_not_found, missing_route};
    use axum::http::StatusCode;
    use serde_json::json;

    /// R2's closed reads and the fallback answer with one builder: for any
    /// path, `missing_route` is exactly what `api_not_found` answers.
    #[tokio::test]
    async fn missing_route_is_the_fallbacks_own_answer() {
        for path in [
            "/quest-places",
            "/treasure-hunts",
            "/loyalty/config",
            "/no/such/route",
        ] {
            let uri: axum::http::Uri = path.parse().expect("a path");
            let (status, body) = api_not_found(uri).await;
            let (built_status, built_body) = missing_route(path);
            assert_eq!(status, StatusCode::NOT_FOUND);
            assert_eq!(built_status, status);
            assert_eq!(built_body.0, body.0);
            assert_eq!(
                body.0,
                json!({"error": "not_found", "detail": format!("no API route matches {path}")})
            );
        }
    }
}
