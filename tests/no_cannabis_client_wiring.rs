//! Nothing cannabis-related anywhere: the owner's ruling of 2026-09-25 (#12),
//! client half.
//!
//! The owner answered a numbered list on 2026-09-25, and item 12 reads, in
//! translation: everything that concerns cannabis must be nowhere. This tree is
//! a derivative of a cannabis shop, and the Mini App still carried the shop in
//! four places a customer can reach: translated sentences, strings a screen
//! writes itself, the served text files (the page, the stylesheets, the game
//! scripts) and the served media under `assets/`. The sweep that answered the
//! ruling is recorded in `specs/turbobaby/legacy_retirement.t27` (the
//! `CANNABIS_RULING_*` declarations); this file is what keeps it answered.
//!
//! `tests/legacy_vocabulary_wiring.rs` already refuses a NEW `strain`, `weed`,
//! `thc`, `cbd`, `gram` or Russian `сорт` anywhere in `src/`, `migrations/` and
//! the served roots. It is deliberately narrow -- a vocabulary guard over the
//! whole tree, with an allowlist of every file that must still spell those
//! words. This file is the other shape: a WIDE vocabulary (the old shop's
//! brand, the plant, its slang, its paraphernalia, in Russian, English and
//! Thai) over the NARROW set of text a customer is shown or sent, so that a
//! word a screen could put in front of somebody fails here even when the
//! narrow guard has no opinion about it.
//!
//! What is read, and why only that:
//!
//! * every translation arm of `src/trios/i18n.rs`, in both tables -- the only
//!   copy a screen renders through `t()`; the other six enumerated languages
//!   fall through to the English table, so two tables are every language;
//! * every string LITERAL under `src/ui` -- the only other place a screen gets
//!   text from. Identifiers and comments are not read: a type named for the
//!   old catalogue reaches no customer, and the retirement is documented in
//!   comments that have to name what they retired;
//! * the served text files: the repository-root `index.html` (Trunk renders
//!   the served `dist/index.html` from it; `dist/` itself is a build output and
//!   is re-read only after a rebuild), `styles/*.css`, and every text file under
//!   `assets/`, all read WHOLE, comments included, because they reach the
//!   browser byte for byte;
//! * the `Trunk*.toml` files, which are tooling rather than served text, but
//!   whose dev proxies used to point at the old shop's backend.
//!
//! Matching is whole-word for the Latin vocabulary and for the Russian word for
//! strain, so `budget`, `buildWoody`, `strain_id` and `сортировка` are not hits
//! and `"woody:contact"` is. The rest of the Russian vocabulary is matched by
//! stem, because the words inflect. The Thai vocabulary is matched as a
//! substring too: Thai is written without spaces between words, so a
//! whole-word match would find the word only where it stands alone and miss it
//! in any ordinary phrase (`ร้านกัญชา`, a cannabis shop). What may still hit
//! is named in [`IDENTIFIER_SURVIVORS`], each with an exact count and a reason,
//! and the list is checked from both ends.

#![allow(clippy::panic, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|e| panic!("{relative} must be readable: {e}"))
        .replace("\r\n", "\n")
}

fn relative(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .expect("under the repository")
        .to_string_lossy()
        .replace('\\', "/")
}

