//! Things the customer catalog used to say that the shop's own rules forbid,
//! guarded as text where the host cannot compile the screens (two since
//! 2026-09-24, a third, the bot's /start line, since 2026-09-25).
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
//!    with units. On 2026-09-25 the owner decided the same, for now: NMAX 155
//!    alone is offered instead. The seed's `offer_instead` now says so, and it is
//!    checked against the screen below as well.
//! 2. **A seeded count was printed as availability.** Both screens printed
//!    «Свободно {0} из {1}» / «Свободно: {0}» from `units_available`, a count
//!    seeded on 2026-09-12 and changed only by an admin. `availability.t27`
//!    declares `FILE_MAY_CONFIRM = false`, and the brain says not to promise
//!    without checking occupancy (brain:knowledge_base:336). The same count also
//!    fed a «Свободны сейчас» filter chip and a «Свободные цвета:» row, which say
//!    the same thing in other words. The customer screens now say that a manager
//!    confirms availability (`T_BIKE_AVAILABILITY_UNKNOWN`), which is copy the
//!    repository already publishes; the admin screen keeps its count.
//! 3. **The bot's /start welcome named CLICK 125 as the catalog's lower end**
//!    (added 2026-09-25). `src/locales.rs` `welcome_feature1` read "from Click
//!    125 to X-ADV 750" in both locales, though CLICK 125 is offered to nobody.
//!    Its ends are now the seed's smallest and largest offered families, and
//!    the guard below derives them from the seed rather than trusting a list.
//!    It reads the bot's locale file, not a screen, which is the one exception
//!    to the sentence above about what this file guards.
//!
//! 3. **A seeded zero was printed as occupancy.** Under a zero count the detail
//!    screen added «Сейчас все байки этой модели заняты.» / "Every bike of this
//!    model is out right now." (`T_BIKE_UNITS_EMPTY`) below the manager line:
//!    the same seeded count, read as a fact about right now. Asked on
//!    2026-09-25 whether to remove it (the owner's second list of that day,
//!    answer 5), the owner answered «Наверное» ("probably"). The line and its
//!    key are gone, and nothing was written in their place.
//!
//! 4. **A seeded zero refused a booking** (added 2026-09-26). Answer 5 named the
//!    line, not the Book control, so under the same zero the control stayed
//!    disabled with «Все байки этой модели заняты» / "Every bike of this model
//!    is taken" (`T_BIKE_BOOK_BLOCKED_NO_UNITS`), and with «Наличие не
//!    подтверждено» / "Availability not confirmed"
//!    (`T_BIKE_BOOK_BLOCKED_UNKNOWN_AVAILABILITY`) when no count was served.
//!    Asked on 2026-09-26 (question I), the owner answered: «Разрешить бронь,
//!    наличие уточнит менеджер» ("Allow booking; the manager will confirm
//!    availability"). No booking decision reads the count now, on the client
//!    or on the server (which never refused on it), both keys are deleted, and
//!    the manager line is what every card and detail says about availability.
//!    `availability.t27` records it (`NO_UNITS_BOOK_REASON_*`); the second
//!    reason's removal is that contract's reading of the answer, and says so.
//!
//! The file may still rule a family out of being offered INSTEAD of CLICK 125
//! (`FILE_MAY_RULE_OUT = true`, the redirect guard above); it rules nobody out
//! of booking.
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
const I18N: &str = "src/trios/i18n.rs";
const BIKE_CARD: &str = "src/ui/components/bike_card.rs";
const ORDERS_API: &str = "src/api/orders.rs";

/// The Book control's two reasons that read the seeded count, deleted
/// 2026-09-26 (question I), as (constant, literal, Russian arm, English arm).
/// None of them may come back without the owner.
const RETIRED_BOOK_REASONS: [(&str, &str, &str, &str); 2] = [
    (
        "T_BIKE_BOOK_BLOCKED_NO_UNITS",
        "\"bike.book.blocked.no_units\"",
        "Все байки этой модели заняты",
        "Every bike of this model is taken",
    ),
    (
        "T_BIKE_BOOK_BLOCKED_UNKNOWN_AVAILABILITY",
        "\"bike.book.blocked.unknown_availability\"",
        "Наличие не подтверждено",
        "Availability not confirmed",
    ),
];

