// Accessories Page - Product catalog for accessories
use dioxus::prelude::*;
use crate::ui::api::client::ApiClient;
use crate::ui::api::types::Accessory;
use crate::ui::components::Loading;

#[component]
pub fn Accessories() -> Element {
    let accessories = use_resource(|| async move {
        match ApiClient::new(String::new(), String::new()).get_accessories().await {
            Ok(a) => a,
            Err(_) => mock_accessories(),
        }
    });

    rsx! {
        div { class: "page accessories-page",
            div { class: "page-header",
                h1 { class: "page-title", "Accessories" }
                p { class: "page-description", "Essential gear for your experience" }
            }

            div { class: "filter-tabs",
                button { class: "filter-tab active", "All" }
                button { class: "filter-tab", "Pipes" }
                button { class: "filter-tab", "Grinders" }
                button { class: "filter-tab", "Rolling" }
                button { class: "filter-tab", "Storage" }
            }

            {
                match &*accessories.read() {
                    Some(items) => {
                        if items.is_empty() {
                            rsx!(EmptyState {})
                        } else {
                            rsx!(ProductGrid { accessories: items.clone() })
                        }
                    },
                    None => {
                        rsx!(Loading {})
                    },
                }
            }
        }
    }
}

#[component]
fn EmptyState() -> Element {
    rsx! {
        div { class: "empty-state",
            div { class: "empty-icon", "box" }
            h3 { "No accessories available" }
            p { "Check back later for new products" }
        }
    }
}

#[component]
fn ProductGrid(accessories: Vec<Accessory>) -> Element {
    rsx! {
        div { class: "products-grid",
            {accessories.into_iter().map(|item| {
                rsx!(AccessoryCard { key: "{item.id}", accessory: item.clone() })
            })}
        }
    }
}

#[component]
fn AccessoryCard(accessory: Accessory) -> Element {
    let available = accessory.is_available;
    let price = if accessory.price.is_finite() {
        format!("{:.0}", accessory.price.max(0.0))
    } else {
        "0".to_string()
    };

    let img_url = accessory.image_url.as_str();
    let has_image = !img_url.is_empty()
        && (img_url.starts_with("http://") || img_url.starts_with("https://") || (img_url.starts_with("/") && !img_url.starts_with("//")));

    rsx! {
        div { class: "product-card",
            div { class: "product-image",
                if !has_image {
                    div { class: "product-placeholder", "tool" }
                } else {
                    img { src: "{img_url}?v=2", alt: "{accessory.name}" }
                }
                if !available {
                    div { class: "product-badge", "Out of Stock" }
                }
            }

            div { class: "product-info",
                div { class: "product-category", "{accessory.category}" }
                h3 { class: "product-name", "{accessory.name}" }
                div { class: "product-price", "{price} THB" }

                button {
                    class: "btn btn-primary btn-sm",
                    disabled: !available,
                    onclick: move |_| {},
                    if available { "Add to Cart" }
                    else { "Unavailable" }
                }
            }
        }
    }
}

fn mock_accessories() -> Vec<Accessory> {
    vec![
        Accessory {
            id: "a1".to_string(),
            name: "Glass Pipe - Classic".to_string(),
            category: "Pipes".to_string(),
            price: 450.0,
            image_url: String::new(),
            is_available: true,
        },
        Accessory {
            id: "a2".to_string(),
            name: "Metal Grinder 4pc".to_string(),
            category: "Grinders".to_string(),
            price: 350.0,
            image_url: String::new(),
            is_available: true,
        },
        Accessory {
            id: "a3".to_string(),
            name: "Rolling Papers King Size".to_string(),
            category: "Rolling".to_string(),
            price: 150.0,
            image_url: String::new(),
            is_available: true,
        },
        Accessory {
            id: "a4".to_string(),
            name: "Smell Proof Jar".to_string(),
            category: "Storage".to_string(),
            price: 250.0,
            image_url: String::new(),
            is_available: false,
        },
    ]
}
