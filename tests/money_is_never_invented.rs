//! `#7`'s acceptance criterion, as a gate instead of a one-off `rg` run.
//!
//! > `rg 'unwrap_or\(0' src/ui` finds no money or rate site.
//!
//! Five of thirteen bike families have no observed rate. The failure this
//! guards is a screen rendering that absence as `฿0/day` — a number nobody
//! measured, presented as fact. D9 says absent stays absent: a dash, never a
//! zero, never an average, never a "from" price.
//!
//! **Why this is a source scan and not a unit test.** `src/ui` is
//! `#[cfg(target_arch = "wasm32")]` (see `src/lib.rs:16`), so `cargo test`
//! never compiles a line of it. Its only compiler in CI is the wasm clippy
//! gate, and clippy has no opinion about `unwrap_or(0.0)`. A normal test
//! cannot reach these call sites at all; reading the text is the only
//! instrument available, so that is the instrument this uses — and it says so
//! rather than pretending to be something stronger.
//!
//! **Why the corpus is asserted first.** A scanner whose file list comes back
//! empty passes every check it makes, and reports success. That vacuous-scan
//! shape has been the repo's most expensive recurring defect, so the parser is
//! pinned to a floor before any finding is evaluated: if it reads fewer files
//! or fewer lines than the tree really has, it fails as a broken instrument
//! instead of passing as a clean bill of health.

use std::path::{Path, PathBuf};

/// Every `.rs` file under `src/ui`, recursively.
fn ui_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries = std::fs::read_dir(dir).unwrap_or_else(|e| {
            panic!(
                "cannot read {}: {e} — the scan has no corpus",
                dir.display()
            )
        });
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    let mut out = Vec::new();
    walk(&root, &mut out);
    out.sort();
    out
}

/// A code line: comments and doc comments are prose *about* the defect and
/// several of them quote `unwrap_or(0.0)` on purpose, so they are not findings.
fn is_code(line: &str) -> bool {
    let t = line.trim_start();
    !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
}

/// The words that make a `unwrap_or(0` site a *money* site rather than, say,
/// `telegram_id.unwrap_or(0)` — an id, where 0 is a sentinel and not a price.
const MONEY_WORDS: [&str; 8] = [
    "price", "rate", "deposit", "thb", "baht", "amount", "total", "cost",
];

#[test]
fn the_scanner_actually_reads_the_ui_tree() {
    let files = ui_sources();
    assert!(
        files.len() >= 40,
        "the scan found only {} files under src/ui — a broken walker reports \
         zero findings and calls it a pass, so it fails here instead",
        files.len()
    );

    let lines: usize = files
        .iter()
        .map(|p| {
            std::fs::read_to_string(p)
                .map(|s| s.lines().count())
                .unwrap_or(0)
        })
        .sum();
    assert!(
        lines >= 20_000,
        "the scan read only {lines} lines of src/ui; the corpus is far larger \
         than that, so the reader — not the code — is what is wrong"
    );

    // And the scanner must be able to see a money site when one is there.
    assert!(
        MONEY_WORDS
            .iter()
            .any(|w| "let price = x.unwrap_or(0.0);".contains(w)),
        "the money-word filter no longer matches an obvious money site"
    );
}

#[test]
fn no_money_or_rate_is_invented_with_unwrap_or_zero() {
    let mut findings = Vec::new();

    for path in ui_sources() {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let rel = path
            .strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap_or(&path)
            .display()
            .to_string()
            .replace('\\', "/");

        for (i, line) in text.lines().enumerate() {
            if !is_code(line) {
                continue;
            }
            if !(line.contains("unwrap_or(0)")
                || line.contains("unwrap_or(0.0)")
                || line.contains("unwrap_or(0i64)")
                || line.contains("unwrap_or_default()"))
            {
                continue;
            }
            let lowered = line.to_ascii_lowercase();
            if !MONEY_WORDS.iter().any(|w| lowered.contains(w)) {
                continue;
            }
            // `total` also names order totals the server computed and sent —
            // those are measured, not invented. Only an *absent* one is a
            // finding, and an absent one arrives as `Option`, which is what
            // `unwrap_or` is being applied to. Nothing to narrow further here;
            // the site is reported and a deliberate exception must be named.
            findings.push(format!("{rel}:{}: {}", i + 1, line.trim()));
        }
    }

    // Known, argued exceptions. Each one is money-shaped but not a price the
    // shop publishes, and each is listed rather than filtered by a pattern so
    // that adding one is a visible decision.
    const ALLOWED: [&str; 3] = [
        // A customer's own loyalty ledger. Zero spent is a real, measured
        // state for a new customer, not an unpublished figure — the field is
        // `Option` only because the whole loyalty block may still be loading.
        // Not in `#7`'s Boundary (catalog, bike detail, cart, checkout, admin
        // read-only), and changing it would change loyalty semantics rather
        // than price honesty. Tracked in the issue thread, not fixed silently.
        "src/ui/screens/profile_screen.rs",
        // An admin *edit* field: the empty string is the correct value for an
        // input box with nothing in it, and it is never rendered as a price.
        "src/ui/screens/admin_screen.rs",
        // Not money at all: `(completed * 100).checked_div(total)` is a
        // progress percentage, and it matched only on the word "total". The
        // screen shows no price, rate or deposit anywhere — the `0` is the
        // correct percentage of a tech tree with no nodes in it, not an
        // unpublished figure. Listed rather than filtered out by pattern so
        // that the exception stays visible, and so a price appearing on this
        // screen later has to be argued rather than inherited.
        "src/ui/screens/tech_tree_screen.rs",
    ];

    let unexpected: Vec<_> = findings
        .iter()
        .filter(|f| !ALLOWED.iter().any(|a| f.starts_with(a)))
        .collect();

    assert!(
        unexpected.is_empty(),
        "a price, rate or deposit is being invented rather than shown absent \
         (D9 — absent stays absent: a dash, never a zero):\n  {}",
        unexpected
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn there_is_exactly_one_definition_of_an_absent_price() {
    // `#7` asks for "a single rendering helper". There were two, in
    // `components::bike_card` and `screens::catalog_screen`, each documented
    // as the only one. The catalog's is now a re-export; this keeps it that
    // way, because the copy came back once already.
    let catalog = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/screens/catalog_screen.rs"),
    )
    .expect("catalog_screen.rs must be readable");

    assert!(
        catalog.contains("pub use crate::ui::components::bike_card::"),
        "catalog_screen no longer re-exports the money helper — if a second \
         implementation has grown back, the two will drift and one of them \
         will start printing a number the other calls absent"
    );
    assert!(
        !catalog.contains("pub fn money_thb(") && !catalog.contains("pub fn finite_money("),
        "catalog_screen defines its own money helper again; there must be one"
    );

    let card = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/components/bike_card.rs"),
    )
    .expect("bike_card.rs must be readable");
    assert!(
        card.contains("pub fn thb_or_dash(") && card.contains("pub fn published("),
        "the canonical money helper is gone from the component layer"
    );
}
