//! The referral credit's API has ONE shape on both sides: what the server
//! registers, reads and answers is what the Mini App calls, sends and parses.
//! Round 5's integration, 2026-09-26.
//!
//! The owner answered R3 on 2026-09-26 («Должно начисляться исключительно за
//! то кто арендовал 10% скидка», «Пригласивший и может забрать скидкой за
//! аренду или деньгами»), and two lanes built it in parallel against one spec:
//! the server lane the six routes of `src/api/referral_credit.rs`, the client
//! lane the referrals page and the admin panel that call them. Both compile
//! against the wire types of `src/trios/referral_credit.rs`, so a FIELD cannot
//! drift between them. Everything that is a string or a choice can: the path,
//! the verb, which shared type a route answers with and which one a screen
//! parses, which body a screen serializes, and the field names and values the
//! server's hand-written parsers read. No compiler compares those, and the
//! screens are not even compiled by `cargo test` (`src/lib.rs` builds `pub mod
//! ui` for wasm32 only). So this file reads both sides as text against one
//! table, `ROUTES`, and every parser it uses is pinned by its own assertions
//! first, because a shape check over nothing found passes perfectly.
//!
//! `tests/ui_endpoints_exist.rs` already checks that every fetched path is
//! served; this file adds the verb, the body and the answer. The in-crate test
//! `the_bodies_the_screens_serialize_are_what_the_parsers_read`
//! (`src/api/referral_credit.rs`) runs each shared body through its parser.

#![allow(clippy::panic, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const CORE: &str = "src/trios/referral_credit.rs";
const SERVER: &str = "src/api/referral_credit.rs";
const LEDGER: &str = "src/db/referral_credit.rs";
const PAGE: &str = "src/ui/pages/referrals.rs";
const ADMIN: &str = "src/ui/screens/admin_screen.rs";

/// One route of the spec's shared API (§14), as each side must see it.
struct Route {
    /// `get` or `post`: the axum method router on the server, the helper's
    /// verb on the client.
    verb: &'static str,
    /// The path `routes()` registers under the `/api` nest.
    server_path: &'static str,
    /// The `format!` literal a screen builds, the API base first.
    client_url: &'static str,
    /// The screen that calls it.
    client_file: &'static str,
    handler: &'static str,
    /// The ledger function the handler answers from.
    ledger_fn: &'static str,
    /// The shared body a screen sends, and the server's parser for it.
    body: Option<(&'static str, &'static str)>,
    /// The shared type of the 200 answer.
    answer: &'static str,
}

const ROUTES: [Route; 6] = [
    Route {
        verb: "get",
        server_path: "/referral-credit/me/:telegram_id",
        client_url: "\"{}/api/referral-credit/me/{}\"",
        client_file: PAGE,
        handler: "get_my_credit",
        ledger_fn: "credit_summary",
        body: None,
        answer: "ReferralCredit",
    },
    Route {
        verb: "post",
        server_path: "/referral-credit/me/:telegram_id/requests",
        client_url: "\"{}/api/referral-credit/me/{}/requests\"",
        client_file: PAGE,
        handler: "post_my_request",
        ledger_fn: "open_request",
        body: Some(("OpenRequestBody", "parse_request_kind")),
        answer: "OpenRequestResponse",
    },
    Route {
        verb: "get",
        server_path: "/admin/referral-credit/overview",
        client_url: "\"{}/api/admin/referral-credit/overview\"",
        client_file: ADMIN,
        handler: "get_overview",
        ledger_fn: "overview",
        body: None,
        answer: "AdminOverview",
    },
    Route {
        verb: "post",
        server_path: "/admin/referral-credit/rentals",
        client_url: "\"{}/api/admin/referral-credit/rentals\"",
        client_file: ADMIN,
        handler: "post_rental",
        ledger_fn: "record_rental",
        body: Some(("RecordRentalBody", "parse_record_body")),
        answer: "RecordRentalResponse",
    },
    Route {
        verb: "post",
        server_path: "/admin/referral-credit/rentals/:id/reverse",
        client_url: "\"{}/api/admin/referral-credit/rentals/{}/reverse\"",
        client_file: ADMIN,
        handler: "post_reversal",
        ledger_fn: "reverse_rental",
        body: Some(("ReverseBody", "parse_reverse_body")),
        answer: "ReverseResponse",
    },
    Route {
        verb: "post",
        server_path: "/admin/referral-credit/requests/:id/resolve",
        client_url: "\"{}/api/admin/referral-credit/requests/{}/resolve\"",
        client_file: ADMIN,
        handler: "post_resolution",
        ledger_fn: "resolve_request",
        body: Some(("ResolveBody", "parse_resolve_body")),
        answer: "ResolveResponse",
    },
];

