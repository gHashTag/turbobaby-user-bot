//! The drain loop is never executed by a test, so this file reads it as text.
//!
//! WHY IT EXISTS. `src/notification_queue.rs` holds its retry rule in three
//! small functions -- `attempts_spent`, `attempts_after_pass`,
//! `budget_is_spent` -- and its unit tests walk a row over ticks by calling
//! them. That walk is a MODEL. `process_batch` needs a live Postgres and a
//! live Telegram, so nothing can execute the real loop, and an adversarial
//! reviewer on 2026-09-21 made exactly that point about
//! `passes_until_the_queue_lets_go` (the model's earlier name): it can agree
//! with a loop that no longer matches it. Rewrite the loop's arithmetic inline
//! and every one of those tests stays green while the shipped queue does
//! something else.
//!
//! So this file pins the WIRING, the way `tests/cart_kind_wiring.rs` and
//! `tests/events_money_decision_wiring.rs` pin theirs: the loop still asks the
//! same three functions, the scan and the give-up still read one declaration,
//! and -- the finding this wave closed -- a counter the database will not
//! write is no longer able to skip the send.
//!
//! WHAT IT DOES NOT PROVE. Not that the SQL is right, not that the retry
//! bound is the right number, not that a message reaches anybody. It proves
//! the decisions are taken in one place and that the loop takes them there.
//! The numbers those decisions produce are the unit tests' subject and the
//! contract's.

use std::fs;
use std::path::PathBuf;

/// The drain.
const DRAIN: &str = "src/notification_queue.rs";
/// The queries it drains through.
const QUERIES: &str = "src/db/notifications.rs";
/// The contract that owns the queue.
const SPEC: &str = "specs/turbobaby/notification_queue.t27";

fn source(rel: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|e| panic!("{rel} is readable: {e}"))
}

/// A line with its `//` comment removed, so prose DESCRIBING a call is never
/// counted as the call. Naive by intent, and the same naivety
/// `tests/cart_kind_wiring.rs` accepts: it does not track string literals.
///
/// It matters more here than anywhere else in this repository. The loop's
/// comments name `increment_attempts`, `continue`, `MAX_ATTEMPTS` and the
/// whole history of the defect; a reader that counted them would find every
/// call twice and the shape of the OLD code in the prose describing it.
fn code_of(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// The body of a top-level item, as `(line number, code)` pairs with comments
/// stripped: from the line after the one starting with `header` to the first
/// `}` in the first column after it.
///
/// Panics when the header is gone, which is the point -- a renamed or deleted
/// drain is a wiring change, and this file must not pass by finding nothing.
fn body_of(source: &str, header: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = source.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with(header))
        .unwrap_or_else(|| panic!("`{header}` is no longer a top-level item of {DRAIN}"));
    let len = lines
        .iter()
        .skip(start + 1)
        .position(|l| l.trim_end() == "}")
        .unwrap_or_else(|| panic!("`{header}` never closes at column 0 in {DRAIN}"));
    lines[start + 1..start + 1 + len]
        .iter()
        .enumerate()
        .map(|(i, l)| (start + 2 + i, code_of(l).to_string()))
        .collect()
}

fn drain_body() -> Vec<(usize, String)> {
    body_of(&source(DRAIN), "async fn process_batch(")
}

/// Every line of the body whose CODE contains `needle`.
fn sites(body: &[(usize, String)], needle: &str) -> Vec<(usize, String)> {
    body.iter()
        .filter(|(_, code)| code.contains(needle))
        .map(|(n, code)| (*n, code.trim().to_string()))
        .collect()
}

