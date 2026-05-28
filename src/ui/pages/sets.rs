// Sets Page - Product sets and bundles
use dioxus::prelude::*;
use crate::ui::api::client::ApiClient;
use crate::ui::api::types::Set;
use crate::ui::components::Loading;

#[component]
pub fn Sets() -> Element {
    let sets = use_resource(|| async move {
        match ApiClient::new(String::new(), String::new()).get_sets().await {
            Ok(s) => s,
            Err(_) => mock_sets(),
        }
    });

    rsx! {
        div { class: "page sets-page",
            div { class: "page-header",
                h1 { class: "page-title", "🎁 Sets" }
                p { class: "page-description", "Подобранные наборы по настроению" }
            }

            // Featured Sets - deal of day
            {
                match &*sets.read() {
                    Some(items) => {
                        let deal_sets: Vec<_> = items.iter().filter(|s| s.is_deal_of_day).collect();
                        let count = items.len();
                        if !deal_sets.is_empty() {
                            rsx!(
                                div { class: "section-title", "Featured Packs" }
                                div { class: "featured-sets",
                                    {deal_sets.into_iter().map(|set| {
                                        rsx!(FeaturedSetCard { key: "{set.id}", set: set.clone() })
                                    })}
                                }
                                div { class: "section-title", "All Sets ({count})" }
                            )
                        } else {
                            rsx!(div { class: "section-title", "All Sets ({count})" })
                        }
                    },
                    None => {
                        rsx!(div { class: "section-title", "Featured Packs" })
                    }
                }
            }

            {
                match &*sets.read() {
                    Some(items) => {
                        if items.is_empty() {
                            rsx!(EmptyState {})
                        } else {
                            rsx!(SetsGrid { sets: items.clone() })
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
            div { class: "empty-icon", "📦" }
            h3 { "No sets available" }
            p { "Check back later for new bundles" }
        }
    }
}

#[component]
fn SetsGrid(sets: Vec<Set>) -> Element {
    rsx! {
        div { class: "sets-grid",
            {sets.into_iter().map(|set| {
                rsx!(SetCard { key: "{set.id}", set: set.clone() })
            })}
        }
    }
}

#[component]
fn FeaturedSetCard(set: Set) -> Element {
    let discount_badge = if set.discount > 0.0 {
        let discount_pct = format!("{:.0}% OFF", set.discount);
        rsx!(
            div { class: "set-badge featured", "{discount_pct}" }
        )
    } else {
        rsx!()
    };

    let price = format!("{:.0}", set.price);
    let original_price = if set.discount > 0.0 && set.discount < 100.0 {
        let original = set.price / (1.0 - set.discount / 100.0);
        format!("{:.0}", original)
    } else {
        String::new()
    };

    let icon = if set.icon.is_empty() {
        "🎁".to_string()
    } else {
        set.icon.clone()
    };

    rsx! {
        div { class: "set-card featured-set",
            {discount_badge}
            div { class: "set-visual",
                if icon.starts_with("http") || icon.contains('/') {
                    img { src: "{icon}?v=2", alt: "{set.name}", loading: "lazy" }
                } else {
                    div { class: "set-icon", "{icon}" }
                }
            }
            div { class: "set-body",
                div { class: "set-type", "{set.set_type}" }
                h3 { class: "set-name", "{set.name}" }
                p { class: "set-description", "{set.description}" }
                div { class: "set-price-line",
                    span { class: "set-price", "฿{price}" }
                    if !original_price.is_empty() {
                        span { class: "set-original-price", "฿{original_price}" }
                    }
                }
            }
        }
    }
}

#[component]
fn SetCard(set: Set) -> Element {
    let available = set.is_available;
    let price = format!("{:.0}", set.price);
    let original_price = if set.discount > 0.0 && set.discount < 100.0 {
        let original = set.price / (1.0 - set.discount / 100.0);
        format!("{:.0}", original)
    } else {
        String::new()
    };

    let discount_badge = if set.discount > 0.0 {
        let discount_pct = format!("{:.0}% OFF", set.discount);
        rsx!(
            div { class: "set-badge", "{discount_pct}" }
        )
    } else {
        rsx!()
    };

    let icon = if set.icon.is_empty() {
        "📦".to_string()
    } else {
        set.icon.clone()
    };

    rsx! {
        div { class: "set-card",
            {discount_badge}
            div { class: "set-visual",
                if icon.starts_with("http") || icon.contains('/') {
                    img { src: "{icon}?v=2", alt: "{set.name}", loading: "lazy" }
                } else {
                    div { class: "set-icon", "{icon}" }
                }
                if !available {
                    div { class: "set-out-of-stock", "Out of Stock" }
                }
            }
            div { class: "set-body",
                h3 { class: "set-name", "{set.name}" }
                p { class: "set-description", "{set.description}" }
                div { class: "set-price-line",
                    span { class: "set-price", "฿{price}" }
                    if !original_price.is_empty() {
                        span { class: "set-original-price", "฿{original_price}" }
                    }
                }
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

fn mock_sets() -> Vec<Set> {
    vec![
        Set {
            id: "s1".to_string(),
            name: "SATIVA SELECT".to_string(),
            description: "Набор лучших сативных сортов для энергии и креативности".to_string(),
            icon: "🎉".to_string(),
            set_type: "party".to_string(),
            items: vec![],
            price: 2520.0,
            discount: 10.0,
            is_available: true,
            is_deal_of_day: true,
        },
        Set {
            id: "s2".to_string(),
            name: "PARTY WEEK".to_string(),
            description: "Недельный набор для вечеринок: 5 сортов + косяки".to_string(),
            icon: "🎉".to_string(),
            set_type: "party".to_string(),
            items: vec![],
            price: 3825.0,
            discount: 15.0,
            is_available: true,
            is_deal_of_day: true,
        },
        Set {
            id: "s3".to_string(),
            name: "TOLERANCE KILLER".to_string(),
            description: "Мощнейшие индика-сорта для опытных пользователей".to_string(),
            icon: "💀".to_string(),
            set_type: "party".to_string(),
            items: vec![],
            price: 2944.0,
            discount: 8.0,
            is_available: true,
            is_deal_of_day: true,
        },
        Set {
            id: "s4".to_string(),
            name: "FIRST STEP".to_string(),
            description: "Идеальный набор для новичков: 2 мягких сорта + гриндер + бумага".to_string(),
            icon: "🟢".to_string(),
            set_type: "party".to_string(),
            items: vec![],
            price: 1440.0,
            discount: 20.0,
            is_available: true,
            is_deal_of_day: true,
        },
    ]
}