/// The client's HTTP helpers and their verbs. `fetch_credit` (the page) and
/// `referral_admin_post` (the admin panel) are the screens' own wrappers; the
/// test below pins what each wraps.
const CLIENT_HELPERS: [(&str, &str); 7] = [
    ("fetch_text_authed_full", "get"),
    ("fetch_text_authed", "get"),
    ("fetch_text_admin", "get"),
    ("fetch_credit", "get"),
    ("post_json_authed", "post"),
    ("post_json_admin_full", "post"),
    ("referral_admin_post", "post"),
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// A tracked file, LF-normalised, with every `//` comment cut (string
/// literals respected), so prose that names a path or a type never reads as
/// code. Its absence fails the test rather than emptying it.
fn code(rel: &str) -> String {
    let text = std::fs::read_to_string(root().join(rel))
        .unwrap_or_else(|e| panic!("{rel} must exist: {e}"))
        .replace("\r\n", "\n");
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
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

/// The part of a source above its first test module.
fn shipped(text: &str) -> &str {
    match text.find("#[cfg(test)]") {
        Some(at) => &text[..at],
        None => text,
    }
}

/// The item whose header starts with `header`, as (header start, body start,
/// body end): the body runs from the first `{` after the header to the
/// matching `}`. Panics when the header is gone or appears twice.
fn span_of(text: &str, header: &str) -> (usize, usize, usize) {
    let start = text
        .find(header)
        .unwrap_or_else(|| panic!("`{header}` is gone"));
    assert!(
        !text[start + header.len()..].contains(header),
        "`{header}` appears twice"
    );
    let open = start + text[start..].find('{').expect("an opening brace");
    let mut depth = 0usize;
    for (i, c) in text[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return (start, open, open + i + 1);
                }
            }
            _ => {}
        }
    }
    panic!("`{header}` never closes")
}

/// The body of the item: from its first `{` to the matching `}`.
fn body_of<'a>(text: &'a str, header: &str) -> &'a str {
    let (_, open, end) = span_of(text, header);
    &text[open..end]
}

/// The whole item, its signature included.
fn item_of<'a>(text: &'a str, header: &str) -> &'a str {
    let (start, _, end) = span_of(text, header);
    &text[start..end]
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// The first string literal in `hay`, and the byte just past its closing quote.
fn first_literal(hay: &str) -> (&str, usize) {
    let open = hay.find('"').expect("a literal");
    let end = open + 1 + hay[open + 1..].find('"').expect("a closing quote");
    (&hay[open + 1..end], end + 1)
}

/// The first string literal after each `needle` in `hay`, in order.
fn literals_after<'a>(hay: &'a str, needle: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = hay[from..].find(needle) {
        let start = from + at + needle.len();
        let (literal, past) = first_literal(&hay[start..]);
        out.push(literal);
        from = start + past;
    }
    out
}

/// The text of each literal that opens right at the end of `prefix`, which
/// ends with its opening quote (`action: "`, `Some("`).
fn quoted_after<'a>(hay: &'a str, prefix: &str) -> Vec<&'a str> {
    assert!(prefix.ends_with('"'), "{prefix} must end with its quote");
    hay.match_indices(prefix)
        .map(|(at, _)| {
            let start = at + prefix.len();
            &hay[start..start + hay[start..].find('"').expect("a closing quote")]
        })
        .collect()
}

/// The shared type names `pub struct`-declared in the core.
fn shared_types(core: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in core.lines() {
        if let Some(rest) = line.strip_prefix("pub struct ") {
            let name: String = rest
                .bytes()
                .take_while(|b| is_ident(*b))
                .map(char::from)
                .collect();
            out.insert(name);
        }
    }
    out
}

