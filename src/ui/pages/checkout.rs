// Checkout Page Component
use dioxus::prelude::*;
use crate::ui::state::use_cart_state;
use crate::ui::components::{Button, ButtonVariant, Loading};

#[component]
pub fn Checkout() -> Element {
    let mut cart = use_cart_state();
    let mut is_processing = use_signal(|| false);
    let mut error_message = use_signal(|| None::<String>);

    let submit_order = move |_| {
        let cart_items = cart.read().items.clone();
        if cart_items.is_empty() {
            return;
        }

        is_processing.set(true);
        error_message.set(None);

        // For demo: generate order ID
        let order_id = format!("ORD-{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs());

        // Clear cart and navigate to success page
        cart.write().clear();
        is_processing.set(false);
        crate::ui::state::set_route(&format!("/success/{}", order_id));
    };

    rsx! {
        div { class: "page checkout-page",
            h1 { class: "page-title", "🛍️ Checkout" }

            if *is_processing.read() {
                div { class: "checkout-loading",
                    Loading {}
                    p { "Processing your order..." }
                }
            } else {
                div { class: "checkout-content",
                    // Order summary
                    div { class: "checkout-summary",
                        h2 { "Your Order" }
                        div { class: "checkout-items",
                            for item in cart.read().items.iter() {
                                div { class: "checkout-item",
                                    span { class: "item-name", "{item.name}" }
                                    span { class: "item-qty", "x{item.quantity}" }
                                    span { class: "item-price", "{item.price * item.quantity as f64} ₽" }
                                }
                            }
                        }
                        div { class: "checkout-total",
                            span { "Total:" }
                            span { class: "total-amount", "{cart.read().total} ₽" }
                        }
                    }

                    // Delivery info (placeholder)
                    div { class: "checkout-info",
                        h2 { "Delivery" }
                        p { "Delivery address will be obtained from Telegram" }
                        div { class: "info-box",
                            p { "📍 Current location" }
                            p { "⏰ 30-45 min delivery" }
                            p { "💰 Free delivery over 1000 ₽" }
                        }
                    }

                    // Error message
                    if let Some(error) = error_message.read().as_ref() {
                        div { class: "error-message", "{error}" }
                    }

                    // Actions
                    div { class: "checkout-actions",
                        Button {
                            variant: ButtonVariant::Secondary,
                            onclick: move |_| {
                                crate::ui::state::set_route("/cart");
                            },
                            "Back"
                        }
                        Button {
                            variant: ButtonVariant::Primary,
                            disabled: cart.read().items.is_empty(),
                            onclick: submit_order,
                            "Place Order"
                        }
                    }
                }
            }
        }
    }
}
