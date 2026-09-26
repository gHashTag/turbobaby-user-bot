//! Source-level guard for the public TurboBaby navigation cutover.
//!
//! Legacy screens still compile while their data paths are retired. These
//! checks make the customer-facing entry points a much narrower contract: the
//! ordinary root route is the bike catalog, and the tab bar advertises only
//! fleet, Ride, orders, cart, and profile.
//!
//! The first version of this file read three sources — `routes.rs`,
//! `bottom_nav.rs`, `i18n.rs` — and asserted the retirement against the tab bar
//! alone. That was enough to keep `bottom_nav.rs` honest and nothing else: the
//! profile tab still offered «🌱 Мой сад», the empty cart offered «В наборы»,
//! the empty order list offered «Смотреть наборы», and the home screen carried
//! a six-tile CATEGORIES grid where five tiles led to retired screens. Every
//! one of those files was invisible to this test. A guard that reads one copy
//! of a decision cannot notice the other four, so the checks below walk the
//! screens themselves.

use std::fs;
use std::path::{Path, PathBuf};

const ROUTES: &str = include_str!("../src/ui/routes.rs");
const BOTTOM_NAV: &str = include_str!("../src/ui/components/bottom_nav.rs");
const I18N: &str = include_str!("../src/trios/i18n.rs");

/// Screens a customer can reach without a deep link: the five tab-bar
/// destinations, the catalog behind `/` and `/menu`, and the two screens the
/// checkout funnel ends on.
///
/// The referrals page joins them because every referral notification the bot
/// sends now carries a button to it — `notification_queue` builds the deep
/// link, and a message a customer receives is a shorter road to a screen than
/// a tab they have to find.
///
/// `share.rs` joins them for the stronger version of the same reason. It is not
/// a screen but the router that turns an incoming `startapp` payload into one,
/// and it sat outside this list until 2026-09-16 while four of its five product
/// kinds pointed at retired screens. A tab a customer never presses costs
/// nothing; a deep link they did not choose to follow puts them on a dead
/// screen with no way back.
const LIVE_SURFACES: [&str; 11] = [
    "src/ui/screens/home_screen.rs",
    "src/ui/screens/catalog_screen.rs",
    "src/ui/screens/menu_screen.rs",
    "src/ui/screens/ride_screen.rs",
    "src/ui/screens/orders_screen.rs",
    "src/ui/screens/cart_screen.rs",
    "src/ui/screens/profile_screen.rs",
    "src/ui/screens/checkout_screen.rs",
    "src/ui/screens/success_screen.rs",
    "src/ui/pages/referrals.rs",
    "src/ui/share.rs",
];

