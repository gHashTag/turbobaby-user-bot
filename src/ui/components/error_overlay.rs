use dioxus::prelude::*;
use serde_json::json;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;
use web_sys::Event;

use crate::ui::api::{context::api_base_url, http::post_json_status_only};

#[derive(Clone, Debug)]
pub struct JsErrorItem {
    pub id: u64,
    pub message: String,
    pub stack: Option<String>,
    pub source: String,
}

/// Global error overlay that catches JS runtime errors and unhandled promise rejections.
/// Displays them as a fixed red banner at the top of the screen.
#[component]
pub fn ErrorOverlay() -> Element {
    let mut errors = use_context::<Signal<Vec<JsErrorItem>>>();
    let items = errors.read().clone();
    // Collapsed by default. The expanded panel is `position:fixed` across the
    // top half of the screen at z-index 99999, so while it is open it swallows
    // every tap underneath — which turned reported errors into an unusable
    // shop twice. A diagnostic must never cost the customer the checkout
    // button, so it now sits as a small badge until someone asks to read it.
    let mut expanded = use_signal(|| false);

    if items.is_empty() {
        return rsx! {};
    }

    if !expanded() {
        return rsx! {
            button {
                style: "
                    position: fixed; top: 6px; right: 6px; z-index: 99999;
                    background: #ff4757; color: #fff; border: none;
                    border-radius: 14px; padding: 4px 10px;
                    font-family: 'Inter', monospace; font-size: 11px; font-weight: 700;
                    box-shadow: 0 2px 6px rgba(0,0,0,0.4); cursor: pointer;
                ",
                onclick: move |_| expanded.set(true),
                "🚨 {items.len()}"
            }
        };
    }

    rsx! {
        div {
            style: "
                position: fixed; top: 0; left: 0; right: 0; z-index: 99999;
                background: #ff4757; color: #fff; padding: 12px;
                font-family: 'Inter', monospace; font-size: 12px;
                max-height: 50vh; overflow-y: auto; overflow-x: hidden;
                box-shadow: 0 4px 12px rgba(0,0,0,0.5);
            ",
            div {
                style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; border-bottom: 1px solid rgba(255,255,255,0.3); padding-bottom: 8px;",
                span { style: "font-weight: 700; font-size: 13px;", "🚨 {items.len()} browser error(s)" }
                button {
                    style: "
                        background: rgba(255,255,255,0.2); color: #fff; border: none;
                        padding: 4px 10px; font-weight: 700; cursor: pointer;
                        font-size: 11px; border-radius: 4px; margin-left: auto; margin-right: 8px;
                    ",
                    onclick: move |_| expanded.set(false),
                    "▲ Свернуть"
                }
                button {
                    style: "
                        background: #fff; color: #ff4757; border: none;
                        padding: 4px 10px; font-weight: 700; cursor: pointer;
                        font-size: 11px; border-radius: 4px;
                    ",
                    onclick: move |_| {
                        errors.write().clear();
                        expanded.set(false);
                    },
                    "✕ Clear All"
                }
            }
            for err in items {
                div {
                    key: "{err.id}",
                    style: "border-top: 1px solid rgba(255,255,255,0.2); padding: 8px 0; word-break: break-word;",
                    div { style: "font-weight: 700; margin-bottom: 2px;", "[{err.source}] {err.message}" }
                    if let Some(ref stack) = err.stack {
                        pre {
                            style: "
                                margin: 4px 0 0; font-size: 10px;
                                white-space: pre-wrap; word-break: break-all;
                                color: #ffe6e6; max-height: 120px; overflow-y: auto;
                                background: rgba(0,0,0,0.2); padding: 6px; border-radius: 4px;
                            ",
                            "{stack}"
                        }
                    }
                }
            }
        }
    }
}

const MAX_ERRORS: usize = 50;
const MAX_MSG_LEN: usize = 2000;
const MAX_STACK_LEN: usize = 5000;

