//! Force autoplay `<video>` previews to be silent.
//!
//! The card previews already declare `muted: true`, but that only sets the
//! `muted` **content attribute**, and per the HTML spec that attribute reflects
//! `defaultMuted` — not the live mute state. `defaultMuted` seeds `.muted` only
//! when the element is created by the parser. Dioxus builds every node with
//! `document.createElement` and then calls `setAttribute("muted", ...)`, which
//! happens *after* `.muted` was already initialised to `false`. Result: the
//! markup looks muted, the element is not, and the card previews play their
//! audio as soon as they autoplay.
//!
//! [`mute_media_element`] is called from `onmounted` on every autoplaying
//! `<video>` and sets the JS property directly, which is the only thing the
//! playback pipeline actually reads.

use dioxus::prelude::*;

/// Silence a just-mounted media element by setting the live JS properties.
///
/// Sets `.muted` (what the playback pipeline reads), `.defaultMuted` (so the
/// element stays silent if the browser reloads the source), and `.volume = 0`
/// as a belt-and-braces fallback for WebViews that unmute on their own during
/// fullscreen transitions.
///
/// Also re-issues `play()`: a browser that blocked the initial autoplay because
/// the element looked unmuted will now allow it, since muted autoplay is
/// permitted everywhere.
pub fn mute_media_element(evt: &Event<MountedData>) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;

        let Some(el) = evt.downcast::<web_sys::Element>() else {
            return;
        };
        let Ok(media) = el.clone().dyn_into::<web_sys::HtmlMediaElement>() else {
            return;
        };

        media.set_muted(true);
        media.set_default_muted(true);
        media.set_volume(0.0);

        // Autoplay may have been rejected while the element still counted as
        // audible. It is muted now, so the retry is allowed to succeed.
        let _ = media.play();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = evt;
    }
}