/// Routes whose data the migrations retire: 083 drops the strain and garden
/// tables outright, 085 unpublishes the remaining cannabis catalogue.
const RETIRED_ROUTES: [&str; 8] = [
    "Garden",
    "Sets",
    "Sommelier",
    "Accessories",
    "Tea",
    "Events",
    "EventDetail",
    "MyBookings",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Read a shipped source file, with the file's own absence treated as a
/// failure rather than as a pass. A renamed screen must break this test, not
/// silently empty it.
fn live_source(relative: &str) -> String {
    let path = repo_root().join(relative);
    // Normalised to LF: several checks below anchor on "\n", and a Windows
    // checkout (`core.autocrlf`) would hand them CRLF and match nothing. A
    // guard that quietly stops matching is the failure mode these guards
    // exist to prevent.
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{relative} is a live customer surface and must exist: {e}"))
        .replace("\r\n", "\n")
}

/// Strip `//` line comments so the retirement notes left where the removed
/// links used to be — they name the routes on purpose — cannot be mistaken for
/// live navigation.
fn code_of(source: &str) -> String {
    source
        .lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn ordinary_home_visit_renders_the_bike_catalog() {
    let start = ROUTES.find("fn Home() -> Element").expect("Home route");
    let end = ROUTES[start..]
        .find("fn Menu() -> Element")
        .map(|offset| start + offset)
        .expect("Menu route after Home");
    let home = &ROUTES[start..end];

    assert!(home.contains("if has_compatibility_redirect"));
    assert!(home.contains("HomeScreen {}"));
    // Checked as a claim, not as a substring: the previous form embedded a
    // newline and sixteen spaces, so it broke on a CRLF checkout and would
    // break again on any reindent of a file this test does not own.
    let otherwise = home
        .split("else {")
        .nth(1)
        .expect("Home has an else branch");
    assert!(
        otherwise.trim_start().starts_with("CatalogScreen {}"),
        "the else branch of Home no longer renders the bike catalog first"
    );
}

#[test]
fn primary_navigation_advertises_only_live_bike_surfaces() {
    for destination in ["Home", "Ride", "Orders", "Cart", "Profile"] {
        assert!(
            BOTTOM_NAV.contains(&format!("Link {{ to: Route::{destination} {{}}")),
            "missing {destination} from primary navigation"
        );
    }
    assert_eq!(BOTTOM_NAV.matches("Link { to: Route::").count(), 5);

    for retired in ["Garden", "Sets", "Accessories", "Tea", "Events"] {
        assert!(
            !BOTTOM_NAV.contains(&format!("Link {{ to: Route::{retired} {{}}")),
            "legacy {retired} route is still advertised"
        );
    }
}

/// The check the tab-bar test could not make: *every* screen a customer can
/// reach must be as clean as the tab bar, not just the one file the retirement
/// was first written into.
#[test]
fn no_live_customer_surface_navigates_to_a_retired_screen() {
    let mut offences = Vec::new();

    for relative in LIVE_SURFACES {
        let code = code_of(&live_source(relative));
        for (index, line) in code.lines().enumerate() {
            for retired in RETIRED_ROUTES {
                // `Route::Sets {}` and `Route::EventDetail { id }` both start
                // here; the trailing brace or space keeps `Tea` from matching
                // a longer name that merely begins with it.
                let needle = format!("Route::{retired}");
                let Some(at) = line.find(&needle) else {
                    continue;
                };
                let tail = line[at + needle.len()..].trim_start();
                if tail.starts_with('{') {
                    offences.push(format!(
                        "{relative}:{} navigates to the retired Route::{retired}",
                        index + 1
                    ));
                }
            }
        }
    }

    assert!(
        offences.is_empty(),
        "live customer surfaces still lead to retired screens:\n  {}",
        offences.join("\n  ")
    );
}

/// Identifiers that only existed to serve the garden. Each was live in
/// `src/ui` before D5; none may come back.
///
/// `T_PROFILE_BONUS_GARDEN` is deliberately absent from this list: the
/// `garden_harvest` and `garden_reward` rows it labelled are *still in the live
/// bonus ledger*, and no receipt was rewritten. Since answer 3 of 2026-09-25 a
/// customer reads them under the generic label, amount and date intact; the key
/// stays declared and no screen prints it (`tests/legacy_view_wiring.rs`).
const RETIRED_GARDEN_IDENTIFIERS: [&str; 10] = [
    "Route::Garden",
    "Target::Garden",
    "PendingGarden",
    "parse_garden_start_param",
    "garden_start_param",
    "garden_deep_link",
    "share_garden",
    "get_user_plants",
    "GrowthStage",
    "/api/garden",
];

/// Every `.rs` file under `src/ui`, which is the half of this crate no test
/// binary compiles.
fn ui_sources() -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        for entry in fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("src/ui must be readable: {e}"))
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                // One separator, whatever the host: UNCOMPILED_TEST_DEBT and
                // the other tables below key on forward slashes, so a
                // backslash here turns a recorded file into an unrecorded
                // one, which reads as "new tests that will never run".
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string()
                    .replace('\\', "/");
                out.push((
                    relative,
                    fs::read_to_string(&path)
                        .expect("readable")
                        .replace("\r\n", "\n"),
                ));
            }
        }
    }
    let root = repo_root();
    let mut out = Vec::new();
    walk(&root.join("src/ui"), &root, &mut out);
    assert!(
        out.len() >= 30,
        "found only {} modules under src/ui — every check built on this walk \
         passes by default over an empty corpus, and this tree has far more \
         than that",
        out.len()
    );
    out
}

