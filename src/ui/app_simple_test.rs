use dioxus::prelude::*;

#[component]
pub fn AppSimple() -> Element {
    rsx! {
        div { class: "container",
            h1 { "🌿 Woody Weed Bot Test" }
            p { "Simple test page" }
            button { class: "btn",
                onclick: move |_| {
                    println!("Button clicked!");
                },
                "Click me!"
            }
        }
        style { "
            .container {{
                max-width: 800px;
                margin: 40px auto;
                padding: 20px;
                background: #1a1a2e;
                border-radius: 12px;
                font-family: system-ui, sans-serif;
            }}
            h1 {{
                color: #e8e8e8;
                margin-bottom: 16px;
            }}
            p {{
                color: #a0a0b0;
                margin-bottom: 20px;
            }}
            .btn {{
                padding: 12px 24px;
                background: #39ff14;
                color: #000;
                border: none;
                border-radius: 8px;
                cursor: pointer;
                font-size: 12px;
            }}
            .btn:hover {{
                background: #00e5ff;
            }}
        " }
    }
}
