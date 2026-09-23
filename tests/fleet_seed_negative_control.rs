//! A negative control for `scripts/verify_fleet_seed.py`, the seed-provenance gate.
//!
//! That gate is the oldest mechanical tie in this repository between a published
//! number and the SQL that ships it: it pins `data/fleet_seed.json` to
//! `migrations/082_bikes_seed.sql` and to `specs/turbobaby/market_profile.t27`.
//! Until 2026-09-24 nothing showed it could fail. A gate that has only ever been
//! seen green is indistinguishable from one that compares nothing -- DECISIONS.md
//! D16 records a route gate that passed for three months while checking zero
//! modules -- so this file runs it three ways:
//!
//! 1. on the tracked tree, where it must pass and report the whole fleet;
//! 2. on an untouched COPY of the seed read through `--seed`, where it must pass
//!    too, so that a red run in (3) cannot be the option itself failing;
//! 3. on a copy with exactly one published figure changed -- the nmax-155 day rate,
//!    449 -> 450 -- where it must exit 1 and name that field.
//!
//! The copies live under `CARGO_TARGET_TMPDIR`, a build-output directory cargo
//! gives integration tests, under fixed names that each run overwrites. Nothing
//! outside `target/` is written and nothing is deleted.

// A panic is how a test reports failure. The restriction lints in Cargo.toml's
// [lints.clippy] exist for production code, as its own comment says.
#![allow(clippy::panic, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The line of `data/fleet_seed.json` that carries nmax-155's published money
/// fields. Asserted to occur exactly once before it is edited, so the plant can
/// never land on a different family or silently on none.
const NMAX_MONEY_LINE: &str =
    r#""base_rate_thb_day": 449, "deposit_thb": 3000, "monthly_low_season_thb": 5000,"#;
const NMAX_MONEY_LINE_PLANTED: &str =
    r#""base_rate_thb_day": 450, "deposit_thb": 3000, "monthly_low_season_thb": 5000,"#;

/// The fleet the tracked seed describes, as the gate prints it under `-v`. Written
/// out rather than matched loosely: a gate that parsed fewer rows would still say
/// "OK", and the counts are what show that it read all of them.
const WHOLE_FLEET: &str = "14 families (13 offered), 37 units";

/// Same probing rule as `tests/t27_gates_run.rs`: CI has `python3`, a Windows
/// checkout usually has only `python`.
fn interpreter() -> &'static str {
    for candidate in ["python3", "python"] {
        let probed = Command::new(candidate).arg("--version").output();
        if matches!(probed, Ok(ref out) if out.status.success()) {
            return candidate;
        }
    }
    panic!("neither `python3` nor `python` runs here, and the seed gate is Python");
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn run_seed_gate(extra: &[&str]) -> Output {
    let root = repo_root();
    let script = root.join("scripts").join("verify_fleet_seed.py");
    assert!(
        script.is_file(),
        "{} is missing; this control would pass by having nothing to run",
        script.display()
    );
    Command::new(interpreter())
        .arg(&script)
        .arg("-v")
        .args(extra)
        .current_dir(&root)
        // The success line carries an em dash; a Windows pipe would otherwise
        // encode it in the console code page and the text match would depend on
        // the host.
        .env("PYTHONIOENCODING", "utf-8")
        .output()
        .unwrap_or_else(|error| panic!("could not run {}: {error}", script.display()))
}

fn both_streams(output: &Output) -> String {
    let mut joined = String::from_utf8_lossy(&output.stdout).into_owned();
    joined.push_str(String::from_utf8_lossy(&output.stderr).as_ref());
    joined
}

/// A fixed path under cargo's per-target scratch directory for integration tests.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("fleet_seed_negative_control");
    fs::create_dir_all(&dir).expect("create the scratch directory under CARGO_TARGET_TMPDIR");
    dir.join(name)
}

fn tracked_seed() -> String {
    fs::read_to_string(repo_root().join("data").join("fleet_seed.json"))
        .expect("read data/fleet_seed.json")
}

#[test]
fn the_untouched_seed_passes_and_reports_the_whole_fleet() {
    let output = run_seed_gate(&[]);
    let text = both_streams(&output);
    assert_eq!(
        output.status.code(),
        Some(0),
        "the seed gate is red on the tracked tree:\n{text}"
    );
    assert!(
        text.contains("OK") && text.contains(WHOLE_FLEET),
        "the seed gate passed without reporting the fleet it read:\n{text}"
    );
}

#[test]
fn an_untouched_copy_read_through_the_option_passes_too() {
    let copy = scratch("untouched.json");
    fs::write(&copy, tracked_seed()).expect("write the untouched copy");
    let output = run_seed_gate(&["--seed", copy.to_str().expect("utf-8 scratch path")]);
    let text = both_streams(&output);
    assert_eq!(
        output.status.code(),
        Some(0),
        "an unmodified copy read through --seed is red, so a red planted run \
         below would prove nothing about the plant:\n{text}"
    );
    assert!(
        text.contains(WHOLE_FLEET),
        "the copy was accepted without the gate reporting the fleet it read:\n{text}"
    );
}

#[test]
fn one_changed_published_figure_turns_the_gate_red() {
    let seed = tracked_seed();
    assert_eq!(
        seed.matches(NMAX_MONEY_LINE).count(),
        1,
        "the nmax-155 money line is not where this control expects it; re-aim the \
         plant rather than let it land on another family or on nothing"
    );
    let planted = seed.replacen(NMAX_MONEY_LINE, NMAX_MONEY_LINE_PLANTED, 1);
    assert_ne!(planted, seed, "the plant changed nothing");

    let copy = scratch("planted_nmax_rate.json");
    fs::write(&copy, planted).expect("write the planted copy");
    let output = run_seed_gate(&["--seed", copy.to_str().expect("utf-8 scratch path")]);
    let text = both_streams(&output);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a seed whose nmax-155 day rate disagrees with migrations/082 was not \
         reported as a mismatch (exit 1):\n{text}"
    );
    assert!(
        text.contains("nmax-155.base_rate_thb_day"),
        "the gate went red without naming the field that was changed:\n{text}"
    );
}
