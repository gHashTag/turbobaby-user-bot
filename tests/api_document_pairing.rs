//! The router and the published document, read as text and compared.
//!
//! `specs/turbobaby/api_surface.t27` owns the relation between the two sides
//! and, before this file existed, nothing in `tests/` held it: a grep of
//! `tests/` for `openapi` or `ApiDoc` on 2026-09-21 returned zero files. The
//! one gate that does read the router — the wiring gate at
//! `src/api/mod.rs:304-494` — takes the MODULE as its subject and never a
//! path, so a module can be merged, pass that gate, and carry a path the
//! document invented or forgot with both readings green.
//!
//! Three separate failures live in that gap, and each one has already been
//! paid for once in this repository:
//!
//!   1. A path that is not served where it reads as being served.
//!      `src/api/quest.rs:16` registered `/api/test` inside a router that
//!      `src/api/mod.rs:77` nests under `/api`. The served path was
//!      `/api/api/test`; a request to `/api/test` reached the miss handler at
//!      `src/api/mod.rs:128`. Nothing in the compiler can see a doubled
//!      prefix, because a path is a string and not a symbol — the same reason
//!      issue #48's nine dead endpoints were invisible.
//!   2. A documented path with no route behind it. The document is a promise
//!      to a client that cannot read the router, so a phantom path is a
//!      published lie. This direction is clean today (0 phantoms) and there
//!      was nothing keeping it that way.
//!   3. The gap widening in silence. 94 of the 103 routed `/api` paths were
//!      undocumented on 2026-09-21 and no reader was told. This file does not
//!      demand the 91 remaining be written up today: it writes the debt down
//!      per module and lets the numbers fall only, in the shape
//!      `tests/customer_surface_wiring.rs` uses for `UNCOMPILED_TEST_DEBT`.
//!
//! Both sides are parsed from source rather than asserted against a list, so
//! a route added tomorrow is covered without anyone remembering to extend
//! anything — and because a "no orphans" check over an empty corpus passes
//! perfectly, both parsers are pinned by their own floors first (D16).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Paths a module registers on purpose and the document does not name, with
/// the reason recorded in the tree rather than asserted here.
///
/// `specs/turbobaby/api_surface.t27` warned about exactly this list before it
/// existed: "inventing a default of exempt would turn the whole gap green".
/// So an entry earns its place only when the source itself says why the path
/// is not part of the published client surface, and the reason names the
/// file and line that says it. Everything else is debt, below, not exemption.
///
/// Both entries are the two cases that contract names as legitimate — a
/// liveness probe and a debug hook.
const DOCUMENTATION_EXEMPT: [(&str, &str); 2] = [
    (
        "/api/ping",
        "src/api/mod.rs:138-140 — the orchestrator's liveness probe, used for \
         'should I restart this container?'; it is operational plumbing and \
         not a client surface",
    ),
    (
        "/api/debug/validate-init-data",
        "src/api/debug.rs:1-6 — exists ONLY to diagnose Mini App auth \
         problems in production; publishing a diagnostic hook in the client \
         document advertises it as something to build against",
    ),
];

/// Routed `/api` paths that no `#[utoipa::path]` stub names, per module.
///
/// Per module and not one total, for the reason the doc comment of
/// `UNCOMPILED_TEST_DEBT` in `tests/customer_surface_wiring.rs` gives about its own ratchet: a
/// path documented in one module and a new one added in another would net out
/// to the same number and this gate would wave both through. It is still a
/// coarse instrument — a module that documents one path and adds another in
/// the same change nets out inside its own row — and the phantom check below
/// is the only thing that reads individual paths. Said plainly here so nobody
/// reads a green run as "every path in that module is accounted for".
///
/// These may only go down. When a module reaches zero, delete its row; the
/// accuracy check below will not let the list quietly become vacuous.
///
/// Measured 2026-09-21 after `/api/test` was removed from `src/api/quest.rs`:
/// 103 routed `/api` paths, 9 documented, 2 exempt above, 91 on this list.
const UNDOCUMENTED_DEBT: [(&str, usize); 17] = [
    ("admin.rs", 15),
    ("catalog.rs", 14),
    ("bikes.rs", 11),
    ("events.rs", 11),
    ("orders.rs", 7),
    ("quest.rs", 5),
    ("referrals.rs", 5),
    ("client_errors.rs", 4),
    ("stars.rs", 4),
    ("tech_tree.rs", 4),
    ("cart.rs", 3),
    ("loyalty.rs", 3),
    ("game.rs", 1),
    ("happy_hour.rs", 1),
    ("share.rs", 1),
    ("upload.rs", 1),
    ("users.rs", 1),
];

