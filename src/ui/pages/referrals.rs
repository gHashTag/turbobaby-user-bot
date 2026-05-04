// Referral Program Page
// Displays the user's referral code, invite link, stat cards, and top-10 leaderboard.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

// ──────────────────────────────────────────────────────────────────
// Data types (mirror of API response shapes)
// ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ReferralStats {
    pub total_invited: i64,
    pub confirmed: i64,
    pub pending: i64,
    pub total_bonus_earned: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ReferralMe {
    pub code: String,
    pub invite_link: String,
    pub stats: ReferralStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TopReferrer {
    pub telegram_id: i64,
    pub referral_count: i64,
    pub total_bonus_earned: f64,
}

// ──────────────────────────────────────────────────────────────────
// Helper: open Telegram link via WebApp JS bridge
// ──────────────────────────────────────────────────────────────────

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
            // Fallback: open in new tab
            let _ = window.open_with_url_and_target(url, "_blank");
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = url; // suppress unused warning
    }
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
    {
        let _ = text;
    }
}

// ──────────────────────────────────────────────────────────────────
// Main Referrals page component
// ──────────────────────────────────────────────────────────────────

#[component]
pub fn Referrals() -> Element {
    // Mock data — in production, fetch from /api/referrals/me/:telegram_id
    let referral_me = use_signal(|| ReferralMe {
        code: "Wd3f9Xk2".to_string(),
        invite_link: "https://t.me/Woody_WeedPecker_bot?start=ref_Wd3f9Xk2".to_string(),
        stats: ReferralStats {
            total_invited: 0,
            confirmed: 0,
            pending: 0,
            total_bonus_earned: 0.0,
        },
    });

    let leaderboard: Signal<Vec<TopReferrer>> = use_signal(Vec::new);
    let copied = use_signal(|| false);

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
                font-family: 'Press Start 2P', monospace;
                padding-bottom: 80px;
            ",

            // ── Header ──────────────────────────────────────────
            div {
                style: "padding: 20px 16px 12px; text-align: center;",
                h1 {
                    style: "font-size: 12px; color: #39ff14; text-shadow: 0 0 8px rgba(57,255,20,0.5);",
                    "🎁 Referral Program"
                }
                p {
                    style: "font-size: 7px; color: #888; margin-top: 6px;",
                    "Invite friends — earn bonuses"
                }
            }

            // ── Invite link card ─────────────────────────────────
            div {
                style: "
                    max-width: 380px; margin: 0 auto 16px; padding: 20px 16px;
                    background: linear-gradient(135deg, #151520, #1f1f2a);
                    border: 2px solid #39ff14;
                    border-radius: 16px;
                    box-shadow: 0 0 20px rgba(57,255,20,0.2);
                ",
                p {
                    style: "font-size: 7px; color: #888; margin-bottom: 8px;",
                    "YOUR REFERRAL LINK"
                }
                div {
                    style: "
                        background: rgba(0,0,0,0.4);
                        border: 1px solid #333;
                        border-radius: 8px;
                        padding: 10px 12px;
                        font-size: 7px;
                        color: #39ff14;
                        word-break: break-all;
                        margin-bottom: 12px;
                    ",
                    "{link}"
                }
                div {
                    style: "display: flex; gap: 8px;",

                    // Copy button
                    button {
                        style: "
                            flex: 1;
                            background: rgba(57,255,20,0.15);
                            border: 1px solid #39ff14;
                            border-radius: 8px;
                            color: #39ff14;
                            font-family: inherit;
                            font-size: 7px;
                            padding: 10px 8px;
                            cursor: pointer;
                        ",
                        onclick: move |_| {
                            copy_to_clipboard(&link_for_copy);
                        },
                        if *copied.read() { "✅ Copied!" } else { "📋 Copy" }
                    }

                    // Share via Telegram button
                    {
                        let share_url = format!(
                            "https://t.me/share/url?url={}&text={}",
                            urlencoding_simple(&link),
                            urlencoding_simple("🪵 Join Woody Weed and get bonuses!")
                        );
                        rsx! {
                            button {
                                style: "
                                    flex: 1;
                                    background: rgba(57,255,20,0.15);
                                    border: 1px solid #39ff14;
                                    border-radius: 8px;
                                    color: #39ff14;
                                    font-family: inherit;
                                    font-size: 7px;
                                    padding: 10px 8px;
                                    cursor: pointer;
                                ",
                                onclick: move |_| {
                                    open_telegram_link(&share_url);
                                },
                                "📤 Share"
                            }
                        }
                    }
                }
            }

            // ── Stats cards ──────────────────────────────────────
            div {
                style: "
                    max-width: 380px; margin: 0 auto 16px; padding: 0 16px;
                    display: grid; grid-template-columns: repeat(2, 1fr); gap: 8px;
                ",

                {stat_card("👥", "Invited", &stats.total_invited.to_string())}
                {stat_card("✅", "Confirmed", &stats.confirmed.to_string())}
                {stat_card("⏳", "Pending", &stats.pending.to_string())}
                {stat_card("💰", "Bonus earned", &format!("{:.0} ฿", stats.total_bonus_earned))}
            }

            // ── Leaderboard ──────────────────────────────────────
            div {
                style: "max-width: 380px; margin: 0 auto; padding: 0 16px;",

                h2 {
                    style: "font-size: 9px; color: #ffd700; margin-bottom: 12px; text-align: center;",
                    "🏆 Top Referrers"
                }

                if board.is_empty() {
                    div {
                        style: "text-align: center; color: #555; font-size: 7px; padding: 20px;",
                        "No data yet — be the first!"
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

// ──────────────────────────────────────────────────────────────────
// Helper sub-components (functions returning Element)
// ──────────────────────────────────────────────────────────────────

fn stat_card(icon: &str, label: &str, value: &str) -> Element {
    let icon = icon.to_string();
    let label = label.to_string();
    let value = value.to_string();
    rsx! {
        div {
            style: "
                background: linear-gradient(135deg, #151520, #1a1a25);
                border: 1px solid #2a2a3a;
                border-radius: 12px;
                padding: 14px 12px;
                text-align: center;
            ",
            div { style: "font-size: 18px; margin-bottom: 6px;", "{icon}" }
            div { style: "font-size: 11px; color: #39ff14; margin-bottom: 4px;", "{value}" }
            div { style: "font-size: 6px; color: #666;", "{label}" }
        }
    }
}

fn leaderboard_row(rank: usize, telegram_id: i64, referral_count: i64, bonus: f64) -> Element {
    let medal = match rank {
        1 => "🥇",
        2 => "🥈",
        3 => "🥉",
        _ => "  ",
    };
    let rank_color = match rank {
        1 => "#ffd700",
        2 => "#c0c0c0",
        3 => "#cd7f32",
        _ => "#555",
    };
    let medal = medal.to_string();
    let rank_style = format!("font-size: 7px; color: {};", rank_color);
    rsx! {
        div {
            style: "
                display: flex; align-items: center; gap: 10px;
                background: rgba(255,255,255,0.03);
                border: 1px solid #2a2a3a;
                border-radius: 8px;
                padding: 10px 12px;
            ",
            span { style: "font-size: 14px; width: 20px; flex-shrink: 0;", "{medal}" }
            span { style: "{rank_style}", "#{rank}" }
            div { style: "flex: 1;",
                div { style: "font-size: 7px; color: #e8e8e8;",
                    {format!("ID: …{}", telegram_id % 10000)}
                }
                div { style: "font-size: 6px; color: #555; margin-top: 2px;",
                    {format!("{} invited • {:.0} ฿ earned", referral_count, bonus)}
                }
            }
        }
    }
}

/// Minimal percent-encoding for URLs (no external deps).
fn urlencoding_simple(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
