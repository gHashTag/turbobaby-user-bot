use dioxus::prelude::*;

#[component]
pub fn AppDebug() -> Element {
    rsx! {
        div { class: "app-container",
            style { "
                .app-container {{
                    padding: 20px;
                    font-size: 24px;
                    color: #fff;
                    text-align: center;
                    margin-top: 100px;
                }}
            " },
            h1 { "🌿 Woody Weed Bot" },
            p { "Dioxus Debug Version" },
            p { "If you see this, the app is working!" },
            button {
                style: "padding: 12px 24px; background: #39ff14; color: #000; border: none; border-radius: 8px; cursor: pointer; font-size: 12px;",
                onclick: move |_| {
                    println!("Button clicked!");
                },
                "Test Button"
            }
        }
    }
}
