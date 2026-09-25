//! A cart kept from the previous shop shows none of it (2026-09-26).
//!
//! The owner ruled rental only on 2026-09-24 and, on 2026-09-25 (answer 12),
//! that nothing of the previous shop may appear anywhere; no row is deleted.
//! A customer who kept a cart from that shop still has its lines in three
//! places, and each one used to hand them back with their names and pictures:
//!
//! * the server's `cart_items` rows, which `GET /api/cart` (and the answers of
//!   the add and merge endpoints) returned;
//! * the abandoned-cart reminder, which named them in a Telegram message;
//! * the Mini App's own saved cart on the device (localStorage `wwb_cart` and
//!   Telegram CloudStorage `wwb_cart_cloud`), read on start-up.
//!
//! One predicate decides for all three: `trios::pricing::cart_kind_is_served`,
//! over `SERVED_CART_KINDS` -- the tag of the one deal the checkout admits,
//! `bike_rental` (`BikeDeal::BikeRental`; `src/db/orders.rs` holds the list to
//! the enum). The contract is `specs/turbobaby/cart_persistence.t27`.
//!
//! `src/ui` is compiled only for wasm32, so the Mini App half is read as text
//! here, the pattern `tests/cart_kind_wiring.rs` uses; the server half is
//! unit-tested in `src/api/cart.rs` and `src/cart_abandonment.rs`, and this
//! file checks that those units are what the handlers and the queries use.

// A panic is how a test reports failure.
#![allow(clippy::panic, clippy::expect_used)]

use std::fs;
use std::path::PathBuf;

use turbobaby_bot::trios::pricing::{cart_kind_is_served, SERVED_CART_KINDS};

/// A tracked file as text, CRLF normalised. A missing file fails the test.
fn source(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{relative} must exist for this guard to mean anything: {e}"))
        .replace("\r\n", "\n")
}

/// The production part of a Rust file: everything before its first
/// `#[cfg(test)]`, so a fixture in a test module is never read as wiring.
fn production(text: &str) -> &str {
    &text[..text.find("#[cfg(test)]").unwrap_or(text.len())]
}