/// The garden cannot come back to `src/ui` unnoticed.
///
/// This is the only instrument that reaches these files from `cargo test`.
/// `src/lib.rs` declares `pub mod ui` under `#[cfg(target_arch = "wasm32")]`,
/// so `cargo test` never compiles a line of it: for the whole client half, a
/// deleted screen and a re-added one look identical to the test suite. The one
/// compiler CI points at `src/ui` is the wasm clippy job, and clippy will
/// happily compile a garden that is wired correctly.
///
/// The predecessor of this test asserted that `home_screen.rs` still *cleared*
/// a `PendingGarden` flag — the correct check while a stale `?startapp=garden`
/// link was still parsed, and a check that could only be satisfied by keeping
/// the garden's plumbing alive. Nothing parses that payload now, so the
/// assertion is not merely obsolete; passing it would require reintroducing
/// what it was written to retire.
#[test]
fn no_garden_identifier_survives_anywhere_in_the_client() {
    let sources = ui_sources();
    assert!(
        sources.len() >= 20,
        "only {} files walked under src/ui; a scan that reads nothing passes by default",
        sources.len()
    );

    let mut offences = Vec::new();
    for (relative, source) in &sources {
        let code = code_of(source);
        for (index, line) in code.lines().enumerate() {
            for retired in RETIRED_GARDEN_IDENTIFIERS {
                if line.contains(retired) {
                    offences.push(format!("{relative}:{} uses {retired}", index + 1));
                }
            }
        }
    }

    assert!(
        offences.is_empty(),
        "the garden is back in the client, where no test binary compiles it (D5):\n  {}",
        offences.join("\n  ")
    );
}

/// A garden deep link must land nowhere — and "nowhere" has to mean the parser
/// refuses it, not that some screen quietly forwards it.
#[test]
fn a_stale_garden_deep_link_lands_nowhere() {
    let deeplink = fs::read_to_string(repo_root().join("src/trios/deeplink.rs"))
        .expect("src/trios/deeplink.rs must exist");
    // Production code only. The test module below `#[cfg(test)]` names every
    // retired payload on purpose — that is the regression guard proving each
    // one parses to `None` — and reading it here would make this test fail for
    // the very evidence that it holds.
    let production = deeplink
        .split_once("#[cfg(test)]")
        .map(|(before, _)| before)
        .expect("deeplink.rs has a test module");
    let code = code_of(production);

    for stale in ["garden", "garden__144022504", "garden__144022504__tg"] {
        assert!(
            !code.contains(&format!("\"{stale}\"")),
            "deeplink.rs still names the retired payload {stale:?}"
        );
    }
    assert!(
        code.contains("Target::Referrals"),
        "the referral notification button needs a target the app can parse"
    );
}

