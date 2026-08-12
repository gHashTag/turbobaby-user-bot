// Garden Screen — plant growing plus both games, so everything playable
// sits behind one tab bar instead of being scattered across the home grid.
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
use crate::ui::screens::skate_screen::SkateGame;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};
use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
use gloo_storage::{LocalStorage, Storage};

const GARDEN_TAB_KEY: &str = "wwb_garden_tab";
const GARDEN_ONBOARDED_KEY: &str = "wwb_garden_onboarded";

/// Which of the three tabs is showing: 0 garden, 1 Woody Catch, 2 Woody Skate.
/// A separate key from `GARDEN_TAB_KEY`, which held a bool — reading a bool
/// back as a number fails, and falling back to the garden tab once after the
/// update is nicer than a broken read on every visit.
const GARDEN_TAB_V2_KEY: &str = "wwb_garden_tab_v2";

const TAB_GARDEN: u8 = 0;
const TAB_CATCH: u8 = 1;
const TAB_SKATE: u8 = 2;

#[cfg(target_arch = "wasm32")]
fn load_garden_tab() -> u8 {
    LocalStorage::get(GARDEN_TAB_V2_KEY).unwrap_or(TAB_GARDEN)
}

#[cfg(not(target_arch = "wasm32"))]
fn load_garden_tab() -> u8 {
    TAB_GARDEN
}

#[cfg(target_arch = "wasm32")]
fn save_garden_tab(tab: u8) {
    let _ = LocalStorage::set(GARDEN_TAB_V2_KEY, tab);
}

#[cfg(not(target_arch = "wasm32"))]
fn save_garden_tab(_tab: u8) {}

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
    let mut tab = use_signal(load_garden_tab);
    let mut onboarded = use_signal(load_onboarded);
    let cart_count = 0u32;
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_telegram_init_data();
    let pending_invite = use_context::<Signal<Option<(i64, String)>>>();
    let mut show_invite_modal = use_signal(|| pending_invite.read().is_some());

    {
        let tab = tab;
        use_effect(move || {
            save_garden_tab(*tab.read());
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

    let current = *tab.read();

    // Three tabs no longer fit at the old padding on a 375px screen, so the
    // buttons are a little tighter than the two-tab version was.
    let tab_style = |selected: bool| {
        if selected {
            "padding: 10px 12px; background: #39ff14; color: #000; border: 4px solid #2d9e0f; font-size: 13px; font-weight: 700; cursor: pointer; border-radius: 20px; white-space: nowrap;"
        } else {
            "padding: 10px 12px; background: rgba(22,33,62,0.15); color: #8b8b9e; border: 4px solid #2a2a4a; font-size: 13px; font-weight: 700; cursor: pointer; border-radius: 20px; white-space: nowrap;"
        }
    };
    let garden_style = tab_style(current == TAB_GARDEN);
    let game_style = tab_style(current == TAB_CATCH);
    let skate_style = tab_style(current == TAB_SKATE);
    let skate_label = if lang == crate::trios::core::Lang::English {
        "🛹 Skate"
    } else {
        "🛹 Скейт"
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
        div { style: "min-height: 100vh; background: #0f0f1a; color: #e8e8e8; padding-bottom: calc(96px + env(safe-area-inset-bottom));",

            // Garden and both games side by side, so there is one place to
            // look for anything playable.
            div { style: "display: flex; gap: 6px; padding: 12px 12px; justify-content: center;",
                button {
                    style: "{garden_style}",
                    onclick: move |_| tab.set(TAB_GARDEN),
                    "🌱 {t(lang, T_GARDEN_TAB_GARDEN)}"
                }
                button {
                    style: "{game_style}",
                    onclick: move |_| tab.set(TAB_CATCH),
                    "🎮 {t(lang, T_GARDEN_TAB_GAME)}"
                }
                button {
                    style: "{skate_style}",
                    onclick: move |_| tab.set(TAB_SKATE),
                    "{skate_label}"
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
            if current == TAB_GARDEN {
                Garden {}
            } else if current == TAB_CATCH {
                LazyScreen { heavy: true, WoodyCatch {} }
            } else {
                LazyScreen { heavy: true, SkateGame {} }
            }
        }

        BottomNav { cart_count }
    }
}