fn files_under(dir: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![repo_root().join(dir)];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).unwrap_or_else(|e| panic!("read_dir {d:?}: {e}")) {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

// ── The vocabulary ──────────────────────────────────────────────────────────

/// Latin-script words, matched whole and case-insensitively. Each is the old
/// shop's brand, the plant, its slang, a form of it, or its paraphernalia.
/// `discreet` and `medical-use` are here because the checkout promised both
/// until the ruling: they were the cannabis-delivery pitch, and a bike rental
/// has no use for either. `joint` and `blunt` are not: they are ordinary
/// English first (the vendored three.js names hand joints six times), and
/// `pre-roll` covers the product.
const LATIN_WORDS: &[&str] = &[
    "cannabis",
    "marijuana",
    "marihuana",
    "ganja",
    "weed",
    "weeds",
    "weedpecker",
    "woody",
    "strain",
    "strains",
    "sativa",
    "indica",
    "thc",
    "cbd",
    "gacp",
    "kush",
    "bud",
    "buds",
    "pre-roll",
    "pre-rolls",
    "preroll",
    "prerolls",
    "dispensary",
    "dispensaries",
    "hemp",
    "bong",
    "bongs",
    "grinder",
    "grinders",
    "stoner",
    "stoned",
    "sommelier",
    "discreet",
    "medical-use",
    "420",
];

/// Thai: cannabis and hemp. Matched as a substring, like the Russian stems:
/// Thai letters are word characters and Thai puts no space between words, so
/// a word boundary exists only at the edge of a phrase. No ordinary Thai word
/// contains either of these; every compound that does is the plant.
const THAI_WORDS: &[&str] = &["กัญชา", "กัญชง"];

/// Russian stems, matched as a substring of a lower-cased text: the words
/// inflect, and every one of these stems belongs to the plant, its slang, its
/// paraphernalia or the old brand. `трав` alone is not here -- it is grass and
/// herbs in general -- but its slang diminutive and its plain forms as a whole
/// word are. `бонг` and `гриндер` are the two paraphernalia chips the
/// accessories screen offered in Russian until the ruling; the other two
/// («Бумага», «Трубка») are ordinary words and are not.
const RUSSIAN_STEMS: &[&str] = &[
    "каннабис",
    "канабис",
    "конопл",
    "марихуан",
    "травк",
    "шишк",
    "шишек",
    "бошк",
    "бошек",
    "гашиш",
    "косяк",
    "накур",
    "укур",
    "диспансер",
    "бонг",
    "гриндер",
    "вуди",
    "тгк",
    "кбд",
    "медицинского использования",
];

/// Russian words matched whole: the word for strain in its case forms
/// (`сортировка`, sorting, must not hit), the plain forms of «трава», and the
/// two plant types, whole because their stems begin ordinary words
/// (`индикатор`).
const RUSSIAN_WORDS: &[&str] = &[
    "сортами",
    "сортов",
    "сортом",
    "сортах",
    "сорта",
    "сорту",
    "сорты",
    "сорт",
    "трава",
    "травы",
    "траву",
    "травой",
    "индика",
    "индики",
    "индику",
    "сатива",
    "сативы",
    "сативу",
];

/// The herb and leaf emoji: the old menu's mark for the plant. Checked in the
/// translation arms only; the admin screen's "nature" theme option is not
/// customer copy.
const EMOJI: &[&str] = &["🌿", "🍃"];

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Every whole-word occurrence of `word` in `text` (both already lower-cased).
fn whole_word_hits(text: &str, word: &str) -> usize {
    let mut hits = 0;
    let mut from = 0;
    while let Some(at) = text[from..].find(word) {
        let start = from + at;
        let end = start + word.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        let bounded_before = before.is_none_or(|c| !is_word_char(c));
        // `420px` is a CSS width, not the number; any other letter or digit
        // after it makes it part of a longer token already.
        let bounded_after = after.is_none_or(|c| !is_word_char(c));
        if bounded_before && bounded_after {
            hits += 1;
        }
        from = end;
    }
    hits
}

/// The vocabulary words `text` holds, one entry per occurrence.
fn vocabulary_in(text: &str, with_emoji: bool) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut found = Vec::new();
    for word in LATIN_WORDS.iter().chain(RUSSIAN_WORDS) {
        for _ in 0..whole_word_hits(&lower, word) {
            found.push((*word).to_string());
        }
    }
    for stem in THAI_WORDS.iter().chain(RUSSIAN_STEMS) {
        for _ in 0..lower.matches(stem).count() {
            found.push((*stem).to_string());
        }
    }
    if with_emoji {
        for glyph in EMOJI {
            for _ in 0..text.matches(glyph).count() {
                found.push((*glyph).to_string());
            }
        }
    }
    found
}

// ── Rust string literals ────────────────────────────────────────────────────

