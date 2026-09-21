//! Third occurrence of one defect: epoch milliseconds written into a
//! `TIMESTAMPTZ` column.
//!
//! ```text
//! 069  streak_last_watered_at  → watering 500-ed
//! 072  reminder_sent_at        → both garden sweeps failed on every tick
//! ???  updated_at              → POST /api/garden/plants/choose 500-ed
//! ```
//!
//! Every time on `garden_plants` / `garden_rewards` is epoch-millis `BIGINT`,
//! so `chrono::Utc::now().timestamp_millis()` is the reflex when writing one.
//! When the target column happens to be `TIMESTAMPTZ`, Postgres does not
//! coerce — it rejects the whole statement:
//!
//! ```text
//! column "updated_at" is of type timestamp with time zone
//!   but expression is of type bigint
//! ```
//!
//! `integration_garden_sweeps.rs` guards a hand-written list of columns, which
//! is why it did not catch `updated_at`: the list only had the columns already
//! known to be broken. This guard needs no list. It asks the database which
//! columns are `TIMESTAMPTZ` and then fails if the source binds a parameter to
//! any of them, because in this codebase a bound parameter for a time column
//! means epoch millis. Comparisons and writes are both caught.
//!
//! If you genuinely need to bind a real timestamp, say so on the line above:
//!
//! ```text
//! // tstz-bind-ok: sea_orm binds a DateTimeWithTimeZone here, not millis
//! ```
//!
//! Run with:
//! ```sh
//! DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/woody_test \
//!   cargo test --features backend --test timestamptz_columns_are_not_bound -- --ignored
//! ```

#![cfg(feature = "backend")]

mod common;

use common::make_app_with_db;
use sea_orm::{ConnectionTrait, DbBackend, Statement};

/// Walk `src/`, returning every `.rs` file's path and contents.
fn rust_sources(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(text) = std::fs::read_to_string(&path) {
                out.push((path.display().to_string().replace('\\', "/"), text));
            }
        }
    }
}

/// Does `line` bind a parameter to exactly the column `col`?
///
/// Matches `<col> = $` only where `col` starts at an identifier boundary, so a
/// column name that is a suffix of a longer one (`at` inside
/// `last_watered_at`) cannot masquerade as it.
fn binds_column(line: &str, col: &str) -> bool {
    let needle = format!("{col} = $");
    let mut from = 0;
    while let Some(rel) = line[from..].find(&needle) {
        let start = from + rel;
        let prev_ok = line[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric() && c != '_');
        if prev_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

#[test]
fn a_column_name_that_is_a_suffix_of_another_is_not_confused_for_it() {
    // The exact four false positives this guard used to produce.
    assert!(!binds_column("SET last_watered_at = $4,", "at"));
    assert!(!binds_column("SET harvested_at = $1 WHERE id = $2", "at"));
    assert!(!binds_column(
        "UPDATE garden_rewards SET expiry_nudge_sent_at = $1",
        "sent_at"
    ));
    // …while the real thing still matches, at either kind of boundary.
    assert!(binds_column("UPDATE queen_report SET at = $1", "at"));
    assert!(binds_column("(at = $1)", "at"));
    assert!(binds_column("SET sent_at = $2", "sent_at"));
    assert!(binds_column("SET updated_at = $1", "updated_at"));
}

#[tokio::test]
#[ignore]
async fn no_timestamptz_column_is_written_from_a_bound_parameter() {
    let Some((_app, db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };

    // Only names that are TIMESTAMPTZ in *every* table carrying them. The scan
    // matches SQL text, which does not tell us the table, and three names are
    // one type here and another there — `reminder_sent_at` is BIGINT on
    // garden_plants and TIMESTAMPTZ on carts. Judging those by name alone
    // produces false accusations, and a guard that cries wolf gets deleted.
    let rows = db
        .orm
        .query_all(Statement::from_string(
            DbBackend::Postgres,
            "SELECT column_name, \
                    count(*) FILTER (WHERE data_type = 'timestamp with time zone') AS tstz, \
                    count(*) AS total \
             FROM information_schema.columns \
             WHERE table_schema = 'public' \
             GROUP BY column_name \
             HAVING count(*) FILTER (WHERE data_type = 'timestamp with time zone') > 0"
                .to_string(),
        ))
        .await
        .expect("information_schema query");

    let mut columns = Vec::new();
    let mut ambiguous = Vec::new();
    for r in &rows {
        let Ok(name) = r.try_get::<String>("", "column_name") else {
            continue;
        };
        let tstz: i64 = r.try_get("", "tstz").unwrap_or(0);
        let total: i64 = r.try_get("", "total").unwrap_or(0);
        if tstz == total {
            columns.push(name);
        } else {
            ambiguous.push(name);
        }
    }
    // Say what is not covered. A bound cap that goes unmentioned reads as full
    // coverage, which is the same class of error this file is about.
    if !ambiguous.is_empty() {
        ambiguous.sort();
        eprintln!(
            "NOT COVERED — these names are TIMESTAMPTZ in one table and another \
             type in a second, so the text scan cannot judge them: {}",
            ambiguous.join(", ")
        );
    }
    // A scan that reads nothing reports a clean tree — the same error the
    // sweeps themselves had. Assert the inputs are non-trivial first.
    assert!(
        columns.len() >= 10,
        "expected the schema to have many TIMESTAMPTZ columns, found {} — is \
         the test database migrated?",
        columns.len()
    );

    let mut sources = Vec::new();
    rust_sources(std::path::Path::new("src"), &mut sources);
    assert!(
        sources.len() > 10,
        "walked src/ and found only {} files — the scan is broken, not the tree",
        sources.len()
    );

    let mut offences = Vec::new();
    for (path, text) in &sources {
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            for col in &columns {
                // `<col> = $` — a bound parameter, as opposed to `= NOW()`.
                //
                // Anchored at an identifier boundary, because this schema has
                // columns literally named `at` (queen_report, queen_transcript)
                // and `sent_at` (promo_deliveries). A plain substring test made
                // every `last_watered_at = $4` and `expiry_nudge_sent_at = $1`
                // read as one of those — four false accusations against BIGINT
                // columns that bind epoch millis entirely correctly. A guard
                // that cries wolf gets deleted, so it has to be precise.
                if !binds_column(line, col) {
                    continue;
                }
                // An explicit, written-down exemption on one of the preceding
                // lines makes this a decision rather than an oversight.
                let exempt = lines[i.saturating_sub(3)..=i]
                    .iter()
                    .any(|l| l.contains("tstz-bind-ok:"));
                if !exempt {
                    offences.push(format!("{path}:{}: {col} = $ … in {}", i + 1, line.trim()));
                }
            }
        }
    }

    assert!(
        offences.is_empty(),
        "a TIMESTAMPTZ column is being written or compared from a bound \
         parameter. In this codebase that parameter is epoch milliseconds, and \
         Postgres rejects the statement rather than coercing it — the endpoint \
         500s. Use NOW(), or bind a real timestamp and mark the line with \
         `// tstz-bind-ok: <reason>`:\n  {}",
        offences.join("\n  ")
    );
}
