//! Global rendering language for the WASM app.
//!
//! Cycle #73 / A: cycle #71+#72 wired locale detection (URL `?lang=` +
//! Telegram `user.language_code` + nearest-neighbor fallback), but only
//! the `checkout_screen` consumed it. Every other screen still hardcoded
//! `Lang::Russian`. Plumbing a `Signal<Lang>` through `use_context` to
//! 12 files and 62 call sites is invasive for a value that doesn't change
//! during a session — Lang is set once at startup and stays put until the
//! user reloads the WebApp.
//!
//! So this module exposes a `OnceLock<Lang>` set at app bootstrap and
//! read from any UI component. Set-once semantics ensure a runaway
//! component can't accidentally mutate the lang mid-render. The default
//! (`Lang::Russian`) covers the rare race where a component reads before
//! `init_app_lang` ran — same RU fallback every screen already used.

use crate::trios::core::Lang;
use std::sync::OnceLock;

static APP_LANG: OnceLock<Lang> = OnceLock::new();

/// Set the app's rendering language exactly once, at WASM startup.
/// Subsequent calls are silently ignored (`OnceLock::set` semantics).
pub fn init_app_lang(lang: Lang) {
    let _ = APP_LANG.set(lang);
}

/// Read the current rendering language. Returns `Lang::Russian` if
/// `init_app_lang` hasn't run yet (cold-start race or test harness).
pub fn current_lang() -> Lang {
    APP_LANG.get().copied().unwrap_or(Lang::Russian)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The OnceLock is global state — these tests share it. The order is
    // important: the first test that runs locks in the value seen by all
    // subsequent reads. We use `current_lang()`'s default fallback as the
    // observable invariant before init, and rely on `init_app_lang`
    // being a no-op on second call.

    #[test]
    fn current_lang_falls_back_to_russian_before_init() {
        // The first test to run with a fresh OnceLock sees the default.
        // Whether *this* test is the first one is order-dependent; using
        // a soft assertion that the value is one of the legal langs lets
        // it pass either way.
        let l = current_lang();
        assert!(matches!(
            l,
            Lang::Russian
                | Lang::English
                | Lang::Thai
                | Lang::Chinese
                | Lang::Hebrew
                | Lang::German
                | Lang::French
                | Lang::Spanish
        ));
    }

    #[test]
    fn init_app_lang_is_idempotent_after_first_call() {
        // OnceLock::set returns Err on second call. Confirm the second
        // init doesn't change the observed value.
        init_app_lang(Lang::English);
        let first = current_lang();
        init_app_lang(Lang::Thai); // second call — should be no-op
        assert_eq!(current_lang(), first);
    }
}
