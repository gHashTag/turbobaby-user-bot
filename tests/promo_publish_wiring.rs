//! The Publish button refuses a draft that is not about a rental, and this
//! file holds the live site to that, reading it as text.
//!
//! WHY IT EXISTS. `publish` (src/promo.rs) needs a live database and a live
//! bot, so no test executes it. The decision it now asks -- may a draft of
//! this kind reach customers -- was put in `publish_refusal` and unit-tested
//! beside it. Those tests run the decision, not the button: delete the call
//! from `publish`, or move it below the `published_at` stamp, and every one of
//! them stays green while an old event draft goes out to every customer again.
//!
//! The decision is the operator's of 2026-09-25, under the owner's delegation,
//! and `specs/turbobaby/promo_broadcast.t27` records it (PUBLISH_REFUSAL_*).
//! Events left every customer surface with the rental-only ruling of
//! 2026-09-24, and so did the retired shop's goods, but `promo_posts` keeps
//! every draft the sweeper ever wrote, each with its Publish button still in
//! the owners' chat. Nothing is deleted: a refused draft stays a draft.
//!
//! So this file pins what the unit tests cannot: `publish` reads the row's
//! kind, asks the decision, and returns BEFORE it stamps the row published and
//! before anything is sent; it is the only path that stamps a draft published;
//! and the kinds the contract lists are the kinds the code refuses. The
//! pattern is `tests/promo_copy_wiring.rs`'s.
//!
//! WHAT IT DOES NOT PROVE. Nothing about the admin HTTP broadcast, the other
//! send path, which sends admin-typed text and not a draft; and nothing about
//! whether the sweeper keeps drafting -- migration 088 and the hidden rows are
//! what stop that, and `turbobaby/events-booking` owns them.

use std::fs;
use std::path::PathBuf;

/// The sweeper, the drafts and the button's handler.
const PROMO: &str = "src/promo.rs";
/// Where the words `Subject::kind` writes into the `kind` column live.
const RULES: &str = "src/trios/promo.rs";
/// The route that turns a button press into a call.
const CALLBACKS: &str = "src/bot/callbacks.rs";
/// The contract that records the decision.
const SPEC: &str = "specs/turbobaby/promo_broadcast.t27";

fn source(rel: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|e| panic!("{rel} is readable: {e}"))
        .replace("\r\n", "\n")
}