/// Every string literal in a Rust source, as (line, contents). Comments,
/// identifiers, char literals and lifetimes are skipped; plain, byte and raw
/// strings (any number of `#`) are read, across lines.
fn string_literals(source: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut line = 1;
    let mut i = 0;
    let begins_token = |k: usize| k == 0 || !is_word_char(chars[k - 1]);
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            '\n' => {
                line += 1;
                i += 1;
            }
            '/' if next == Some('/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if next == Some('*') => {
                let mut depth = 0;
                while i < chars.len() {
                    if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                        depth += 1;
                        i += 2;
                    } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                        depth -= 1;
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        if chars[i] == '\n' {
                            line += 1;
                        }
                        i += 1;
                    }
                }
            }
            'r' if begins_token(i) || (i > 0 && chars[i - 1] == 'b' && begins_token(i - 1)) => {
                let hashes = chars[i + 1..].iter().take_while(|&&h| h == '#').count();
                if chars.get(i + 1 + hashes) == Some(&'"') {
                    let start_line = line;
                    let mut j = i + 2 + hashes;
                    let mut text = String::new();
                    while j < chars.len() {
                        if chars[j] == '"' && (1..=hashes).all(|k| chars.get(j + k) == Some(&'#')) {
                            break;
                        }
                        if chars[j] == '\n' {
                            line += 1;
                        }
                        text.push(chars[j]);
                        j += 1;
                    }
                    out.push((start_line, text));
                    i = j + 1 + hashes;
                } else {
                    i += 1;
                }
            }
            '"' => {
                let start_line = line;
                let mut j = i + 1;
                let mut text = String::new();
                while j < chars.len() && chars[j] != '"' {
                    if chars[j] == '\\' {
                        if let Some(&escaped) = chars.get(j + 1) {
                            if escaped == '\n' {
                                line += 1;
                            }
                            text.push(escaped);
                        }
                        j += 2;
                        continue;
                    }
                    if chars[j] == '\n' {
                        line += 1;
                    }
                    text.push(chars[j]);
                    j += 1;
                }
                out.push((start_line, text));
                i = j + 1;
            }
            '\'' => {
                // A char literal, escaped or plain; otherwise a lifetime.
                if next == Some('\\') {
                    let close = chars
                        .get(i + 3..)
                        .and_then(|rest| rest.iter().position(|&q| q == '\''));
                    i += close.map_or(1, |p| p + 4);
                } else if chars.get(i + 2) == Some(&'\'') {
                    i += 3;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    out
}

// ── What may still hit, and why ─────────────────────────────────────────────

/// A file allowed to carry vocabulary in the text this file reads, with the
/// exact number of hits and the reason. Every entry is an IDENTIFIER that
/// happens to be a string: a wire value the server or a saved record still
/// uses, a storage key, or an event name. None is shown to anybody.
struct Survivor {
    path: &'static str,
    hits: usize,
    reason: &'static str,
}

const IDENTIFIER_SURVIVORS: &[Survivor] = &[
    Survivor {
        path: "src/ui/state.rs",
        hits: 1,
        reason: "`CartItemType::from` reads the kind string \"strain\" off a cart line saved \
                 before migration 083 (tests/legacy_vocabulary_wiring.rs, WIRE_CART). A wire \
                 value, never rendered.",
    },
    Survivor {
        path: "src/ui/api/http.rs",
        hits: 1,
        reason: "`cart_item_type_to_kind` is an exhaustive match and maps the old variant back \
                 to the server's kind string \"strain\". A wire value, never rendered.",
    },
    Survivor {
        path: "src/ui/share.rs",
        hits: 2,
        reason: "`ProductKind::Strain`'s wire name \"strain\" for share links already sent \
                 before 083, and the test fixture that parses one. Links resolve to the \
                 catalog; the word is never rendered.",
    },
    Survivor {
        path: "src/ui/screens/profile_screen.rs",
        hits: 1,
        reason: "A loyalty tier name the server may still send for an old account (\"woody\", \
                 the old shop's top tier) is matched and shown as the Gold wheel. The name is \
                 read, never rendered.",
    },
    Survivor {
        path: "src/ui/screens/checkout_screen.rs",
        hits: 1,
        reason: "The `woody:contact` DOM event name the Telegram contact bridge in \
                 src/ui/telegram.rs dispatches. An event name inside the bundle, never \
                 rendered; renaming it is safe but is not copy, and was left to a separate \
                 change.",
    },
    Survivor {
        path: "src/ui/telegram.rs",
        hits: 3,
        reason: "The `woody:mainbutton` event (dispatched and listened for) and the \
                 `woody:contact` event (dispatched), the bridge between Telegram's callbacks \
                 and the WASM listeners. Event names, never rendered.",
    },
    Survivor {
        path: "src/ui/routes.rs",
        hits: 1,
        reason: "The `/sommelier` route path stays declared so the `startapp=sommelier` deep \
                 links already sitting in customers' chats land on the catalog instead of a \
                 router miss. A path, never rendered; the screen behind it is deleted.",
    },
    Survivor {
        path: "index.html",
        hits: 2,
        reason: "The `woody:telegram-ready` event the page dispatches when the Telegram SDK \
                 loads and listens for itself. An event name in served markup, never \
                 rendered; renaming it is safe but is not copy, and was left to a separate \
                 change.",
    },
];

/// The media (non-text) files `assets/` may serve. Every one of them is a
/// decision: the old shop's photographs, videos, pack art, member cards and
/// garden sprites were removed from `assets/` on 2026-09-25, and a new image
/// has to be added here, with the path that loads it, before it ships.
const SERVED_MEDIA: &[(&str, &str)] = &[(
    "assets/logo.jpg",
    "the owner's channel avatar (DECISIONS.md D20), loaded as `logo::MAIN` and by the ride \
     screen's start card",
)];

/// Text extensions `assets/` serves; anything else there is media.
const TEXT_EXTENSIONS: &[&str] = &["html", "js", "css", "json", "svg", "md", "txt"];

// ── The census ──────────────────────────────────────────────────────────────

struct Census {
    /// Hits per file, with the line and the word, outside the translation table.
    hits: Vec<(String, usize, String)>,
    /// Hits inside translation arms (never allowed).
    arm_hits: Vec<(usize, String, String)>,
    arms_read: usize,
    literals_read: usize,
    ui_files_read: usize,
    served_files_read: usize,
}

fn census() -> Census {
    let mut census = Census {
        hits: Vec::new(),
        arm_hits: Vec::new(),
        arms_read: 0,
        literals_read: 0,
        ui_files_read: 0,
        served_files_read: 0,
    };

    // 1. Translation arms: `T_KEY => "value",` in both tables.
    let i18n = read("src/trios/i18n.rs");
    for (index, line) in i18n.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("T_") {
            continue;
        }
        let Some(arrow) = line.find("=> \"") else {
            continue;
        };
        let value = &line[arrow + 4..line.rfind('"').unwrap_or(line.len())];
        census.arms_read += 1;
        for word in vocabulary_in(value, true) {
            census.arm_hits.push((index + 1, word, trimmed.to_string()));
        }
    }

    // 2. String literals a screen writes itself.
    for path in files_under("src/ui") {
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let rel = relative(&path);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{rel}: {e}"))
            .replace("\r\n", "\n");
        census.ui_files_read += 1;
        for (line, literal) in string_literals(&source) {
            census.literals_read += 1;
            for word in vocabulary_in(&literal, false) {
                census.hits.push((rel.clone(), line, word));
            }
        }
    }

    // 3. Served text, whole: the page, the stylesheets, the text under assets/.
    //    And the Trunk configs, read the same way.
    let mut served: Vec<PathBuf> = vec![repo_root().join("index.html")];
    served.extend(
        files_under("styles")
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "css")),
    );
    served.extend(files_under("assets").into_iter().filter(|p| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| TEXT_EXTENSIONS.contains(&e))
    }));
    for entry in fs::read_dir(repo_root()).expect("repository root") {
        let path = entry.expect("dir entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with("Trunk") && name.ends_with(".toml") {
            served.push(path);
        }
    }
    for path in served {
        let rel = relative(&path);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{rel}: served as text and unreadable: {e}"))
            .replace("\r\n", "\n");
        census.served_files_read += 1;
        for (index, line) in text.lines().enumerate() {
            for word in vocabulary_in(line, false) {
                census.hits.push((rel.clone(), index + 1, word));
            }
        }
    }
    census
}

