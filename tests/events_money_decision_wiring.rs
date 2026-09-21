//! The events module makes two decisions about money, and each is made in one
//! place. This file reads `src/` as text and proves the HANDLERS still ask.
//!
//! WHY IT EXISTS. The 2026-09-21 wave extracted both decisions into pure
//! functions — `delete_verdict` (may this delete destroy paid bookings?) and
//! `next_promotable` (who does the freed seat reach?) — and unit-tested them.
//! An adversarial reviewer then reverted the WIRING, left the two functions
//! untouched, and six tests stayed green:
//! `a_delete_refuses_while_the_event_holds_paid_bookings`,
//! `a_delete_with_nothing_paid_still_goes_through`,
//! `a_freed_seat_passes_the_head_who_cannot_pay`,
//! `a_skipped_customer_keeps_his_place_in_the_line`,
//! `the_scan_resumes_after_a_candidate_falls_through`,
//! `a_line_of_people_who_cannot_pay_promotes_nobody`. They test the rule; not
//! one of them says the handler obeys it. A grep of `tests/` for
//! `delete_event` on 2026-09-21 returned zero files.
//!
//! The handlers cannot be called from a unit test — both need a database — so
//! this file does what `tests/cart_kind_wiring.rs` does for the cart's wire
//! kinds: it reads the source and pins WHERE each decision is taken, and HOW
//! MANY places take it. The count is half the point. A rule with two
//! implementations is the drift DECISIONS.md D15 exists to prevent, and a
//! second `DELETE FROM events` or a second promoter would be exactly that —
//! green against both pure functions and destroying paid rows anyway.
//!
//! What this file does NOT prove: that the SQL is right, that the transaction
//! is held, or that the numbers reaching `delete_verdict` are the ones the
//! database holds. It proves the call is made, from the one place, and nowhere
//! else. The rest is the integration tests' subject.

use std::fs;
use std::path::PathBuf;

/// The file both decisions live in.
const EVENTS: &str = "src/api/events.rs";
/// The one reader of the delete's refusal codes, shared by the admin screen.
const ERROR_READER: &str = "src/trios/api_errors.rs";
/// The screen that presses Delete.
const ADMIN_SCREEN: &str = "src/ui/screens/admin_screen.rs";
/// The contract that owns both decisions.
const SPEC: &str = "specs/turbobaby/events_booking.t27";

/// Every `.rs` file under `src/`, label and text. A walk and not a fixed list,
/// for the reason `cart_kind_wiring.rs` gives: the invariant is *nowhere
/// else*, and a fixed list cannot say that about a file added tomorrow.
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
                // One separator whatever the host: every expectation below is
                // written with forward slashes, and a backslash makes them all
                // miss and reports a drift that is not there.
                let label = path
                    .strip_prefix(root.parent().expect("src has a parent"))
                    .unwrap_or(&path)
                    .display()
                    .to_string()
                    .replace('\\', "/");
                out.push((label, fs::read_to_string(&path).expect("readable")));
            }
        }
    }
    // D16: a walk that finds too little must be red, not quietly clean.
    // Measured 2026-09-21: 181 files.
    assert!(
        out.len() >= 100,
        "the walk found {} files — it is not reaching src/ (181 on 2026-09-21)",
        out.len()
    );
    out
}

