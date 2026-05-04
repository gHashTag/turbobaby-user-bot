use dioxus::prelude::*;
use crate::ui::state::CartItem;
use crate::ui::components::{Button, ButtonVariant};

#[derive(Props, PartialEq, Clone)]
pub struct CartItemProps {
    item: CartItem,
}

#[component]
pub fn CartItemComponent(props: CartItemProps) -> Element {
    let item = props.item.clone();

    rsx! {
        div { class: "cart-item",
            if let Some(image_url) = &item.image_url {
                img {
                    class: "cart-item-image",
                    src: "{image_url}?v=2",
                    alt: "{item.name}"
                }
            }
            div { class: "cart-item-details",
                h4 { class: "cart-item-name", "{item.name}" }
                p { class: "cart-item-price", "{item.price} ₽" }
                div { class: "cart-item-controls",
                    Button {
                        variant: ButtonVariant::Secondary,
                        size: crate::ui::components::ButtonSize::Small,
                        "-",
                    }
                    span { class: "cart-item-quantity", "{item.quantity}" }
                    Button {
                        variant: ButtonVariant::Secondary,
                        size: crate::ui::components::ButtonSize::Small,
                        "+",
                    }
                }
            }
            Button {
                variant: ButtonVariant::Danger,
                size: crate::ui::components::ButtonSize::Small,
                class: "cart-item-remove",
                "🗑️"
            }
        }
    }
}
