//! A customer's own reads never hand back the previous shop's stored content,
//! and this file holds the live handlers and screens to that.
//!
//! WHY IT EXISTS. Owner, 2026-09-25, answer 3 of the second numbered list put
//! to him that day, verbatim: «Все канабисное аналировать». The operator read
//! it as: analyse all of it and take it out of customers' sight, without
//! deleting or rewriting stored data. The database was forked from the
//! previous shop's bot (DECISIONS.md D19), so a customer's order history,
//! bonus history and server cart can hold what that shop stored: the names of
//! its goods, its shop name on every order, and bonus rows from its garden.
//! The rules live in `src/trios/legacy_view.rs` and are unit-tested there on
//! neutral fixtures, because nothing outside a comment under `src/` may name
//! the retired goods (`tests/legacy_vocabulary_wiring.rs`).
//!
//! So this file does the two things those unit tests cannot:
//!
//! * it runs the rules on the shapes the stored rows actually have -- each kind
//!   of line with its keys and figures, the shop values the old checkout wrote,
//!   a garden row -- and holds that no stored name reaches the customer. The
//!   names and ids in those lines are stand-ins: the rule never reads them, and
//!   the owner ruled on 2026-09-25 that nothing cannabis-related may appear
//!   anywhere. The shop values are the real ones, because they are what the
//!   shop rule has to recognise;
//! * it reads the handlers and screens as text and holds that every customer
//!   read goes through the rules and that the admin reads, which are the
//!   archive, do not. Inline the old direct read back into a handler and every
//!   unit test stays green while the stored names print again.
//!
//! The rule for an order line is recorded in
//! `specs/turbobaby/order_presentation.t27` (`RETIRED_LINE_*`), the answer and
//! the other reads in `specs/turbobaby/legacy_retirement.t27`
//! (`OWNER_ANSWER_3_*`), and the constants there are checked against the code
//! here.
//!
//! WHAT IT DOES NOT PROVE. What production stores: no row was read. The rules
//! do not depend on the answer, which is why they fail closed.

// A panic is how a test reports failure. The restriction lints in Cargo.toml's
// [lints.clippy] exist for production code, as its own comment says.
#![allow(clippy::panic, clippy::expect_used)]

use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use turbobaby_bot::trios::core::Lang;
use turbobaby_bot::trios::legacy_view::{
    customer_bonus_description, customer_bonus_tx_type, customer_order_items, customer_shop_id,
    previous_catalogue_name, shown_line_name, ADMIN_DEDUCTION_SENTENCE, DESCRIBED_TX_TYPES,
    GARDEN_ERA_TX_TYPES, KEPT_LINE_KEYS, MASKED_LINE_NAME_KEY, REFERRAL_BONUS_SENTENCE,
    RETIRED_KIND_KEYS, WITHHELD_TX_TYPE,
};

const ORDERS_API: &str = "src/api/orders.rs";
const ORDERS_DB: &str = "src/db/orders.rs";
const LOYALTY_API: &str = "src/api/loyalty.rs";
const CART_API: &str = "src/api/cart.rs";
const REFERRALS_DB: &str = "src/db/referrals.rs";
const RULES: &str = "src/trios/legacy_view.rs";
const ORDERS_SCREEN: &str = "src/ui/screens/orders_screen.rs";
const DETAIL_SCREEN: &str = "src/ui/screens/order_detail_screen.rs";
const PROFILE_SCREEN: &str = "src/ui/screens/profile_screen.rs";
const CART_SCREEN: &str = "src/ui/screens/cart_screen.rs";
const CHECKOUT_SCREEN: &str = "src/ui/screens/checkout_screen.rs";
const CART_ITEM_COMPONENT: &str = "src/ui/components/cart_item.rs";
const CART_STATE: &str = "src/ui/state.rs";
const PRESENTATION_SPEC: &str = "specs/turbobaby/order_presentation.t27";
const RETIREMENT_SPEC: &str = "specs/turbobaby/legacy_retirement.t27";
/// This file, as the contracts name it.
const THIS_FILE: &str = "tests/legacy_view_wiring.rs";