/// The `BookBlock` arms that read the count, gone with their keys.
const RETIRED_BOOK_ARMS: [&str; 2] = ["NoUnitsFree", "UnknownAvailability"];

/// The key of the detail's «all taken» line, deleted 2026-09-25, and the two
/// sentences it carried. None of the three may come back without the owner.
const RETIRED_UNITS_EMPTY_KEY: &str = "T_BIKE_UNITS_EMPTY";
const RETIRED_UNITS_EMPTY_LITERAL: &str = "\"bike.units.empty\"";
const RETIRED_UNITS_EMPTY_SENTENCES: [&str; 2] = [
    "Сейчас все байки этой модели заняты.",
    "Every bike of this model is out right now.",
];

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

#[test]
fn the_click_125_redirect_is_the_seeds_offer_instead() {
    // The owner decided on 2026-09-25, for now, that NMAX 155 alone is offered
    // instead of CLICK 125 (DECISIONS.md, D12 amendment of that date). The seed
    // records the decision as family keys, the screen prints labels, and until
    // that date the two lists differed (three keys against one label) with
    // nothing to notice. Each key is resolved to its seed family's model, so the
    // decision and what a customer reads cannot drift apart in silence.
    let seed: serde_json::Value =
        serde_json::from_str(&source(SEED)).expect("data/fleet_seed.json must parse");
    let keys: Vec<String> = seed["pricing_policy"]["not_offered"]["click-125"]["offer_instead"]
        .as_array()
        .expect("the seed must carry pricing_policy.not_offered.click-125.offer_instead")
        .iter()
        .map(|key| {
            key.as_str()
                .expect("offer_instead holds family keys")
                .to_string()
        })
        .collect();
    // D16: an empty decision would compare equal to an emptied screen constant.
    assert!(
        !keys.is_empty(),
        "offer_instead parsed empty; the comparison below would prove nothing"
    );
    let families = seed["families"]
        .as_array()
        .expect("the seed must carry a families array");
    let labels: Vec<String> = keys
        .iter()
        .map(|key| {
            families
                .iter()
                .find(|family| family["key"].as_str() == Some(key.as_str()))
                .and_then(|family| family["model"].as_str())
                .unwrap_or_else(|| {
                    panic!("offer_instead names {key}, which is no in-stock family of the seed")
                })
                .to_string()
        })
        .collect();
    let catalog = code(&source(CATALOG));
    let rust = str_array_const(&catalog, "CLICK_125_ALTERNATIVES")
        .expect("catalog_screen.rs must declare CLICK_125_ALTERNATIVES");
    assert_eq!(
        rust, labels,
        "the screen's CLICK 125 redirect is not the owner's decision the seed records \
         (keys {keys:?})"
    );
}

// -- The bot's /start catalog line -----------------------------------------------

/// The bot's locale file. Its `welcome_feature1` field is the first line of the
/// welcome `/start` sends, and it names the catalog's two ends.
const LOCALES: &str = "src/locales.rs";

/// The two ends `welcome_feature1` names, once per published locale, in file
/// order (ru, then en). A line of any other shape fails, so a rewording has to
/// come back here and cannot slip past the comparison below.
fn start_catalog_ranges() -> Vec<(String, String)> {
    let text = source(LOCALES);
    let rows: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("welcome_feature1: \"")?;
            Some(rest[..rest.find('"')?].to_string())
        })
        .collect();
    // D16: both published locales, or the comparison below proves nothing.
    assert_eq!(
        rows.len(),
        2,
        "{LOCALES} must set welcome_feature1 once per published locale, found {rows:?}"
    );
    rows.iter()
        .map(|row| {
            let ends = if let Some(rest) = row.strip_prefix("Каталог: от ") {
                rest.split_once(" до ")
            } else if let Some(rest) = row.strip_prefix("Catalog: from ") {
                rest.split_once(" to ")
            } else {
                None
            };
            let (low, high) = ends.unwrap_or_else(|| {
                panic!("welcome_feature1 changed shape, re-read this guard: {row}")
            });
            (low.to_string(), high.to_string())
        })
        .collect()
}

