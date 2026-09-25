//! The owners' promo sales report does not print a name the dropped product
//! table supplied, and this file holds the live site to that.
//!
//! WHY IT EXISTS. Owner, 2026-09-25, answer 12 of the numbered list put to him
//! that day: «всё что касается канабиса нигде не должно быть» -- nothing
//! cannabis-related may appear anywhere. `promo_posts` keeps every row the
//! promoter ever wrote, and from 2026-08-19 until migration 083 it wrote about
//! the old shop's strains too: rows of the kind "strain", and bestseller rows
//! whose stored link opens a strain card. `crate::promo::report` printed each
//! row's stored `subject_name` -- a strain name -- to the owners in `/promo`
//! and on `GET /api/admin/promo-report`. The rule that stops it is
//! `report_kind_and_name` in `src/trios/promo.rs`, unit-tested beside it
//! without naming the retired word, because nothing outside a comment under
//! `src/` may (`tests/legacy_vocabulary_wiring.rs`).
//!
//! So this file does the two things those unit tests cannot:
//!
//! * it runs the rule on the retired word itself, and on a link to a strain
//!   card, which is what the stored rows actually hold;
//! * it reads `report` as text and holds that the live query selects the
//!   stored link and routes every row through the rule before a
//!   `PromoResult` is built. Inline the old direct read back into `report`
//!   and every unit test stays green while the strain names print again.
//!
//! The rule is recorded in `specs/turbobaby/legacy_retirement.t27`
//! (`PROMO_REPORT_NAME_*`), and the citations there are checked against the
//! code here.
//!
//! WHAT IT DOES NOT PROVE. Whether a stored draft may still be SENT: that is
//! the Publish button's refusal, `turbobaby/promo-broadcast`'s. Nothing about
//! production rows either: whether a published strain row exists today was
//! not read, and the rule does not depend on the answer.

// A panic is how a test reports failure. The restriction lints in Cargo.toml's
// [lints.clippy] exist for production code, as its own comment says.
#![allow(clippy::panic, clippy::expect_used)]

use std::fs;
use std::path::PathBuf;
use turbobaby_bot::trios::promo::{
    report_kind_and_name, report_may_print_name, REPORT_WITHHELD_NAME,
};

/// The report's query and the rows it builds.
const PROMO: &str = "src/promo.rs";
/// The rule and the label.
const RULES: &str = "src/trios/promo.rs";
/// The contract that records the rule.
const SPEC: &str = "specs/turbobaby/legacy_retirement.t27";
/// This file, as the contract names it.
const THIS_FILE: &str = "tests/promo_report_names_wiring.rs";

/// The retired kind's word, as the promoter wrote it into `promo_posts.kind`
/// before 083 (the variant's `kind()` arm, gone since 2026-09-16).
const RETIRED_KIND: &str = "strain";

/// A link the promoter stored for a strain before 083: the strain card's
/// prefix, a catalogue UUID and the campaign source.
const STRAIN_BESTSELLER_LINK: &str =
    "p_strain_3f2c8a1e-6b7d-4c19-9e0a-5d4b2f7c8e91__promo_bestseller";

/// The same row about a set, which a live bestseller opens.
const SET_BESTSELLER_LINK: &str = "p_set_3f2c8a1e-6b7d-4c19-9e0a-5d4b2f7c8e91__promo_bestseller";

fn source(rel: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|e| panic!("{rel} is readable: {e}"))
        .replace("\r\n", "\n")
}

/// A line with its `//` comment removed, so prose describing a call is never
/// counted as the call. Naive by intent, as in `tests/promo_copy_wiring.rs`:
/// it does not track string literals, and none of the needles below contains
/// `//`.
fn code_of(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// The code of a top-level item: from the line after the one starting with
/// `header` to the first `}` in the first column. Panics when the header is
/// gone -- a renamed `report` is a wiring change, and this file must not pass
/// by finding nothing.
fn body_of(rel: &str, header: &str) -> Vec<String> {
    let text = source(rel);
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with(header))
        .unwrap_or_else(|| panic!("`{header}` is no longer a top-level item of {rel}"));
    let len = lines
        .iter()
        .skip(start + 1)
        .position(|l| l.trim_end() == "}")
        .unwrap_or_else(|| panic!("`{header}` never closes at column 0 in {rel}"));
    lines[start + 1..start + 1 + len]
        .iter()
        .map(|l| code_of(l).to_string())
        .collect()
}

/// How many lines of `body` hold `needle` in their code.
fn count(body: &[String], needle: &str) -> usize {
    body.iter().filter(|l| l.contains(needle)).count()
}

/// The first line of `body` holding `needle`, by index.
fn first(body: &[String], needle: &str) -> usize {
    body.iter()
        .position(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("`{needle}` is not in report's code"))
}

