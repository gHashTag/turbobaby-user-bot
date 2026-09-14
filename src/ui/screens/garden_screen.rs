// Garden Screen — plant growing plus both games, so everything playable
// sits behind one tab bar instead of being scattered across the home grid.
use crate::trios::i18n::{
    t, T_GARDEN_INVITE_ACCEPT, T_GARDEN_INVITE_BODY, T_GARDEN_INVITE_SKIP, T_GARDEN_INVITE_TITLE,
    T_GARDEN_ONBOARD_CTA, T_GARDEN_ONBOARD_STEP1, T_GARDEN_ONBOARD_STEP2, T_GARDEN_ONBOARD_STEP3,
    T_GARDEN_ONBOARD_TITLE, T_GARDEN_TAB_GARDEN,
};
use crate::ui::api::context::api_base_url;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::lazy_screen::LazyScreen;
use crate::ui::game::Garden;
use crate::ui::game::WoodyShop;
use crate::ui::lang;
use crate::ui::screens::ride_screen::RideGame;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};
use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
use gloo_storage::{LocalStorage, Storage};

const GARDEN_ONBOARDED_KEY: &str = "wwb_garden_onboarded";

/// Which of the three tabs is showing: 0 garden, 2 TurboBaby Ride, 3 shop.
/// Value 2 deliberately survives the Skate-to-Ride cutover so an existing
/// localStorage selection opens the replacement game rather than another tab.
const GARDEN_TAB_V2_KEY: &str = "wwb_garden_tab_v2";

const TAB_GARDEN: u8 = 0;
// 1 was Woody Catch, removed. The remaining numbers keep their values rather
// than being compacted: a customer with `1` in local storage would otherwise
// silently reopen on Ride, and `load_garden_tab` sends them to the garden.
const TAB_RIDE: u8 = 2;
const TAB_SHOP: u8 = 3;

#[cfg(target_arch = "wasm32")]
fn load_garden_tab() -> u8 {
    let stored: u8 = LocalStorage::get(GARDEN_TAB_V2_KEY).unwrap_or(TAB_GARDEN);
    // Anything that is not a tab any more — 1, which was Woody Catch, or a
    // value from a future build — opens the garden rather than nothing at all.
    match stored {
        TAB_GARDEN | TAB_RIDE | TAB_SHOP => stored,
        _ => TAB_GARDEN,
    }
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
        let pending = pending_invite;
        use_effect(move || {
            if pending.read().is_some() {
                show_invite_modal.set(true);
            }
        });
    }

    let current = *tab.read();

    // Four labels need 383px at the two-tab sizing and the narrowest phone
    // gives 375, so the buttons are tighter. The row still scrolls sideways as
    // a fallback for longer translations.
    let tab_style = |selected: bool| {
        if selected {
            "padding: 9px 9px; background: #39ff14; color: #000; border: 3px solid #2d9e0f; font-size: 12px; font-weight: 700; cursor: pointer; border-radius: 18px; white-space: nowrap;"
        } else {
            "padding: 9px 9px; background: rgba(22,33,62,0.15); color: #8b8b9e; border: 3px solid #2a2a4a; font-size: 12px; font-weight: 700; cursor: pointer; border-radius: 18px; white-space: nowrap;"
        }
    };
    let garden_style = tab_style(current == TAB_GARDEN);
    let ride_style = tab_style(current == TAB_RIDE);
    let shop_style = tab_style(current == TAB_SHOP);
    let english = lang == crate::trios::core::Lang::English;
    let ride_label = if english {
        "🏍️ Ride"
    } else {
        "🏍️ Заезд"
    };
    // The tycoon game used to be the bottom nav's "Игра", which collided with
    // this tab bar's own "Игра" (Woody Catch). Naming it after the shop it
    // simulates is the only way both can sit in one row.
    let shop_label = if english {
        "🏪 Shop"
    } else {
        "🏪 Магазин"
    };

    let invite_modal = {
        if let Some((referrer_id, source)) = pending_invite.read().clone() {
            let init = init_data.clone();
            let mut pending = pending_invite;
            let mut modal = show_invite_modal;
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
                                    let mut pending = pending;
                                    let mut modal = modal;
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

            // Garden and the games side by side, so there is one place to look
            // for anything playable. Still scrolls sideways rather than
            // wrapping: a wrapped second row pushes the game down.
            div { style: "display: flex; gap: 6px; padding: 12px 12px; overflow-x: auto; -webkit-overflow-scrolling: touch; scrollbar-width: none;",
                button {
                    style: "{garden_style}",
                    onclick: move |_| tab.set(TAB_GARDEN),
                    "🌱 {t(lang, T_GARDEN_TAB_GARDEN)}"
                }
                button {
                    style: "{ride_style}",
                    onclick: move |_| tab.set(TAB_RIDE),
                    "{ride_label}"
                }
                button {
                    style: "{shop_style}",
                    onclick: move |_| tab.set(TAB_SHOP),
                    "{shop_label}"
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
            } else if current == TAB_RIDE {
                LazyScreen { heavy: true, RideGame {} }
            } else {
                LazyScreen { heavy: true, WoodyShop {} }
            }
        }

        BottomNav { cart_count }
    }
}
