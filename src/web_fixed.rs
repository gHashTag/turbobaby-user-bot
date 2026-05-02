use dioxus::prelude::*;

fn main() {
    dioxus_web::launch::launch(App, vec![], vec![]);
}

#[component]
fn App() -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: column; align-items: center; justify-content: center; min-height: 100vh; background: #0f0f1a; color: #fff; padding: 20px;",
            h1 { style: "color: #39ff14; font-size: 50px;", "🌿 TEST PAGE" }
            p { style: "font-size: 24px; margin-top: 30px;", "Если видите это - работает!" }
        }
    }
}