#[test]
fn the_start_reply_names_the_smallest_and_largest_offered_families() {
    // Until 2026-09-25 the line read "from Click 125 to X-ADV 750" in both
    // locales. CLICK 125 is closed to new rentals and offered to nobody (D12),
    // so the welcome named a machine the shop does not rent. The two ends are
    // now measured from the seed: the offered families (price-list-only ones
    // are not in `families` at all), smallest and largest by displacement, a
    // tie broken by the published daily rate (the lower one for the smallest
    // end). That rule gives NMAX 155 (155 cc, shared with XSR 155, which rents
    // for more; NMAX 155 is also the cheapest offered family outright) and
    // X-ADV 750 (alone at 750 cc).
    let seed: serde_json::Value =
        serde_json::from_str(&source(SEED)).expect("data/fleet_seed.json must parse");
    let families = seed["families"]
        .as_array()
        .expect("the seed must carry a families array");
    let mut offered: Vec<(i64, i64, String)> = families
        .iter()
        .filter(|f| f["offered"].as_bool() == Some(true))
        .map(|f| {
            let model = f["model"].as_str().unwrap_or("").to_string();
            let cc = f["displacement_cc"]
                .as_i64()
                .unwrap_or_else(|| panic!("{model}: an offered family has no displacement"));
            let rate = f["base_rate_thb_day"]
                .as_i64()
                .unwrap_or_else(|| panic!("{model}: an offered family has no published rate"));
            (cc, rate, model)
        })
        .collect();
    // D16: the seed publishes thirteen offered families; a parser that lost
    // them would compare two empty ends.
    assert!(
        offered.len() >= 10,
        "only {} offered families parsed; the seed parser has gone blind",
        offered.len()
    );
    offered.sort();
    let smallest = offered.first().expect("offered families").2.clone();
    let largest = offered.last().expect("offered families").2.clone();

    let not_in_fleet = seed_price_list_only_models();
    for (low, high) in start_catalog_ranges() {
        for end in [&low, &high] {
            assert!(
                !not_in_fleet.contains(end),
                "the /start reply names {end}, a price-list-only model with no machine"
            );
            assert!(
                offered.iter().any(|(_, _, model)| model == end),
                "the /start reply names {end}, which is no offered family of the seed"
            );
        }
        assert_eq!(
            (low.as_str(), high.as_str()),
            (smallest.as_str(), largest.as_str()),
            "the /start reply's catalog range is not the seed's smallest and largest offered families"
        );
    }

    // The contract records the same two labels; a change of either side has to
    // reach the other.
    let spec = source(AVAILABILITY_SPEC);
    let line = spec
        .lines()
        .find(|l| l.starts_with("pub const START_CATALOG_RANGE_LABELS "))
        .expect("availability.t27 must declare START_CATALOG_RANGE_LABELS");
    let contract: Vec<String> = line
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect();
    assert_eq!(
        contract,
        vec![smallest, largest],
        "availability.t27 records a /start catalog range the seed does not give"
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

// -- A seeded zero is not occupancy either (2026-09-25) -----------------------

/// A top-level `pub const NAME : bool = true|false;` of `availability.t27`.
fn spec_bool(spec: &str, name: &str) -> bool {
    let head = format!("pub const {name} : bool = ");
    let line = spec
        .lines()
        .find(|l| l.starts_with(&head))
        .unwrap_or_else(|| panic!("availability.t27 must declare `{name}` as a bool"));
    match line[head.len()..].trim().trim_end_matches(';').trim() {
        "true" => true,
        "false" => false,
        other => panic!("availability.t27 `{name}` holds `{other}`, not a bool"),
    }
}

#[test]
fn the_detail_prints_no_all_taken_line_and_its_key_is_gone() {
    // The owner's second list of 2026-09-25, answer 5 («Наверное»).
    let detail = code(&source(DETAIL));
    assert!(
        !has_identifier(&detail, RETIRED_UNITS_EMPTY_KEY),
        "{DETAIL} renders {RETIRED_UNITS_EMPTY_KEY} again; the owner removed that line on \
         2026-09-25 (availability.t27 UNITS_EMPTY_LINE_IS_SHOWN = false)"
    );
    assert!(
        !detail.contains("units_available == Some(0)"),
        "{DETAIL} branches on a seeded zero again; since 2026-09-26 no customer \
         decision reads the count (availability.t27, question I)"
    );
    let i18n = code(&source(I18N));
    assert!(
        !has_identifier(&i18n, RETIRED_UNITS_EMPTY_KEY)
            && !i18n.contains(RETIRED_UNITS_EMPTY_LITERAL),
        "{I18N} declares the retired key again; locale_policy.t27 counts it as deleted"
    );
    for path in [I18N, DETAIL, CATALOG] {
        let text = code(&source(path));
        for sentence in RETIRED_UNITS_EMPTY_SENTENCES {
            assert!(
                !text.contains(sentence),
                "{path} carries the retired sentence {sentence:?} again"
            );
        }
    }
    // D16: the block the line stood in still renders its heading and the
    // manager line, or "no all-taken line" would be true of a block nobody
    // renders.
    assert!(
        has_identifier(&detail, "T_BIKE_UNITS_TITLE") && detail.contains("\"{availability}\""),
        "{DETAIL} no longer renders its availability block; this check is blind"
    );
    // The contract records the same state; a flag flipped back without the
    // code (or the reverse) is a drift this test sees.
    let spec = source(AVAILABILITY_SPEC);
    assert!(
        !spec_bool(&spec, "UNITS_EMPTY_LINE_IS_SHOWN"),
        "availability.t27 says the all-taken line is shown, and the screen does not show it"
    );
    assert!(
        !spec_bool(&spec, "UNITS_EMPTY_KEY_IS_DECLARED"),
        "availability.t27 says the retired key is declared, and {I18N} does not declare it"
    );
}

// -- A seeded count refuses no booking (owner, 2026-09-26) --------------------

#[test]
fn the_book_control_reads_no_unit_count() {
    // Question I, 2026-09-26: «Разрешить бронь, наличие уточнит менеджер».
    let catalog = code(&source(CATALOG));
    let body = fn_body(&catalog, "pub fn book_block(");
    for field in ["units_available", "units_total"] {
        assert!(
            !body.contains(field),
            "book_block reads `{field}` again; the owner allowed booking whatever the seeded \
             count says (availability.t27 CUSTOMER_BOOKING_READS_THE_FILE = false):\n{body}"
        );
    }
    // D16: the function still decides something, so "reads no count" is not
    // true of an empty body. The three arms left are the offer, the rate and
    // the handler.
    for arm in [
        "return Some(BookBlock::NotOffered)",
        "return Some(BookBlock::NoPublishedRate)",
        "return Some(BookBlock::NotWired)",
    ] {
        assert!(
            body.contains(arm),
            "book_block lost `{arm}`; this check is blind"
        );
    }
    for arm in RETIRED_BOOK_ARMS {
        assert!(
            !has_identifier(&catalog, arm),
            "{CATALOG} declares BookBlock::{arm} again"
        );
    }
    let detail = code(&source(DETAIL));
    assert!(
        detail.contains("book_block(&bike, on_book.is_some())")
            && detail.contains("reason.reason_key()"),
        "{DETAIL} no longer renders the Book control's reason; this check is blind"
    );
}

#[test]
fn the_retired_book_reasons_are_gone_with_their_keys() {
    let i18n = code(&source(I18N));
    for (constant, literal, ru, en) in RETIRED_BOOK_REASONS {
        assert!(
            !has_identifier(&i18n, constant) && !i18n.contains(literal),
            "{I18N} declares {constant} again; locale_policy.t27 counts it as deleted"
        );
        for path in [I18N, CATALOG, DETAIL, BIKE_CARD] {
            let text = code(&source(path));
            assert!(
                !has_identifier(&text, constant),
                "{path} uses {constant} again"
            );
            for sentence in [ru, en] {
                assert!(
                    !text.contains(sentence),
                    "{path} carries the retired sentence {sentence:?} again"
                );
            }
        }
    }
    // D16: the three reasons that stay are still declared and still named by
    // their arms, so a parser that stopped matching would fail here first.
    for kept in [
        "T_BIKE_BOOK_BLOCKED_NOT_OFFERED",
        "T_BIKE_BOOK_BLOCKED_NO_RATE",
        "T_BIKE_BOOK_BLOCKED_NOT_WIRED",
    ] {
        assert!(
            i18n.contains(&format!("pub const {kept}: Key = ")),
            "{I18N} no longer declares {kept}; this check is blind"
        );
    }
}

#[test]
fn the_card_component_reads_no_unit_count_either() {
    // No screen mounts it, and it gated its add-to-cart on the same count and
    // printed it as «Свободно: N» / «Все в аренде» until 2026-09-26.
    let card = code(&source(BIKE_CARD));
    assert_eq!(
        card.matches("units_available").count(),
        1,
        "{BIKE_CARD} reads the seeded count again; its only mention is the field itself"
    );
    assert!(
        card.contains("pub units_available: u32,"),
        "{BIKE_CARD} no longer declares the field; this check is blind"
    );
    assert!(
        has_identifier(&card, MANAGER_CONFIRMS_KEY),
        "{BIKE_CARD} no longer prints the manager line"
    );
    for gone in ["Все в аренде", "All rented out", "Свободно:", "Available:"] {
        assert!(!card.contains(gone), "{BIKE_CARD} prints {gone:?} again");
    }
}

#[test]
fn create_order_does_not_refuse_on_the_unit_count() {
    // The server never refused on it, and must not start: a count of what is
    // at base today says nothing about a booking weeks out.
    let orders = code(&source(ORDERS_API));
    let body = fn_body(&orders, "async fn check_bike_lines(");
    let guard = "if listing.units_available < units.ceil() as i64 {";
    let at = body.find(guard).unwrap_or_else(|| {
        panic!("check_bike_lines no longer compares the count; re-read it:\n{body}")
    });
    let block = &body[at..];
    let block = &block[..block
        .find("\n        }\n")
        .expect("the shortfall block closes at its own indentation")];
    assert!(
        block.contains("tracing::info!") && !block.contains("return"),
        "check_bike_lines refuses on the unit count:\n{block}"
    );
    // No refusal variant names the count either.
    let refusals = fn_body(&orders, "pub(crate) enum BikeLineCheck {");
    for word in ["Unit", "Available", "Stock"] {
        assert!(
            !refusals.contains(word),
            "BikeLineCheck gained a count refusal ({word}):\n{refusals}"
        );
    }
}

#[test]
fn the_contract_records_the_owners_answer_to_question_i() {
    let spec = source(AVAILABILITY_SPEC);
    assert!(
        spec.contains("pub const NO_UNITS_BOOK_REASON_DECIDED_ON : str = \"2026-09-26\";"),
        "availability.t27 no longer dates the owner's answer to question I"
    );
    for (flag, want) in [
        ("NO_UNITS_BOOK_REASON_WAS_ANSWERED", true),
        ("NO_UNITS_BOOK_REASON_IS_SHOWN", false),
        ("UNKNOWN_AVAILABILITY_BOOK_REASON_IS_SHOWN", false),
        ("UNKNOWN_AVAILABILITY_REMOVAL_IS_THIS_FILES_READING", true),
        ("CUSTOMER_BOOKING_READS_THE_FILE", false),
        ("SERVER_REFUSES_ON_THE_COUNT", false),
        // What the file may still do: rule a redirect out, never confirm.
        ("FILE_MAY_RULE_OUT", true),
        ("FILE_MAY_CONFIRM", false),
    ] {
        assert_eq!(
            spec_bool(&spec, flag),
            want,
            "availability.t27 `{flag}` disagrees with the code this file reads"
        );
    }
}
