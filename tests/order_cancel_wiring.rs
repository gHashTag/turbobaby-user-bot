//! A customer cancelling their own order, guarded as text where the host
//! cannot compile it.
//!
//! `src/lib.rs` gates `pub mod ui;` on `#[cfg(target_arch = "wasm32")]`, so
//! `cargo test` compiles nothing under `src/ui`. The READING of the server's
//! answer -- cancelled, refused, a failure of the server's own, or no answer
//! at all -- and every sentence the
//! customer is told about it live in `src/trios/api_errors.rs`, which the host
//! compiles and tests. What is guarded here is the wiring of the order detail
//! screen to that reading, and nothing stronger is claimed.
//!
//! WHY IT EXISTS (measured 2026-09-22, upstream main a33e500): the screen sent
//! `POST /api/orders/:id/cancel`, bound the answer to an unread local
//! (`let _resp`) and closed its dialog on every path. A success, a refusal and
//! a lost connection looked the same, and past the one-hour poll the status
//! never told the difference either.
//!
//! The rules pinned below:
//! - the answer is read, and the dialog closes only on a confirmed success
//!   (AGENTS.md, lesson 4: never close the UI before the server confirms):
//!   across the card and the sending task it has exactly two closes, the
//!   verdict's and the customer's own "no";
//! - every answer asks the PARENT to drop its last status reading and read it
//!   again, through an event handler; the card never restarts or clears the
//!   parent's resource itself (AGENTS.md, lesson 5);
//! - the answer is told above the block that disappears when the order leaves
//!   pending, so a refusal whose re-read removes that block is still told;
//! - after no answer, or a failure of the server's own, the outcome is
//!   unknown, and Confirm is gated on the reading's own verdict instead of
//!   being offered again blind -- a verdict fed from what landed in the
//!   screen's own resource, never from a constant, over an attempt that only
//!   the send and the answer ever write.
//!
//! This file imports nothing from the crate on purpose: it compiles against
//! any tree, so on a tree where the screen still discards the answer it fails
//! by CONTENT, which is the failure a reviewer needs to see.

use std::fs;
use std::path::{Path, PathBuf};

const DETAIL_PATH: &str = "src/ui/screens/order_detail_screen.rs";
/// The one fragment of the one URL this guard is about. The bookings cancel in
/// the events screen and the admin's event routes are different URLs and never
/// match it.
const CANCEL_URL: &str = "/api/orders/{}/cancel?telegram_id=";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// A tracked file, normalised to LF (a Windows checkout hands the reader CRLF).
/// A missing file is a failure, not an empty string.
fn source(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|e| panic!("{relative} must exist for this guard to mean anything: {e}"))
        .replace("\r\n", "\n")
}

/// The code of one line: the part before a `//` that is not inside a string
/// literal, so a URL or a style string is never mistaken for a comment.
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

/// Brace depth change of one code line, counting only braces outside string
/// literals: the URL literal itself carries `{}`.
fn brace_delta(code: &str) -> i32 {
    let mut in_string = false;
    let mut escaped = false;
    let mut delta = 0;
    for c in code.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => delta += 1,
            '}' => delta -= 1,
            _ => {}
        }
    }
    delta
}

/// The lines (1-based number, code) from the line that builds the cancel URL
/// down to the brace that closes the body holding it. On a tree that sends
/// from inside the button's task that body is the task; on a tree that sends
/// from a helper it is the helper.
fn cancel_body() -> Vec<(usize, String)> {
    let text = source(DETAIL_PATH);
    let lines: Vec<&str> = text.lines().collect();
    let starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| code_of(l).contains(CANCEL_URL))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        starts.len(),
        1,
        "{DETAIL_PATH} builds the order-cancel URL on {} lines; this guard reads exactly one",
        starts.len()
    );
    let mut depth = 0;
    let mut body = Vec::new();
    for (i, line) in lines.iter().enumerate().skip(starts[0]) {
        let code = code_of(line);
        depth += brace_delta(&code);
        body.push((i + 1, code));
        if depth < 0 {
            break;
        }
    }
    assert!(
        depth < 0,
        "the body holding the cancel URL never closes -- the scan is broken, not the code"
    );
    body
}

