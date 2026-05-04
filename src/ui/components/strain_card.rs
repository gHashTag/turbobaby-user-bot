// Strain Card Component for Menu
use dioxus::prelude::*;
use crate::ui::api::types::Strain;
use crate::ui::components::{Button, ButtonVariant};

#[derive(Props, PartialEq, Clone)]
pub struct StrainCardProps {
    #[props(default)]
    strain: Strain,
    #[props(default)]
    on_add_to_cart: EventHandler<Strain>,
}

#[component]
pub fn StrainCard(props: StrainCardProps) -> Element {
    let is_available = props.strain.is_available;
    let is_strain_of_day = props.strain.is_strain_of_day;
    let strain_for_callback = props.strain.clone();

    rsx! {
        div {
            class: "strain-card",
            class: if is_strain_of_day { "strain-of-day" } else { "" },
            class: if !is_available { "unavailable" } else { "" },

            // Badge for strain of day
            if is_strain_of_day {
                div { class: "sod-badge", "🌟 Strain of Day" }
            }

            // Image
            div { class: "strain-image",
                img {
                    src: "{props.strain.image_url}",
                    alt: "{props.strain.name}",
                    loading: "lazy"
                }
            }

            // Content
            div { class: "strain-content",
                h3 { class: "strain-name", "{props.strain.name}" }
                div { class: "strain-type",
                    span { class: "type-badge {strain_type_class(&props.strain)}",
                        "{strain_type_emoji(&props.strain)} {strain_type_name(&props.strain)}"
                    }
                }
                if let Some(thc) = props.strain.thc {
                    div { class: "strain-thc", "THC: {thc:.1}%" }
                }
                if let Some(cbd) = props.strain.cbd {
                    div { class: "strain-cbd", "CBD: {cbd:.1}%" }
                }
                if let Some(desc) = &props.strain.description {
                    p { class: "strain-description", "{desc}" }
                }
                div { class: "strain-price",
                    span { class: "price", "{props.strain.price} ₽" }
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
                        "Add to Cart 🛒"
                    } else {
                        "Sold Out"
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
