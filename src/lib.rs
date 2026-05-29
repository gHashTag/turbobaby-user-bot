// Library for WASM compilation
#![cfg(target_arch = "wasm32")]

pub mod trios;
pub mod ui;

use crate::ui::app::App;

#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run() {
    console_error_panic_hook::set_once();
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(|info| {
            let msg = format!("PANIC: {}", info);
            web_sys::console::error_1(&msg.clone().into());
            if let Some(window) = web_sys::window() {
                if let Some(document) = window.document() {
                    if let Some(body) = document.body() {
                        if let Ok(div) = document.create_element("div") {
                            let _ = div.set_attribute("style", "position:fixed;inset:0;background:#000;color:#ff4757;padding:20px;font-family:monospace;white-space:pre-wrap;z-index:99999;overflow:auto;");
                            div.set_inner_html(&format!("<h1 style='color:#ff4757'>🚨 PANIC</h1><pre style='font-size:14px'>{}</pre>", msg));
                            let _ = body.append_child(&div);
                        }
                    }
                }
            }
        }));
    }
    dioxus::launch(App);
}