/// Stand-ins for the names the previous shop stored: one in Latin script and
/// two in Cyrillic, as its catalogue wrote them (migrations 013 and 018 seeded
/// Russian names). What they say is irrelevant to the rule, which never reads
/// a name; only that none of them comes back out.
const STORED_NAMES: [&str; 3] = [
    "Stored item name",
    "Сохранённое название",
    "Ещё одно название",
];
/// Stand-ins for the slug ids the same seeds stored.
const STORED_IDS: [&str; 3] = ["stored-item-4p", "stored-item-raw", "stored-item-green"];
/// Every shop value the old checkout stored, in the order it stored them
/// (`src/ui/screens/checkout_screen.rs` from 2026-05-07 to the brand sweep of
/// 2026-09-14).
const STORED_SHOPS: [&str; 7] = [
    "🏠 Woody Sukhumvit",
    "🏠 Woody Thonglor",
    "🏠 Woody Silom",
    "🏠 Woody Phangan",
    "🏠 Woody Haad Rin",
    "🏠 Woody Srithanu",
    "🏠 Woody Weed Pecker",
];
/// What the checkout has stored since the brand sweep.
const LIVE_SHOP: &str = "🏠 TurboBaby";

fn source(rel: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|e| panic!("{rel} is readable: {e}"))
        .replace("\r\n", "\n")
}

/// The source with every `//` comment removed, so prose describing a call is
/// never counted as the call. Naive by intent: no needle below contains `//`
/// and no string literal in the scanned bodies does either.
fn code_of(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(i) => &line[..i],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The body of the first `fn` whose header starts with `header`, braces
/// counted outside string and char literals.
fn body_of(text: &str, header: &str) -> String {
    let start = text
        .find(header)
        .unwrap_or_else(|| panic!("`{header}` is not in the file"));
    let open = start
        + text[start..]
            .find('{')
            .unwrap_or_else(|| panic!("`{header}` has no body"));
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    let mut in_string = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_string {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == b'"' {
                in_string = false;
            }
        } else if c == b'"' {
            in_string = true;
        } else if c == b'{' {
            depth += 1;
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                return text[open..=i].to_string();
            }
        }
        i += 1;
    }
    panic!("`{header}` has an unbalanced body");
}

/// The value of a `pub const NAME : T = value;` declaration in a contract,
/// quotes stripped.
fn spec_value(spec: &str, name: &str) -> String {
    let text = source(spec);
    let needle = format!("pub const {name} :");
    let line = text
        .lines()
        .find(|l| l.starts_with(&needle))
        .unwrap_or_else(|| panic!("{spec} declares no {name}"));
    let value = line
        .split_once('=')
        .map(|(_, v)| v.trim().trim_end_matches(';').trim())
        .unwrap_or_else(|| panic!("{spec}: {name} has no value"));
    value.trim_matches('"').to_string()
}

fn neutral() -> String {
    previous_catalogue_name(Lang::Russian).to_string()
}

// --- The rules on the stored shapes ------------------------------------------------------------

/// Every line shape the old catalogue stored, with the words it stored.
fn stored_lines() -> Value {
    json!([
        { "strain_id": "3f2c8a1e-6b7d-4c19-9e0a-5d4b2f7c8e91", "strain_name": STORED_NAMES[0],
          "accessory_id": null, "accessory_name": null, "tea_id": null, "tea_name": null,
          "set_id": null, "set_name": null, "quantity": 3.5, "unit_price": 350.0,
          "is_set": false, "is_accessory": false, "is_tea": false, "is_tea_set": false },
        { "accessory_id": STORED_IDS[0], "accessory_name": STORED_NAMES[1],
          "quantity": 1.0, "unit_price": 500.0, "is_accessory": true },
        { "tea_id": STORED_IDS[2], "tea_name": STORED_NAMES[2], "quantity": 2.0,
          "unit_price": 180.0, "is_tea": true, "fulfillment": "dine_in" },
        { "set_id": STORED_IDS[1], "set_name": STORED_NAMES[0], "quantity": 1.0, "is_set": true },
        // A shape of the previous shop's bot that this repository never declared.
        { "name": STORED_NAMES[0], "weight": 3.5, "price": 1225.0, "quantity": 1.0 },
    ])
}

