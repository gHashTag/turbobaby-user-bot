//! The live draft site is never executed by a test, so this file reads it as
//! text.
//!
//! WHY IT EXISTS. `write_copy` (src/promo.rs) needs a live model and a live
//! database, so the decision that puts words under the shop's name was
//! extracted into `copy_from_answer` and unit-tested there. An adversarial
//! reviewer on 2026-09-21 made the obvious point: those tests execute the
//! extracted function, not the path the sweeper runs. Inline the old `match`
//! back into `write_copy` and every one of them stays green while the shipped
//! sweeper publishes the sentinel again.
//!
//! The sentinel is what makes that worth a file of its own. When the prompt
//! sanitiser trips, `ask_grok` answers with `crate::ai::PROMPT_FILTERED_REPLY`
//! -- a sentence this repository wrote -- inside the same `Some` a real answer
//! arrives in. Five call sites are chats, where that is a reply. The sixth is
//! this one, where there is no chat and it becomes the BODY of a promotional
//! post, stored with source = "model" and one owner press from every
//! subscriber. It passes `usable_copy` cleanly: that function's refusal list
//! holds "i cannot" and "i'm sorry" and not "i can't", and the sentence is
//! well inside its length bounds.
//!
//! So this file pins three things the unit tests cannot: the live site still
//! routes through the decision, the decision still asks the recogniser, and it
//! asks BEFORE the owner's copy check rather than after it. The pattern is
//! `tests/events_money_decision_wiring.rs`'s, including its habit of reading
//! the contract's own citation back out of the spec.
//!
//! WHAT IT DOES NOT PROVE. Nothing about what the model says, nothing about
//! whether the fallback copy is good, and nothing about the five chat sites --
//! those still send the sentinel as the model's answer, which is a defect
//! `specs/turbobaby/ai_assist.t27` FILTER_REPLY_NOTE keeps open on purpose.

use std::fs;
use std::path::PathBuf;

/// The sweeper that drafts posts.
const PROMO: &str = "src/promo.rs";
/// Where the sentinel and its recogniser live.
const AI: &str = "src/ai.rs";
/// The contract that owns the consequence for a broadcast.
const SPEC: &str = "specs/turbobaby/promo_broadcast.t27";

fn source(rel: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|e| panic!("{rel} is readable: {e}"))
}

/// A line with its `//` comment removed, so prose DESCRIBING a call is never
/// counted as the call. Naive by intent, the same naivety
/// `tests/cart_kind_wiring.rs` accepts: it does not track string literals.
///
/// It earns its place here. `copy_from_answer`'s doc comment names
/// `usable_copy`, `fallback_copy` and `PROMPT_FILTERED_REPLY` while explaining
/// the defect, and an ordering read off those sentences would be the prose's
/// ordering and not the code's.
fn code_of(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// The body of a top-level item as `(line number, code)`: from the line after
/// the one starting with `header` to the first `}` in the first column.
///
/// Panics when the header is gone. That is the point -- a renamed or deleted
/// decision is a wiring change, and this file must not pass by finding
/// nothing.
fn body_of(rel: &str, header: &str) -> Vec<(usize, String)> {
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
        .enumerate()
        .map(|(i, l)| (start + 2 + i, code_of(l).to_string()))
        .collect()
}

/// Lines of a body whose CODE contains `needle`, as `(line, trimmed code)`.
fn sites(body: &[(usize, String)], needle: &str) -> Vec<(usize, String)> {
    body.iter()
        .filter(|(_, code)| code.contains(needle))
        .map(|(n, code)| (*n, code.trim().to_string()))
        .collect()
}

/// Every `.rs` file under `src/`, label and text. A walk and not a fixed list,
/// for the reason `cart_kind_wiring.rs` gives: the claim below is *nowhere
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
                // One separator whatever the host: the labels below are
                // written with forward slashes, and a backslash makes every
                // comparison miss and reports a drift that is not there.
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
    assert!(
        out.len() >= 100,
        "the walk found {} files -- it is not reaching src/",
        out.len()
    );
    out
}

/// The value of a `pub const NAME : str = "value";` in a contract. Panics when
/// the declaration is gone: a contract that stopped saying this is a contract
/// nobody is holding the code to.
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

/// Calibration. A reader that kept comments would find every needle in the
/// prose that explains the defect and call the wiring sound while it was gone.
#[test]
fn the_reader_sees_the_decision_and_not_the_prose_about_it() {
    let decision = body_of(PROMO, "fn copy_from_answer(");
    assert!(
        decision.len() > 5,
        "the reader kept {} lines of copy_from_answer",
        decision.len()
    );
    // `usable_copy`'s refusal list is quoted in the doc comment ABOVE this
    // function and nowhere in its body except the one call.
    assert_eq!(
        sites(&decision, "i cannot").len(),
        0,
        "the reader kept copy_from_answer's doc comment, so every ordering \
         below is the prose's and not the code's"
    );
    assert_eq!(
        sites(&decision, "return (usable, \"model\")").len()
            + sites(&decision, "(usable, \"model\")").len(),
        1,
        "the reader did not reach the end of copy_from_answer"
    );
}

