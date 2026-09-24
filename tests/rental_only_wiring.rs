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
/// every event row that existed. What was left open was the admin write: a new
/// event defaulted to public, and an edit could re-publish a hidden one. Both
/// now store the flag through `storable_public_flag`, which ANDs it with
/// `EVENTS_PUBLISHABLE`, which is false.
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
#[test]
fn no_customer_sentence_names_a_place_other_than_phuket() {
    let i18n = source("src/trios/i18n.rs");
    let mut offences = Vec::new();
    for (index, line) in i18n.lines().enumerate() {
        // Only translation arms: `T_KEY => "..."`.
        let Some(arrow) = line.find("=> \"") else {
            continue;
        };
        let value = &line[arrow..];
        for place in [
            "Samui",
            "Самуи",
            "Phangan",
            "Панган",
            "Pattaya",
            "Паттай",
            "Krabi",
            "Краби",
            "Bangkok",
            "Бангкок",
        ] {
            if value.contains(place) {
                offences.push(format!("src/trios/i18n.rs:{} names {place}", index + 1));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "a customer sentence names a place other than Phuket:\n  {}",
        offences.join("\n  ")
    );
    // D16: the scan must be reading translation arms that name Phuket.
    assert!(i18n.contains("Пхукет") && i18n.contains("Phuket"));
}
