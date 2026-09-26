//! The name a bike line is printed under on the customer's order screens.
//!
//! A bike line of an order (`OrderItem::bike`, `BikeLine` in
//! `src/db/orders.rs`) names its bike with two fields: `bike_key`, the family
//! key, which every bike line carries, and `bike_name`, the brand and model as
//! they read when the line was written, which may be absent. The customer's
//! order screens (the list, the detail and the profile's recent orders) read
//! only the old catalogue's name keys, and a bike line fills none of them --
//! the server even serves them empty on a bike line
//! (`legacy_view::customer_order_line`) -- so until 2026-09-26 every bike line
//! printed the screens' last resort, the untranslated "Unknown".
//!
//! The rule is the wire's own and nothing more: the stored `bike_name`, else
//! the `bike_key`. `BikeLine::bike_name` documents that fallback ("`None` falls
//! back to `bike_key` at render time — never to a guess at the model"), and the
//! staff message (`admin_item_line` in `src/api/orders.rs`) names a bike line
//! the same way. Nothing is looked up: the order screens hold no catalogue, and
//! a family key is what the line itself names. Nothing is invented either: a
//! line that carries neither field has no name here, and the screen keeps its
//! old fallback for it.
//!
//! Host-compiled on purpose. The screens live under `src/ui`, which
//! `cargo test` never compiles (`src/lib.rs` gates it on wasm32), so the rule
//! and its tests live here and the screens call it.
//! `specs/turbobaby/order_presentation.t27` records it (`BIKE_LINE_NAME_*`).

use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// The two fields of a served bike line that can name it, and nothing else.
///
/// Deserialized leniently: the order screens deserialize a whole list of
/// orders in one pass, so a `bike` of an unexpected shape must degrade to "no
/// name here" rather than fail every order the customer has. `null` or an
/// absent `bike` is `None` at the field (`Option<OrderLineBike>`); anything
/// else that is not an object, and any field that is not a string, reads as
/// absent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderLineBike {
    /// `bikes.key`, the family key (`"nmax-155"`).
    pub bike_key: Option<String>,
    /// Brand and model as stored with the line (`"Yamaha NMAX 155"`).
    pub bike_name: Option<String>,
}

/// The wire fields [`OrderLineBike`] reads, in the order they are tried.
pub const BIKE_LINE_NAME_FIELDS: [&str; 2] = ["bike_name", "bike_key"];

impl OrderLineBike {
    /// Read the naming fields out of a served `bike` value.
    pub fn from_value(value: &Value) -> Self {
        let text = |field: &str| value.get(field).and_then(Value::as_str).map(str::to_string);
        Self {
            bike_key: text(BIKE_LINE_NAME_FIELDS[1]),
            bike_name: text(BIKE_LINE_NAME_FIELDS[0]),
        }
    }

    /// The stored name, else the family key, each only when it holds more
    /// than whitespace. `None` when the line carries neither.
    pub fn name(&self) -> Option<String> {
        let usable = |field: &Option<String>| {
            field
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
        };
        usable(&self.bike_name).or_else(|| usable(&self.bike_key))
    }
}

impl<'de> Deserialize<'de> for OrderLineBike {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Ok(Self::from_value(&value))
    }
}