/// D15: one place decides what a post says and who wrote it.
///
/// A second copy would pass every unit test in `src/promo.rs` while the live
/// sweeper used the other one.
#[test]
fn one_place_decides_what_the_post_says() {
    let mut homes: Vec<String> = Vec::new();
    for (label, text) in rust_sources() {
        if text
            .lines()
            .any(|l| code_of(l).contains("fn copy_from_answer("))
        {
            homes.push(label);
        }
    }
    homes.sort();
    assert_eq!(
        homes,
        vec![PROMO.to_string()],
        "the draft decision is declared in {} places -- one, or they drift (D15)",
        homes.len()
    );
}

/// The live site asks the decision instead of making it again.
///
/// This is the revert the reviewer described: leave `copy_from_answer` in
/// place, inline the old `match` into `write_copy`, and four unit tests stay
/// green behind a sweeper that publishes the sentinel.
#[test]
fn the_live_draft_site_routes_through_the_decision() {
    let live = body_of(PROMO, "async fn write_copy(");
    assert_eq!(
        sites(&live, "copy_from_answer(").len(),
        1,
        "write_copy no longer hands the model's answer to copy_from_answer: {live:?}"
    );
    for reimplemented in [
        "usable_copy(",
        "fallback_copy(",
        "is_prompt_filtered_reply(",
    ] {
        assert!(
            sites(&live, reimplemented).is_empty(),
            "write_copy calls `{reimplemented}` itself -- the decision is being \
             taken twice, and only one of the two is under test"
        );
    }
    // And it is the sweeper's own path: one caller, and it is the tick.
    let callers: Vec<(String, usize)> = rust_sources()
        .into_iter()
        .flat_map(|(label, text)| {
            text.lines()
                .enumerate()
                .filter(|(_, l)| {
                    code_of(l).contains("write_copy(") && !code_of(l).contains("fn write_copy(")
                })
                .map(|(n, _)| (label.clone(), n + 1))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        callers.len(),
        1,
        "write_copy is called from {} places: {callers:?}",
        callers.len()
    );
    assert_eq!(callers[0].0, PROMO);
}

/// The decision refuses the sentinel BY IDENTITY and BEFORE the owner's copy
/// check.
///
/// Both halves matter and they fail differently. Drop the call and the
/// sentence is published as the post. Move it after `usable_copy` and it is
/// published too, because `usable_copy` returns the sentence unchanged -- it
/// is the one input that passes every bound that function applies.
#[test]
fn the_sentinel_is_refused_by_identity_before_the_copy_check() {
    let decision = body_of(PROMO, "fn copy_from_answer(");
    let refusal = sites(&decision, "is_prompt_filtered_reply(");
    assert_eq!(
        refusal.len(),
        1,
        "copy_from_answer asks the recogniser {} times: {refusal:?}",
        refusal.len()
    );
    let check = sites(&decision, "usable_copy(");
    assert_eq!(
        check.len(),
        1,
        "copy_from_answer runs the owner's copy check {} times: {check:?}",
        check.len()
    );
    assert!(
        refusal[0].0 < check[0].0,
        "the sentinel is refused at {PROMO}:{}, AFTER the copy check at :{} -- \
         and the copy check hands the sentence straight back, so that order \
         publishes it",
        refusal[0].0,
        check[0].0
    );

    // Identity, not prose. The recogniser compares against the declaration, so
    // rewording the sentinel cannot leave this consumer matching words it no
    // longer uses -- which is the whole reason the constant was named.
    let recogniser = body_of(AI, "pub(crate) fn is_prompt_filtered_reply(");
    assert!(
        recogniser
            .iter()
            .any(|(_, code)| code.contains("PROMPT_FILTERED_REPLY")),
        "is_prompt_filtered_reply no longer compares against the declaration: \
         {recogniser:?}"
    );
}

/// The contract cites a line, and the line is measured rather than
/// remembered.
///
/// `SENTINEL_REFUSAL_SITE` is how `specs/turbobaby/promo_broadcast.t27` points
/// a reader at the refusal. A citation that drifts off its declaration is a
/// failure this corpus has already had (order_status.t27:308-311), and it is
/// the kind a green build cannot otherwise see.
#[test]
fn the_contracts_citation_still_lands_on_the_refusal() {
    let decision = body_of(PROMO, "fn copy_from_answer(");
    let refusal = sites(&decision, "is_prompt_filtered_reply(");
    let measured = format!("{PROMO}:{}", refusal[0].0);
    assert_eq!(
        spec_str("SENTINEL_REFUSAL_SITE"),
        measured,
        "{SPEC} cites a line the refusal is no longer on; it is at {measured}"
    );
}
