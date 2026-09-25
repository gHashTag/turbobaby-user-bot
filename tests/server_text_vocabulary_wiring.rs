//! Source-level guard: the text the server can send carries no cannabis
//! vocabulary.
//!
//! Owner, 2026-09-25, answer 12 of the numbered list put to him that day,
//! verbatim: «всё что касается канабиса нигде не должно быть» -- nothing
//! cannabis-related may appear anywhere. This file holds the server side of
//! that ruling: the Telegram bot's replies, buttons and command descriptions,
//! the bot's own locale file, the promoter's drafts and model prompt, every
//! string an HTTP handler can answer with (the served OpenAPI document
//! included, which is built from attribute strings), the admin notifications,
//! and the two front pages of the repository, `README.md` and
//! `docs/README.md`. The Mini App's own copy (`src/ui`, and the translations in
//! `src/trios/i18n.rs` it shares) is a separate sweep with its own guard,
//! `tests/legacy_vocabulary_wiring.rs`, and is left out here on purpose.
//!
//! What is read, and why only that.
//!
//! * In Rust, only STRING LITERALS count: plain, byte, C and raw strings,
//!   attribute strings included. Text is what a server can send; an
//!   identifier (`CartItemType::Strain`, `strain_id`) is not text, and a
//!   comment is never sent. Comments therefore do not count, which is also
//!   where the retirements are explained.
//! * Literals inside a `#[cfg(test)]` item do not count either. A fixture is
//!   compiled only into the test binary and is sent nowhere. The scanner skips
//!   the item the attribute is on -- a `mod`, a `fn`, a `use` -- by brace
//!   depth, with strings, chars and comments lexed so a brace inside them is
//!   not counted.
//! * In the two Markdown files every word counts: they are read as they are.
//! * Which Rust files: every `.rs` under `src/` except `src/ui/` (the Mini
//!   App, WebAssembly only) and `src/trios/` (shared with the Mini App), with
//!   one exception brought back in: `src/trios/promo.rs`, the promoter's copy
//!   and prompt, which only the server sends.
//!
//! The vocabulary, case-insensitive: the names of the plant and of the shop
//! that sold it as SUBSTRINGS (`cannabis` also hits `083_drop_cannabis_...`),
//! and the trade words as WHOLE WORDS, so `strain_of_day`, `p_strain`,
//! `telegram` and `сортировка` are not hits while `"strain"`, `THC: 20%` and
//! `сорта` are. The leaf the old shop marked its catalogue with, 🌿 and 🍃,
//! counts as a substring.
//!
//! What still holds a hit, and why, is [`SURVIVORS`]: one entry per file, the
//! exact number of hits and the reason. Checked from both ends, like
//! `tests/legacy_vocabulary_wiring.rs`: a hit the list does not admit fails,
//! and an entry whose count no longer matches the tree fails too, so the list
//! can only shrink on purpose.

// A panic is how a test reports failure. The restriction lints in Cargo.toml's
// [lints.clippy] exist for production code, as its own comment says.
#![allow(clippy::panic, clippy::expect_used)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Found anywhere inside a word.
const SUBSTRINGS: &[&str] = &[
    "cannabis",
    "каннабис",
    // The owner's own spelling, with one н.
    "канабис",
    "конопл",
    "marijuana",
    "марихуан",
    "dispensar",
    "weedpecker",
    "woody",
    "вуди",
    "ganja",
    "гандж",
    "hashish",
    "гашиш",
    "🌿",
    "🍃",
];

/// Found only as a whole word: the characters on either side are not
/// letters, digits or `_`.
const WHOLE_WORDS: &[&str] = &[
    "weed",
    "weeds",
    "strain",
    "strains",
    "thc",
    "тгк",
    "cbd",
    "gacp",
    "kush",
    "sativa",
    "indica",
    "сатива",
    "индика",
    "420",
    "bong",
    "bongs",
    "бонг",
    "бонги",
    "grinder",
    "grinders",
    "гриндер",
    "stoner",
    "сорт",
    "сорта",
    "сортов",
    "сортом",
    "сортах",
    "сорту",
    "сорты",
    "сортами",
];

/// A file allowed to hold hits in the text it can send, and why.
struct Survivor {
    path: &'static str,
    /// Exact: one more is a new occurrence, one fewer means the survivor
    /// shrank and the entry must follow it down.
    hits: usize,
    reason: &'static str,
}

