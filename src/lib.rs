// Library for WASM compilation
#![cfg(target_arch = "wasm32")]

pub mod trios;
pub mod ui;

use crate::ui::app::App;

#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run() {
    console_error_panic_hook::set_once();
    dioxus::launch::launch_cfg(App, dioxus::launch::Config::new());
}
