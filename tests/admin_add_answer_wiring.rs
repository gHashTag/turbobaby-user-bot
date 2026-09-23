//! The admin's two Add buttons (a bike family, a bike unit), guarded as text
//! where the host cannot compile them.
//!
//! `src/lib.rs` gates `pub mod ui;` on `#[cfg(target_arch = "wasm32")]`, so
//! `cargo test` compiles nothing under `src/ui`. What is guarded here is the
//! order of events inside `src/ui/screens/admin_screen.rs`, and nothing
//! stronger is claimed.
//!
//! WHY IT EXISTS (measured 2026-09-24 on upstream main f0640f8, epic #31 AC4):
//! both Add handlers set «✅ Добавлена!» / «✅ Добавлен!» and emptied the form
//! BEFORE the request was sent, so the success tick stood on screen while the
//! server had not answered, and stayed there when the answer was a refusal
//! until the task replaced it. The refusal itself read «❌ Ошибка добавления»
//! with no reason, although the server answers a duplicate key with a 409 and
//! a sentence of its own (`src/api/bikes.rs`, `admin_create_bike`) and the
//! screen already reads such sentences elsewhere (the edit cards' «HTTP {st}:
//! ...», the access check's `reason`). A refused add also threw away everything
//! the owner had typed.
//!
//! The rules pinned below (AGENTS.md lesson 4, never report success before the
//! server confirms):
//! - each success line is written only after the request is sent, inside the
//!   arm that has seen a 2xx;
//! - the form is emptied in that same arm, so a refusal keeps the input;
//! - each failure line carries a reason read from the answer, in words the
//!   screen already uses (no new copy).
//!
//! This file imports nothing from the crate on purpose: it compiles against any
//! tree, so on a tree where the tick comes first it fails by CONTENT.

use std::fs;
use std::path::{Path, PathBuf};

const ADMIN: &str = "src/ui/screens/admin_screen.rs";

/// One Add flow: the POST URL literal that identifies it, its success line, and
/// one field its form empties after a success.
struct AddFlow {
    url: &'static str,
    success: &'static str,
    form_reset: &'static str,
}

const FLOWS: [AddFlow; 2] = [
    AddFlow {
        url: "format!(\"{}/api/admin/bikes\", api_base_url())",
        success: "status.set(\"✅ Добавлена!\".into());",
        form_reset: "family_key.set(String::new());",
    },
    AddFlow {
        url: "format!(\"{}/api/admin/bike-units\", api_base_url())",
        success: "status.set(\"✅ Добавлен!\".into());",
        form_reset: "unit_code.set(String::new());",
    },
];

const FAILURE_WORDS: &str = "Ошибка добавления";
const REASON_HELPER: &str = "admin_add_failure_reason(";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn source(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|e| panic!("{relative} must exist for this guard to mean anything: {e}"))
        .replace("\r\n", "\n")
}

/// The code of one line: the part before a `//` that is not inside a string
/// literal, so a URL or a comment that quotes a success line is never read as
/// the line itself.
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

fn code_lines() -> Vec<String> {
    source(ADMIN).lines().map(code_of).collect()
}

/// Every line index at which `needle` occurs.
fn lines_with(lines: &[String], needle: &str) -> Vec<usize> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains(needle))
        .map(|(i, _)| i)
        .collect()
}

/// The last index in `candidates` strictly below `limit`.
fn last_before(candidates: &[usize], limit: usize) -> Option<usize> {
    candidates.iter().copied().filter(|&i| i < limit).max()
}

/// The first index in `candidates` strictly above `limit`.
fn first_after(candidates: &[usize], limit: usize) -> Option<usize> {
    candidates.iter().copied().filter(|&i| i > limit).min()
}

/// Where one Add flow happens: the button's `onclick`, the line that builds its
/// POST URL, the request's `.send().await`, and the first `is_success()` that
/// reads the answer. Anchored on the URL that is followed by `.post(&url)`, so
/// the family list's GET of the same URL is never taken for the Add.
struct PostSite {
    onclick: usize,
    send: usize,
    verdict: usize,
}