/// The `pub` field names of a shared struct, in declaration order.
fn fields_of(core: &str, name: &str) -> Vec<String> {
    body_of(core, &format!("pub struct {name} "))
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub "))
        .map(|rest| rest.split(':').next().expect("a field").trim().to_string())
        .collect()
}

/// Calls of `name(` in `code` (not its definition), as byte positions.
fn calls_of(code: &str, name: &str) -> Vec<usize> {
    let needle = format!("{name}(");
    let bytes = code.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = code[from..].find(&needle) {
        let pos = from + at;
        from = pos + needle.len();
        if pos > 0 && is_ident(bytes[pos - 1]) {
            continue;
        }
        if code[..pos].ends_with("fn ") {
            continue;
        }
        out.push(pos);
    }
    out
}

/// Every call of a client helper in `code`, sorted by position.
fn helper_calls(code: &str) -> Vec<(usize, &'static str)> {
    let mut out: Vec<(usize, &'static str)> = CLIENT_HELPERS
        .iter()
        .flat_map(|(name, _)| {
            calls_of(code, name)
                .into_iter()
                .map(move |pos| (pos, *name))
        })
        .collect();
    out.sort();
    out
}

fn verb_of(helper: &str) -> &'static str {
    CLIENT_HELPERS
        .iter()
        .find(|(name, _)| *name == helper)
        .map(|(_, verb)| *verb)
        .expect("a helper")
}

/// The helper call a URL literal at `at` reaches, found three ways in turn:
/// the call whose argument list encloses the literal (a `format!` around it
/// is looked through); else the helper that a `let <var> = format!(` binding
/// of it is handed to; else the first helper call after it (the admin
/// panel's resolve and reverse build their URL in a `match` and share one
/// call).
fn reached_call(code: &str, at: usize) -> (usize, &'static str) {
    let bytes = code.as_bytes();
    let mut depth = 0usize;
    let mut i = at;
    while i > 0 {
        i -= 1;
        match bytes[i] {
            b')' => depth += 1,
            b'(' if depth > 0 => depth -= 1,
            b'(' => {
                let mut s = i;
                while s > 0
                    && (is_ident(bytes[s - 1]) || bytes[s - 1] == b':' || bytes[s - 1] == b'!')
                {
                    s -= 1;
                }
                let ident = code[s..i].rsplit("::").next().unwrap_or("");
                if let Some((name, _)) = CLIENT_HELPERS.iter().find(|(n, _)| *n == ident) {
                    let start = i - name.len();
                    return (start, name);
                }
            }
            b';' | b'{' | b'}' if depth == 0 => break,
            _ => {}
        }
    }
    let statement = &code[i..at];
    if let Some(bind) = statement.rfind("let ") {
        let rest = &statement[bind + 4..];
        let var: String = rest
            .bytes()
            .take_while(|b| is_ident(*b))
            .map(char::from)
            .collect();
        if !var.is_empty() && rest[var.len()..].trim_start().starts_with("= format!(") {
            let handed = helper_calls(code)
                .into_iter()
                .filter(|(pos, _)| *pos > at)
                .find(|(pos, name)| {
                    let args = code[pos + name.len() + 1..].trim_start();
                    let args = args.strip_prefix('&').unwrap_or(args);
                    args.starts_with(var.as_str())
                        && !args
                            .as_bytes()
                            .get(var.len())
                            .copied()
                            .is_some_and(is_ident)
                });
            return handed.unwrap_or_else(|| panic!("`{var}` is handed to no helper"));
        }
    }
    helper_calls(code)
        .into_iter()
        .find(|(pos, _)| *pos > at)
        .expect("a helper call after the URL")
}

/// Shared types named in `region` right after `from_str::<` (a path prefix and
/// whitespace allowed): the types a screen parses there.
fn parsed_in(region: &str, shared: &BTreeSet<String>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut from = 0;
    while let Some(at) = region[from..].find("from_str::<") {
        let start = from + at + "from_str::<".len();
        let rest = region[start..].trim_start();
        let rest = rest
            .strip_prefix("crate::trios::referral_credit::")
            .unwrap_or(rest);
        let name: String = rest
            .bytes()
            .take_while(|b| is_ident(*b))
            .map(char::from)
            .collect();
        if shared.contains(&name) {
            out.insert(name);
        }
        from = start;
    }
    out
}

