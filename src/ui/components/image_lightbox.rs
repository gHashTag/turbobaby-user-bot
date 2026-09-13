//! Full-screen image lightbox with pinch-free zoom.
//!
//! The mini-app viewport disables user scaling (`maximum-scale=1.0,
//! user-scalable=no`) so the browser pinch-zoom gesture is unavailable.  This
//! overlay provides an app-controlled zoom instead: tap +/−, double-tap to
//! toggle, and tap the backdrop or ✕ to close.  The image is rendered at the
//! largest size that fits the screen, then scaled up so users can read fine
//! print (e.g. model names on promo art).

use dioxus::prelude::*;

const MIN_ZOOM: f64 = 1.0;
const MAX_ZOOM: f64 = 3.0;
const ZOOM_STEP: f64 = 0.5;

#[derive(Props, PartialEq, Clone)]
pub struct ImageLightboxProps {
    /// Image source URL.
    pub src: String,
    /// Accessible label (usually the product name).
    pub alt: String,
    /// Called when the user taps the backdrop, ✕, or hits Escape.
    pub on_close: EventHandler<()>,
}

#[component]
pub fn ImageLightbox(props: ImageLightboxProps) -> Element {
    let on_close = props.on_close;
    let src = props.src.clone();
    let alt = props.alt.clone();
    let mut zoom = use_signal(|| MIN_ZOOM);

    let zoom_in = move |_| {
        zoom.set((zoom() + ZOOM_STEP).min(MAX_ZOOM));
    };
    let zoom_out = move |_| {
        zoom.set((zoom() - ZOOM_STEP).max(MIN_ZOOM));
    };
    let toggle_zoom = move |e: Event<MouseData>| {
        e.stop_propagation();
        let next = if zoom() > MIN_ZOOM { MIN_ZOOM } else { 2.0 };
        zoom.set(next);
    };

    rsx! {
        div {
            style: "position:fixed;inset:0;background:rgba(0,0,0,0.92);display:flex;align-items:center;justify-content:center;z-index:1100;padding:16px;overflow:hidden;",
            onclick: move |_| on_close.call(()),
            role: "dialog",
            "aria-modal": "true",
            "aria-label": "Увеличенное изображение",
            tabindex: "-1",
            onmounted: move |e: Event<MountedData>| {
                spawn(async move {
                    let _ = e.set_focus(true).await;
                });
            },
            onkeydown: move |e: Event<KeyboardData>| {
                if e.key() == Key::Escape {
                    on_close.call(());
                }
            },

            // Zoomed image — double tap toggles between fit and 2x.
            div {
                style: "max-width:100%;max-height:100%;display:flex;align-items:center;justify-content:center;transition:transform 0.2s ease;transform:scale({zoom()});",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),
                ondoubleclick: toggle_zoom,
                img {
                    src: "{src}",
                    alt: "{alt}",
                    style: "max-width:100%;max-height:85vh;object-fit:contain;display:block;cursor:zoom-in;"
                }
            }

            // Close (✕)
            button {
                style: "position:absolute;top:12px;right:12px;width:48px;height:48px;border-radius:50%;background:rgba(0,0,0,0.6);border:2px solid #fff;color:#fff;font-size:20px;line-height:1;display:flex;align-items:center;justify-content:center;cursor:pointer;z-index:2;",
                "aria-label": "Закрыть",
                onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_close.call(()); },
                "✕"
            }

            // Zoom controls (bottom-right).
            div {
                style: "position:absolute;bottom:20px;right:16px;display:flex;gap:10px;z-index:2;",
                button {
                    style: "width:48px;height:48px;border-radius:50%;background:rgba(0,0,0,0.6);border:2px solid #fff;color:#fff;font-size:22px;font-weight:800;display:flex;align-items:center;justify-content:center;cursor:pointer;",
                    "aria-label": "Уменьшить",
                    onclick: zoom_out,
                    "−"
                }
                button {
                    style: "width:48px;height:48px;border-radius:50%;background:rgba(0,0,0,0.6);border:2px solid #fff;color:#fff;font-size:22px;font-weight:800;display:flex;align-items:center;justify-content:center;cursor:pointer;",
                    "aria-label": "Увеличить",
                    onclick: zoom_in,
                    "+"
                }
            }
        }
    }
}