/// The text of one top-level item, from its signature to the next item.
fn item(text: &str, open: &str, next: &str) -> String {
    let start = text
        .find(open)
        .unwrap_or_else(|| panic!("{DETAIL_PATH}: `{open}` is gone -- this guard reads nothing"));
    let rest = &text[start..];
    let end = rest[open.len()..]
        .find(next)
        .map(|at| at + open.len())
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

fn card(text: &str) -> String {
    item(text, "fn OrderDetailCard(", "pub fn OrderDetailScreen(")
}

fn screen(text: &str) -> String {
    item(text, "pub fn OrderDetailScreen(", "\n}\n")
}

fn code_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(code_of)
        .filter(|c| !c.trim().is_empty())
        .collect()
}

/// The 0-based line range of the top-level item holding line `at`: from the
/// nearest `fn` signature at column 0 above it down to the first `}` at column
/// 0 (rustfmt closes every top-level item there).
fn item_holding(lines: &[&str], at: usize) -> (usize, usize) {
    let start = (0..=at)
        .rev()
        .find(|&i| {
            let l = lines[i];
            !l.starts_with(char::is_whitespace) && code_of(l).contains("fn ")
        })
        .unwrap_or_else(|| panic!("{DETAIL_PATH}:{}: no item signature above it", at + 1));
    let end = (at..lines.len())
        .find(|&i| lines[i] == "}")
        .unwrap_or_else(|| panic!("{DETAIL_PATH}:{}: the item never closes", at + 1));
    (start, end)
}

/// The arguments of the first call `name` (which ends in `(`) in `text`, split at
/// the call's own commas, with every whitespace character removed so that a
/// chain rustfmt breaks across lines reads the same as one on a single line.
/// `None` when there is no such call or it never closes.
fn call_args(text: &str, name: &str) -> Option<Vec<String>> {
    let at = text.find(name)? + name.len();
    let mut depth = 1;
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for c in text[at..].chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            current.push(c);
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                current.push(c);
            }
            '(' | '[' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    args.push(current);
                    let args: Vec<String> = args
                        .iter()
                        .map(|a| a.chars().filter(|c| !c.is_whitespace()).collect())
                        .filter(|a: &String| !a.is_empty())
                        .collect();
                    return Some(args);
                }
                current.push(c);
            }
            ',' if depth == 1 => args.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    None
}

/// Every write to a dialog-visibility signal -- a lower-case name holding
/// `confirm` followed by a method call -- in `code`, as the method and its
/// arguments with whitespace removed (`set(false)`, `set(true)`, `toggle()`).
/// Reads (`read`, `peek`, `cloned`, `with`) and a bare call (`name()`) are not
/// writes; the upper-case i18n keys never match.
fn confirm_writes(code: &str) -> Vec<String> {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut writes = Vec::new();
    let mut from = 0;
    while let Some(off) = code[from..].find("confirm") {
        let pos = from + off;
        let end = pos
            + code[pos..]
                .find(|c: char| !is_ident(c))
                .unwrap_or(code.len() - pos);
        from = end;
        let Some(rest) = code[end..].strip_prefix('.') else {
            continue;
        };
        let method_end = rest.find(|c: char| !is_ident(c)).unwrap_or(rest.len());
        let method = &rest[..method_end];
        if matches!(method, "read" | "peek" | "cloned" | "with") {
            continue;
        }
        let args = call_args(&rest[method_end..], "(").unwrap_or_default();
        writes.push(format!("{method}({})", args.join(",")));
    }
    writes
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// D16: the guards below read one body of one file. If the URL moved, was
/// renamed or was duplicated, they would read nothing or the wrong thing and
/// pass, so the scan first proves it sees exactly one order cancellation in
/// the whole client.
#[test]
fn the_scan_finds_the_one_order_cancellation() {
    let mut files = Vec::new();
    walk(&repo_root().join("src/ui"), &mut files);
    assert!(
        files.len() >= 20,
        "walked only {} files under src/ui -- a scan that reads nothing passes by default",
        files.len()
    );
    let mut hits = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path)
            .expect("readable")
            .replace("\r\n", "\n");
        for (i, line) in text.lines().enumerate() {
            if code_of(line).contains(CANCEL_URL) {
                let rel = path
                    .strip_prefix(repo_root())
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");
                hits.push(format!("{rel}:{}", i + 1));
            }
        }
    }
    assert_eq!(
        hits.len(),
        1,
        "the order cancellation is sent from {} places: {hits:?}",
        hits.len()
    );
    assert!(
        hits[0].starts_with(DETAIL_PATH),
        "the order cancellation moved out of {DETAIL_PATH}: {}",
        hits[0]
    );
    let body = cancel_body();
    assert!(
        body.len() >= 5,
        "the cancel body is {} lines -- too short to hold a send and its answer",
        body.len()
    );
}

