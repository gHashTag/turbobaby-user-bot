//! Source-level guard: the cannabis-era vocabulary may not come back.
//!
//! This tree is a standalone derivative of a cannabis shop bot. The rebrand
//! (DECISIONS.md D2-D5, D19) retired the catalogue, hid what it could not
//! delete, and left a residue that is there on purpose: enums that still read
//! cart lines and share links minted before migration 083, a strain-of-day
//! carousel whose retirement is the owner's call, and a migration history that
//! is never edited (D2). (The i18n keys of the retired strain menu were part of
//! that residue until the owner's ruling of 2026-09-25 deleted them.) What nothing guarded was the other direction -- a NEW
//! occurrence. A pasted fixture, a copied match arm or a fresh label saying
//! "strain" or "THC" would have shipped with every test green.
//!
//! So this walks `src/`, `migrations/`, `styles/`, `assets/` and the served
//! `dist/index.html` as text (what is left out, and why, is at the end of this
//! header) and finds the vocabulary as whole words, case-insensitively:
//!
//! * `strain`, `weed`, `thc`, `cbd`, `gram`, each with an optional plural `s`
//!   (the plural is how `T_MENU_NO_RESULTS` spelt it: "No {0} strains found",
//!   until the owner's ruling of 2026-09-25 deleted the key);
//! * the Russian word for strain in its case forms (`сорт`, `сорта`, `сортов`,
//!   ...), because the Russian twin of every dead key is where the word
//!   actually lived (`T_MENU_DESC` read "Наши премиальные сорта" until the
//!   same ruling deleted it). As whole words only, so `сортировка` (sorting)
//!   is not a hit.
//!
//! "Whole word" means the characters on either side are not letters, digits
//! or `_`. So `telegram`, `program` and `strain_of_day` are not hits, and
//! `CartItemType::Strain`, `"strain"` and `woody-weed` are. A compound
//! identifier (`StrainOfDay`, `ApiStrain`, `strain_ids`) is NOT seen: this is a
//! vocabulary guard, not an identifier census, and it says so here rather than
//! implying more.
//!
//! What is allowed, and why, is written down below, and the list is checked
//! from both ends like `tests/retired_table_wiring.rs`: a hit the list does not
//! cover fails, and an entry whose count no longer matches the tree fails too,
//! so the list can only shrink deliberately.
//!
//! * Words inside COMMENTS are allowed wholesale in the two SOURCE roots,
//!   `src/` and `migrations/`. They are compiled or run and never sent as text,
//!   and the retirement is documented in comments that have to name what they
//!   retired. Comment grammar: `//` and `/* */` in Rust, `--` and `/* */` in
//!   SQL, `/* */` in CSS, none in any other file (fail closed); what the lexer
//!   guarantees, grammar by grammar, is written on `comment_mask`.
//! * In the SERVED roots nothing is a comment. `styles/`, `assets/` and
//!   `dist/index.html` reach the browser byte for byte, so a comment in a
//!   stylesheet, a script, an SVG or a page ships to the customer like any other
//!   text, and every hit there counts. (Until 2026-09-24 `styles/` was read with
//!   the CSS comment grammar; no comment in it held a hit, so no count moved.)
//! * Migrations `001`-`076` are allowed wholesale: D2 forbids editing them, so
//!   a word in them is history, not a choice anybody can still make.
//! * Everything else is an entry in [`SURVIVORS`], one per file, with the
//!   exact number of code hits it holds and the reason it may hold them.
//!
//! What is scanned beyond source, and what is left out. Measured 2026-09-24;
//! the routing is read in `src/main.rs`, named by binding and route, not line.
//!
//! * `assets/` is served at /assets and again at /images (two `ServeDir`s in
//!   `static_assets`). Its text is 13 files -- a brand preview page and its
//!   README, seven SVGs, the three game scripts under `assets/game/` and the
//!   vendored three.js module -- and they hold 0 hits, comments included. Its
//!   media (webp, png, jpeg, mp4) does not read as UTF-8 and is skipped; a file
//!   with a text extension (`SERVED_TEXT_EXTENSIONS`) that does not read fails
//!   the scan instead.
//! * Of `dist/`, only `index.html` is read: it is the page every SPA route in
//!   `spa_routes` answers with (`spa_handler`) and the one `serve_dist`, the
//!   router's fallback, answers a client route with. Trunk renders it from the
//!   repository-root `index.html`, which reaches a customer only through it.
//!   The rest of `dist/` is left out because no line of it is written here:
//!   the top-level `*.js` and everything under `snippets/` is wasm-bindgen and
//!   trunk glue and the dioxus crates' own snippets; the `*_bg.wasm` is binary,
//!   and its strings are compiled from `src/`, which is scanned with every
//!   string counted; the three hashed `*.css` are byte-identical to
//!   `styles/variables.css`, `main.css` and `admin.css` (compared 2026-09-24),
//!   which are scanned; `*.br` and `*.gz` are compressed twins; `version.txt`
//!   is the bundle's build id.
//! * The repository root is neither scanned nor served. `storybook.html` and
//!   `typography.html` there still carry the old shop's name ("Woody Weed Bot";
//!   storybook's body text calls it a cannabis delivery service), six
//!   whole-word hits in each. Nothing routes a path to the root:
//!   `static_assets` nests /styles, /assets, /images and /uploads (a runtime
//!   volume, not the repository), and `serve_dist` and `spa_handler` answer
//!   from the in-memory copy of `dist/` that `walk_dir` loads. The Dockerfile's
//!   runtime stage does not ship them either: it copies the server binary,
//!   `dist`, `styles`, `assets` and `migrations`. The day one of them is
//!   routed, it belongs in `SCANNED_ROOTS`.