/// A line with its `//` comment removed, so prose *describing* a call is never
/// counted as the call. Naive by intent, and the same naivety
/// `cart_kind_wiring.rs` accepts: it does not track string literals, and no
/// line this file judges puts `//` inside one.
fn code_of(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// The half of a file that SHIPS: every line outside a `#[cfg(test)]` item,
/// with its 1-based number and its comments stripped.
///
/// The block is skipped by INDENTATION and not by cutting the file at the
/// first `#[cfg(test)]`, because four files under `src/` carry shipped code
/// after their first one — measured 2026-09-21: `src/bot/mod.rs` (100 lines
/// after :39), `src/db/strains.rs` (75 after :54), `src/trios/core.rs` (89
/// after :82), `src/trios/calendar.rs` (22 after :97). A cut-at-first-match
/// reader goes blind on 286 lines and then reports the blindness as "nowhere
/// else", which is the failure shape this whole file exists to catch.
///
/// `cargo fmt` is what makes the indentation anchor sound: the closing brace
/// of an item sits at the attribute's own column, and `cargo fmt --check` is a
/// gate. `test_module_is_skipped_and_nothing_after_it_is` calibrates it both
/// ways rather than trusting it.
fn shipped_lines(source: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut skip_indent: Option<usize> = None;
    for (i, raw) in source.lines().enumerate() {
        let code = code_of(raw);
        match skip_indent {
            None => {
                if code.trim_start().starts_with("#[cfg(test)]") {
                    skip_indent = Some(code.len() - code.trim_start().len());
                    continue;
                }
                out.push((i + 1, code.to_string()));
            }
            Some(indent) => {
                if code.trim_end() == format!("{}}}", " ".repeat(indent)) {
                    skip_indent = None;
                }
            }
        }
    }
    out
}

/// Every shipped line under `src/` that contains `needle`, as
/// `(file, line, code)`. `exclude` drops the declaration itself: the text of
/// `fn delete_verdict(` contains `delete_verdict(`, and a function is not its
/// own call site.
fn shipped_sites(needle: &str, exclude: Option<&str>) -> Vec<(String, usize, String)> {
    let mut hits = Vec::new();
    let mut shipped_total = 0usize;
    for (label, source) in rust_sources() {
        for (n, code) in shipped_lines(&source) {
            shipped_total += 1;
            if code.contains(needle) && !exclude.is_some_and(|e| code.contains(e)) {
                hits.push((label.clone(), n, code.trim().to_string()));
            }
        }
    }
    // D16 again, on the reader rather than the walk: a stripper that ate the
    // whole corpus would report "one call site" for everything, including for
    // a needle that is not there at all. Measured 2026-09-21: 61903 shipped
    // lines.
    assert!(
        shipped_total >= 30_000,
        "the reader kept only {shipped_total} shipped lines of src/ — it is \
         dropping the corpus, not the tests (61903 on 2026-09-21)"
    );
    hits.sort();
    hits
}

/// The line range of a top-level item, 1-based and inclusive: from the line
/// that starts with `header` to the first `}` in the first column after it.
///
/// Panics when the header is gone. That is the point — a renamed handler is a
/// wiring change, and this file must not pass by finding nothing.
fn item_range(source: &str, header: &str) -> (usize, usize) {
    let lines: Vec<&str> = source.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with(header))
        .unwrap_or_else(|| panic!("`{header}` is no longer a top-level item of {EVENTS}"));
    let end = lines
        .iter()
        .skip(start + 1)
        .position(|l| l.trim_end() == "}")
        .unwrap_or_else(|| panic!("`{header}` never closes at column 0 in {EVENTS}"));
    (start + 1, start + 1 + end + 1)
}

/// The value of a `pub const NAME : str = "value";` declaration in a contract.
/// Panics when the declaration is gone: a contract that stopped saying this is
/// a contract nobody is holding the code to.
fn spec_str(spec: &str, name: &str) -> String {
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

fn events_source() -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(EVENTS))
        .expect("src/api/events.rs is readable")
}

/// Calibration, both directions. A reader that silently kept the test module
/// would count the unit tests as call sites and always find "enough"; a reader
/// that stopped at the first `#[cfg(test)]` would go blind on four files and
/// call it "nowhere else".
#[test]
fn test_module_is_skipped_and_nothing_after_it_is() {
    let events = events_source();
    let shipped: Vec<String> = shipped_lines(&events).into_iter().map(|(_, c)| c).collect();
    assert!(
        !shipped
            .iter()
            .any(|c| c.contains("fn a_delete_refuses_while_the_event_holds_paid_bookings")),
        "the reader kept {EVENTS}'s test module — every count below is then \
         counting its own unit tests"
    );
    assert!(
        shipped.iter().any(|c| c.contains("fn delete_verdict(")),
        "the reader dropped the shipped half of {EVENTS}"
    );

    // The blind-spot half: this function is declared AFTER that file's first
    // test module (src/trios/calendar.rs:97, the function at :325).
    let calendar =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/trios/calendar.rs"))
            .expect("src/trios/calendar.rs is readable");
    assert!(
        shipped_lines(&calendar)
            .iter()
            .any(|(_, c)| c.contains("pub fn time_range(")),
        "the reader stopped at the first #[cfg(test)] and went blind on the \
         rest of src/trios/calendar.rs"
    );
}

