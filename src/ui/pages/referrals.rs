//! The referral screen: your link, your numbers, your friends, your ladder.
//!
//! The friends list and the milestone ladder did not start here. They were
//! rendered by the garden screen, which is gone (D5) — but the garden only
//! ever *hosted* them: both panels read `referral_events` and
//! `referral_milestones`, tables migration 083 does not touch. Deleting the
//! host and the tenants together would have retired a live mechanic by
//! association, so the two panels moved to the page they were always about.
//!
//! One thing did not move. The garden's milestone panel printed its own
//! `1 => 100, 3 => 300, 5 => 500`, a fourth hand-written copy of a ladder that
//! also existed in three places on the server. A client that invents the
//! reward is right until somebody edits `loyalty_config`, and silently wrong
//! for ever after. Every number below comes off the wire.

use crate::trios::i18n::{
    t, tf, T_LOADING, T_REFERRAL_COPIED, T_REFERRAL_COPY, T_REFERRAL_EMPTY_LEADERBOARD,
    T_REFERRAL_ID_MASK, T_REFERRAL_INVITEES_EMPTY, T_REFERRAL_INVITEES_TITLE,
    T_REFERRAL_INVITEE_JOINED, T_REFERRAL_INVITEE_ORDERED, T_REFERRAL_INVITEE_UNKNOWN,
    T_REFERRAL_LINK_LABEL, T_REFERRAL_MILESTONE_AWARDED, T_REFERRAL_MILESTONE_SUBTITLE,
    T_REFERRAL_MILESTONE_TITLE, T_REFERRAL_ROW_META, T_REFERRAL_SHARE, T_REFERRAL_SHARE_TEXT,
    T_REFERRAL_STAT_BONUS, T_REFERRAL_STAT_CONFIRMED, T_REFERRAL_STAT_INVITED,
    T_REFERRAL_STAT_PENDING, T_REFERRAL_SUBTITLE, T_REFERRAL_TITLE, T_REFERRAL_TOP,
};
use crate::ui::api::context::api_base_url;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};
use dioxus::prelude::*;
use serde::Deserialize;

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

/// One person who followed this user's invite link.
///
/// `status` and `source` are on the wire and deliberately unread: `status` is
/// already aggregated into the confirmed/pending cards above, and `source` is
/// the A/B share bucket — an internal measurement, not something to show the
/// customer whose friend it describes.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Invitee {
    /// Ready to print, and already localised for everyone who has a name or a
    /// handle. Only the anonymous case needs this screen's help.
    pub display_name: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub is_anonymous: bool,
    /// The `#NNNN` tail, sent on its own exactly when `display_name` had to
    /// fall back to the English word the server had no language to translate.
    #[serde(default)]
    pub suffix: Option<String>,
    #[serde(default)]
    pub has_ordered: bool,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
struct InviteesResponse {
    /// `count` is on the wire too. It is not read here: a length sent beside
    /// the list it counts is one more number that can disagree with the truth
    /// next to it, and `Vec::len` cannot.
    #[serde(default)]
    invitees: Vec<Invitee>,
}

/// A rung of the referral ladder and the money attached to it.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MilestoneRung {
    pub milestone: i32,
    #[serde(default)]
    pub bonus_amount: f64,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
struct MilestonesResponse {
    #[serde(default)]
    confirmed: i64,
    /// Every rung the ladder has, with what it pays **today** — the server
    /// reads `loyalty_config` for these. This is also the ladder itself: the
    /// response carries a bare `thresholds` array as well, for clients shipped
    /// before this panel existed, and reading both would be two copies of one
    /// list in one struct.
    #[serde(default)]
    bonuses: Vec<MilestoneRung>,
    /// The rungs already reached, with what each one **actually credited**,
    /// read from its `referral_milestones` row. Not the same number as
    /// `bonuses` for the same rung whenever the shop has edited the config
    /// since — which is the whole reason the server sends both.
    #[serde(default)]
    awards: Vec<MilestoneRung>,
}