// A panic is how a test reports failure. The restriction lints in Cargo.toml's
// [lints.clippy] exist for production code, as its own comment says.
#![allow(clippy::panic, clippy::expect_used)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// English stems; each also matches with one trailing `s`.
const ENGLISH_STEMS: &[&str] = &["strain", "weed", "thc", "cbd", "gram"];

/// The Russian word for "strain" in the case forms this tree could use,
/// longest first so `сортами` is not read as `сорта` + `ми`.
const RUSSIAN_FORMS: &[&str] = &[
    "сортами",
    "сортов",
    "сортом",
    "сортах",
    "сорта",
    "сорту",
    "сорты",
    "сорт",
];

/// How the text under a scanned root reaches anybody, which decides whether its
/// comments may hold the vocabulary.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Reach {
    /// Compiled or run, never sent as text: comments are exempt, read with the
    /// file's own comment grammar.
    Source,
    /// Sent to the browser byte for byte: a comment ships like any other text,
    /// so nothing is a comment and every hit counts (fail closed).
    Served,
}

/// A directory walked recursively, or one file.
struct Root {
    path: &'static str,
    reach: Reach,
}

/// Everything the scan reads. Each root is required to exist and to yield at
/// least one readable file; what is left out, and why, is in the header.
const SCANNED_ROOTS: &[Root] = &[
    Root {
        path: "src",
        reach: Reach::Source,
    },
    Root {
        path: "migrations",
        reach: Reach::Source,
    },
    Root {
        path: "styles",
        reach: Reach::Served,
    },
    Root {
        path: "assets",
        reach: Reach::Served,
    },
    Root {
        path: "dist/index.html",
        reach: Reach::Served,
    },
];

/// The extensions of the text a served root sends. A file with one of them that
/// does not read as UTF-8 fails the scan instead of being skipped as media: a
/// served text file the scan cannot read is a hole, not an image.
const SERVED_TEXT_EXTENSIONS: &[&str] = &["html", "js", "css", "json", "svg", "md"];

/// Migrations up to and including this number are immutable history (D2).
const LAST_HISTORICAL_MIGRATION: u32 = 76;

/// A file allowed to hold the vocabulary outside its comments, and why.
///
/// `hits` is the exact number of whole-word hits in code (strings included,
/// comments excluded). It is exact on purpose: one more is a new occurrence and
/// fails; one fewer means the survivor shrank and the entry must follow it down.
struct Survivor {
    path: &'static str,
    hits: usize,
    reason: &'static str,
}

const WIRE_CART: &str = "Wire-compat enum `CartItemType::Strain`. A cart or an order saved before \
     migration 083 still carries a line of kind \"strain\"; every exhaustive match over the \
     enum must name the variant, and what the shop owes that customer is the open D9 question \
     `tests/retired_table_wiring.rs` records for the owner.";