// ── The checks ──────────────────────────────────────────────────────────────

#[test]
fn no_translated_sentence_carries_the_old_shop() {
    let census = census();
    assert!(
        census.arm_hits.is_empty(),
        "a translation arm a screen can render carries cannabis-era vocabulary (owner's \
         ruling 2026-09-25, #12 -- delete the key or cut the clause; never reword it into a \
         new sentence nobody approved):\n  {}",
        census
            .arm_hits
            .iter()
            .map(|(line, word, arm)| format!("src/trios/i18n.rs:{line} `{word}` in {arm}"))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
    // D16: two tables, one arm per declared key in each. A parser that stopped
    // matching would read nothing and pass.
    let declared = read("src/trios/i18n.rs")
        .lines()
        .filter(|l| l.starts_with("pub const T_") && l.contains(": Key = \""))
        .count();
    assert_eq!(
        census.arms_read,
        declared * 2,
        "read {} translation arms for {declared} declared keys; the arm parser is blind",
        census.arms_read
    );
}

#[test]
fn every_other_hit_is_a_named_identifier() {
    let census = census();
    let mut problems = Vec::new();
    let mut paths: Vec<&str> = census.hits.iter().map(|(p, _, _)| p.as_str()).collect();
    paths.sort_unstable();
    paths.dedup();
    for path in paths {
        let lines: Vec<String> = census
            .hits
            .iter()
            .filter(|(p, _, _)| p == path)
            .map(|(p, line, word)| format!("{p}:{line} `{word}`"))
            .collect();
        match IDENTIFIER_SURVIVORS.iter().find(|s| s.path == path) {
            None => problems.push(format!(
                "{path}: {} hit(s) of cannabis-era vocabulary in text a customer can be \
                 shown or sent, and the file is not on the list:\n      {}",
                lines.len(),
                lines.join("\n      ")
            )),
            Some(s) if lines.len() > s.hits => problems.push(format!(
                "{path}: {} hit(s), the list admits {} -- a NEW occurrence:\n      {}",
                lines.len(),
                s.hits,
                lines.join("\n      ")
            )),
            Some(_) => {}
        }
    }
    assert!(
        problems.is_empty(),
        "owner's ruling 2026-09-25 (#12): nothing cannabis-related anywhere.\n  {}",
        problems.join("\n  ")
    );
}

#[test]
fn no_survivor_entry_is_stale() {
    let census = census();
    let mut problems = Vec::new();
    for s in IDENTIFIER_SURVIVORS {
        assert!(s.reason.len() > 40, "{}: an entry needs a reason", s.path);
        let found = census.hits.iter().filter(|(p, _, _)| p == s.path).count();
        if found != s.hits {
            problems.push(format!(
                "{}: the list admits {} hit(s) and the file holds {found}. If an identifier \
                 went, lower the count (or drop the entry) in the same change.",
                s.path, s.hits
            ));
        }
    }
    let mut paths: Vec<_> = IDENTIFIER_SURVIVORS.iter().map(|s| s.path).collect();
    paths.sort_unstable();
    paths.dedup();
    assert_eq!(
        paths.len(),
        IDENTIFIER_SURVIVORS.len(),
        "a file is listed twice"
    );
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// D16: the three scans read real text. Measured 2026-09-25: 1102 arms, well
/// over twenty screen files, and the page plus its stylesheets and scripts.
#[test]
fn the_scans_read_the_client() {
    let census = census();
    assert!(census.ui_files_read > 20, "{}", census.ui_files_read);
    assert!(census.literals_read > 1000, "{}", census.literals_read);
    assert!(
        census.served_files_read >= 8,
        "{}",
        census.served_files_read
    );
    // The survivors are seen, so the literal lexer is not blind either.
    assert!(census
        .hits
        .iter()
        .any(|(p, _, w)| p == "src/ui/state.rs" && w == "strain"));
}

#[test]
fn the_matcher_reads_words_and_stems() {
    let words = |s: &str| vocabulary_in(s, true);
    assert_eq!(words("No {0} strains found"), ["strains"]);
    assert_eq!(words("Наши премиальные сорта"), ["сорта"]);
    assert_eq!(words("🌿 Доставка травы!"), ["травы", "🌿"]);
    assert_eq!(words("Вуди подошёл к столу"), ["вуди"]);
    assert_eq!(words("and agree to medical-use terms"), ["medical-use"]);
    assert_eq!(words("Fast & discreet"), ["discreet"]);
    assert_eq!(words("'woody:contact'"), ["woody"]);
    assert_eq!(words("Всё для курения, бонг и шишки"), ["шишк", "бонг"]);
    assert_eq!(words("Гриндер"), ["гриндер"]);
    assert_eq!(words("กัญชา"), ["กัญชา"]);
    // Thai has no spaces between words: the word inside a phrase is a hit.
    assert_eq!(words("ร้านกัญชา"), ["กัญชา"]);
    assert_eq!(words("ขายกัญชาที่ภูเก็ต"), ["กัญชา"]);
    assert_eq!(words("กัญชงไทย"), ["กัญชง"]);
    // Not hits: longer tokens, identifiers, sorting, a CSS width, grass in general.
    assert!(words("budget buildWoody strain_id woody_last_zone_id").is_empty());
    assert!(words("Сортировка по цене").is_empty());
    assert!(words("max-width: 420px").is_empty());
    assert!(words("чай из трав").is_empty());
    assert!(words("เช่ารถมอเตอร์ไซค์ที่ภูเก็ต").is_empty());
}

#[test]
fn the_literal_lexer_reads_strings_and_skips_comments() {
    let src = "let a = \"one\"; // \"not me\"\n/* \"nor me\" */ let b = r#\"raw \"two\"\"#;\nlet c = '\"'; let d = \"three\\\"x\";";
    let got: Vec<String> = string_literals(src).into_iter().map(|(_, s)| s).collect();
    assert_eq!(got, ["one", "raw \"two\"", "three\"x"]);
}

// ── Media ───────────────────────────────────────────────────────────────────

/// `assets/` is served at /assets and at /images. On 2026-09-25 it held 128
/// files of the old shop -- strain photographs and videos, pack and member-card
/// art, paraphernalia product shots, the garden's growth sprites, the mascot
/// portrait and the orphaned catch game -- and every one of them was removed.
/// What may be served as media from now on is the list above.
#[test]
fn assets_serve_no_media_nobody_decided_on() {
    let mut unlisted = Vec::new();
    let mut seen = Vec::new();
    for path in files_under("assets") {
        let rel = relative(&path);
        let is_text = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| TEXT_EXTENSIONS.contains(&e));
        if is_text {
            continue;
        }
        if SERVED_MEDIA.iter().any(|(p, _)| *p == rel) {
            seen.push(rel);
        } else {
            unlisted.push(rel);
        }
    }
    assert!(
        unlisted.is_empty(),
        "assets/ serves media nobody listed in SERVED_MEDIA (add it with the path that loads \
         it, or do not ship it):\n  {}",
        unlisted.join("\n  ")
    );
    for (path, reason) in SERVED_MEDIA {
        assert!(
            seen.iter().any(|s| s == path),
            "{path} is listed ({reason}) and no longer exists"
        );
    }
}

/// The client may not name a media path under /assets that the tree no longer
/// ships: `src/ui/assets.rs` held six member-card constants whose artwork was a
/// cannabis bud, and they went with the files.
#[test]
fn the_asset_table_names_no_removed_artwork() {
    let table = read("src/ui/assets.rs");
    for gone in ["member-cards", "packs/", "game/1.png", "Mac-1", "ava.webp"] {
        let code: String = string_literals(&table)
            .into_iter()
            .map(|(_, s)| s)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains(gone),
            "src/ui/assets.rs names removed artwork again: {gone}"
        );
    }
    assert!(
        table.contains("pub const MAIN: &str = \"/assets/logo.jpg\";"),
        "the one asset constant the client loads has moved; this check is blind"
    );
}

