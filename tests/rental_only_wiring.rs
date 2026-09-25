//! Rental only, Phuket only: the owner's decision of 2026-09-24.
//!
//! The owner ruled that a customer may only RENT a bike, and only in Phuket.
//! Bike sales (the buy-out) and events leave every customer surface; the
//! referral programme, loyalty and the ride game stay. Nothing is deleted: a
//! placed order may carry a sale line and a booking may name an event, and
//! both must keep reading. So the retirement is done the way this repository
//! already retires things -- rows kept, surfaces changed, the change named --
//! and not by dropping a table (migration 083's way) or by trusting a data
//! flag to keep a surface dark (legacy_retirement.t27: a data decision never
//! retires a surface).
//!
//! The owner's answer of 2026-09-25 ("7 да") carried the ruling over the
//! previous shop's own client paths: its goods (`/accessories`, `/tea`), its
//! game (`/game`) and its hunts now open the catalog (the tech tree too, on
//! this repository's reading of the same answer), the ride game, referrals and
//! loyalty stay, and the quest a customer opens from the profile stays without
//! its minimum-purchase line.
//!
//! `src/ui` is compiled only for wasm32, so no host test binary ever sees a
//! screen. The checks below read the sources as text, the instrument
//! `customer_surface_wiring.rs` already uses for the same reason. The server
//! halves are also covered by unit tests beside the code they pin
//! (`bike_sale_lines_are_retired` in `src/api/orders.rs`,
//! `a_new_event_is_never_stored_public_while_events_are_retired` in
//! `src/api/events.rs`, and the public-view tests in `src/api/bikes.rs` and
//! `src/db/bikes.rs`); these text checks are what ties the call sites to them.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// A shipped file, normalised to LF. A missing file fails rather than passes:
/// a renamed screen must break these checks, not silently empty them.
fn source(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{relative} must exist: {e}"))
        .replace("\r\n", "\n")
}

/// Strip `//` line comments. The retirement notes name what was removed on
/// purpose, and a note is not live code.
fn code_of(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The text from `start` up to the first `end` after it. Both must exist.
fn span<'a>(text: &'a str, start: &str, end: &str, what: &str) -> &'a str {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("{what}: `{start}` not found"));
    let to = text[from + start.len()..]
        .find(end)
        .map(|offset| from + start.len() + offset)
        .unwrap_or_else(|| panic!("{what}: nothing closes `{start}` with `{end}`"));
    &text[from..to]
}

