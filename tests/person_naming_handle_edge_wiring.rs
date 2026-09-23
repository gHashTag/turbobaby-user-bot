//! One bound, one owner. The attendee normaliser's handle-length bound belongs to
//! `turbobaby/events-booking`; `turbobaby/person-naming` names that owner as an edge
//! and consumes its verdict. This file reads both contracts as TEXT and pins that.
//!
//! WHY IT EXISTS. Until 2026-09-22 `specs/turbobaby/person_naming.t27` restated that
//! bound in four constants and three functions of its own, beside an unbound copy of
//! the module's line count, while the owner held the same bound under six declarations.
//! Two present-tense copies of one fact is the drift this corpus forbids: a fix to the
//! check in `src/trios/attendees.rs` would have had to find both.
//!
//! WHY A HOST TEST AND NOT ONLY AN ASSERTION IN THE CORPUS. The contract records the
//! resolution as `THIS_FILE_RESTATES_THE_HANDLE_BOUND == false`, and that is a
//! self-report. `scripts/execute_t27_assertions.py` evaluates one contract at a time and
//! cannot see that a declaration is ABSENT, nor that a name one contract says another
//! declares is declared there; the pinned compiler discards test bodies entirely. So the
//! flag could read `false` with every copy back in place, and every gate would stay
//! green. The two properties that make the resolution real are checked here instead.
//!
//! THE NAMES ARE WRITTEN HERE, not read from the file they police. The contract keeps
//! its own list of what it dropped, as a landing point for a grep, but a list the
//! consumer carries can be edited in the same change that brings a copy back.
//!
//! What this file does NOT prove: that the owner's numbers are true of
//! `src/trios/attendees.rs` (that is gate 3's subject, `scripts/verify_t27_against_source.py`),
//! or that no copy of the bound returns under a name nobody has used yet. The prefix rule
//! below narrows that second gap to the families it lists; it does not close it.

use std::fs;
use std::path::{Path, PathBuf};

/// The contract that consumes the bound.
const CONSUMER: &str = "specs/turbobaby/person_naming.t27";
/// The contract that owns it, found by ID rather than by path.
const OWNER_ID: &str = "turbobaby/events-booking";
/// The edge, in the shape every other edge in the consumer has.
const EDGE_LINE: &str = "pub const REUSES_EVENTS_BOOKING_ID : str = \"turbobaby/events-booking\";";
/// Where the corpus floor lives. Read, not copied, so the two cannot disagree.
const ASSERTION_GATE: &str = "scripts/execute_t27_assertions.py";

/// Declarations the consumer held until 2026-09-22 and must never hold again. The first
/// seven restated the owner's bound; `SIBLING_MODULE_LINES` copied its
/// `ATTENDEE_SOURCE_LINES`; `HANDLE_BOUND_IS_ALREADY_OWNED_BY` named the owner outside
/// the edge block and is superseded by the edge; the last three moved to the owner and
/// are the OLD halves of `MOVED`.
const DROPPED: [&str; 12] = [
    "ATTENDEE_HANDLE_LOWER_BOUND_STATED",
    "ATTENDEE_HANDLE_UPPER_BOUND_STATED",
    "ATTENDEE_HANDLE_LOWER_BOUND_ENFORCED",
    "ATTENDEE_HANDLE_UPPER_BOUND_ENFORCED",
    "attendee_handle_admits",
    "attendee_handle_admits_as_stated",
    "implementation_matches_its_comment",
    "SIBLING_MODULE_LINES",
    "HANDLE_BOUND_IS_ALREADY_OWNED_BY",
    "ATTENDEE_HANDLE_TESTS_COVER_THE_LOWER_BOUND",
    "ATTENDEE_HANDLE_LENGTH_EXAMPLES_IN_TESTS",
    "SIBLING_MODULE_TESTS",
];

/// The test and the invariant that ran over the local copy. A check under either name
/// again means the copy it read came back with it.
const DROPPED_CHECKS: [&str; 2] = [
    "the_attendee_length_check_guards_one_end_of_a_two_ended_range",
    "the_stated_handle_range_and_the_enforced_one_are_not_the_same_range",
];

/// Name prefixes of the restated family. Any consumer declaration starting with one of
/// these is a copy under a new name, which the fixed list above cannot see. The first
/// three are the families the dropped names were written in; `SIBLING_MODULE_` was added
/// 2026-09-22 after a checker planted `SIBLING_MODULE_TEST_COUNT` beside the kept
/// `SIBLING_MODULE` (which has no trailing underscore and stays legal) and all three
/// tests passed. `ATTENDEE_SOURCE_` is the owner's own family for the module's measured
/// facts: `SUPPLIED` catches its exact names and not a variant of them. A name in a
/// family nobody has used yet -- that checker's `SIBLING_HANDLE_FLOOR_STATED` -- still
/// passes.
const RESTATED_PREFIXES: [&str; 4] = [
    "ATTENDEE_HANDLE_",
    "attendee_handle_",
    "SIBLING_MODULE_",
    "ATTENDEE_SOURCE_",
];