/// Every `.rs` file under `src/`, label and code with comments removed.
fn rust_sources() -> Vec<(String, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![root.clone()];
    let mut out = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("src/ is readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let label = path
                    .strip_prefix(root.parent().expect("src has a parent"))
                    .unwrap_or(&path)
                    .display()
                    .to_string()
                    .replace('\\', "/");
                let code: String = fs::read_to_string(&path)
                    .expect("readable")
                    .lines()
                    .map(|l| format!("{}\n", code_of(l)))
                    .collect();
                out.push((label, code));
            }
        }
    }
    assert!(
        out.len() >= 100,
        "the walk found {} files -- it is not reaching src/",
        out.len()
    );
    out
}

/// The value of `pub const NAME : str = "value";` in the contract.
fn spec_str(name: &str) -> String {
    let spec = source(SPEC);
    let head = format!("pub const {name} : str =");
    let line = spec
        .lines()
        .find(|l| l.starts_with(&head))
        .unwrap_or_else(|| panic!("`{name}` is no longer declared in {SPEC}"));
    let open = line.find('"').expect("a str declaration is quoted");
    let close = line.rfind('"').expect("a str declaration is quoted");
    assert!(close > open, "`{name}` in {SPEC} has no value");
    line[open + 1..close].to_string()
}

/// The value of `pub const NAME : u8 = N;` in the contract.
fn spec_u8(name: &str) -> usize {
    let spec = source(SPEC);
    let head = format!("pub const {name} : u8 =");
    let line = spec
        .lines()
        .find(|l| l.starts_with(&head))
        .unwrap_or_else(|| panic!("`{name}` is no longer declared in {SPEC}"));
    line[head.len()..]
        .trim()
        .trim_end_matches(';')
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("`{name}` in {SPEC} is not a number: {e}"))
}

/// The retired kind's rows print neither their name nor their word, with a
/// link or without one -- even a link a live set row would pass with.
#[test]
fn a_row_of_the_retired_kind_prints_neither_its_name_nor_its_word() {
    for link in [
        None,
        Some(SET_BESTSELLER_LINK),
        Some(STRAIN_BESTSELLER_LINK),
    ] {
        assert!(
            !report_may_print_name(RETIRED_KIND, link),
            "a {RETIRED_KIND} row printed its name under {link:?}"
        );
        assert_eq!(
            report_kind_and_name(RETIRED_KIND.into(), link, "DA FUNK".into()),
            (String::new(), REPORT_WITHHELD_NAME.to_string()),
            "under {link:?}"
        );
    }
}

/// A bestseller about a strain keeps its kind -- the word is neutral -- and
/// loses its name; the same row about a set keeps both.
#[test]
fn a_bestseller_whose_link_opens_a_strain_card_is_withheld() {
    assert!(!report_may_print_name(
        "bestseller",
        Some(STRAIN_BESTSELLER_LINK)
    ));
    assert_eq!(
        report_kind_and_name(
            "bestseller".into(),
            Some(STRAIN_BESTSELLER_LINK),
            "DA FUNK".into()
        ),
        ("bestseller".to_string(), REPORT_WITHHELD_NAME.to_string())
    );
    assert!(report_may_print_name(
        "bestseller",
        Some(SET_BESTSELLER_LINK)
    ));
    assert_eq!(
        report_kind_and_name(
            "bestseller".into(),
            Some(SET_BESTSELLER_LINK),
            "Party Pack".into()
        ),
        ("bestseller".to_string(), "Party Pack".to_string())
    );
}

/// The label is what an owner reads in place of the name: present, and none
/// of the retired words. The vocabulary is the server guard's core, not all
/// of it; the label is two plain Russian words.
#[test]
fn the_label_is_plain() {
    assert!(!REPORT_WITHHELD_NAME.trim().is_empty());
    let lower = REPORT_WITHHELD_NAME.to_lowercase();
    for word in [
        "strain",
        "сорт",
        "cannabis",
        "каннабис",
        "weed",
        "woody",
        "вуди",
        "thc",
    ] {
        assert!(!lower.contains(word), "the label says {word:?}");
    }
}

/// Calibration: the reader drops comments, so `report`'s own doc and inline
/// prose cannot stand in for the code.
#[test]
fn the_reader_sees_code_and_not_the_prose_about_it() {
    assert_eq!(code_of("    // report_kind_and_name( is called"), "    ");
    assert_eq!(code_of("let x = 1; // link_payload"), "let x = 1; ");
    let body = body_of(PROMO, "pub async fn report(");
    assert!(
        body.iter().all(|l| !l.contains("REPORT_WITHHELD_NAME")),
        "report's code names the label itself; only its doc comment should"
    );
}

