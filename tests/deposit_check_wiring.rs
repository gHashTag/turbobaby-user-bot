//! The rental deposit check, bound to the contract bool that says it ships.
//!
//! Contract: `specs/turbobaby/commerce.t27` (turbobaby/commerce), obligation
//! (1): `CLIENT_DEPOSIT_RECONCILIATION_IS_NOT_SHIPPED` went from `true` to
//! `false` on 2026-09-24 (T27 C3), when `check_bike_lines` in
//! `src/api/orders.rs` started holding every rental line of a family to the
//! family's published deposit through `rental_deposit_refusal`.
//!
//! WHY IT EXISTS (review, 2026-09-24): gate 3
//! (`scripts/verify_t27_against_source.py`) binds `CLIENT_DEPOSIT_REFUSALS`
//! to the ORDER in which `deposit_refusal` returns its refusals, and nothing
//! bound the one fact the `false` rests on: that the verdict is CALLED on the
//! order path. Deleting or commenting out the call left every gate green, the
//! unit tests of the pure verdict green, and the contract saying "shipped".
//! The gate cannot hold it either: its `compare()` refuses to bind a bool
//! against a count. So the relation is checked here, both ways: the call is
//! live inside `check_bike_lines` exactly when the contract says the check is
//! shipped.
//!
//! Only CODE counts: `//` line comments and `/* */` block comments are
//! stripped and string literals emptied before anything is looked for (the
//! gate's `NOT_IN_A_LINE_COMMENT` prefix sees only the first of these). The
//! negative controls at the end plant each way of switching the call off and
//! show that this guard sees it.
//!
//! This file imports nothing from the crate on purpose: it compiles against
//! any tree, so a tree where the call is gone fails here by CONTENT.

use std::fs;
use std::path::{Path, PathBuf};

const ORDERS_PATH: &str = "src/api/orders.rs";
const CONTRACT_PATH: &str = "specs/turbobaby/commerce.t27";
const NOT_SHIPPED: &str = "CLIENT_DEPOSIT_RECONCILIATION_IS_NOT_SHIPPED";
/// The fleet-side gate the check lives in. A column-zero private `async fn`.
const GATE_OPENER: &str = "\nasync fn check_bike_lines(";
const VERDICT_CALL: &str = "rental_deposit_refusal(";
const REFUSAL_RETURN: &str = "BikeLineCheck::DepositRefused(";

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