/// The verbs a `.route(...)` argument can carry, as axum spells them.
const VERBS: [&str; 7] = ["get", "post", "put", "delete", "patch", "head", "options"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(relative: &str) -> String {
    let path = repo_root().join(relative);
    // Normalised to LF for the same reason
    // `tests/customer_surface_wiring.rs:76-79` gives: a Windows checkout
    // (`core.autocrlf`) hands a matcher CRLF and it silently matches nothing.
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{relative} must exist: {e}"))
        .replace("\r\n", "\n")
}

/// Every `src/api/*.rs` file, by file name, in a stable order.
fn api_sources() -> Vec<(String, String)> {
    let dir = repo_root().join("src/api");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .expect("src/api is readable")
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("file name")
            .to_string();
        out.push((name.clone(), read(&format!("src/api/{name}"))));
    }
    out.sort();
    out
}

/// Cut a source at its first `#[cfg(test)]`.
///
/// `src/api/observability.rs` registers a route inside its own test module,
/// and counting it would put a path in the census that no client can reach.
fn production_part(source: &str) -> &str {
    match source.find("#[cfg(test)]") {
        Some(at) => &source[..at],
        None => source,
    }
}

/// Drop `//` comments, but only where a `//` is not inside a string literal.
///
/// Not decoration: the first run of this file reported
/// `src/api/quest.rs registers "/api/test"` after that route had been deleted,
/// because the note left in its place QUOTES the registration it describes —
/// and a note saying what was removed and why is exactly what this tree writes
/// where something used to be (`tests/customer_surface_wiring.rs:84-87` strips
/// comments for the same reason). A naive `find("//")` would cut
/// `"https://…"` in half and leave an unbalanced quote for the literal scanner
/// to pair with a later line, so the string state is tracked instead.
fn code_only(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        let bytes = line.as_bytes();
        let mut in_string = false;
        let mut cut = line.len();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' if in_string => i += 1,
                b'"' => in_string = !in_string,
                b'/' if !in_string && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                    cut = i;
                    break;
                }
                _ => {}
            }
            i += 1;
        }
        out.push_str(&line[..cut]);
        out.push('\n');
    }
    out
}

/// The string literal opened by the first `"` at or after `from`, and where it ended.
///
/// Byte offsets rather than lines, because rustfmt wraps any `.route(` call
/// whose handler list is long: a line-oriented parser misses systematically
/// the paths with the most verbs on them. `api_surface.t27:44-49` measures
/// that error at 24 registrations and 24 distinct paths.
fn literal_after(hay: &str, from: usize) -> Option<(&str, usize)> {
    let open = from + hay[from..].find('"')?;
    let end = open + 1 + hay[open + 1..].find('"')?;
    Some((&hay[open + 1..end], end))
}

/// The index of the `)` closing the `(` at `open`.
fn call_end(src: &str, open: usize) -> usize {
    let mut depth = 0usize;
    for (offset, ch) in src[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return open + offset;
                }
            }
            _ => {}
        }
    }
    src.len()
}

/// One `.route("…", …)` registration: which module wrote it, the literal as
/// written, and the verbs its handler list carries.
struct Registration {
    module: String,
    literal: String,
    verbs: BTreeSet<String>,
}

impl Registration {
    /// The path a client types, as the router assembles it.
    ///
    /// `src/api/mod.rs:77` mounts everything under `.nest("/api", …)` with the
    /// single exception of `/health`, registered on the outer router at
    /// `src/api/mod.rs:76`. That one exception is spelled out rather than
    /// inferred, exactly as `tests/ui_endpoints_exist.rs:62-65` spells it: a
    /// parser that guessed would mis-prefix every route the day someone adds a
    /// second nest.
    fn served_path(&self) -> String {
        if self.module == "mod.rs" && self.literal == "/health" {
            return self.literal.clone();
        }
        format!("/api{}", self.literal)
    }
}