/// The defect itself: `let _resp = ... .send().await;` -- an answer nobody read.
#[test]
fn the_cancellation_answer_is_read_and_never_discarded() {
    let body = cancel_body();
    let discarded: Vec<String> = body
        .iter()
        .filter(|(_, code)| code.trim_start().starts_with("let _"))
        .map(|(n, code)| format!("{DETAIL_PATH}:{n}: `{}`", code.trim()))
        .collect();
    assert!(
        discarded.is_empty(),
        "the cancellation's answer is bound to a name nobody reads:\n  {}",
        discarded.join("\n  ")
    );
    assert!(
        body.iter()
            .any(|(_, code)| code.contains("order_cancel_answer(")),
        "the cancel body never asks order_cancel_answer( what the server said"
    );
}

/// AGENTS.md lesson 4. Every `.set(false)` in the cancel body closes the dialog,
/// and each one must sit directly under the reading's own verdict.
#[test]
fn the_dialog_closes_only_on_a_confirmed_success() {
    let body = cancel_body();
    let mut closes = 0usize;
    let mut offences = Vec::new();
    for (at, (n, code)) in body.iter().enumerate() {
        if !code.contains(".set(false)") {
            continue;
        }
        closes += 1;
        let guard = body[..at]
            .iter()
            .rev()
            .find(|(_, c)| !c.trim().is_empty())
            .map(|(_, c)| c.trim().to_string())
            .unwrap_or_default();
        if !(guard.starts_with("if ") && guard.contains("order_cancel_dialog_closes(")) {
            offences.push(format!(
                "{DETAIL_PATH}:{n}: `{}` follows `{guard}`",
                code.trim()
            ));
        }
    }
    assert!(
        closes >= 1,
        "the cancel body never closes the dialog -- a success would leave it open"
    );
    assert!(
        offences.is_empty(),
        "the dialog closes without the server's confirmation:\n  {}",
        offences.join("\n  ")
    );
}

/// AGENTS.md lesson 5, and the re-read. Every answer -- a success, a refusal
/// and no answer alike -- is followed by a fresh reading of the status, and it
/// is the PARENT that reads: the card calls the handler, the screen drops the
/// stale reading and restarts its own resource.
#[test]
fn every_answer_asks_the_parent_to_read_the_status_again() {
    let body = cancel_body();
    let mut depth = 0;
    let mut unconditional_call = false;
    for (_, code) in &body {
        if depth == 0 && code.contains(".call(())") {
            unconditional_call = true;
        }
        depth += brace_delta(code);
    }
    assert!(
        unconditional_call,
        "the cancel body never tells the parent that an answer arrived, outside every `if`"
    );

    let text = source(DETAIL_PATH);
    let mut child = code_lines(&card(&text));
    child.extend(body.iter().map(|(_, c)| c.clone()));
    // `.restart()` anywhere in the card is a resource restarted from the child;
    // `.clear()` only counts on the status resource, because the reorder path
    // clears the cart, which is a different defect of a different family.
    let mutations: Vec<&String> = child
        .iter()
        .filter(|c| {
            c.contains(".restart()") || (c.contains("live_status") && c.contains(".clear()"))
        })
        .collect();
    assert!(
        mutations.is_empty(),
        "the card mutates its parent's resource from its own code: {mutations:?}"
    );

    let parent = code_lines(&screen(&text)).join("\n");
    let handler_at = parent
        .find("on_cancel_answered: EventHandler::new(")
        .expect("OrderDetailScreen gives the card no on_cancel_answered handler");
    let handler = &parent[handler_at..];
    let handler = &handler[..handler.find("}),").unwrap_or(handler.len())];
    let clear = handler.find("live_status_res.clear();");
    let restart = handler.find("live_status_res.restart();");
    assert!(
        matches!((clear, restart), (Some(c), Some(r)) if c < r),
        "the handler must drop the stale reading and then read again, in that order:\n{handler}"
    );
}

/// A 409 means the order has left pending; the re-read then removes the whole
/// pending block, dialog included. The sentence about the attempt must be
/// rendered above that block, or the refusal is told for one frame.
#[test]
fn the_answer_is_told_outside_the_pending_gate() {
    let text = source(DETAIL_PATH);
    let card = code_lines(&card(&text)).join("\n");
    let told = card
        .find("order_cancel_line(")
        .expect("the card never renders order_cancel_line( -- the answer is told nowhere");
    let gate = card
        .find("if is_pending")
        .expect("the card has no pending gate any more");
    assert!(
        told < gate,
        "the answer is rendered inside the pending gate, so a refusal that removes it is never told"
    );
}

