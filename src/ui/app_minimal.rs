use dioxus::prelude::*;

#[component]
pub fn AppMinimal() -> Element {
    rsx! {
        div {
            style: "padding: 20px; font-family: sans-serif;",
            h1 { "🌿 Woody Weed Bot - Minimal Test" },
            p { "If you see this, Dioxus is working!" },
            button {
                style: "padding: 12px 24px; background: #39ff14; border: none; cursor: pointer;",
                onclick: move |_| {
                    println!("Button clicked!");
                },
                "Click me!"
            }
        }
    }
}