/// Truncate to at most `max_chars` Unicode scalar values. UTF-8-safe —
/// unlike `String::truncate(N)` which panics if byte N lands inside a
/// multi-byte sequence. `crate::util::truncate_string` is the backend-side
/// twin; this is the WASM-side copy because `util` is `#[cfg(not(wasm))]`.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect()
    }
}

fn push_error(mut errors: Signal<Vec<JsErrorItem>>, mut item: JsErrorItem) {
    // Cycle #75: was `String::truncate(N)` — panics when byte N lands
    // mid-codepoint. JS error messages and stack traces routinely
    // contain non-ASCII (cyrillic property names, emoji in user data),
    // so a runtime error overlay that itself panics on its own input
    // is exactly the worst failure mode.
    item.message = truncate_chars(&item.message, MAX_MSG_LEN);
    if let Some(ref mut s) = item.stack {
        *s = truncate_chars(s, MAX_STACK_LEN);
    }
    let mut vec = errors.write();
    vec.push(item);
    if vec.len() > MAX_ERRORS {
        vec.remove(0);
    }
}

#[cfg(target_arch = "wasm32")]
fn send_error_telemetry(item: &JsErrorItem) {
    let payload = json!({
        "source": item.source,
        "message": item.message,
        "stack": item.stack,
        "url_path": web_sys::window()
            .and_then(|w| w.location().pathname().ok())
            .unwrap_or_default(),
        "user_agent": web_sys::window()
            .and_then(|w| w.navigator().user_agent().ok())
            .unwrap_or_default(),
    })
    .to_string();
    let url = format!("{}/api/client-errors", api_base_url());
    spawn_local(async move {
        // Fire-and-forget: telemetry must not block the UI or re-throw.
        let _ = post_json_status_only(&url, &payload).await;
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn send_error_telemetry(_item: &JsErrorItem) {}

/// Report a Rust panic through the same pipe as a JS error.
///
/// The panic hook in `src/lib.rs` printed `PANIC: {info}` — with the file, the
/// line and the message — to the console and to a full-screen overlay, and sent
/// **none of it** anywhere. The only thing that reached telemetry was
/// `window.onerror`, which for a wasm panic under `panic = "abort"` sees
/// nothing but
///
/// ```text
/// RuntimeError: Unreachable code should not be executed
/// ```
///
/// So a production panic report named the URL and nothing else: no message, no
/// file, no line. The one artefact that says what actually broke was the one
/// artefact not collected.
///
/// `pub` and free-standing so the hook, which runs before any component exists
/// and cannot hold a `Signal`, can call it.
#[cfg(target_arch = "wasm32")]
pub fn report_panic(message: String, location: String) {
    send_error_telemetry(&JsErrorItem {
        id: js_sys::Date::now() as u64,
        message: format!("{message} at {location}"),
        // A wasm panic has no JS stack worth keeping — the Rust location is
        // the stack. Kept as a distinct source so these are separable from
        // ordinary JS noise when reading the table.
        stack: Some(location),
        source: "rust.panic".into(),
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub fn report_panic(_message: String, _location: String) {}

/// Install global JS error handlers and Rust panic hook.
/// Call this once inside App or a top-level provider.
pub fn install_error_handlers(errors: Signal<Vec<JsErrorItem>>) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            // Tell the pre-boot catcher in index.html to stand down. It exists
            // only to diagnose a wasm module that never loads; from here the
            // Rust overlay reports errors, and it filters benign cancellations.
            // Leaving both active is what let a harmless AbortError open a
            // fixed, tap-swallowing panel across the top of every screen.
            let _ = js_sys::Reflect::set(
                &window,
                &wasm_bindgen::JsValue::from_str("__wasm_booted"),
                &wasm_bindgen::JsValue::from_bool(true),
            );
            // window.onerror
            let errors_clone = errors;
            let onerror = Closure::wrap(Box::new(move |event: Event| {
                let msg = js_sys::Reflect::get(&event, &"message".into())
                    .ok()
                    .and_then(|v| v.as_string())
                    .unwrap_or_else(|| "Unknown error".into());
                let filename = js_sys::Reflect::get(&event, &"filename".into())
                    .ok()
                    .and_then(|v| v.as_string())
                    .unwrap_or_default();
                let lineno = js_sys::Reflect::get(&event, &"lineno".into())
                    .ok()
                    .and_then(|v| v.as_f64())
                    .map(|v| v as u32)
                    .unwrap_or(0);
                let colno = js_sys::Reflect::get(&event, &"colno".into())
                    .ok()
                    .and_then(|v| v.as_f64())
                    .map(|v| v as u32)
                    .unwrap_or(0);
                let stack = get_stack_from_event(&event);
                let full_msg = format!("{} at {}:{}:{}", msg, filename, lineno, colno);
                if !crate::trios::js_errors::onerror_is_reportable(&msg, &filename, lineno, colno) {
                    return;
                }
                let item = JsErrorItem {
                    id: js_sys::Date::now() as u64,
                    message: full_msg,
                    stack,
                    source: "window.onerror".into(),
                };
                send_error_telemetry(&item);
                push_error(errors_clone, item);
            }) as Box<dyn FnMut(_)>);
            // Registered as a listener, not as `window.onerror`.
            //
            // The `onerror` *property* is invoked with five positional
            // arguments — (message, source, lineno, colno, error) — but this
            // closure takes a single value, so it received the message string
            // and then read `.message` / `.filename` / `.lineno` off a string,
            // which have no such properties. Every real error therefore
            // arrived as "Unknown error at :0:0": thirteen of them were logged
            // against /cart on 2026-08-10, stripped of the very text needed to
            // diagnose them, and the resulting noise opened the tap-blocking
            // error panel.
            //
            // `addEventListener("error", …)` delivers a real `ErrorEvent`, so
            // the fields below are populated. It also fires for failed
            // resource loads, where there genuinely is no message — those are
            // the opaque case the filter is for.
            let _ =
                window.add_event_listener_with_callback("error", onerror.as_ref().unchecked_ref());
            onerror.forget();

            // unhandledrejection
            let errors_clone = errors;
            let onunhandled = Closure::wrap(Box::new(move |event: Event| {
                let reason_val = js_sys::Reflect::get(&event, &"reason".into()).ok();
                let reason = reason_val
                    .as_ref()
                    .and_then(|v| {
                        // String reason → use directly.
                        if let Some(s) = v.as_string() {
                            return Some(s);
                        }
                        // Error / DOMException object → "Name: message".
                        let name = js_sys::Reflect::get(v, &"name".into())
                            .ok()
                            .and_then(|x| x.as_string());
                        let message = js_sys::Reflect::get(v, &"message".into())
                            .ok()
                            .and_then(|x| x.as_string());
                        match (name, message) {
                            (Some(n), Some(m)) => Some(format!("{n}: {m}")),
                            (Some(n), None) => Some(n),
                            (None, Some(m)) => Some(m),
                            _ => None,
                        }
                    })
                    .unwrap_or_else(|| "Promise rejected".into());
                if crate::trios::js_errors::is_benign_js_error(&reason) {
                    return;
                }
                let item = JsErrorItem {
                    id: js_sys::Date::now() as u64,
                    message: reason,
                    stack: None,
                    source: "unhandledrejection".into(),
                };
                send_error_telemetry(&item);
                push_error(errors_clone, item);
            }) as Box<dyn FnMut(_)>);
            window
                .add_event_listener_with_callback(
                    "unhandledrejection",
                    onunhandled.as_ref().unchecked_ref(),
                )
                .ok();
            onunhandled.forget();

            // Note: std::panic::set_hook requires Send+Sync which Signal doesn't
            // implement on wasm32. JS runtime errors (including Rust panics mapped
            // to RuntimeError) are already caught by window.onerror above.
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn get_stack_from_event(event: &Event) -> Option<String> {
    let error = js_sys::Reflect::get(event, &"error".into()).ok()?;
    if error.is_undefined() || error.is_null() {
        return None;
    }
    js_sys::Reflect::get(&error, &"stack".into())
        .ok()
        .and_then(|v| v.as_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn get_stack_from_event(_event: &Event) -> Option<String> {
    None
}
