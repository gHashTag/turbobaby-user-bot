//! The market's clock has one home, and every reader goes through it (D18).
//!
//! `src/lib.rs` gates `pub mod ui;` on `#[cfg(target_arch = "wasm32")]`, so
//! `cargo test` never compiles `src/ui`. Two of the seven sites this guards
//! live there (`events_screen.rs`, `admin_screen.rs`) and no compiled assertion
//! can reach them — the same blackout that let two cart converters drift apart
//! in `cart_kind_wiring.rs`. So this reads `src/` as text, the pattern
//! `customer_surface_wiring.rs` and `price_provenance_wiring.rs` already use
//! and CI already runs.
//!
//! Why a guard and not just the edit: the defect being prevented is not a wrong
//! offset, it is a *restated* one. Measured 2026-09-14 the tree held seven
//! spellings of "UTC+7" across seven files — `7 * 3600`, `Duration::hours(7)`,
//! `BANGKOK_OFFSET_SECONDS`, `"+07:00"` — and D18's own list of them named only
//! five. That is the cost in miniature: nobody can enumerate a rule that is
//! written a different way in every file, so the list in the decision document
//! was wrong and stayed wrong. One home makes the set countable.

use std::fs;
use std::path::PathBuf;

/// The module that is allowed to know the offset. Everything else asks it.
const HOME: &str = "src/trios/market.rs";

/// The seven files measured as reading the shop clock, which must now read it
/// from the profile. Listed explicitly so that *removing* a call site is as
/// visible as adding one — a file that silently stops consulting the market is
/// how the next ambient literal gets in.
const WIRED: [&str; 7] = [
    "src/api/happy_hour.rs",
    "src/api/orders.rs",
    "src/api/share.rs",
    "src/api/events.rs",
    "src/promo.rs",
    "src/ui/screens/events_screen.rs",
    "src/ui/screens/admin_screen.rs",
];

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{rel} is readable: {e}"))
}

/// Every `.rs` file under `src/`, as (repo-relative label, contents). A walk
/// rather than a fixed list because the invariant is *nowhere*, and a fixed
/// list cannot say that about a file somebody adds tomorrow.
fn rust_sources() -> Vec<(String, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let parent = root.parent().expect("src has a parent").to_path_buf();
    let mut stack = vec![root];
    let mut out = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("src/ is readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let label = path
                    .strip_prefix(&parent)
                    .unwrap_or(&path)
                    .display()
                    .to_string()
                    .replace('\\', "/");
                out.push((label, fs::read_to_string(&path).expect("readable")));
            }
        }
    }
    assert!(
        out.len() > 20,
        "the walk found {} files — it is not reaching src/",
        out.len()
    );
    out
}

/// The part of a file that ships: everything before the first `#[cfg(test)]`.
///
/// Test fixtures legitimately name an offset — `trios::calendar` takes one as a
/// parameter and its tests pass `"+07:00"` to prove the parameter is honoured,
/// which is the opposite of hardcoding a market. Rust convention puts `mod
/// tests` last, so truncating there separates the two without a per-file
/// allowlist. A file that opened with `#[cfg(test)]` would go dark, which is
/// why [`every_clock_reader_goes_through_the_market_profile`] pins the readers
/// by name as well.
fn shipped(source: &str) -> &str {
    match source.find("#[cfg(test)]") {
        Some(i) => &source[..i],
        None => source,
    }
}

