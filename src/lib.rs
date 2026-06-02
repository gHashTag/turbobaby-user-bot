// ─── WASM build (Dioxus + web_sys + Telegram MiniApp init) ──────────
//
// Cycle #162: lib.rs split into wasm + backend halves. The wasm half is
// the original `#![cfg(target_arch = "wasm32")]`-gated content from
// before this cycle. The backend half exposes the same modules
// `src/main.rs` already declares, so `tests/*.rs` integration tests can
// reach `woody_weed_bot::api::router`, `woody_weed_bot::AppState`, etc.
// Both halves coexist by `cfg`, so the WASM `cdylib` build pulls only
// the WASM tree and the native `rlib` build pulls only the backend tree.

// ─── Shared (compiles for both wasm and backend builds) ─────────────

pub mod trios;

// ─── WASM half ──────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
pub mod ui;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run() {
    web_sys::console::log_1(&"[WASM] Step 1: run() called".into());
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
    web_sys::console::log_1(&"[WASM] Step 2: panic hook set".into());

    // Cycle #73 + this commit: pick the rendering Lang once at
    // startup from:
    //   1. ?lang=xx URL override (always wins)
    //   2. `localStorage["wwb_lang"]` — user's prior manual choice
    //      from the profile-screen switcher (cycle #74+).
    //   3. Telegram WebApp `user.language_code` (passive device locale).
    //   4. Russian default.
    // Storage between (1) and (3) means a user who picked EN last
    // session won't be reset to Telegram's RU/EN every reload.
    // `current_lang()` (inside Dioxus) reads back from a GlobalSignal
    // that subscribes the calling component, so the profile-screen
    // switcher's `set_app_lang(new)` re-renders every screen.
    let url_query: Option<String> = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .filter(|s| !s.is_empty());
    let tg_lang_code: Option<String> = crate::ui::telegram::TelegramApp::init().get_language_code();
    let resolved = if url_query.is_some() {
        // URL still wins outright — let pick_lang do its thing.
        crate::trios::core::pick_lang(url_query.as_deref(), tg_lang_code.as_deref())
    } else if let Some(stored) = crate::ui::lang::read_stored_lang() {
        stored
    } else {
        crate::trios::core::pick_lang(None, tg_lang_code.as_deref())
    };
    crate::ui::lang::init_app_lang(resolved);
    web_sys::console::log_1(&format!("[WASM] Step 3: lang resolved to {}", resolved).into());

    web_sys::console::log_1(&"[WASM] Step 4: launching Dioxus App".into());
    dioxus::launch(crate::ui::app::App);
    web_sys::console::log_1(&"[WASM] Step 5: App launched".into());
}

// ─── Backend half ───────────────────────────────────────────────────
//
// The `cfg` here matches the gates in `src/main.rs`. Both lib and bin
// declare the same module paths so source files under `src/api/`,
// `src/db/`, etc. compile into both crates from their crate-relative
// `use crate::*` imports. Integration tests link against the lib
// (rlib) and reach the backend through these declarations.

#[cfg(all(not(target_arch = "wasm32"), feature = "backend"))]
pub mod ai;
#[cfg(all(not(target_arch = "wasm32"), feature = "backend"))]
pub mod api;
#[cfg(all(not(target_arch = "wasm32"), feature = "backend"))]
pub mod bot;
#[cfg(all(not(target_arch = "wasm32"), feature = "backend"))]
pub mod config;
#[cfg(all(not(target_arch = "wasm32"), feature = "backend"))]
pub mod db;
#[cfg(all(not(target_arch = "wasm32"), feature = "backend"))]
pub mod locales;
#[cfg(all(not(target_arch = "wasm32"), feature = "backend"))]
pub mod metrics;
#[cfg(all(not(target_arch = "wasm32"), feature = "backend"))]
pub mod notify;
#[cfg(all(not(target_arch = "wasm32"), feature = "backend"))]
pub mod s3;
#[cfg(all(not(target_arch = "wasm32"), feature = "backend"))]
pub mod util;

/// Application shared state — the type plumbed through every Axum
/// handler. Lives in lib.rs (cycle #162) so integration tests can
/// construct one.
#[cfg(all(not(target_arch = "wasm32"), feature = "backend"))]
#[derive(Clone)]
pub struct AppState {
    pub db: std::sync::Arc<crate::db::Database>,
    pub config: std::sync::Arc<crate::config::Config>,
    pub bot: std::sync::Arc<teloxide::Bot>,
    pub cache: std::sync::Arc<crate::api::cache::ETagCache>,
}