/// The two panels the garden hosted are still on screen, and still reading the
/// server for every number they print.
///
/// Both halves matter. The panels moved to the referrals page when the garden
/// was removed (D5) — nothing else renders `referral_events` or
/// `referral_milestones`, so if these eight strings lose their only reader
/// again, a live mechanic goes dark and the tests stay green, because
/// `cargo test` compiles no part of `src/ui`.
///
/// The second half is the money. The garden's milestone panel printed its own
/// `1 => 100, 3 => 300, 5 => 500` — a fourth copy of a ladder that already
/// existed three times on the server. A client that knows the reward renders
/// without a network call, which is exactly what makes re-adding one so easy;
/// it is right until somebody edits `loyalty_config`, and silently wrong
/// afterwards, for everybody, for ever.
#[test]
fn the_referral_panels_print_the_servers_numbers_and_none_of_their_own() {
    let code = code_of(&live_source("src/ui/pages/referrals.rs"));

    for key in [
        "T_REFERRAL_INVITEES_TITLE",
        "T_REFERRAL_INVITEES_EMPTY",
        "T_REFERRAL_INVITEE_JOINED",
        "T_REFERRAL_INVITEE_ORDERED",
        "T_REFERRAL_INVITEE_UNKNOWN",
        "T_REFERRAL_MILESTONE_TITLE",
        "T_REFERRAL_MILESTONE_SUBTITLE",
        "T_REFERRAL_MILESTONE_AWARDED",
    ] {
        assert!(
            code.contains(key),
            "{key} has no reader left; the panel it belongs to is off the screen"
        );
    }

    // The two arrays the endpoint publishes: what each reached rung actually
    // paid, and what an unreached one pays today.
    for field in ["awards", "bonuses"] {
        assert!(
            code.contains(field),
            "the milestone panel stopped reading `{field}` off the wire"
        );
    }

    let mut invented = Vec::new();
    for (index, line) in code.lines().enumerate() {
        for amount in ["100", "300", "500"] {
            // `=> 100` is the match-arm shape the deleted screen used;
            // `100.0` is the same table written as an array of pairs.
            if line.contains(&format!("=> {amount}")) || line.contains(&format!("{amount}.0")) {
                invented.push(format!("referrals.rs:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        invented.is_empty(),
        "the referral screen is deciding what a milestone pays:\n  {}",
        invented.join("\n  ")
    );
}

/// The milestone line takes its currency from the market formatter, not from a
/// translation file.
///
/// Both translations carried `+{1}฿` while the string had no reader. The
/// market profile puts the symbol in front (`THB_MARKET.symbol_suffix` is
/// `false`), so the panel would have printed a suffixed glyph that no other
/// price on the screen uses — and once `format_baht` supplies its own, a
/// well-meant restoration of the old wording renders `฿500฿`.
// `the_milestone_line_does_not_carry_its_own_currency_symbol` stood here and
// checked one key. `no_translation_carries_a_currency_symbol`, below, checks
// every arm in the file including that one, so keeping both would be a rule
// written twice — and the narrow copy was the one that would go on passing
// while the other 400 arms drifted.

#[test]
fn the_receipt_promises_no_cannabis_rewards() {
    let success = live_source("src/ui/screens/success_screen.rs");
    assert!(
        !success.contains("T_SUCCESS_REWARDS_GARDEN"),
        "the order receipt still promises a garden seed"
    );
    assert!(
        !I18N.contains("success.rewards.garden"),
        "the garden-seed reward string outlived the screen that showed it"
    );
}

#[test]
fn bike_navigation_labels_exist_in_both_customer_locales() {
    for declaration in ["T_NAV_FLEET", "T_NAV_RIDE", "T_NAV_ORDERS"] {
        assert!(I18N.contains(&format!("pub const {declaration}: Key")));
    }
    for russian in ["Байки", "Заезд", "Заказы"] {
        assert!(I18N.contains(russian));
    }
    for english in ["Bikes", "Ride", "Orders"] {
        assert!(I18N.contains(english));
    }
}

/// Empty states are the first thing a new renter sees on the cart and orders
/// tabs. Their call to action must name bikes — the wording that shipped named
/// «наборы» long after the sets were unpublished.
#[test]
fn empty_state_calls_to_action_send_the_customer_to_the_bikes() {
    for (relative, key) in [
        ("src/ui/screens/cart_screen.rs", "T_CART_BROWSE_MENU"),
        ("src/ui/screens/orders_screen.rs", "T_ORDERS_BROWSE_BIKES"),
    ] {
        let code = code_of(&live_source(relative));
        assert!(
            code.contains(key),
            "{relative} no longer uses {key} for its empty state"
        );
        assert!(
            code.contains("Route::Menu {}"),
            "{relative} must send an empty-state visitor to the bike catalog"
        );
    }

    for (key, russian, english) in [
        ("T_CART_BROWSE_MENU", "🏍 К байкам", "🏍 Browse bikes"),
        ("T_ORDERS_BROWSE_BIKES", "🏍 К байкам", "🏍 Browse bikes"),
    ] {
        assert!(
            I18N.contains(&format!("{key} => \"{russian}\"")),
            "{key} has no Russian bike wording"
        );
        assert!(
            I18N.contains(&format!("{key} => \"{english}\"")),
            "{key} has no English bike wording"
        );
    }
}

// ──────────────────────────────────────────────────────────────────
// Money wears the market's symbol, and only the market decides where
// ──────────────────────────────────────────────────────────────────
//
// `crate::trios::pricing::format_baht` renders an amount against
// `THB_MARKET`, which puts the symbol in front and groups thousands: `฿3,000`.
// That profile is the market's (D18), so no screen and no translation may
// decide the question for itself.
//
// Before 2026-09-16 many did, and the cost was visible on one screen at one
// moment: the checkout MainButton said `฿3,000` while the bonus row six pixels
// above said `3000 ฿`, because the row's glyph lived in `i18n.rs` where no
// market profile can reach it. The admin screen carried a second `money_thb`
// that had drifted the same way, plus a third convention that put the currency
// in a card *heading* — and `T_PROFILE_EARN_PER_REF` had gone further still and
// baked `฿100` into both translations of a figure whose configured value is
// 200.

/// The double-quoted string literals on one line of Rust.
///
/// Line-level, not literal-level, was the first thing tried and it was useless:
/// `input { …, placeholder: "Депозит ฿ — пусто = прочерк", value: "{deposit}" }`
/// has the glyph and a brace on the same line in two *different* strings, and
/// that field hint is not a formatted amount. The defect is one literal holding
/// both.
fn string_literals(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != '"' {
            i += 1;
            continue;
        }
        i += 1;
        let mut literal = String::new();
        while i < chars.len() && chars[i] != '"' {
            // Step over an escaped character so an escaped quote does not end
            // the literal early.
            if chars[i] == '\\' {
                i += 1;
            }
            if i < chars.len() {
                literal.push(chars[i]);
                i += 1;
            }
        }
        i += 1;
        out.push(literal);
    }
    out
}

/// Whole-line `//` comments, dropped.
///
/// Only whole-line: truncating at the first `//` anywhere would cut every
/// `"https://…"` in half and quietly stop testing the rest of the line.
fn without_comment_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
        .map(|(i, line)| (i + 1, line))
}

/// Ways this market's currency gets written by hand.
const CURRENCY_SPELLINGS: [&str; 2] = ["฿", "Бат"];

#[test]
fn no_translation_carries_a_currency_symbol() {
    let arms: Vec<(usize, &str)> = without_comment_lines(I18N)
        .filter(|(_, line)| line.contains("=> \""))
        .collect();
    assert!(
        arms.len() >= 200,
        "found only {} translation arms in i18n.rs — a scan that reads almost \
         nothing passes by default",
        arms.len()
    );

    let offenders: Vec<String> = arms
        .iter()
        .filter(|(_, line)| {
            string_literals(line)
                .iter()
                .any(|lit| CURRENCY_SPELLINGS.iter().any(|sym| lit.contains(sym)))
        })
        .map(|(number, line)| format!("i18n.rs:{number}: {}", line.trim()))
        .collect();

    assert!(
        offenders.is_empty(),
        "a translation formats money itself, so the market profile cannot \
         place the symbol — pass `format_baht(...)` in as an argument \
         instead:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn no_screen_glues_a_currency_onto_an_interpolated_amount() {
    let mut offenders = Vec::new();

    for (relative, source) in ui_sources() {
        for (number, line) in without_comment_lines(&source) {
            for literal in string_literals(line) {
                let has_currency = CURRENCY_SPELLINGS
                    .iter()
                    .any(|symbol| literal.contains(symbol));
                // `{` is what makes it an amount rather than a field hint:
                // either an `rsx!`/`format!` interpolation or a format spec.
                if has_currency && literal.contains('{') {
                    offenders.push(format!("{relative}:{number}: {}", line.trim()));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "these render an amount and write the currency themselves — route them \
         through `crate::trios::pricing::format_baht`, or `money_thb` where an \
         absent price must stay absent (D9):\n{}",
        offenders.join("\n")
    );
}

// ──────────────────────────────────────────────────────────────────
// An asset path nothing loads is a corpse the compiler cannot see
// ──────────────────────────────────────────────────────────────────
//
// `src/ui/assets.rs` held 39 constants on 2026-09-16 and exactly one of them —
// `logo::MAIN` — was named by any code in the repository. The other 38 were
// the cannabis shop's: twelve strain photographs, three pack renders, fourteen
// `game/N.png` sprites, and `strain_image_url`, a lookup that mapped «Super
// Lemon Haze» onto one of them.
//
// Nothing warned, and nothing could. `dead_code` does not fire on a `pub const`
// inside a `pub mod` — it is reachable from outside the crate by definition —
// and `src/ui` is additionally invisible to `cargo test`, because `src/lib.rs`
// gates the whole module on `target_arch = "wasm32"`. The single compiler that
// ever reads these files is the wasm clippy gate, and it had nothing to say
// either. A source walk is the only instrument left.

/// `mod::NAME` for every `pub const` declared in `src/ui/assets.rs`.
///
/// Qualified, not bare: the identifiers in that file are `MAIN`, `GOLD`,
/// `SILVER`. Searching the tree for `MAIN` would match any constant anywhere
/// and the gate would pass on hits that have nothing to do with assets — the
/// module path is what makes a reference a reference.
fn declared_asset_paths() -> Vec<String> {
    let source = fs::read_to_string(repo_root().join("src/ui/assets.rs"))
        .expect("src/ui/assets.rs must be readable");
    let mut module = String::new();
    let mut out = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("pub mod ") {
            module = rest.trim_end_matches('{').trim().to_string();
        } else if let Some(rest) = trimmed.strip_prefix("pub const ") {
            let name = rest.split(':').next().unwrap_or_default().trim();
            if !name.is_empty() {
                out.push(format!("{module}::{name}"));
            }
        }
    }
    // Seven survived the 2026-09-16 sweep. Six of them were the member-card
    // paths, whose artwork was a cannabis bud; the owner's ruling of
    // 2026-09-25 (#12) removed the files and the constants with them, so one
    // constant is the whole table and the floor follows it down.
    assert!(
        !out.is_empty(),
        "parsed no constant out of src/ui/assets.rs — either the file lost \
         `logo::MAIN`, the one constant the client loads, or this parser \
         stopped recognising the declarations and is now scanning an empty \
         corpus"
    );
    out
}

/// Asset constants deliberately kept while nothing reads them, and why.
///
/// Every entry is a decision somebody made, not a leftover: an unexplained
/// exception here is indistinguishable from the thirty-eight corpses this gate
/// exists to have caught.
///
/// Empty since 2026-09-25. It held the six `member_cards::*` paths, kept "until
/// the owner signs off on vector art" because their artwork was a cannabis
/// bud; the owner's ruling of that day (#12, nothing cannabis-related
/// anywhere) removed the files, and the constants went with them.
const UNREFERENCED_BY_DECISION: [(&str, &str); 0] = [];

#[test]
fn every_asset_path_is_loaded_by_something_or_kept_on_purpose() {
    let sources: Vec<(String, String)> = ui_sources()
        .into_iter()
        .filter(|(relative, _)| relative != "src/ui/assets.rs")
        .collect();

    let mut orphans = Vec::new();
    for path in declared_asset_paths() {
        if UNREFERENCED_BY_DECISION
            .iter()
            .any(|(kept, _)| *kept == path)
        {
            continue;
        }
        if !sources.iter().any(|(_, source)| source.contains(&path)) {
            orphans.push(path);
        }
    }

    assert!(
        orphans.is_empty(),
        "src/ui/assets.rs declares {} path(s) that no screen loads:\n  {}\n\n\
         Delete the constant, or add it to UNREFERENCED_BY_DECISION with the \
         reason it stays. `dead_code` will never tell you: a `pub const` in a \
         `pub mod` is reachable by definition, and src/ui is not compiled by \
         `cargo test` at all.",
        orphans.len(),
        orphans.join("\n  ")
    );
}

#[test]
fn nothing_is_kept_on_purpose_that_something_already_loads() {
    let sources: Vec<(String, String)> = ui_sources()
        .into_iter()
        .filter(|(relative, _)| relative != "src/ui/assets.rs")
        .collect();
    let declared = declared_asset_paths();

    let mut stale = Vec::new();
    for (kept, reason) in UNREFERENCED_BY_DECISION {
        if !declared.iter().any(|path| path == kept) {
            stale.push(format!("{kept} — no longer declared ({reason})"));
        } else if let Some((relative, _)) = sources.iter().find(|(_, s)| s.contains(kept)) {
            stale.push(format!("{kept} — now loaded by {relative} ({reason})"));
        }
    }

    assert!(
        stale.is_empty(),
        "UNREFERENCED_BY_DECISION has entries that are no longer true:\n  {}\n\n\
         An exception list nobody prunes is the same restated-list defect it \
         was written to guard against — it would go on excusing a constant \
         that has since been deleted, or one a screen has started using.",
        stale.join("\n  ")
    );
}

// ──────────────────────────────────────────────────────────────────
// The tests under src/ui do not run, and they look exactly like tests
// ──────────────────────────────────────────────────────────────────
//
// `src/lib.rs` reads:
//
//     #[cfg(target_arch = "wasm32")]
//     pub mod ui;
//
// so `cargo test` — which builds for the host — never compiles `src/ui` at
// all. Every `#[cfg(test)] mod tests` under that directory is inert: it is not
// skipped, not ignored, not reported. It contributes nothing to the 46 green
// targets and nothing to CI, and the only signal that anything is wrong is its
// absence from a count nobody has a reason to check.
//
// `share.rs` says so in a comment of its own, and had already been bitten:
// its 14 test functions assert things about deep-link payload shapes, and a
// drift between `garden__<id>` and `garden_<id>` would have shipped green past
// all of them. The fix there was to move the vocabulary into
// `crate::trios::deeplink`, which IS compiled by `cargo test` — 313 of the
// tests that actually run live under `src/trios`.
//
// This gate does not demand the remaining ones be rescued today. It ratchets:
// the debt is written down per file, and the numbers may only fall. Adding a
// `#[test]` under `src/ui` now fails here, which is the only place it can fail.

/// Test functions under `src/ui` that `cargo test` cannot see, per file.
///
/// Per file, not a single total: a rescue in one file and a new dead test in
/// another would net out to the same number and this gate would wave both
/// through.
///
/// These may only go down. When a file reaches zero, delete its row — the
/// emptiness check below will not let the list quietly become vacuous.
const UNCOMPILED_TEST_DEBT: [(&str, usize); 5] = [
    ("src/ui/share.rs", 14),
    ("src/ui/screens/catalog_screen.rs", 6),
    ("src/ui/api/local_client.rs", 5),
    ("src/ui/screens/bike_detail.rs", 4),
    ("src/ui/lang.rs", 3),
];

#[test]
fn no_file_grows_more_tests_that_cargo_test_cannot_see() {
    let mut offences = Vec::new();

    for (relative, source) in ui_sources() {
        let live = source
            .lines()
            .filter(|line| line.trim() == "#[test]")
            .count();
        let allowed = UNCOMPILED_TEST_DEBT
            .iter()
            .find(|(path, _)| *path == relative)
            .map(|(_, count)| *count)
            .unwrap_or(0);

        if live > allowed {
            offences.push(format!(
                "{relative}: {live} #[test] fn(s), {allowed} on record — \
                 {} new one(s) that will never run",
                live - allowed
            ));
        }
    }

    assert!(
        offences.is_empty(),
        "new tests were added under src/ui, where `cargo test` never compiles \
         them:\n  {}\n\n\
         They will report neither pass nor fail. Put the logic somewhere the \
         host can reach — `src/trios` is compiled — and test it there, or add \
         a source-walking check to tests/ like this one. If you are certain \
         the dead test earns its place, raise the count in \
         UNCOMPILED_TEST_DEBT and say why.",
        offences.join("\n  ")
    );
}

#[test]
fn the_uncompiled_test_debt_is_written_down_accurately() {
    let sources = ui_sources();
    let mut wrong = Vec::new();

    for (relative, recorded) in UNCOMPILED_TEST_DEBT {
        let Some((_, source)) = sources.iter().find(|(path, _)| path == relative) else {
            wrong.push(format!("{relative}: on record, but no such file"));
            continue;
        };
        let live = source
            .lines()
            .filter(|line| line.trim() == "#[test]")
            .count();
        if live < recorded {
            wrong.push(format!(
                "{relative}: {recorded} on record, {live} in the file — \
                 {} were rescued or deleted; lower the number so the ratchet \
                 holds at the new floor",
                recorded - live
            ));
        }
    }

    assert!(
        wrong.is_empty(),
        "UNCOMPILED_TEST_DEBT no longer describes the tree:\n  {}\n\n\
         A ratchet that is not tightened after a win is a ratchet that has \
         given the ground back — the freed slots would silently authorise new \
         dead tests.",
        wrong.join("\n  ")
    );
}
