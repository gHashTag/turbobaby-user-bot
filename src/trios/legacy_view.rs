//! What a customer is shown of the content the previous shop left in storage.
//!
//! Owner, 2026-09-25, answer 3 of the second numbered list put to him that
//! day, verbatim: «Все канабисное аналировать». The operator read it as:
//! analyse all of it and take it out of customers' sight, without deleting or
//! rewriting stored data. So nothing here writes. Every rule below decides what
//! one READ hands a customer; the rows stay as they are, and the admin views,
//! which are the archive, still read them whole.
//!
//! The database was forked from the previous shop's bot (DECISIONS.md D19), so
//! what it holds was not all written by this repository, and this repository
//! cannot classify all of it. Every rule therefore fails closed: a customer is
//! served what this code can vouch for, and the rest is replaced or withheld.
//!
//! Four reads, and the rules they get:
//!
//! * An ORDER of the previous shop is not shown to its customer at all since
//!   2026-09-26 ([`customer_sees_order`]). Asked that day about the lines
//!   shown as «Позиция прежнего каталога», the owner answered, verbatim:
//!   «А зачем это вообще там?». The operator read it as: a customer must not
//!   see the previous shop's orders at all. Such an order is neither listed nor
//!   answered by id; the customer is told what a missing order tells them.
//!   An order holding a rental line beside a line of a retired kind is this
//!   shop's, and stays shown with that line masked as below: since that day it
//!   is the one case the neutral name is still served for.
//! * An ORDER LINE of a retired kind -- any line that is not a bike line -- is
//!   served as the neutral name plus its stored quantity and unit price, and
//!   nothing else ([`customer_order_line`]). The neutral name is the
//!   operator's wording under answer 3, the key
//!   [`T_ORDER_LINE_PREVIOUS_CATALOGUE`]. The server writes it in Russian, the
//!   language every stored name was written in, under the name key every
//!   customer screen reads first, so a bundle that predates this module prints
//!   it too. `specs/turbobaby/order_presentation.t27` records the rule.
//! * The SHOP an order names is withheld when it names the previous shop
//!   ([`customer_shop_id`]), and since the operator's decision of 2026-09-26 a
//!   withheld shop reaches the screens as no shop label at all
//!   ([`shown_shop`]), never as this shop's name.
//! * A BONUS HISTORY row's stored description is served only when it is one of
//!   the sentences this repository writes, word for word apart from its figures
//!   ([`customer_bonus_description`]), and a garden-era row's type is served
//!   empty ([`customer_bonus_tx_type`]), so every bundle -- an old cached one
//!   too -- labels it with its generic bonus label instead of the garden's.
//!
//! A CART is not this module's. Answer 3 first masked a cart line of a retired
//! kind here too, under the neutral name and with no picture, on the server and
//! on a cart kept on the device. The kept-cart change of 2026-09-26 went
//! further: such a line is not served at all -- not by the cart API, not by the
//! abandoned-cart reminder, and not into the Mini App's cart, whose every way
//! in asks the same predicate (`trios::pricing::cart_kind_is_served`;
//! `specs/turbobaby/cart_persistence.t27`, `SERVED_CART_KINDS`). When the two
//! were merged on 2026-09-26 that rule was kept and the neutral-name cart path
//! was removed: a line that never reaches a customer needs no name.
//!
//! The client's half is where the client builds the text itself:
//! [`shown_line_name`] prints the neutral name in the reader's language on the
//! order screens, and a garden-era bonus row carries the generic label since
//! answer 3 (`bonus_tx_label` in `src/ui/screens/profile_screen.rs`), whatever
//! type a server serves it. `specs/turbobaby/legacy_retirement.t27` records the
//! answer and classifies the reads.
//!
//! Nothing in this file names the old goods: the fixtures below are neutral on
//! purpose (`tests/legacy_vocabulary_wiring.rs` reads every literal under
//! `src/`). `tests/legacy_view_wiring.rs` runs the same rules on the stored
//! shapes themselves and holds the handlers to them.