fn open_telegram_link(url: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;
        if let Some(window) = web_sys::window() {
            if let Ok(tg) = js_sys::Reflect::get(&window, &JsValue::from_str("Telegram")) {
                if let Ok(webapp) = js_sys::Reflect::get(&tg, &JsValue::from_str("WebApp")) {
                    let func =
                        js_sys::Reflect::get(&webapp, &JsValue::from_str("openTelegramLink"))
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
    {
        let _ = url;
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

// Use the urlencoding crate (already in Cargo.toml) for correct URL encoding.
fn urlencoding_simple(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

#[component]
pub fn Referrals() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0i64);
    let lang = crate::ui::lang::current_lang();
    let referral_me = use_signal(ReferralMe::default);
    let leaderboard = use_signal(Vec::<TopReferrer>::new);
    // `None` is "not answered", not "empty". A failed request must leave these
    // panels off the screen rather than render the empty state, which says
    // «пригласи друзей» to somebody who may have invited a dozen.
    let invitees = use_signal(|| None::<Vec<Invitee>>);
    let milestones = use_signal(|| None::<MilestonesResponse>);
    let loading = use_signal(|| true);
    let mut copied = use_signal(|| false);
    let init_data = use_telegram_init_data();

    {
        let mut me_c = referral_me;
        let mut board_c = leaderboard;
        let mut invitees_c = invitees;
        let mut milestones_c = milestones;
        let mut loading_c = loading;
        let tid = telegram_id;
        let init = init_data.clone();
        use_hook(move || {
            if tid == 0 {
                me_c.set(ReferralMe {
                    code: "N/A".into(),
                    invite_link: "https://t.me/turboagent_phuket_bot?start=ref_NA".to_string(),
                    stats: ReferralStats::default(),
                });
                loading_c.set(false);
                return;
            }
            spawn(async move {
                let base = api_base_url();
                let client = crate::ui::api::local_client::LocalClient::new();

                if let Ok(resp) = client
                    .get(&format!("{}/api/referrals/me/{}", base, tid))
                    .header("X-Telegram-Init-Data", init.as_str())
                    .send()
                    .await
                {
                    if let Ok(text) = resp.text().await {
                        if let Ok(me) = serde_json::from_str::<ReferralMe>(&text) {
                            me_c.set(me);
                        }
                    }
                }

                if let Ok(resp) = client
                    .get(&format!("{}/api/referrals/leaderboard?limit=10", base))
                    .send()
                    .await
                {
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

                // The link and the counters are the page; everything below is
                // detail. Releasing the spinner here keeps time-to-first-paint
                // exactly where it was before the two panels moved in.
                loading_c.set(false);

                if let Ok(text) = crate::ui::api::http::fetch_text_authed(
                    &format!("{}/api/referrals/me/{}/invitees", base, tid),
                    &init,
                )
                .await
                {
                    if let Ok(resp) = serde_json::from_str::<InviteesResponse>(&text) {
                        invitees_c.set(Some(resp.invitees));
                    }
                }

                if let Ok(text) = crate::ui::api::http::fetch_text_authed(
                    &format!("{}/api/referrals/me/{}/milestones", base, tid),
                    &init,
                )
                .await
                {
                    if let Ok(resp) = serde_json::from_str::<MilestonesResponse>(&text) {
                        milestones_c.set(Some(resp));
                    }
                }
            });
        });
    }

    let me = referral_me.read().clone();
    let link = me.invite_link.clone();
    let link_for_copy = link.clone();
    let stats = me.stats.clone();
    let board = leaderboard.read().clone();
    let friends = invitees.read().clone();
    let ladder = milestones.read().clone();

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
                    "{t(lang, T_REFERRAL_TITLE)}"
                }
                p {
                    style: "font-size: 15px; color: #888; margin-top: 6px;",
                    "{t(lang, T_REFERRAL_SUBTITLE)}"
                }
            }

            if *loading.read() {
                div { style: "text-align: center; padding: 30px;",
                    span { style: "font-size: 15px; color: #39ff14;", "{t(lang, T_LOADING)}" }
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
                        "{t(lang, T_REFERRAL_LINK_LABEL)}"
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
                            if *copied.read() { "{t(lang, T_REFERRAL_COPIED)}" } else { "{t(lang, T_REFERRAL_COPY)}" }
                        }

                        {
                            let share_text = t(lang, T_REFERRAL_SHARE_TEXT);
                            let share_url = format!(
                                "https://t.me/share/url?url={}&text={}",
                                urlencoding_simple(&link),
                                urlencoding_simple(share_text)
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
                                    "{t(lang, T_REFERRAL_SHARE)}"
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
                    {stat_card("\u{1F465}", t(lang, T_REFERRAL_STAT_INVITED), &stats.total_invited.to_string())}
                    {stat_card("\u{2705}", t(lang, T_REFERRAL_STAT_CONFIRMED), &stats.confirmed.to_string())}
                    {stat_card("\u{23F3}", t(lang, T_REFERRAL_STAT_PENDING), &stats.pending.to_string())}
                    {stat_card("\u{1F4B0}", t(lang, T_REFERRAL_STAT_BONUS), &crate::trios::pricing::format_baht(
                        if stats.total_bonus_earned.is_finite() {
                            stats.total_bonus_earned.max(0.0)
                        } else { 0.0 }
                    ))}
                }

                if let Some(m) = ladder {
                    {milestones_panel(lang, m)}
                }

                if let Some(list) = friends {
                    {invitees_panel(lang, list)}
                }

                div {
                    style: "max-width: 380px; margin: 0 auto; padding: 0 16px;",

                    h2 {
                        style: "font-size: 13px; font-weight: 700; color: #ffd700; margin-bottom: 12px; text-align: center;",
                        "{t(lang, T_REFERRAL_TOP)}"
                    }

                    if board.is_empty() {
                        div {
                            style: "text-align: center; color: #555; font-size: 15px; padding: 20px;",
                            "{t(lang, T_REFERRAL_EMPTY_LEADERBOARD)}"
                        }
                    } else {
                        div {
                            style: "display: flex; flex-direction: column; gap: 6px;",
                            for (idx, entry) in board.iter().enumerate() {
                                {leaderboard_row(lang, idx + 1, entry.telegram_id, entry.referral_count, entry.total_bonus_earned)}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The ladder: every rung, what it pays, and which ones are already paid.
///
/// Reached rungs print the amount that was **actually credited**; unreached
/// ones print what the shop pays for them today. The two can differ, and the
/// server sends both for exactly that reason — see `MilestonesResponse`.
fn milestones_panel(lang: crate::trios::core::Lang, m: MilestonesResponse) -> Element {
    if m.bonuses.is_empty() {
        // No ladder came back. A panel with a title and nothing under it says
        // less than no panel at all.
        return rsx! {};
    }

    // The server builds this list in ladder order, but "the caller happens to
    // sort it" is not a property the caller declared.
    let mut rungs = m.bonuses.clone();
    rungs.sort_by_key(|r| r.milestone);

    let top = rungs.last().map(|r| r.milestone).unwrap_or(0);
    let goal = rungs
        .iter()
        .map(|r| r.milestone)
        .find(|rung| i64::from(*rung) > m.confirmed)
        .unwrap_or(top);
    let subtitle = tf(
        lang,
        T_REFERRAL_MILESTONE_SUBTITLE,
        &[m.confirmed.to_string(), goal.to_string()],
    );

    rsx! {
        div {
            style: "
                max-width: 380px; margin: 0 auto 16px; padding: 16px;
                background: #16213e;
                border: 4px solid #2a2a4a;
                box-shadow: 4px 4px 0 #000;
            ",
            h2 {
                style: "font-size: 13px; font-weight: 700; color: #ffd700; text-align: center;",
                "{t(lang, T_REFERRAL_MILESTONE_TITLE)}"
            }
            p {
                style: "font-size: 15px; color: #888; margin: 6px 0 12px; text-align: center;",
                "{subtitle}"
            }
            div {
                style: "display: flex; flex-direction: column; gap: 6px;",
                for rung in rungs.iter() {
                    {milestone_row(lang, rung, &m.awards)}
                }
            }
        }
    }
}

fn milestone_row(
    lang: crate::trios::core::Lang,
    rung: &MilestoneRung,
    awards: &[MilestoneRung],
) -> Element {
    let paid = awards.iter().find(|a| a.milestone == rung.milestone);
    let amount = paid.map(|a| a.bonus_amount).unwrap_or(rung.bonus_amount);
    let text = tf(
        lang,
        T_REFERRAL_MILESTONE_AWARDED,
        &[
            rung.milestone.to_string(),
            crate::trios::pricing::format_baht(amount),
        ],
    );
    let (colour, mark) = if paid.is_some() {
        ("#39ff14", "\u{2705}")
    } else {
        ("#666", "\u{2022}")
    };
    rsx! {
        div {
            style: "
                display: flex; align-items: center; gap: 8px;
                background: rgba(0,0,0,0.25);
                border: 4px solid #2a2a4a;
                padding: 8px 10px;
            ",
            span { style: "font-size: 14px; width: 18px; flex-shrink: 0;", "{mark}" }
            span { style: "font-size: 15px; color: {colour};", "{text}" }
        }
    }
}

/// The friends panel: who followed the link, and how far each of them got.
fn invitees_panel(lang: crate::trios::core::Lang, list: Vec<Invitee>) -> Element {
    rsx! {
        div {
            style: "max-width: 380px; margin: 0 auto 16px; padding: 0 16px;",
            h2 {
                style: "font-size: 13px; font-weight: 700; color: #ffd700; margin-bottom: 12px; text-align: center;",
                "{t(lang, T_REFERRAL_INVITEES_TITLE)}"
            }
            if list.is_empty() {
                div {
                    style: "text-align: center; color: #555; font-size: 15px; padding: 20px;",
                    "{t(lang, T_REFERRAL_INVITEES_EMPTY)}"
                }
            } else {
                div {
                    style: "display: flex; flex-direction: column; gap: 6px;",
                    for invitee in list.iter() {
                        {invitee_row(lang, invitee)}
                    }
                }
            }
        }
    }
}

fn invitee_row(lang: crate::trios::core::Lang, inv: &Invitee) -> Element {
    // The server holds no language, so somebody it knows nothing about arrives
    // as the English word plus a `#NNNN` tail. It sends the tail separately for
    // this substitution; everyone with a name or a handle is already right.
    let line = if inv.is_anonymous {
        match inv.suffix.as_deref() {
            Some(suffix) => format!("{} {}", t(lang, T_REFERRAL_INVITEE_UNKNOWN), suffix),
            None => t(lang, T_REFERRAL_INVITEE_UNKNOWN).to_string(),
        }
    } else {
        inv.display_name.clone()
    };

    // `display_name` is the whole line, handle included; `username` is that
    // handle on its own. Peeling off the exact string the server also sent —
    // rather than guessing at its format — is what lets the row give the
    // handle its own colour. If the tail is not there, the line prints whole
    // and nothing is lost.
    let (name, handle) = match inv.username.as_deref() {
        Some(u) => {
            let tail = format!("@{u}");
            match line.strip_suffix(&tail).map(str::trim_end) {
                Some(head) if !head.is_empty() => (head.to_string(), Some(tail)),
                // `@handle` alone: there is no name to put beside it.
                Some(_) => (tail, None),
                None => (line, None),
            }
        }
        None => (line, None),
    };

    let (mark, state) = if inv.has_ordered {
        ("\u{1F6D2}", t(lang, T_REFERRAL_INVITEE_ORDERED))
    } else {
        ("\u{1F44B}", t(lang, T_REFERRAL_INVITEE_JOINED))
    };

    rsx! {
        div {
            style: "
                display: flex; align-items: center; gap: 10px;
                background: #16213e;
                border: 4px solid #2a2a4a;
                box-shadow: 4px 4px 0 #000;
                padding: 10px 12px;
            ",
            span { style: "font-size: 14px; width: 20px; flex-shrink: 0;", "{mark}" }
            div { style: "flex: 1; min-width: 0;",
                div { style: "font-size: 15px; color: #e8e8e8; overflow: hidden; text-overflow: ellipsis;", "{name}" }
                if let Some(handle) = handle {
                    div { style: "font-size: 15px; color: #39ff14; margin-top: 2px;", "{handle}" }
                }
                div { style: "font-size: 15px; color: #555; margin-top: 2px;", "{state}" }
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

fn leaderboard_row(
    lang: crate::trios::core::Lang,
    rank: usize,
    telegram_id: i64,
    referral_count: i64,
    bonus: f64,
) -> Element {
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
    let safe_bonus = if bonus.is_finite() {
        bonus.max(0.0)
    } else {
        0.0
    };
    let id_short = telegram_id % 10000;
    let id_text = tf(lang, T_REFERRAL_ID_MASK, &[format!("{id_short}")]);
    let meta_text = tf(
        lang,
        T_REFERRAL_ROW_META,
        &[
            referral_count.to_string(),
            crate::trios::pricing::format_baht(safe_bonus),
        ],
    );
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
                div { style: "font-size: 15px; color: #e8e8e8;", "{id_text}" }
                div { style: "font-size: 15px; color: #555; margin-top: 2px;", "{meta_text}" }
            }
        }
    }
}
