//! Delivery zones + ETA estimates.
//!
//! Kept as a small config-driven module so zones/ETA can be tuned without
//! touching order logic. In production, override via `DELIVERY_ZONES_JSON`;
//! otherwise the built-in defaults below apply.
//!
//! # Those defaults are the other shop's map
//!
//! `Haad Rin`, `Srithanu` and `Thong Sala` are villages on Koh Phangan.
//! TurboBaby is in Kamala, Phuket — a different island, and the two are not
//! adjacent. The header of this file used to assert the Koh Phangan location
//! as fact, which is how a wrong delivery map reads as a correct one.
//!
//! The names are left in place rather than guessed at, for the same reason
//! `migrations/085_unpublish_woody_catalog.sql` leaves the `delivery_zones`
//! rows alone and says so: a zone is a name, a fee and an ETA, and inventing
//! any of the three puts a number on a customer's screen that nobody
//! measured (D11). TurboBaby's Phuket zones are the owner's to define. Until
//! he does, this module's job is to be obviously unanswered instead of
//! quietly wrong.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryZone {
    pub id: String,
    pub name: String,
    pub name_en: Option<String>,
    pub min_eta_minutes: u32,
    pub max_eta_minutes: u32,
    pub delivery_fee_baht: f64,
}

impl Default for DeliveryZones {
    fn default() -> Self {
        Self {
            zones: vec![
                DeliveryZone {
                    id: "haad_rin".into(),
                    name: "Хаад Рин".into(),
                    name_en: Some("Haad Rin".into()),
                    min_eta_minutes: 20,
                    max_eta_minutes: 40,
                    delivery_fee_baht: 50.0,
                },
                DeliveryZone {
                    id: "srithanu".into(),
                    name: "Сритхану".into(),
                    name_en: Some("Srithanu".into()),
                    min_eta_minutes: 30,
                    max_eta_minutes: 60,
                    delivery_fee_baht: 80.0,
                },
                DeliveryZone {
                    id: "thongsala".into(),
                    name: "Тонгсала".into(),
                    name_en: Some("Thong Sala".into()),
                    min_eta_minutes: 25,
                    max_eta_minutes: 50,
                    delivery_fee_baht: 60.0,
                },
                DeliveryZone {
                    id: "bottle_beach".into(),
                    name: "Боттл Бич".into(),
                    name_en: Some("Bottle Beach".into()),
                    min_eta_minutes: 45,
                    max_eta_minutes: 90,
                    delivery_fee_baht: 150.0,
                },
                DeliveryZone {
                    id: "pickup".into(),
                    name: "Самовывоз (TurboBaby)".into(),
                    name_en: Some("Pickup at TurboBaby".into()),
                    min_eta_minutes: 15,
                    max_eta_minutes: 15,
                    delivery_fee_baht: 0.0,
                },
            ],
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
