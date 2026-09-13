// Lazy Screen Loader — deferred rendering wrapper
//
// Wraps any screen component with a configurable delay so the browser can
// paint a lightweight skeleton *before* the heavy screen mounts.
// The `heavy` flag increases the delay and yields extra time to the event loop
// for large screens (Admin, Ride/Game, TechTree) that otherwise cause jank.

use dioxus::prelude::*;

/// Lazy-loading wrapper for screen components.
///
/// On the very first render it shows a shimmer skeleton.
/// After `delay_ms` (or 80 ms when `heavy` is true) it swaps in the real
/// `children`, giving the perceived effect of lazy loading without requiring
/// WASM code-splitting (which Rust cannot do yet).
#[component]
pub fn LazyScreen(
    #[props(default = 16)] delay_ms: u32,
    #[props(default = false)] heavy: bool,
    children: Element,
) -> Element {
    let mut ready = use_signal(|| false);

    let actual_delay = if heavy { 80 } else { delay_ms };

    // Spawn a micro-delay that yields to the browser's paint cycle.
    // `use_future` runs once on mount (no reactive deps ⇒ no re-runs).
    use_future(move || async move {
        gloo_timers::future::TimeoutFuture::new(actual_delay).await;
        if heavy {
            // Heavy screens get an extra tick so the browser can process
            // any pending layout/paint work before we drop a large subtree.
            gloo_timers::future::TimeoutFuture::new(16).await;
        }
        ready.set(true);
    });

    if ready() {
        rsx! { {children} }
    } else {
        rsx! {
            div {
                style: "
                    min-height: 100vh;
                    background: #0f0f1a;
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    justify-content: center;
                    padding: 24px;
                    gap: 16px;
                ",

                // ── Spinner ──────────────────────────────────
                div {
                    style: "
                        width: 40px; height: 40px;
                        border: 3px solid #2a2a4a;
                        border-top-color: #39ff14;
                        border-radius: 50%;
                        animation: lazy-spin 0.8s linear infinite;
                    ",
                }

                // ── Skeleton bars ────────────────────────────
                div {
                    style: "
                        display: flex;
                        flex-direction: column;
                        gap: 8px;
                        width: 200px;
                    ",

                    div {
                        style: "
                            height: 14px;
                            width: 80%;
                            background: linear-gradient(90deg, #1e1e3a 25%, #2a2a4a 50%, #1e1e3a 75%);
                            background-size: 200% 100%;
                            border-radius: 6px;
                            animation: lazy-shimmer 1.5s ease-in-out infinite;
                        ",
                    }
                    div {
                        style: "
                            height: 14px;
                            width: 60%;
                            background: linear-gradient(90deg, #1e1e3a 25%, #2a2a4a 50%, #1e1e3a 75%);
                            background-size: 200% 100%;
                            border-radius: 6px;
                            animation: lazy-shimmer 1.5s ease-in-out infinite;
                        ",
                    }
                    div {
                        style: "
                            height: 14px;
                            width: 45%;
                            background: linear-gradient(90deg, #1e1e3a 25%, #2a2a4a 50%, #1e1e3a 75%);
                            background-size: 200% 100%;
                            border-radius: 6px;
                            animation: lazy-shimmer 1.5s ease-in-out infinite;
                        ",
                    }
                }

                // ── Inline keyframes (scoped, no external CSS needed) ──
                style { {r#"
                    @keyframes lazy-spin {
                        to { transform: rotate(360deg); }
                    }
                    @keyframes lazy-shimmer {
                        0%   { background-position: 200% 0; }
                        100% { background-position: -200% 0; }
                    }
                "#} }
            }
        }
    }
}
