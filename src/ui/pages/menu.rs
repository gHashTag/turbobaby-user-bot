// Menu Page Component - Main storefront
use dioxus::prelude::*;
use crate::ui::api::types::Strain;
use crate::ui::components::StrainGrid;

#[component]
pub fn Menu() -> Element {
    // Fetch strains on mount
    let strains = use_resource(|| async move {
        // Try to fetch from API, return mock data if fails
        match crate::ui::api::client::ApiClient::new("/api".to_string()).get_strains().await {
            Ok(s) => s,
            Err(_) => mock_strains(),
        }
    });

    rsx! {
        div { class: "page menu-page",
            // Header
            div { class: "page-header",
                h1 { class: "page-title", "🌿 Menu" }
                p { class: "page-description", "Browse our premium selection" }
            }

            // Filter tabs (placeholder)
            div { class: "filter-tabs",
                button { class: "filter-tab active", "All" }
                button { class: "filter-tab", "Sativa" }
                button { class: "filter-tab", "Indica" }
                button { class: "filter-tab", "Hybrid" }
            }

            // Strain grid or loading state
            {
                match &*strains.read() {
                    Some(items) => {
                        rsx!(StrainGrid { strains: items.clone() })
                    },
                    None => {
                        rsx!(
                            div { class: "loading-container",
                                p { "Loading strains..." }
                            }
                        )
                    },
                }
            }
        }
    }
}

// Mock data for development (fallback when API is unreachable)
fn mock_strains() -> Vec<Strain> {
    use crate::ui::assets::strains;
    vec![
        Strain {
            id: "banana-fritter".to_string(),
            name: "BANANA FRITTER".to_string(),
            strain_type: crate::ui::api::types::StrainType::Sativa,
            thc: Some(20.0),
            cbd: None,
            description: Some("Classic sativa with tropical banana notes".to_string()),
            effect: None,
            flavor_profile: None,
            image_url: strains::BANANA_FRITTER.to_string(),
            price: 350.0,
            available_grams: None,
            is_available: true,
            is_strain_of_day: true,
            strain_of_day_discount: 0.0,
        },
        Strain {
            id: "black-mamba".to_string(),
            name: "BLACK MAMBA".to_string(),
            strain_type: crate::ui::api::types::StrainType::Indica,
            thc: Some(22.0),
            cbd: None,
            description: Some("Potent indica with deep relaxation effects".to_string()),
            effect: None,
            flavor_profile: None,
            image_url: strains::BLACK_MAMBA.to_string(),
            price: 380.0,
            available_grams: None,
            is_available: true,
            is_strain_of_day: false,
            strain_of_day_discount: 0.0,
        },
        Strain {
            id: "mac-1".to_string(),
            name: "MAC 1".to_string(),
            strain_type: crate::ui::api::types::StrainType::Hybrid,
            thc: Some(23.0),
            cbd: None,
            description: Some("Premium hybrid with creamy, earthy flavour".to_string()),
            effect: None,
            flavor_profile: None,
            image_url: strains::MAC_1.to_string(),
            price: 400.0,
            available_grams: None,
            is_available: true,
            is_strain_of_day: false,
            strain_of_day_discount: 0.0,
        },
        Strain {
            id: "super-lemon-haze".to_string(),
            name: "SUPER LEMON HAZE".to_string(),
            strain_type: crate::ui::api::types::StrainType::Sativa,
            thc: Some(21.0),
            cbd: None,
            description: Some("Energising sativa with zesty lemon aroma".to_string()),
            effect: None,
            flavor_profile: None,
            image_url: strains::SUPER_LEMON_HAZE.to_string(),
            price: 360.0,
            available_grams: None,
            is_available: true,
            is_strain_of_day: false,
            strain_of_day_discount: 0.0,
        },
    ]
}