/// Facts the consumer held and the owner did not, moved on 2026-09-22 and renamed on
/// arrival to what was measured. Must equal the consumer's FACTS_MOVED_TO_THE_OWNER.
const MOVED: [(&str, &str); 3] = [
    (
        "ATTENDEE_HANDLE_TESTS_COVER_THE_LOWER_BOUND",
        "HANDLE_GAP_EXAMPLES_IN_TESTS",
    ),
    (
        "ATTENDEE_HANDLE_LENGTH_EXAMPLES_IN_TESTS",
        "HANDLE_OVER_MAX_EXAMPLES_IN_TESTS",
    ),
    ("SIBLING_MODULE_TESTS", "ATTENDEE_SOURCE_TESTS"),
];

/// Every declaration the consumer's EVENTS_BOOKING_SUPPLIES note names. Each must be
/// declared by the owner, named by the note, and declared by nobody else here.
const SUPPLIED: [&str; 11] = [
    "HANDLE_MIN_DECLARED",
    "HANDLE_MIN_ENFORCED",
    "HANDLE_MAX_DECLARED",
    "HANDLE_MAX_ENFORCED",
    "handle_is_accepted",
    "handle_bounds_agree",
    "length_is_in_the_handle_gap",
    "HANDLE_GAP_EXAMPLES_IN_TESTS",
    "HANDLE_OVER_MAX_EXAMPLES_IN_TESTS",
    "ATTENDEE_SOURCE_LINES",
    "ATTENDEE_SOURCE_TESTS",
];

/// The owner's invariant the consumer's replaced invariant points at by name.
const OWNER_INVARIANT: &str = "the_handle_lower_bound_is_declared_and_not_enforced";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{} is unreadable: {e}", path.display()))
}

/// Top-level declaration names, read at column zero: `pub const NAME :`, `pub fn NAME(`,
/// `test NAME {` and `invariant NAME`. `str::lines` drops a trailing `\r`, so a CRLF
/// checkout reads the same as an LF one.
fn declared(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let rest = line
            .strip_prefix("pub const ")
            .or_else(|| line.strip_prefix("pub fn "))
            .or_else(|| line.strip_prefix("test "))
            .or_else(|| line.strip_prefix("invariant "));
        if let Some(rest) = rest {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                out.push(name);
            }
        }
    }
    out
}

/// The body of a one-line `pub const NAME : str = "...";`.
fn str_const(text: &str, name: &str) -> String {
    let head = format!("pub const {name} : str = \"");
    let line = text
        .lines()
        .find(|l| l.starts_with(&head))
        .unwrap_or_else(|| panic!("{CONSUMER} does not declare {name} as a one-line str"));
    let body = &line[head.len()..];
    let end = body
        .rfind("\";")
        .unwrap_or_else(|| panic!("{name} does not close on its own line"));
    body[..end].to_string()
}

/// The elements of `pub const NAME : [N]str = [` ... `];`, one quoted element per line,
/// which is the only array layout the consumer uses.
fn str_array(text: &str, name: &str) -> Vec<String> {
    let head = format!("pub const {name} :");
    let mut lines = text.lines().skip_while(|l| !l.starts_with(&head));
    if lines.next().is_none() {
        panic!("{CONSUMER} does not declare {name}");
    }
    let mut items = Vec::new();
    for line in lines {
        if line.trim() == "];" {
            return items;
        }
        match (line.find('"'), line.rfind('"')) {
            (Some(open), Some(close)) if close > open => {
                items.push(line[open + 1..close].to_string())
            }
            _ => panic!("{name}: unexpected array line {line:?}"),
        }
    }
    panic!("{name}: the array is never closed");
}

/// Identifiers a prose note names: runs of `[A-Za-z0-9_]` that hold an underscore.
fn identifiers_in(note: &str) -> Vec<String> {
    note.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| w.contains('_') && w.chars().any(|c| c.is_ascii_alphabetic()))
        .map(str::to_string)
        .collect()
}

/// The corpus floor, read out of the assertion gate so this walk and that gate cannot
/// hold two different numbers for one rule (DECISIONS.md D16).
fn min_spec_files() -> usize {
    let text = read(&root().join(ASSERTION_GATE));
    let line = text
        .lines()
        .find(|l| l.starts_with("MIN_SPEC_FILES = "))
        .unwrap_or_else(|| panic!("{ASSERTION_GATE} no longer declares MIN_SPEC_FILES"));
    line["MIN_SPEC_FILES = ".len()..]
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("MIN_SPEC_FILES in {ASSERTION_GATE} is not a count: {e}"))
}

