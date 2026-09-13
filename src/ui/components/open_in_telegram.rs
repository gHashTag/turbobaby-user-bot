//! The state a screen shows when it is opened outside Telegram.
//!
//! Orders, profile and checkout derive the customer's identity from Telegram's
//! `initData`; in a plain browser tab there is none, and the raw resource error
//! ("No telegram_id") reads as a broken tab — it was reported as one on
//! 2026-09-13, minutes after the shop went live. This component says what is
//! actually wrong and what to do about it, in both shop languages (D13).

use crate::trios::i18n::{t, T_OPEN_IN_TELEGRAM_BODY, T_OPEN_IN_TELEGRAM_TITLE};
use crate::ui::lang::current_lang;
use dioxus::prelude::*;

#[component]
pub fn OpenInTelegramNotice() -> Element {
    let lang = current_lang();
    rsx! {
        div { style: "
            min-height: 60vh;
            display: flex; flex-direction: column;
            align-items: center; justify-content: center;
            text-align: center; padding: 32px 20px; gap: 8px;
        ",
            div { style: "font-size: 72px; margin-bottom: 8px;", "🏍️" }
            h2 { style: "font-size: 17px; color: #e8e8e8; margin: 0;", "{t(lang, T_OPEN_IN_TELEGRAM_TITLE)}" }
            p { style: "font-size: 13px; color: #8b8b9e; margin: 0; max-width: 280px; line-height: 1.5;",
                "{t(lang, T_OPEN_IN_TELEGRAM_BODY)}"
            }
        }
    }
}
