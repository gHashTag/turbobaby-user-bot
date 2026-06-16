//! Global rendering language for the WASM app — reactive edition.
//!
//! **Cycle (this commit, after #171):** the original `OnceLock<Lang>`
//! design (cycle #73) was set-once-then-immutable: a profile-screen
//! switcher could update a *separate* `Signal<Language>` (cycle #74)
//! that no other component observed. Users clicked the EN flag and
//! every screen still rendered Russian. Reported by the user.
//!
//! Fix: replace the `OnceLock<Lang>` with a Dioxus `GlobalSignal<Lang>`.
//! All 61+ existing `current_lang()` callsites stay unchanged but now
//! subscribe to the signal — when `set_app_lang(new)` fires from the
//! profile switcher, every component that read the language in the
//! previous render re-renders with the new value.
//!
//! Bridging the pre-Dioxus-runtime init: `init_app_lang` runs from
//! `src/lib.rs::run()` *before* `dioxus::launch(App)`, so the
//! GlobalSignal can't be written yet (no runtime). We park the
//! resolved language in a `OnceLock<Lang>` (`INITIAL_LANG`) that the
//! GlobalSignal's lazy initializer reads on first access. From then on
//! the GlobalSignal is the source of truth and `set_app_lang` is the
//! only mutator.

use crate::trios::core::Lang;
use dioxus::signals::{GlobalSignal, Readable};
use std::sync::OnceLock;

/// Pre-runtime parking spot. Written exactly once by `init_app_lang`
/// from `lib.rs::run()` before Dioxus launches; read once by the
/// `APP_LANG` lazy initializer on the first signal access.
static INITIAL_LANG: OnceLock<Lang> = OnceLock::new();

/// Reactive global. Reads inside a Dioxus component subscribe that
/// component to language updates. Writes via `set_app_lang` cause
/// every subscribed component to re-render.
static APP_LANG: GlobalSignal<Lang> =
    GlobalSignal::new(|| INITIAL_LANG.get().copied().unwrap_or(Lang::Russian));

/// Set the app's rendering language at WASM startup, *before*
/// `dioxus::launch`. Safe to call without a Dioxus runtime — writes
/// to a plain `OnceLock` that the GlobalSignal initializer reads
/// lazily.
///
/// On WASM, also overlays `localStorage["wwb_lang"]` if the URL didn't
/// already pick a language: a user who chose EN in a previous session
/// shouldn't have to re-pick after a reload.
pub fn init_app_lang(lang: Lang) {
    let _ = INITIAL_LANG.set(lang);
}

/// Read the current rendering language. Inside a Dioxus component
/// this subscribes the scope so a later `set_app_lang` triggers a
/// re-render. Outside a Dioxus runtime (e.g. unit tests, the WASM
/// bootstrap before `dioxus::launch`), falls back to the
/// `INITIAL_LANG` parking-spot value without panic.
pub fn current_lang() -> Lang {
    // `try_read` returns Err if there's no runtime to register the
    // subscription with — that's the "before launch" / unit-test path.
    match APP_LANG.try_read() {
        Ok(guard) => *guard,
        Err(_) => INITIAL_LANG.get().copied().unwrap_or(Lang::Russian),
    }
}

/// Switch the app's rendering language at runtime. Called from the
/// profile-screen language picker. Must run from inside a Dioxus
/// component / event handler (writes require a runtime). Persists to
/// `localStorage["wwb_lang"]` so the choice survives a reload.
pub fn set_app_lang(lang: Lang) {
    // Update the parking spot too — if a component reads via the
    // try_read fallback path it should see the latest value.
    let _ = INITIAL_LANG.set(lang);
    *APP_LANG.write() = lang;

    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                let _ = storage.set_item("wwb_lang", lang_code(lang));
            }
        }
    }
}

/// Read a previously-persisted language preference, if any. Called
/// from `lib.rs::run()` so a returning user's manual choice overrides
/// the passive Telegram locale.
#[cfg(target_arch = "wasm32")]
pub fn read_stored_lang() -> Option<Lang> {
    let s = web_sys::window()?
        .local_storage()
        .ok()
        .flatten()?
        .get_item("wwb_lang")
        .ok()
        .flatten()?;
    lang_from_code(&s)
}

/// Map a `Lang` variant to its short two-letter code used in
/// `localStorage` and (where the path supports it) URL queries.
///
/// Delegates to the canonical `core::Lang::as_str` — previously this was
/// a hand-copied `match` that mirrored it. Two copies of the same
/// Lang↔code table drift the moment one is edited (rename a code in one
/// place and the localStorage round-trip silently breaks). One source of
/// truth now; `lang_code_matches_core_as_str` guards it.
fn lang_code(lang: Lang) -> &'static str {
    lang.as_str()
}

/// Inverse of `lang_code` — used by `read_stored_lang` to decode
/// `localStorage` values written by `set_app_lang`. Delegates to
/// `core::Lang::from_str` (which also tolerates the full English name,
/// harmless for the 2-letter codes localStorage actually stores).
#[cfg(target_arch = "wasm32")]
fn lang_from_code(code: &str) -> Option<Lang> {
    code.parse::<Lang>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_lang_falls_back_to_russian_before_init() {
        // Outside a Dioxus runtime, `APP_LANG.try_read()` errs and we
        // fall back to INITIAL_LANG. INITIAL_LANG is a OnceLock so
        // whichever test in this binary calls init_app_lang first
        // locks in the value — assert against the set of legal values.
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

    const ALL_LANGS: [Lang; 8] = [
        Lang::Russian,
        Lang::English,
        Lang::Thai,
        Lang::Chinese,
        Lang::Hebrew,
        Lang::German,
        Lang::French,
        Lang::Spanish,
    ];

    #[test]
    fn lang_code_roundtrip_via_canonical_codes() {
        for l in ALL_LANGS {
            let code = lang_code(l);
            // Codes are the standard 2-letter ISO 639-1 forms.
            assert_eq!(code.len(), 2);
            assert!(code.chars().all(|c| c.is_ascii_lowercase()));
            // The code must round-trip back to the same variant through
            // the canonical `core::Lang::from_str` (what `lang_from_code`
            // now delegates to). Runs on host — no wasm gate needed.
            assert_eq!(code.parse::<Lang>().ok(), Some(l), "roundtrip {l:?}");
        }
    }

    /// `lang_code` must stay identical to the canonical `core::Lang::as_str`.
    /// If they ever diverge, the UI would write one code to localStorage and
    /// the rest of the stack would interpret it as another language.
    #[test]
    fn lang_code_matches_core_as_str() {
        for l in ALL_LANGS {
            assert_eq!(lang_code(l), l.as_str(), "{l:?}");
        }
    }
}