const SURVIVORS: &[Survivor] = &[
    // --- forward migrations that must still name the old catalogue -------------------
    Survivor {
        path: "migrations/081_cart_rental_lines.sql",
        hits: 1,
        reason: "The widened cart `kind` CHECK keeps 'strain' in its list: `ADD CONSTRAINT` \
                 validates existing rows, and a live cart may still hold one (DECISIONS.md \
                 D8 amendment, point 1).",
    },
    Survivor {
        path: "migrations/083_drop_cannabis_catalog.sql",
        hits: 4,
        reason: "The drop itself: it probes `public.strains`, counts it, refuses on a live \
                 shop, and drops the table. It cannot do that without naming it.",
    },
    // --- wire-compat enums: CartItemType::Strain -------------------------------------
    Survivor {
        path: "src/ui/state.rs",
        hits: 3,
        reason: "The `CartItemType::Strain` variant and the `\"strain\"` arm that reads it \
                 off a saved cart line. The enum's home; see WIRE_CART.",
    },
    Survivor {
        path: "src/ui/api/http.rs",
        hits: 2,
        reason: "`cart_item_type_to_kind` is an exhaustive match, so it must map \
                 `CartItemType::Strain` to the server's kind string \"strain\" like every \
                 other variant. See WIRE_CART.",
    },
    Survivor {
        path: "src/ui/screens/checkout_screen.rs",
        hits: 2,
        reason: WIRE_CART,
    },
    Survivor {
        path: "src/ui/screens/home_screen.rs",
        hits: 1,
        reason: WIRE_CART,
    },
    Survivor {
        path: "src/ui/screens/orders_screen.rs",
        hits: 1,
        reason: WIRE_CART,
    },
    Survivor {
        path: "src/ui/screens/profile_screen.rs",
        hits: 1,
        reason: WIRE_CART,
    },
    Survivor {
        path: "src/ui/screens/order_detail_screen.rs",
        hits: 1,
        reason: WIRE_CART,
    },
    Survivor {
        path: "src/ui/screens/success_screen.rs",
        hits: 1,
        reason: WIRE_CART,
    },
    Survivor {
        path: "src/api/cart.rs",
        hits: 16,
        reason: "`parse_kind` still accepts \"strain\" and `resolve_catalog_snapshot` prices \
                 such a line through the `strain` entity -- the reader \
                 `tests/retired_table_wiring.rs` keeps as a SURVIVOR pending the owner's D9 \
                 ruling -- plus the unit tests that pin both.",
    },
    // --- wire-compat enums: share links and deep links minted before 083 ------------
    Survivor {
        path: "src/ui/share.rs",
        hits: 12,
        reason: "Wire-compat enum `ProductKind::Strain`. Share links carrying `p_strain` are \
                 already in chats; the variant maps them to the menu instead of failing to \
                 parse them, and its tests pin that mapping.",
    },
    Survivor {
        path: "src/trios/deeplink.rs",
        hits: 6,
        reason: "`Kind::Strain`, the shared deep-link kind the client's `ProductKind` converts \
                 through, with its `p_strain` payload prefix, its `strain` wire name and its \
                 `/menu` target, so a link minted before 083 still parses.",
    },
    Survivor {
        path: "src/api/share.rs",
        hits: 6,
        reason: "`ShareKind::Strain`, the server side of the same deep-link wire. It answers \
                 no preview since 083 dropped the table, and a test pins the prefix pair.",
    },
    Survivor {
        path: "src/api/admin.rs",
        hits: 1,
        reason: "The broadcast deep-link prefix map (`\"strain\" => \"p_strain\"`), the same \
                 wire as `src/trios/deeplink.rs` for a product kind a stored broadcast row can \
                 still carry.",
    },
    Survivor {
        path: "src/trios/promo.rs",
        hits: 5,
        reason: "The digest icon for a legacy \"strain\" row and the ranking tests that use \
                 legacy rows as fixtures (one of them a Russian sentence about a set of \
                 strains). The producer stopped reading `strains` with 083; the kind stays \
                 readable for rows already stored.",
    },
    // --- the strains table's readers, kept by tests/retired_table_wiring.rs ----------
    Survivor {
        path: "src/db/strains.rs",
        hits: 9,
        reason: "The wire struct `Strain` and the `From<strain::Model>` conversions the two \
                 surviving readers need (`tests/retired_table_wiring.rs` SURVIVORS).",
    },
    Survivor {
        path: "src/db/entities/strain.rs",
        hits: 1,
        reason: "The SeaORM entity's `table_name = \"strains\"`; it goes when its readers go.",
    },
    Survivor {
        path: "src/db/entities/mod.rs",
        hits: 1,
        reason: "`pub mod strain;`, the declaration of the entity above.",
    },
    Survivor {
        path: "src/db/mod.rs",
        hits: 6,
        reason: "`mod strains` and its re-export, and `get_strains_of_day`, the query behind \
                 the strain-of-day carousel (see `src/bot/callbacks.rs`).",
    },
    Survivor {
        path: "src/ui/api/types.rs",
        hits: 7,
        reason: "The client twin of `db::strains::Strain`, with its `thc`/`cbd` fields. Its own \
                 comment records why it stays: share links and cart lines saved before 083 \
                 still round-trip `ProductKind::Strain`.",
    },
    Survivor {
        path: "src/api/cache.rs",
        hits: 1,
        reason: "`invalidate_strains` clears the `strains` ETag key, named for the retired \
                 `/api/strains` route; the admin marketing-display toggle still calls it.",
    },
    // --- the strain-of-day callback survivor, pending the owner's retirement ---------
    Survivor {
        path: "src/bot/callbacks.rs",
        hits: 5,
        reason: "The strain-of-day carousel's pagination callback, which still prints a THC \
                 line. Retiring the carousel is a product decision recorded for the owner \
                 (`tests/retired_table_wiring.rs`, the `src/db/mod.rs` survivor), not a tidy-up.",
    },
    Survivor {
        path: "src/locales.rs",
        hits: 2,
        reason: "The carousel's heading, `strain_of_day`, in ru and en. It goes with the \
                 callback above.",
    },
    // `src/trios/i18n.rs` stood here with 3 hits: `T_MENU_DESC` and
    // `T_MENU_NO_RESULTS`, keys of the retired strain menu, kept until the owner
    // decided their copy. The owner's ruling of 2026-09-25 (#12, nothing
    // cannabis-related anywhere) decided it: both keys were deleted, not
    // reworded, so the file holds no hit and has no entry.
    // --- guards that must spell the vocabulary to refuse it --------------------------
    Survivor {
        path: "src/config.rs",
        hits: 1,
        reason: "Treats the old shop's production URL (`woody-weed-bot-production`) in \
                 WEB_APP_URL as unset and falls back to the TurboBaby URL, so the menu button \
                 cannot point at the other bot again (D19).",
    },
    Survivor {
        path: "src/api/openapi.rs",
        hits: 3,
        reason: "The RETIRED word list a test holds the published API description against.",
    },
    // --- the legacy catalogue D19 hides rather than deletes --------------------------
    Survivor {
        path: "src/trios/drink_categories.rs",
        hits: 1,
        reason: "\"cbd tea\", one of the historical tea subcategory labels the drinks model \
                 folds into the single `tea` category. The tea catalogue is legacy stock \
                 migration 085 hides and D19 keeps readable for old orders.",
    },
    Survivor {
        path: "src/trios/packs.rs",
        hits: 8,
        reason: "Unit tests of the legacy pack weight line (\"N strains\", ru \"N сортов\"). \
                 The `sets` screens stay per D19; what they become is issue #2's decision.",
    },
    Survivor {
        path: "src/api/catalog.rs",
        hits: 1,
        reason: "A test fixture name (\"Strain Set\") for the legacy set validator D19 keeps.",
    },
];