fn registrations() -> Vec<Registration> {
    let mut out = Vec::new();
    for (module, source) in api_sources() {
        let src = code_only(production_part(&source));
        let src = src.as_str();
        let mut at = 0;
        while let Some(idx) = src[at..].find(".route(") {
            let open = at + idx + ".route".len();
            let end = call_end(src, open);
            at = open + 1;
            let Some((literal, literal_end)) = literal_after(src, open) else {
                continue;
            };
            if !literal.starts_with('/') || literal_end > end {
                continue;
            }
            let handlers = &src[literal_end..end];
            let mut verbs = BTreeSet::new();
            for verb in VERBS {
                // `get(` as a whole word: `get(` matches, `target(` does not.
                let mut from = 0;
                while let Some(hit) = handlers[from..].find(&format!("{verb}(")) {
                    let pos = from + hit;
                    let boundary = pos == 0
                        || !handlers[..pos]
                            .chars()
                            .next_back()
                            .is_some_and(|c| c.is_alphanumeric() || c == '_');
                    if boundary {
                        verbs.insert(verb.to_string());
                    }
                    from = pos + verb.len();
                }
            }
            out.push(Registration {
                module: module.clone(),
                literal: literal.to_string(),
                verbs,
            });
        }
    }
    out
}

/// `src/api/openapi.rs` with its comments dropped, so a stub that has been
/// commented out documents nothing here either.
fn document_source() -> String {
    code_only(&read("src/api/openapi.rs"))
}

/// A documented verb and path, as `#[utoipa::path(...)]` declares it.
fn documented_pairs() -> BTreeSet<(String, String)> {
    let doc = document_source();
    let mut out = BTreeSet::new();
    let mut at = 0;
    while let Some(idx) = doc[at..].find("#[utoipa::path(") {
        let open = at + idx + "#[utoipa::path".len();
        let end = call_end(&doc, open);
        at = open + 1;
        let attribute = &doc[open..end];
        let verb = attribute
            .trim_start_matches('(')
            .trim_start()
            .split([',', '\n'])
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        let Some(path_at) = attribute.find("path =") else {
            continue;
        };
        let Some((path, _)) = literal_after(attribute, path_at) else {
            continue;
        };
        out.insert((verb, path.to_string()));
    }
    out
}

