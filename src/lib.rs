// Library for WASM compilation
#![cfg(target_arch = "wasm32")]

pub mod trios;
pub mod ui;

use crate::ui::app::App;

#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run() {
    web_sys::console::log_1(&"[WASM] Step 1: run() called".into());
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
                            let safe_msg = msg
                                .replace('&', "&amp;")
                                .replace('<', "&lt;")
                                .replace('>', "&gt;");
                            div.set_inner_html(&format!("<h1 style='color:#ff4757'>🚨 PANIC</h1><pre style='font-size:14px'>{}</pre>", safe_msg));
                            let _ = body.append_child(&div);
                        }
                    }
                }
            }
        }));
    }
    web_sys::console::log_1(&"[WASM] Step 2: panic hook set".into());

    // Cycle #73: pick the rendering Lang once at startup from
    //   1. ?lang=xx URL override, 2. Telegram WebApp user.language_code,
    //   3. Russian default.
    // `current_lang()` reads back from this OnceLock anywhere in the UI
    // so individual components don't have to thread Lang through context.
    #[cfg(target_arch = "wasm32")]
    {
        let url_query: Option<String> = web_sys::window()
            .and_then(|w| w.location().search().ok())
            .filter(|s| !s.is_empty());
        let tg_lang_code: Option<String> =
            crate::ui::telegram::TelegramApp::init().get_language_code();
        let resolved = crate::trios::core::pick_lang(url_query.as_deref(), tg_lang_code.as_deref());
        crate::ui::lang::init_app_lang(resolved);
        web_sys::console::log_1(&format!("[WASM] Step 3: lang resolved to {}", resolved).into());
    }

    web_sys::console::log_1(&"[WASM] Step 4: launching Dioxus App".into());
    dioxus::launch(App);
    web_sys::console::log_1(&"[WASM] Step 5: App launched".into());
}