/// The owner's rule for an unknown outcome: look again BEFORE repeating. The
/// Confirm button's `disabled:` asks the reading, which keeps it shut after no
/// answer until a fresh status shows the order still cancellable.
#[test]
fn an_unanswered_cancellation_is_never_sent_again_blind() {
    let text = source(DETAIL_PATH);
    let card = code_lines(&card(&text));
    let gated: Vec<&String> = card
        .iter()
        .filter(|c| c.contains("disabled:") && c.contains("order_cancel_may_send("))
        .collect();
    assert_eq!(
        gated.len(),
        1,
        "the Confirm button must be gated by order_cancel_may_send( exactly once: {gated:?}"
    );
    assert!(
        !card.iter().any(|c| c.contains("disabled: cancelling()")),
        "the Confirm button is still gated on the in-flight flag alone"
    );
}

/// The gate above is only as good as what it is fed. `since_answer` is the
/// reading's verdict on what the screen has read of the status SINCE the
/// answer, and it must come from the screen's own resource: a constant there
/// (`Some(true)`) passes every check above and reopens Confirm the moment no
/// answer arrives, the blind repeat the owner's rule forbids. So the card
/// derives it once, through `StatusSinceAnswer::from_landing(`, from what has
/// landed in `live_status_res` and from the same `is_pending` that gates the
/// block, and that one binding is what both the line and the gate receive.
#[test]
fn the_unknown_outcome_gate_reads_what_the_screen_read_since_the_answer() {
    let text = source(DETAIL_PATH);
    let card = code_lines(&card(&text)).join("\n");

    let bindings = card.matches("let since_answer").count();
    assert_eq!(
        bindings, 1,
        "the card binds since_answer {bindings} times; this guard reads exactly one"
    );
    let at = card.find("let since_answer").expect("counted above");
    let statement = &card[at..at + card[at..].find(';').expect("the binding never ends")];
    let args = call_args(statement, "StatusSinceAnswer::from_landing(").unwrap_or_else(|| {
        panic!("since_answer is not derived through StatusSinceAnswer::from_landing(:\n{statement}")
    });
    assert_eq!(
        args,
        [
            "live_status_res.read().as_ref().map(Option::is_some)",
            "is_pending"
        ],
        "since_answer is not fed from what landed in the screen's resource and from the \
         pending gate:\n{statement}"
    );
    assert!(
        !card
            .lines()
            .any(|l| l.trim_start().starts_with("since_answer =")),
        "since_answer is overwritten after it is derived"
    );

    for consumer in ["order_cancel_line(", "order_cancel_may_send("] {
        let calls = card.matches(consumer).count();
        assert_eq!(calls, 1, "the card calls {consumer} {calls} times");
        let args = call_args(&card, consumer).expect("counted above");
        assert_eq!(
            args.last().map(String::as_str),
            Some("since_answer"),
            "{consumer} is not given the card's since_answer: {args:?}"
        );
        assert!(
            args.iter().any(|a| a == "cancel_progress()"),
            "{consumer} is not given the card's own progress: {args:?}"
        );
    }

    // The resource the card reads is the one the screen drops and reads again.
    let parent = code_lines(&screen(&text));
    let passed = parent
        .iter()
        .skip_while(|c| !c.contains("OrderDetailCard {"))
        .take_while(|c| !c.contains("on_cancel_answered:"))
        .any(|c| c.trim() == "live_status_res,");
    assert!(
        passed,
        "OrderDetailScreen does not hand the card the live_status_res it clears and restarts"
    );
}

