use crate::ui::components::{Button, ButtonVariant};
use crate::ui::state::CartItem;
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct CartItemProps {
    item: CartItem,
}

#[component]
pub fn CartItemComponent(props: CartItemProps) -> Element {
    let item = props.item.clone();

    let img_url = item.image_url.as_deref().unwrap_or("");
    let has_image = !img_url.is_empty()
        && (img_url.starts_with("http://")
            || img_url.starts_with("https://")
            || (img_url.starts_with("/") && !img_url.starts_with("//")));
    let safe_price = if item.price.is_finite() {
        item.price.max(0.0)
    } else {
        0.0
    };

    rsx! {
        div { class: "cart-item",
            if has_image {
                img {
                    class: "cart-item-image",
                    src: "{img_url}?v=2",
                    alt: "{item.name}"
                }
            }
            div { class: "cart-item-details",
                h4 { class: "cart-item-name", "{item.name}" }
                p { class: "cart-item-price", "{crate::trios::pricing::format_baht(safe_price)}" }
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