fn rust_files_under(dir: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![repo_root().join(dir)];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).unwrap_or_else(|e| panic!("read_dir {d:?}: {e}")) {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

// -- The first line every customer reads -------------------------------------

/// `/` renders the catalog, and the catalog's subtitle is the first sentence
/// of the app. It offered a buy-out. The new wording is the old one with the
/// buying half deleted, in both locales, so no sentence was invented.
#[test]
fn the_catalog_subtitle_offers_rental_only_and_still_names_phuket() {
    let i18n = source("src/trios/i18n.rs");
    let arms: Vec<&str> = i18n
        .lines()
        .filter(|line| line.trim_start().starts_with("T_BIKE_CATALOG_DESC =>"))
        .collect();
    assert_eq!(
        arms.len(),
        2,
        "T_BIKE_CATALOG_DESC must keep exactly its Russian and English arms"
    );
    for arm in &arms {
        let lower = arm.to_lowercase();
        for word in ["выкуп", "продаж", "купит", "buy", "sale", "sell"] {
            assert!(
                !lower.contains(word),
                "the catalog subtitle still offers a sale ({word}): {arm}"
            );
        }
    }
    // File order is Russian first, English second (the parity test relies on
    // the same two match blocks).
    assert!(
        arms[0].contains("Аренда") && arms[0].contains("Пхукет"),
        "the Russian subtitle no longer says rental in Phuket: {}",
        arms[0]
    );
    assert!(
        arms[1].contains("Rent") && arms[1].contains("Phuket"),
        "the English subtitle no longer says rental in Phuket: {}",
        arms[1]
    );
}

// -- The buy-out block ---------------------------------------------------------

/// The detail overlay rendered a «Выкуп» block whenever the family carried the
/// admin's sale flag or a sale price. A data gate is not a retirement: one tick
/// of the admin checkbox put the block back in front of every customer. The
/// block is gone from the render; the wire fields stay parsed and the keys stay
/// translated, as `colors_available` and `T_BIKE_FILTER_FREE_NOW` did.
#[test]
fn the_bike_detail_screen_renders_no_buy_out_block() {
    let detail = code_of(&source("src/ui/screens/bike_detail.rs"));
    for token in [
        "T_BIKE_SALE_TITLE",
        "T_BIKE_SALE_PRICE",
        "sale_price_thb",
        "for_sale",
    ] {
        assert!(
            !detail.contains(token),
            "src/ui/screens/bike_detail.rs still reads `{token}`"
        );
    }
    // D16: the scan must still be looking at the live screen.
    assert!(
        detail.contains("T_BIKE_BOOK"),
        "bike_detail.rs no longer renders its Book control; this check is blind"
    );

    // No other customer screen may pick the block up. The admin screen is the
    // shop's own record and keeps its sale fields (it reaches no customer).
    let mut offences = Vec::new();
    for path in rust_files_under("src/ui") {
        let relative = path
            .strip_prefix(repo_root())
            .expect("under the repo")
            .to_string_lossy()
            .replace('\\', "/");
        if relative == "src/ui/screens/admin_screen.rs" {
            continue;
        }
        let code = code_of(
            &fs::read_to_string(&path)
                .expect("readable")
                .replace("\r\n", "\n"),
        );
        for key in ["T_BIKE_SALE_TITLE", "T_BIKE_SALE_PRICE"] {
            if code.contains(key) {
                offences.push(format!("{relative} renders {key}"));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "a customer screen renders the retired buy-out block:\n  {}",
        offences.join("\n  ")
    );
}

// -- The events screens ----------------------------------------------------------

/// `/events`, `/events/:id` and `/my-bookings` stay DECLARED, because a link
/// already sent never expires, and each lands on the catalog, exactly as
/// `/sets` and `/sommelier` do. No handler mounts an events screen any more.
#[test]
fn the_event_paths_survive_as_catalog_aliases() {
    let routes = source("src/ui/routes.rs");
    for path in ["/events", "/events/:id", "/my-bookings"] {
        assert!(
            routes.contains(&format!("#[route(\"{path}\")]")),
            "{path} is no longer declared: an old link would become a router miss"
        );
    }
    for handler in ["fn Events(", "fn EventDetail(", "fn MyBookings("] {
        let body = code_of(span(&routes, handler, "#[component]", handler));
        assert!(
            body.contains("CatalogScreen {}"),
            "{handler} no longer lands on the catalog:\n{body}"
        );
    }
    let code = code_of(&routes);
    for screen in ["EventsScreen", "EventDetailScreen", "MyBookingsScreen"] {
        assert!(
            !code.contains(screen),
            "src/ui/routes.rs still reaches the retired {screen}"
        );
    }
}

// -- The previous shop's paths (owner, 2026-09-25) ---------------------------------

/// Every client path a retirement kept for the links already sent, with the
/// handler that answers it. The first five are the precedent: `/sets` and
/// `/sommelier` since their screens were deleted, the three events paths since
/// 2026-09-24. The other seven joined on the owner's answer of 2026-09-25 ("7
/// да"): the previous shop's goods, its game and its hunts open the catalog,
/// and so does the tech tree, which the answer does not name and which
/// `legacy_retirement.t27` counts as an old non-rental address on its own
/// reading. `/skate` is kept as well but renders the ride, which
/// `tests/ride_asset_wiring.rs` pins.
const CATALOG_ALIASES: [(&str, &str); 12] = [
    ("/sets", "fn Sets("),
    ("/sommelier", "fn Sommelier("),
    ("/events", "fn Events("),
    ("/events/:id", "fn EventDetail("),
    ("/my-bookings", "fn MyBookings("),
    ("/accessories", "fn Accessories("),
    ("/tea", "fn Tea("),
    ("/game", "fn Game("),
    ("/treasure-hunt", "fn TreasureHunt("),
    ("/ar-hunt", "fn ArHunt("),
    ("/location-quest", "fn LocationQuest("),
    ("/tech-tree", "fn TechTree("),
];

/// The screens those seven paths mounted until 2026-09-25. Nothing is deleted:
/// each stays compiled and exported, and no handler mounts it.
const UNMOUNTED_ON_2026_09_25: [&str; 7] = [
    "AccessoriesScreen",
    "TeaScreen",
    "GameScreen",
    "TreasureHuntScreen",
    "ARHuntScreen",
    "LocationQuestScreen",
    "TechTreeScreen",
];

/// A handler body (comments stripped) that renders the catalog and nothing
/// else: no second screen, no loading shell, no branch. Home renders the
/// catalog too, but behind a branch and beside `HomeScreen`, so it is not one.
fn renders_only_the_catalog(body: &str) -> bool {
    body.contains("CatalogScreen {}")
        && !body.replace("CatalogScreen {}", "").contains("Screen")
        && !body.contains("if ")
}

/// Every kept path stays DECLARED, because a link already sent never expires,
/// and its handler lands on the catalog. Read from both ends: each listed path
/// resolves to a catalog-only handler, and every catalog-only handler in the
/// router is a listed one, so a thirteenth repoint cannot land unrecorded.
#[test]
fn every_retired_path_survives_as_a_catalog_alias() {
    let routes = source("src/ui/routes.rs");
    let mut listed = Vec::new();
    for (path, handler) in CATALOG_ALIASES {
        assert!(
            routes.contains(&format!("#[route(\"{path}\")]")),
            "{path} is no longer declared: an old link would become a router miss"
        );
        let body = code_of(span(&routes, handler, "#[component]", handler));
        assert!(
            renders_only_the_catalog(&body),
            "{path} no longer lands on the catalog alone ({handler}):\n{body}"
        );
        listed.push(handler.trim_start_matches("fn ").trim_end_matches('('));
    }

    let code = code_of(&routes);
    let mut found = Vec::new();
    for chunk in code.split("#[component]").skip(1) {
        let Some(name) = chunk
            .trim_start()
            .strip_prefix("fn ")
            .and_then(|rest| rest.split('(').next())
        else {
            continue;
        };
        if renders_only_the_catalog(chunk) {
            found.push(name);
        }
    }
    listed.sort_unstable();
    found.sort_unstable();
    assert_eq!(
        found, listed,
        "the handlers that render only the catalog are not the listed aliases"
    );
}

/// How many times `code` mounts `screen`. `TeaScreen {` is a mount;
/// `TeaScreen() -> Element {` is the definition and `TeaScreen;` the export.
fn mounts_of(code: &str, screen: &str) -> usize {
    let mut count = 0;
    let mut rest = code;
    while let Some(at) = rest.find(screen) {
        rest = &rest[at + screen.len()..];
        if rest.trim_start().starts_with('{') {
            count += 1;
        }
    }
    count
}

/// Every `.rs` file under `src/ui`, as its repo-relative path and its code with
/// comments stripped. Read through `source`, so an unreadable file fails.
fn ui_code() -> Vec<(String, String)> {
    rust_files_under("src/ui")
        .iter()
        .map(|path| {
            let relative = path
                .strip_prefix(repo_root())
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let code = code_of(&source(&relative));
            (relative, code)
        })
        .collect()
}

/// The seven screens are unmounted, not deleted: `src/ui/screens/mod.rs` still
/// exports each, and no file under `src/ui` mounts one.
#[test]
fn the_previous_shops_screens_stay_compiled_and_unmounted() {
    let exports = source("src/ui/screens/mod.rs");
    for screen in UNMOUNTED_ON_2026_09_25 {
        assert!(
            exports
                .lines()
                .any(|line| line.starts_with("pub use ") && line.contains(screen)),
            "src/ui/screens/mod.rs no longer exports {screen}: this retirement keeps the screen"
        );
    }
    let mut mounts = Vec::new();
    let mut ride_mounts = 0;
    for (relative, code) in ui_code() {
        for screen in UNMOUNTED_ON_2026_09_25 {
            if mounts_of(&code, screen) > 0 {
                mounts.push(format!("{relative} mounts {screen}"));
            }
        }
        ride_mounts += mounts_of(&code, "RideScreen");
    }
    assert!(
        mounts.is_empty(),
        "a screen the owner retired on 2026-09-25 is mounted again:\n  {}",
        mounts.join("\n  ")
    );
    // D16: the same walk and the same instrument see mounts that are there
    // (`/ride` and `/skate` both mount the ride).
    assert!(
        ride_mounts >= 2,
        "the scan saw {ride_mounts} mounts of RideScreen"
    );
}

/// No screen a customer can open sends them to a retired path. A navigation is
/// a `Link { to: Route::X` or a `push`/`replace` of one; the unmounted screens
/// themselves are left out, because code nobody can open navigates nowhere
/// (the events screens still link to each other).
#[test]
fn no_mounted_screen_navigates_to_a_retired_path() {
    const UNMOUNTED_FILES: [&str; 9] = [
        "src/ui/screens/events_screen.rs",
        "src/ui/screens/accessories_screen.rs",
        "src/ui/screens/tea_screen.rs",
        "src/ui/screens/game_screen.rs",
        "src/ui/game/shop_game.rs",
        "src/ui/screens/treasure_hunt_screen.rs",
        "src/ui/screens/ar_hunt_screen.rs",
        "src/ui/screens/location_quest_screen.rs",
        "src/ui/screens/tech_tree_screen.rs",
    ];
    for file in UNMOUNTED_FILES {
        // A renamed file must break this list, not silently widen the scan.
        source(file);
    }
    let variants: Vec<&str> = CATALOG_ALIASES
        .iter()
        .map(|(_, handler)| handler.trim_start_matches("fn ").trim_end_matches('('))
        .collect();
    /// Where `line` opens `Route::{variant}` (a navigation, not a match arm).
    fn opens(line: &str, variant: &str) -> bool {
        ["to: Route::", "push(Route::", "replace(Route::"]
            .iter()
            .any(|lead| {
                let needle = format!("{lead}{variant}");
                line.find(&needle)
                    .is_some_and(|at| line[at + needle.len()..].trim_start().starts_with('{'))
            })
    }
    let mut offences = Vec::new();
    let mut live_navigations = 0;
    for (relative, code) in ui_code() {
        if UNMOUNTED_FILES.contains(&relative.as_str()) || relative == "src/ui/routes.rs" {
            continue;
        }
        for (index, line) in code.lines().enumerate() {
            for variant in &variants {
                if opens(line, variant) {
                    offences.push(format!("{relative}:{} opens Route::{variant}", index + 1));
                }
            }
            if opens(line, "Orders") {
                live_navigations += 1;
            }
        }
    }
    assert!(
        offences.is_empty(),
        "a mounted screen still opens a retired path:\n  {}",
        offences.join("\n  ")
    );
    // D16: the same walk and the same matcher see a live navigation (the
    // profile, among others, opens the orders).
    assert!(
        live_navigations > 0,
        "the scan saw no navigation to Route::Orders"
    );
}

/// The owner kept the quest a customer opens from the profile and had its
/// minimum-purchase line, the previous shop's rule, removed (2026-09-25). The
/// key stays translated, as the buy-out keys do; no screen renders it, nor the
/// two purchase keys defined beside it.
#[test]
fn the_quest_screen_states_no_purchase_rule_and_stays_reachable() {
    let mut offences = Vec::new();
    let mut quest_scanned = false;
    for (relative, code) in ui_code() {
        for key in ["T_PURCHASE_MIN", "T_PURCHASE_REQUIRED", "T_ERROR_PURCHASE"] {
            if code.contains(key) {
                offences.push(format!("{relative} renders {key}"));
            }
        }
        // D16: the walk reaches the live quest screen, and reads its code.
        if relative == "src/ui/game/quest.rs" && code.contains("T_SCAN_QR") {
            quest_scanned = true;
        }
    }
    assert!(
        offences.is_empty(),
        "a screen states the previous shop's purchase rule:\n  {}",
        offences.join("\n  ")
    );
    assert!(
        quest_scanned,
        "the scan never read src/ui/game/quest.rs rendering its scan control"
    );

    // What stays: the profile row, the route and the screen behind it.
    let profile = code_of(&source("src/ui/screens/profile_screen.rs"));
    assert!(
        profile.contains("Link { to: Route::Quest { id: \"daily\".to_string() },"),
        "Profile no longer opens the quest the owner kept"
    );
    let routes = source("src/ui/routes.rs");
    assert!(routes.contains("#[route(\"/quest/:id\")]"));
    let quest_handler = code_of(span(&routes, "fn Quest(", "#[component]", "fn Quest("));
    assert!(
        quest_handler.contains("QuestScreen { id }"),
        "/quest/:id no longer mounts the quest screen:\n{quest_handler}"
    );
}

/// What the owner kept (2026-09-24, again 2026-09-25): the ride game, the
/// referral programme and the profile that carries loyalty. Each path still
/// mounts its own screen, so no alias above swallowed a kept surface.
#[test]
fn the_kept_surfaces_still_mount_their_screens() {
    let routes = source("src/ui/routes.rs");
    for (path, handler, screen) in [
        ("/ride", "fn Ride(", "RideScreen {}"),
        ("/referrals", "fn Referrals(", "ReferralsScreen {}"),
        ("/profile", "fn Profile(", "ProfileScreen {}"),
    ] {
        assert!(
            routes.contains(&format!("#[route(\"{path}\")]")),
            "{path} is no longer declared"
        );
        let body = code_of(span(&routes, handler, "#[component]", handler));
        assert!(
            body.contains(screen) && !body.contains("CatalogScreen"),
            "{path} no longer mounts {screen}:\n{body}"
        );
    }
}

// -- The public catalog wire ------------------------------------------------------

/// Removing the render is the Mini App half. The public API still served the
/// stored sale flag and asking price to anyone; now both public reads serve the
/// family with its sale offer withheld (both keys present, `false` and `null`,
/// so no money key is ever omitted), and the admin list keeps the stored values.
#[test]
fn the_public_catalog_withholds_the_sale_offer_and_the_admin_list_does_not() {
    let bikes = code_of(&source("src/api/bikes.rs"));
    let list = span(&bikes, "async fn list_bikes(", "\nasync fn ", "list_bikes");
    let get = span(&bikes, "async fn get_bike(", "\nasync fn ", "get_bike");
    let admin = span(
        &bikes,
        "async fn admin_list_bikes(",
        "\nasync fn ",
        "admin_list_bikes",
    );
    assert!(
        list.contains(".with_sale_withheld()"),
        "GET /api/bikes serves the stored sale offer again"
    );
    assert!(
        get.contains(".with_sale_withheld()"),
        "GET /api/bikes/:key serves the stored sale offer again"
    );
    assert!(
        !admin.contains("with_sale_withheld"),
        "the admin list must keep the shop's stored sale fields"
    );
}

// -- The order path -----------------------------------------------------------------

/// A sale line was admitted on POST /api/orders whenever its price was sane or
/// absent. A new one is now refused with a status alone (no new sentence); a
/// malformed one is still malformed first. Placed sale orders keep reading,
/// because the `BikeSale` variant stays in the serde vocabulary.
#[test]
fn a_new_order_refuses_a_sale_line() {
    let orders = code_of(&source("src/api/orders.rs"));
    let validate = span(
        &orders,
        "fn validate_bike_lines(",
        "\n}\n",
        "validate_bike_lines",
    );
    let sale_arm = &validate[validate
        .rfind("BikeDeal::BikeSale")
        .expect("validate_bike_lines still names the sale deal")..];
    assert!(
        sale_arm.contains("UNPROCESSABLE_ENTITY"),
        "validate_bike_lines admits a new sale line again:\n{sale_arm}"
    );
    let db_orders = source("src/db/orders.rs");
    assert!(
        db_orders.contains("BikeSale"),
        "the BikeSale variant must stay: placed sale orders still deserialize through it"
    );
}

// -- The events API -------------------------------------------------------------------

/// Every customer read of an event filters `is_public`, and migration 085 hid
/// every event row that existed then (088, below, hides any made public since).
/// What was left open was the admin write: a new event defaulted to public, and
/// an edit could re-publish a hidden one. Both now store the flag through
/// `storable_public_flag`, which ANDs it with `EVENTS_PUBLISHABLE`, which is false.
#[test]
fn an_event_can_no_longer_be_published_to_customers() {
    let events = code_of(&source("src/api/events.rs"));
    assert!(
        events.contains("const EVENTS_PUBLISHABLE: bool = false;"),
        "src/api/events.rs no longer declares events unpublishable"
    );
    let gate = span(
        &events,
        "fn storable_public_flag(",
        "\n}\n",
        "storable_public_flag",
    );
    assert!(
        gate.contains("&& EVENTS_PUBLISHABLE"),
        "storable_public_flag no longer consults EVENTS_PUBLISHABLE:\n{gate}"
    );
    let create = span(
        &events,
        "fn validate_event_request(",
        "\n}\n",
        "validate_event_request",
    );
    assert!(
        create.contains("storable_public_flag("),
        "a new event can be stored public again:\n{create}"
    );
    let update = span(
        &events,
        "fn validate_update_request(",
        "\n}\n",
        "validate_update_request",
    );
    assert!(
        update.contains("map(storable_public_flag)"),
        "an edit can re-publish a hidden event again:\n{update}"
    );
}

/// The write gate cannot reach a row that is already public: one made public
/// between migration 085 and the ruling. Migration 088 hides it in 085's shape
/// -- one visibility UPDATE, no row deleted, no table dropped -- and is
/// registered, so the runner applies it on the next boot.
#[test]
fn every_event_row_still_public_is_hidden_by_088() {
    let sql = source("migrations/088_unpublish_events.sql");
    let statements: Vec<&str> = sql
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("--"))
        .collect();
    assert_eq!(
        statements,
        ["UPDATE events SET is_public = FALSE WHERE is_public = TRUE;"],
        "migration 088 is no longer the one visibility UPDATE"
    );
    let db = source("src/db/mod.rs");
    assert!(
        db.contains("include_str!(\"../../migrations/088_unpublish_events.sql\")"),
        "migration 088 is not in MIGRATIONS, so no database ever runs it"
    );
}

/// The share card read an event by id with no `is_public` filter, so a hidden
/// event could still be rendered into a Telegram card. It now answers as the
/// retired strain kind does: parseable, and not found.
#[test]
fn a_retired_event_is_never_rendered_into_a_share_card() {
    let share = code_of(&source("src/api/share.rs"));
    assert!(
        share.contains("ShareKind::Event => return Ok(None)"),
        "POST /api/share/prepare builds an event card again"
    );
    assert!(
        share.contains("ShareKind::Strain => return Ok(None)"),
        "the precedent this follows has moved; re-read it"
    );
}

// -- What the product says it is ------------------------------------------------------

/// The served OpenAPI document and the README described a rental AND SALES app.
/// Both lose the sales half, word for word, and nothing else.
#[test]
fn the_published_self_descriptions_say_rental_only() {
    let openapi = source("src/api/openapi.rs");
    let described = openapi
        .lines()
        .find(|line| line.contains("description = \"REST API for TurboBaby Bot"))
        .expect("the OpenAPI description line");
    assert!(
        described.contains("motorbike rental Telegram Mini App"),
        "{described}"
    );
    assert!(!described.to_lowercase().contains("sale"), "{described}");

    let readme = source("README.md");
    assert!(
        readme.contains("motorbike **rental** in Kamala, Phuket"),
        "README.md no longer describes the rental-only product"
    );
    assert!(!readme.contains("rental and sales"));
}

// -- Phuket only ----------------------------------------------------------------------

/// Every translated sentence a customer can be shown names Phuket or no place.
/// Green on the tree it was written for (measured 2026-09-24: no i18n value
/// names another island or city); it exists so that stays true.
///
/// Widened under review the same day: the place list gained the standard
/// Russian spelling of Koh Phangan and the four village names of the zones
/// migration 087 deactivated, in both languages, and the scan now also reads
/// the bot's own copy (`src/locales.rs`) and every string a screen writes
/// itself (`src/ui`), comments excepted. Still green on this tree.
#[test]
fn no_customer_sentence_names_a_place_other_than_phuket() {
    const OTHER_PLACES: [&str; 19] = [
        "Samui",
        "Самуи",
        "Phangan",
        "Панган",
        "Пханган",
        "Pattaya",
        "Паттай",
        "Krabi",
        "Краби",
        "Bangkok",
        "Бангкок",
        // The four villages of the zones migration 087 deactivated.
        "Thong Sala",
        "Тонгсала",
        "Haad Rin",
        "Хаад Рин",
        "Srithanu",
        "Сритхану",
        "Bottle Beach",
        "Боттл Бич",
    ];
    let mut offences = Vec::new();
    let mut scan = |relative: &str, line_number: usize, text: &str| {
        for place in OTHER_PLACES {
            if text.contains(place) {
                offences.push(format!("{relative}:{line_number} names {place}"));
            }
        }
    };

    let i18n = source("src/trios/i18n.rs");
    for (index, line) in i18n.lines().enumerate() {
        // Only translation arms: `T_KEY => "..."`.
        let Some(arrow) = line.find("=> \"") else {
            continue;
        };
        scan("src/trios/i18n.rs", index + 1, &line[arrow..]);
    }

    // The bot's replies: every string literal in the locale table.
    let locales = source("src/locales.rs");
    for (index, line) in code_of(&locales).lines().enumerate() {
        if line.contains('"') {
            scan("src/locales.rs", index + 1, line);
        }
    }

    // Strings a screen writes itself, outside the translation table.
    let screens = rust_files_under("src/ui");
    for path in &screens {
        let relative = path
            .strip_prefix(repo_root())
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let text = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{relative} must read: {e}"))
            .replace("\r\n", "\n");
        for (index, line) in code_of(&text).lines().enumerate() {
            if line.contains('"') {
                scan(&relative, index + 1, line);
            }
        }
    }

    assert!(
        offences.is_empty(),
        "a customer sentence names a place other than Phuket:\n  {}",
        offences.join("\n  ")
    );
    // D16: each scan must be reading text that names Phuket, or it read nothing.
    assert!(i18n.contains("Пхукет") && i18n.contains("Phuket"));
    assert!(locales.contains("Пхукет") && locales.contains("Phuket"));
    assert!(
        screens.len() > 20,
        "the screen scan found {} files under src/ui",
        screens.len()
    );
}

/// A zone id saved before migration 087 can name a Koh Phangan row the server
/// no longer serves; `POST /api/orders` refuses it with 422. The checkout sends
/// the saved id only while the picker shows that same zone
/// (`crate::trios::store::served_zone_id`, unit-tested beside it), never the
/// raw signal.
#[test]
fn the_checkout_never_submits_a_saved_zone_the_served_list_lacks() {
    let checkout = code_of(&source("src/ui/screens/checkout_screen.rs"));
    assert!(
        !checkout.contains("\"delivery_zone_id\": delivery_zone_id(),"),
        "the checkout submits the raw saved zone id again"
    );
    assert!(
        checkout.contains(
            "\"delivery_zone_id\": zone_to_submit(delivery_zone_id(), zone_info.as_ref()),"
        ),
        "the order body no longer carries the zone the picker shows"
    );
    let helper = span(&checkout, "fn zone_to_submit(", "\n}", "zone_to_submit");
    assert!(
        helper.contains("served_zone_id(stored, shown.map(|z| z.id.as_str()))"),
        "zone_to_submit no longer asks served_zone_id:\n{helper}"
    );
}
