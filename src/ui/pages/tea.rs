// Tea Page - Product catalog for tea products
use dioxus::prelude::*;
use crate::ui::api::client::ApiClient;
use crate::ui::api::types::TeaProduct;
use crate::ui::components::Loading;

#[component]
pub fn Tea() -> Element {
    let teas = use_resource(|| async move {
        match ApiClient::new(String::new(), String::new()).get_tea_products().await {
            Ok(t) => t,
            Err(_) => mock_teas(),
        }
    });

    rsx! {
        div { class: "page tea-page",
            div { class: "page-header",
                div { class: "page-icon", "🍵" }
                h1 { class: "page-title", "Tea" }
                p { class: "page-description", "Premium tea for relaxation" }
            }

            div { class: "filter-tabs",
                button { class: "filter-tab active", "All" }
                button { class: "filter-tab", "CBD Tea" }
                button { class: "filter-tab", "Herbal" }
                button { class: "filter-tab", "Flower" }
            }

            {
                match &*teas.read() {
                    Some(items) => {
                        if items.is_empty() {
                            rsx!(EmptyState {})
                        } else {
                            rsx!(ProductGrid { teas: items.clone() })
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
            div { class: "empty-icon", "🍵" }
            h3 { "No tea available" }
            p { "Check back later for new varieties" }
        }
    }
}

#[component]
fn ProductGrid(teas: Vec<TeaProduct>) -> Element {
    rsx! {
        div { class: "products-grid",
            {teas.into_iter().map(|item| {
                rsx!(TeaCard { key: "{item.id}", tea: item.clone() })
            })}
        }
    }
}

#[component]
fn TeaCard(tea: TeaProduct) -> Element {
    let available = tea.is_available;
    let price = if tea.price.is_finite() {
        format!("{:.0}", tea.price.max(0.0))
    } else {
        "0".to_string()
    };

    let img_url = tea.image_url.as_str();
    let has_image = !img_url.is_empty()
        && (img_url.starts_with("http://") || img_url.starts_with("https://") || (img_url.starts_with("/") && !img_url.starts_with("//")));
    rsx! {
        div { class: "product-card",
            div { class: "product-image",
                if !has_image {
                    div { class: "product-placeholder", "🍵" }
                } else {
                    img { src: "{img_url}?v=2", alt: "{tea.name}", loading: "lazy" }
                }
                if !available {
                    div { class: "product-badge", "Out of Stock" }
                }
            }

            div { class: "product-info",
                h3 { class: "product-name", "{tea.name}" }
                p { class: "product-description", "{tea.description}" }
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

fn mock_teas() -> Vec<TeaProduct> {
    vec![
        TeaProduct {
            id: "t1".to_string(),
            name: "CBD Chamomile Blend".to_string(),
            description: "Relaxing chamomile with CBD".to_string(),
            price: 350.0,
            image_url: String::new(),
            is_available: true,
        },
        TeaProduct {
            id: "t2".to_string(),
            name: "Hibiscus Mint Infusion".to_string(),
            description: "Refreshing herbal blend".to_string(),
            price: 280.0,
            image_url: String::new(),
            is_available: true,
        },
        TeaProduct {
            id: "t3".to_string(),
            name: "Lavender Dream".to_string(),
            description: "Calming lavender flower tea".to_string(),
            price: 320.0,
            image_url: String::new(),
            is_available: true,
        },
        TeaProduct {
            id: "t4".to_string(),
            name: "Green Tea Matcha".to_string(),
            description: "Premium matcha blend".to_string(),
            price: 450.0,
            image_url: String::new(),
            is_available: false,
        },
    ]
}
