// Quest screen — static location-quest UI (no backend quest/state endpoint yet).
use dioxus::prelude::*;
use web_sys::window;
use js_sys::eval;
use crate::trios::quest::get_checkpoints;
use crate::trios::core::Lang;
use crate::trios::i18n::{
    t, T_TITLE, T_SUBTITLE, T_SCAN_QR, T_SCAN_QR_DESC,
    T_CHECKIN_SUCCESS, T_QUEST_COMPLETE, T_REWARD_CLAIM, T_REWARD,
    T_PURCHASE_MIN, T_STATUS_LOCKED, T_STATUS_ACTIVE, T_STATUS_COMPLETED,
};

fn checkpoint_emoji(id: u8) -> &'static str {
    match id {
        1 => "🌊",
        2 => "⚓",
        3 => "🌴",
        4 => "🏴‍☠️",
        5 => "🔺",
        _ => "📍",
    }
}

#[component]
pub fn Quest() -> Element {
    let current_checkpoint = use_signal(|| 0u8);
    let scanning = use_signal(|| false);
    let scan_result = use_signal(|| String::new());
    let loading = use_signal(|| false);
    let checkpoints = get_checkpoints();

    let scan_qr = move |_| {
        let mut scanning_c = scanning.clone();
        let mut scan_result_c = scan_result.clone();
        spawn(async move {
            if let Some(_w) = window() {
                let _ = eval(r#"
                    if (window.Telegram?.WebApp?.showScanQrPopup) {
                        window.Telegram.WebApp.showScanQrPopup({text: 'Scan location QR'});
                    }
                "#);
                scanning_c.set(true);
                scan_result_c.set("Scan the QR code at the location".to_string());
            }
        });
    };

    let cur_cp = *current_checkpoint.read();
    let is_loading = *loading.read();
    let is_scanning = *scanning.read();
    let scan_msg = scan_result.read().clone();
    let is_complete = cur_cp >= checkpoints.len() as u8;

    let quest_title = t(Lang::Russian, T_TITLE).to_string();
    let quest_subtitle = t(Lang::Russian, T_SUBTITLE).to_string();
    let scan_qr_text = t(Lang::Russian, T_SCAN_QR).to_string();
    let scan_desc = t(Lang::Russian, T_SCAN_QR_DESC).to_string();

    let bg = "#0f0f1a";
    let bg_card = "#1a1a2e";
    let cyan = "#00e5ff";
    let scan_margin = if is_complete { "0".to_string() } else { "20".to_string() };

    rsx! {
        div { style: "min-height: 100vh; background: {bg}; color: #e8e8e8; font-family: 'Press Start 2P', monospace; padding-bottom: 80px;",

            div { style: "padding: 20px 16px 12px; text-align: center;",
                h1 { style: "font-size: 18px; color: {cyan}; text-shadow: 0 0 8px rgba(0,229,255,0.5);",
                    "🎯 {quest_title}"
                }
                p { style: "font-size: 11px; color: #8b8b9e; margin-top: 4px;",
                    "{quest_subtitle}"
                }
            }

            if is_complete {
                div { style: "
                    max-width: 380px; margin: 20px auto;
                    background: rgba(255,215,0,0.1);
                    border: 2px solid #ffd700;
                    border-radius: 16px;
                    padding: 24px; text-align: center;
                ",
                    div { style: "font-size: 48px; margin-bottom: 12px;", "🏆" }
                    div { style: "font-size: 14px; color: #ffd700; margin-bottom: 8px;",
                        "{t(Lang::Russian, T_QUEST_COMPLETE)}"
                    }
                    div { style: "font-size: 12px; color: #e8e8e8; margin-bottom: 4px;",
                        "{t(Lang::Russian, T_REWARD_CLAIM)}"
                    }
                    div { style: "font-size: 16px; color: #39ff14;",
                        "{t(Lang::Russian, T_REWARD)}"
                    }
                }
            }

            if !scan_msg.is_empty() {
                div { style: "
                    max-width: 380px; margin: 0 auto 12px;
                    background: rgba(57,255,20,0.1);
                    border: 2px solid #39ff14;
                    border-radius: 8px;
                    padding: 6px 10px; font-size: 18px; color: #39ff14; text-align: center;
                ",
                    "{t(Lang::Russian, T_CHECKIN_SUCCESS)}"
                }
            }

            div { style: "
                max-width: 380px; margin: {scan_margin}px auto;
                background: rgba(0,229,255,0.05);
                border: 2px dashed {cyan};
                border-radius: 16px;
                padding: 24px; text-align: center;
            ",
                div { style: "font-size: 48px; margin-bottom: 12px;", "📷" }
                div { style: "font-size: 14px; color: {cyan}; margin-bottom: 8px; text-shadow: 0 0 6px rgba(0,229,255,0.4);",
                    "{scan_qr_text}"
                }
                div { style: "font-size: 18px; color: #666;", "{scan_desc}" }
            }

            if is_loading {
                div { style: "text-align: center; padding: 40px;",
                    div { style: "font-size: 36px; animation: pulse-glow 2s infinite;", "🎯" }
                    p { style: "font-size: 12px; color: #8b8b9e; margin-top: 8px;", "Loading quest..." }
                }
            } else {
                div { style: "max-width: 380px; margin: 0 auto; padding: 0 16px;",
                    {checkpoints.iter().map(|cp| {
                        let cp_num = cp.id;
                        let is_done = cp_num <= cur_cp;
                        let is_current = cp_num == cur_cp + 1 && !is_complete;
                        let _is_locked = cp_num > cur_cp + 1;

                        let title = cp.title(Lang::Russian).to_string();
                        let emoji = checkpoint_emoji(cp.id);

                        let (border_color, status_text, status_color, status_bg, opacity) = if is_done {
                            ("#39ff14", t(Lang::Russian, T_STATUS_COMPLETED), "#39ff14", "rgba(57,255,20,0.2)", "1")
                        } else if is_current {
                            (cyan, t(Lang::Russian, T_STATUS_ACTIVE), cyan, "rgba(0,229,255,0.2)", "1")
                        } else {
                            ("rgba(255,255,255,0.1)", t(Lang::Russian, T_STATUS_LOCKED), "#666", "rgba(255,255,255,0.05)", "0.5")
                        };

                        let progress_pct = if is_done { "100%" } else if is_current { "50%" } else { "0%" };
                        let bar_bg = if is_done { "linear-gradient(90deg, #00e5ff, #39ff14)" } else if is_current { cyan } else { "#333" };

                        rsx! {
                            div {
                                key: "{cp_num}",
                                style: "
                                    background: {bg_card}; border: 2px solid {border_color};
                                    border-radius: 8px; padding: 16px; margin-bottom: 12px;
                                    display: flex; gap: 12px; align-items: center;
                                    opacity: {opacity};
                                ",
                                div { style: "font-size: 32px; min-width: 48px; text-align: center;", "{emoji}" }
                                div { style: "flex: 1;",
                                    div { style: "font-size: 14px; color: #e0e0e0; margin-bottom: 4px;", "{title}" }
                                    div { style: "font-size: 18px; color: #ffd700; margin-bottom: 6px;",
                                        "🎁 +{cp.reward_bat} BAT"
                                    }
                                    div { style: "height: 6px; background: rgba(0,0,0,0.4); border-radius: 3px; overflow: hidden;",
                                        div { style: "height: 100%; width: {progress_pct}; border-radius: 3px; background: {bar_bg};" }
                                    }
                                }
                                span { style: "
                                    font-size: 10px; padding: 4px 8px;
                                    border-radius: 8px; white-space: nowrap;
                                    background: {status_bg}; color: {status_color};
                                ", "{status_text}" }
                            }
                        }
                    })}
                }
            }

            if !is_complete && !is_loading {
                div { style: "max-width: 380px; margin: 16px auto; padding: 0 16px;",
                    button {
                        style: "
                            width: 100%; padding: 12px; border: none; border-radius: 8px;
                            font-family: 'Press Start 2P', monospace; font-size: 14px;
                            cursor: pointer; color: #0a0a0a;
                            background: linear-gradient(135deg, #00e5ff, #39ff14);
                        ",
                        disabled: is_scanning,
                        onclick: scan_qr,
                        if is_scanning { "⏳ Scanning..." } else { "📷 {scan_qr_text}" }
                    }
                    div { style: "font-size: 18px; color: #555; text-align: center; margin-top: 8px;",
                        "{t(Lang::Russian, T_PURCHASE_MIN)}"
                    }
                }
            }
        }
    }
}
