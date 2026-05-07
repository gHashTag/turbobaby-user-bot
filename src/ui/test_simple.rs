// Simple test page - NO complexity, just text
use dioxus::prelude::*;

#[component]
pub fn SimpleTest() -> Element {
    rsx! {
        div {
            style: "padding: 40px; background: #1a1a2e; min-height: 100vh; color: #fff;",
            h1 {
                style: "color: #39ff14; font-size: 32px; margin-bottom: 20px;",
                "🌿 Woody Weed Bot"
            }
            p {
                style: "font-size: 14px; line-height: 1.6;",
                "Сайт работает! Это простая тестовая страница."
            }
            div {
                style: "background: #0f0f1a; padding: 20px; border-radius: 8px; margin-top: 20px;",
                h3 { style: "margin-bottom: 10px; color: #00e5ff;", "Статус системы:" }
                ul {
                    style: "margin-left: 20px;",
                    li { "✅ HTML загружен" }
                    li { "✅ CSS работает" }
                    li { "✅ Dioxus рендерит" }
                    li { "✅ WASM выполняется" }
                }
            }
            button {
                onclick: move |_| {
                    // Simple click handler
                },
                style: "background: #39ff14; color: #000; border: none; padding: 15px 30px; font-size: 12px; border-radius: 8px; cursor: pointer; margin-top: 30px;",
                "Нажми меня!"
            }
        }
    }
}
