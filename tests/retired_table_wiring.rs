//! Source-level guard: no shipped code may read a table the migrations drop.
//!
//! This repo has already shipped the defect this file exists to prevent, four
//! times over. Migration `083_drop_cannabis_catalog.sql` dropped six tables;
//! the handlers that read them stayed. `/admin/reviews` and
//! `/admin/strains/:id/lab-cert` answered 500 on every call. The customer-facing
//! review form did worse — it discarded the error and printed «🙌 Спасибо за
//! отзыв!» over a review the server had refused. `get_invitees` carried a
//! `LEFT JOIN garden_plants`, and Postgres does not shrug at a join onto a
//! relation that is not there: it fails the whole statement, so the invitee
//! list could not be produced at all.
//!
//! None of those were caught, because nothing was looking. The orphan check in
//! `src/db/mod.rs` looks the other way down the same road — it asks which
//! tables have no code — and it was itself held green by the defect: its
//! parser read `CREATE TABLE` and not `DROP TABLE`, so the six dropped tables
//! still counted as live, and the only thing keeping them off its orphan list
//! was the dead handlers naming them. Fixing the handlers alone would have
//! turned that gate red.
//!
//! So this is the other direction, and it is derived rather than listed: the
//! retired set is computed from the migrations themselves, in filename order,
//! so a table dropped by some future `091` gets the same protection without
//! anyone remembering to add it here.
//!
//! Three ways a table is reached, and all three are checked: a SeaORM entity
//! declaring `table_name`, raw SQL naming it after `FROM`/`JOIN`/`INTO`/
//! `UPDATE`, and a reference to the entity module that maps it.
//!
//! What survives is written down in [`SURVIVORS`] with a reason, and the list
//! is checked from both ends — a reference that is not on it fails, and an
//! entry on it that no longer matches anything also fails. An allowlist that
//! cannot go stale is a to-do list; one that can is a place things go to hide.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// A reference to a retired table that is deliberately still here, and why.
///
/// `path` is repo-relative. `table` is the retired table it reaches. Every
/// entry must still match something — see `no_survivor_entry_is_stale`.
struct Survivor {
    path: &'static str,
    table: &'static str,
    reason: &'static str,
}

const SURVIVORS: &[Survivor] = &[
    Survivor {
        path: "src/db/entities/strain.rs",
        table: "strains",
        reason: "The entity itself. It cannot go while the readers below still \
                 exist (the cart line waits on D9, the carousel reader on its test).",
    },
    Survivor {
        path: "src/api/cart.rs",
        table: "strains",
        reason: "`resolve_catalog_snapshot` prices a cart line of kind \
                 \"strain\". This is the open D9 question that was put to the \
                 owner and has not come back: a cart saved before 083 still \
                 carries such lines, and what the shop owes that customer is \
                 the owner's call, not a tidy-up. Note the failure mode — this \
                 one maps the error to 500 rather than swallowing it.",
    },
    Survivor {
        path: "src/db/mod.rs",
        table: "strains",
        reason: "`get_strains_of_day`. No production caller since 2026-09-25, \
                 when the carousel's buttons began to answer with the rental menu \
                 (owner: nothing cannabis-related anywhere), and no test caller \
                 since 2026-09-26, when tests/integration_strain_of_day.rs was \
                 rewritten to hold the carousel's retirement instead of its cap. \
                 It stays compiled because it is public; removing it is a \
                 separate change.",
    },
    Survivor {
        path: "src/db/strains.rs",
        table: "strains",
        reason: "The `From<strain::Model>` conversions the two readers above \
                 need. It holds no query of its own and goes when they go.",
    },
];