use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_ORDER_LINE_PREVIOUS_CATALOGUE};
use serde_json::{Map, Value};

/// The keys an order line of the old catalogue names its item with, one id and
/// one name per kind (`OrderItem` in `src/db/orders.rs`). None of them reaches a
/// customer with its stored value.
pub const RETIRED_KIND_KEYS: [&str; 8] = [
    "strain_id",
    "strain_name",
    "accessory_id",
    "accessory_name",
    "tea_id",
    "tea_name",
    "set_id",
    "set_name",
];

/// Where a masked line carries the neutral name: the key every customer screen
/// reads first (`item_name` in the orders list and in the order detail,
/// `profile_item_name` in the profile), so a bundle that predates this module
/// prints the neutral name and not its own fallback.
pub const MASKED_LINE_NAME_KEY: &str = "strain_name";

/// What a masked line keeps, exactly as stored: how many, and at what unit
/// price. Everything else a line of the old catalogue stored -- its ids, its
/// kind flags, its drink service choice, and any key the previous shop's bot
/// wrote that this repository never declared -- is left out, so no id can
/// rebuild the old item in a cart ("reorder" drops a line without one).
pub const KEPT_LINE_KEYS: [&str; 2] = ["quantity", "unit_price"];

/// The language the server writes the neutral name in.
pub const SERVED_NAME_LANG: Lang = Lang::Russian;

/// The previous shop's name, as every shop value its checkout ever stored
/// spells it (each carries it, up to the brand sweep of 2026-09-14). Read from
/// the text, like `retired_tier` in `src/api/loyalty.rs`, so a spelling nobody
/// listed is caught too.
pub const OLD_SHOP_NAME: &str = "woody";

/// What a withheld shop is served as: an empty shop.
///
/// Operator, 2026-09-26: an order of the previous shop must not be shown as
/// TurboBaby's. Until then a withheld shop was served as no shop (`null`), and
/// both order screens print their fallback for no shop, which names
/// TurboBaby, so such an order read as placed with this shop. An absent shop
/// keeps that fallback, so the withheld one needs a value of its own that
/// still reads as a shop to every bundle: an empty one. A bundle built since
/// prints no shop label for it ([`shown_shop`]); one built before prints an
/// empty label, and neither prints this shop's name. The Mini App's checkout
/// never sends an empty shop: it sends its one shop's label with every order
/// (`shops` in `src/ui/screens/checkout_screen.rs`).
pub const WITHHELD_SHOP: &str = "";

/// The bonus types the profile labelled as a garden reward until answer 3.
/// The garden (D5) was the previous shop's mechanic; its rows now carry the
/// generic label and no description.
pub const GARDEN_ERA_TX_TYPES: [&str; 2] = ["garden_harvest", "garden_reward"];

/// The type a garden-era bonus row is served under: none. Every bundle that
/// has shown a bonus history (`bonus_tx_label` in
/// `src/ui/screens/profile_screen.rs`, since 2026-08-07) labels a type it does
/// not know with the generic bonus label, and the garden's own label is the
/// one it would print for the stored type. The row keeps its type.
pub const WITHHELD_TX_TYPE: &str = "";

/// The bonus types whose description a customer may still read, because this
/// repository writes it from a fixed sentence holding no stored name: the
/// cashback (`src/db/orders.rs`), the two referral credits
/// (`src/db/referrals.rs`) and the admin debit (`src/api/loyalty.rs`). The
/// welcome credit is not among them. Its sentence named the garden until
/// 2026-09-26 and names none since ([`WELCOME_CREDIT_SENTENCE`]), but no owner
/// has worded it, so a customer is served no description for a welcome row,
/// old or new: only the generic label, until the owner supplies the wording.
pub const DESCRIBED_TX_TYPES: [&str; 4] = [
    "order_cashback",
    "referral_bonus",
    "referral_milestone",
    "admin_deduction",
];

/// The referral credit's sentence, as `confirm_referral` writes it.
pub const REFERRAL_BONUS_SENTENCE: &str = "Referral bonus for new user";