#[test]
fn a_retired_kind_line_never_reaches_a_customer_with_its_stored_name() {
    let stored = stored_lines();
    let served = customer_order_items(&stored);
    let text = served.to_string();
    for word in STORED_NAMES.iter().chain(STORED_IDS.iter()) {
        assert!(
            !text.contains(word),
            "`{word}` reached the customer: {text}"
        );
    }
    for key in RETIRED_KIND_KEYS {
        if key == MASKED_LINE_NAME_KEY {
            continue;
        }
        assert!(!text.contains(key), "`{key}` reached the customer: {text}");
    }
    for word in ["weight", "\"price\"", "fulfillment", "is_tea", "3f2c8a1e"] {
        assert!(
            !text.contains(word),
            "`{word}` reached the customer: {text}"
        );
    }

    let (stored, served) = (
        stored.as_array().expect("a list"),
        served.as_array().expect("a list"),
    );
    assert_eq!(served.len(), stored.len(), "no line is dropped");
    for (s, c) in stored.iter().zip(served) {
        assert_eq!(c[MASKED_LINE_NAME_KEY], json!(neutral()), "{c}");
        // Quantity and money exactly as stored, and nothing invented.
        for key in KEPT_LINE_KEYS {
            assert_eq!(c.get(key), s.get(key), "{key} of {s}");
        }
        let kept = c.as_object().expect("an object").len();
        let expected = 1 + KEPT_LINE_KEYS
            .iter()
            .filter(|k| s.get(**k).is_some())
            .count();
        assert_eq!(kept, expected, "only the name and the stored figures: {c}");
    }
}

#[test]
fn a_bike_line_passes_as_stored_beside_a_masked_one() {
    let bike = json!({
        "strain_id": null, "strain_name": null, "accessory_id": null, "accessory_name": null,
        "tea_id": null, "tea_name": null, "set_id": null, "set_name": null,
        "quantity": 1.0, "unit_price": null, "is_set": null, "is_accessory": null,
        "is_tea": null, "is_tea_set": null, "fulfillment": "pickup",
        "bike": { "bike_key": "nmax-155", "bike_name": "Yamaha NMAX 155",
                  "deal": { "kind": "bike_rental", "rental_start": "2026-10-01",
                            "rental_end": "2026-10-03", "rate_thb_day": null, "deposit": null } }
    });
    let order = json!([bike.clone(), { "strain_name": STORED_NAMES[0], "quantity": 1.0 }]);
    let served = customer_order_items(&order);
    assert_eq!(served[0], bike);
    assert_eq!(
        served[1],
        json!({ "strain_name": neutral(), "quantity": 1.0 })
    );
}

#[test]
fn every_shop_the_old_checkout_stored_is_withheld_and_this_one_is_kept() {
    for shop in STORED_SHOPS {
        assert_eq!(customer_shop_id(Some(shop.to_string())), None, "{shop}");
    }
    assert_eq!(
        customer_shop_id(Some(LIVE_SHOP.to_string())).as_deref(),
        Some(LIVE_SHOP)
    );
    // The live value is still what the checkout writes; if it changes, the list
    // above is what to re-read.
    let checkout = source(CHECKOUT_SCREEN);
    assert!(
        checkout.contains(&format!("let shops = [(\"{LIVE_SHOP}\",")),
        "{CHECKOUT_SCREEN} no longer writes {LIVE_SHOP}"
    );
}

#[test]
fn the_sentences_the_writers_compose_are_the_ones_the_rule_serves() {
    let orders = code_of(&source(ORDERS_DB));
    assert!(
        orders.contains("\"Cashback {}% for order {}\""),
        "{ORDERS_DB} no longer writes the cashback sentence the rule reads"
    );
    let referrals = code_of(&source(REFERRALS_DB));
    assert!(referrals.contains(&format!("\"{REFERRAL_BONUS_SENTENCE}\"")));
    assert!(referrals.contains("\"Milestone bonus for {} referrals\""));
    let loyalty = code_of(&source(LOYALTY_API));
    assert!(loyalty.contains(&format!("\"{ADMIN_DEDUCTION_SENTENCE}\"")));

    let served = |tx: &str, text: String| customer_bonus_description(tx, Some(text.clone()));
    let cashback = format!("Cashback {}% for order {}", 5.0_f64, "ord-7f3a9c2e");
    assert_eq!(served("order_cashback", cashback.clone()), Some(cashback));
    let milestone = format!("Milestone bonus for {} referrals", 5);
    assert_eq!(
        served("referral_milestone", milestone.clone()),
        Some(milestone)
    );

    // The welcome credit's sentence names the garden; it is written today and
    // withheld on read.
    assert!(referrals.contains("\"Welcome bonus from a friend's garden invite\""));
    assert!(!DESCRIBED_TX_TYPES.contains(&"referral_welcome"));
    assert_eq!(
        served(
            "referral_welcome",
            "Welcome bonus from a friend's garden invite".to_string()
        ),
        None
    );
    // A garden row, whatever its description stored.
    for tx in GARDEN_ERA_TX_TYPES {
        assert_eq!(served(tx, format!("Harvest: {}", STORED_NAMES[0])), None);
    }
}

