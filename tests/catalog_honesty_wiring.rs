//! Two things the customer catalog used to say that the shop's own rules forbid,
//! guarded as text where the host cannot compile the screens.
//!
//! `src/lib.rs` gates `pub mod ui;` on `#[cfg(target_arch = "wasm32")]`, so
//! `cargo test` compiles nothing under `src/ui`. What is guarded here is the
//! text of the two customer screens (`catalog_screen.rs`, `bike_detail.rs`)
//! against the seed and the contract, and nothing stronger is claimed.
//!
//! WHY IT EXISTS (measured 2026-09-24 on upstream main f0640f8):
//!
//! 1. **The CLICK 125 redirect named two machines the shop does not have.**
//!    `CLICK_125_ALTERNATIVES` was `["PCX 150", "ADV 150", "NMAX 155"]` and the
//!    closed-family card printed it as «Вместо неё: PCX 150 · ADV 150 · NMAX 155».
//!    PCX 150 and ADV 150 sit in the seed's `price_list_only` block with zero
//!    units, `specs/turbobaby/availability.t27` records `[0, 0, 5]` units for the
//!    three redirects, and the owner's knowledge base forbids naming a model
//!    outside the fleet "not even as a replacement" (brain:knowledge_base:108-110
//!    and :345-346, brain:business_rules:7-8). Only NMAX 155 is an offered family
//!    with units.
//! 2. **A seeded count was printed as availability.** Both screens printed
//!    «Свободно {0} из {1}» / «Свободно: {0}» from `units_available`, a count
//!    seeded on 2026-09-12 and changed only by an admin. `availability.t27`
//!    declares `FILE_MAY_CONFIRM = false`, and the brain says not to promise
//!    without checking occupancy (brain:knowledge_base:336). The same count also
//!    fed a «Свободны сейчас» filter chip and a «Свободные цвета:» row, which say
//!    the same thing in other words. The customer screens now say that a manager
//!    confirms availability (`T_BIKE_AVAILABILITY_UNKNOWN`), which is copy the
//!    repository already publishes; the admin screen keeps its count.
//!
//! What stays, on purpose: a zero count may still RULE a family out (the
//! detail's «all taken» line and the Book control's reason), because the
//! contract allows the file to rule out (`FILE_MAY_RULE_OUT = true`) and forbids
//! it only to confirm.
//!
//! This file imports nothing from the crate on purpose: it compiles against any
//! tree, so on a tree where a screen still prints the count it fails by CONTENT.

use std::fs;
use std::path::{Path, PathBuf};

const CATALOG: &str = "src/ui/screens/catalog_screen.rs";
const DETAIL: &str = "src/ui/screens/bike_detail.rs";
const ADMIN: &str = "src/ui/screens/admin_screen.rs";
const SEED: &str = "data/fleet_seed.json";
const AVAILABILITY_SPEC: &str = "specs/turbobaby/availability.t27";

/// i18n keys that state, in one wording or another, that a unit is free right
/// now. Every one of them reads the seeded count as a confirmation.
const CONFIRMING_KEYS: [&str; 4] = [
    "T_BIKE_AVAILABILITY",
    "T_BIKE_AVAILABILITY_FREE",
    "T_BIKE_COLORS_AVAILABLE",
    "T_BIKE_FILTER_FREE_NOW",
];

/// The copy the customer is shown instead. It exists in both locales already.
const MANAGER_CONFIRMS_KEY: &str = "T_BIKE_AVAILABILITY_UNKNOWN";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// A tracked file, normalised to LF. A missing file is a failure, not an empty
/// string.
fn source(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|e| panic!("{relative} must exist for this guard to mean anything: {e}"))
        .replace("\r\n", "\n")
}