/// A line with its `//` comment removed, so prose describing the rule is never
/// mistaken for the rule. Naive by intent: no line read here puts `//` inside
/// a string literal.
fn code_of(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// The code lines of a file's production part, comments stripped.
fn code_lines(text: &str) -> Vec<String> {
    production(text)
        .lines()
        .map(|l| code_of(l).to_string())
        .collect()
}

/// The body of `fn NAME`, from its signature to the first line that closes at
/// the signature's own indentation. `rustfmt` runs on this tree.
fn fn_body(text: &str, signature: &str, label: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.contains(signature))
        .unwrap_or_else(|| panic!("{label}: `{signature}` is gone"));
    let indent = lines[start].len() - lines[start].trim_start().len();
    let closer = format!("{}}}", " ".repeat(indent));
    let end = lines[start..]
        .iter()
        .position(|l| *l == closer)
        .unwrap_or_else(|| panic!("{label}: `{signature}` never closes"));
    lines[start..=start + end]
        .iter()
        .map(|l| code_of(l))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The kinds migration 081's CHECK lets `cart_items` store, read out of the
/// migration itself rather than restated here.
fn storable_kinds() -> Vec<String> {
    let sql = source("migrations/081_cart_rental_lines.sql");
    let at = sql
        .find("CHECK (kind IN (")
        .expect("migration 081 declares the cart kind CHECK");
    let rest = &sql[at + "CHECK (kind IN (".len()..];
    let list = &rest[..rest.find(')').expect("the CHECK list closes")];
    list.split(',')
        .map(|k| k.trim().trim_matches('\'').to_string())
        .collect()
}

#[test]
fn the_cart_serves_the_rental_kind_and_every_other_storable_kind_is_retired() {
    let storable = storable_kinds();
    assert_eq!(
        storable,
        ["strain", "set", "accessory", "tea", "bike_rental"],
        "migration 081's CHECK moved; re-read which stored kinds are retired"
    );
    for served in SERVED_CART_KINDS {
        assert!(
            storable.iter().any(|k| k == served),
            "{served} is served but no cart row can hold it"
        );
    }
    let retired: Vec<&String> = storable
        .iter()
        .filter(|k| !cart_kind_is_served(k))
        .collect();
    assert_eq!(
        retired,
        ["strain", "set", "accessory", "tea"],
        "the four kinds of the previous shop's catalogue are stored and never served"
    );
}

// ── The server's cart API ───────────────────────────────────────────────────

#[test]
fn every_cart_response_is_built_from_the_served_rows_only() {
    let cart = source("src/api/cart.rs");
    let prod = production(&cart);

    // The one response builder iterates the served rows, not the loaded ones.
    let builder = fn_body(prod, "fn cart_model_to_resp(", "src/api/cart.rs");
    assert!(
        builder.contains("= served_rows(items)"),
        "cart_model_to_resp no longer builds its lines from served_rows:\n{builder}"
    );
    assert!(
        !builder.contains("items\n        .iter()") && !builder.contains("items.iter()"),
        "cart_model_to_resp iterates every loaded row again:\n{builder}"
    );
    // The total is summed inside the same pass, so a hidden row adds nothing.
    assert!(builder.contains("total += price * q as f64;"));

    // served_rows filters on served_row, which asks the shared predicate.
    let rows = fn_body(prod, "fn served_rows(", "src/api/cart.rs");
    assert!(rows.contains(".filter(|row| served_row(row))"), "{rows}");
    let row = fn_body(prod, "fn served_row(", "src/api/cart.rs");
    assert!(
        row.contains("crate::trios::pricing::cart_kind_is_served(&row.kind)"),
        "{row}"
    );

    // Every JSON cart the API answers with goes through that builder: get, add
    // and merge. A new response path built another way would bypass it.
    let lines = code_lines(&cart);
    let answers = lines
        .iter()
        .filter(|l| l.contains("Json(cart_model_to_resp("))
        .count();
    assert_eq!(answers, 3, "get_cart, add_cart_item and merge_cart");
    let literals = lines
        .iter()
        .filter(|l| l.trim_start().starts_with("CartResp {"))
        .count();
    assert_eq!(
        literals, 1,
        "one CartResp literal, inside cart_model_to_resp"
    );
    assert!(builder.contains(
        "    CartResp {\n        telegram_id: model.telegram_id,\n        items: item_resp,"
    ));
}

#[test]
fn no_cart_request_reaches_a_hidden_row_by_id_or_by_clearing() {
    let cart = source("src/api/cart.rs");
    let prod = production(&cart);

    // PATCH and DELETE by id: a hidden row answers 404, as a missing id does,
    // before the owner check and before any write.
    for (handler, write) in [
        ("async fn update_cart_item(", "am.update("),
        ("async fn delete_cart_item(", "delete_by_id("),
    ] {
        let body = fn_body(prod, handler, "src/api/cart.rs");
        let filter = body
            .find(".filter(served_row)\n        .ok_or(StatusCode::NOT_FOUND)?;")
            .unwrap_or_else(|| panic!("{handler} serves a hidden row by id:\n{body}"));
        let written = body
            .find(write)
            .unwrap_or_else(|| panic!("{handler} lost its write `{write}`"));
        assert!(filter < written, "{handler} writes before it filters");
    }

    // Clearing a cart deletes its served lines and leaves a hidden row stored.
    let clear = fn_body(prod, "async fn clear_cart(", "src/api/cart.rs");
    assert!(
        clear.contains("delete_many()\n            .filter(served_lines_of(c.id))"),
        "clear_cart deletes lines the API does not serve:\n{clear}"
    );
    let lines_of = fn_body(prod, "fn served_lines_of(", "src/api/cart.rs");
    assert!(
        lines_of.contains(".add(ItemCol::Kind.is_in(crate::trios::pricing::SERVED_CART_KINDS))")
            && lines_of.contains(".add(ItemCol::CartId.eq(cart_id))")
            && lines_of.contains("Condition::all()"),
        "{lines_of}"
    );

    // No other statement in the API deletes cart lines.
    let deletes = code_lines(&cart)
        .iter()
        .filter(|l| l.contains("delete_many()") || l.contains("delete_by_id("))
        .count();
    assert_eq!(
        deletes, 2,
        "clear_cart and delete_cart_item, both behind the filter"
    );
}

// ── The abandoned-cart reminder ─────────────────────────────────────────────

#[test]
fn the_reminder_reads_only_served_lines_so_it_never_names_a_hidden_one() {
    let reminder = source("src/cart_abandonment.rs");
    let lines = code_lines(&reminder);

    // Every production line that names the table carries the kind filter.
    let reads: Vec<&String> = lines.iter().filter(|l| l.contains("cart_items")).collect();
    assert_eq!(
        reads.len(),
        2,
        "the due-cart join and the line query, and nothing else: {reads:#?}"
    );
    for read in &reads {
        assert!(
            read.contains("kind = ANY($"),
            "a read of cart_items without the served-kind filter: {read}"
        );
    }

    // The join is an INNER join with the filter in its ON: a cart whose every
    // line is hidden has no joined row, so it is never selected -- no message,
    // no SUM of hidden money, and its reminder counter is not spent.
    let join = reads
        .iter()
        .find(|l| l.contains("JOIN cart_items"))
        .expect("the due-cart query joins cart_items");
    assert!(
        join.trim_start()
            .starts_with("JOIN cart_items ci ON ci.cart_id = c.id AND ci.kind = ANY($3) \\"),
        "{join}"
    );
    assert!(!join.contains("LEFT"), "{join}");
    let line_query = reads
        .iter()
        .find(|l| l.contains("SELECT name, quantity, unit_price FROM cart_items"))
        .expect("the line query reads cart_items");
    assert!(
        line_query.contains("WHERE cart_id = $1 AND kind = ANY($2) ORDER BY created_at"),
        "{line_query}"
    );

    // Both placeholders are bound to the shared list: `$2` of the line query
    // directly, `$3` of the due-cart query through due_cart_binds, which is
    // what that query binds. Nothing else binds the list.
    let binds: Vec<&String> = lines
        .iter()
        .filter(|l| l.contains("reminder_kinds().into()"))
        .collect();
    assert_eq!(binds.len(), 2, "{binds:#?}");
    assert!(binds
        .iter()
        .any(|l| l.trim() == "[cart_id.clone().into(), reminder_kinds().into()],"));
    let due = fn_body(
        production(&reminder),
        "fn due_cart_binds(",
        "src/cart_abandonment.rs",
    );
    assert!(
        due.contains("reminder_count.into(),\n        threshold_minutes.into(),\n        reminder_kinds().into(),"),
        "{due}"
    );
    assert_eq!(
        lines
            .iter()
            .filter(|l| l.trim() == "due_cart_binds(reminder_count, threshold_minutes),")
            .count(),
        1,
        "the due-cart query binds its three values through due_cart_binds"
    );
    let kinds = fn_body(
        production(&reminder),
        "fn reminder_kinds(",
        "src/cart_abandonment.rs",
    );
    assert!(
        kinds.contains("crate::trios::pricing::SERVED_CART_KINDS"),
        "{kinds}"
    );

    // The message text is built from the rows those queries return and from
    // no other read: the summary formats `cart.items`, and `items` is filled
    // from the line query alone.
    assert_eq!(
        lines
            .iter()
            .filter(|l| l.contains("format_item_summary(&cart.items)"))
            .count(),
        2,
        "the first reminder and the second nudge"
    );
}

// ── The Mini App's saved cart ───────────────────────────────────────────────

#[test]
fn the_mini_app_drops_retired_lines_whenever_it_loads_a_saved_cart() {
    let app = source("src/ui/app.rs");
    let lines = code_lines(&app);

    // Two reads of a saved cart -- the device's localStorage copy and the
    // Telegram CloudStorage copy -- and each result is filtered before use.
    let parses = lines
        .iter()
        .filter(|l| l.contains("serde_json::from_str::<Cart>("))
        .count();
    assert_eq!(parses, 2, "localStorage and CloudStorage");
    let filtered = lines
        .iter()
        .filter(|l| l.contains("parsed.without_retired_lines()"))
        .count();
    assert_eq!(filtered, parses, "a saved cart is used unfiltered");
    assert!(
        !lines
            .iter()
            .any(|l| l.trim() == "cart = parsed;" || l.contains(".set(parsed)")),
        "a saved cart reaches the signal unfiltered"
    );
    // Both storage keys are what the loads read and what the persist effect
    // writes back, from the (filtered) signal: the device's own copy loses
    // the retired lines on the next save. No other write site exists.
    assert!(app.contains("const CART_STORAGE_KEY: &str = \"wwb_cart\";"));
    assert!(app.contains("const CART_CLOUD_KEY: &str = \"wwb_cart_cloud\";"));
    let persist = app
        .find("let cart_data = cart.read().clone();")
        .expect("the persist effect reads the signal");
    let rest = &app[persist..];
    assert!(rest.contains("storage.set_item(CART_STORAGE_KEY, &json)"));
    assert!(rest.contains("tg.cloud_storage_set(CART_CLOUD_KEY, &json)"));
    assert_eq!(app.matches("set_item(CART_STORAGE_KEY").count(), 1);
    assert_eq!(app.matches("cloud_storage_set(CART_CLOUD_KEY").count(), 1);

    // The filter keeps only served lines, asked of the shared predicate.
    let state = source("src/ui/state.rs");
    let keep = fn_body(&state, "pub fn without_retired_lines(", "src/ui/state.rs");
    assert!(
        keep.contains("self.items.retain(CartItem::is_served);"),
        "{keep}"
    );
    assert!(keep.contains("self.recalculate_total();"), "{keep}");
    let served = fn_body(&state, "pub fn is_served(", "src/ui/state.rs");
    assert!(
        served.contains("crate::trios::pricing::cart_kind_is_served(")
            && served.contains("cart_item_type_to_kind("),
        "{served}"
    );
}

#[test]
fn a_server_cart_line_of_a_retired_kind_never_becomes_a_client_line() {
    // `from_server` is the one reader of a server cart line in the Mini App
    // (tests/cart_kind_wiring.rs holds that), used by the start-up recovery,
    // the cart screen's recovery and the merge response. It asks the shared
    // predicate before it classifies the kind.
    let state = source("src/ui/state.rs");
    let reader = fn_body(&state, "pub fn from_server(", "src/ui/state.rs");
    let gate = reader
        .find("match served_kind(&item.kind)? {")
        .unwrap_or_else(|| {
            panic!("from_server classifies a kind it has not asked about:\n{reader}")
        });
    let first_arm = reader
        .find("=> CartItemType::")
        .expect("from_server classifies");
    assert!(gate < first_arm);
    let kind = fn_body(&state, "fn served_kind(", "src/ui/state.rs");
    assert!(
        kind.contains("crate::trios::pricing::cart_kind_is_served(kind).then_some(kind)"),
        "{kind}"
    );
    for (path, uses) in [
        ("src/ui/app.rs", 1),
        ("src/ui/screens/cart_screen.rs", 1),
        ("src/ui/api/http.rs", 1),
    ] {
        let text = source(path);
        let n = code_lines(&text)
            .iter()
            .filter(|l| l.contains("CartItem::from_server"))
            .count();
        assert_eq!(n, uses, "{path} reads server cart lines another way");
    }
}