fn post_site(lines: &[String], flow: &AddFlow) -> PostSite {
    let posts: Vec<usize> = lines_with(lines, flow.url)
        .into_iter()
        .filter(|&u| {
            lines[u + 1..(u + 4).min(lines.len())]
                .iter()
                .any(|l| l.contains(".post(&url)"))
        })
        .collect();
    // D16: exactly one Add per flow, or the guard is looking at nothing.
    assert_eq!(
        posts.len(),
        1,
        "`{}` followed by `.post(&url)` must occur exactly once in {ADMIN}, found {}",
        flow.url,
        posts.len()
    );
    let url = posts[0];
    let onclick = last_before(&lines_with(lines, "onclick:"), url)
        .unwrap_or_else(|| panic!("no onclick before the POST of `{}`", flow.url));
    let send = first_after(&lines_with(lines, ".send().await"), url)
        .unwrap_or_else(|| panic!("no send after the POST of `{}`", flow.url));
    let verdict = first_after(&lines_with(lines, "is_success()"), send)
        .unwrap_or_else(|| panic!("the answer to `{}` is never read", flow.url));
    PostSite {
        onclick,
        send,
        verdict,
    }
}

#[test]
fn success_is_reported_only_after_a_2xx() {
    let lines = code_lines();
    for flow in &FLOWS {
        let at = lines_with(&lines, flow.success);
        // D16: a guard that finds nothing passes vacuously.
        assert_eq!(
            at.len(),
            1,
            "`{}` must occur exactly once in {ADMIN}, found {}",
            flow.success,
            at.len()
        );
        let success_line = at[0];
        let site = post_site(&lines, flow);
        assert!(
            success_line > site.verdict,
            "`{}` (line {}) is written before the answer to its own POST is read \
             (onclick {}, send {}, is_success {}): the tick is shown before the server \
             answers (AGENTS.md lesson 4)",
            flow.success,
            success_line + 1,
            site.onclick + 1,
            site.send + 1,
            site.verdict + 1,
        );
    }
}

#[test]
fn a_refused_add_keeps_what_the_owner_typed() {
    let lines = code_lines();
    for flow in &FLOWS {
        let site = post_site(&lines, flow);
        let resets = lines_with(&lines, flow.form_reset);
        assert!(
            !resets.is_empty(),
            "`{}` is gone; the guard would be vacuous",
            flow.form_reset
        );
        let early: Vec<usize> = resets
            .iter()
            .copied()
            .filter(|&i| i > site.onclick && i < site.send)
            .collect();
        assert!(
            early.is_empty(),
            "`{}` empties the form before the request is sent (line(s) {:?}), so a \
             refused add throws the input away",
            flow.form_reset,
            early.iter().map(|i| i + 1).collect::<Vec<_>>()
        );
        assert!(
            resets.iter().any(|&i| i > site.verdict),
            "`{}` must empty the form after a confirmed add",
            flow.form_reset
        );
    }
}

#[test]
fn every_failed_add_names_its_reason() {
    let lines = code_lines();
    let failures = lines_with(&lines, FAILURE_WORDS);
    // D16: the two Add flows, and nothing else, carry these words.
    assert_eq!(
        failures.len(),
        FLOWS.len(),
        "`{FAILURE_WORDS}` must occur exactly once per Add flow, found {}",
        failures.len()
    );
    for i in failures {
        let line = &lines[i];
        assert!(
            line.contains("format!(") && line.contains(REASON_HELPER),
            "line {}: a failed add must say why, through {REASON_HELPER}..); found `{}`",
            i + 1,
            line.trim()
        );
    }
}

#[test]
fn the_reason_is_read_from_the_answer_in_words_the_screen_already_uses() {
    // The trailing newline keeps a helper that ends the file closable below.
    let text: String = code_lines().join("\n") + "\n";
    let at = text
        .find("async fn admin_add_failure_reason(")
        .expect("admin_screen.rs must declare admin_add_failure_reason");
    let rest = &text[at..];
    let body = &rest[..rest
        .find("\n}\n")
        .expect("the helper must close at column zero")];
    for needle in [".as_u16()", ".text().await", "HTTP {st}", "сеть/таймаут"] {
        assert!(
            body.contains(needle),
            "admin_add_failure_reason no longer contains `{needle}`:\n{body}"
        );
    }
    // No new copy: the words it uses are the edit cards' own.
    let edit_card_words = [
        "Err(format!(\"HTTP {st}\"))",
        "Err(format!(\"HTTP {st}: {snip}\"))",
        "Err(\"сеть/таймаут\".to_string())",
    ];
    for words in edit_card_words {
        assert!(
            text.contains(words),
            "the edit cards no longer say `{words}`; the add reason would be the only \
             place that does, which makes it new copy"
        );
    }
}
