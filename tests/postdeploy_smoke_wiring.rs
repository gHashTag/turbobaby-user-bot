//! `scripts/postdeploy-smoke.sh` stays read-only, stays pointed at TurboBaby, and
//! keeps asking the server the questions the server actually answers (2026-09-24).
//!
//! The smoke is run against the live shop after every deploy and rollback
//! (`docs/ROLLBACK.md`), so a single write in it would be a write to production
//! on every run. Nothing compiles a shell script, so the guard reads it as text:
//!
//! * no HTTP write verb appears anywhere in the file (the brief's own check,
//!   `grep -c -E 'POST|PUT|PATCH|DELETE'` must be 0);
//! * every `curl` invocation carries no flag that sends a body or changes the
//!   method, and there is at least one invocation to judge (DECISIONS.md D16:
//!   a gate whose input can reach zero must say that it found something);
//! * the default target is the canonical origin in `src/config.rs`, never the
//!   other shop's service that shares the Railway project (AGENTS.md §12);
//! * each shape the script expects is still the shape the handler emits, so a
//!   renamed key turns this red here instead of turning the smoke red in
//!   production for a reason nobody can see.

const SMOKE: &str = include_str!("../scripts/postdeploy-smoke.sh");
const CONFIG: &str = include_str!("../src/config.rs");
const API_MOD: &str = include_str!("../src/api/mod.rs");
const API_BIKES: &str = include_str!("../src/api/bikes.rs");
const API_ORDERS: &str = include_str!("../src/api/orders.rs");
const DB_BIKES: &str = include_str!("../src/db/bikes.rs");
const MAIN: &str = include_str!("../src/main.rs");