/// The attempt's state has exactly two writers: Confirm marks it in flight, and
/// the sending task records the answer. Anything else -- reopening the dialog
/// resetting it to Idle, say -- would forget an unknown outcome and let Confirm
/// send again blind, past the gate above.
#[test]
fn the_attempt_is_written_only_by_the_send_and_the_answer() {
    let text = source(DETAIL_PATH);
    let card = code_lines(&card(&text));
    let card_writes: Vec<&str> = card
        .iter()
        .filter(|c| c.contains("cancel_progress."))
        .map(|c| c.trim())
        .collect();
    assert_eq!(
        card_writes,
        ["cancel_progress.set(crate::trios::api_errors::OrderCancelProgress::InFlight);"],
        "the card writes the attempt somewhere other than Confirm's send"
    );

    let lines: Vec<&str> = text.lines().collect();
    let url_at = lines
        .iter()
        .position(|l| code_of(l).contains(CANCEL_URL))
        .expect("the cancel URL is gone");
    let (start, end) = item_holding(&lines, url_at);
    let sender_writes: Vec<String> = lines[start..=end]
        .iter()
        .map(|l| code_of(l).trim().to_string())
        .filter(|c| c.starts_with("progress.") || c.contains(" progress."))
        .collect();
    assert_eq!(
        sender_writes,
        ["progress.set(OrderCancelProgress::Answered(answer));"],
        "the sending task writes the attempt other than with its answer"
    );
}

/// AGENTS.md lesson 4, over the whole card and not only the task's body. The
/// dialog has exactly two ways to close, and a close anywhere else -- first
/// thing in Confirm's own onclick, say, before anything is sent -- closes it
/// before the server has said a word. The two: the verdict's, directly under
/// `if order_cancel_dialog_closes(`, and the customer's own "no", whose onclick
/// does nothing else. A write this guard cannot classify (`toggle()`,
/// `write()`) is refused rather than guessed at.
#[test]
fn every_close_of_the_dialog_is_the_verdicts_or_the_customers_own_no() {
    let text = source(DETAIL_PATH);
    let lines: Vec<&str> = text.lines().collect();
    let card_at = lines
        .iter()
        .position(|l| l.starts_with("fn OrderDetailCard("))
        .expect("the card is gone");
    let url_at = lines
        .iter()
        .position(|l| code_of(l).contains(CANCEL_URL))
        .expect("the cancel URL is gone");
    let mut ranges = vec![item_holding(&lines, card_at)];
    let sender = item_holding(&lines, url_at);
    if !ranges.contains(&sender) {
        ranges.push(sender);
    }
    assert!(
        ranges.iter().map(|(s, e)| e - s).sum::<usize>() >= 150,
        "the card and the sender read as {ranges:?} -- too short to hold the dialog"
    );

    let mut verdict_closes = 0usize;
    let mut no_closes = 0usize;
    let mut offences = Vec::new();
    for (start, end) in ranges {
        let code: Vec<(usize, String)> = (start..=end)
            .map(|i| (i + 1, code_of(lines[i])))
            .filter(|(_, c)| !c.trim().is_empty())
            .collect();
        for (at, (n, line)) in code.iter().enumerate() {
            for write in confirm_writes(line) {
                match write.as_str() {
                    "set(true)" => {}
                    "set(false)" => {
                        let before = at
                            .checked_sub(1)
                            .map(|p| code[p].1.trim())
                            .unwrap_or_default();
                        let after = code.get(at + 1).map(|(_, c)| c.trim()).unwrap_or_default();
                        if before.starts_with("if ")
                            && before.contains("order_cancel_dialog_closes(")
                        {
                            verdict_closes += 1;
                        } else if line.trim()
                            == "onclick: move |_| { show_cancel_confirm.set(false); },"
                            && after.contains("T_MODAL_CANCEL")
                        {
                            no_closes += 1;
                        } else {
                            offences.push(format!(
                                "{DETAIL_PATH}:{n}: `{}` closes the dialog outside the verdict \
                                 and outside the customer's own \"no\"",
                                line.trim()
                            ));
                        }
                    }
                    other => offences.push(format!(
                        "{DETAIL_PATH}:{n}: `{}` writes the dialog with `{other}`, which this \
                         guard cannot classify",
                        line.trim()
                    )),
                }
            }
        }
    }
    assert!(
        offences.is_empty(),
        "the dialog can close before the server confirms:\n  {}",
        offences.join("\n  ")
    );
    assert_eq!(
        (verdict_closes, no_closes),
        (1, 1),
        "expected exactly one close under the verdict and one on the customer's own \"no\""
    );
}

/// Guard against the tempting one-line fix. The general mapper's 409 sentence is
/// the checkout's "this order has already been placed", and a refused
/// cancellation is exactly a 409. Scoped to the cancel body: other defects on
/// the same screen may map their own statuses through the general mapper.
#[test]
fn the_cancel_body_never_calls_the_general_mapper() {
    let body = cancel_body();
    assert!(
        !body
            .iter()
            .any(|(_, code)| code.contains("friendly_response_error(")),
        "the cancellation is told through the checkout's sentences"
    );
}