/// A line with its `//` comment removed, so prose *describing* the old literal
/// is never mistaken for the literal. Every site replaced here kept a comment
/// saying what it used to be, so without this the guard would fail on its own
/// documentation. Naive by intent: it does not track string literals, and no
/// line involved here puts `//` inside one.
fn code_of(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Is this a quoted RFC 3339 numeric offset, e.g. `"+07:00"`? Matched by shape
/// rather than by the one value Thailand uses, so relocating the deployment
/// cannot quietly re-introduce the literal in a new disguise.
fn has_quoted_offset_literal(code: &str) -> bool {
    let b = code.as_bytes();
    b.windows(8).any(|w| {
        w[0] == b'"'
            && (w[1] == b'+' || w[1] == b'-')
            && w[2].is_ascii_digit()
            && w[3].is_ascii_digit()
            && w[4] == b':'
            && w[5].is_ascii_digit()
            && w[6].is_ascii_digit()
            && w[7] == b'"'
    })
}

#[test]
fn the_market_offset_has_exactly_one_home() {
    // Each needle is one of the spellings measured in the tree on 2026-09-14.
    // `east_opt` is included because constructing a `FixedOffset` at all is the
    // act being centralised: a caller that builds its own has, by definition,
    // decided the market has a single offset without consulting whether it does.
    let needles: [(&str, &str); 4] = [
        ("east_opt", "builds its own FixedOffset"),
        ("7 * 3600", "restates the offset in seconds"),
        (
            "hours(7)",
            "shifts the instant instead of reading a wall clock",
        ),
        ("BANGKOK_OFFSET", "names a city where a market belongs"),
    ];
    let mut offences: Vec<String> = Vec::new();
    for (label, source) in rust_sources() {
        if label == HOME {
            continue;
        }
        for (n, line) in shipped(&source).lines().enumerate() {
            let code = code_of(line);
            for (needle, why) in needles {
                if code.contains(needle) {
                    offences.push(format!("{label}:{} — {why} (`{needle}`)", n + 1));
                }
            }
            if has_quoted_offset_literal(code) {
                offences.push(format!(
                    "{label}:{} — writes an RFC 3339 offset as a literal",
                    n + 1
                ));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "the market's UTC offset is spelled outside {HOME}:\n  {}\n\
         Ask `trios::market::MARKET` instead. Seven files once each had their \
         own spelling and D18's list of them was two short — that is what an \
         uncountable rule costs (D15, D18).",
        offences.join("\n  ")
    );
}

#[test]
fn every_clock_reader_goes_through_the_market_profile() {
    for rel in WIRED {
        let source = read(rel);
        assert!(
            source.contains("trios::market::MARKET") || source.contains("market::{MarketClock"),
            "{rel} reads a clock but never consults the declared market (D18). \
             If this file legitimately stopped needing the time, remove it from \
             WIRED in this test and say so — do not let it drop out silently."
        );
    }
}

#[test]
fn the_wasm_screens_are_wired_where_no_compiler_will_check() {
    // These two are the whole reason this file walks source instead of calling
    // functions: `cargo test` does not compile `src/ui`, so a revert here is
    // invisible to every other gate in CI.
    let events = read("src/ui/screens/events_screen.rs");
    assert!(
        events.contains("MARKET") && events.contains("fixed_offset()"),
        "events_screen.rs no longer derives its offset from the market profile"
    );
    // Comment-stripped: the replacement's own doc comment explains the
    // `unreachable!` it removed, and a raw substring search condemns it for
    // saying so. Caught by this guard on its first run.
    let panicking = shipped(&events)
        .lines()
        .enumerate()
        .find(|(_, l)| code_of(l).contains("unreachable!"));
    assert!(
        panicking.is_none(),
        "events_screen.rs:{} reintroduced an `unreachable!` around the offset. \
         It was a fair reading when the argument was a literal `7 * 3600`; with \
         the offset coming from a profile the arm is reachable, and in a WASM \
         bundle a panic is a blank screen.",
        panicking.map_or(0, |(n, _)| n + 1)
    );

    let admin = read("src/ui/screens/admin_screen.rs");
    // The prefix, not `rfc3339_offset()`: the call is `rfc3339_offset_or_utc()`
    // and either accessor is a legitimate way to reach the profile.
    assert!(
        admin.contains("rfc3339_offset"),
        "admin_screen.rs no longer builds the event-form offset from the market \
         profile — an admin's start time would be stamped with a fixed zone the \
         deployment may not be in"
    );
}

/// The Rust profile says what the seed says.
///
/// `src/trios/market.rs` carried its own pin for this — four `assert_eq!`s
/// against `"Asia/Bangkok"`, `7`, `false`, `"+07:00"` — under a doc comment
/// claiming the values were "pinned to the seed's `market` block". They were
/// not pinned to anything. They were a hand-typed fourth copy of a fact that
/// already existed in three places (`data/fleet_seed.json`,
/// `specs/turbobaby/market_profile.t27`, and the Rust const), and editing the
/// seed would leave that test passing on the old numbers while calling itself
/// the thing that would have caught it.
///
/// `scripts/verify_fleet_seed.py` pins seed ↔ `.t27`. This pins seed ↔ Rust.
/// Together the three agree or something is red; nobody has to remember a
/// fourth list.
#[test]
fn the_rust_market_profile_matches_the_seed() {
    let seed: serde_json::Value =
        serde_json::from_str(&read("data/fleet_seed.json")).expect("parse fleet seed");
    let market = seed
        .get("market")
        .and_then(serde_json::Value::as_object)
        .expect("data/fleet_seed.json has a `market` block (D18)");

    use turbobaby_bot::trios::market::{MARKET, MARKET_DIALING};

    assert_eq!(
        market.get("timezone_name").and_then(|v| v.as_str()),
        Some(MARKET.timezone_name),
        "MARKET.timezone_name disagrees with the seed"
    );
    assert_eq!(
        market.get("utc_offset_hours").and_then(|v| v.as_i64()),
        Some(i64::from(MARKET.utc_offset_hours)),
        "MARKET.utc_offset_hours disagrees with the seed"
    );
    assert_eq!(
        market.get("dst_observed").and_then(|v| v.as_bool()),
        Some(MARKET.dst_observed),
        "MARKET.dst_observed disagrees with the seed"
    );
    assert_eq!(
        market.get("default_calling_code").and_then(|v| v.as_str()),
        Some(MARKET_DIALING.default_calling_code),
        "MARKET_DIALING.default_calling_code disagrees with the seed"
    );
}
