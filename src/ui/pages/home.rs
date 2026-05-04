use dioxus::prelude::*;
use crate::ui::routes::Route;
use crate::ui::assets;

#[component]
pub fn Home() -> Element {
    let mut cart_count = use_signal(|| 0);

    // Fetch strain of the day
    let sod = use_resource(|| async move {
        match crate::ui::api::client::ApiClient::new(String::new()).get_strains().await {
            Ok(strains) => strains.into_iter().find(|s| s.is_strain_of_day),
            Err(_) => None,
        }
    });

    let sod_card = match &*sod.read() {
        Some(Some(strain)) => {
            let name = strain.name.clone();
            let img = strain.image_url.clone();
            let price = strain.price_display();
            let thc = strain.thc_display();
            let discount = if strain.strain_of_day_discount > 0.0 {
                format!("{:.0}% OFF", strain.strain_of_day_discount * 100.0)
            } else {
                "20% OFF".to_string()
            };
            let type_label = format!("{} • {}",
                strain_type_emoji(&strain.strain_type),
                strain_type_name(&strain.strain_type)
            );
            let thc_line = thc.unwrap_or_default();
            rsx! {
                div { class: "sod-card",
                    div { class: "sod-badge", "⭐ {discount}" }
                    div { class: "sod-image",
                        img { src: "{img}", alt: "{name}", loading: "lazy" }
                    }
                    div { class: "sod-name", "{name}" }
                    div { class: "sod-type", "{type_label} • THC {thc_line}" }
                    button {
                        class: "btn btn-primary",
                        onclick: move |_| { cart_count += 1; },
                        "Add to Cart — {price}"
                    }
                }
            }
        },
        _ => {
            rsx! {
                div { class: "sod-card",
                    div { class: "sod-badge", "⭐ Strain of the Day" }
                    div { class: "sod-name", "Loading..." }
                }
            }
        }
    };

    rsx! {
        div { class: "home-page",
            // Header with logo
            div { class: "header",
                img {
                    class: "logo",
                    src: assets::logo::MAIN,
                    alt: "Woody Weed Bot"
                }
                p { class: "subtitle", "Premium Cannabis Delivery in Bangkok" }
            }

            // Hero section — Strain of the Day from API
            div { class: "hero",
                h2 { "🔥 Strain of the Day" }
                { sod_card }
            }

            // Categories with SVG icons
            div { class: "categories",
                h2 { "Categories" }
                div { class: "category-grid",
                    Link { to: Route::Menu {},
                        div { class: "category-card",
                            img { class: "category-icon", src: assets::icons::PLANT, alt: "Strains" }
                            div { class: "category-name", "Strains" }
                        }
                    }
                    Link { to: Route::Sets {},
                        div { class: "category-card",
                            div { class: "category-icon emoji", "📦" }
                            div { class: "category-name", "Sets" }
                        }
                    }
                    Link { to: Route::Sommelier {},
                        div { class: "category-card",
                            div { class: "category-icon emoji", "🍷" }
                            div { class: "category-name", "Sommelier" }
                        }
                    }
                    Link { to: Route::Accessories {},
                        div { class: "category-card",
                            div { class: "category-icon emoji", "💨" }
                            div { class: "category-name", "Accessories" }
                        }
                    }
                    Link { to: Route::Tea {},
                        div { class: "category-card",
                            div { class: "category-icon emoji", "🍵" }
                            div { class: "category-name", "Tea" }
                        }
                    }
                    Link { to: Route::Garden {},
                        div { class: "category-card",
                            div { class: "category-icon emoji", "🌱" }
                            div { class: "category-name", "Garden" }
                        }
                    }
                }
            }

            // Quick links
            div { class: "quick-links",
                Link { to: Route::Quest { id: "daily".to_string() },
                    div { class: "quest-banner",
                        div { class: "quest-icon", "🎯" }
                        div { class: "quest-text",
                            div { class: "quest-title", "Daily Quest" }
                            div { class: "quest-desc", "Scan QR codes to earn rewards!" }
                        }
                    }
                }
            }
        }

        // Bottom Navigation
        nav { class: "bottom-nav",
            Link { to: Route::Home {}, class: "nav-item active",
                span { "🏠" }
                span { class: "nav-label", "Home" }
            }
            Link { to: Route::Menu {}, class: "nav-item",
                span { "🌿" }
                span { class: "nav-label", "Menu" }
            }
            Link {
                to: Route::Cart {},
                class: "nav-item",
                span { "🛒" }
                span { class: "cart-badge", "{cart_count}" }
                span { class: "nav-label", "Cart" }
            }
            Link { to: Route::Orders {}, class: "nav-item",
                span { "📋" }
                span { class: "nav-label", "Orders" }
            }
            Link { to: Route::Profile {}, class: "nav-item",
                span { "👤" }
                span { class: "nav-label", "Profile" }
            }
        }
    }
}

fn strain_type_emoji(t: &crate::ui::api::types::StrainType) -> &'static str {
    match t {
        crate::ui::api::types::StrainType::Sativa => "☀️",
        crate::ui::api::types::StrainType::Indica => "🌙",
        crate::ui::api::types::StrainType::Hybrid => "⚖️",
    }
}

fn strain_type_name(t: &crate::ui::api::types::StrainType) -> &'static str {
    match t {
        crate::ui::api::types::StrainType::Sativa => "Sativa",
        crate::ui::api::types::StrainType::Indica => "Indica",
        crate::ui::api::types::StrainType::Hybrid => "Hybrid",
    }
}