/// `text` reduced to code: every `//` line comment and every (nested)
/// `/* */` block comment removed, and every string literal emptied to `""`
/// (its line breaks kept), so a name that only appears in prose or in a log
/// message is not mistaken for a call. A `//` or `/*` inside a string is not a
/// comment; a char literal (`'"'`, `'\''`) is copied whole so its quote does
/// not open a string. A lifetime (`'a`) is left alone. Raw strings (`r"..."`)
/// are read as ordinary ones; none sits in the function this guard reads.
fn code_only(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut block_depth = 0usize;
    let mut in_string = false;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if block_depth > 0 {
            if c == '/' && next == Some('*') {
                block_depth += 1;
                i += 2;
            } else if c == '*' && next == Some('/') {
                block_depth -= 1;
                i += 2;
            } else {
                // Keep line breaks so a line count of the result still means
                // something to whoever reads a failure.
                if c == '\n' {
                    out.push('\n');
                }
                i += 1;
            }
            continue;
        }
        if in_string {
            if c == '\\' && next.is_some() {
                if next == Some('\n') {
                    out.push('\n');
                }
                i += 2;
                continue;
            }
            if c == '"' {
                out.push('"');
                in_string = false;
            } else if c == '\n' {
                out.push('\n');
            }
            i += 1;
            continue;
        }
        match (c, next) {
            ('/', Some('/')) => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            ('/', Some('*')) => {
                block_depth = 1;
                i += 2;
            }
            ('"', _) => {
                in_string = true;
                out.push(c);
                i += 1;
            }
            ('\'', Some('\\')) => {
                // An escaped char literal: copy up to its closing quote.
                let start = i;
                i += 2;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
                i = (i + 1).min(chars.len());
                out.extend(&chars[start..i]);
            }
            ('\'', Some(_)) if chars.get(i + 2) == Some(&'\'') => {
                out.extend(&chars[i..i + 3]);
                i += 3;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// The comment-free body of `check_bike_lines`: from its opener to the first
/// column-zero `}` after it (rustfmt runs on this tree, so a column-zero item
/// closes there). `None` when the gate is not in `source` at all.
fn gate_body(source: &str) -> Option<String> {
    let code = code_only(source);
    let start = code.find(GATE_OPENER)? + 1;
    let rest = &code[start..];
    let end = rest.find("\n}\n").map_or(rest.len(), |e| e + 3);
    Some(rest[..end].to_string())
}

/// Whether the order path calls the deposit verdict and returns its refusal:
/// both on live code lines inside `check_bike_lines`.
fn deposit_check_is_called(source: &str) -> bool {
    gate_body(source)
        .is_some_and(|body| body.contains(VERDICT_CALL) && body.contains(REFUSAL_RETURN))
}

/// The contract's `NOT_SHIPPED` bool, declared exactly once at column zero.
/// Located by NAME, never by line; `..._BEFORE_THE_DEPOSIT_CHECK` shares the
/// prefix and must not answer for it.
fn contract_says_not_shipped(contract: &str) -> bool {
    let values: Vec<&str> = contract
        .lines()
        .filter_map(|line| line.strip_prefix("pub const "))
        .filter_map(|decl| {
            let (name, rest) = decl.split_once(':')?;
            (name.trim() == NOT_SHIPPED).then_some(rest)
        })
        .filter_map(|rest| rest.split_once('=').map(|(_, value)| value.trim()))
        .collect();
    assert_eq!(
        values.len(),
        1,
        "{CONTRACT_PATH} declares `pub const {NOT_SHIPPED}` {} times at column zero; \
         this guard reads exactly one",
        values.len()
    );
    match values[0] {
        "true;" => true,
        "false;" => false,
        other => panic!("{CONTRACT_PATH}: {NOT_SHIPPED} is not a bool literal: {other:?}"),
    }
}

#[test]
fn the_contract_says_shipped_exactly_when_check_bike_lines_calls_the_verdict() {
    let orders = source(ORDERS_PATH);
    assert!(
        gate_body(&orders).is_some(),
        "{ORDERS_PATH}: `{}` is gone, so this guard reads nothing",
        GATE_OPENER.trim()
    );
    let called = deposit_check_is_called(&orders);
    let not_shipped = contract_says_not_shipped(&source(CONTRACT_PATH));
    assert_eq!(
        called,
        !not_shipped,
        "{CONTRACT_PATH} says {NOT_SHIPPED} = {not_shipped}, but check_bike_lines in \
         {ORDERS_PATH} {} `{VERDICT_CALL}..)` with its `{REFUSAL_RETURN}..)` on a code line. \
         Either the order path lost the deposit check and the contract must say so again, \
         or the contract is claiming a check the code does not make.",
        if called { "does call" } else { "does not call" }
    );
}

// ── Negative controls: each way of switching the call off, planted ─────────

/// A reduced `check_bike_lines` whose gate line is `call`.
fn planted(call: &str) -> String {
    format!(
        "fn before() {{}}\n\
         \n\
         async fn check_bike_lines(\n    items: &[OrderItem],\n) -> Result<BikeLineCheck, E> {{\n    \
         for (key, listing) in found {{\n        \
         {call}\n            \
         return Ok(BikeLineCheck::DepositRefused((*key).to_string(), why));\n        \
         }}\n    }}\n    Ok(BikeLineCheck::Ok)\n}}\n\
         \n\
         fn after() {{\n    let _ = rental_deposit_refusal(&[], \"k\", None);\n}}\n"
    )
}

#[test]
fn the_live_call_is_seen() {
    let live = "if let Some(why) = rental_deposit_refusal(items, key, listing.bike.deposit_thb) {";
    assert!(deposit_check_is_called(&planted(live)));
}

#[test]
fn a_call_commented_out_with_slashes_is_not_seen() {
    let off =
        "// if let Some(why) = rental_deposit_refusal(items, key, listing.bike.deposit_thb) {";
    assert!(!deposit_check_is_called(&planted(off)));
}

#[test]
fn a_call_inside_a_block_comment_is_not_seen() {
    // The gate's NOT_IN_A_LINE_COMMENT prefix would read this one as live.
    let off =
        "/* if let Some(why) = rental_deposit_refusal(items, key, listing.bike.deposit_thb) { */";
    assert!(!deposit_check_is_called(&planted(off)));
}

#[test]
fn a_call_outside_check_bike_lines_does_not_count() {
    // `after()` in the plant calls the verdict; check_bike_lines does not.
    let off = "if let Some(why) = None::<()> {";
    assert!(!deposit_check_is_called(&planted(off)));
}

#[test]
fn a_call_named_only_in_a_string_does_not_count() {
    // The name inside a log message is text, not a call. The plant's
    // `DepositRefused(` return is still there, which is why the call itself
    // is looked for and not the refusal alone.
    let off =
        "tracing::info!(\"rental_deposit_refusal( skipped\"); if let Some(why) = None::<()> {";
    assert!(!deposit_check_is_called(&planted(off)));
}

#[test]
fn a_gate_without_the_refusal_return_is_not_seen() {
    // The verdict called and its answer dropped is no check at all.
    let source = planted("let _ = rental_deposit_refusal(items, key, None);").replace(
        "return Ok(BikeLineCheck::DepositRefused((*key).to_string(), why));",
        "",
    );
    assert!(!deposit_check_is_called(&source));
}

#[test]
fn the_contract_bool_is_read_by_name_not_by_prefix() {
    let contract = "pub const CLIENT_DEPOSIT_RECONCILIATION_IS_NOT_SHIPPED_BEFORE_THE_DEPOSIT_CHECK : bool = true;\n\
                    pub const CLIENT_DEPOSIT_RECONCILIATION_IS_NOT_SHIPPED : bool = false;\n";
    assert!(!contract_says_not_shipped(contract));
}