#[test]
fn a_garden_row_reaches_every_bundle_as_a_generic_bonus() {
    // The type is served empty, and the profile's label function -- the same
    // since it first shipped on 2026-08-07 -- labels a type it does not know
    // with the generic bonus label, so a bundle cached before answer 3 prints
    // that label too, not the garden's.
    for tx in GARDEN_ERA_TX_TYPES {
        assert_eq!(customer_bonus_tx_type(tx), WITHHELD_TX_TYPE);
    }
    let profile = code_of(&source(PROFILE_SCREEN));
    let label = body_of(&profile, "fn bonus_tx_label(");
    assert!(label.contains("_ => t(lang, T_PROFILE_BONUS_OTHER).to_string(),"));
    assert!(
        !label.contains(&format!("\"{WITHHELD_TX_TYPE}\"")),
        "the withheld type has an arm of its own"
    );
    // Every other type reaches the label function as stored.
    for tx in [
        "order_cashback",
        "referral_bonus",
        "admin_grant",
        "admin_deduction",
    ] {
        assert_eq!(customer_bonus_tx_type(tx), tx);
    }
}

// --- The handlers --------------------------------------------------------------------------------

#[test]
fn the_customer_order_reads_go_through_the_rule_and_the_admin_reads_do_not() {
    let api = code_of(&source(ORDERS_API));
    for header in ["async fn get_user_orders(", "async fn get_order_details("] {
        let body = body_of(&api, header);
        assert!(
            body.contains("Order::for_customer"),
            "{header} does not serve the customer view"
        );
        assert!(
            !body.contains("Order::from"),
            "{header} serves the row whole"
        );
    }
    // The archive: the two admin reads keep the stored row.
    for header in ["async fn get_orders(", "async fn get_order("] {
        let body = body_of(&api, header);
        assert!(body.contains("check_admin("), "{header} is not admin-only");
        assert!(body.contains("Order::from"), "{header} changed");
    }
    // Nothing else in the handler file builds an order for the wire.
    let production = &api[..api.find("#[cfg(test)]").expect("a test module")];
    assert_eq!(production.matches("Order::from").count(), 2, "{ORDERS_API}");
    assert_eq!(production.matches("Order::for_customer").count(), 2);

    let db = code_of(&source(ORDERS_DB));
    let view = body_of(&db, "pub(crate) fn for_customer(");
    // The money conversion the contract names (order-presentation's
    // ORDER_ENDPOINTS_SERIALISE_THROUGH_ONE_CONVERSION) runs first and is the
    // only one; the view then touches the lines and the shop and nothing else.
    assert!(view.contains("let mut order = Self::from(m);"));
    assert_eq!(view.matches("order.items").count(), 2, "{view}");
    assert_eq!(view.matches("order.shop_id").count(), 2, "{view}");
    assert_eq!(view.matches("order.").count(), 4, "{view}");
    assert!(view.contains("customer_order_items(&order.items)"));
    assert!(view.contains("customer_shop_id(order.shop_id.take())"));
}

#[test]
fn the_bonus_history_serves_no_description_the_rule_did_not_pass() {
    let loyalty = code_of(&source(LOYALTY_API));
    let body = body_of(&loyalty, "async fn get_bonus_history(");
    assert!(body.contains(
        "\"description\": crate::trios::legacy_view::customer_bonus_description(&m.tx_type, m.description),"
    ));
    assert_eq!(
        body.matches("m.description").count(),
        1,
        "one read, through the rule"
    );
    assert!(body
        .contains("\"tx_type\": crate::trios::legacy_view::customer_bonus_tx_type(&m.tx_type),"));
    // The type is read twice, each time as a rule's argument.
    assert_eq!(body.matches("m.tx_type").count(), 2);
    assert_eq!(body.matches("(&m.tx_type").count(), 2);
}

