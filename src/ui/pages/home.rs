use dioxus::prelude::*;
use crate::ui::routes::Route;

#[component]
pub fn Home() -> Element {
    let mut cart_count = use_signal(|| 0);

    rsx! {
        div { class: "home-page",
            // Header
            div { class: "header",
                h1 { "🌿 Woody Weed Bot" }
                p { class: "subtitle", "Premium Cannabis Delivery in Bangkok" }
            }

            // Hero section
            div { class: "hero",
                h2 { "Strain of the Day" }
                div { class: "sod-card",
                    div { class: "sod-badge", "⭐ 20% OFF" }
                    div { class: "sod-name", "Girl Scout Cookies" }
                    div { class: "sod-type", "Hybrid • 24% THC" }
                    button {
                        class: "btn btn-primary",
                        onclick: move |_| {
                            cart_count += 1;
                        },
                        "Add to Cart - 400 ฿"
                    }
                }
            }

            // Categories
            div { class: "categories",
                h2 { "Categories" }
                div { class: "category-grid",
                    Link { to: Route::Menu {},
                        div { class: "category-card",
                            div { class: "category-icon", "🌿" }
                            div { class: "category-name", "Strains" }
                        }
                    }
                    Link { to: Route::Sets {},
                        div { class: "category-card",
                            div { class: "category-icon", "📦" }
                            div { class: "category-name", "Sets" }
                        }
                    }
                    Link { to: Route::Sommelier {},
                        div { class: "category-card",
                            div { class: "category-icon", "🍷" }
                            div { class: "category-name", "Sommelier" }
                        }
                    }
                    Link { to: Route::Accessories {},
                        div { class: "category-card",
                            div { class: "category-icon", "💨" }
                            div { class: "category-name", "Accessories" }
                        }
                    }
                    Link { to: Route::Tea {},
                        div { class: "category-card",
                            div { class: "category-icon", "🍵" }
                            div { class: "category-name", "Tea" }
                        }
                    }
                    Link { to: Route::Garden {},
                        div { class: "category-card",
                            div { class: "category-icon", "🌱" }
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
            Link { to: Route::Home {}, class: "nav-item active", "🏠" }
            Link { to: Route::Menu {}, class: "nav-item", "🌿" }
            Link {
                to: Route::Cart {},
                class: "nav-item",
                span { class: "cart-badge", "{cart_count}" }
                "🛒"
            }
            Link { to: Route::Orders {}, class: "nav-item", "📋" }
            Link { to: Route::Profile {}, class: "nav-item", "👤" }
        }
    }
}
