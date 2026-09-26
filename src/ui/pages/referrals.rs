//! The referral screen: your link, your numbers, your friends, your balance.
//!
//! The friends list did not start here. It was rendered by the garden screen,
//! which is gone (D5) — but the garden only ever *hosted* it: the panel reads
//! `referral_events`, a table migration 083 does not touch, so it moved to the
//! page it was always about.
//!
//! The owner answered R3 on 2026-09-26: «Должно начисляться исключительно за то
//! кто арендовал 10% скидка», «Пригласивший и может забрать скидкой за аренду
//! или деньгами», and on the points «Убрать, только скидка 10%». So the
//! milestone ladder and the fourth stat card, which printed points the shop no
//! longer credits, are gone, and the referral balance took their place: 10% of
//! every rental an invited friend completes, recorded by a manager. The
//! customer can hold the whole balance against their next rental or ask for a
//! payout; a manager settles either by hand. This page moves no money. Every
//! number on it comes off the wire, and the arithmetic of the credit lives in
//! `crate::trios::referral_credit`, not here.
//!
//! The top-referrers list that closed the page is gone too. The owner, of that
//! list and the public address it read (2026-09-26, verbatim): «Убрать топ и
//! закрыть адрес». It printed other customers' ids and the point totals frozen
//! before R3; `/api/referrals/leaderboard` now answers an admin only, and this
//! page no longer asks it.

use crate::trios::i18n::{
    t, T_API_ERR_UNKNOWN, T_LOADING, T_REFERRAL_APPLY_TO_RENTAL, T_REFERRAL_BALANCE,
    T_REFERRAL_COPIED, T_REFERRAL_COPY, T_REFERRAL_INVITEES_TITLE, T_REFERRAL_INVITEE_JOINED,
    T_REFERRAL_INVITEE_ORDERED, T_REFERRAL_INVITEE_UNKNOWN, T_REFERRAL_LINK_LABEL,
    T_REFERRAL_PAYOUT_REQUESTED, T_REFERRAL_REQUEST_PAYOUT, T_REFERRAL_SHARE,
    T_REFERRAL_STAT_CONFIRMED, T_REFERRAL_STAT_INVITED, T_REFERRAL_STAT_PENDING,
    T_REFERRAL_SUBTITLE, T_REFERRAL_TITLE,
};
use crate::trios::referral_credit::{
    shown_balance, OpenRequestBody, OpenRequestResponse, ReferralCredit,
};
use crate::ui::api::context::api_base_url;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};
use dioxus::prelude::*;
use serde::Deserialize;