/// Answer 3 first masked a cart line of a retired kind too, under the neutral
/// name and with no picture. Merged on 2026-09-26 with the kept-cart change, a
/// line of a retired kind is not served at all, and that rule is
/// `specs/turbobaby/cart_persistence.t27`'s (`SERVED_CART_KINDS`;
/// `tests/legacy_cart_hidden_wiring.rs` holds its three readers). This holds the
/// two changes to ONE rule: nothing on a cart's path renames a line, and every
/// way into the Mini App's cart asks the served-kind predicate first, so no
/// cart screen is ever handed a line that would need a neutral name.
#[test]
fn a_cart_line_is_held_to_the_kept_cart_rule_and_never_renamed() {
    // The server answers only served rows, each as stored: a served row is a
    // rental row, whose stored name and picture are its own.
    let cart = code_of(&source(CART_API));
    let body = body_of(&cart, "fn cart_model_to_resp(");
    assert!(body.contains("= served_rows(items)"), "{body}");
    assert!(body.contains("name: i.name.clone(),"), "{body}");
    assert!(body.contains("image_url: i.image_url.clone(),"), "{body}");
    // No reader of a cart line calls a renaming rule, and the rules' module
    // declares none.
    for file in [CART_API, CART_SCREEN, CHECKOUT_SCREEN, CART_ITEM_COMPONENT] {
        let code = code_of(&source(file));
        for gone in ["shown_cart_line_name", "cart_line_image", "cart_line_name"] {
            assert!(!code.contains(gone), "{file} still calls {gone}");
        }
    }
    let rules = code_of(&source(RULES));
    for gone in ["fn cart_line", "fn shown_cart_line_name", "LIVE_CART_KIND"] {
        assert!(!rules.contains(gone), "{RULES} declares {gone}");
    }
    // The three ways into the Mini App's cart: a line added, a saved cart
    // loaded, a server cart read. Each asks the one predicate first.
    let state = code_of(&source(CART_STATE));
    let add = body_of(&state, "pub fn add_item(");
    let gate = add
        .find("if !item.is_served() {")
        .unwrap_or_else(|| panic!("{CART_STATE}: add_item does not ask is_served:\n{add}"));
    let push = add
        .find("self.items.push(item);")
        .unwrap_or_else(|| panic!("{CART_STATE}: add_item no longer pushes:\n{add}"));
    assert!(gate < push, "{add}");
    assert!(
        body_of(&state, "pub fn without_retired_lines(").contains("retain(CartItem::is_served)"),
        "{CART_STATE}"
    );
    assert!(
        body_of(&state, "pub fn from_server(").contains("match served_kind(&item.kind)? {"),
        "{CART_STATE}"
    );
    // Nothing else writes a line into a cart.
    let mut writers = 0usize;
    let mut stack = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ui")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("src/ui is readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = code_of(&fs::read_to_string(&path).expect("readable"));
                writers += text.matches(".items.push(").count()
                    + text.matches(".items.extend(").count()
                    + text.matches(".items.insert(").count();
            }
        }
    }
    assert_eq!(writers, 1, "the one push is add_item's");
    // And the contract says the same thing.
    assert_eq!(
        spec_value(RETIREMENT_SPEC, "OWNER_ANSWER_3_CART_RULE_OWNER_ID"),
        "turbobaby/cart-persistence"
    );
    assert_eq!(
        spec_value(RETIREMENT_SPEC, "OWNER_ANSWER_3_RENAMES_A_CART_LINE"),
        "false"
    );
}

// --- The screens --------------------------------------------------------------------------------

#[test]
fn the_order_screens_print_the_neutral_name_in_the_readers_language() {
    // (screen, the one call that prints a line's name, the name function)
    for (screen, call, name_fn) in [
        (
            ORDERS_SCREEN,
            "crate::trios::legacy_view::shown_line_name(lang, &item_name(item))",
            "item_name(",
        ),
        (
            DETAIL_SCREEN,
            "crate::trios::legacy_view::shown_line_name(lang, &item_name(item))",
            "item_name(",
        ),
        (
            PROFILE_SCREEN,
            "crate::trios::legacy_view::shown_line_name(lang, &profile_item_name(i))",
            "profile_item_name",
        ),
    ] {
        let code = code_of(&source(screen));
        assert_eq!(code.matches(call).count(), 1, "{screen}");
        // The name function has its declaration and that one call, so nothing
        // prints a line's name without the rule.
        let uses = code
            .matches(name_fn)
            .count()
            .saturating_sub(code.matches(&format!("_{name_fn}")).count());
        assert_eq!(uses, 2, "{screen}: every use of {name_fn}");
    }
    for sent in [
        neutral(),
        previous_catalogue_name(Lang::English).to_string(),
    ] {
        assert_eq!(
            shown_line_name(Lang::English, &sent),
            "Item from the previous catalogue"
        );
        assert_eq!(shown_line_name(Lang::Russian, &sent), neutral());
    }
}

