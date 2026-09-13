use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(unreachable_pub)]
// Used as a field of pub structs in src/api/*.rs request/response bodies; pub(crate) would cascade.
// The bike cutover orphaned this wire type on the server path: nothing
// constructs it since the catalog reads `bikes` (D4/D6). Named type
// references remain in the legacy UI/promo surfaces, which rebrand epic
// #26 retires; deletion happens there, not as a drive-by.
#[allow(dead_code)]
pub struct Strain {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    pub thc_percent: Option<f64>,
    pub cbd_percent: Option<f64>,
    pub effect: Option<String>,
    pub flavor_profile: Option<String>,
    pub description: Option<String>,
    pub price_per_gram: f64,
    pub available_grams: Option<f64>,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub is_available: bool,
    pub is_strain_of_day: bool,
    pub strain_of_day_discount: f64,
    // Bilingual EN fields (nullable, migration 016)
    pub name_en: Option<String>,
    pub description_en: Option<String>,
    pub effect_en: Option<String>,
    pub flavor_profile_en: Option<String>,
    pub strain_type_en: Option<String>,
    // Marketing flags (migration 028, TZ #2). Wire-compatible: clients that
    // don't know about these fields just ignore them.
    #[serde(default)]
    pub discount_percent: f64,
    #[serde(default)]
    pub sale_price: Option<f64>,
    #[serde(default)]
    pub sale_active: bool,
    /// RFC3339 string when set; None means "no expiry" or "not set".
    #[serde(default)]
    pub sale_until: Option<String>,
    #[serde(default)]
    pub is_best_seller: bool,
    #[serde(default)]
    pub is_new_arrival: bool,
    #[serde(default)]
    pub new_until: Option<String>,
    #[serde(default)]
    pub display_order: i32,
}

#[cfg(test)]
mod tests {
    use super::Strain;

    #[test]
    fn test_strain_serde_roundtrip() {
        let s = Strain {
            id: "uuid-1".into(),
            name: "OG Kush".into(),
            category: Some("Indica".into()),
            thc_percent: Some(22.5),
            cbd_percent: Some(0.1),
            effect: Some("Relax".into()),
            flavor_profile: Some("Earthy".into()),
            description: Some("Classic.".into()),
            price_per_gram: 350.0,
            available_grams: Some(100.0),
            image_url: Some("/uploads/abc.jpg".into()),
            video_url: None,
            is_available: true,
            is_strain_of_day: false,
            strain_of_day_discount: 0.0,
            name_en: Some("OG Kush EN".into()),
            description_en: None,
            effect_en: None,
            flavor_profile_en: None,
            strain_type_en: None,
            discount_percent: 0.0,
            sale_price: None,
            sale_active: false,
            sale_until: None,
            is_best_seller: false,
            is_new_arrival: false,
            new_until: None,
            display_order: 0,
        };
        let json = serde_json::to_value(&s).unwrap();
        let back: Strain = serde_json::from_value(json).unwrap();
        assert_eq!(back.id, "uuid-1");
        assert_eq!(back.price_per_gram, 350.0);
        assert!(back.is_available);
    }
}

// Cycle #80: `Strain::from_row` removed — all SELECTs now go through
// SeaORM and use `Strain::from(Model)`. See the From-impl below.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrainOfDay {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    pub thc_percent: Option<f64>,
    pub price_per_gram: f64,
    pub strain_of_day_discount: f64,
    pub image_url: Option<String>,
    /// Carousel: a featured strain can be shown as a card OR a video.
    pub video_url: Option<String>,
    /// Carousel: localized name for non-RU users (lang::localized fallback).
    pub name_en: Option<String>,
}

// Cycle #80: `StrainOfDay::from_row` removed alongside `Strain::from_row`.
// Conversion now via `From<Model>` below.

// Cycle #80: SeaORM `Model` → wire-shape `Strain` conversion. Wire shape
// has two cosmetic differences from the entity:
//   * `sale_until` / `new_until` are RFC3339 strings (clients parse them),
//     entity has `DateTimeWithTimeZone`.
//   * No `created_at` / `strain_of_day_set_at` on the wire (admin-only).
//
// f64 columns get the same `is_finite().max(0.0)` clamp `from_row` did —
// guards against the rare NaN/Inf that can sneak in from manual SQL fixes.
impl From<crate::db::entities::strain::Model> for Strain {
    fn from(m: crate::db::entities::strain::Model) -> Self {
        let clamp = |v: f64| -> f64 {
            if v.is_finite() {
                v.max(0.0)
            } else {
                0.0
            }
        };
        Self {
            id: m.id,
            name: m.name,
            category: m.category,
            thc_percent: m.thc_percent.filter(|v| v.is_finite()),
            cbd_percent: m.cbd_percent.filter(|v| v.is_finite()),
            effect: m.effect,
            flavor_profile: m.flavor_profile,
            description: m.description,
            price_per_gram: clamp(m.price_per_gram),
            available_grams: m.available_grams.filter(|v| v.is_finite()),
            image_url: m.image_url,
            video_url: m.video_url,
            is_available: m.is_available,
            is_strain_of_day: m.is_strain_of_day,
            strain_of_day_discount: clamp(m.strain_of_day_discount),
            name_en: m.name_en,
            description_en: m.description_en,
            effect_en: m.effect_en,
            flavor_profile_en: m.flavor_profile_en,
            strain_type_en: m.strain_type_en,
            discount_percent: clamp(m.discount_percent),
            sale_price: m.sale_price.filter(|v| v.is_finite()),
            sale_active: m.sale_active,
            sale_until: m.sale_until.map(|t| t.to_rfc3339()),
            is_best_seller: m.is_best_seller,
            is_new_arrival: m.is_new_arrival,
            new_until: m.new_until.map(|t| t.to_rfc3339()),
            display_order: m.display_order,
        }
    }
}

impl From<crate::db::entities::strain::Model> for StrainOfDay {
    fn from(m: crate::db::entities::strain::Model) -> Self {
        let clamp = |v: f64| -> f64 {
            if v.is_finite() {
                v.max(0.0)
            } else {
                0.0
            }
        };
        Self {
            id: m.id,
            name: m.name,
            category: m.category,
            thc_percent: m.thc_percent.filter(|v| v.is_finite()),
            price_per_gram: clamp(m.price_per_gram),
            strain_of_day_discount: clamp(m.strain_of_day_discount),
            image_url: m.image_url,
            video_url: m.video_url,
            name_en: m.name_en,
        }
    }
}