const WIRE_STRAIN: &str =
    "The wire name of the retired catalogue kind, \"strain\". A cart line, a \
     share link or a stored broadcast row written before migration 083 still carries it, and \
     the code must still recognise it in order to refuse or re-route it rather than fail to \
     parse (tests/retired_table_wiring.rs and tests/legacy_vocabulary_wiring.rs keep the same \
     readers). It names a kind; it is never shown as a word to anybody.";

const SURVIVORS: &[Survivor] = &[
    Survivor {
        path: "src/api/cart.rs",
        hits: 4,
        reason: "Three are the wire name of the retired kind in `parse_kind` and \
                 `resolve_catalog_snapshot` (see WIRE_STRAIN); the fourth is the log line those \
                 lookups write on a database error, read by the operator and never sent.",
    },
    Survivor {
        path: "src/api/share.rs",
        hits: 1,
        reason: WIRE_STRAIN,
    },
    Survivor {
        path: "src/api/admin.rs",
        hits: 1,
        reason: WIRE_STRAIN,
    },
    Survivor {
        path: "src/api/cache.rs",
        hits: 1,
        reason: "The ETag cache key \"strains\", named for the retired /api/strains route; the \
                 admin marketing-display toggle still invalidates it. A key, never sent.",
    },
    Survivor {
        path: "src/db/entities/strain.rs",
        hits: 1,
        reason: "`table_name = \"strains\"`, the entity of the dropped table that the cart reader \
                 tests/retired_table_wiring.rs keeps still maps.",
    },
    Survivor {
        path: "src/db/mod.rs",
        hits: 4,
        reason: "The file names of migrations 083 and 085, each spelled twice in the migration \
                 list (its name and its include_str! path). Applied migrations are never \
                 renamed or edited (DECISIONS.md D2), and the name is what the database \
                 records as applied.",
    },
    Survivor {
        path: "src/config.rs",
        hits: 3,
        reason: "The two production URLs of the old shop's Mini App, spelled so that a \
                 WEB_APP_URL still pointing at either is treated as unset (DECISIONS.md D19). \
                 A guard has to name what it refuses.",
    },
    Survivor {
        path: "src/api/loyalty.rs",
        hits: 1,
        reason: "`retired_tier`, the filter that keeps the tier named after the old shop off \
                 GET /api/loyalty/tiers since 2026-09-25. It has to name what it refuses; the \
                 name is compared, never sent.",
    },
    Survivor {
        path: "src/api/auth.rs",
        hits: 1,
        reason: "The HMAC key admin tokens are derived with (`generate_admin_token`). Never \
                 shown to anybody; changing it would sign every admin out and is not a copy \
                 change.",
    },
];

// ──────────────────────────────────────────────────────────────────
// The matcher
// ──────────────────────────────────────────────────────────────────

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Every hit in one piece of text, as the matched words, in order.
fn hits_in(text: &str) -> Vec<String> {
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    let mut out = Vec::new();
    for word in SUBSTRINGS {
        let w: Vec<char> = word.chars().collect();
        let mut i = 0;
        while i + w.len() <= lower.len() {
            if lower[i..i + w.len()] == w[..] {
                out.push((i, word.to_string()));
                i += w.len();
            } else {
                i += 1;
            }
        }
    }
    for word in WHOLE_WORDS {
        let w: Vec<char> = word.chars().collect();
        let mut i = 0;
        while i + w.len() <= lower.len() {
            let before_ok = i == 0 || !is_word_char(lower[i - 1]);
            let after_ok = lower.get(i + w.len()).is_none_or(|&c| !is_word_char(c));
            if before_ok && after_ok && lower[i..i + w.len()] == w[..] {
                out.push((i, word.to_string()));
                i += w.len();
            } else {
                i += 1;
            }
        }
    }
    out.sort();
    out.into_iter().map(|(_, w)| w).collect()
}

// ──────────────────────────────────────────────────────────────────
// The Rust lexer: string literals outside #[cfg(test)] items
// ──────────────────────────────────────────────────────────────────

/// A string literal the program can send: its first line and its content.
#[derive(Debug, PartialEq, Eq)]
struct Literal {
    line: usize,
    text: String,
}

fn starts_with_at(c: &[char], at: usize, s: &str) -> bool {
    s.chars()
        .enumerate()
        .all(|(k, ch)| c.get(at + k) == Some(&ch))
}

/// Whether position `at` begins a token (no identifier character before it).
fn begins_token(c: &[char], at: usize) -> bool {
    at == 0 || !is_word_char(c[at - 1])
}

