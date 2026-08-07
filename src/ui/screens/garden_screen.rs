// Garden Screen — Plant growing + Woody Catch game
use crate::trios::i18n::{
    t, T_GARDEN_ONBOARD_CTA, T_GARDEN_ONBOARD_STEP1, T_GARDEN_ONBOARD_STEP2, T_GARDEN_ONBOARD_STEP3,
    T_GARDEN_ONBOARD_TITLE, T_GARDEN_TAB_GAME, T_GARDEN_TAB_GARDEN,
};
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::lazy_screen::LazyScreen;
use crate::ui::game::Garden;
use crate::ui::game::WoodyCatch;
use crate::ui::lang;
use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
use gloo_storage::{LocalStorage, Storage};

const GARDEN_TAB_KEY: &str = "wwb_garden_tab";
const GARDEN_ONBOARDED_KEY: &str = "wwb_garden_onboarded";

#[cfg(target_arch = "wasm32")]
fn load_garden_tab() -> bool {
    LocalStorage::get(GARDEN_TAB_KEY).unwrap_or(false)
}

#[cfg(not(target_arch = "wasm32"))]
fn load_garden_tab() -> bool {
    false
}

#[cfg(target_arch = "wasm32")]
fn save_garden_tab(show_game: bool) {
    let _ = LocalStorage::set(GARDEN_TAB_KEY, show_game);
}

#[cfg(not(target_arch = "wasm32"))]
fn save_garden_tab(_show_game: bool) {}

#[cfg(target_arch = "wasm32")]
fn load_onboarded() -> bool {
    LocalStorage::get(GARDEN_ONBOARDED_KEY).unwrap_or(false)
}

#[cfg(not(target_arch = "wasm32"))]
fn load_onboarded() -> bool {
    true
}

#[cfg(target_arch = "wasm32")]
fn save_onboarded(value: bool) {
    let _ = LocalStorage::set(GARDEN_ONBOARDED_KEY, value);
}

#[cfg(not(target_arch = "wasm32"))]
fn save_onboarded(_value: bool) {}

#[component]
pub fn GardenScreen() -> Element {
    let lang = lang::current_lang();
    let mut show_game = use_signal(load_garden_tab);
    let mut onboarded = use_signal(load_onboarded);
    let cart_count = 0u32;

    {
        let show_game = show_game;
        use_effect(move || {
            save_garden_tab(*show_game.read());
        });
    }

    let is_game = *show_game.read();

    let garden_style = if is_game {
        "padding: 10px 16px; background: rgba(22,33,62,0.15); color: #8b8b9e; border: 4px solid #2a2a4a; font-size: 13px; font-weight: 700; cursor: pointer; border-radius: 20px;"
    } else {
        "padding: 10px 16px; background: #39ff14; color: #000; border: 4px solid #2d9e0f; font-size: 13px; font-weight: 700; cursor: pointer; border-radius: 20px;"
    };

    let game_style = if !is_game {
        "padding: 10px 16px; background: rgba(22,33,62,0.15); color: #8b8b9e; border: 4px solid #2a2a4a; font-size: 13px; font-weight: 700; cursor: pointer; border-radius: 20px;"
    } else {
        "padding: 10px 16px; background: #39ff14; color: #000; border: 4px solid #2d9e0f; font-size: 13px; font-weight: 700; cursor: pointer; border-radius: 20px;"
    };

    rsx! {
        div { style: "min-height: 100vh; background: #0f0f1a; color: #e8e8e8; padding-bottom: 80px;",

            // Toggle between Garden and Game
            div { style: "display: flex; gap: 8px; padding: 12px 16px; justify-content: center;",
                button {
                    style: "{garden_style}",
                    onclick: move |_| show_game.set(false),
                    "🌱 {t(lang, T_GARDEN_TAB_GARDEN)}"
                }
                button {
                    style: "{game_style}",
                    onclick: move |_| show_game.set(true),
                    "🎮 {t(lang, T_GARDEN_TAB_GAME)}"
                }
            }

            // First-run onboarding overlay
            if !*onboarded.read() {
                div {
                    style: "position:fixed;inset:0;z-index:900;background:rgba(15,15,26,0.92);display:flex;align-items:center;justify-content:center;padding:20px;",
                    onclick: move |_| {
                        onboarded.set(true);
                        save_onboarded(true);
                    },
                    div {
                        style: "background:#1a1a2e;border:4px solid #39ff14;border-radius:12px;padding:20px;max-width:320px;width:100%;box-shadow:4px 4px 0 #000;text-align:center;",
                        onclick: move |e: Event<MouseData>| e.stop_propagation(),
                        h2 { style: "font-size:18px;font-weight:800;color:#39ff14;text-shadow:2px 2px 0 #000;margin:0 0 12px;", "{t(lang, T_GARDEN_ONBOARD_TITLE)}" }
                        p { style: "font-size:13px;color:#e8e8e8;line-height:1.5;margin:6px 0;", "{t(lang, T_GARDEN_ONBOARD_STEP1)}" }
                        p { style: "font-size:13px;color:#e8e8e8;line-height:1.5;margin:6px 0;", "{t(lang, T_GARDEN_ONBOARD_STEP2)}" }
                        p { style: "font-size:13px;color:#e8e8e8;line-height:1.5;margin:6px 0;", "{t(lang, T_GARDEN_ONBOARD_STEP3)}" }
                        button {
                            style: "margin-top:16px;width:100%;padding:12px;background:#39ff14;color:#000;border:3px solid #2d9e0f;font-size:14px;font-weight:700;cursor:pointer;box-shadow:2px 2px 0 #000;",
                            onclick: move |_| {
                                onboarded.set(true);
                                save_onboarded(true);
                            },
                            "{t(lang, T_GARDEN_ONBOARD_CTA)}"
                        }
                    }
                }
            }

            // Show content based on selection
            if !is_game {
                Garden {}
            } else {
                LazyScreen { heavy: true, WoodyCatch {} }
            }
        }

        BottomNav { cart_count }
    }
}
