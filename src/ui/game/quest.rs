// Quest screen — location-quest UI wired up to backend `/api/quest/scan`.
use crate::trios::i18n::{
    t, T_CHECKIN_SUCCESS, T_PURCHASE_MIN, T_QUEST_COMPLETE, T_REWARD, T_REWARD_CLAIM, T_SCAN_QR,
    T_SCAN_QR_DESC, T_STATUS_ACTIVE, T_STATUS_COMPLETED, T_STATUS_LOCKED, T_SUBTITLE, T_TITLE,
};
use crate::trios::quest::get_checkpoints;
use crate::ui::api::context::api_base_url;
use crate::ui::components::ErrorBanner;
use crate::ui::telegram::use_telegram_init_data;
use dioxus::prelude::*;
use js_sys::eval;
use serde_json::json;
use web_sys::window;

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
    let scan_result = use_signal(String::new);
    let loading = use_signal(|| false);
    let checkpoints = get_checkpoints();
    let init_data = use_telegram_init_data();

    let scan_qr = move |_| {
        // Capture signals into the spawned task.
        let mut scanning_c = scanning;
        let mut scan_result_c = scan_result;
        let mut current_checkpoint_c = current_checkpoint;
        let init_data = init_data.clone();
        spawn(async move {
            if window().is_none() {
                return;
            }

            // 1. Reset shared bucket + register a one-shot qrTextReceived
            //    listener that drops the scanned text into `window.__woody_qr`
            //    and closes the popup. We unbind the listener immediately so
            //    consecutive scans don't double-fire.
            let _ = eval(
                r#"
                window.__woody_qr = '';
                if (window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.showScanQrPopup) {
                    var tg = window.Telegram.WebApp;
                    var handler = function(event) {
                        var text = (event && (event.data || event.text)) || (typeof event === 'string' ? event : '');
                        if (text && typeof text === 'string') {
                            window.__woody_qr = text;
                            try { tg.closeScanQrPopup(); } catch (e) {}
                            try { tg.offEvent('qrTextReceived', handler); } catch (e) {}
                        }
                    };
                    tg.onEvent('qrTextReceived', handler);
                    tg.showScanQrPopup({text: 'Scan location QR'});
                }
            "#,
            );
            scanning_c.set(true);
            scan_result_c.set(String::new());

            // 2. Poll `window.__woody_qr` for up to ~60 s (120 × 500 ms).
            //    Telegram's popup itself caps interaction time; if the user
            //    bails the bucket stays empty and we exit cleanly.
            let mut token: Option<String> = None;
            for _ in 0..120 {
                gloo_timers::future::TimeoutFuture::new(500).await;
                if let Ok(v) = eval("window.__woody_qr || ''") {
                    if let Some(s) = v.as_string() {
                        if !s.is_empty() {
                            token = Some(s);
                            break;
                        }
                    }
                }
                if !*scanning_c.read() {
                    break;
                }
            }
            scanning_c.set(false);

            // 3. POST to backend and progress the checkpoint on success.
            let Some(token) = token else {
                return;
            };
            // Cap inbound QR length to mirror backend's validate (`> 200`).
            let token = if token.len() > 200 {
                token.chars().take(200).collect()
            } else {
                token
            };
            let url = format!("{}/api/quest/scan", api_base_url());
            let body = json!({ "qr_token": token }).to_string();
            match crate::ui::api::http::post_json_authed(&url, &init_data, &body).await {
                Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(val)
                        if val
                            .get("success")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false) =>
                    {
                        let cur = *current_checkpoint_c.read();
                        current_checkpoint_c.set(cur.saturating_add(1));
                        scan_result_c.set("ok".into());
                    }
                    Ok(_) => scan_result_c.set("Неверный QR".into()),
                    Err(_) => scan_result_c.set("Bad response".into()),
                },
                Err(e) => {
                    // Backend returns 404 if location not found, 401 on bad init data.
                    scan_result_c.set(format!(
                        "Ошибка: {}",
                        e.chars().take(60).collect::<String>()
                    ));
                }
            }
        });
    };

    let cur_cp = *current_checkpoint.read();
    let is_loading = *loading.read();
    let is_scanning = *scanning.read();
    let scan_msg = scan_result.read().clone();
    let is_complete = cur_cp >= checkpoints.len() as u8;

    let quest_title = t(crate::ui::lang::current_lang(), T_TITLE).to_string();
    let quest_subtitle = t(crate::ui::lang::current_lang(), T_SUBTITLE).to_string();
    let scan_qr_text = t(crate::ui::lang::current_lang(), T_SCAN_QR).to_string();
    let scan_desc = t(crate::ui::lang::current_lang(), T_SCAN_QR_DESC).to_string();

    let bg = "#0f0f1a";
    let bg_card = "#1a1a2e";
    let cyan = "#00e5ff";
    let scan_margin = if is_complete {
        "0".to_string()
    } else {
        "20".to_string()
    };

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
                        "{t(crate::ui::lang::current_lang(), T_QUEST_COMPLETE)}"
                    }
                    div { style: "font-size: 12px; color: #e8e8e8; margin-bottom: 4px;",
                        "{t(crate::ui::lang::current_lang(), T_REWARD_CLAIM)}"
                    }
                    div { style: "font-size: 16px; color: #39ff14;",
                        "{t(crate::ui::lang::current_lang(), T_REWARD)}"
                    }
                }
            }

            if scan_msg == "ok" {
                div { style: "
                    max-width: 380px; margin: 0 auto 12px;
                    background: rgba(57,255,20,0.1);
                    border: 2px solid #39ff14;
                    border-radius: 8px;
                    padding: 6px 10px; font-size: 18px; color: #39ff14; text-align: center;
                ",
                    "{t(crate::ui::lang::current_lang(), T_CHECKIN_SUCCESS)}"
                }
            } else if !scan_msg.is_empty() {
                ErrorBanner {
                    message: scan_msg.clone(),
                    margin: "0 auto 12px".to_string(),
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

                        let title = cp.title(crate::ui::lang::current_lang()).to_string();
                        let emoji = checkpoint_emoji(cp.id);

                        let (border_color, status_text, status_color, status_bg, opacity) = if is_done {
                            ("#39ff14", t(crate::ui::lang::current_lang(), T_STATUS_COMPLETED), "#39ff14", "rgba(57,255,20,0.2)", "1")
                        } else if is_current {
                            (cyan, t(crate::ui::lang::current_lang(), T_STATUS_ACTIVE), cyan, "rgba(0,229,255,0.2)", "1")
                        } else {
                            ("rgba(255,255,255,0.1)", t(crate::ui::lang::current_lang(), T_STATUS_LOCKED), "#666", "rgba(255,255,255,0.05)", "0.5")
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
                        "{t(crate::ui::lang::current_lang(), T_PURCHASE_MIN)}"
                    }
                }
            }
        }
    }
}
