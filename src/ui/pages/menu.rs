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

// Mock data for development
fn mock_strains() -> Vec<Strain> {
    vec![
        Strain {
            id: "1".to_string(),
            name: "Northern Lights".to_string(),
            strain_type: crate::ui::api::types::StrainType::Indica,
            thc: Some("18%".to_string()),
            cbd: Some("<1%".to_string()),
            description: "Classic indica with relaxing effects".to_string(),
            price: 1200.0,
            image_url: "https://images.unsplash.com/photo-1603909223429-26bb69f9e9c?w=400".to_string(),
            is_available: true,
            is_strain_of_day: true,
        },
        Strain {
            id: "2".to_string(),
            name: "Sour Diesel".to_string(),
            strain_type: crate::ui::api::types::StrainType::Sativa,
            thc: Some("22%".to_string()),
            cbd: Some("<1%".to_string()),
            description: "Energizing sativa for daytime use".to_string(),
            price: 1400.0,
            image_url: "https://images.unsplash.com/photo-1601062819309-6b95fc4b5c2?w=400".to_string(),
            is_available: true,
            is_strain_of_day: false,
        },
        Strain {
            id: "3".to_string(),
            name: "Blue Dream".to_string(),
            strain_type: crate::ui::api::types::StrainType::Hybrid,
            thc: Some("21%".to_string()),
            cbd: Some("<1%".to_string()),
            description: "Balanced hybrid with sweet berry aroma".to_string(),
            price: 1500.0,
            image_url: "https://images.unsplash.com/photo-1592794569454-5a1b7c6e3c2?w=400".to_string(),
            is_available: true,
            is_strain_of_day: false,
        },
        Strain {
            id: "4".to_string(),
            name: "OG Kush".to_string(),
            strain_type: crate::ui::api::types::StrainType::Hybrid,
            thc: Some("25%".to_string()),
            cbd: Some("<1%".to_string()),
            description: "Legendary hybrid with earthy pine scent".to_string(),
            price: 1600.0,
            image_url: "https://images.unsplash.com/photo-1583784553571-e331f707ee95?w=400".to_string(),
            is_available: false,
            is_strain_of_day: false,
        },
    ]
}