/// The code of one line: the part before a `//` that is not inside a string
/// literal, so a doc comment that names a key is not mistaken for a use of it.
fn code_of(line: &str) -> String {
    let mut in_string = false;
    let mut escaped = false;
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            break;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// The whole file with every line comment removed.
fn code(text: &str) -> String {
    text.lines().map(code_of).collect::<Vec<_>>().join("\n")
}

/// Does `word` occur in `text` as a whole identifier (not as a prefix of a
/// longer one, so `T_BIKE_AVAILABILITY` does not match `T_BIKE_AVAILABILITY_UNKNOWN`)?
fn has_identifier(text: &str, word: &str) -> bool {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut start = 0;
    while let Some(found) = text[start..].find(word) {
        let at = start + found;
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        if !before.is_some_and(is_ident) && !after.is_some_and(is_ident) {
            return true;
        }
        start = at + word.len();
    }
    false
}

/// The string literals inside `const NAME: [&str; N] = [ ... ];`, in order, or
/// `None` when the constant is not declared in that shape.
fn str_array_const(text: &str, name: &str) -> Option<Vec<String>> {
    let head = format!("const {name}: [&str; ");
    let at = text.find(&head)?;
    let rest = &text[at..];
    let open = rest.find("= [")? + 3;
    let close = rest[open..].find("];")? + open;
    let body = &rest[open..close];
    let mut out = Vec::new();
    let mut remaining = body;
    while let Some(q) = remaining.find('"') {
        let tail = &remaining[q + 1..];
        let end = tail.find('"')?;
        out.push(tail[..end].to_string());
        remaining = &tail[end + 1..];
    }
    let declared: usize = rest[head.len()..].split(']').next()?.trim().parse().ok()?;
    assert_eq!(
        declared,
        out.len(),
        "{name} declares {declared} elements and lists {}",
        out.len()
    );
    Some(out)
}

/// The body of `fn NAME(` up to its closing brace at the same indentation.
fn fn_body<'a>(text: &'a str, signature: &str) -> &'a str {
    let at = text
        .find(signature)
        .unwrap_or_else(|| panic!("`{signature}` must be declared for this guard to read it"));
    let rest = &text[at..];
    let end = rest
        .find("\n}\n")
        .unwrap_or_else(|| panic!("`{signature}` has no closing brace at column zero"));
    &rest[..end]
}

/// The offered, in-stock families of the seed, as `(model, units_total)`.
fn seed_offered_families_with_units() -> Vec<(String, i64)> {
    let seed: serde_json::Value =
        serde_json::from_str(&source(SEED)).expect("data/fleet_seed.json must parse");
    let families = seed["families"]
        .as_array()
        .expect("the seed must carry a families array");
    // D16: a check over an empty list passes for the worst reason.
    assert!(
        families.len() >= 10,
        "the seed parser found only {} families; it has gone blind",
        families.len()
    );
    families
        .iter()
        .filter(|f| f["offered"].as_bool() == Some(true))
        .filter_map(|f| {
            let units = f["units_total"].as_i64()?;
            (units >= 1).then(|| (f["model"].as_str().unwrap_or("").to_string(), units))
        })
        .collect()
}

/// Models the seed publishes in the tariff with no machine behind them.
fn seed_price_list_only_models() -> Vec<String> {
    let seed: serde_json::Value =
        serde_json::from_str(&source(SEED)).expect("data/fleet_seed.json must parse");
    let rows = seed["price_list_only"]["families"]
        .as_array()
        .expect("the seed must carry price_list_only.families");
    assert!(
        !rows.is_empty(),
        "price_list_only parsed empty; it has gone blind"
    );
    rows.iter()
        .filter_map(|f| f["model"].as_str().map(str::to_string))
        .collect()
}

// -- The CLICK 125 redirect ---------------------------------------------------