/// Shared `...Body` structs built as literals in `region` (`Name {`).
fn bodies_built_in(region: &str, shared: &BTreeSet<String>) -> BTreeSet<String> {
    shared
        .iter()
        .filter(|name| name.ends_with("Body"))
        .filter(|name| {
            let needle = format!("{name} {{");
            region
                .match_indices(&needle)
                .any(|(pos, _)| pos == 0 || !is_ident(region.as_bytes()[pos - 1]))
        })
        .cloned()
        .collect()
}

// ------------------------------------------------------------------ the parsers

#[test]
fn the_table_and_the_parsers_see_what_they_claim() {
    let core = code(CORE);
    let shared = shared_types(&core);
    assert!(
        shared.len() >= 16,
        "the core declares {} types",
        shared.len()
    );
    for route in &ROUTES {
        assert!(
            shared.contains(route.answer),
            "{} is not shared",
            route.answer
        );
        if let Some((body, _)) = route.body {
            assert!(shared.contains(body), "{body} is not shared");
        }
    }
    assert_eq!(
        fields_of(&core, "RecordRentalBody"),
        [
            "customer_telegram_id",
            "rental_amount_thb",
            "order_id",
            "note",
            "idempotency_key"
        ]
    );
    // The call finder: a definition is not a call, a longer name is not this one.
    let sample = "fn fetch_credit(u: &str) {}\nlet a = fetch_credit(&u);\nfetch_text_authed_full(&u);\nfetch_text_authed(&u);";
    assert_eq!(calls_of(sample, "fetch_credit").len(), 1);
    assert_eq!(calls_of(sample, "fetch_text_authed").len(), 1);
    // The three ways a URL reaches a helper.
    let enclosed = "x.then(|| { post_json_authed(\n &format!(\"{}/p\", b), &i) });";
    let at = enclosed.find("\"{}/p\"").expect("sample");
    assert_eq!(reached_call(enclosed, at).1, "post_json_authed");
    let bound = "{ let me = format!(\"{}/m\", b);\n post_json_authed(&other, &i);\n fetch_credit(&me, &i); }";
    let at = bound.find("\"{}/m\"").expect("sample");
    assert_eq!(reached_call(bound, at).1, "fetch_credit");
    let later =
        "match a { A { .. } => (format!(\"{}/r\", b), 1), };\n referral_admin_post(url, json);";
    let at = later.find("\"{}/r\"").expect("sample");
    assert_eq!(reached_call(later, at).1, "referral_admin_post");
    // The parse finder reads through a path prefix and a line break.
    let parse = "from_str::<\n crate::trios::referral_credit::ReverseResponse,\n>(&t); from_str::<Other>(&t)";
    assert_eq!(
        parsed_in(parse, &shared),
        BTreeSet::from(["ReverseResponse".to_string()])
    );
}

// ------------------------------------------------------------------ the server

#[test]
fn the_server_registers_exactly_the_six_routes_with_their_verbs() {
    let server = code(SERVER);
    let table = body_of(&server, "pub(crate) fn routes()");
    let mut registered = BTreeSet::new();
    let mut count = 0usize;
    let mut from = 0;
    while let Some(at) = table[from..].find(".route(") {
        let start = from + at + ".route(".len();
        let (path, past) = first_literal(&table[start..]);
        let after = start + past;
        let tail = table[after..]
            .trim_start()
            .trim_start_matches(',')
            .trim_start();
        let verb: String = tail
            .bytes()
            .take_while(|b| is_ident(*b))
            .map(char::from)
            .collect();
        let handler: String = tail[verb.len() + 1..]
            .bytes()
            .take_while(|b| is_ident(*b))
            .map(char::from)
            .collect();
        registered.insert((path.to_string(), verb, handler));
        count += 1;
        from = after;
    }
    let expected: BTreeSet<(String, String, String)> = ROUTES
        .iter()
        .map(|r| {
            (
                r.server_path.to_string(),
                r.verb.to_string(),
                r.handler.to_string(),
            )
        })
        .collect();
    assert_eq!(registered, expected);
    assert_eq!(count, ROUTES.len(), "a path registered twice");
}