/// The string literals of a Rust file that are not inside a `#[cfg(test)]`
/// item.
///
/// Guarantees, for source the Rust 2021 lexer accepts: `//` and nested
/// `/* */` comments are skipped; plain, byte and C strings (escapes honoured)
/// and raw strings with any number of `#` are read whole across lines; char
/// and byte-char literals are told apart from lifetimes, so a quote or a brace
/// inside one moves nothing. A `#[cfg(test)]` attribute marks the next item:
/// if a `;` comes before any `{`, that was the item (a `use`); otherwise the
/// braces from the first `{` to its match are skipped. Anything the lexer
/// misreads can only make it count MORE (fail closed), except a brace it
/// misses, which the self-tests below pin for the shapes this tree uses.
fn shipped_literals(src: &str) -> Vec<Literal> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1;
    let mut depth = 0usize;
    let mut test_attr_pending = false;
    let mut skipping_from: Option<usize> = None;
    while i < c.len() {
        let ch = c[i];
        let next = c.get(i + 1).copied();
        // Comments.
        if ch == '/' && next == Some('/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if ch == '/' && next == Some('*') {
            let mut nest = 1;
            i += 2;
            while i < c.len() && nest > 0 {
                if c[i] == '\n' {
                    line += 1;
                }
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    nest += 1;
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    nest -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // Raw strings: r"..", r#".."#, br".., cr"..
        if ch == 'r' {
            let prefixed = i > 0 && matches!(c[i - 1], 'b' | 'c') && begins_token(&c, i - 1);
            if begins_token(&c, i) || prefixed {
                let hashes = c[i + 1..].iter().take_while(|&&h| h == '#').count();
                if c.get(i + 1 + hashes) == Some(&'"') {
                    let start_line = line;
                    let mut j = i + 2 + hashes;
                    let mut text = String::new();
                    while j < c.len() {
                        if c[j] == '"' && (1..=hashes).all(|k| c.get(j + k) == Some(&'#')) {
                            break;
                        }
                        if c[j] == '\n' {
                            line += 1;
                        }
                        text.push(c[j]);
                        j += 1;
                    }
                    if skipping_from.is_none() {
                        out.push(Literal {
                            line: start_line,
                            text,
                        });
                    }
                    i = j + 1 + hashes;
                    continue;
                }
            }
        }
        // Plain, byte and C strings.
        if ch == '"' {
            let start_line = line;
            let mut j = i + 1;
            let mut text = String::new();
            while j < c.len() && c[j] != '"' {
                if c[j] == '\\' {
                    if c.get(j + 1) == Some(&'\n') {
                        line += 1;
                    }
                    text.push(c[j]);
                    if let Some(&e) = c.get(j + 1) {
                        text.push(e);
                    }
                    j += 2;
                    continue;
                }
                if c[j] == '\n' {
                    line += 1;
                }
                text.push(c[j]);
                j += 1;
            }
            if skipping_from.is_none() {
                out.push(Literal {
                    line: start_line,
                    text,
                });
            }
            i = j + 1;
            continue;
        }
        // Char and byte-char literals, and lifetimes.
        if ch == '\'' {
            if next == Some('\\') {
                let close = c
                    .get(i + 3..)
                    .and_then(|rest| rest.iter().position(|&q| q == '\''));
                i += close.map_or(1, |p| p + 4);
                continue;
            }
            if c.get(i + 2) == Some(&'\'') {
                i += 3;
                continue;
            }
            i += 1;
            continue;
        }
        // The test attribute, and the item it is on.
        if ch == '#' && starts_with_at(&c, i, "#[cfg(test)]") {
            test_attr_pending = skipping_from.is_none();
            i += "#[cfg(test)]".chars().count();
            continue;
        }
        match ch {
            '\n' => line += 1,
            '{' => {
                depth += 1;
                if test_attr_pending {
                    skipping_from = Some(depth);
                    test_attr_pending = false;
                }
            }
            '}' => {
                if skipping_from == Some(depth) {
                    skipping_from = None;
                }
                depth = depth.saturating_sub(1);
            }
            ';' if test_attr_pending => test_attr_pending = false,
            _ => {}
        }
        i += 1;
    }
    out
}

// ──────────────────────────────────────────────────────────────────
// The census
// ──────────────────────────────────────────────────────────────────

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn relative(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .expect("scanned path under the repository")
        .to_string_lossy()
        .replace('\\', "/")
}

/// The server's Rust files (see the header for why `src/ui` and `src/trios`
/// are left out, and why `src/trios/promo.rs` is not).
fn is_server_rust(rel: &str) -> bool {
    rel.ends_with(".rs")
        && !rel.starts_with("src/ui/")
        && (!rel.starts_with("src/trios/") || rel == "src/trios/promo.rs")
}

/// Read as they are: every word counts.
const MARKDOWN_PAGES: &[&str] = &["README.md", "docs/README.md"];

struct Census {
    rust_files: usize,
    literals: usize,
    /// Hits per file, with the line each sits on.
    hits: BTreeMap<String, Vec<(usize, String)>>,
}

fn census() -> Census {
    let root = repo_root();
    let mut files = Vec::new();
    walk(&root.join("src"), &mut files);
    let mut census = Census {
        rust_files: 0,
        literals: 0,
        hits: BTreeMap::new(),
    };
    for path in files {
        let rel = relative(&path);
        if !is_server_rust(&rel) {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        census.rust_files += 1;
        for lit in shipped_literals(&text) {
            census.literals += 1;
            for word in hits_in(&lit.text) {
                census
                    .hits
                    .entry(rel.clone())
                    .or_default()
                    .push((lit.line, word));
            }
        }
    }
    for page in MARKDOWN_PAGES {
        let text =
            fs::read_to_string(root.join(page)).unwrap_or_else(|e| panic!("read {page}: {e}"));
        for (index, line) in text.lines().enumerate() {
            for word in hits_in(line) {
                census
                    .hits
                    .entry(page.to_string())
                    .or_default()
                    .push((index + 1, word));
            }
        }
    }
    census
}

// ──────────────────────────────────────────────────────────────────
// The checks
// ──────────────────────────────────────────────────────────────────

#[test]
fn no_server_text_carries_the_vocabulary_beyond_the_allowlist() {
    let census = census();
    let mut problems = Vec::new();
    for (path, hits) in &census.hits {
        let lines = hits
            .iter()
            .map(|(line, word)| format!("{path}:{line} `{word}`"))
            .collect::<Vec<_>>()
            .join("\n      ");
        match SURVIVORS.iter().find(|s| s.path == path) {
            None => problems.push(format!(
                "{path}: {} hit(s) in text the server can send, and the file is not on the \
                 allowlist:\n      {lines}",
                hits.len()
            )),
            Some(s) if hits.len() > s.hits => problems.push(format!(
                "{path}: {} hit(s), the allowlist admits {} -- a NEW occurrence:\n      {lines}",
                hits.len(),
                s.hits
            )),
            Some(_) => {}
        }
    }
    assert!(
        problems.is_empty(),
        "cannabis vocabulary in server text (owner, 2026-09-25: nothing cannabis-related \
         anywhere; see the header of tests/server_text_vocabulary_wiring.rs):\n  {}",
        problems.join("\n  ")
    );
}

#[test]
fn no_allowlist_entry_is_stale() {
    let census = census();
    let mut problems = Vec::new();
    for s in SURVIVORS {
        assert!(
            s.reason.trim().len() > 40,
            "{}: an entry needs a reason",
            s.path
        );
        let found = census.hits.get(s.path).map_or(0, Vec::len);
        if found != s.hits {
            problems.push(format!(
                "{}: the allowlist admits {} hit(s) and the file holds {found}. If a survivor \
                 went, lower the count (or drop the entry) in the same change.",
                s.path, s.hits
            ));
        }
    }
    let mut paths: Vec<_> = SURVIVORS.iter().map(|s| s.path).collect();
    paths.sort_unstable();
    paths.dedup();
    assert_eq!(paths.len(), SURVIVORS.len(), "a file is listed twice");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// DECISIONS.md D16: a scan that reads nothing reports nothing wrong. Pin that
/// it read the server's files, found literals in them, and sees the
/// vocabulary where it is known to be.
#[test]
fn the_scan_reads_the_server_and_sees_the_vocabulary() {
    let census = census();
    assert!(
        census.rust_files >= 60,
        "only {} server Rust files read",
        census.rust_files
    );
    assert!(
        census.literals >= 3000,
        "only {} string literals read: the lexer has gone blind",
        census.literals
    );
    for known in ["src/config.rs", "src/db/mod.rs"] {
        assert!(
            census.hits.contains_key(known),
            "{known} holds a known survivor and the scan does not see it"
        );
    }
    for page in MARKDOWN_PAGES {
        assert!(repo_root().join(page).is_file(), "{page} is missing");
    }
}

/// The bot's own locale file is the one server file allowed NO survivor at
/// all: every reply and button label in both published locales is a literal
/// there. Pinned on its own so that an allowlist entry for it can never be
/// added quietly. (The served OpenAPI document, which is built from attribute
/// strings and doc comments, is checked as built by the unit test in
/// `src/api/openapi.rs`.)
#[test]
fn the_bot_locale_file_holds_no_hit_in_any_value() {
    assert!(
        SURVIVORS.iter().all(|s| s.path != "src/locales.rs"),
        "src/locales.rs may not be on the allowlist"
    );
    let src = fs::read_to_string(repo_root().join("src/locales.rs")).expect("read src/locales.rs");
    let hits: Vec<_> = shipped_literals(&src)
        .into_iter()
        .flat_map(|l| hits_in(&l.text).into_iter().map(move |w| (l.line, w)))
        .collect();
    assert!(hits.is_empty(), "src/locales.rs: {hits:?}");
}

#[test]
fn the_matcher_reads_substrings_and_whole_words() {
    assert_eq!(hits_in("083_drop_cannabis_catalog.sql"), ["cannabis"]);
    assert_eq!(hits_in("Woody Elite"), ["woody"]);
    assert_eq!(hits_in("Личные встречи с Вуди"), ["вуди"]);
    assert_eq!(hits_in("⚡ THC: 20%"), ["thc"]);
    assert_eq!(hits_in("\"strain\" => Ok"), ["strain"]);
    assert_eq!(hits_in("Эксклюзивные сорта"), ["сорта"]);
    assert_eq!(hits_in("🌿 Posting fact"), ["🌿"]);
    assert_eq!(hits_in("Asia 420"), ["420"]);
    assert_eq!(hits_in("woody-weed-bot-production"), ["woody", "weed"]);
    // Not hits: inside a longer word or an identifier.
    assert!(hits_in("p_strain strain_of_day sotd_next_ restrained").is_empty());
    assert!(hits_in("telegram program сортировка по цене").is_empty());
    assert!(hits_in("8420420131 indicates weedy").is_empty());
}

#[test]
fn only_shipped_string_literals_are_read() {
    let texts =
        |src: &str| -> Vec<String> { shipped_literals(src).into_iter().map(|l| l.text).collect() };
    // Comments are not read; strings are, across lines.
    assert_eq!(
        texts("let a = \"one\"; // \"two\"\n/* \"three\" */ let b = \"fo\nur\";"),
        ["one", "fo\nur"]
    );
    // Raw, byte and attribute strings are read; a quote inside a raw string
    // does not end it.
    assert_eq!(
        texts("#[command(description = \"Menu\")]\nlet r = r#\"say \"hi\"\"#; let k = b\"key\";"),
        ["Menu", "say \"hi\"", "key"]
    );
    // Char literals and lifetimes move nothing.
    assert_eq!(
        texts("fn f<'a>(x: &'a str) { let q = ['\"', '{', '\\'']; let s = \"kept\"; }"),
        ["kept"]
    );
    // A #[cfg(test)] module is skipped whole, braces in its strings included,
    // and the code after it is read again.
    assert_eq!(
        texts(
            "let a = \"before\";\n#[cfg(test)]\nmod tests {\n    fn t() { let x = \"{ fixture\"; }\n}\nlet b = \"after\";"
        ),
        ["before", "after"]
    );
    // A #[cfg(test)] use ends at its semicolon and skips nothing after it.
    assert_eq!(
        texts("#[cfg(test)]\nuse std::fs;\nfn g() { let s = \"sent\"; }"),
        ["sent"]
    );
    // A #[cfg(test)] fn is skipped too, with the attributes after it.
    assert_eq!(
        texts(
            "#[cfg(test)]\n#[allow(dead_code)]\nfn h() { \"fixture\"; }\nconst C: &str = \"sent\";"
        ),
        ["sent"]
    );
}

/// A planted reply is caught with its line, and a planted fixture is not.
#[test]
fn a_planted_reply_is_caught_and_a_planted_fixture_is_not() {
    let src = "fn reply() -> &'static str {\n    \"🔥 Сорт дня\"\n}\n#[cfg(test)]\nmod tests {\n    const X: &str = \"OG Kush, THC 22%\";\n}\n";
    let hits: Vec<(usize, String)> = shipped_literals(src)
        .into_iter()
        .flat_map(|l| hits_in(&l.text).into_iter().map(move |w| (l.line, w)))
        .collect();
    assert_eq!(hits, [(2, "сорт".to_string())]);
}
