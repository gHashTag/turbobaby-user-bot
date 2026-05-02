// Garden Page Component
use dioxus::prelude::*;

#[component]
pub fn Garden() -> Element {
    rsx! {
        div {
            class: "page garden-page",
            h1 {
                class: "page-title",
                "🌱 Your Plants"
            }
            p {
                class: "page-description",
                "Coming soon..."
            }
        }
    }
}
