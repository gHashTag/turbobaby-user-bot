use dioxus::prelude::*;
use web_sys::Event;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;

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

fn push_error(mut errors: Signal<Vec<JsErrorItem>>, mut item: JsErrorItem) {
    item.message.truncate(MAX_MSG_LEN);
    if let Some(ref mut s) = item.stack {
        s.truncate(MAX_STACK_LEN);
    }
    let mut vec = errors.write();
    vec.push(item);
    if vec.len() > MAX_ERRORS {
        vec.remove(0);
    }
}

/// Install global JS error handlers and Rust panic hook.
/// Call this once inside App or a top-level provider.
pub fn install_error_handlers(errors: Signal<Vec<JsErrorItem>>) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            // window.onerror
            let errors_clone = errors.clone();
            let onerror = Closure::wrap(Box::new(move |event: Event| {
                let msg = js_sys::Reflect::get(&event, &"message".into())
                    .ok().and_then(|v| v.as_string()).unwrap_or_else(|| "Unknown error".into());
                let filename = js_sys::Reflect::get(&event, &"filename".into())
                    .ok().and_then(|v| v.as_string()).unwrap_or_default();
                let lineno = js_sys::Reflect::get(&event, &"lineno".into())
                    .ok().and_then(|v| v.as_f64()).map(|v| v as u32).unwrap_or(0);
                let colno = js_sys::Reflect::get(&event, &"colno".into())
                    .ok().and_then(|v| v.as_f64()).map(|v| v as u32).unwrap_or(0);
                let stack = get_stack_from_event(&event);
                let full_msg = format!("{} at {}:{}:{}", msg, filename, lineno, colno);
                push_error(errors_clone.clone(), JsErrorItem {
                    id: js_sys::Date::now() as u64,
                    message: full_msg,
                    stack,
                    source: "window.onerror".into(),
                });
            }) as Box<dyn FnMut(_)>);
            window.set_onerror(Some(onerror.as_ref().unchecked_ref()));
            onerror.forget();

            // unhandledrejection
            let errors_clone = errors.clone();
            let onunhandled = Closure::wrap(Box::new(move |event: Event| {
                let reason = js_sys::Reflect::get(&event, &"reason".into())
                    .ok().and_then(|v| v.as_string())
                    .unwrap_or_else(|| "Promise rejected".into());
                push_error(errors_clone.clone(), JsErrorItem {
                    id: js_sys::Date::now() as u64,
                    message: reason,
                    stack: None,
                    source: "unhandledrejection".into(),
                });
            }) as Box<dyn FnMut(_)>);
            window.add_event_listener_with_callback("unhandledrejection", onunhandled.as_ref().unchecked_ref()).ok();
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
        .ok().and_then(|v| v.as_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn get_stack_from_event(_event: &Event) -> Option<String> {
    None
}