#[test]
fn a_garden_era_bonus_row_carries_the_generic_label() {
    let profile = code_of(&source(PROFILE_SCREEN));
    let label = body_of(&profile, "fn bonus_tx_label(");
    assert!(label.contains(
        "\"garden_harvest\" | \"garden_reward\" => t(lang, T_PROFILE_BONUS_OTHER).to_string(),"
    ));
    assert_eq!(GARDEN_ERA_TX_TYPES, ["garden_harvest", "garden_reward"]);
    // The garden label is declared and translated, and no screen prints it.
    let mut stack = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ui")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("src/ui is readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = code_of(&fs::read_to_string(&path).expect("readable"));
                assert!(
                    !text.contains("T_PROFILE_BONUS_GARDEN"),
                    "{} prints the garden label",
                    path.display()
                );
            }
        }
    }
}

// --- The contracts ------------------------------------------------------------------------------

#[test]
fn the_contracts_record_what_the_code_does() {
    assert_eq!(
        spec_value(PRESENTATION_SPEC, "RETIRED_LINE_NAME_KEY"),
        "T_ORDER_LINE_PREVIOUS_CATALOGUE"
    );
    assert_eq!(
        spec_value(PRESENTATION_SPEC, "RETIRED_LINE_NAME_FIELD"),
        MASKED_LINE_NAME_KEY
    );
    assert_eq!(
        spec_value(PRESENTATION_SPEC, "RETIRED_LINE_KEPT_FIELD_COUNT"),
        KEPT_LINE_KEYS.len().to_string()
    );
    assert_eq!(spec_value(PRESENTATION_SPEC, "RETIRED_LINE_RULE"), RULES);
    assert_eq!(
        spec_value(PRESENTATION_SPEC, "RETIRED_LINE_WITNESS"),
        THIS_FILE
    );
    assert_eq!(
        spec_value(RETIREMENT_SPEC, "OWNER_ANSWER_3_AT"),
        "2026-09-25"
    );
    assert_eq!(spec_value(RETIREMENT_SPEC, "OWNER_ANSWER_3_RULES"), RULES);
    assert_eq!(
        spec_value(RETIREMENT_SPEC, "OWNER_ANSWER_3_WITNESS"),
        THIS_FILE
    );
    assert_eq!(
        spec_value(RETIREMENT_SPEC, "OWNER_ANSWER_3_DESCRIBED_TX_TYPE_COUNT"),
        DESCRIBED_TX_TYPES.len().to_string()
    );
    assert_eq!(
        spec_value(RETIREMENT_SPEC, "OWNER_ANSWER_3_WITHHELD_TX_TYPE_COUNT"),
        GARDEN_ERA_TX_TYPES.len().to_string()
    );
    assert_eq!(
        spec_value(RETIREMENT_SPEC, "OWNER_ANSWER_3_WITHHELD_TX_TYPE_IS_EMPTY"),
        WITHHELD_TX_TYPE.is_empty().to_string()
    );
    let rules = source(RULES);
    assert!(rules.contains("«Все канабисное аналировать»"), "{RULES}");
    // The key the contract cites is the one the rule looks the name up by (gate 3 cannot
    // evaluate an identifier, so this holds it).
    assert_eq!(
        code_of(&rules)
            .matches("t(lang, T_ORDER_LINE_PREVIOUS_CATALOGUE)")
            .count(),
        1,
        "{RULES}"
    );
    // Each print site the contract cites lands on the call that prints through the rule.
    let text = source(PRESENTATION_SPEC);
    let start = text
        .find("pub const RETIRED_LINE_PRINT_SITES : [3]str = [")
        .expect("the print sites are declared");
    let block = &text[start..start + text[start..].find("];").expect("the list closes")];
    let sites: Vec<&str> = block.split('"').skip(1).step_by(2).collect();
    assert_eq!(sites.len(), 3, "{block}");
    for site in sites {
        let (file, line) = site.split_once(':').expect("path:line");
        let line: usize = line.parse().expect("a line number");
        let cited = source(file)
            .lines()
            .nth(line - 1)
            .unwrap_or_default()
            .to_string();
        assert!(
            cited.contains("crate::trios::legacy_view::shown_line_name(lang, &"),
            "{site} does not print through the rule: {cited}"
        );
    }
}