/// `total_bonus_earned` is on the wire too and deliberately unread since
/// 2026-09-26: it sums the referral points credited before R3, which the shop
/// no longer credits, so under this page's numbers it would read as the
/// balance. The balance is `ReferralCredit`, from its own route.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct ReferralStats {
    pub total_invited: i64,
    pub confirmed: i64,
    pub pending: i64,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct ReferralMe {
    pub code: String,
    pub invite_link: String,
    pub stats: ReferralStats,
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

/// The referral balance as the server has it, or `None` when it did not
/// answer with one. `None` hides the block: a failed read must not print a
/// balance of zero to somebody who may have one.
async fn fetch_credit(url: &str, init_data: &str) -> Option<ReferralCredit> {
    match crate::ui::api::http::fetch_text_authed_full(url, init_data).await {
        Ok((200, text)) => serde_json::from_str::<ReferralCredit>(&text).ok(),
        _ => None,
    }
}

/// One tap on «Списать в счёт аренды» (`"redeem"`) or «Запросить выплату»
/// (`"payout"`). The server holds the whole available balance and tells the
/// manager; nothing here moves money. The page then shows the server's state,
/// read again after every request, and only when that state holds no open
/// request after a failure does it say that something went wrong.
fn open_request(
    kind: &'static str,
    telegram_id: i64,
    init_data: String,
    mut credit: Signal<Option<ReferralCredit>>,
    mut busy: Signal<bool>,
    mut failed: Signal<bool>,
) {
    if *busy.peek() {
        return;
    }
    busy.set(true);
    failed.set(false);
    spawn(async move {
        let base = api_base_url();
        let me = format!("{}/api/referral-credit/me/{}", base, telegram_id);
        let answered = match serde_json::to_string(&OpenRequestBody {
            kind: kind.to_string(),
        }) {
            Ok(body) => crate::ui::api::http::post_json_authed(
                &format!("{}/api/referral-credit/me/{}/requests", base, telegram_id),
                &init_data,
                &body,
            )
            .await
            .ok()
            .and_then(|text| serde_json::from_str::<OpenRequestResponse>(&text).ok()),
            Err(_) => None,
        };
        if let Some(response) = &answered {
            credit.set(Some(response.credit.clone()));
        }
        let fresh = fetch_credit(&me, &init_data).await;
        if let Some(state) = fresh {
            credit.set(Some(state));
        }
        if answered.is_none() {
            let something_open = credit
                .peek()
                .as_ref()
                .is_some_and(|c| c.open_request.is_some());
            failed.set(!something_open);
        }
        busy.set(false);
    });
}

#[component]
pub fn Referrals() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0i64);
    let lang = crate::ui::lang::current_lang();
    let referral_me = use_signal(ReferralMe::default);
    // `None` is "not answered", not "empty". A failed request must leave these
    // panels off the screen rather than render an empty state.
    let invitees = use_signal(|| None::<Vec<Invitee>>);
    let credit = use_signal(|| None::<ReferralCredit>);
    let busy = use_signal(|| false);
    let failed = use_signal(|| false);
    let loading = use_signal(|| true);
    let mut copied = use_signal(|| false);
    let init_data = use_telegram_init_data();

    {
        let mut me_c = referral_me;
        let mut invitees_c = invitees;
        let mut credit_c = credit;
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

                // The link and the counters are the page; everything below is
                // detail. Releasing the spinner here keeps time-to-first-paint
                // where it was before the panels moved in.
                loading_c.set(false);

                if let Some(state) =
                    fetch_credit(&format!("{}/api/referral-credit/me/{}", base, tid), &init).await
                {
                    credit_c.set(Some(state));
                }

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
            });
        });
    }

    let me = referral_me.read().clone();
    let link = me.invite_link.clone();
    let link_for_copy = link.clone();
    let stats = me.stats.clone();
    let friends = invitees.read().clone();
    let balance = credit.read().clone();

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
                            // The link alone. The text that rode along promised
                            // the friend bonuses, and since R3 the friend gets
                            // nothing; no other sentence was worded for it.
                            let share_url = format!(
                                "https://t.me/share/url?url={}",
                                urlencoding_simple(&link)
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
                        display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px;
                    ",
                    {stat_card("\u{1F465}", t(lang, T_REFERRAL_STAT_INVITED), &stats.total_invited.to_string())}
                    {stat_card("\u{2705}", t(lang, T_REFERRAL_STAT_CONFIRMED), &stats.confirmed.to_string())}
                    {stat_card("\u{23F3}", t(lang, T_REFERRAL_STAT_PENDING), &stats.pending.to_string())}
                }

                if let Some(state) = balance {
                    {credit_panel(lang, telegram_id, init_data.clone(), state, credit, busy, failed)}
                }

                if let Some(list) = friends {
                    if !list.is_empty() {
                        {invitees_panel(lang, list)}
                    }
                }
            }
        }
    }
}