// ──────────────────────────────────────────────────────────────────
// The scanner
// ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Grammar {
    Rust,
    Sql,
    Css,
    /// No comment syntax is known: every hit counts as code (fail closed).
    Plain,
}

fn grammar_of(path: &Path) -> Grammar {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => Grammar::Rust,
        Some("sql") => Grammar::Sql,
        Some("css") => Grammar::Css,
        _ => Grammar::Plain,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Hit {
    line: usize,
    word: String,
    in_comment: bool,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn same_letter(c: char, lower: char) -> bool {
    c == lower || c.to_lowercase().eq(std::iter::once(lower))
}

fn matches_at(chars: &[char], at: usize, word: &str) -> Option<usize> {
    let mut len = 0;
    for w in word.chars() {
        match chars.get(at + len) {
            Some(&c) if same_letter(c, w) => len += 1,
            _ => return None,
        }
    }
    Some(len)
}

fn bounded_after(chars: &[char], end: usize) -> bool {
    chars.get(end).is_none_or(|&c| !is_word_char(c))
}

/// Every whole-word occurrence of the vocabulary in one line, as (start, len).
fn words_in(chars: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if i > 0 && is_word_char(chars[i - 1]) {
            i += 1;
            continue;
        }
        let mut found = None;
        for stem in ENGLISH_STEMS {
            if let Some(len) = matches_at(chars, i, stem) {
                if chars.get(i + len).is_some_and(|&c| same_letter(c, 's'))
                    && bounded_after(chars, i + len + 1)
                {
                    found = Some(len + 1);
                } else if bounded_after(chars, i + len) {
                    found = Some(len);
                }
            }
            if found.is_some() {
                break;
            }
        }
        if found.is_none() {
            for form in RUSSIAN_FORMS {
                if let Some(len) = matches_at(chars, i, form) {
                    if bounded_after(chars, i + len) {
                        found = Some(len);
                        break;
                    }
                }
            }
        }
        match found {
            Some(len) => {
                out.push((i, len));
                i += len;
            }
            None => i += 1,
        }
    }
    out
}

/// Where the scanner stands; carried from the end of one line into the next.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lexer {
    Code,
    /// Inside a block comment, nested this deep (Rust and SQL nest them).
    Block(usize),
    /// Inside a quoted literal that this character closes.
    Quoted(char),
    /// Inside a Rust raw string that `"` plus this many `#` closes.
    Raw(usize),
}