// ── The copy the sweep cut, pinned so it does not grow back ──────────────────

/// The accessories screen offered four paraphernalia filter chips and a
/// smoking-gear subtitle. The chips list and the label arms are gone; the
/// screen keeps its generic categories.
#[test]
fn the_accessories_screen_offers_no_paraphernalia() {
    let screen = read("src/ui/screens/accessories_screen.rs");
    let literals: Vec<String> = string_literals(&screen)
        .into_iter()
        .map(|(_, s)| s.to_lowercase())
        .collect();
    for gone in ["grinder", "papers", "rolling", "pipe", "bong", "vaporizer"] {
        assert!(
            !literals.iter().any(|l| l == gone),
            "src/ui/screens/accessories_screen.rs matches or offers `{gone}` again"
        );
    }
    assert!(
        screen.contains("const CATEGORIES: &[&str] = &[\"All\","),
        "the category chip list moved; this check is blind"
    );
}

/// Five arms were cut to their non-cannabis half and two were aligned with
/// their other-language twin (`specs/turbobaby/legacy_retirement.t27`,
/// `CANNABIS_RULING_ARMS_CUT` and `CANNABIS_RULING_ARMS_ALIGNED_TO_THEIR_TWIN`).
/// Every arm of those five keys is pinned whole, Russian table first, so a cut
/// that went too far, a clause that grew back, or a cut that quietly gained
/// something (an emoji, an exclamation mark) is seen: the ruling removed copy
/// and wrote none.
///
/// One of the five is pinned absent instead. The checkout age notice kept its
/// age half after the cut, and later the same day the owner removed the 20+
/// box it belonged to, for now (answer 1 of the second list, 2026-09-25;
/// `specs/turbobaby/checkout_contact.t27`), so its key was deleted whole.
#[test]
fn the_cut_sentences_keep_their_rental_half() {
    let i18n = read("src/trios/i18n.rs");
    let arms = |key: &str| -> Vec<String> {
        let prefix = format!("{key} => \"");
        i18n.lines()
            .filter_map(|l| l.trim_start().strip_prefix(&prefix))
            .map(|rest| {
                let rest = rest.trim_end();
                rest.strip_suffix("\",").unwrap_or(rest).to_string()
            })
            .collect()
    };
    // Cut on 2026-09-25 to its age half, then deleted whole the same day with
    // the 20+ box: no arm may come back in either table.
    assert_eq!(
        arms("T_CHECKOUT_AGE_NOTICE"),
        Vec::<String>::new(),
        "T_CHECKOUT_AGE_NOTICE has an arm again; the 20+ box it belonged to was removed"
    );
    let pinned: &[(&str, [&str; 2], &str)] = &[
        (
            "T_SUCCESS_BACK_MENU",
            ["В меню", "Back to Menu"],
            "cut: the Russian arm lost its herb emoji; the English arm had none",
        ),
        (
            "T_GAME_EVENT_HERB_DELIVERY",
            ["Все грядки политы", "All farm plots watered"],
            "cut: both lost the herb-delivery half and gained nothing",
        ),
        (
            "T_GAME_LOG_MOVED_TO_TABLE",
            ["TurboBaby подошёл к столу {0}", "TurboBaby moved to table {0}"],
            "aligned: the Russian arm named the old mascot, the English one TurboBaby",
        ),
        (
            "T_CHECKOUT_TRUST_TITLE",
            ["Почему нам доверяют", "Why people trust us"],
            "aligned: the English arm promised discretion, the Russian one asked why people trust us",
        ),
    ];
    for (key, want, why) in pinned {
        assert_eq!(
            arms(key),
            want.to_vec(),
            "{key} ({why}) -- one arm per table, exactly as the sweep left it"
        );
    }
}
