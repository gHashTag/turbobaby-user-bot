//! A timed loop that logs nothing on an empty tick is not observable.
//!
//! Both garden sweeps were broken for months and nobody could tell: they
//! errored, logged a warning, and nothing 500-ed. Migration 072 fixed the
//! cause. But the first healthy run after that fix was being watched through
//! an instrument that could not distinguish "ran, nothing to do" from "stopped
//! running" — because every one of the seven timed loops wrote
//!
//!     Ok(0) => {}
//!
//! and an empty arm produces the same output as a dead task: none.
//!
//! This guards the CLASS, not the instance, in the same spirit as
//! `integration_garden_sweeps.rs`: presence-checking one loop would never have
//! caught this, because the defect is a shape that any new loop can repeat by
//! copying its neighbour.
//!
//! If you are adding a timed loop and this test fails, choose ONE and say which:
//!
//!   - log the empty case: `tracing::info!("<label>: nothing due (tick ok)")`
//!   - or keep it silent and justify that in place with a `// silent-tick:` comment
//!
//! Silence is sometimes right, and the review that produced this test found three
//! cases where it is. The notification worker ticks every 30 seconds -- a line per
//! tick is 2880 a day. And in two loops `Ok(0)` counts DELIVERIES, not work found:
//! `send_event_reminders` increments `sent` only after a successful Telegram send,
//! so `Ok(0)` there can mean every send failed. Logging "nothing due" in that case
//! announces health during a total outage, which is worse than saying nothing.
//!
//! So the rule this guards is not "always log". It is: **the empty tick is a
//! decision, and the decision is written down**. An unannotated bare arm is an
//! omission; an annotated one is a choice a reader can disagree with.

use std::fs;
use std::path::{Path, PathBuf};

/// Every `.rs` file under `src/`, recursively.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        // A directory we cannot read is not an absence of offenders. Fail loudly
        // rather than silently scanning nothing and reporting success.
        Err(e) => panic!("cannot read {}: {e}", dir.display()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_timed_loop_swallows_its_empty_tick() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);

    // Calibrate before measuring: a scan that finds no files is a broken scan,
    // not a clean tree.
    assert!(
        files.len() > 10,
        "source scan found only {} files under {} - suspect the scan, not the tree",
        files.len(),
        root.display()
    );

    let mut offenders = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path).expect("readable source file");
        for (i, line) in text.lines().enumerate() {
            // Accept `Ok(0) => {}` and `Ok(0) => (),` alike: an earlier draft matched
            // one exact string, so a one-character edit walked straight past the guard.
            let arm = line.trim().trim_end_matches(',');
            if arm == "Ok(0) => {}" || arm == "Ok(0) => ()" {
                // Silence is permitted when it is justified on the preceding lines.
                let lines: Vec<&str> = text.lines().collect();
                let justified = lines[i.saturating_sub(4)..i]
                    .iter()
                    .any(|prev| prev.contains("silent-tick:"));
                if !justified {
                    offenders.push(format!("{}:{}", path.display(), i + 1));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a timed loop is silent on an empty tick, so a running loop and a stopped \
         one produce identical logs:\n  {}\n\nEither log the empty case:\n  \
         Ok(0) => tracing::info!(\"<label>: nothing due (tick ok)\"),\n\nor keep it \
         silent and say why on the line above:\n  // silent-tick: <reason>",
        offenders.join("\n  ")
    );
}

/// The opposite defect: a voice where silence was decided.
///
/// `no_timed_loop_swallows_its_empty_tick` fails on a bare arm — silence where a
/// line was wanted. It says nothing about the reverse, and the reverse shipped:
/// `cart_abandonment.rs` holds TWO `Ok(0)` arms, a revert regex matched only the
/// first, and the second announced an empty tick every 300 seconds for four hours
/// in production before anyone measured it. The diff showed the file had changed.
/// It had changed halfway.
///
/// The rule this enforces: within one file the loops share their interval, so they
/// share the decision. If any arm there is justified silent, they all must be.
/// A file that both silences and announces is a half-applied edit until its author
/// says otherwise — and saying otherwise means a `silent-tick:` note on each.
#[test]
fn a_file_does_not_both_silence_and_announce() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);
    assert!(
        files.len() > 10,
        "source scan found only {} files",
        files.len()
    );

    let mut mixed = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path).expect("readable source file");
        let silent = text.contains("silent-tick:");
        let announcing: Vec<usize> = text
            .lines()
            .enumerate()
            .filter(|(_, l)| l.contains("Ok(0) => tracing::info!") || l.contains("Ok(0) => info!"))
            .map(|(i, _)| i + 1)
            .collect();
        if silent && !announcing.is_empty() {
            mixed.push(format!(
                "{} silences one tick and announces another at line(s) {:?}",
                path.display(),
                announcing
            ));
        }
    }

    assert!(
        mixed.is_empty(),
        "a file decided silence for one loop and a line for another:\n  {}\n\n\
         Loops in one file share an interval, so they share the decision. Either \
         silence them all with a `// silent-tick:` note each, or announce them all.",
        mixed.join("\n  ")
    );
}

/// What these two guards cannot judge, said out loud.
///
/// Borrowed from `timestamptz_columns_are_not_bound.rs`, which prints the three
/// column names it cannot decide about. The reasoning there applies here exactly:
/// **an uncovered case that goes unmentioned reads as coverage.**
///
/// Both guards above key on an `Ok(0)` arm. A timed loop whose sweep returns `()`
/// or `bool`, or that matches only `Err`, has no such arm — it is equally
/// unobservable and equally invisible to them. This test never fails; it reports,
/// so the reader of a green run knows what green did not cover.
#[test]
fn report_the_loops_these_guards_cannot_judge() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);
    assert!(
        files.len() > 10,
        "source scan found only {} files",
        files.len()
    );

    let mut spawned_with_interval = 0usize;
    let mut unjudged = Vec::new();

    for path in &files {
        let text = fs::read_to_string(path).expect("readable source file");
        // A timed loop, for this purpose, is a spawn whose body builds an interval.
        if !(text.contains("tokio::spawn") && text.contains("tokio::time::interval")) {
            continue;
        }
        spawned_with_interval += 1;
        let has_arm = text.contains("Ok(0)");
        if !has_arm {
            unjudged.push(path.display().to_string().replace('\\', "/"));
        }
    }

    println!(
        "  {} file(s) spawn an interval loop; {} carry no Ok(0) arm and are therefore \
         outside both guards above.",
        spawned_with_interval,
        unjudged.len()
    );
    for u in &unjudged {
        println!("    not judged: {u}");
    }
    if unjudged.is_empty() {
        println!("    every interval loop in src/ is within reach of the two guards.");
    }
    println!(
        "  Neither guard can see a sweep that returns () or bool, or matches only Err. \
         Silence about an uncovered case reads as coverage, so it is printed here."
    );
}