/// The number of `#` when a Rust raw string (`r"`, `r#"`, `br##"`, `cr#"`, ...)
/// opens at the `r` at `at`. The `r` must begin a token, alone or after a `b` or
/// `c` that does: Rust 2021 reserves every other identifier prefix before a
/// quote, so `bar"` never lexes, and a raw identifier (`r#type`) has no quote.
fn raw_string_hashes(chars: &[char], at: usize) -> Option<usize> {
    let begins_token = |k: usize| k == 0 || !is_word_char(chars[k - 1]);
    let prefixed = at > 0 && matches!(chars[at - 1], 'b' | 'c') && begins_token(at - 1);
    if !begins_token(at) && !prefixed {
        return None;
    }
    let hashes = chars[at + 1..].iter().take_while(|&&c| c == '#').count();
    (chars.get(at + 1 + hashes) == Some(&'"')).then_some(hashes)
}

/// Which characters of each line sit inside a comment.
///
/// Block comments, quoted literals and Rust raw strings are carried across
/// lines, so a `//` or `/*` on the continuation line of a string that spans
/// lines is text. (Until 2026-09-24 strings were reset at every line end, and
/// such a line was read as a comment from the `//`: the CSP header string in
/// `src/main.rs` had its `https://telegram.org` line hidden that way.) What
/// is guaranteed, per grammar -- a comment read as code only fails closed, so
/// the claims that matter are the ones about text read as a comment:
///
/// * Rust: exact for source the Rust 2021 lexer accepts. `//` and nested
///   `/* */` are comments; strings (plain, byte and C, escapes honoured) and raw
///   strings with any number of `#` are text; char and byte literals are told
///   apart from lifetimes. Source the lexer rejects gets no promise.
/// * SQL: `--` and nested `/* */` (PostgreSQL nests them) are comments; `'...'`
///   and `"..."` are text across lines, a doubled quote staying inside. Two
///   shapes are NOT lexed: a dollar-quoted body is read as SQL rather than as
///   one string, and in an `E'...'` string `\'` is read as the end. Either can
///   misplace a string boundary, and a `--` inside a string then hides the
///   words after it: that edge is fail-open. Neither misleads on the tree as
///   it stands (measured 2026-09-24): every `$$`/`$tag$` body under
///   migrations/ is a DO block or a PL/pgSQL function, whose comments and
///   strings are SQL's, so reading it as SQL is right; and `E'` occurs nowhere.
/// * CSS: `/* */`, not nested; strings honour `\` escapes and end at the end of
///   their line unless the newline is escaped, as CSS's tokenizer ends them.
/// * Plain: nothing is a comment (fail closed).
fn comment_mask(text: &str, grammar: Grammar) -> Vec<Vec<bool>> {
    let nests = matches!(grammar, Grammar::Rust | Grammar::Sql);
    let escapes = matches!(grammar, Grammar::Rust | Grammar::Css);
    let mut masks = Vec::new();
    let mut state = Lexer::Code;
    for line in text.lines() {
        let chars: Vec<char> = line.chars().collect();
        let mut mask = vec![false; chars.len()];
        if grammar == Grammar::Plain {
            masks.push(mask);
            continue;
        }
        let mut i = 0;
        let mut newline_escaped = false;
        while i < chars.len() {
            let c = chars[i];
            let next = chars.get(i + 1).copied();
            match state {
                Lexer::Block(depth) => {
                    mask[i] = true;
                    if c == '*' && next == Some('/') {
                        mask[i + 1] = true;
                        state = if depth > 1 {
                            Lexer::Block(depth - 1)
                        } else {
                            Lexer::Code
                        };
                        i += 2;
                    } else if nests && c == '/' && next == Some('*') {
                        mask[i + 1] = true;
                        state = Lexer::Block(depth + 1);
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                Lexer::Quoted(q) => {
                    if escapes && c == '\\' {
                        newline_escaped = next.is_none();
                        i += 2;
                    } else {
                        if c == q {
                            state = Lexer::Code;
                        }
                        i += 1;
                    }
                }
                Lexer::Raw(hashes) => {
                    if c == '"' && (1..=hashes).all(|k| chars.get(i + k) == Some(&'#')) {
                        state = Lexer::Code;
                        i += 1 + hashes;
                    } else {
                        i += 1;
                    }
                }
                Lexer::Code => {
                    let line_comment = match grammar {
                        Grammar::Rust => c == '/' && next == Some('/'),
                        Grammar::Sql => c == '-' && next == Some('-'),
                        _ => false,
                    };
                    if line_comment {
                        mask[i..].iter_mut().for_each(|m| *m = true);
                        break;
                    }
                    if c == '/' && next == Some('*') {
                        state = Lexer::Block(1);
                        mask[i] = true;
                        mask[i + 1] = true;
                        i += 2;
                        continue;
                    }
                    match (grammar, c) {
                        (Grammar::Rust, '"')
                        | (Grammar::Sql, '\'' | '"')
                        | (Grammar::Css, '"' | '\'') => state = Lexer::Quoted(c),
                        (Grammar::Rust, 'r') => {
                            if let Some(hashes) = raw_string_hashes(&chars, i) {
                                state = Lexer::Raw(hashes);
                                i += 2 + hashes;
                                continue;
                            }
                        }
                        // A Rust char literal ('"', '\'', 'x') must not open a
                        // string; a lifetime ('a) has no closing quote two places
                        // on. An escaped literal closes after its escaped
                        // character, so '\'' ends at its fourth character.
                        (Grammar::Rust, '\'') => {
                            if next == Some('\\') {
                                let close = chars
                                    .get(i + 3..)
                                    .and_then(|rest| rest.iter().position(|&c| c == '\''));
                                i += close.map_or(1, |p| p + 4);
                                continue;
                            }
                            if chars.get(i + 2) == Some(&'\'') {
                                i += 3;
                                continue;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
        }
        // A CSS string cannot hold a raw newline: its tokenizer ends it there.
        if grammar == Grammar::Css && matches!(state, Lexer::Quoted(_)) && !newline_escaped {
            state = Lexer::Code;
        }
        masks.push(mask);
    }
    masks
}

fn scan_text(text: &str, grammar: Grammar) -> Vec<Hit> {
    let masks = comment_mask(text, grammar);
    let mut hits = Vec::new();
    for (index, (line, mask)) in text.lines().zip(masks.iter()).enumerate() {
        let chars: Vec<char> = line.chars().collect();
        for (start, len) in words_in(&chars) {
            hits.push(Hit {
                line: index + 1,
                word: chars[start..start + len].iter().collect(),
                in_comment: mask.get(start).copied().unwrap_or(false),
            });
        }
    }
    hits
}

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

/// `migrations/NNN_*.sql` with NNN <= 076, top level only.
fn is_historical_migration(rel: &str) -> bool {
    let Some(name) = rel.strip_prefix("migrations/") else {
        return false;
    };
    if name.contains('/') {
        return false;
    }
    name.get(..3)
        .and_then(|n| n.parse::<u32>().ok())
        .is_some_and(|n| n <= LAST_HISTORICAL_MIGRATION)
}

fn is_served_text(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| SERVED_TEXT_EXTENSIONS.contains(&e))
}

struct Census {
    /// Files read, per scanned root.
    files_scanned: BTreeMap<&'static str, usize>,
    /// Code hits (comments excluded) per file, historical migrations excluded,
    /// with the lines they sit on.
    code_hits: BTreeMap<String, Vec<(usize, String)>>,
    historical_hits: usize,
    comment_hits: usize,
}

fn census() -> Census {
    let repo = repo_root();
    let mut census = Census {
        files_scanned: BTreeMap::new(),
        code_hits: BTreeMap::new(),
        historical_hits: 0,
        comment_hits: 0,
    };
    for root in SCANNED_ROOTS {
        let path = repo.join(root.path);
        let mut files = Vec::new();
        if path.is_file() {
            files.push(path);
        } else {
            assert!(
                path.is_dir(),
                "{} is missing; the scan would pass by reading nothing",
                root.path
            );
            walk(&path, &mut files);
        }
        let mut read = 0;
        for path in files {
            let rel = relative(&path);
            let text = match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) if root.reach == Reach::Served && is_served_text(&path) => {
                    panic!(
                        "{rel}: served as text and unreadable as UTF-8 ({e}); the scan \
                         cannot skip it"
                    )
                }
                // Only text: a binary (an image, a video) is not read.
                Err(_) => continue,
            };
            read += 1;
            let grammar = match root.reach {
                Reach::Source => grammar_of(&path),
                Reach::Served => Grammar::Plain,
            };
            for hit in scan_text(&text, grammar) {
                if hit.in_comment {
                    census.comment_hits += 1;
                } else if is_historical_migration(&rel) {
                    census.historical_hits += 1;
                } else {
                    census
                        .code_hits
                        .entry(rel.clone())
                        .or_default()
                        .push((hit.line, hit.word));
                }
            }
        }
        census.files_scanned.insert(root.path, read);
    }
    census
}

// ──────────────────────────────────────────────────────────────────
// The checks
// ──────────────────────────────────────────────────────────────────

#[test]
fn every_legacy_word_in_code_is_on_the_allowlist() {
    let census = census();
    let mut problems = Vec::new();
    for (path, hits) in &census.code_hits {
        let lines = hits
            .iter()
            .map(|(line, word)| format!("{path}:{line} `{word}`"))
            .collect::<Vec<_>>()
            .join("\n      ");
        match SURVIVORS.iter().find(|s| s.path == path) {
            None => problems.push(format!(
                "{path}: {} hit(s) of the cannabis-era vocabulary outside a comment, and the \
                 file is not on the allowlist:\n      {lines}",
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
        "legacy vocabulary where none may be added (see the header of \
         tests/legacy_vocabulary_wiring.rs):\n  {}",
        problems.join("\n  ")
    );
}

#[test]
fn no_allowlist_entry_is_stale() {
    let census = census();
    let mut problems = Vec::new();
    for s in SURVIVORS {
        assert!(
            !s.reason.trim().is_empty(),
            "{}: an entry needs a reason",
            s.path
        );
        let found = census.code_hits.get(s.path).map_or(0, Vec::len);
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

/// D16: a scan that reads nothing reports nothing wrong. Pin that it read every
/// root and found the vocabulary where it is known to be.
#[test]
fn the_scan_reads_the_trees_and_sees_the_vocabulary() {
    let census = census();
    for root in SCANNED_ROOTS {
        let read = census.files_scanned.get(root.path).copied().unwrap_or(0);
        assert!(
            read > 0,
            "{}: no file read; the scan would pass by reading nothing there",
            root.path
        );
    }
    let total: usize = census.files_scanned.values().sum();
    assert!(
        total >= 200,
        "only {total} files scanned over all roots: {:?}",
        census.files_scanned
    );
    assert!(
        census.historical_hits > 0,
        "no hit in migrations 001-076, which hold the whole cannabis catalogue: the \
         scanner has gone blind"
    );
    assert!(
        census.comment_hits > 0,
        "no hit in any comment, where the retirement is documented: the comment \
         grammar or the matcher has gone blind"
    );
    assert!(
        census.code_hits.contains_key("src/ui/state.rs"),
        "`CartItemType::Strain` is not seen in src/ui/state.rs"
    );
}

#[test]
fn the_matcher_reads_whole_words_only() {
    let words = |s: &str| -> Vec<String> {
        scan_text(s, Grammar::Plain)
            .into_iter()
            .map(|h| h.word)
            .collect()
    };
    assert_eq!(words("CartItemType::Strain"), ["Strain"]);
    assert_eq!(words("\"strain\" => Ok(\"strain\")"), ["strain", "strain"]);
    assert_eq!(words("No {0} strains found"), ["strains"]);
    assert_eq!(words("woody-weed-bot-production"), ["weed"]);
    assert_eq!(words("THC: 20% / CBD"), ["THC", "CBD"]);
    assert_eq!(words("per gram, 10 grams"), ["gram", "grams"]);
    assert_eq!(words("Наши премиальные сорта"), ["сорта"]);
    assert_eq!(words("Не найдено {0} сортов"), ["сортов"]);
    // Not hits: inside a longer word or a compound identifier.
    assert!(words("telegram program instagram").is_empty());
    assert!(words("strain_of_day StrainOfDay ApiStrain strained restrain").is_empty());
    assert!(words("сортировка по цене").is_empty());
    assert!(words("weedy thcx cbdx grammar").is_empty());
}

#[test]
fn comments_are_told_apart_from_code() {
    let kinds = |s: &str, g: Grammar| -> Vec<bool> {
        scan_text(s, g).into_iter().map(|h| h.in_comment).collect()
    };
    // Rust: a line comment, a doc comment, a block comment across lines.
    assert_eq!(
        kinds("let k = \"strain\"; // strain", Grammar::Rust),
        [false, true]
    );
    assert_eq!(kinds("/// a strain card", Grammar::Rust), [true]);
    assert_eq!(
        kinds(
            "/* the strain\n   menu */ let x = \"strain\";",
            Grammar::Rust
        ),
        [true, false]
    );
    // A `//` inside a string does not start a comment.
    assert_eq!(
        kinds("let u = \"https://x/strain\"; // weed", Grammar::Rust),
        [false, true]
    );
    // A char literal holding a quote does not open a string.
    assert_eq!(
        kinds("s.replace('\"', \"\"); // strain", Grammar::Rust),
        [true]
    );
    assert_eq!(
        kinds("let k = '\\''; let s = \"weed\";", Grammar::Rust),
        [false]
    );
    // SQL: `--` comments, and a `--` inside a string is text.
    assert_eq!(
        kinds("INSERT INTO t VALUES ('strain'); -- strain", Grammar::Sql),
        [false, true]
    );
    assert_eq!(kinds("SELECT '--strain';", Grammar::Sql), [false]);
    // CSS: only block comments.
    assert_eq!(
        kinds("/* weed */ .weed { color: red; }", Grammar::Css),
        [true, false]
    );
    // Unknown grammar: everything is code.
    assert_eq!(kinds("// strain", Grammar::Plain), [false]);
}

/// Added 2026-09-24. Until then the mask reset its string state at every line
/// end, so on the continuation line of a string that spans lines a `//` (a URL)
/// started a comment and hid the rest of the line, and a `/*` hid every line up
/// to the next `*/`. Every case is checked and every miss reported at once.
#[test]
fn a_string_is_a_string_on_every_line_it_spans() {
    let kinds = |s: &str, g: Grammar| -> Vec<bool> {
        scan_text(s, g).into_iter().map(|h| h.in_comment).collect()
    };
    let cases: &[(&str, Grammar, &[bool])] = &[
        // The shape of the CSP header in src/main.rs: a `\` continuation line
        // holding a URL. The `//` is inside the string.
        (
            "let csp = \"default-src 'self'; \\\n     script-src https://telegram.org; strain\";",
            Grammar::Rust,
            &[false],
        ),
        // A `/*` on a continuation line is text too. Read as a comment, it hid
        // every line after it, here to the end of the file.
        (
            "let s = \"a\nb /* strain\nc\";\nlet w = \"weed\";",
            Grammar::Rust,
            &[false, false],
        ),
        // Raw strings, any number of `#`: a `"` inside does not end them, and a
        // `//` inside is text, on the opening line or on a later one.
        (
            r###"let r = r#"say "hi // strain"#; // weed"###,
            Grammar::Rust,
            &[false, true],
        ),
        (
            "let r = r##\"one \"# two\nhttps://x.example/ strain\n\"##; let t = \"thc\";",
            Grammar::Rust,
            &[false, false],
        ),
        (
            r####"let b = br##"a "# // "##; // strain"####,
            Grammar::Rust,
            &[true],
        ),
        // A raw identifier is not a raw string.
        ("let r#type = bar; // strain", Grammar::Rust, &[true]),
        // An escaped char literal ends after the escaped character: `'\''` next
        // to `'"'` must not open a string.
        (r#"let q = ['\'','"']; // strain"#, Grammar::Rust, &[true]),
        // Lifetimes are not char literals.
        (
            "fn f<'a>(x: &'a str) -> &'a str { x } // strain",
            Grammar::Rust,
            &[true],
        ),
        // Rust block comments nest.
        (
            "/* a /* b */ strain */ let w = \"weed\";",
            Grammar::Rust,
            &[true, false],
        ),
        // SQL: a quoted literal may span lines, and a quoted identifier is not a
        // comment.
        (
            "INSERT INTO t VALUES ('one\n-- strain');\n-- weed",
            Grammar::Sql,
            &[false, true],
        ),
        ("SELECT \"x -- strain\" FROM t;", Grammar::Sql, &[false]),
        // CSS: an escaped quote does not end a string.
        (
            ".a::after { content: \"\\\" /* strain\"; }",
            Grammar::Css,
            &[false],
        ),
    ];
    let wrong: Vec<String> = cases
        .iter()
        .filter_map(|(src, g, want)| {
            let got = kinds(src, *g);
            (got != *want).then(|| format!("{g:?} {src:?}: in_comment {got:?}, want {want:?}"))
        })
        .collect();
    assert!(
        wrong.is_empty(),
        "the comment mask misread {} of {} case(s):\n  {}",
        wrong.len(),
        cases.len(),
        wrong.join("\n  ")
    );
}

#[test]
fn only_migrations_up_to_076_are_history() {
    assert!(is_historical_migration("migrations/001_initial.sql"));
    assert!(is_historical_migration(
        "migrations/076_promo_broadcast.sql"
    ));
    assert!(!is_historical_migration("migrations/077_bikes.sql"));
    assert!(!is_historical_migration(
        "migrations/083_drop_cannabis_catalog.sql"
    ));
    assert!(!is_historical_migration(
        "migrations/wip/077_quest_progress.sql"
    ));
    assert!(!is_historical_migration("src/db/mod.rs"));
}