/// The script's logical lines: a trailing backslash continues onto the next line.
fn logical_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        if let Some(head) = line.strip_suffix('\\') {
            current.push_str(head);
            current.push(' ');
        } else {
            current.push_str(line);
            lines.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// The argument lists of every `curl` invocation: what follows `curl ` when the
/// next token is a flag. `command -v curl` and `for tool in curl …` are not
/// invocations and are not matched.
fn curl_invocations(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in logical_lines(text) {
        if line.trim_start().starts_with('#') {
            continue;
        }
        let mut rest = line.as_str();
        while let Some(at) = rest.find("curl -") {
            let args = &rest[at + "curl ".len()..];
            found.push(args.to_string());
            rest = args;
        }
    }
    found
}

/// A flag that sends a request body, uploads, or picks a method other than GET.
fn is_write_flag(token: &str) -> bool {
    const LONG: [&str; 11] = [
        "--data",
        "--data-raw",
        "--data-binary",
        "--data-urlencode",
        "--data-ascii",
        "--json",
        "--form",
        "--form-string",
        "--upload-file",
        "--request",
        "--head",
    ];
    if let Some(long) = token.strip_prefix("--") {
        let name = long.split('=').next().unwrap_or(long);
        return LONG.contains(&format!("--{name}").as_str());
    }
    // A short-flag cluster such as `-sS`: any of d, F, T, X, I inside it is a write
    // (or a HEAD, which is not the GET this script promises either).
    match token.strip_prefix('-') {
        Some(cluster)
            if !cluster.is_empty() && cluster.chars().all(|c| c.is_ascii_alphabetic()) =>
        {
            cluster
                .chars()
                .any(|c| matches!(c, 'd' | 'F' | 'T' | 'X' | 'I'))
        }
        _ => false,
    }
}

#[test]
fn the_smoke_names_no_write_verb() {
    assert!(
        SMOKE.contains("#!/usr/bin/env bash") && SMOKE.len() > 1000,
        "the guard is reading the real script, not an empty file"
    );
    for verb in ["POST", "PUT", "PATCH", "DELETE"] {
        let lines: Vec<&str> = SMOKE.lines().filter(|l| l.contains(verb)).collect();
        assert!(
            lines.is_empty(),
            "scripts/postdeploy-smoke.sh must stay GET-only; `{verb}` appears in: {lines:?}"
        );
    }
}

#[test]
fn every_curl_invocation_is_a_bodiless_get() {
    let invocations = curl_invocations(SMOKE);
    assert_eq!(
        invocations.len(),
        1,
        "exactly one curl invocation (inside `fetch`), found {invocations:?}"
    );
    for args in &invocations {
        let bad: Vec<&str> = args
            .split_whitespace()
            .filter(|token| is_write_flag(token))
            .collect();
        assert!(
            bad.is_empty(),
            "a curl invocation sends a body or changes the method: {bad:?} in `curl {args}`"
        );
    }
}

#[test]
fn the_flag_reader_would_catch_a_write() {
    // The checker above passes for the worst reason if it cannot see a write at
    // all, so it is shown one of each shape first.
    for token in [
        "-d",
        "-sSd",
        "-X",
        "-F",
        "-T",
        "-I",
        "--data",
        "--data-raw",
        "--json",
        "--form",
        "--upload-file",
        "--request",
        "--request=GET",
        "--head",
    ] {
        assert!(is_write_flag(token), "{token} must read as a write flag");
    }
    for token in ["-sS", "-o", "-w", "--max-time", "-m", "\"$BASE$path\""] {
        assert!(
            !is_write_flag(token),
            "{token} must not read as a write flag"
        );
    }
    let planted = "x=\"$(curl -sS -X \\\n  GET \"$BASE\")\"\n";
    let invocations = curl_invocations(planted);
    assert_eq!(
        invocations.len(),
        1,
        "a continued invocation is one invocation"
    );
    assert!(
        invocations[0].split_whitespace().any(is_write_flag),
        "a method flag on a continued line is still seen"
    );
}

#[test]
fn the_default_target_is_turbobabys_canonical_origin() {
    let marker = "let canonical_web_app_url = \"";
    let start = CONFIG
        .find(marker)
        .map(|at| at + marker.len())
        .unwrap_or(CONFIG.len());
    assert!(
        start < CONFIG.len(),
        "src/config.rs names the canonical web app URL"
    );
    let end = CONFIG[start..]
        .find('"')
        .map(|len| start + len)
        .unwrap_or(start);
    assert!(
        end > start,
        "the canonical URL literal is closed and not empty"
    );
    let canonical = &CONFIG[start..end];
    assert!(
        canonical.starts_with("https://turbobaby-bot-"),
        "the canonical origin is TurboBaby's: {canonical}"
    );
    assert!(
        SMOKE.contains(&format!("BASE=\"${{1:-${{E2E_BASE_URL:-{canonical}}}}}\"")),
        "the smoke defaults to $1, then E2E_BASE_URL, then {canonical}"
    );
    for foreign in ["woody-weed-bot-production", "woody-woodpecker"] {
        assert!(
            !SMOKE.contains(foreign),
            "the smoke never targets another shop's service ({foreign})"
        );
    }
}

#[test]
fn what_the_smoke_expects_is_what_the_handlers_emit() {
    // 1. /health: 200 with db ok.
    assert!(SMOKE.contains("fetch health.json /health"));
    assert!(SMOKE.contains("db == \"ok\""));
    assert!(API_MOD.contains(".route(\"/health\", get(health_handler))"));
    assert!(API_MOD
        .contains("json!({\"status\": \"ok\", \"service\": \"turbobaby-bot\", \"db\": \"ok\"})"));

    // 2. /api/bikes: an object holding the `bikes` list.
    assert!(SMOKE.contains("fetch bikes.json /api/bikes"));
    assert!(SMOKE.contains("probe list \"$OUT_DIR/bikes.json\" bikes"));
    assert!(API_BIKES.contains(".route(\"/bikes\", get(list_bikes))"));
    assert!(API_BIKES.contains("json!({ \"bikes\": families })"));
    // The photo count reads `image_url` at the top of each family: the field lives
    // on `Bike`, which `BikeListing` flattens into the row.
    assert!(SMOKE.contains("item.get(\"image_url\")"));
    assert!(DB_BIKES.contains("pub image_url: Option<String>,"));
    assert!(DB_BIKES.contains("#[serde(flatten)]") && DB_BIKES.contains("pub bike: Bike,"));

    // 3. /api/delivery/zones: an object holding the `zones` list.
    assert!(SMOKE.contains("fetch zones.json /api/delivery/zones"));
    assert!(SMOKE.contains("probe list \"$OUT_DIR/zones.json\" zones"));
    assert!(API_ORDERS.contains(".route(\"/delivery/zones\", get(list_delivery_zones))"));
    assert!(API_ORDERS.contains("Ok(Json(json!({ \"zones\": zones })))"));

    // 4. An unmatched /api path: the JSON 404, reached only through the fallback.
    assert!(SMOKE.contains("UNMATCHED_PATH=\"/api/postdeploy-smoke-unmatched-route\""));
    assert!(SMOKE.contains("error == \"not_found\""));
    assert!(API_MOD.contains("router.fallback(api_not_found)"));
    assert!(API_MOD.contains("\"error\": \"not_found\","));
    for source in [API_MOD, API_BIKES, API_ORDERS, MAIN] {
        assert!(
            !source.contains("postdeploy-smoke"),
            "no route may claim the path the smoke uses to reach the fallback"
        );
    }

    // 5. `/` is the dist/index.html loaded at boot, served as is.
    assert!(SMOKE.contains("fetch index.served.html /"));
    assert!(SMOKE.contains("show \"$ref_sha:dist/index.html\""));
    assert!(MAIN.contains(".route(\"/\", get(spa_handler.clone()))"));
    assert!(MAIN.contains("walk_dir(&mut static_cache, std::path::Path::new(\"dist\"));"));
    assert!(MAIN.contains("String::from_utf8_lossy(&file.raw).to_string()"));
}