/// The owner's text, found by walking `specs/` for the one contract whose ID matches --
/// the same key a REUSES_ constant carries.
fn owner_text() -> String {
    let mut stack = vec![root().join("specs")];
    let mut seen = 0usize;
    let mut hits = Vec::new();
    let needle = format!("pub const ID : str = \"{OWNER_ID}\";");
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("specs/ is readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "t27") {
                seen += 1;
                let text = read(&path);
                if text.lines().any(|l| l == needle) {
                    hits.push(text);
                }
            }
        }
    }
    let floor = min_spec_files();
    assert!(
        seen >= floor,
        "the walk found {seen} specs, under the floor of {floor} in {ASSERTION_GATE}"
    );
    assert_eq!(
        hits.len(),
        1,
        "exactly one contract must declare ID {OWNER_ID}"
    );
    hits.remove(0)
}

/// The consumer holds none of the owner's bound: not under the names it used to, not
/// under the owner's own names, and not under a new name in the family the copy was
/// written in.
#[test]
fn person_naming_restates_none_of_the_handle_bound() {
    let text = read(&root().join(CONSUMER));
    let mine = declared(&text);
    // D16: an absence is only a measurement while the reader can still see what is
    // present. Both witnesses are declarations the consumer has held since it was
    // written, so a reader that cannot find them has gone blind, not clean.
    for witness in ["ID", "HANDLE_BOUND_OWNER_DECLARATIONS"] {
        assert!(
            mine.iter().any(|m| m == witness),
            "the declaration reader cannot see {witness} in {CONSUMER}; it has gone blind"
        );
    }
    for name in DROPPED.iter().chain(DROPPED_CHECKS.iter()) {
        assert!(
            !mine.iter().any(|m| m == name),
            "{CONSUMER} declares {name} again: that is a copy of what {OWNER_ID} owns"
        );
    }
    for name in &mine {
        for prefix in RESTATED_PREFIXES {
            assert!(
                !name.starts_with(prefix),
                "{CONSUMER} declares {name}: a new name in the family that restated \
                 {OWNER_ID}'s handle bound"
            );
        }
        assert!(
            !SUPPLIED.contains(&name.as_str()),
            "{CONSUMER} declares {name}, which it names as {OWNER_ID}'s"
        );
    }
}

/// The owner is named in the edge block, in the shape the consumer's other edges have,
/// and not by a constant of its own elsewhere in the file.
#[test]
fn person_naming_names_the_handle_owner_as_an_edge() {
    let text = read(&root().join(CONSUMER));
    assert!(
        text.lines().any(|l| l == EDGE_LINE),
        "{CONSUMER} does not declare REUSES_EVENTS_BOOKING_ID -- the owner of the \
         attendee handle bound is not named as an edge"
    );
    let holders: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("pub const ") && l.contains(&format!("= \"{OWNER_ID}\";")))
        .collect();
    assert_eq!(
        holders,
        vec![EDGE_LINE],
        "{CONSUMER} holds {OWNER_ID} in a constant other than its edge"
    );
}

/// Every name the consumer says it takes from the owner is declared THERE: the note's
/// names, the owner-declaration array, the moved facts and the invariant the consumer
/// points at. A rename in the owner turns this red instead of leaving the note dangling.
#[test]
fn every_name_person_naming_takes_from_the_handle_owner_is_declared_there() {
    let consumer = read(&root().join(CONSUMER));
    let theirs = declared(&owner_text());
    let owner_declares = |name: &str| theirs.iter().any(|t| t == name);

    let note = str_const(&consumer, "EVENTS_BOOKING_SUPPLIES");
    let named = identifiers_in(&note);
    for name in SUPPLIED {
        assert!(
            named.iter().any(|n| n == name),
            "EVENTS_BOOKING_SUPPLIES in {CONSUMER} does not name {name}"
        );
    }
    for name in &named {
        assert!(
            owner_declares(name),
            "EVENTS_BOOKING_SUPPLIES names {name}, and {OWNER_ID} declares no such thing"
        );
    }

    let array = str_array(&consumer, "HANDLE_BOUND_OWNER_DECLARATIONS");
    assert!(
        !array.is_empty(),
        "HANDLE_BOUND_OWNER_DECLARATIONS is empty (D16)"
    );
    for name in &array {
        assert!(
            SUPPLIED.contains(&name.as_str()) && owner_declares(name),
            "HANDLE_BOUND_OWNER_DECLARATIONS names {name}, which is not a name {OWNER_ID} \
             supplies and declares"
        );
    }

    let moved = str_array(&consumer, "FACTS_MOVED_TO_THE_OWNER");
    let expected: Vec<String> = MOVED
        .iter()
        .map(|(old, new)| format!("{old} is now {new}"))
        .collect();
    assert_eq!(
        moved, expected,
        "FACTS_MOVED_TO_THE_OWNER disagrees with the pairs this test pins"
    );
    for (_, new) in MOVED {
        assert!(
            owner_declares(new),
            "{new} was moved to {OWNER_ID} and it does not declare it"
        );
    }

    assert!(
        owner_declares(OWNER_INVARIANT),
        "{OWNER_ID} no longer declares invariant {OWNER_INVARIANT}, which {CONSUMER} cites"
    );
    assert!(
        consumer.contains(OWNER_INVARIANT),
        "{CONSUMER} no longer points at {OWNER_INVARIANT}"
    );
}