/// Calibration, both ways. A reader that swallowed the whole file would find
/// every needle below and say "wired"; a reader that stopped at the signature
/// would find none and panic on the first one, which at least fails loudly --
/// but a reader that kept the COMMENTS would find the old code's shape in the
/// prose that describes it and report a defect that is not there.
#[test]
fn the_reader_sees_the_loop_and_not_the_prose_about_it() {
    let body = drain_body();
    assert!(
        body.len() > 100,
        "the reader kept {} lines of process_batch -- it is not reading the loop",
        body.len()
    );
    assert!(
        sites(&body, "for row in rows").len() == 1,
        "the reader did not find the per-row loop in process_batch"
    );
    // The stripper, calibrated on a name the loop MENTIONS and never calls.
    // `POLL_INTERVAL_SECS` is declared at the top of the file and used by the
    // spawner; inside `process_batch` it appears only in the comment telling
    // the story of the unbounded resend. A reader that kept comments would
    // find it here, and would equally find `increment_attempts`, `continue`
    // and the whole shape of the OLD code in the prose describing it -- which
    // is the direction of error that hides a real defect.
    let raw = source(DRAIN);
    assert!(
        raw.contains("POLL_INTERVAL_SECS seconds"),
        "the loop no longer tells the story the counts below are calibrated on"
    );
    assert!(
        sites(&body, "POLL_INTERVAL_SECS").is_empty(),
        "the reader kept process_batch's comments, so every count in this file \
         is the prose's and not the code's"
    );
    // And it must not have eaten the test module, whose model mentions every
    // rule name this file counts.
    assert!(
        sites(&body, "fn walk(").is_empty(),
        "the reader ran past the end of process_batch into the rest of the file"
    );
}

/// The loop decides with the same three functions the unit tests drive.
///
/// This is the drift the reviewer named. Inline `row.attempts + 1` or
/// `>= MAX_ATTEMPTS` here and the model in `src/notification_queue.rs` keeps
/// agreeing with itself while the shipped loop stops agreeing with the model.
#[test]
fn the_drain_decides_with_the_rules_its_tests_drive() {
    let body = drain_body();
    for rule in [
        "attempts_spent(",
        "attempts_after_pass(",
        "budget_is_spent(",
    ] {
        assert!(
            !sites(&body, rule).is_empty(),
            "process_batch no longer calls `{rule}` -- the unit tests are now \
             walking a rule the shipped loop does not use"
        );
    }
    // `attempts_spent` is the one that reads BOTH halves of the count, so the
    // loop must open every pass with it. One site: two would mean two readings
    // of how much a row has already cost.
    let spent = sites(&body, "attempts_spent(");
    assert_eq!(
        spent.len(),
        1,
        "process_batch reads how much a row has spent in {} places: {:?}",
        spent.len(),
        spent
    );
}

/// The scan's ceiling and the give-up are one declaration, not two copies.
///
/// `src/db/notifications.rs` used to carry its own literal `3`. The day one of
/// the two moved, rows would have been withheld from the drain without ever
/// being abandoned by it -- a queue that quietly forgets messages.
#[test]
fn the_scan_and_the_giveup_read_one_bound() {
    let body = drain_body();
    let scan = sites(&body, "pending_notifications(");
    assert_eq!(
        scan.len(),
        1,
        "the drain scans from {} places: {scan:?}",
        scan.len()
    );
    assert!(
        scan[0].1.contains("MAX_ATTEMPTS"),
        "{DRAIN}:{} no longer hands the bound to the scan: {}",
        scan[0].0,
        scan[0].1
    );

    let queries = source(QUERIES);
    assert!(
        queries.contains("max_attempts: i32"),
        "{QUERIES} no longer takes the bound from its caller -- it has gone back \
         to declaring its own copy of it"
    );
}

/// THE FINDING, 2026-09-21. The first cut of the resend bound ran the
/// increment before the send and `continue`d when it failed, so an UPDATE the
/// database refused stopped the message. Revoke UPDATE on
/// `notification_queue` and nothing went out at all -- a single point of
/// failure outbound delivery never had.
///
/// One early exit is left in this loop and it is the row with no chat id,
/// which reaches nobody whatever the database says. Any second one is a new
/// way for a write to cancel a delivery, which is the class this asserts
/// against rather than the one line that caused it.
#[test]
fn only_a_row_with_nobody_to_send_to_leaves_the_loop_early() {
    let body = drain_body();
    let exits = sites(&body, "continue");
    assert_eq!(
        exits.len(),
        1,
        "process_batch has {} early exits and may have one: {:?}. A second one is \
         how a failed write becomes a cancelled delivery -- the 2026-09-21 \
         finding, where a refused `attempts` UPDATE skipped the send.",
        exits.len(),
        exits
    );
    let no_chat = sites(&body, "telegram_id == 0");
    assert_eq!(
        no_chat.len(),
        1,
        "the no-chat guard moved or multiplied: {no_chat:?}"
    );
    assert!(
        exits[0].0 > no_chat[0].0,
        "the one early exit at {DRAIN}:{} is above the no-chat guard at :{} -- it \
         is therefore some other condition skipping the send",
        exits[0].0,
        no_chat[0].0
    );

    // And the counter is written once a pass, from a `match` whose error arm
    // falls through. A `?` or a `let ... else` here would be the same defect
    // wearing different syntax.
    let counter = sites(&body, "increment_attempts(");
    assert_eq!(
        counter.len(),
        1,
        "the attempt is counted in {} places: {counter:?}",
        counter.len()
    );
    assert!(
        !counter[0].1.contains('?'),
        "{DRAIN}:{} lets the counter's failure leave process_batch: {}",
        counter[0].0,
        counter[0].1
    );
}

