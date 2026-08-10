// Garden Screen — Plant growing + Woody Catch game
use crate::trios::i18n::{
    t, T_GARDEN_INVITE_ACCEPT, T_GARDEN_INVITE_BODY, T_GARDEN_INVITE_SKIP, T_GARDEN_INVITE_TITLE,
    T_GARDEN_ONBOARD_CTA, T_GARDEN_ONBOARD_STEP1, T_GARDEN_ONBOARD_STEP2, T_GARDEN_ONBOARD_STEP3,
    T_GARDEN_ONBOARD_TITLE, T_GARDEN_TAB_GAME, T_GARDEN_TAB_GARDEN,
};
use crate::ui::api::context::api_base_url;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::lazy_screen::LazyScreen;
use crate::ui::game::Garden;
use crate::ui::game::WoodyCatch;
use crate::ui::lang;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};
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
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_telegram_init_data();
    let pending_invite = use_context::<Signal<Option<(i64, String)>>>();
    let mut show_invite_modal = use_signal(|| pending_invite.read().is_some());

    {
        let show_game = show_game;
        use_effect(move || {
            save_garden_tab(*show_game.read());
        });
    }

    // Cycle #19: garden invite deep-link landed here. Show a one-time welcome
    // modal that records the referral when the user accepts.
    {
        let pending = pending_invite.clone();
        use_effect(move || {
            if pending.read().is_some() {
                show_invite_modal.set(true);
            }
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

    let invite_modal = {
        if let Some((referrer_id, source)) = pending_invite.read().clone() {
            let init = init_data.clone();
            let mut pending = pending_invite.clone();
            let mut modal = show_invite_modal.clone();
            rsx! {
                div {
                    style: "position:fixed;inset:0;z-index:950;background:rgba(15,15,26,0.92);display:flex;align-items:center;justify-content:center;padding:20px;",
                    onclick: move |_| {
                        modal.set(false);
                        pending.set(None);
                    },
                    div {
                        style: "background:#1a1a2e;border:4px solid #39ff14;border-radius:12px;padding:20px;max-width:320px;width:100%;box-shadow:4px 4px 0 #000;text-align:center;",
                        onclick: move |e: Event<MouseData>| e.stop_propagation(),
                        div { style: "font-size:40px;margin-bottom:12px;", "🌱🤝🌱" }
                        h2 { style: "font-size:18px;font-weight:800;color:#39ff14;text-shadow:2px 2px 0 #000;margin:0 0 12px;", {t(lang, T_GARDEN_INVITE_TITLE)} }
                        p { style: "font-size:13px;color:#e8e8e8;line-height:1.5;margin:6px 0;", {t(lang, T_GARDEN_INVITE_BODY)} }
                        div { style: "display:flex;gap:10px;justify-content:center;margin-top:16px;",
                            button {
                                style: "padding:10px 16px;background:transparent;color:#8b8b9e;border:2px solid #2a2a4a;border-radius:8px;font-size:12px;font-weight:700;cursor:pointer;",
                                onclick: move |_| {
                                    modal.set(false);
                                    pending.set(None);
                                },
                                {t(lang, T_GARDEN_INVITE_SKIP)}
                            }
                            button {
                                style: "padding:10px 16px;background:#39ff14;color:#000;border:3px solid #2d9e0f;border-radius:8px;font-size:12px;font-weight:700;cursor:pointer;box-shadow:2px 2px 0 #000;",
                                onclick: move |_| {
                                    let tid = telegram_id;
                                    let init = init.clone();
                                    let source = source.clone();
                                    let mut pending = pending.clone();
                                    let mut modal = modal.clone();
                                    spawn(async move {
                                        let base = api_base_url();
                                        let url = format!("{}/api/referrals/me/{}/garden-invite", base, tid);
                                        let body = serde_json::json!({
                                            "referrer_id": referrer_id,
                                            "source": source,
                                        }).to_string();
                                        if let Ok(_text) = crate::ui::api::http::post_json_authed(&url, &init, &body
                                        ).await {
                                            crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                                        }
                                        pending.set(None);
                                        modal.set(false);
                                    });
                                },
                                {t(lang, T_GARDEN_INVITE_ACCEPT)}
                            }
                        }
                    }
                }
            }
        } else {
            rsx! {}
        }
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

            // Cycle #19: garden invite welcome modal.
            {invite_modal}

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