/// D15: one implementation of each decision. Two copies of `delete_verdict`
/// would pass every unit test in `src/api/events.rs` while one of them decided
/// the live deletes.
#[test]
fn each_money_decision_has_exactly_one_implementation() {
    let verdict = shipped_sites("fn delete_verdict(", None);
    assert_eq!(
        verdict.len(),
        1,
        "delete_verdict is declared in {} places: {:?}",
        verdict.len(),
        verdict
    );
    assert_eq!(verdict[0].0, EVENTS);

    let scan = shipped_sites("fn next_promotable(", None);
    assert_eq!(
        scan.len(),
        1,
        "next_promotable is declared in {} places: {:?}",
        scan.len(),
        scan
    );
    assert_eq!(scan[0].0, EVENTS);
}

/// The delete handler asks the rule. Reverting that call is what left six
/// green tests behind a handler that destroyed paid bookings again.
#[test]
fn the_delete_handler_is_the_one_caller_of_delete_verdict() {
    let sites = shipped_sites("delete_verdict(", Some("fn delete_verdict("));
    assert_eq!(
        sites.len(),
        1,
        "delete_verdict is called from {} places, expected exactly 1 (the \
         handler). 0 means the handler stopped asking and the cascade is \
         unguarded again; 2 means a second place decides who may destroy paid \
         bookings. Sites: {:?}",
        sites.len(),
        sites
    );
    let (file, line, _) = &sites[0];
    assert_eq!(file, EVENTS);
    let (from, to) = item_range(&events_source(), "async fn delete_event(");
    assert!(
        from < *line && *line < to,
        "the only call to delete_verdict is at {file}:{line}, outside \
         delete_event ({EVENTS}:{from}-{to}) — the handler no longer takes the \
         verdict it was given"
    );
}

/// The escape hatch is only an escape hatch while the handler reads it. Drop
/// the parameter and the refusal becomes unconditional again: an event that
/// already happened, holding paid bookings, could never be deleted by anyone.
#[test]
fn the_delete_handler_reads_the_acknowledgement() {
    let sites = shipped_sites(
        "ABANDON_PAID_BOOKINGS_PARAM",
        Some("const ABANDON_PAID_BOOKINGS_PARAM"),
    );
    assert_eq!(
        sites.len(),
        1,
        "the acknowledgement parameter is read in {} places, expected exactly \
         1. 0 means the refusal has no door again; 2 means a second place \
         decides what counts as consent. Sites: {:?}",
        sites.len(),
        sites
    );
    let (file, line, _) = &sites[0];
    assert_eq!(file, EVENTS);
    let (from, to) = item_range(&events_source(), "async fn delete_event(");
    assert!(
        from < *line && *line < to,
        "the acknowledgement is read at {file}:{line}, outside delete_event \
         ({EVENTS}:{from}-{to})"
    );
}

/// The cascade has one door. `migrations/043_events_booking.sql:29` declares
/// `ON DELETE CASCADE`, so a second `DELETE FROM events` anywhere in `src/`
/// destroys paid bookings without ever meeting the verdict — and every unit
/// test in this repository stays green while it does.
#[test]
fn one_statement_deletes_an_event_and_the_verdict_stands_in_front_of_it() {
    let sites = shipped_sites("DELETE FROM events", None);
    assert_eq!(
        sites.len(),
        1,
        "{} statements delete an event, expected exactly 1: {:?}",
        sites.len(),
        sites
    );
    let (file, line, _) = &sites[0];
    assert_eq!(file, EVENTS);
    let (from, to) = item_range(&events_source(), "async fn delete_event(");
    assert!(
        from < *line && *line < to,
        "an event is deleted at {file}:{line}, outside delete_event \
         ({EVENTS}:{from}-{to}) — that road never meets delete_verdict"
    );
}

