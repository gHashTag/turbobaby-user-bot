//! Full-screen video player popup, opened by the ▶️ button on a product card.
//!
//! This exact overlay (fixed `inset:0` backdrop at z-index 1000, inner box
//! that stops propagation, `<video controls>`, «Закрыть» button) had been
//! hand-copied into four places — `menu_screen`, `accessories_screen`,
//! `sets_screen`, and `admin_screen` — and had already drifted: the admin
//! copy used a 0.8 backdrop and a `<source>` child while the others used
//! 0.85 and a `src` attribute. Inconsistent clones are a documented fault
//! source (Juergens et al., "Do Code Clones Matter?", ICSE 2009), so this
//! consolidates them into one component.
//!
//! `playsinline` / `webkit-playsinline` are set so muted/inline playback
//! behaves inside Telegram/iOS WebViews (same reason the inline card videos
//! carry them) instead of forcing fullscreen.

use crate::trios::i18n::{t, T_MODAL_CLOSE};
use crate::ui::lang;
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct VideoModalProps {
    /// Video source URL (http(s) or root-relative).
    pub url: String,
    /// Called when the user taps the backdrop or the «Закрыть» button.
    pub on_close: EventHandler<()>,
}

#[component]
pub fn VideoModal(props: VideoModalProps) -> Element {
    let lang = lang::current_lang();
    let on_close = props.on_close;
    let url = props.url.clone();
    rsx! {
        div {
            style: "position:fixed;inset:0;background:rgba(0,0,0,0.85);display:flex;align-items:center;justify-content:center;z-index:1000;padding:16px;",
            onclick: move |_| on_close.call(()),
            div {
                style: "background:#1a1a2e;padding:16px;border-radius:8px;max-width:90vw;max-height:80vh;display:flex;flex-direction:column;align-items:center;gap:8px;",
                role: "dialog",
                "aria-modal": "true",
                "aria-label": "Видео",
                // WCAG 2.4.3: move focus into the dialog on open.
                tabindex: "-1",
                onmounted: move |e: Event<MountedData>| {
                    spawn(async move {
                        let _ = e.set_focus(true).await;
                    });
                },
                // WCAG 2.1.2: Escape closes the dialog (keyboard parity with ✕).
                onkeydown: move |e: Event<KeyboardData>| {
                    if e.key() == Key::Escape {
                        on_close.call(());
                    }
                },
                onclick: move |e: Event<MouseData>| e.stop_propagation(),
                video {
                    style: "max-width:100%;max-height:60vh;border-radius:6px;",
                    controls: true,
                    src: "{url}",
                    "playsinline": "true",
                    "webkit-playsinline": "true",
                }
                button {
                    style: "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;cursor:pointer;",
                    onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_close.call(()); },
                    "{t(lang, T_MODAL_CLOSE)}"
                }
            }
        }
    }
}
