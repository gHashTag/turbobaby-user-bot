// Strain Card Component for Menu
use crate::trios::i18n::{
    t, T_ADD_TO_CART, T_MENU_SOLD_OUT, T_STRAIN_BADGE_BEST, T_STRAIN_BADGE_NEW,
    T_STRAIN_BADGE_SALE, T_STRAIN_BADGE_SOTD,
};
use crate::ui::api::types::Strain;
use crate::ui::components::{Button, ButtonVariant};
use crate::ui::lang;
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct StrainCardProps {
    #[props(default)]
    strain: Strain,
    #[props(default)]
    on_add_to_cart: EventHandler<Strain>,
}

#[component]
pub fn StrainCard(props: StrainCardProps) -> Element {
    let lang = lang::current_lang();
    let is_available = props.strain.is_available;
    let is_strain_of_day = props.strain.is_strain_of_day;
    let strain_for_callback = props.strain.clone();
    let thc_text = props.strain.thc_display();
    let cbd_text = props.strain.cbd_display();
    let name = props.strain.name.clone();
    let add_label = t(lang, T_ADD_TO_CART);
    let sold_label = t(lang, T_MENU_SOLD_OUT);

    // Cycle #131: marketing flag rendering, mirroring menu_screen.rs's
    // render_strain_card. Pre-#131 only the SOTD badge rendered — Sale,
    // Best, New Arrival were dropped silently. The wire `Strain` now
    // carries all seven TZ #2 fields.
    let sale_live = props.strain.sale_active
        && crate::trios::pricing::is_active_until(
            props.strain.sale_until.as_deref(),
            chrono::Utc::now(),
        );
    let new_live = props.strain.is_new_arrival
        && crate::trios::pricing::is_active_until(
            props.strain.new_until.as_deref(),
            chrono::Utc::now(),
        );
    let is_best = props.strain.is_best_seller;

    // Effective price via the shared trios::pricing helper — same math
    // as the server-side price-authority check, so a sale customer's
    // total can't drift between UI and backend.
    let priced = crate::trios::pricing::effective_strain_price(
        &crate::trios::pricing::MarketingFlags {
            price_per_gram: props.strain.price,
            is_strain_of_day,
            strain_of_day_discount: props.strain.strain_of_day_discount,
            sale_active: props.strain.sale_active,
            sale_until: props.strain.sale_until.as_deref(),
            sale_price: props.strain.sale_price,
            discount_percent: props.strain.discount_percent,
            is_new_arrival: props.strain.is_new_arrival,
            new_until: props.strain.new_until.as_deref(),
        },
        chrono::Utc::now(),
    );
    let has_discount = priced.has_discount;
    let original_price = props.strain.price;
    let effective_price_text = if priced.price > 0.0 {
        format!("{:.0} ฿/g", priced.price)
    } else {
        "Price on request".to_string()
    };

    rsx! {
        div {
            class: "strain-card",
            class: if is_strain_of_day { "strain-of-day" } else { "" },
            class: if !is_available { "unavailable" } else { "" },

            // TZ #2 badge stack — same precedence as menu_screen.
            if is_strain_of_day {
                div { class: "sod-badge", "{t(lang, T_STRAIN_BADGE_SOTD)}" }
            }
            if new_live {
                div { class: "new-badge", "{t(lang, T_STRAIN_BADGE_NEW)}" }
            }
            if is_best {
                div { class: "best-badge", "{t(lang, T_STRAIN_BADGE_BEST)}" }
            }
            if sale_live {
                div { class: "sale-badge", "{t(lang, T_STRAIN_BADGE_SALE)}" }
            }

            // Image
            div { class: "strain-image",
                {
                    let url = &props.strain.image_url;
                    if !url.is_empty() && (url.starts_with("http://") || url.starts_with("https://") || (url.starts_with("/") && !url.starts_with("//"))) {
                        rsx! {
                            img {
                                src: "{url}?v=2",
                                alt: "{name}",
                                loading: "lazy"
                            }
                        }
                    } else {
                        rsx! { div { class: "strain-placeholder", "🌿" } }
                    }
                }
            }

            // Content
            div { class: "strain-content",
                h3 { class: "strain-name", "{name}" }
                div { class: "strain-type",
                    span { class: "type-badge {strain_type_class(&props.strain)}",
                        "{strain_type_emoji(&props.strain)} {strain_type_name(&props.strain)}"
                    }
                }
                if let Some(thc) = thc_text {
                    div { class: "strain-thc", "THC: {thc}" }
                }
                if let Some(cbd) = cbd_text {
                    div { class: "strain-cbd", "CBD: {cbd}" }
                }
                if let Some(effect) = &props.strain.effect {
                    p { class: "strain-effect", "✨ {effect}" }
                }
                p { class: "strain-description", "{props.strain.description}" }
                // TZ #2: strikethrough original price + new effective
                // price when any discount applies. Otherwise just the
                // single price.
                div { class: "strain-price",
                    if has_discount && original_price > 0.0 {
                        span { class: "price-old",
                            style: "text-decoration:line-through;color:#888;margin-right:8px;",
                            "{original_price:.0} ฿/g"
                        }
                        span { class: "price price-sale", "{effective_price_text}" }
                    } else {
                        span { class: "price", "{effective_price_text}" }
                    }
                }
            }

            // Add to cart button
            div { class: "strain-actions",
                Button {
                    variant: if is_available { ButtonVariant::Primary } else { ButtonVariant::Ghost },
                    disabled: !is_available,
                    onclick: move |_| {
                        props.on_add_to_cart.call(strain_for_callback.clone());
                    },
                    if is_available {
                        "{add_label} 🛒"
                    } else {
                        "{sold_label}"
                    }
                }
            }
        }
    }
}

fn strain_type_class(strain: &Strain) -> &'static str {
    match strain.strain_type {
        crate::ui::api::types::StrainType::Sativa => "sativa",
        crate::ui::api::types::StrainType::Indica => "indica",
        crate::ui::api::types::StrainType::Hybrid => "hybrid",
    }
}

fn strain_type_emoji(strain: &Strain) -> &'static str {
    match strain.strain_type {
        crate::ui::api::types::StrainType::Sativa => "☀️",
        crate::ui::api::types::StrainType::Indica => "🌙",
        crate::ui::api::types::StrainType::Hybrid => "⚖️",
    }
}

fn strain_type_name(strain: &Strain) -> &'static str {
    match strain.strain_type {
        crate::ui::api::types::StrainType::Sativa => "Sativa",
        crate::ui::api::types::StrainType::Indica => "Indica",
        crate::ui::api::types::StrainType::Hybrid => "Hybrid",
    }
}