#[test]
fn every_click_125_alternative_is_an_offered_seed_family_with_units() {
    let catalog = code(&source(CATALOG));
    let alternatives = str_array_const(&catalog, "CLICK_125_ALTERNATIVES").expect(
        "catalog_screen.rs must declare `const CLICK_125_ALTERNATIVES: [&str; N] = [..];` \
         -- an empty list is allowed, a missing constant is not",
    );
    let offered = seed_offered_families_with_units();
    let not_in_fleet = seed_price_list_only_models();

    let mut offences = Vec::new();
    for label in &alternatives {
        if not_in_fleet.iter().any(|m| m == label) {
            offences.push(format!(
                "{label}: a price-list-only model with zero units in the fleet"
            ));
        } else if !offered.iter().any(|(model, _)| model == label) {
            offences.push(format!(
                "{label}: not the model of any offered seed family with units"
            ));
        }
    }
    assert!(
        offences.is_empty(),
        "the CLICK 125 redirect names machines the shop cannot hand over \
         (brain:knowledge_base:345-346 forbids them even as a replacement):\n  {}",
        offences.join("\n  ")
    );
}

#[test]
fn the_click_125_redirect_matches_the_contracts_shown_labels() {
    // Gate 3 binds the same pair; this is the host-side half, so a cargo test
    // run sees a drift without running the python gates.
    let catalog = code(&source(CATALOG));
    let rust = str_array_const(&catalog, "CLICK_125_ALTERNATIVES")
        .expect("catalog_screen.rs must declare CLICK_125_ALTERNATIVES");
    let spec = source(AVAILABILITY_SPEC);
    let line = spec
        .lines()
        .find(|l| l.starts_with("pub const CLICK_125_REDIRECT_LABELS_SHOWN "))
        .expect("availability.t27 must declare CLICK_125_REDIRECT_LABELS_SHOWN");
    let contract: Vec<String> = line
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect();
    assert_eq!(
        rust, contract,
        "the redirect the screen shows and the one the contract records disagree"
    );
}

// -- A seeded count is not availability ---------------------------------------

#[test]
fn no_customer_screen_reads_the_seeded_count_as_a_confirmation() {
    let mut offences = Vec::new();
    for path in [CATALOG, DETAIL] {
        let text = code(&source(path));
        for key in CONFIRMING_KEYS {
            if has_identifier(&text, key) {
                offences.push(format!("{path} uses {key}"));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "a customer screen still prints the seeded unit count as availability \
         (availability.t27 FILE_MAY_CONFIRM = false):\n  {}",
        offences.join("\n  ")
    );
}

#[test]
fn the_availability_line_says_a_manager_confirms_and_reads_no_count() {
    let catalog = code(&source(CATALOG));
    let body = fn_body(&catalog, "pub fn availability_line(");
    assert!(
        has_identifier(body, MANAGER_CONFIRMS_KEY),
        "availability_line must answer with {MANAGER_CONFIRMS_KEY}:\n{body}"
    );
    for field in ["units_available", "units_total"] {
        assert!(
            !body.contains(field),
            "availability_line reads `{field}`, so the line can again become a count:\n{body}"
        );
    }
    // D16: both screens must still show the line, or "reads no count" is true
    // of a function nobody calls.
    for path in [CATALOG, DETAIL] {
        let text = code(&source(path));
        assert!(
            text.contains("availability_line(lang)"),
            "{path} no longer calls availability_line; the guard above would be vacuous"
        );
    }
}

#[test]
fn the_catalog_list_offers_no_free_now_filter_over_the_seeded_count() {
    let catalog = code(&source(CATALOG));
    assert!(
        !catalog.contains("units_available, Some(n) if n > 0"),
        "the catalog list filters on the seeded count again"
    );
    assert!(
        !has_identifier(&catalog, "only_free"),
        "the catalog list keeps a free-now toggle over the seeded count"
    );
}

#[test]
fn the_admin_screen_keeps_its_count() {
    // The admin view stays by design: the count is the admin's own record, and
    // the admin is the one who changes it.
    let admin = code(&source(ADMIN));
    assert!(
        admin.contains("count_or_dash(bike.units_available)"),
        "the admin family row no longer shows its unit count"
    );
    assert!(
        admin.contains("count_or_dash(s.units_available)"),
        "the admin stat card no longer shows the free-unit count"
    );
}
