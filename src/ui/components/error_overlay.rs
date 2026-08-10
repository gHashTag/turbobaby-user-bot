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

    if items.is_empty() {
        return rsx! {};
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
                        background: #fff; color: #ff4757; border: none;
                        padding: 4px 10px; font-weight: 700; cursor: pointer;
                        font-size: 11px; border-radius: 4px;
                    ",
                    onclick: move |_| errors.write().clear(),
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

/// Benign, non-actionable JS errors that must NOT pop the error overlay.
/// `AbortError` / "operation was aborted" / "play() request was interrupted"
/// come from autoplay `<video>` previews on cards: when a card re-renders or
/// scrolls out, the browser cancels the in-flight media load — expected, not a
/// bug. `ResizeObserver loop` is the classic harmless browser warning. These are
/// cancellations, not failures, so filtering them is safe (real fetch failures
/// surface as TypeError/NetworkError, which still show).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn is_benign_js_error(msg: &str) -> bool {
    msg.contains("AbortError")
        || msg.contains("operation was aborted")
        || msg.contains("play() request was interrupted")
        || msg.contains("request is not allowed by the user agent")
        || msg.contains("ResizeObserver loop")
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
                if is_benign_js_error(&full_msg) {
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
            window.set_onerror(Some(onerror.as_ref().unchecked_ref()));
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
                if is_benign_js_error(&reason) {
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
