// Simple app for testing
use dioxus::prelude::*;

#[component]
pub fn SimpleApp() -> Element {
    rsx! {
        div {
            style: "padding: 40px; background: #0f0f1a; min-height: 100vh; color: #fff;",
            h1 {
                style: "color: #39ff14; font-size: 48px; margin-bottom: 20px; text-align: center;",
                "🌿 Woody Weed Bot"
            }
            p {
                style: "font-size: 16px; line-height: 1.8; text-align: center; margin-bottom: 30px;",
                "Сайт работает! Это простая тестовая страница."
            }
            div {
                style: "background: #1a1a2e; padding: 25px; border-radius: 12px; margin: 20px auto; max-width: 400px; border: 2px solid #2a2a4a;",
                h3 { style: "margin-bottom: 15px; color: #00e5ff; text-align: center;", "✅ Статус системы:" }
                ul {
                    style: "margin-left: 30px; line-height: 2;",
                    li { "📄 HTML загружен" }
                    li { "🎨 CSS работает" }
                    li { "⚛️  Dioxus рендерит" }
                    li { "🔥 WASM выполняется" }
                    li { "📱 Telegram API готов" }
                }
            }
            div {
                style: "text-align: center; margin-top: 40px;",
                    button {
                        onclick: move |_| {
                            web_sys::window().unwrap().alert_with_message("Кнопка работает!").unwrap();
                        },
                        style: "background: #39ff14; color: #000; border: none; padding: 18px 40px; font-size: 14px; border-radius: 10px; cursor: pointer; font-weight: bold;",
                        "🎉 Нажми меня!"
                    }
            }
            p {
                style: "text-align: center; margin-top: 30px; color: #a0a0b0; font-size: 11px;",
                "Если вы это видите - сайт работает!"
            }
        }
    }
}