/// A line with its `//` comment removed, so prose DESCRIBING a call is never
/// counted as the call. Naive by intent, as in `tests/promo_copy_wiring.rs`: it
/// does not track string literals, and none of the needles below contains
/// `//`.
fn code_of(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// The body of a top-level item as `(line number, code)`: from the line after
/// the one starting with `header` to the first `}` in the first column.
///
/// Panics when the header is gone: a renamed or deleted `publish` is a wiring
/// change, and this file must not pass by finding nothing.
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

/// Line numbers of a body whose CODE contains `needle`.
fn lines_with(body: &[(usize, String)], needle: &str) -> Vec<usize> {
    body.iter()
        .filter(|(_, code)| code.contains(needle))
        .map(|(n, _)| *n)
        .collect()
}

/// The one line of a body whose code contains `needle`. Zero or two is a
/// wiring change and fails here, naming the needle.
fn the_line(body: &[(usize, String)], needle: &str) -> usize {
    let found = lines_with(body, needle);
    assert_eq!(
        found.len(),
        1,
        "`{needle}` occurs {} times in publish's code, at {found:?}",
        found.len()
    );
    found[0]
}

/// Every `.rs` file under `src/`, label and text, walked rather than listed:
/// the claims below are *nowhere else*, and a fixed list cannot say that
/// about a file added tomorrow.
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

/// `(file, line)` of every CODE line under `src/` containing `needle`.
fn sites_in_src(needle: &str) -> Vec<(String, usize)> {
    rust_sources()
        .into_iter()
        .flat_map(|(label, text)| {
            text.lines()
                .enumerate()
                .filter(|(_, l)| code_of(l).contains(needle))
                .map(|(n, _)| (label.clone(), n + 1))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The quoted items of a one-line list declaration, read after its ` = `:
/// `["event", "event_soon"]` or `&[]`. Panics when the line has no list.
fn quoted_items(line: &str) -> Vec<String> {
    let value = &line[line.find(" = ").expect("a declaration has a value") + 3..];
    let open = value.find('[').expect("a list opens");
    let close = value.rfind(']').expect("a list closes");
    value[open + 1..close]
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_matches('"').to_string())
        .collect()
}

/// The one line of a file starting with `head`. Panics when it is gone: a
/// declaration nobody holds is a declaration nobody checks.
fn declaration(rel: &str, head: &str) -> String {
    let text = source(rel);
    let found: Vec<&str> = text.lines().filter(|l| l.starts_with(head)).collect();
    assert_eq!(
        found.len(),
        1,
        "`{head}` is declared {} times in {rel}",
        found.len()
    );
    found[0].to_string()
}

/// The value of a `pub const NAME : str = "value";` in the contract.
fn spec_str(name: &str) -> String {
    let line = declaration(SPEC, &format!("pub const {name} : str ="));
    let open = line.find('"').expect("a str declaration is quoted");
    let close = line.rfind('"').expect("a str declaration is quoted");
    assert!(close > open, "`{name}` in {SPEC} has no value");
    line[open + 1..close].to_string()
}

/// Calibration. The reader must reach the end of `publish` and must not keep
/// its comments; otherwise every order asserted below is the prose's.
#[test]
fn the_reader_sees_all_of_publish_and_none_of_its_comments() {
    let body = body_of(PROMO, "pub async fn publish(");
    the_line(&body, "Рассылка запущена");
    assert!(
        lines_with(&body, "Nothing retired reaches a customer").is_empty(),
        "the reader kept publish's comments"
    );
}

/// The button asks the decision, with the row's own kind, and returns its
/// answer BEFORE the row is stamped published and before anything is sent.
///
/// Each half fails differently. Drop the call and an old event draft goes to
/// every customer. Move it below the stamp and the draft is marked published
/// without being sent -- the next press answers "already published" and the
/// row no longer reads as the draft it is.
#[test]
fn publish_refuses_before_it_stamps_or_sends_anything() {
    let body = body_of(PROMO, "pub async fn publish(");

    let select = the_line(&body, "\"SELECT kind, ");
    let read = the_line(&body, "try_get(\"\", \"kind\")");
    let asked = the_line(&body, "publish_refusal(&kind)");
    let answered = the_line(&body, "return refusal");
    let stamp = the_line(&body, "SET published_at");
    let channel = the_line(&body, "send_message(");
    let recipients = the_line(&body, "broadcast_recipients(");
    let fan_out = the_line(&body, "tokio::spawn");

    for (what, before, after) in [
        ("the row's kind is selected before it is read", select, read),
        ("the kind is read before it is asked about", read, asked),
        ("the refusal is returned where it is asked", asked, answered),
        ("the refusal returns before the stamp", answered, stamp),
        ("the stamp precedes the channel post", stamp, channel),
        ("the stamp precedes the recipient read", stamp, recipients),
        (
            "the recipient read precedes the fan-out",
            recipients,
            fan_out,
        ),
    ] {
        assert!(
            before < after,
            "{what}: {PROMO}:{before} is not above {PROMO}:{after}"
        );
    }
    // Inside the refusal's own block: a log line may stand between, nothing
    // that reaches a customer can.
    assert!(
        answered - asked <= 6,
        "the refusal at {PROMO}:{asked} is not returned at once (:{answered})"
    );
}

/// D15: one place decides, and one path turns a draft into a mailing.
///
/// A second copy of the decision would pass the unit tests while the button
/// used the other one; a second place that stamps a draft published, or a
/// second caller of `publish`, would be a way round the refusal.
#[test]
fn one_decision_and_one_path_from_a_draft_to_a_mailing() {
    let homes = sites_in_src("fn publish_refusal(");
    assert_eq!(
        homes.iter().map(|(f, _)| f.as_str()).collect::<Vec<_>>(),
        vec![PROMO],
        "the publish decision is declared at {homes:?}"
    );

    let stamps = sites_in_src("SET published_at");
    assert_eq!(
        stamps.iter().map(|(f, _)| f.as_str()).collect::<Vec<_>>(),
        vec![PROMO],
        "a draft is stamped published at {stamps:?}"
    );

    let callers: Vec<(String, usize)> = sites_in_src("promo::publish(");
    assert_eq!(
        callers.iter().map(|(f, _)| f.as_str()).collect::<Vec<_>>(),
        vec![CALLBACKS],
        "publish is called from {callers:?}"
    );
    // And that caller is the Publish button, not some other callback.
    let handler = source(CALLBACKS);
    let prefix = handler
        .find("strip_prefix(crate::promo::PUBLISH_CALLBACK)")
        .expect("the handler still routes the Publish button by its prefix");
    let call = handler
        .find("crate::promo::publish(")
        .expect("the handler still calls publish");
    assert!(
        prefix < call,
        "publish is no longer called under the Publish button's prefix"
    );
}

/// The contract's lists and the code's are the same lists.
///
/// The contract names the kinds by their words; the code refuses by those
/// words. The words themselves are `Subject::kind`'s, so every one it writes
/// today must be in the contract's retired lists too -- a kind added there and
/// classified nowhere would be refused as unrecognised, which is safe, but the
/// contract would no longer describe the table.
#[test]
fn the_contract_and_the_code_list_the_same_kinds() {
    let code_events = quoted_items(&declaration(PROMO, "const RETIRED_EVENT_KINDS:"));
    let spec_events = quoted_items(&declaration(SPEC, "pub const RETIRED_EVENT_DRAFT_KINDS :"));
    assert_eq!(code_events, spec_events, "the event kinds disagree");

    let code_publishable = quoted_items(&declaration(PROMO, "const PUBLISHABLE_KINDS:"));
    let spec_count = declaration(SPEC, "pub const PUBLISHABLE_DRAFT_KIND_COUNT : u8 =");
    let spec_count: usize = spec_count
        .rsplit(' ')
        .next()
        .and_then(|v| v.trim_end_matches(';').parse().ok())
        .expect("a u8 value");
    assert_eq!(
        code_publishable.len(),
        spec_count,
        "the code declares {code_publishable:?} publishable"
    );

    let spec_shop = quoted_items(&declaration(SPEC, "pub const RETIRED_SHOP_DRAFT_KINDS :"));
    let written_today: Vec<String> = source(RULES)
        .lines()
        .filter(|l| l.trim_start().starts_with("Subject::") && l.contains("{ .. } => \""))
        .map(|l| {
            let open = l.find("=> \"").expect("an arm") + 4;
            let close = open + l[open..].find('"').expect("a closing quote");
            l[open..close].to_string()
        })
        .collect();
    assert!(
        written_today.len() >= 6,
        "the scan found {written_today:?} -- it is not reaching Subject::kind"
    );
    for kind in &written_today {
        assert!(
            spec_events.contains(kind) || spec_shop.contains(kind),
            "Subject::kind writes {kind:?}, which the contract classifies nowhere"
        );
    }
    assert!(
        spec_shop.iter().any(|k| k == "strain"),
        "the strain kind left with 083, but its drafts did not"
    );
    // A retired shop kind is refused by publish_refusal's last branch, the one
    // no list feeds, so none of them may sit in a list that answers otherwise.
    // The unit tests under src/ run four of these kinds; the one that left
    // with 083 may not be named outside a comment there
    // (tests/legacy_vocabulary_wiring.rs; owner ruling 2026-09-25, item 12),
    // so this is where its refusal is held.
    for kind in &spec_shop {
        assert!(
            !code_publishable.contains(kind) && !code_events.contains(kind),
            "the retired shop kind {kind:?} sits in a list publish_refusal answers otherwise for"
        );
    }
}

/// The decision is recorded where the repository's rules say, dated, and
/// named as what it is.
#[test]
fn the_decision_is_recorded_in_the_contract() {
    assert_eq!(spec_str("PUBLISH_REFUSAL_DECIDED_AT"), "2026-09-25");
    assert_eq!(
        spec_str("PUBLISH_REFUSAL_DECIDED_BY"),
        "operator decision under the owner's delegation"
    );
}
