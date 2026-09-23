//! Delivery zones: the shop's Phuket table, and the ETA rule the API serves.
//!
//! Kept as a small config-driven module so zones can be tuned without
//! touching order logic. In production, override via `DELIVERY_ZONES_JSON`;
//! otherwise the built-in defaults below apply. The table customers pick from
//! is the `delivery_zones` rows (migration 087); these defaults mirror them.
//!
//! # The table is the owner's (owner, 2026-09-24)
//!
//! TurboBaby delivers on Phuket, from its counter in Kamala. The 18 zones are
//! the owner's live delivery sheet -- all 16 of its zones at the sheet's own
//! prices -- plus Kathu and Nai Harn, which the brain's delivery rule names
//! and the sheet lacks. Until 2026-09-24 this module shipped four Koh Phangan
//! villages (Haad Rin, Srithanu, Thong Sala, Bottle Beach) beside the pickup
//! row and said so; migration 087 deactivates them in the database.
//!
//! # No ETA is published
//!
//! The shop promises a delivery window, never a travel time in minutes, so no
//! zone carries an ETA and the API serves `null` for both minute fields. Every
//! fee here is indicative; the price a customer pays is named by a human (D11).
//! Not modelled here, on purpose: the sheet's out-of-belt settings and the
//! 17:30 cut-off (`specs/turbobaby/delivery_terms.t27` records both).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryZone {
    pub id: String,
    pub name: String,
    pub name_en: Option<String>,
    /// `None` for every shipped zone: no travel time is published.
    #[serde(default)]
    pub min_eta_minutes: Option<u32>,
    #[serde(default)]
    pub max_eta_minutes: Option<u32>,
    pub delivery_fee_baht: f64,
}