/// The welcome credit's sentence, as `confirm_referral` stores it for the
/// invited customer since 2026-09-26: the sentence it stored until then with
/// the garden's words dropped and nothing else changed. It is a stored record
/// and is NOT served ([`DESCRIBED_TX_TYPES`] leaves the welcome credit out).
/// The trimmed words are the lane's interim choice, not the owner's, and the
/// repository writes no customer copy of its own; the wording a customer
/// reads is a question put to the owner. No row is rewritten.
pub const WELCOME_CREDIT_SENTENCE: &str = "Welcome bonus from a friend's invite";

/// The admin debit's sentence, as `use_bonus` writes it.
pub const ADMIN_DEDUCTION_SENTENCE: &str = "use_bonus by admin";

/// The neutral name of a line of the old catalogue, in `lang`.
pub fn previous_catalogue_name(lang: Lang) -> &'static str {
    t(lang, T_ORDER_LINE_PREVIOUS_CATALOGUE)
}

/// A bike line: one whose `bike` is an object (`BikeLine` in
/// `src/db/orders.rs`). Every other line is of a retired kind.
pub fn is_bike_line(line: &Value) -> bool {
    line.get("bike").is_some_and(Value::is_object)
}

/// One stored order line, as its customer is served it.
///
/// A bike line is served as stored, except that a retired kind's key holding a
/// value is served empty: the API refuses such a line today
/// (`validate_bike_lines`), and a row stored before that refusal would
/// otherwise print the old item's name above the bike. Every other line is
/// served as the neutral name, [`KEPT_LINE_KEYS`] exactly as stored, and
/// nothing else.
pub fn customer_order_line(stored: &Value) -> Value {
    if is_bike_line(stored) {
        let mut line = stored.clone();
        if let Some(fields) = line.as_object_mut() {
            for key in RETIRED_KIND_KEYS {
                if fields.get(key).is_some_and(|v| !v.is_null()) {
                    fields.insert(key.to_string(), Value::Null);
                }
            }
        }
        return line;
    }
    let mut masked = Map::new();
    masked.insert(
        MASKED_LINE_NAME_KEY.to_string(),
        Value::String(previous_catalogue_name(SERVED_NAME_LANG).to_string()),
    );
    for key in KEPT_LINE_KEYS {
        if let Some(value) = stored.get(key) {
            masked.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(masked)
}

/// An order's stored `items`, as its customer is served them: every line
/// through [`customer_order_line`]. A stored value that is not a list is not
/// served at all (`null`): no writer here stores one, it cannot be read line by
/// line, and an empty list would claim an order with no lines.
pub fn customer_order_items(stored: &Value) -> Value {
    match stored.as_array() {
        Some(lines) => Value::Array(lines.iter().map(customer_order_line).collect()),
        None => Value::Null,
    }
}

/// Whether a stored text names the previous shop, in any case.
pub fn names_the_old_shop(text: &str) -> bool {
    text.to_lowercase().contains(OLD_SHOP_NAME)
}

/// Whether a customer is shown a stored order at all (owner, 2026-09-26; the
/// module header quotes the answer).
///
/// An order is the previous shop's, and is neither listed nor answered by id,
/// when its stored shop names the previous shop ([`names_the_old_shop`]) or
/// when none of its lines is a bike line ([`is_bike_line`]): every line of a
/// retired kind, an empty list, or items that are not a list at all. The
/// previous shop rented no bikes, `create_order` refuses an empty cart, and
/// since migration 085 hid the old catalogue its checkout can store no line
/// that is not a bike line, so an order this code cannot vouch for is not
/// shown -- the same fail-closed reading as every rule here.
///
/// An order holding a bike line AND a line of a retired kind -- a rental this
/// shop took beside the old catalogue before 085 -- is this shop's, and is
/// shown: the rental is the customer's own booking. Its other lines keep the
/// neutral name ([`customer_order_line`]), which since 2026-09-26 is the one
/// case the neutral name is served for. The shop rule decides first: an order
/// naming the previous shop is not shown whatever it holds. Nothing is
/// written; the row keeps what it stored, and the admin reads show it whole.
pub fn customer_sees_order(items: &Value, shop_id: Option<&str>) -> bool {
    if shop_id.is_some_and(names_the_old_shop) {
        return false;
    }
    items
        .as_array()
        .is_some_and(|lines| lines.iter().any(is_bike_line))
}

/// The shop an order names, as its customer is served it: withheld when it
/// names the previous shop, which is served as [`WITHHELD_SHOP`] since
/// 2026-09-26, and as stored otherwise. No shop stays no shop. The row keeps
/// what it stored.
pub fn customer_shop_id(stored: Option<String>) -> Option<String> {
    stored.map(|shop| {
        if names_the_old_shop(&shop) {
            WITHHELD_SHOP.to_string()
        } else {
            shop
        }
    })
}

/// The shop label a customer screen prints for an order, from the shop it
/// was served: `None` for a withheld shop -- no shop label at all (operator,
/// 2026-09-26) -- and otherwise `Some` of the served shop, which is itself
/// `None` when the order names no shop; the screens label that one with their
/// own fallback exactly as before.
pub fn shown_shop(served: Option<&str>) -> Option<Option<&str>> {
    match served {
        Some(shop) if shop == WITHHELD_SHOP => None,
        served => Some(served),
    }
}

/// Whether a bonus row is of a garden-era type.
pub fn is_garden_era_tx(tx_type: &str) -> bool {
    GARDEN_ERA_TX_TYPES.contains(&tx_type)
}

/// A bonus row's stored type, as its customer is served it: withheld
/// ([`WITHHELD_TX_TYPE`]) for a garden-era row, so no bundle can label it as
/// the garden's, and as stored otherwise.
pub fn customer_bonus_tx_type(stored: &str) -> String {
    if is_garden_era_tx(stored) {
        WITHHELD_TX_TYPE.to_string()
    } else {
        stored.to_string()
    }
}

/// A bonus row's stored description, as its customer is served it: kept only
/// when the row is of one of [`DESCRIBED_TX_TYPES`] and the text is that type's
/// sentence, word for word apart from its figures. Anything else -- a
/// garden-era row, a welcome credit old or new (the new one's sentence awaits
/// the owner's wording, [`WELCOME_CREDIT_SENTENCE`]), a type an admin typed, a
/// type the previous shop's bot wrote -- is withheld, because nothing here can
/// tell its text from the old shop's or vouch for its words.
pub fn customer_bonus_description(tx_type: &str, stored: Option<String>) -> Option<String> {
    stored.filter(|text| is_a_sentence_this_code_writes(tx_type, text))
}

fn is_a_sentence_this_code_writes(tx_type: &str, text: &str) -> bool {
    match tx_type {
        "order_cashback" => is_cashback_sentence(text),
        "referral_bonus" => text == REFERRAL_BONUS_SENTENCE,
        "referral_milestone" => is_milestone_sentence(text),
        "admin_deduction" => text == ADMIN_DEDUCTION_SENTENCE,
        _ => false,
    }
}

/// `Cashback {pct}% for order {order_id}`, as `complete_order` writes it: the
/// percent a plain number, the order id an id.
fn is_cashback_sentence(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("Cashback ") else {
        return false;
    };
    let Some((pct, order_id)) = rest.split_once("% for order ") else {
        return false;
    };
    is_plain_number(pct) && is_an_id(order_id)
}

/// `Milestone bonus for {n} referrals`, as `maybe_award_referral_milestones`
/// writes it.
fn is_milestone_sentence(text: &str) -> bool {
    text.strip_prefix("Milestone bonus for ")
        .and_then(|rest| rest.strip_suffix(" referrals"))
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

fn is_plain_number(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|c| c.is_ascii_digit() || c == '.')
        && text.chars().any(|c| c.is_ascii_digit())
}

fn is_an_id(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The name a customer screen prints for an order or cart line: the neutral
/// name in the reader's language when the line carries the neutral name in
/// either language, and the line's own name otherwise. Recognised by the one
/// table the server wrote it from, so the two sides cannot drift apart.
pub fn shown_line_name(lang: Lang, name: &str) -> String {
    if name == previous_catalogue_name(SERVED_NAME_LANG)
        || name == previous_catalogue_name(Lang::English)
    {
        previous_catalogue_name(lang).to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A name and an id as a row of the old catalogue stores them. Neutral on
    /// purpose; what they held is irrelevant to the rule, which never reads it.
    const STORED_NAME: &str = "Stored catalogue name";
    const STORED_ID: &str = "stored-catalogue-id";

    fn neutral_ru() -> Value {
        Value::String(previous_catalogue_name(Lang::Russian).to_string())
    }

    #[test]
    fn the_neutral_name_is_the_operators_wording_in_both_languages() {
        assert_eq!(
            previous_catalogue_name(Lang::Russian),
            "Позиция прежнего каталога"
        );
        assert_eq!(
            previous_catalogue_name(Lang::English),
            "Item from the previous catalogue"
        );
        assert_eq!(SERVED_NAME_LANG, Lang::Russian);
    }

    #[test]
    fn a_line_of_every_retired_kind_keeps_the_neutral_name_quantity_and_price_only() {
        for (id_key, name_key) in [
            ("strain_id", "strain_name"),
            ("accessory_id", "accessory_name"),
            ("tea_id", "tea_name"),
            ("set_id", "set_name"),
        ] {
            let stored = json!({
                id_key: STORED_ID,
                name_key: STORED_NAME,
                "quantity": 2.5,
                "unit_price": 350.0,
                "is_set": false,
                "is_accessory": true,
                "is_tea": false,
                "is_tea_set": false,
                "fulfillment": "takeaway",
            });
            let served = customer_order_line(&stored);
            assert_eq!(
                served,
                json!({
                    "strain_name": neutral_ru(),
                    "quantity": 2.5,
                    "unit_price": 350.0,
                }),
                "{name_key}"
            );
        }
    }

    #[test]
    fn the_figures_are_kept_exactly_as_stored_and_an_absent_one_stays_absent() {
        // A null price, a figure stored as a string, a negative one: the rule
        // copies, it never judges or fills in a figure (D9 is the screens').
        let stored = json!({ "tea_name": STORED_NAME, "quantity": "3", "unit_price": null });
        assert_eq!(
            customer_order_line(&stored),
            json!({ "strain_name": neutral_ru(), "quantity": "3", "unit_price": null })
        );
        let stored = json!({ "set_name": STORED_NAME, "quantity": 1, "unit_price": -5 });
        assert_eq!(
            customer_order_line(&stored),
            json!({ "strain_name": neutral_ru(), "quantity": 1, "unit_price": -5 })
        );
        let stored = json!({ "set_name": STORED_NAME });
        assert_eq!(
            customer_order_line(&stored),
            json!({ "strain_name": neutral_ru() })
        );
    }

    #[test]
    fn a_line_this_repository_never_declared_is_masked_too() {
        // The previous shop's bot may have stored keys of its own.
        let stored = json!({
            "name": STORED_NAME,
            "title": STORED_NAME,
            "description": STORED_NAME,
            "image_url": "/assets/stored.webp",
            "quantity": 1,
            "unit_price": 100,
        });
        assert_eq!(
            customer_order_line(&stored),
            json!({ "strain_name": neutral_ru(), "quantity": 1, "unit_price": 100 })
        );
        // Not even an object: a bare stored name.
        assert_eq!(
            customer_order_line(&json!(STORED_NAME)),
            json!({ "strain_name": neutral_ru() })
        );
    }

    #[test]
    fn a_bike_line_is_served_as_stored() {
        let stored = json!({
            "strain_id": null,
            "strain_name": null,
            "quantity": 1.0,
            "unit_price": null,
            "fulfillment": "pickup",
            "bike": {
                "bike_key": "nmax-155",
                "bike_name": "Yamaha NMAX 155",
                "deal": { "kind": "bike_rental", "rental_start": "2026-10-01", "rental_end": "2026-10-03" }
            }
        });
        assert!(is_bike_line(&stored));
        assert_eq!(customer_order_line(&stored), stored);
    }

    #[test]
    fn a_bike_line_never_carries_a_retired_kinds_value() {
        let stored = json!({
            "accessory_id": STORED_ID,
            "accessory_name": STORED_NAME,
            "quantity": 1.0,
            "bike": { "bike_key": "nmax-155", "deal": { "kind": "bike_sale" } }
        });
        let served = customer_order_line(&stored);
        assert_eq!(served["accessory_id"], Value::Null);
        assert_eq!(served["accessory_name"], Value::Null);
        assert_eq!(served["bike"], stored["bike"]);
        assert_eq!(served["quantity"], json!(1.0));
    }

    #[test]
    fn a_bike_key_that_is_not_an_object_does_not_make_a_bike_line() {
        let stored = json!({ "bike": STORED_NAME, "set_name": STORED_NAME, "quantity": 1 });
        assert!(!is_bike_line(&stored));
        assert_eq!(
            customer_order_line(&stored),
            json!({ "strain_name": neutral_ru(), "quantity": 1 })
        );
    }

    #[test]
    fn no_stored_name_or_id_survives_an_order() {
        let stored = json!([
            { "strain_id": STORED_ID, "strain_name": STORED_NAME, "quantity": 1, "unit_price": 1 },
            { "accessory_id": STORED_ID, "accessory_name": STORED_NAME, "quantity": 2 },
            { "tea_id": STORED_ID, "tea_name": STORED_NAME, "quantity": 3, "fulfillment": "dine_in" },
            { "set_id": STORED_ID, "set_name": STORED_NAME, "is_set": true, "quantity": 4 },
            STORED_NAME,
        ]);
        let served = customer_order_items(&stored);
        let text = served.to_string();
        assert!(!text.contains(STORED_NAME), "{text}");
        assert!(!text.contains(STORED_ID), "{text}");
        assert_eq!(served.as_array().map(Vec::len), Some(5), "one line each");
        for line in served.as_array().into_iter().flatten() {
            assert_eq!(line["strain_name"], neutral_ru(), "{line}");
        }
    }

    #[test]
    fn items_that_are_not_a_list_are_not_served() {
        assert_eq!(
            customer_order_items(&json!({ "0": STORED_NAME })),
            Value::Null
        );
        assert_eq!(customer_order_items(&json!(STORED_NAME)), Value::Null);
        assert_eq!(customer_order_items(&Value::Null), Value::Null);
        assert_eq!(customer_order_items(&json!([])), json!([]));
    }

    fn bike_line() -> Value {
        json!({
            "quantity": 1.0,
            "bike": { "bike_key": "nmax-155", "deal": { "kind": "bike_rental" } }
        })
    }

    /// Owner, 2026-09-26: a customer does not see the previous shop's orders
    /// at all. An order with no bike line is not shown, whatever it holds.
    #[test]
    fn an_order_with_no_bike_line_is_not_shown() {
        for items in [
            json!([{ "set_name": STORED_NAME, "quantity": 1 }]),
            json!([
                { "strain_id": STORED_ID, "strain_name": STORED_NAME, "quantity": 1 },
                { "accessory_id": STORED_ID, "quantity": 2 },
            ]),
            json!([{ "bike": STORED_NAME, "quantity": 1 }]),
            json!([STORED_NAME]),
            json!([]),
            json!({ "0": STORED_NAME }),
            json!(STORED_NAME),
            Value::Null,
        ] {
            assert!(!customer_sees_order(&items, None), "{items}");
            assert!(!customer_sees_order(&items, Some("TurboBaby")), "{items}");
        }
    }

    #[test]
    fn a_rental_order_of_this_shop_is_shown_and_one_naming_the_previous_shop_is_not() {
        let rental = json!([bike_line()]);
        assert!(customer_sees_order(&rental, None));
        assert!(customer_sees_order(&rental, Some("\u{1f3e0} TurboBaby")));
        assert!(customer_sees_order(&rental, Some("")));
        for shop in ["\u{1f3e0} Woody Phangan", "WOODY", "the woody one"] {
            assert!(!customer_sees_order(&rental, Some(shop)), "{shop}");
        }
    }

    /// The one case the neutral name is still served for: a rental beside a
    /// line of the old catalogue is this shop's order, shown with the rental as
    /// stored and the other line masked.
    #[test]
    fn a_mixed_order_is_shown_with_its_retired_line_masked() {
        let stored = json!([
            bike_line(),
            { "accessory_id": STORED_ID, "accessory_name": STORED_NAME, "quantity": 1, "unit_price": 250 },
        ]);
        assert!(customer_sees_order(&stored, Some("\u{1f3e0} TurboBaby")));
        let served = customer_order_items(&stored);
        assert_eq!(served[0], bike_line());
        assert_eq!(
            served[1],
            json!({ "strain_name": neutral_ru(), "quantity": 1, "unit_price": 250 })
        );
    }

    #[test]
    fn the_previous_shop_is_withheld_in_any_case_and_this_one_is_kept() {
        let withheld = Some(WITHHELD_SHOP.to_string());
        assert_eq!(customer_shop_id(Some("Woody Pier".to_string())), withheld);
        assert_eq!(customer_shop_id(Some("WOODY".to_string())), withheld);
        assert_eq!(
            customer_shop_id(Some("the woody one".to_string())),
            withheld
        );
        assert_eq!(
            customer_shop_id(Some("TurboBaby".to_string())),
            Some("TurboBaby".to_string())
        );
        assert_eq!(customer_shop_id(None), None);
    }

    /// Operator, 2026-09-26: a withheld shop prints no shop label at all, and
    /// an order of this shop keeps its label exactly as before -- the stored
    /// one, or the screens' own fallback when it names none.
    #[test]
    fn a_withheld_shop_prints_no_label_and_every_other_order_its_label_as_before() {
        // What both order screens do with the served shop, fallback included.
        fn label(served: Option<&str>) -> Option<&str> {
            shown_shop(served).map(|shop| shop.unwrap_or("TurboBaby"))
        }
        let fallback = "TurboBaby";

        let withheld = customer_shop_id(Some("\u{1f3e0} Woody Phangan".to_string()));
        assert_eq!(label(withheld.as_deref()), None);
        assert_eq!(shown_shop(Some(WITHHELD_SHOP)), None);

        let live = customer_shop_id(Some("\u{1f3e0} TurboBaby".to_string()));
        assert_eq!(label(live.as_deref()), Some("\u{1f3e0} TurboBaby"));
        let absent = customer_shop_id(None);
        assert_eq!(label(absent.as_deref()), Some(fallback));
        assert_eq!(shown_shop(None), Some(None));
        assert_eq!(shown_shop(Some("Kamala")), Some(Some("Kamala")));
    }

    #[test]
    fn the_sentences_this_code_writes_are_served() {
        let served = |tx: &str, text: &str| customer_bonus_description(tx, Some(text.to_string()));
        for text in [
            "Cashback 5% for order 7f3a9c2e-1b4d-4e8f-9a6b-2c1d0e9f8a7b",
            "Cashback 7.5% for order ORD_20260801_42",
        ] {
            assert_eq!(served("order_cashback", text), Some(text.to_string()));
        }
        assert_eq!(
            served("referral_bonus", REFERRAL_BONUS_SENTENCE),
            Some(REFERRAL_BONUS_SENTENCE.to_string())
        );
        assert_eq!(
            served("referral_milestone", "Milestone bonus for 3 referrals"),
            Some("Milestone bonus for 3 referrals".to_string())
        );
        assert_eq!(
            served("admin_deduction", ADMIN_DEDUCTION_SENTENCE),
            Some(ADMIN_DEDUCTION_SENTENCE.to_string())
        );
    }

    #[test]
    fn anything_else_is_withheld() {
        let served = |tx: &str, text: &str| customer_bonus_description(tx, Some(text.to_string()));
        // A garden-era row, whatever it says.
        for tx in GARDEN_ERA_TX_TYPES {
            assert!(is_garden_era_tx(tx));
            assert_eq!(served(tx, STORED_NAME), None);
            assert_eq!(served(tx, REFERRAL_BONUS_SENTENCE), None);
        }
        // A welcome credit, old or new, whatever it says: the sentence a new
        // one stores since 2026-09-26 names no garden, but it is not the
        // owner's wording, so it is withheld too until the owner supplies one.
        assert!(!DESCRIBED_TX_TYPES.contains(&"referral_welcome"));
        assert_eq!(served("referral_welcome", WELCOME_CREDIT_SENTENCE), None);
        assert_eq!(served("referral_welcome", STORED_NAME), None);
        assert_eq!(served("referral_welcome", REFERRAL_BONUS_SENTENCE), None);
        assert_eq!(served("referral_bonus", WELCOME_CREDIT_SENTENCE), None);
        // A type an admin typed, or one the previous shop's bot wrote.
        for tx in ["admin_grant", "manual_grant", "manual", "", "order_bonus"] {
            assert_eq!(served(tx, STORED_NAME), None, "{tx}");
        }
        // A described type whose text is not its sentence.
        assert_eq!(served("order_cashback", STORED_NAME), None);
        assert_eq!(
            served(
                "order_cashback",
                "Cashback 5% for order 42 and a stored name"
            ),
            None
        );
        assert_eq!(served("order_cashback", "Cashback % for order 42"), None);
        assert_eq!(served("order_cashback", "Cashback 5% for order "), None);
        assert_eq!(
            served("referral_bonus", "Referral bonus for new user!"),
            None
        );
        assert_eq!(
            served("referral_milestone", "Milestone bonus for many referrals"),
            None
        );
        assert_eq!(
            served("referral_milestone", "Milestone bonus for  referrals"),
            None
        );
        assert_eq!(served("admin_deduction", STORED_NAME), None);
        // Nothing stored stays nothing.
        assert_eq!(customer_bonus_description("order_cashback", None), None);
        assert_eq!(customer_bonus_description("referral_welcome", None), None);
    }

    #[test]
    fn a_garden_era_row_is_served_without_its_type_and_every_other_row_with_it() {
        for tx in GARDEN_ERA_TX_TYPES {
            assert_eq!(customer_bonus_tx_type(tx), WITHHELD_TX_TYPE);
        }
        assert_eq!(WITHHELD_TX_TYPE, "");
        assert!(!is_garden_era_tx(WITHHELD_TX_TYPE));
        for tx in [
            "order_cashback",
            "referral_bonus",
            "referral_welcome",
            "referral_milestone",
            "admin_grant",
            "admin_deduction",
            "",
            "garden",
        ] {
            assert_eq!(customer_bonus_tx_type(tx), tx, "{tx}");
        }
    }

    #[test]
    fn the_neutral_name_is_printed_in_the_readers_language() {
        for sent in [
            previous_catalogue_name(Lang::Russian),
            previous_catalogue_name(Lang::English),
        ] {
            assert_eq!(
                shown_line_name(Lang::English, sent),
                "Item from the previous catalogue"
            );
            assert_eq!(
                shown_line_name(Lang::Russian, sent),
                "Позиция прежнего каталога"
            );
            // Every other language reads the English table (`t`).
            assert_eq!(
                shown_line_name(Lang::Thai, sent),
                "Item from the previous catalogue"
            );
        }
        // Any other name is the line's own.
        assert_eq!(shown_line_name(Lang::English, "NMAX 155"), "NMAX 155");
        assert_eq!(shown_line_name(Lang::Russian, ""), "");
    }
}
