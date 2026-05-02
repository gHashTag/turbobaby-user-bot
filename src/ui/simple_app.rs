use dioxus::prelude::*;

#[component]
pub fn SimpleApp() -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: column; align-items: center; justify-content: center; min-height: 100vh; background: #0f0f1a; color: #fff; padding: 20px;",
            h1 { style: "color: #39ff14; font-size: 60px; margin-bottom: 30px;", "🌿 TEST" }
            p { style: "font-size: 28px; margin-bottom: 40px;", "Если видите это - работает!" }
            div {
                style: "background: #1a1a2e; padding: 30px; border-radius: 15px; border: 3px solid #39ff14;",
                div { style: "font-size: 24px; margin: 15px 0;", "✅ HTML загружен" }
                div { style: "font-size: 24px; margin: 15px 0;", "✅ CSS работает" }
                div { style: "font-size: 24px; margin: 15px 0;", "✅ WASM загружен" }
                div { style: "font-size: 24px; margin: 15px 0;", "✅ Dioxus рендерит" }
            }
        }
    }
}