/// Path parameters carry different spellings on the two sides — `:telegram_id`
/// in axum, `{telegram_id}` in OpenAPI — and the NAME is not the subject here.
/// Being lenient about it is deliberate, for the reason
/// `tests/ui_endpoints_exist.rs:181-186` gives: a gate that also argued about
/// parameter names would produce noise that gets it switched off.
fn normalise(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if segment.starts_with(':') || (segment.starts_with('{') && segment.ends_with('}')) {
                "*"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn routed_api_paths() -> BTreeSet<String> {
    registrations()
        .iter()
        .map(|r| r.served_path())
        .filter(|p| p.starts_with("/api"))
        .collect()
}

// ─────────────────────────────────────────── the parsers must see something

#[test]
fn the_route_parser_sees_the_router() {
    let regs = registrations();
    assert!(
        regs.len() >= 100,
        "only {} `.route(` registrations parsed out of src/api — the parser has \
         stopped matching the router's shape, and every check below would now be \
         measuring an empty surface (D16, DECISIONS.md:235-262)",
        regs.len()
    );
    let paths = routed_api_paths();
    assert!(
        paths.len() >= 80,
        "only {} distinct /api paths parsed: {paths:?}",
        paths.len()
    );
    // One path from a handful of independent modules, so a file that stops
    // parsing cannot hide behind the others keeping the count up.
    for expected in [
        "/api/bikes/:key",
        "/api/orders",
        "/api/loyalty/tiers",
        "/api/quest-places",
        "/api/admin/managers",
        "/api/ping",
    ] {
        assert!(
            paths.contains(expected),
            "{expected} is not in the parsed router"
        );
    }
}

#[test]
fn the_document_parser_sees_the_document() {
    let pairs = documented_pairs();
    assert!(
        pairs.len() >= 5,
        "only {} documented verb-and-path pairs parsed out of src/api/openapi.rs — \
         a document parser that finds nothing makes the phantom check below pass \
         over an empty set, which is the shape of a gate that has switched itself \
         off: {pairs:?}",
        pairs.len()
    );
    assert!(
        pairs.contains(&("get".to_string(), "/api/loyalty/tiers".to_string())),
        "a known stub is missing from the parse: {pairs:?}"
    );
    // Every stub must also be LISTED in `paths(...)`, or it documents nothing:
    // utoipa builds the served document from that list, not from the file.
    let doc = document_source();
    let listed = doc
        .split_once("paths(")
        .map(|(_, rest)| rest.split_once(')').map(|(inner, _)| inner.to_string()))
        .unwrap_or_default()
        .unwrap_or_default();
    let stubs = doc.matches("#[utoipa::path(").count();
    let listed_count = listed
        .lines()
        .filter(|l| !l.trim().trim_end_matches(',').is_empty())
        .count();
    assert_eq!(
        stubs, listed_count,
        "src/api/openapi.rs has {stubs} #[utoipa::path] stub(s) but lists {listed_count} \
         in paths(...) — a stub that is not listed is invisible to every client, and \
         reading the file rather than the list is how a phantom check goes blind"
    );
}

// ────────────────────────────────── a module must not respell the nest prefix

#[test]
fn no_module_route_repeats_the_nest_prefix() {
    // `src/api/mod.rs` is exempt as the composer: it owns the `.nest("/api", …)`
    // at :77 and the outer router's own `/health` at :76, so a literal there is
    // read against a different parent. Every other file in `src/api` is merged
    // INTO the nest, and the prefix is already supplied.
    let offenders: Vec<String> = registrations()
        .iter()
        .filter(|r| r.module != "mod.rs" && r.literal.starts_with("/api"))
        .map(|r| format!("  src/api/{} registers {:?}", r.module, r.literal))
        .collect();

    assert!(
        offenders.is_empty(),
        "a module's routes() spells the nest prefix that src/api/mod.rs:77 already \
         supplies:\n{}\n\n\
         The served path doubles it — {:?} is served at \"/api/api/…\" and a request \
         to the path as written reaches the miss handler at src/api/mod.rs:128. \
         Nothing in the compiler can see this: a path is a string, not a symbol, and \
         both spellings type-check. Drop the prefix from the literal.",
        offenders.join("\n"),
        registrations()
            .iter()
            .find(|r| r.module != "mod.rs" && r.literal.starts_with("/api"))
            .map(|r| r.literal.clone())
            .unwrap_or_default(),
    );
}

// ─────────────────────────────── a documented path must have a route behind it

#[test]
fn no_documented_path_is_a_phantom() {
    let routed: BTreeSet<String> = routed_api_paths().iter().map(|p| normalise(p)).collect();
    let routed_pairs: BTreeSet<(String, String)> = registrations()
        .iter()
        .filter(|r| r.served_path().starts_with("/api"))
        .flat_map(|r| {
            let path = normalise(&r.served_path());
            r.verbs
                .iter()
                .map(move |v| (v.clone(), path.clone()))
                .collect::<Vec<_>>()
        })
        .collect();

    let mut phantoms = Vec::new();
    for (verb, path) in documented_pairs() {
        let normalised = normalise(&path);
        if !routed.contains(&normalised) {
            phantoms.push(format!("  {verb} {path} — no route answers this path"));
        } else if !routed_pairs.contains(&(verb.clone(), normalised)) {
            phantoms.push(format!(
                "  {verb} {path} — the path is routed, but not for {verb}"
            ));
        }
    }

    assert!(
        phantoms.is_empty(),
        "src/api/openapi.rs documents {} path(s) the router does not serve:\n{}\n\n\
         The document is a promise made to a client that cannot read the router. \
         An exemption can excuse a routed path nobody wrote up; no reason makes a \
         promise about a path that does not exist true \
         (specs/turbobaby/api_surface.t27, path_is_consistent).",
        phantoms.len(),
        phantoms.join("\n"),
    );
}

// ──────────────────────────────────────────────── the gap may shrink, not grow

/// Undocumented, non-exempt `/api` paths, by the module that registered them.
fn undocumented_by_module() -> BTreeMap<String, BTreeSet<String>> {
    let documented: BTreeSet<String> = documented_pairs()
        .iter()
        .map(|(_, path)| normalise(path))
        .collect();
    let exempt: BTreeSet<String> = DOCUMENTATION_EXEMPT
        .iter()
        .map(|(path, _)| normalise(path))
        .collect();

    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for registration in registrations() {
        let served = registration.served_path();
        if !served.starts_with("/api") {
            continue;
        }
        let normalised = normalise(&served);
        if documented.contains(&normalised) || exempt.contains(&normalised) {
            continue;
        }
        out.entry(registration.module).or_default().insert(served);
    }
    out
}

#[test]
fn no_module_grows_an_undocumented_path() {
    let live = undocumented_by_module();
    let mut offences = Vec::new();

    for (module, paths) in &live {
        let allowed = UNDOCUMENTED_DEBT
            .iter()
            .find(|(name, _)| name == module)
            .map(|(_, count)| *count)
            .unwrap_or(0);
        if paths.len() > allowed {
            let mut listed: Vec<&String> = paths.iter().collect();
            listed.sort();
            offences.push(format!(
                "  src/api/{module}: {} undocumented /api path(s), {allowed} on record — \
                 {} more than the gate was told about: {listed:?}",
                paths.len(),
                paths.len() - allowed
            ));
        }
    }

    assert!(
        offences.is_empty(),
        "the undocumented half of the API surface grew:\n{}\n\n\
         src/api/openapi.rs is what a client reads instead of the router, and on \
         2026-09-21 it named 9 of 103 routed /api paths. This gate does not ask for \
         the other 91 today; it asks that the number stop climbing in silence. \
         Either add a #[utoipa::path] stub and list it in paths(...), or — if the \
         path is deliberately not a client surface — put it in DOCUMENTATION_EXEMPT \
         with the file and line that says why. Raising a number in UNDOCUMENTED_DEBT \
         is the last resort and needs a sentence saying what it bought.",
        offences.join("\n"),
    );
}

#[test]
fn the_undocumented_debt_is_written_down_accurately() {
    let live = undocumented_by_module();
    let mut wrong = Vec::new();

    for (module, recorded) in UNDOCUMENTED_DEBT {
        let count = live.get(module).map(|p| p.len()).unwrap_or(0);
        if count < recorded {
            wrong.push(format!(
                "  src/api/{module}: {recorded} on record, {count} in the tree — \
                 {} were documented, exempted or deleted; lower the number so the \
                 ratchet holds at the new floor",
                recorded - count
            ));
        }
    }

    assert!(
        wrong.is_empty(),
        "UNDOCUMENTED_DEBT no longer describes the tree:\n{}\n\n\
         A ratchet that is not tightened after a win is a ratchet that has given the \
         ground back — the freed slots would silently authorise new undocumented \
         paths (the assertion in tests/customer_surface_wiring.rs's \
         the_uncompiled_test_debt_is_written_down_accurately makes the same argument \
         about its own).",
        wrong.join("\n"),
    );
}

#[test]
fn the_exemption_register_holds_only_live_reasons() {
    let routed: BTreeSet<String> = routed_api_paths().iter().map(|p| normalise(p)).collect();
    let documented: BTreeSet<String> = documented_pairs()
        .iter()
        .map(|(_, path)| normalise(path))
        .collect();

    let mut stale = Vec::new();
    for (path, reason) in DOCUMENTATION_EXEMPT {
        let normalised = normalise(path);
        if !routed.contains(&normalised) {
            stale.push(format!(
                "  {path} — excused, but no route registers it ({reason})"
            ));
        } else if documented.contains(&normalised) {
            stale.push(format!(
                "  {path} — excused, but the document now names it ({reason})"
            ));
        }
    }

    assert!(
        stale.is_empty(),
        "DOCUMENTATION_EXEMPT has entries that are no longer true:\n{}\n\n\
         An exception list nobody prunes is the restated-list defect it was written \
         to guard against: it goes on excusing a path that has since been deleted, or \
         one the document has started naming, and the excuse is what keeps the gap \
         from being counted.",
        stale.join("\n"),
    );
}