/// The referral balance and its two requests (owner, 2026-09-26, R3).
///
/// The balance is printed through `shown_balance`, so a negative one — a
/// reversal after the credit was spent — reads as zero: no approved sentence
/// explains a debt. Both buttons are live only while something is available
/// and nothing is held. While a redemption is held its button carries a tick
/// and stays pressed; while a payout is held the page says the manager has the
/// request. Everything else that can go wrong prints the generic sentence.
fn credit_panel(
    lang: crate::trios::core::Lang,
    telegram_id: i64,
    init_data: String,
    state: ReferralCredit,
    credit: Signal<Option<ReferralCredit>>,
    busy: Signal<bool>,
    failed: Signal<bool>,
) -> Element {
    let amount = crate::trios::pricing::format_baht(shown_balance(state.balance_thb) as f64);
    let open_kind = state.open_request.as_ref().map(|r| r.kind.clone());
    let redeem_open = open_kind.as_deref() == Some("redeem");
    let payout_open = open_kind.as_deref() == Some("payout");
    let can_request = open_kind.is_none() && state.available_thb >= 1 && !*busy.read();
    let redeem_label = if redeem_open {
        format!("\u{2705} {}", t(lang, T_REFERRAL_APPLY_TO_RENTAL))
    } else {
        t(lang, T_REFERRAL_APPLY_TO_RENTAL).to_string()
    };
    let button_style = if can_request {
        "flex: 1; background: rgba(57,255,20,0.15); border: 4px solid #2d9e0f; \
         box-shadow: 3px 3px 0 #000; color: #39ff14; font-family: inherit; font-size: 14px; \
         font-weight: 700; padding: 10px 8px; cursor: pointer;"
    } else {
        "flex: 1; background: rgba(0,0,0,0.25); border: 4px solid #2a2a4a; \
         box-shadow: 3px 3px 0 #000; color: #888; font-family: inherit; font-size: 14px; \
         font-weight: 700; padding: 10px 8px; cursor: default;"
    };
    let init_redeem = init_data.clone();
    let init_payout = init_data;

    rsx! {
        div {
            style: "
                max-width: 380px; margin: 0 auto 16px; padding: 16px;
                background: #16213e;
                border: 4px solid #2a2a4a;
                box-shadow: 4px 4px 0 #000;
                text-align: center;
            ",
            p {
                style: "font-size: 13px; font-weight: 700; color: #888; margin-bottom: 6px;",
                "{t(lang, T_REFERRAL_BALANCE)}"
            }
            div {
                style: "font-size: 26px; font-weight: 800; color: #ffe600; margin-bottom: 12px;",
                "{amount}"
            }
            div {
                style: "display: flex; gap: 8px;",
                button {
                    style: "{button_style}",
                    disabled: !can_request,
                    onclick: move |_| {
                        open_request("redeem", telegram_id, init_redeem.clone(), credit, busy, failed);
                    },
                    "{redeem_label}"
                }
                button {
                    style: "{button_style}",
                    disabled: !can_request,
                    onclick: move |_| {
                        open_request("payout", telegram_id, init_payout.clone(), credit, busy, failed);
                    },
                    "{t(lang, T_REFERRAL_REQUEST_PAYOUT)}"
                }
            }
            if payout_open {
                p {
                    style: "font-size: 15px; color: #39ff14; margin-top: 10px;",
                    "{t(lang, T_REFERRAL_PAYOUT_REQUESTED)}"
                }
            }
            if *failed.read() {
                p {
                    style: "font-size: 15px; color: #ff6b6b; margin-top: 10px;",
                    "{t(lang, T_API_ERR_UNKNOWN)}"
                }
            }
        }
    }
}

/// The friends panel: who followed the link, and how far each of them got.
/// Not rendered for an empty list: the sentence it printed then promised the
/// friend bonuses, and no other was worded (R3).
fn invitees_panel(lang: crate::trios::core::Lang, list: Vec<Invitee>) -> Element {
    rsx! {
        div {
            style: "max-width: 380px; margin: 0 auto 16px; padding: 0 16px;",
            h2 {
                style: "font-size: 13px; font-weight: 700; color: #ffd700; margin-bottom: 12px; text-align: center;",
                "{t(lang, T_REFERRAL_INVITEES_TITLE)}"
            }
            div {
                style: "display: flex; flex-direction: column; gap: 6px;",
                for invitee in list.iter() {
                    {invitee_row(lang, invitee)}
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
