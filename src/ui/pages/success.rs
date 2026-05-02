// Success Page Component
use dioxus::prelude::*;
use crate::ui::components::{Button, ButtonVariant};

#[component]
pub fn Success(id: String) -> Element {
    rsx! {
        div { class: "page success-page",
            div { class: "success-content",
                div { class: "success-icon", "✅" }
                h1 { class: "success-title", "Order Placed!" }
                p { class: "success-message", "Your order #{id} has been received" }
                p { class: "success-info", "We'll contact you shortly" }

                div { class: "success-actions",
                    Button {
                        variant: ButtonVariant::Primary,
                        onclick: move |_| {
                            crate::ui::state::set_route("/menu");
                        },
                        "Back to Menu"
                    }
                    Button {
                        variant: ButtonVariant::Secondary,
                        onclick: move |_| {
                            crate::ui::state::set_route("/orders");
                        },
                        "My Orders"
                    }
                }
            }
        }
    }
}
