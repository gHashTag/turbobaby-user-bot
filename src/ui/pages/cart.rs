// Cart Page - Full implementation
use dioxus::prelude::*;
use crate::ui::state::{use_cart_state, Cart, CartItem};
use crate::ui::components::{Button, ButtonVariant};

#[component]
pub fn CartPage() -> Element {
    let cart = use_cart_state();
    let items = cart.read().items.clone();
    let total = cart.read().total;
    let is_empty = items.is_empty();

    rsx! {
        div { class: "page cart-page",
            h1 { class: "page-title", "🛒 Корзина" }

            if is_empty {
                div { class: "empty-cart",
                    p { "Корзина пуста" }
                    Button {
                        variant: ButtonVariant::Primary,
                        onclick: move |_| {
                            crate::ui::state::set_route("/menu");
                        },
                        "Перейти в меню"
                    }
                }
            } else {
                div { class: "cart-items",
                    for item in &items {
                        CartItemRow {
                            item: item.clone(),
                            cart: cart.clone()
                        }
                    }
                }

                div { class: "cart-summary",
                    div { class: "cart-total",
                        span { class: "total-label", "Итого:" }
                        span { class: "total-amount", "{total} ₽" }
                    }
                    div { class: "cart-actions",
                        Button {
                            variant: ButtonVariant::Secondary,
                            onclick: move |_| {
                                crate::ui::state::set_route("/menu");
                            },
                            "Продолжить покупки"
                        }
                        Button {
                            variant: ButtonVariant::Primary,
                            onclick: move |_| {
                                crate::ui::state::set_route("/checkout");
                            },
                            "Оформить заказ"
                        }
                    }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct CartItemRowProps {
    item: CartItem,
    cart: Signal<Cart>,
}

#[component]
fn CartItemRow(props: CartItemRowProps) -> Element {
    let item = props.item.clone();
    let item_id_decrease = item.id.clone();
    let item_id_increase = item.id.clone();
    let item_id_remove = item.id.clone();
    let mut cart = props.cart.clone();

    let img_url = item.image_url.as_deref().unwrap_or("");
    let has_image = !img_url.is_empty()
        && (img_url.starts_with("http://") || img_url.starts_with("https://") || img_url.starts_with('/'));

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
                p { class: "cart-item-price", "{item.price} ₽" }
                div { class: "cart-item-controls",
                    Button {
                        variant: ButtonVariant::Secondary,
                        onclick: move |_| {
                            let id = item_id_decrease.clone();
                            let mut c = cart.write();
                            if let Some(it) = c.items.iter_mut().find(|i| i.id == id) {
                                if it.quantity > 1 {
                                    it.quantity -= 1;
                                    c.recalculate_total();
                                } else {
                                    c.remove_item(&id);
                                }
                            }
                        },
                        "-"
                    }
                    span { class: "cart-item-quantity", "{item.quantity}" }
                    Button {
                        variant: ButtonVariant::Secondary,
                        onclick: move |_| {
                            let id = item_id_increase.clone();
                            let mut c = cart.write();
                            if let Some(it) = c.items.iter_mut().find(|i| i.id == id) {
                                it.quantity = it.quantity.saturating_add(1);
                                c.recalculate_total();
                            }
                        },
                        "+"
                    }
                }
            }
            Button {
                variant: ButtonVariant::Danger,
                onclick: move |_| {
                    let id = item_id_remove.clone();
                    let mut c = cart.write();
                    c.remove_item(&id);
                },
                class: "cart-item-remove",
                "🗑️"
            }
        }
    }
}