#[test]
fn each_route_answers_the_shared_type_the_table_names() {
    let server = code(SERVER);
    let ledger = code(LEDGER);
    for route in &ROUTES {
        let handler = item_of(shipped(&server), &format!("async fn {}(", route.handler));
        let call = format!("crate::db::referral_credit::{}(", route.ledger_fn);
        assert!(
            handler.contains(&call),
            "{} does not answer from {}",
            route.handler,
            route.ledger_fn
        );
        // `Json(value)`, or `.map(Json)` under a `Json<...>` return type.
        assert!(
            handler.contains("Json(") || handler.contains(".map(Json)"),
            "{} answers no JSON",
            route.handler
        );
        let header = format!("pub(crate) async fn {}(", route.ledger_fn);
        let start = ledger
            .find(&header)
            .unwrap_or_else(|| panic!("{header} is gone"));
        let signature = &ledger[start..start + ledger[start..].find('{').expect("a body")];
        let returns = signature.rsplit("->").next().expect("a return type").trim();
        let answered = returns
            .strip_prefix("CreditResult<")
            .or_else(|| returns.strip_prefix("Result<"))
            .unwrap_or_else(|| panic!("{} returns {returns}", route.ledger_fn));
        let answered: String = answered
            .bytes()
            .take_while(|b| is_ident(*b))
            .map(char::from)
            .collect();
        assert_eq!(
            answered, route.answer,
            "{} answers {answered}",
            route.server_path
        );
    }
}

#[test]
fn each_body_parser_reads_exactly_the_shared_bodys_fields() {
    let core = code(CORE);
    let server = code(SERVER);
    for route in &ROUTES {
        let handler = item_of(shipped(&server), &format!("async fn {}(", route.handler));
        let Some((body, parser)) = route.body else {
            assert!(
                !handler.contains("body: Bytes"),
                "{} is a GET and reads a body",
                route.handler
            );
            continue;
        };
        assert!(
            handler.contains("body: Bytes"),
            "{} reads no body",
            route.handler
        );
        assert!(
            handler.contains(&format!("{parser}(&body)")),
            "{} does not read its body with {parser}",
            route.handler
        );
        let read: BTreeSet<String> =
            literals_after(body_of(shipped(&server), &format!("fn {parser}(")), ".get(")
                .into_iter()
                .map(str::to_string)
                .collect();
        let declared: BTreeSet<String> = fields_of(&core, body).into_iter().collect();
        assert_eq!(read, declared, "{parser} against {body}");
    }
}

// ------------------------------------------------------------------ the client