/// What the loop does instead: it SEES the refused write, reports it, and
/// carries the attempt in memory so the bound is still reached.
///
/// Without these calls the loop would send an unbounded number of times on the
/// path where the column never moves, which is the other half of the property
/// and the reason the early exit above cannot simply be deleted.
#[test]
fn an_attempt_the_column_refuses_is_held_and_reported() {
    let body = drain_body();
    for held in [
        "uncounted.remembered(",
        "uncounted.remember(",
        "uncounted.forget(",
    ] {
        assert!(
            !sites(&body, held).is_empty(),
            "process_batch no longer calls `{held}` -- a row whose counter cannot \
             be written is back to being bounded by nothing"
        );
    }
    // The report. A condition nobody can see is one that decides the outcome
    // by accident, which is what this whole wave was about.
    let warn = body
        .iter()
        .any(|(_, code)| code.contains("tracing::warn!") || code.contains("refused the write"));
    assert!(
        warn,
        "process_batch swallows the refused counter write without a line saying so"
    );
}

/// The value of a `pub const NAME : str = "value";` in the contract. Panics
/// when the declaration is gone: a contract that stopped saying this is a
/// contract nobody is holding the code to.
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

/// 1-based line of the one shipped line containing `needle`.
fn only_line_with(text: &str, needle: &str) -> usize {
    let hits: Vec<usize> = text
        .lines()
        .enumerate()
        .filter(|(_, l)| code_of(l).contains(needle))
        .map(|(n, _)| n + 1)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "`{needle}` appears on {} lines of {DRAIN}: {hits:?}",
        hits.len()
    );
    hits[0]
}

/// The contract cites four lines of this file by number, and all four are
/// measured rather than remembered.
///
/// The 2026-09-21 rewrite moved every one of them, and a citation that has
/// drifted off its declaration is a failure this corpus has already had
/// (specs/turbobaby/order_status.t27:308-311 records one). A green build
/// cannot otherwise see it: the contract's constants are strings, and a string
/// that points at the wrong line is still a string.
#[test]
fn the_contracts_citations_still_land_on_their_declarations() {
    let text = source(DRAIN);
    for (constant, needle) in [
        ("INTERVAL_SOURCE_LINE", "const POLL_INTERVAL_SECS"),
        ("BUDGET_DECLARATION_SITE", "const MAX_ATTEMPTS"),
        (
            "ATTEMPTS_HELD_IN_MEMORY_DECLARATION_SITE",
            "struct UncountedAttempts",
        ),
        ("ATTEMPTS_SUM_SITE", "fn attempts_spent("),
    ] {
        let measured = format!("{DRAIN}:{}", only_line_with(&text, needle));
        assert_eq!(
            spec_str(constant),
            measured,
            "{SPEC}'s {constant} cites a line `{needle}` is no longer on; it is at {measured}"
        );
    }
}

/// One send per pass. A second `send_message` in this loop would be a second
/// message per attempt, and the budget counts passes rather than messages.
#[test]
fn a_pass_puts_at_most_one_message_on_the_wire() {
    let body = drain_body();
    let sends = sites(&body, "send_message(");
    assert_eq!(
        sends.len(),
        1,
        "process_batch sends from {} places: {sends:?}",
        sends.len()
    );
}
