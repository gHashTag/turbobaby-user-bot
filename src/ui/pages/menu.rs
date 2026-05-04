// Menu Page Component - Main storefront
use dioxus::prelude::*;
use crate::ui::api::types::Strain;
use crate::ui::components::StrainGrid;

#[component]
pub fn Menu() -> Element {
    // Fetch strains on mount
    let strains = use_resource(|| async move {
        // Try to fetch from API, return mock data if fails
        match crate::ui::api::client::ApiClient::new(String::new()).get_strains().await {
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

// Mock data for development — uses real asset paths as fallback
fn mock_strains() -> Vec<Strain> {
    vec![
        Strain {
            id: "banana-fritter".to_string(),
            name: "Banana Fritter".to_string(),
            strain_type: crate::ui::api::types::StrainType::Sativa,
            thc: Some(24.0),
            cbd: Some(0.1),
            description: "Comforting and relaxing with anxiety relief".to_string(),
            price: 400.0,
            image_url: "/assets/Banana-fritter.webp".to_string(),
            is_available: true,
            is_strain_of_day: true,
            ..Default::default()
        },
        Strain {
            id: "black-mamba".to_string(),
            name: "Black Mamba".to_string(),
            strain_type: crate::ui::api::types::StrainType::Indica,
            thc: Some(28.0),
            cbd: Some(0.2),
            description: "Deep body stone and relaxation".to_string(),
            price: 450.0,
            image_url: "/assets/Black-Mamba.webp".to_string(),
            is_available: true,
            is_strain_of_day: false,
            ..Default::default()
        },
        Strain {
            id: "super-lemon-haze".to_string(),
            name: "Super Lemon Haze".to_string(),
            strain_type: crate::ui::api::types::StrainType::Sativa,
            thc: Some(22.0),
            cbd: Some(0.1),
            description: "Energizing, focused, uplifting".to_string(),
            price: 400.0,
            image_url: "/assets/Super-Lemon-Haze.webp".to_string(),
            is_available: true,
            is_strain_of_day: false,
            ..Default::default()
        },
    ]
}