/// One built-in row: `(id, name, name_en, fee in baht)`.
type ZoneRow = (&'static str, &'static str, &'static str, f64);

/// The shop's own counter in Kamala. A handover point rather than a
/// destination, kept apart from the places; collecting there costs nothing.
const PICKUP_ZONE: ZoneRow = (
    "pickup",
    "Самовывоз (TurboBaby)",
    "Pickup at TurboBaby",
    0.0,
);

/// The owner's 18 Phuket zones, in served order -- the order migration 087
/// gives them: by ladder step, cheapest first; within a step, in the live
/// sheet's row order; Kathu and Nai Harn (from the brain's rule) close their
/// step. Pa Khlok 490 and Mai Khao 990 are owner decisions of 2026-09-06; the
/// airport's 690 is the owner's choice of 2026-09-24 from the rule's 590-690.
const PHUKET_ZONES: [ZoneRow; 18] = [
    ("bang_tao", "Банг Тао", "Bang Tao", 290.0),
    ("surin", "Сурин", "Surin", 290.0),
    ("kamala", "Камала", "Kamala", 290.0),
    ("patong", "Патонг", "Patong", 290.0),
    ("thalang_north", "Таланг север", "Thalang North", 390.0),
    ("karon", "Карон", "Karon", 390.0),
    ("phuket_town", "Пхукет-таун", "Phuket Town", 490.0),
    ("pa_khlok", "Паклок", "Pa Khlok", 490.0),
    ("thalang_east", "Таланг восток", "Thalang East", 490.0),
    ("kata", "Ката", "Kata", 490.0),
    ("kathu", "Кату", "Kathu", 490.0),
    ("rawai", "Раваи", "Rawai", 590.0),
    ("chalong", "Чалонг", "Chalong", 590.0),
    ("cape_panwa", "Кейп Панва", "Cape Panwa", 590.0),
    ("nai_thon", "Найтон", "Nai Thon", 590.0),
    ("nai_harn", "Найхарн", "Nai Harn", 590.0),
    ("airport", "Аэропорт", "Airport", 690.0),
    ("mai_khao", "Майкхао", "Mai Khao", 990.0),
];

impl Default for DeliveryZones {
    fn default() -> Self {
        let zone = |(id, name, name_en, fee): ZoneRow| DeliveryZone {
            id: id.into(),
            name: name.into(),
            name_en: Some(name_en.into()),
            min_eta_minutes: None,
            max_eta_minutes: None,
            delivery_fee_baht: fee,
        };
        Self {
            zones: std::iter::once(PICKUP_ZONE)
                .chain(PHUKET_ZONES)
                .map(zone)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryZones {
    pub zones: Vec<DeliveryZone>,
}

impl DeliveryZones {
    /// Load from `DELIVERY_ZONES_JSON` env, or fall back to built-in defaults.
    pub fn from_env_or_default() -> Self {
        if let Ok(raw) = std::env::var("DELIVERY_ZONES_JSON") {
            if !raw.trim().is_empty() {
                match serde_json::from_str::<DeliveryZones>(&raw) {
                    Ok(z) => return z,
                    Err(e) => {
                        tracing::warn!("DELIVERY_ZONES_JSON invalid, using defaults: {}", e);
                    }
                }
            }
        }
        Self::default()
    }
}

/// The ETA the API serves for one stored zone row: both edges, or neither.
///
/// Migration 087 stores no ETA on any row (owner, 2026-09-24), so this is
/// `None` for every shipped zone and both endpoints send `null`. A pair an
/// admin stores is clamped the way the handlers always clamped it -- a
/// negative edge to 0, the upper edge never below the lower -- and a pair with
/// one edge missing is not an ETA: it is `None`, never a number filled in.
pub(crate) fn served_eta(eta_min: Option<i32>, eta_max: Option<i32>) -> Option<(u32, u32)> {
    let min = eta_min?.max(0);
    let max = eta_max?.max(min);
    // Both are >= 0 here, so `unsigned_abs` is the value itself.
    Some((min.unsigned_abs(), max.unsigned_abs()))
}

/// [`served_eta`]'s lower edge for a stored zone row: what both endpoints send.
pub(crate) fn zone_eta_min(zone: &crate::db::entities::delivery_zone::Model) -> Option<u32> {
    served_eta(zone.eta_min, zone.eta_max).map(|(min, _)| min)
}

/// [`served_eta`]'s upper edge for a stored zone row: what both endpoints send.
pub(crate) fn zone_eta_max(zone: &crate::db::entities::delivery_zone::Model) -> Option<u32> {
    served_eta(zone.eta_min, zone.eta_max).map(|(_, max)| max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// The owner's Phuket table (owner, 2026-09-24), in served order: `(name_en, fee)`.
    /// Written out here rather than read from `PHUKET_ZONES`, so a drift in the table is
    /// a failure and not a tautology.
    const DECIDED: [(&str, f64); 18] = [
        ("Bang Tao", 290.0),
        ("Surin", 290.0),
        ("Kamala", 290.0),
        ("Patong", 290.0),
        ("Thalang North", 390.0),
        ("Karon", 390.0),
        ("Phuket Town", 490.0),
        ("Pa Khlok", 490.0),
        ("Thalang East", 490.0),
        ("Kata", 490.0),
        ("Kathu", 490.0),
        ("Rawai", 590.0),
        ("Chalong", 590.0),
        ("Cape Panwa", 590.0),
        ("Nai Thon", 590.0),
        ("Nai Harn", 590.0),
        ("Airport", 690.0),
        ("Mai Khao", 990.0),
    ];

    /// The built-in table as the wire sees it, so these tests read what a caller would.
    fn defaults() -> Vec<Value> {
        let json = serde_json::to_value(DeliveryZones::default()).expect("the table serialises");
        json["zones"].as_array().expect("a zones array").clone()
    }

    fn stored_zone(
        eta_min: Option<i32>,
        eta_max: Option<i32>,
    ) -> crate::db::entities::delivery_zone::Model {
        crate::db::entities::delivery_zone::Model {
            id: uuid::Uuid::nil(),
            name: "Патонг".into(),
            name_en: Some("Patong".into()),
            fee: 290.0,
            min_order: 0.0,
            eta_min,
            eta_max,
            is_active: true,
            sort_order: 4,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn the_built_in_table_is_the_owners_eighteen_phuket_zones_behind_the_pickup_row() {
        let zones = defaults();
        assert_eq!(
            zones.len(),
            1 + DECIDED.len(),
            "the pickup row plus 18 places"
        );
        assert_eq!(zones[0]["name_en"], "Pickup at TurboBaby");
        assert_eq!(
            zones[0]["delivery_fee_baht"], 0.0,
            "collection at the shop is free"
        );
        let places: Vec<(String, f64)> = zones[1..]
            .iter()
            .map(|z| {
                (
                    z["name_en"].as_str().unwrap_or_default().to_string(),
                    z["delivery_fee_baht"].as_f64().unwrap_or(f64::NAN),
                )
            })
            .collect();
        let decided: Vec<(String, f64)> =
            DECIDED.iter().map(|(n, f)| (n.to_string(), *f)).collect();
        assert_eq!(places, decided, "names, fees and order are the owner's");
    }

    #[test]
    fn every_built_in_zone_has_a_name_in_both_locales_and_a_unique_id() {
        let zones = defaults();
        let mut ids: Vec<&str> = zones
            .iter()
            .map(|z| z["id"].as_str().unwrap_or_default())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), zones.len(), "zone ids are unique");
        for z in &zones {
            assert!(!z["name"].as_str().unwrap_or_default().is_empty(), "{z}");
            assert!(!z["name_en"].as_str().unwrap_or_default().is_empty(), "{z}");
        }
    }

    #[test]
    fn no_built_in_zone_carries_an_eta() {
        for z in defaults() {
            assert!(
                z["min_eta_minutes"].is_null(),
                "an ETA nobody published: {z}"
            );
            assert!(
                z["max_eta_minutes"].is_null(),
                "an ETA nobody published: {z}"
            );
        }
    }

    #[test]
    fn no_built_in_zone_is_on_the_other_island() {
        for z in defaults() {
            for village in ["Haad Rin", "Srithanu", "Thong Sala", "Bottle Beach"] {
                assert_ne!(z["name_en"], village, "a Koh Phangan village: {z}");
            }
        }
    }

    #[test]
    fn an_override_without_minutes_reads_as_an_absent_eta() {
        let raw = r#"{"zones":[{"id":"patong","name":"Patong","delivery_fee_baht":290}]}"#;
        let parsed: DeliveryZones = serde_json::from_str(raw).expect("an ETA is optional");
        let json = serde_json::to_value(&parsed).expect("serialises");
        assert!(json["zones"][0]["min_eta_minutes"].is_null());
        assert!(json["zones"][0]["max_eta_minutes"].is_null());
    }

    #[test]
    fn an_absent_eta_is_served_as_absent_and_never_as_zero() {
        assert_eq!(served_eta(None, None), None);
        assert_eq!(
            served_eta(Some(20), None),
            None,
            "no upper edge is invented"
        );
        assert_eq!(
            served_eta(None, Some(40)),
            None,
            "no lower edge is invented"
        );
    }

    #[test]
    fn a_stored_pair_is_served_clamped_as_the_handlers_always_did() {
        assert_eq!(served_eta(Some(20), Some(40)), Some((20, 40)));
        assert_eq!(served_eta(Some(-5), Some(10)), Some((0, 10)));
        assert_eq!(served_eta(Some(30), Some(10)), Some((30, 30)));
        assert_eq!(served_eta(Some(-5), Some(-9)), Some((0, 0)));
    }

    #[test]
    fn a_stored_row_without_minutes_is_served_as_null_by_both_edges() {
        let shipped = stored_zone(None, None);
        assert_eq!(zone_eta_min(&shipped), None);
        assert_eq!(zone_eta_max(&shipped), None);
        let json = serde_json::json!({
            "min_eta_minutes": zone_eta_min(&shipped),
            "max_eta_minutes": zone_eta_max(&shipped),
        });
        assert!(json["min_eta_minutes"].is_null() && json["max_eta_minutes"].is_null());
        let half = stored_zone(Some(20), None);
        assert_eq!((zone_eta_min(&half), zone_eta_max(&half)), (None, None));
        let pair = stored_zone(Some(20), Some(40));
        assert_eq!(
            (zone_eta_min(&pair), zone_eta_max(&pair)),
            (Some(20), Some(40))
        );
    }
}
