//! Every URL the app fetches must be a URL the server answers.
//!
//! This gate exists because of a defect that shipped to production and stayed
//! there: the home screen fetched `/api/strains/strain-of-day`, a route
//! migration 083 retired along with the cannabis catalog. Nothing failed
//! loudly. Axum has no such route, so the request fell through to the SPA
//! fallback and came back **200 OK with `content-type: text/html`** — the
//! index page. The screen's `serde_json` parse failed, the `Some(Err(_))`
//! branch ran, and every visitor saw «⭐ Сорт дня» over a loading skeleton
//! that would never resolve. A 404 would have been caught in a day.
//!
//! `tests/retired_table_wiring.rs` guards the same defect one layer down —
//! code that outlived its *table*. It could not catch this one, because the
//! strain table had gone and so had its entity; what survived was a *string*.
//! A path is not a symbol, so no compiler and no dead-code lint will ever see
//! it. The only instrument that can is a test that reads both sides as text.
//!
//! Both sides are parsed from source rather than asserted against a list, so a
//! route added tomorrow is covered without anyone remembering to extend
//! anything. And because a "no orphans" check over an empty corpus passes
//! perfectly, both parsers are pinned by their own assertions first.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn rust_files(dir: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::from(dir)];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap_or_else(|e| panic!("read_dir {d:?}: {e}")) {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// The string literal opened by the first `"` at or after `from`.
///
/// Used for both sides. `.route(` is frequently wrapped onto the next line by
/// rustfmt when the handler list is long, so a line-oriented parser would miss
/// exactly the routes with the most handlers on them.
fn literal_after(hay: &str, from: usize) -> Option<&str> {
    let open = from + hay[from..].find('"')?;
    let end = open + 1 + hay[open + 1..].find('"')?;
    Some(&hay[open + 1..end])
}

// ---------------------------------------------------------------- server side

/// Paths the axum router answers, as full paths from the root.
///
/// `src/api/mod.rs` mounts everything under `.nest("/api", …)` with the single
/// exception of `/health`, which is registered on the outer router. That one
/// exception is spelled out rather than inferred: a parser that guessed would
/// silently mis-prefix every route the day someone adds a second nest.
fn server_routes() -> BTreeSet<String> {
    let mut routes = BTreeSet::new();
    for file in rust_files("src/api") {
        let src = read(&file);
        let unnested = file.ends_with("mod.rs");
        let mut at = 0;
        while let Some(idx) = src[at..].find(".route(") {
            let pos = at + idx;
            at = pos + ".route(".len();
            let Some(path) = literal_after(&src, at) else {
                continue;
            };
            if !path.starts_with('/') {
                continue;
            }
            if unnested && path == "/health" {
                routes.insert(path.to_string());
            } else {
                routes.insert(format!("/api{path}"));
            }
        }
    }
    routes
}

#[test]
fn the_router_parser_actually_sees_the_api() {
    let routes = server_routes();
    assert!(
        routes.len() >= 80,
        "only {} routes parsed out of src/api — the parser has stopped matching \
         the router's shape, and the orphan check below would now condemn every \
         call site in the app: {routes:?}",
        routes.len()
    );
    // One route from a handful of independent modules, so a file that stops
    // parsing cannot hide behind the others keeping the count up.
    for expected in [
        "/api/bikes/:key",
        "/api/orders",
        "/api/loyalty/tiers",
        "/api/quest-places",
        "/api/admin/managers",
        "/health",
    ] {
        assert!(
            routes.contains(expected),
            "{expected} is not in the parsed router"
        );
    }
}

// -------------------------------------------------------------- client side

/// A URL the UI builds, with its source location.
#[derive(Debug)]
struct CallSite {
    file: String,
    line: usize,
    path: String,
}

/// Every string literal in `src/ui` that contains `/api/`, reduced to the path.
///
/// The literals come in three shapes — a bare `"/api/sets"`, a `format!`
/// template `"{}/api/orders/user/{}"`, and an inline-captured
/// `"{api_base_url()}/api/orders/{id}/promptpay-qr"`. All three are handled the
/// same way: cut everything before `/api/`, cut the query string, and treat any
/// segment containing `{` as a parameter.
fn ui_call_sites() -> Vec<CallSite> {
    let mut sites = Vec::new();
    for file in rust_files("src/ui") {
        let src = read(&file);
        for (n, line) in src.lines().enumerate() {
            let mut rest = line;
            while let Some(q) = rest.find('"') {
                let after = &rest[q + 1..];
                let Some(close) = after.find('"') else { break };
                let lit = &after[..close];
                rest = &after[close + 1..];
                let Some(start) = lit.find("/api/") else {
                    continue;
                };
                let path = lit[start..].split(['?', '#']).next().unwrap_or_default();
                sites.push(CallSite {
                    file: file.display().to_string(),
                    line: n + 1,
                    path: path.trim_end_matches('/').to_string(),
                });
            }
        }
    }
    sites
}

#[test]
fn the_call_site_parser_actually_sees_the_screens() {
    let sites = ui_call_sites();
    assert!(
        sites.len() >= 100,
        "only {} /api/ call sites found in src/ui — src/ui is never compiled by \
         `cargo test` (it is `#[cfg(target_arch = \"wasm32\")]`), so this text \
         scan is the only thing looking at it at all",
        sites.len()
    );
    assert!(
        sites.iter().any(|s| s.path == "/api/bikes"),
        "the catalog's own fetch is missing from the scan"
    );
}

/// Does a UI path match a registered route?
///
/// Segment-wise, same length. A server segment beginning `:` is a path
/// parameter and matches anything; a UI segment containing `{` is a format
/// placeholder and likewise matches anything. Being lenient in both directions
/// is deliberate — this gate is here to catch paths with *no route at all*,
/// which is the defect that shipped, and a gate that also argued about
/// parameter names would produce noise that gets it switched off.
fn matches(ui: &str, route: &str) -> bool {
    let (u, r): (Vec<&str>, Vec<&str>) = (ui.split('/').collect(), route.split('/').collect());
    u.len() == r.len()
        && u.iter()
            .zip(&r)
            .all(|(a, b)| a == b || b.starts_with(':') || a.contains('{'))
}

#[test]
fn no_screen_fetches_a_route_the_server_does_not_serve() {
    let routes = server_routes();
    let mut orphans: Vec<String> = ui_call_sites()
        .into_iter()
        .filter(|s| !routes.iter().any(|r| matches(&s.path, r)))
        .map(|s| format!("  {}:{} fetches {}", s.file, s.line, s.path))
        .collect();
    orphans.sort();
    orphans.dedup();

    assert!(
        orphans.is_empty(),
        "{} call site(s) fetch a path no axum route answers:\n{}\n\n\
         These do not 404. Unmatched paths fall through to the SPA fallback and \
         return 200 with an HTML body, so the screen's JSON parse fails and its \
         error branch renders — a permanent, silent, user-visible defect. Either \
         register the route or delete the call site.",
        orphans.len(),
        orphans.join("\n")
    );
}
