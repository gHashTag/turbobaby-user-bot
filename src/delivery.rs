//! Delivery zones + ETA estimates for the Koh Phangan shop.
//!
//! Kept as a small config-driven module so zones/ETA can be tuned without
//! touching order logic. In production, override via `DELIVERY_ZONES_JSON`;
//! otherwise the built-in Koh Phangan defaults apply.

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
                    name: "Самовывоз (Woody Weed)".into(),
                    name_en: Some("Pickup at Woody Weed".into()),
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