/// The promotion loop asks the scan, and nothing else promotes. A second
/// `UPDATE ... SET status = 'confirmed'` on a waitlisted row is a second
/// answer to "who gets the freed seat", taken without the balance rule and
/// without the head-of-line skip.
#[test]
fn the_promotion_loop_is_the_one_caller_of_next_promotable() {
    let source = events_source();
    let (from, to) = item_range(&source, "async fn cancel_booking_and_promote(");

    let sites = shipped_sites("next_promotable(", Some("fn next_promotable("));
    assert_eq!(
        sites.len(),
        1,
        "next_promotable is called from {} places, expected exactly 1 (the \
         promotion loop). 0 means the loop stopped asking and the seat stops \
         at the head again; 2 means a second scan. Sites: {:?}",
        sites.len(),
        sites
    );
    let (file, line, _) = &sites[0];
    assert_eq!(file, EVENTS);
    assert!(
        from < *line && *line < to,
        "the only call to next_promotable is at {file}:{line}, outside \
         cancel_booking_and_promote ({EVENTS}:{from}-{to})"
    );

    let promotes = shipped_sites("UPDATE event_bookings SET status = 'confirmed'", None);
    assert_eq!(
        promotes.len(),
        1,
        "{} statements promote a booking to confirmed, expected exactly 1: {:?}",
        promotes.len(),
        promotes
    );
    let (pfile, pline, _) = &promotes[0];
    assert_eq!(pfile, EVENTS);
    assert!(
        from < *pline && *pline < to,
        "a booking is promoted at {pfile}:{pline}, outside \
         cancel_booking_and_promote ({EVENTS}:{from}-{to}) — that promotion \
         never asks next_promotable who the seat belongs to"
    );
}

/// One writer, one reader. The refusal is a 409 with a machine word in it, and
/// a word only the server says is a word no screen can act on: the admin's
/// Delete button showed one toast for "three people paid" and for "the network
/// is down". The code now travels to exactly one reader, and that reader is
/// tested (`src/trios/api_errors.rs`, `admin_event_delete_failure`).
#[test]
fn the_refusal_code_has_one_writer_and_one_reader() {
    let homes: Vec<String> = shipped_sites("paid_bookings_exist", None)
        .into_iter()
        .map(|(f, _, _)| f)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(
        homes,
        vec![EVENTS.to_string(), ERROR_READER.to_string()],
        "the delete's refusal code lives in {homes:?}. Expected the writer \
         ({EVENTS}) and the one reader ({ERROR_READER}): a screen that parses \
         it on its own is a second place deciding what the refusal means, and \
         no test compiles the screens (src/lib.rs gates `pub mod ui;` on \
         wasm32)"
    );

    let screen = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(ADMIN_SCREEN))
        .expect("src/ui/screens/admin_screen.rs is readable");
    assert!(
        screen.contains("admin_event_delete_failure"),
        "{ADMIN_SCREEN} no longer routes a failed delete through \
         admin_event_delete_failure — the admin is back to one toast for a \
         refusal that names three paid bookings and for a dropped connection"
    );
}

/// The contract's wire words are the words the server sends.
///
/// `specs/turbobaby/events_booking.t27` declares `DELETE_REFUSAL_CODE`,
/// `DELETE_STALE_ACK_CODE` and `DELETE_HATCH_PARAM`, and a contract that names
/// a wire word is only worth reading while that word is the one on the wire.
/// `scripts/verify_t27_against_source.py` owns the general contract-to-source
/// binder and its table was out of this change's reach, so the three strings
/// are bound here, beside the rest of this decision's wiring. Renaming the
/// parameter or either code in one of the two files turns this red.
#[test]
fn the_contracts_wire_words_are_the_ones_the_server_sends() {
    let spec = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SPEC))
        .expect("specs/turbobaby/events_booking.t27 is readable");
    let events = events_source();
    let shipped: Vec<String> = shipped_lines(&events).into_iter().map(|(_, c)| c).collect();
    let reader = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(ERROR_READER))
        .expect("src/trios/api_errors.rs is readable");
    let reader_shipped: Vec<String> = shipped_lines(&reader).into_iter().map(|(_, c)| c).collect();

    let param = spec_str(&spec, "DELETE_HATCH_PARAM");
    let declared = format!("const ABANDON_PAID_BOOKINGS_PARAM: &str = \"{param}\";");
    assert!(
        shipped.iter().any(|c| c.contains(&declared)),
        "{SPEC} calls the escape hatch {param:?}; {EVENTS} declares no parameter \
         of that name, so the contract names a door that is not on the wire"
    );

    for constant in ["DELETE_REFUSAL_CODE", "DELETE_STALE_ACK_CODE"] {
        let code = spec_str(&spec, constant);
        let sent = format!("\"error\": \"{code}\"");
        assert!(
            shipped.iter().any(|c| c.contains(&sent)),
            "{SPEC} declares {constant} = {code:?} and {EVENTS} answers with no \
             such code"
        );
        let read = format!("\"{code}\" =>");
        assert!(
            reader_shipped.iter().any(|c| c.contains(&read)),
            "{ERROR_READER} does not read {code:?} — the server would send a \
             refusal the admin screen cannot tell from any other failure"
        );
    }
}