/// What the order screens print for a line before their old-catalogue keys:
/// the bike's name when the line is a bike line that names one.
pub fn bike_line_name(bike: Option<&OrderLineBike>) -> Option<String> {
    bike.and_then(OrderLineBike::name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The shape the order screens deserialize a line into, cut down to what
    /// this rule reads plus one old-catalogue name key.
    #[derive(Debug, Deserialize)]
    struct Line {
        strain_name: Option<String>,
        bike: Option<OrderLineBike>,
    }

    fn line(value: Value) -> Line {
        serde_json::from_value(value).expect("a served line parses")
    }

    #[test]
    fn a_bike_line_is_named_by_its_stored_name() {
        // The shape `legacy_view::customer_order_line` serves for a bike line:
        // the stored line, with any old-catalogue key emptied.
        let served = line(json!({
            "strain_name": null,
            "quantity": 1.0,
            "fulfillment": "delivery",
            "bike": {
                "bike_key": "nmax-155",
                "bike_name": "Yamaha NMAX 155",
                "deal": { "kind": "bike_rental" }
            }
        }));
        assert_eq!(
            bike_line_name(served.bike.as_ref()).as_deref(),
            Some("Yamaha NMAX 155")
        );
        assert_eq!(served.strain_name, None);
    }

    #[test]
    fn a_bike_line_without_a_name_is_named_by_its_family_key() {
        for bike in [
            json!({ "bike_key": "nmax-155" }),
            json!({ "bike_key": "nmax-155", "bike_name": null }),
            json!({ "bike_key": "nmax-155", "bike_name": "   " }),
            json!({ "bike_key": "nmax-155", "bike_name": 42 }),
        ] {
            let served = line(json!({ "bike": bike }));
            assert_eq!(
                bike_line_name(served.bike.as_ref()).as_deref(),
                Some("nmax-155"),
                "{bike}"
            );
        }
        // Surrounding whitespace is not part of either name.
        let padded = OrderLineBike {
            bike_key: Some(" xadv-750 ".to_string()),
            bike_name: None,
        };
        assert_eq!(padded.name().as_deref(), Some("xadv-750"));
    }

    #[test]
    fn a_line_that_is_not_a_bike_line_gets_no_name_here() {
        // The neutral old-catalogue line the server serves, with no bike.
        let masked = line(json!({ "strain_name": "neutral", "quantity": 2.0 }));
        assert!(masked.bike.is_none());
        assert_eq!(bike_line_name(masked.bike.as_ref()), None);
        // A null bike, as a legacy row stores it.
        let legacy = line(json!({ "strain_name": "neutral", "bike": null }));
        assert!(legacy.bike.is_none());
        assert_eq!(bike_line_name(None), None);
    }

    #[test]
    fn an_unexpected_bike_shape_names_nothing_and_fails_nothing() {
        for bike in [
            json!("nmax-155"),
            json!(7),
            json!([]),
            json!({}),
            json!(true),
        ] {
            let served = line(json!({ "strain_name": "kept", "bike": bike }));
            assert_eq!(bike_line_name(served.bike.as_ref()), None, "{bike}");
            // The rest of the line still parses.
            assert_eq!(served.strain_name.as_deref(), Some("kept"));
        }
        let blank = OrderLineBike {
            bike_key: Some(String::new()),
            bike_name: Some(" ".to_string()),
        };
        assert_eq!(blank.name(), None);
    }

    /// The fields are the wire's own: a line written by the backend's own
    /// `OrderItem`, served through the customer read's rule, is named by this
    /// module. A rename of either field in `BikeLine` fails here instead of
    /// quietly naming every bike line nothing.
    #[cfg(all(not(target_arch = "wasm32"), feature = "backend"))]
    #[test]
    fn a_line_the_backend_writes_and_the_customer_read_serves_is_named() {
        use crate::db::orders::{BikeDeal, BikeLine, OrderItem};
        let item = |bike_name: Option<&str>| OrderItem {
            strain_id: None,
            strain_name: None,
            accessory_id: None,
            accessory_name: None,
            tea_id: None,
            tea_name: None,
            set_id: None,
            set_name: None,
            quantity: 1.0,
            unit_price: None,
            is_set: None,
            is_accessory: None,
            is_tea: None,
            is_tea_set: None,
            fulfillment: Some("pickup".to_string()),
            bike: Some(BikeLine {
                bike_key: "nmax-155".to_string(),
                bike_name: bike_name.map(str::to_string),
                deal: BikeDeal::BikeSale { price_thb: None },
            }),
        };
        for (stored, want) in [
            (Some("Yamaha NMAX 155"), "Yamaha NMAX 155"),
            (None, "nmax-155"),
        ] {
            let written = serde_json::to_value(item(stored)).expect("an order line serializes");
            let bike = written.get("bike").expect("a bike line carries its bike");
            for field in BIKE_LINE_NAME_FIELDS {
                assert!(
                    bike.get(field).is_some(),
                    "{field} is not on the wire: {bike}"
                );
            }
            let served = crate::trios::legacy_view::customer_order_line(&written);
            let read = line(served);
            assert_eq!(bike_line_name(read.bike.as_ref()).as_deref(), Some(want));
        }
    }
}