#[test]
fn each_screen_calls_each_route_with_its_verb_and_parses_its_answer() {
    let core = code(CORE);
    let shared = shared_types(&core);
    for file in [PAGE, ADMIN] {
        let text = code(file);
        let calls = helper_calls(&text);
        let mut groups: Vec<(usize, &'static str, Vec<&Route>)> = Vec::new();
        for route in ROUTES.iter().filter(|r| r.client_file == file) {
            let sites: Vec<usize> = text
                .match_indices(route.client_url)
                .map(|(p, _)| p)
                .collect();
            assert!(
                !sites.is_empty(),
                "{file} never builds {}",
                route.client_url
            );
            for site in sites {
                let (pos, helper) = reached_call(&text, site);
                assert_eq!(
                    verb_of(helper),
                    route.verb,
                    "{file}: {} goes out through {helper}",
                    route.client_url
                );
                match groups.iter_mut().find(|(p, _, _)| *p == pos) {
                    Some((_, _, routes)) => {
                        if !routes.iter().any(|r| r.client_url == route.client_url) {
                            routes.push(route);
                        }
                    }
                    None => groups.push((pos, helper, vec![route])),
                }
            }
        }
        for (pos, helper, routes) in &groups {
            let answers: BTreeSet<String> = routes.iter().map(|r| r.answer.to_string()).collect();
            if *helper == "fetch_credit" {
                // The page's wrapper parses inside itself; pinned below.
                assert_eq!(answers, BTreeSet::from(["ReferralCredit".to_string()]));
                continue;
            }
            let next = calls
                .iter()
                .map(|(p, _)| *p)
                .find(|p| p > pos)
                .unwrap_or(text.len());
            let parsed = parsed_in(&text[*pos..next], &shared);
            let parsed: BTreeSet<String> = parsed.into_iter().filter(|t| t != "ApiError").collect();
            assert_eq!(
                parsed, answers,
                "{file}: what the call at byte {pos} parses"
            );
            let bodies: BTreeSet<String> = routes
                .iter()
                .filter_map(|r| r.body.map(|(b, _)| b.to_string()))
                .collect();
            if !bodies.is_empty() {
                let prev = calls
                    .iter()
                    .rev()
                    .map(|(p, _)| *p)
                    .find(|p| p < pos)
                    .unwrap_or(0);
                assert_eq!(
                    bodies_built_in(&text[prev..*pos], &shared),
                    bodies,
                    "{file}: what the call at byte {pos} sends"
                );
            }
        }
        // No credit path beyond the table's.
        let table: BTreeSet<&str> = ROUTES
            .iter()
            .filter(|r| r.client_file == file)
            .map(|r| r.client_url.trim_matches('"'))
            .collect();
        for needle in ["/api/referral-credit", "/api/admin/referral-credit"] {
            for (at, _) in text.match_indices(needle) {
                let open = text[..at].rfind('"').expect("inside a literal");
                let close = at + text[at..].find('"').expect("a closing quote");
                let literal = &text[open + 1..close];
                assert!(
                    table.contains(literal),
                    "{file} builds {literal}, not in the table"
                );
            }
        }
    }
}

#[test]
fn the_screens_wrappers_are_the_verbs_they_claim() {
    let core = code(CORE);
    let shared = shared_types(&core);
    let page = code(PAGE);
    let fetch = body_of(&page, "async fn fetch_credit(");
    assert!(fetch.contains("fetch_text_authed_full("));
    assert!(!fetch.contains("post_json"));
    assert_eq!(
        parsed_in(fetch, &shared),
        BTreeSet::from(["ReferralCredit".to_string()])
    );
    let admin = code(ADMIN);
    let post = body_of(&admin, "async fn referral_admin_post(");
    assert!(post.contains("post_json_admin_full("));
    assert!(post.contains("AdminAuth"));
    assert!(!post.contains("fetch_text"));
}

#[test]
fn the_kinds_and_actions_the_screens_send_are_the_ones_the_server_admits() {
    let core = code(CORE);
    let server = code(SERVER);
    // `pub const REQUEST_KINDS: [&str; 2] = [...];`, whose type holds a `;` too.
    let declaration = &core[core.find("pub const REQUEST_KINDS").expect("the kinds")..];
    let declaration = &declaration[declaration.find("= [").expect("a list")..];
    let mut list = &declaration[..declaration.find("];").expect("the end")];
    let mut kinds = BTreeSet::new();
    while list.contains('"') {
        let (literal, past) = first_literal(list);
        kinds.insert(literal.to_string());
        list = &list[past..];
    }
    assert_eq!(
        kinds,
        BTreeSet::from(["payout".to_string(), "redeem".to_string()])
    );

    let page = code(PAGE);
    let sent_kinds: BTreeSet<String> = quoted_after(&page, "open_request(\"")
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(sent_kinds, kinds, "the page's two buttons");

    let admin = code(ADMIN);
    let panel = &admin[admin.find("struct ReferralRentalForm").expect("the panel")..];
    let sent_actions: BTreeSet<String> = quoted_after(panel, "action: \"")
        .into_iter()
        .map(str::to_string)
        .collect();
    let admitted: BTreeSet<String> = quoted_after(
        body_of(shipped(&server), "fn parse_resolve_body("),
        "Some(\"",
    )
    .into_iter()
    .map(str::to_string)
    .collect();
    assert_eq!(
        admitted,
        BTreeSet::from(["paid".to_string(), "declined".to_string()])
    );
    assert_eq!(sent_actions, admitted, "the panel's resolve buttons");
}
