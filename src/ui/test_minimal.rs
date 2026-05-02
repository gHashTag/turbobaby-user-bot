use dioxus::prelude::*;

#[component]
pub fn TestMinimal() -> Element {
    rsx! {
        div {
            style: "padding: 20px; font-size: 24px; color: #fff;",
            "TEST: If you see this, Dioxus works!"
        }
    }
}