/// The live query selects the stored link -- in the published set, in the
/// outer SELECT and in its GROUP BY -- and reads it off every row.
#[test]
fn the_report_query_reads_the_stored_link() {
    let body = body_of(PROMO, "pub async fn report(");
    assert_eq!(
        count(
            &body,
            "SELECT dedup_key, kind, subject_id, subject_name, link_payload, published_at"
        ),
        1,
        "the published set no longer carries the link"
    );
    assert_eq!(
        count(
            &body,
            "SELECT p.dedup_key, p.kind, p.subject_name, p.link_payload,"
        ),
        1,
        "the outer SELECT no longer returns the link"
    );
    assert_eq!(
        count(
            &body,
            "GROUP BY p.dedup_key, p.kind, p.subject_name, p.link_payload,"
        ),
        1,
        "the GROUP BY no longer carries the link"
    );
    assert_eq!(count(&body, "try_get(\"\", \"link_payload\")"), 1);
}

/// Every row goes through the rule, once, before the `PromoResult` is built,
/// and the stored kind and name reach the result only through it.
#[test]
fn every_row_is_routed_through_the_rule_before_a_result_is_built() {
    let body = body_of(PROMO, "pub async fn report(");
    assert_eq!(
        count(&body, "crate::trios::promo::report_kind_and_name("),
        1,
        "report must ask the rule exactly once per row"
    );
    let asked = first(&body, "report_kind_and_name(");
    let built = first(&body, "PromoResult {");
    assert!(
        asked < built,
        "the result is built before the rule is asked"
    );
    assert!(
        first(&body, "try_get(\"\", \"kind\")") > asked
            && first(&body, "try_get(\"\", \"subject_name\")") > asked
            && first(&body, "link.as_deref()") > asked,
        "the stored kind, name and link are no longer the rule's arguments"
    );
    // The result takes the rule's answer, not the row.
    assert_eq!(
        count(&body, "subject_name: r.try_get"),
        0,
        "the stored name is read straight into the result"
    );
    assert_eq!(
        count(&body, "kind: r.try_get"),
        0,
        "the stored kind is read straight into the result"
    );
    assert_eq!(count(&body, "                subject_name,"), 1);
    assert_eq!(count(&body, "                kind,"), 1);
}

/// Nowhere else: the only code that builds a `PromoResult` is `report`, the
/// stored name is read in `src/promo.rs` only, and the readers the contract
/// counts are the ones that call `report`.
#[test]
fn the_report_is_the_only_road_from_the_stored_name_to_a_reader() {
    let sources = rust_sources();
    let built: Vec<&str> = sources
        .iter()
        .filter(|(_, code)| {
            code.matches("PromoResult {").count() > code.matches("struct PromoResult {").count()
        })
        .map(|(label, _)| label.as_str())
        .collect();
    assert_eq!(built, vec![PROMO], "a PromoResult is built outside report");
    let promo_code = &sources
        .iter()
        .find(|(l, _)| l == PROMO)
        .expect("src/promo.rs")
        .1;
    assert_eq!(
        promo_code.matches("PromoResult {").count()
            - promo_code.matches("struct PromoResult {").count(),
        1,
        "src/promo.rs builds a PromoResult twice"
    );

    let readers_of_the_name: Vec<&str> = sources
        .iter()
        .filter(|(_, code)| code.contains("subject_name"))
        .map(|(label, _)| label.as_str())
        .collect();
    assert_eq!(
        readers_of_the_name,
        vec![PROMO],
        "the stored name is read outside src/promo.rs"
    );

    let callers: usize = sources
        .iter()
        .map(|(_, code)| code.matches("crate::promo::report(").count())
        .sum();
    assert_eq!(
        callers,
        spec_u8("PROMO_REPORT_READER_COUNT"),
        "the report has another reader"
    );

    let defined: usize = sources
        .iter()
        .map(|(_, code)| code.matches("pub fn report_kind_and_name(").count())
        .sum();
    assert_eq!(defined, 1, "the rule is defined more than once");
}

/// The contract's citations land on the code.
#[test]
fn the_contract_cites_the_live_names() {
    assert_eq!(
        spec_str("PROMO_REPORT_NAME_SITE"),
        format!("{PROMO} report")
    );
    assert!(source(PROMO)
        .lines()
        .any(|l| l.starts_with("pub async fn report(")));

    assert_eq!(
        spec_str("PROMO_REPORT_NAME_DECISION"),
        format!("{RULES} report_kind_and_name")
    );
    assert!(source(RULES)
        .lines()
        .any(|l| l.starts_with("pub fn report_kind_and_name(")));

    assert_eq!(
        spec_str("PROMO_REPORT_NAME_LABEL"),
        format!("{RULES} REPORT_WITHHELD_NAME")
    );
    assert!(source(RULES)
        .lines()
        .any(|l| l.starts_with("pub const REPORT_WITHHELD_NAME: &str =")));

    assert_eq!(spec_str("PROMO_REPORT_NAME_WIRING_TEST"), THIS_FILE);
    assert!(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(THIS_FILE)
        .is_file());
    assert_eq!(spec_str("PROMO_REPORT_NAME_DECIDED_AT"), "2026-09-25");
}
