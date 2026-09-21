//! The two `.t27` gates, run from a place CI already executes.
//!
//! `scripts/execute_t27_assertions.py` and `scripts/verify_t27_against_source.py`
//! belong in their own workflow jobs: they are stdlib-only, need no toolchain,
//! and answer in under a second each, so a false assertion should be reported
//! long before a Rust build finishes. That wiring is written and waiting; the
//! token that pushes this branch has no `workflow` scope, so it cannot land.
//!
//! Until it does, the gates run here. `cargo test --features backend` is
//! already a CI job, and a test that shells out to a checker is a shape this
//! tree uses elsewhere — `tests/ride_asset_wiring.rs` runs the Ride model
//! through `node` the same way. The cost is feedback speed, not coverage: the
//! same two gates, the same exit codes, seven minutes later than they need to be.
//!
//! What they check, and why a Rust test file is not a silly place for it:
//!
//! `--no-crosscheck` is passed on purpose, and it costs something worth naming.
//! With a compiler configured, the gate also compares every declaration and
//! function body against `t27c`'s own parse, and that comparison is what found
//! three defects in the pinned compiler (a body truncated at an inner `;`
//! comment, a dropped `while (c) : (step)` loop, a `packed struct` split in
//! two). There is no compiler in this job, and the gate refuses to run blind in
//! CI rather than skipping quietly — correctly. So the cross-check has no CI
//! home until the workflow jobs land; the 8 111 assertions still all execute
//! here, which is the part that had no home at all before.
//!
//! * The pinned compiler returns `TestBlock` and `InvariantBlock` nodes with
//!   EMPTY children — it discards test and invariant bodies — so all 8 111
//!   assertions in `specs/` are comments as far as the toolchain is concerned.
//!   The first gate is the only thing in this repository that runs them.
//! * The second binds contract constants to the Rust and SQL they describe.
//!   Before it there was exactly one mechanical tie between the corpus and the
//!   tree (`scripts/verify_fleet_seed.py` reads `market_profile.t27`); a
//!   contract checked against nothing drifts silently, and this one is the
//!   reason a stale line count in `customer_surface.t27` was caught the same
//!   hour it was created.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The interpreter to call, chosen by trying rather than by assuming.
///
/// CI runs Ubuntu, where `python3` is the name and `python` may not exist. A
/// Windows checkout has `python` and usually not `python3`. Guessing either one
/// makes this test a host assumption, which is the exact defect the commit
/// before this one spent its whole diff removing.
fn interpreter() -> &'static str {
    for candidate in ["python3", "python"] {
        let probed = Command::new(candidate).arg("--version").output();
        if matches!(probed, Ok(ref out) if out.status.success()) {
            return candidate;
        }
    }
    panic!(
        "neither `python3` nor `python` runs here, and both .t27 gates are Python. \
         CI installs one by default; a local checkout needs one on PATH."
    );
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn run_gate(script: &str, args: &[&str]) -> Output {
    let root = repo_root();
    let path = root.join("scripts").join(script);
    assert!(
        path.is_file(),
        "{} is missing. It is a gate, not an optional helper: removing it would \
         make this test pass by having nothing to check.",
        path.display()
    );
    Command::new(interpreter())
        .arg(&path)
        .args(args)
        .current_dir(&root)
        .output()
        .unwrap_or_else(|error| panic!("could not run {}: {error}", path.display()))
}

/// Both streams as one owned string. A gate may summarise on either, and the
/// borrow has to outlive the `Cow`s it came from.
fn both_streams(output: &Output) -> String {
    let mut joined = String::from_utf8_lossy(&output.stdout).into_owned();
    joined.push_str(String::from_utf8_lossy(&output.stderr).as_ref());
    joined
}

fn assert_gate_green(script: &str, args: &[&str]) {
    let output = run_gate(script, args);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{script} failed.\n\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    // A gate that runs, exits 0 and says nothing has usually found nothing to
    // look at. Both of these print a summary line on success, so an empty run
    // means the corpus moved out from under them.
    assert!(
        !stdout.trim().is_empty() || !stderr.trim().is_empty(),
        "{script} exited 0 and printed nothing, which is what an empty scan \
         looks like from the outside"
    );
}

/// Every assertion in the corpus is executed, and every one of them holds.
#[test]
fn every_t27_assertion_is_executed_and_holds() {
    assert_gate_green("execute_t27_assertions.py", &["--no-crosscheck", "-v"]);
}

/// Every bound contract constant still agrees with the source it constrains.
#[test]
fn every_t27_source_binding_holds() {
    assert_gate_green(
        "verify_t27_against_source.py",
        &["--require-git-tracked", "-v"],
    );
}

/// The gates are not reported as green by a run that checked nothing.
///
/// Both carry their own floors — a minimum file count, a minimum assertion
/// count, a minimum binding count — and both are written to fail closed. This
/// reads the summary they print and pins the shape of it, so a future edit that
/// turns a gate into a no-op cannot also turn this test into one.
#[test]
fn the_gates_report_a_corpus_and_not_an_empty_scan() {
    let assertions = run_gate("execute_t27_assertions.py", &["--no-crosscheck"]);
    let summary = both_streams(&assertions);
    assert!(
        summary.contains("spec(s)") && summary.contains("passed"),
        "the assertion gate no longer reports how much it ran:\n{summary}"
    );

    let bindings = run_gate("verify_t27_against_source.py", &["--require-git-tracked"]);
    let summary = both_streams(&bindings);
    assert!(
        summary.contains("binding"),
        "the source-binding gate no longer reports how much it ran:\n{summary}"
    );
}