// ──────────────────────────────────────────────────────────────────
// Deriving the retired set from the migrations
// ──────────────────────────────────────────────────────────────────

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Read `CREATE TABLE` / `DROP TABLE` across all migrations in filename order
/// and return the tables that were created and then dropped without coming
/// back. Order matters: a table dropped by `083` and recreated by `090` is
/// live, and one created by `090` is live no matter what `083` said.
fn retired_tables() -> BTreeSet<String> {
    let mut entries: Vec<_> = fs::read_dir(repo_root().join("migrations"))
        .expect("read migrations dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "sql"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut live: BTreeSet<String> = BTreeSet::new();
    let mut retired: BTreeSet<String> = BTreeSet::new();
    for entry in entries {
        let content = fs::read_to_string(entry.path()).expect("read migration");
        for line in content.lines() {
            let t = line.trim();
            let lower = t.to_ascii_lowercase();
            if let Some(name) =
                ident_after(t, &lower, &["create table if not exists ", "create table "])
            {
                retired.remove(&name);
                live.insert(name);
            } else if let Some(name) =
                ident_after(t, &lower, &["drop table if exists ", "drop table "])
            {
                // Only a table this repo actually created counts as retired.
                // A defensive `DROP TABLE IF EXISTS` against something that
                // never existed here is not a signal about our own code.
                if live.remove(&name) {
                    retired.insert(name);
                }
            }
        }
    }
    retired
}

/// The identifier following one of `prefixes`, taken from the original-case
/// line so the returned name keeps its spelling.
fn ident_after(line: &str, lower: &str, prefixes: &[&str]) -> Option<String> {
    let rest = prefixes
        .iter()
        .find_map(|p| lower.strip_prefix(p).map(|r| &line[line.len() - r.len()..]))?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

// ──────────────────────────────────────────────────────────────────
// Reading the sources
// ──────────────────────────────────────────────────────────────────

/// Every `.rs` file under `src/`, repo-relative, sorted.
fn source_files() -> Vec<String> {
    let root = repo_root();
    let mut out = Vec::new();
    walk(&root.join("src"), &root, &mut out);
    out.sort();
    out
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, root, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(
                path.strip_prefix(root)
                    .expect("path under root")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

/// Blank out comments, keeping string literals and byte offsets intact.
///
/// Comments are where the record of a removal lives — this file's own header
/// names all six tables — so a guard that greps raw text would have to choose
/// between reading the history and being honest about it. Offsets are
/// preserved (comment bytes become spaces) so a hit still reports its real
/// line number.
fn strip_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0usize;
    while i < b.len() {
        // Raw string: r"...", r#"..."#, r##"..."##
        if b[i] == b'r' && i + 1 < b.len() && (b[i + 1] == b'"' || b[i + 1] == b'#') {
            let mut j = i + 1;
            let mut hashes = 0usize;
            while j < b.len() && b[j] == b'#' {
                hashes += 1;
                j += 1;
            }
            if j < b.len() && b[j] == b'"' {
                out.extend_from_slice(&b[i..=j]);
                j += 1;
                while j < b.len() {
                    if b[j] == b'"' && b[j + 1..].iter().take(hashes).all(|c| *c == b'#') {
                        let end = (j + 1 + hashes).min(b.len());
                        out.extend_from_slice(&b[j..end]);
                        j = end;
                        break;
                    }
                    out.push(b[j]);
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        // Ordinary string literal.
        if b[i] == b'"' {
            out.push(b[i]);
            i += 1;
            while i < b.len() {
                if b[i] == b'\\' && i + 1 < b.len() {
                    out.extend_from_slice(&b[i..i + 2]);
                    i += 2;
                    continue;
                }
                out.push(b[i]);
                let done = b[i] == b'"';
                i += 1;
                if done {
                    break;
                }
            }
            continue;
        }
        // Char literal — `'"'` would otherwise open a phantom string.
        if b[i] == b'\'' && i + 2 < b.len() {
            let len = if b[i + 1] == b'\\' { 4 } else { 3 };
            if i + len <= b.len() && b[i + len - 1] == b'\'' {
                out.extend_from_slice(&b[i..i + len]);
                i += len;
                continue;
            }
        }
        // Line comment.
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                out.push(b' ');
                i += 1;
            }
            continue;
        }
        // Block comment, nesting as Rust does.
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            let mut depth = 1usize;
            out.extend_from_slice(b"  ");
            i += 2;
            while i < b.len() && depth > 0 {
                if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
                    depth += 1;
                    out.extend_from_slice(b"  ");
                    i += 2;
                } else if b[i] == b'*' && i + 1 < b.len() && b[i + 1] == b'/' {
                    depth -= 1;
                    out.extend_from_slice(b"  ");
                    i += 2;
                } else {
                    out.push(if b[i] == b'\n' { b'\n' } else { b' ' });
                    i += 1;
                }
            }
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn is_word_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Offsets where `needle` appears as a whole word.
fn word_positions(hay: &str, needle: &str) -> Vec<usize> {
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = hay[from..].find(needle) {
        let at = from + rel;
        let before_ok = at == 0 || !is_word_byte(h[at - 1]);
        let after = at + n.len();
        let after_ok = after >= h.len() || !is_word_byte(h[after]);
        if before_ok && after_ok {
            out.push(at);
        }
        from = at + 1;
    }
    out
}

fn line_of(src: &str, offset: usize) -> usize {
    src[..offset].bytes().filter(|c| *c == b'\n').count() + 1
}

/// SQL keywords that put the next identifier in table position.
const TABLE_POSITION: [&str; 5] = ["from", "join", "into", "update", "table"];

/// True when the word at `offset` sits in SQL table position — i.e. the
/// previous word is one of [`TABLE_POSITION`]. This is what separates
/// `FROM strains` from the ETag cache key `"strains"` in `src/api/cache.rs`,
/// which is a string that happens to share the name and reaches no database.
fn in_table_position(hay: &str, offset: usize) -> bool {
    let before = &hay[..offset];
    let trimmed = before.trim_end_matches([' ', '\t', '\r', '\n', '\\']);
    // Walk back by chars, not bytes: these sources are full of `—` and
    // Cyrillic, and `rfind(..) + 1` lands inside a multi-byte char.
    let prev: String = trimmed
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    TABLE_POSITION.contains(&prev.to_ascii_lowercase().as_str())
}

/// True when the whole word `module` at `at` is being used as a module path
/// rather than as an ordinary identifier that happens to share the name.
///
/// Three shapes count, and the third is the one that hides: `strain::Entity`,
/// `crate::db::entities::strain`, and a member of a braced group —
/// `use crate::db::entities::{accessory, strain, tea_product};` — where the
/// name is flanked by a comma and a brace and by nothing that looks like a
/// path at all. The brace case is confirmed by walking back to the opening
/// `entities::{` and checking the group has not closed in between.
fn is_module_reference(code: &str, module: &str, at: usize) -> bool {
    if code[at + module.len()..].starts_with("::") {
        return true;
    }
    let before = code[..at].trim_end();
    if before.ends_with("::") {
        return true;
    }
    if before.ends_with('{') || before.ends_with(',') {
        if let Some(open) = before.rfind("entities::{") {
            return !before[open..].contains('}');
        }
    }
    false
}

/// Retired table → the entity module that maps it, e.g. `strains` → `strain`.
fn entity_modules_for(retired: &BTreeSet<String>) -> BTreeMap<String, String> {
    let dir = repo_root().join("src/db/entities");
    let mut out = BTreeMap::new();
    for entry in fs::read_dir(&dir).expect("read entities dir").flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|x| x == "rs") {
            let src = fs::read_to_string(&path).expect("read entity");
            for table in retired {
                if src.contains(&format!("table_name = \"{table}\"")) {
                    let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
                    out.insert(table.clone(), stem);
                }
            }
        }
    }
    out
}

// ──────────────────────────────────────────────────────────────────
// The checks
// ──────────────────────────────────────────────────────────────────

/// One reference to a retired table, as reported to a human.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Hit {
    path: String,
    table: String,
    line: usize,
    how: &'static str,
}

/// Every reference from shipped `src/` code to a retired table.
fn find_hits() -> Vec<Hit> {
    let retired = retired_tables();
    assert!(
        retired.contains("strains"),
        "the migrations no longer retire `strains`; if that is deliberate, this \
         guard's premise has changed and its survivors need rereading"
    );
    let modules = entity_modules_for(&retired);
    let mut hits = Vec::new();

    for path in source_files() {
        let raw = fs::read_to_string(repo_root().join(&path)).expect("read source");
        let code = strip_comments(&raw);
        let is_entity_file = path.starts_with("src/db/entities/");

        for table in &retired {
            // A. The entity that maps the table.
            if is_entity_file {
                let decl = format!("table_name = \"{table}\"");
                if let Some(at) = code.find(&decl) {
                    hits.push(Hit {
                        path: path.clone(),
                        table: table.clone(),
                        line: line_of(&code, at),
                        how: "SeaORM entity declares `table_name`",
                    });
                }
            }

            // B. Raw SQL naming it in table position.
            for at in word_positions(&code, table) {
                if in_table_position(&code, at) {
                    hits.push(Hit {
                        path: path.clone(),
                        table: table.clone(),
                        line: line_of(&code, at),
                        how: "raw SQL names it in table position",
                    });
                    break;
                }
            }

            // C. A reference to the entity module that maps it. The entity
            //    file itself is its own definition, not a reader of it.
            if let Some(module) = modules.get(table) {
                if is_entity_file {
                    continue;
                }
                if let Some(at) = word_positions(&code, module)
                    .into_iter()
                    .find(|p| is_module_reference(&code, module, *p))
                {
                    hits.push(Hit {
                        path: path.clone(),
                        table: table.clone(),
                        line: line_of(&code, at),
                        how: "reads the entity module for the table",
                    });
                }
            }
        }
    }
    hits.sort();
    hits.dedup_by(|a, b| a.path == b.path && a.table == b.table);
    hits
}

#[test]
fn no_shipped_code_reads_a_retired_table() {
    let hits = find_hits();
    let unexpected: Vec<&Hit> = hits
        .iter()
        .filter(|h| {
            !SURVIVORS
                .iter()
                .any(|s| s.path == h.path && s.table == h.table)
        })
        .collect();

    assert!(
        unexpected.is_empty(),
        "shipped code reads a table the migrations drop. Such a query does not \
         return \"no rows\" — Postgres fails the statement, so the endpoint \
         fails whole. Either retire the code or add it to SURVIVORS with a \
         reason.\n{}",
        unexpected
            .iter()
            .map(|h| format!("  {}:{} — `{}` ({})", h.path, h.line, h.table, h.how))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn no_survivor_entry_is_stale() {
    let hits = find_hits();
    let orphaned: Vec<&Survivor> = SURVIVORS
        .iter()
        .filter(|s| !hits.iter().any(|h| h.path == s.path && h.table == s.table))
        .collect();
    assert!(
        orphaned.is_empty(),
        "a SURVIVORS entry no longer matches anything. The code it excused is \
         gone, so the excuse should go with it — otherwise the list stops being \
         a to-do list and becomes a place things hide.\n{}",
        orphaned
            .iter()
            .map(|s| format!("  {} — `{}`", s.path, s.table))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn every_survivor_carries_a_reason() {
    for s in SURVIVORS {
        assert!(
            s.reason.len() > 40,
            "{} excuses `{}` without saying why",
            s.path,
            s.table
        );
    }
}

// ──────────────────────────────────────────────────────────────────
// The reader itself, checked against hand-written cases
// ──────────────────────────────────────────────────────────────────

#[test]
fn comments_are_stripped_and_strings_are_not() {
    let src = r#"
// FROM strains
/* FROM strains */
let sql = "SELECT * FROM strains";
"#;
    let out = strip_comments(src);
    assert_eq!(
        word_positions(&out, "strains")
            .iter()
            .filter(|p| in_table_position(&out, **p))
            .count(),
        1,
        "the SQL string must survive and both comments must not:\n{out}"
    );
    assert_eq!(
        out.lines().count(),
        src.lines().count(),
        "line numbering must be preserved"
    );
}

#[test]
fn a_bare_mention_is_not_a_table_reference() {
    // `src/api/cache.rs` holds ETag cache keys that share the name.
    let src = r#"h.remove("strains"); h.remove("strains_admin");"#;
    let out = strip_comments(src);
    assert!(!word_positions(&out, "strains")
        .iter()
        .any(|p| in_table_position(&out, *p)));
}

#[test]
fn a_join_across_a_line_break_is_found() {
    // The query this guard was written for wrapped exactly like this.
    let src = "let sql = \"SELECT x\n        FROM referral_events re\n        LEFT JOIN\n            garden_plants gp ON gp.user_id = re.referred_id\";";
    let out = strip_comments(src);
    assert!(word_positions(&out, "garden_plants")
        .iter()
        .any(|p| in_table_position(&out, *p)));
}

#[test]
fn a_longer_name_is_not_a_hit_for_its_prefix() {
    let src = r#"let sql = "SELECT * FROM strain_reviews";"#;
    let out = strip_comments(src);
    assert!(!word_positions(&out, "strains")
        .iter()
        .any(|p| in_table_position(&out, *p)));
    assert!(word_positions(&out, "strain_reviews")
        .iter()
        .any(|p| in_table_position(&out, *p)));
}

#[test]
fn the_three_shapes_of_a_module_reference_are_all_seen() {
    // This check was written once with the offsets confused — it compared the
    // position of `entities` against the word positions of `strain` — and so
    // it recognised nothing at all while reporting green. The mutation that
    // caught it is the first case below.
    for src in [
        "use crate::db::entities::strain;",
        "strain::Entity::find_by_id(id)",
        "use crate::db::entities::{accessory, strain, tea_product};",
    ] {
        let found = word_positions(src, "strain")
            .into_iter()
            .any(|p| is_module_reference(src, "strain", p));
        assert!(found, "module reference not recognised in: {src}");
    }
    for src in [
        "let strain = row.name;",
        "m.strain_name.clone()",
        "use crate::db::entities::{accessory, tea_product};\nlet v = vec![a, strain_id];",
    ] {
        let found = word_positions(src, "strain")
            .into_iter()
            .any(|p| is_module_reference(src, "strain", p));
        assert!(
            !found,
            "ordinary identifier read as a module path in: {src}"
        );
    }
}

#[test]
fn the_retired_set_is_derived_not_assumed() {
    let retired = retired_tables();
    for t in [
        "strains",
        "strain_reviews",
        "lab_certificates",
        "garden_plants",
        "garden_rewards",
        "garden_config",
    ] {
        assert!(retired.contains(t), "083 drops `{t}` and it is not retired");
    }
    for t in ["orders", "bikes", "bike_units", "referral_events"] {
        assert!(
            !retired.contains(t),
            "`{t}` is live and must not be retired"
        );
    }
}
