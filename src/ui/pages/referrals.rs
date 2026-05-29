use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::api::context::api_base_url;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct ReferralStats {
    pub total_invited: i64,
    pub confirmed: i64,
    pub pending: i64,
    pub total_bonus_earned: f64,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct ReferralMe {
    pub code: String,
    pub invite_link: String,
    pub stats: ReferralStats,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TopReferrer {
    pub telegram_id: i64,
    pub referral_count: i64,
    pub total_bonus_earned: f64,
}

fn open_telegram_link(url: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;
        if let Some(window) = web_sys::window() {
            if let Ok(tg) = js_sys::Reflect::get(&window, &JsValue::from_str("Telegram")) {
                if let Ok(webapp) = js_sys::Reflect::get(&tg, &JsValue::from_str("WebApp")) {
                    let func = js_sys::Reflect::get(&webapp, &JsValue::from_str("openTelegramLink"))
                        .ok()
                        .and_then(|f| f.dyn_into::<js_sys::Function>().ok());
                    if let Some(f) = func {
                        let _ = f.call1(&webapp, &JsValue::from_str(url));
                        return;
                    }
                }
            }
            let _ = window.open_with_url_and_target(url, "_blank");
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    { let _ = url; }
}

fn copy_to_clipboard(text: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let clipboard = window.navigator().clipboard();
            let _ = clipboard.write_text(text);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    { let _ = text; }
}

// Use the urlencoding crate (already in Cargo.toml) for correct URL encoding.
fn urlencoding_simple(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

#[component]
pub fn Referrals() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0i64);
    let referral_me = use_signal(|| ReferralMe::default());
    let leaderboard = use_signal(Vec::<TopReferrer>::new);
    let loading = use_signal(|| true);
    let mut copied = use_signal(|| false);
    let init_data = use_telegram_init_data();

    {
        let mut me_c = referral_me;
        let mut board_c = leaderboard;
        let mut loading_c = loading;
        let tid = telegram_id;
        let init = init_data.clone();
        use_hook(move || {
            if tid == 0 {
                me_c.set(ReferralMe {
                    code: "N/A".into(),
                    invite_link: format!("https://t.me/Woody_WeedPecker_bot?start=ref_NA"),
                    stats: ReferralStats::default(),
                });
                loading_c.set(false);
                return;
            }
            spawn(async move {
                let base = api_base_url();
                let client = reqwest::Client::new();

                if let Ok(resp) = client
                    .get(format!("{}/api/referrals/me/{}", base, tid))
                    .header("X-Telegram-Init-Data", &init)
                    .send().await
                {
                    if let Ok(text) = resp.text().await {
                        if let Ok(me) = serde_json::from_str::<ReferralMe>(&text) {
                            me_c.set(me);
                        }
                    }
                }

                if let Ok(resp) = client.get(format!("{}/api/referrals/leaderboard?limit=10", base)).send().await {
                    if let Ok(text) = resp.text().await {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(arr) = val.get("leaderboard").and_then(|v| v.as_array()) {
                                let items: Vec<TopReferrer> = arr
                                    .iter()
                                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                    .collect();
                                board_c.set(items);
                            }
                        }
                    }
                }

                loading_c.set(false);
            });
        });
    }

    let me = referral_me.read().clone();
    let link = me.invite_link.clone();
    let link_for_copy = link.clone();
    let stats = me.stats.clone();
    let board = leaderboard.read().clone();

    rsx! {
        div {
            style: "
                min-height: 100vh;
                background: #0f0f1a;
                color: #e8e8e8;
                padding-bottom: 80px;
            ",

            div {
                style: "padding: 20px 16px 12px; text-align: center;",
                h1 {
                    style: "font-size: 24px; font-weight: 800; color: #39ff14; text-shadow: 2px 2px 0 #000;",
                    "\u{1F381} Referral Program"
                }
                p {
                    style: "font-size: 15px; color: #888; margin-top: 6px;",
                    "Invite friends \u{2014} earn bonuses"
                }
            }

            if *loading.read() {
                div { style: "text-align: center; padding: 30px;",
                    span { style: "font-size: 15px; color: #39ff14;", "Loading..." }
                }
            } else {
                div {
                    style: "
                        max-width: 380px; margin: 0 auto 16px; padding: 20px 16px;
                        background: #16213e;
                        border: 4px solid #2a2a4a;
                        box-shadow: 4px 4px 0 #000;
                    ",
                    p {
                        style: "font-size: 13px; font-weight: 700; color: #888; margin-bottom: 8px;",
                        "YOUR REFERRAL LINK"
                    }
                    div {
                        style: "
                            background: rgba(0,0,0,0.4);
                            border: 4px solid #2a2a4a;
                            box-shadow: 3px 3px 0 #000;
                            padding: 10px 12px;
                            font-size: 15px;
                            color: #39ff14;
                            word-break: break-all;
                            margin-bottom: 12px;
                        ",
                        "{link}"
                    }
                    div {
                        style: "display: flex; gap: 8px;",

                        button {
                            style: "
                                flex: 1;
                                background: rgba(57,255,20,0.15);
                                border: 4px solid #2d9e0f;
                                box-shadow: 3px 3px 0 #000;
                                color: #39ff14;
                                font-family: inherit;
                                font-size: 14px;
                                font-weight: 700;
                                padding: 10px 8px;
                                cursor: pointer;
                            ",
                            onclick: move |_| {
                                copy_to_clipboard(&link_for_copy);
                                copied.set(true);
                            },
                            if *copied.read() { "\u{2705} Copied!" } else { "\u{1F4CB} Copy" }
                        }

                        {
                            let share_url = format!(
                                "https://t.me/share/url?url={}&text={}",
                                urlencoding_simple(&link),
                                urlencoding_simple("\u{1FAB5} Join Woody Weed and get bonuses!")
                            );
                            rsx! {
                                button {
                                    style: "
                                        flex: 1;
                                        background: rgba(57,255,20,0.15);
                                        border: 4px solid #2d9e0f;
                                        box-shadow: 3px 3px 0 #000;
                                        color: #39ff14;
                                        font-family: inherit;
                                        font-size: 14px;
                                        font-weight: 700;
                                        padding: 10px 8px;
                                        cursor: pointer;
                                    ",
                                    onclick: move |_| {
                                        open_telegram_link(&share_url);
                                    },
                                    "\u{1F4E4} Share"
                                }
                            }
                        }
                    }
                }

                div {
                    style: "
                        max-width: 380px; margin: 0 auto 16px; padding: 0 16px;
                        display: grid; grid-template-columns: repeat(2, 1fr); gap: 8px;
                    ",
                    {stat_card("\u{1F465}", "Invited", &stats.total_invited.to_string())}
                    {stat_card("\u{2705}", "Confirmed", &stats.confirmed.to_string())}
                    {stat_card("\u{23F3}", "Pending", &stats.pending.to_string())}
                    {stat_card("\u{1F4B0}", "Bonus", &format!("{:.0} \u{0E3F}", stats.total_bonus_earned))}
                }

                div {
                    style: "max-width: 380px; margin: 0 auto; padding: 0 16px;",

                    h2 {
                        style: "font-size: 13px; font-weight: 700; color: #ffd700; margin-bottom: 12px; text-align: center;",
                        "\u{1F3C6} Top Referrers"
                    }

                    if board.is_empty() {
                        div {
                            style: "text-align: center; color: #555; font-size: 15px; padding: 20px;",
                            "No data yet \u{2014} be the first!"
                        }
                    } else {
                        div {
                            style: "display: flex; flex-direction: column; gap: 6px;",
                            for (idx, entry) in board.iter().enumerate() {
                                {leaderboard_row(idx + 1, entry.telegram_id, entry.referral_count, entry.total_bonus_earned)}
                            }
                        }
                    }
                }
            }
        }
    }
}

fn stat_card(icon: &str, label: &str, value: &str) -> Element {
    let icon = icon.to_string();
    let label = label.to_string();
    let value = value.to_string();
    rsx! {
        div {
            style: "
                background: #16213e;
                border: 4px solid #2a2a4a;
                box-shadow: 4px 4px 0 #000;
                padding: 14px 12px;
                text-align: center;
            ",
            div { style: "font-size: 14px; margin-bottom: 6px;", "{icon}" }
            div { style: "font-size: 20px; font-weight: 800; color: #ffe600; margin-bottom: 4px;", "{value}" }
            div { style: "font-size: 15px; color: #666;", "{label}" }
        }
    }
}

fn leaderboard_row(rank: usize, telegram_id: i64, referral_count: i64, bonus: f64) -> Element {
    let medal = match rank {
        1 => "\u{1F947}",
        2 => "\u{1F948}",
        3 => "\u{1F949}",
        _ => "  ",
    };
    let rank_color = match rank {
        1 => "#ffd700",
        2 => "#c0c0c0",
        3 => "#cd7f32",
        _ => "#555",
    };
    let medal = medal.to_string();
    rsx! {
        div {
            style: "
                display: flex; align-items: center; gap: 10px;
                background: #16213e;
                border: 4px solid #2a2a4a;
                box-shadow: 4px 4px 0 #000;
                padding: 10px 12px;
            ",
            span { style: "font-size: 14px; width: 20px; flex-shrink: 0;", "{medal}" }
            span { style: "font-size: 15px; font-weight: 700; color: {rank_color};", "#{rank}" }
            div { style: "flex: 1;",
                div { style: "font-size: 15px; color: #e8e8e8;",
                    {format!("ID: \u{2026}{}", telegram_id % 10000)}
                }
                div { style: "font-size: 15px; color: #555; margin-top: 2px;",
                    {format!("{} invited \u{2022} {:.0} \u{0E3F} earned", referral_count, bonus)}
                }
            }
        }
    }
}
