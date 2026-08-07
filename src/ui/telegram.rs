// Telegram WebApp SDK wrapper for Dioxus 0.6
//
// Provides access to Telegram Mini App features using document::eval()

use dioxus::prelude::*;
use gloo_events::EventListener;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HapticStyle {
    Light,
    Medium,
    Heavy,
    Rigid,
    Soft,
}

impl HapticStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Medium => "medium",
            Self::Heavy => "heavy",
            Self::Rigid => "rigid",
            Self::Soft => "soft",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HapticNotification {
    Error,
    Success,
    Warning,
}

impl HapticNotification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Success => "success",
            Self::Warning => "warning",
        }
    }
}

/// Telegram WebApp wrapper using document::eval()
#[derive(Clone, Copy, Debug)]
pub struct TelegramApp;

impl TelegramApp {
    /// Initialize Telegram WebApp
    pub fn init() -> Self {
        let _ = document::eval("if(window.Telegram && window.Telegram.WebApp) { window.Telegram.WebApp.ready(); window.Telegram.WebApp.expand(); }");
        Self
    }

    /// Call WebApp.ready()
    pub fn ready(&self) {
        let _ = document::eval(
            "if(window.Telegram && window.Telegram.WebApp) { window.Telegram.WebApp.ready(); }",
        );
    }

    /// Expand Mini App to full height
    pub fn expand(&self) {
        let _ = document::eval(
            "if(window.Telegram && window.Telegram.WebApp) { window.Telegram.WebApp.expand(); }",
        );
    }

    /// Close Mini App
    pub fn close(&self) {
        let _ = document::eval(
            "if(window.Telegram && window.Telegram.WebApp) { window.Telegram.WebApp.close(); }",
        );
    }

    /// Escape a string for safe injection into a JavaScript double-quoted string literal.
    /// Also defangs `</script>` to prevent breaking out of a `<script>` context.
    fn js_escape(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\'', "\\'")
            .replace('`', "\\`")
            .replace('$', "\\$")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
            .replace('\0', "\\0")
            .replace("</script>", "<\\/script>")
            .replace("</SCRIPT>", "<\\/SCRIPT>")
    }

    /// Set main button text and show it
    pub fn set_main_button_text(&self, text: &str) {
        let escaped = Self::js_escape(text);
        let _ = document::eval(&format!(
            r#"if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.MainButton) {{ window.Telegram.WebApp.MainButton.setText("{}"); window.Telegram.WebApp.MainButton.show(); }}"#,
            escaped
        ));
    }

    /// Hide main button
    pub fn hide_main_button(&self) {
        let _ = document::eval("if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.MainButton) { window.Telegram.WebApp.MainButton.hide(); }");
    }

    /// Enable the main button
    pub fn enable_main_button(&self) {
        let _ = document::eval("if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.MainButton) { window.Telegram.WebApp.MainButton.enable(); }");
    }

    /// Disable the main button
    pub fn disable_main_button(&self) {
        let _ = document::eval("if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.MainButton) { window.Telegram.WebApp.MainButton.disable(); }");
    }

    /// Show the MainButton activity indicator (spinner) and keep the given text.
    /// Call this right before starting an async submit so the user sees the
    /// app is working and can't double-tap.
    pub fn show_main_button_progress(&self, text: &str, leave_active: bool) {
        let escaped = Self::js_escape(text);
        let leave = if leave_active { "true" } else { "false" };
        let _ = document::eval(&format!(
            r#"if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.MainButton) {{ window.Telegram.WebApp.MainButton.setText("{}"); window.Telegram.WebApp.MainButton.showProgress({}); }}"#,
            escaped, leave
        ));
    }

    /// Hide the MainButton activity indicator and restore the given text.
    pub fn hide_main_button_progress(&self, text: &str) {
        let escaped = Self::js_escape(text);
        let _ = document::eval(&format!(
            r#"if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.MainButton) {{ window.Telegram.WebApp.MainButton.hideProgress(); window.Telegram.WebApp.MainButton.setText("{}"); }}"#,
            escaped
        ));
    }

    /// Prevent accidental close while the user is in the middle of a form.
    pub fn enable_closing_confirmation(&self) {
        let _ = document::eval("if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.enableClosingConfirmation){ window.Telegram.WebApp.enableClosingConfirmation(); }");
    }

    /// Re-allow the Telegram swipe-to-close gesture.
    pub fn disable_closing_confirmation(&self) {
        let _ = document::eval("if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.disableClosingConfirmation){ window.Telegram.WebApp.disableClosingConfirmation(); }");
    }

    /// Tint the native Telegram header to match the current screen theme.
    pub fn set_header_color(&self, color: &str) {
        let escaped = Self::js_escape(color);
        let _ = document::eval(&format!(
            r#"if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.setHeaderColor){{ window.Telegram.WebApp.setHeaderColor("{}"); }}"#,
            escaped
        ));
    }

    /// Wire Telegram MainButton.onClick to a DOM CustomEvent that Rust can
    /// listen to reliably. Telegram only exposes a single onClick callback, so
    /// this overwrites any previous JS handler with a dispatcher.
    pub fn enable_main_button_click_dispatch(&self) {
        let _ = document::eval(
            r#"
            (function(){
                if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.MainButton){
                    window.Telegram.WebApp.MainButton.onClick(function(){
                        if(window.dispatchEvent){
                            window.dispatchEvent(new CustomEvent('woody:mainbutton', { bubbles: false }));
                        }
                    });
                }
            })();
        "#,
        );
    }

    /// Show back button
    pub fn show_back_button(&self) {
        let _ = document::eval("if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.BackButton) { window.Telegram.WebApp.BackButton.show(); }");
    }

    /// Hide back button
    pub fn hide_back_button(&self) {
        let _ = document::eval("if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.BackButton) { window.Telegram.WebApp.BackButton.hide(); }");
    }

    /// Trigger haptic feedback impact
    pub fn haptic_impact(&self, style: HapticStyle) {
        let _ = document::eval(&format!(
            r#"if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.HapticFeedback) {{ window.Telegram.WebApp.HapticFeedback.impactOccurred("{}"); }}"#,
            style.as_str()
        ));
    }

    /// Trigger haptic feedback notification
    pub fn haptic_notification(&self, notification: HapticNotification) {
        let _ = document::eval(&format!(
            r#"if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.HapticFeedback) {{ window.Telegram.WebApp.HapticFeedback.notificationOccurred("{}"); }}"#,
            notification.as_str()
        ));
    }

    /// Show alert/popup
    pub fn show_alert(&self, message: &str) {
        let escaped = Self::js_escape(message);
        let _ = document::eval(&format!(
            r#"if(window.Telegram && window.Telegram.WebApp) {{ window.Telegram.WebApp.showAlert("{}"); }}"#,
            escaped
        ));
    }

    /// Request permission for the bot to send messages to the user. This is the
    /// Telegram-native gate that enables order-status push notifications.
    /// Returns `true` if granted; outside Telegram or on cancellation returns `false`.
    pub async fn request_write_access(&self) -> bool {
        let js = r#"new Promise((resolve) => {
            try {
                if (window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.requestWriteAccess) {
                    window.Telegram.WebApp.requestWriteAccess(function(granted){
                        resolve(granted === true);
                    });
                } else {
                    resolve(false);
                }
            } catch(e) { resolve(false); }
        })"#;
        let Some(promise_val) = js_sys::eval(js).ok() else {
            return false;
        };
        let Ok(promise) = promise_val.dyn_into::<js_sys::Promise>() else {
            return false;
        };
        let Ok(result) = wasm_bindgen_futures::JsFuture::from(promise).await else {
            return false;
        };
        result.as_bool().unwrap_or(false)
    }

    /// Get Telegram user ID from WebApp.
    ///
    /// Tries two sources in order:
    ///   1. `window.Telegram.WebApp.initDataUnsafe.user.id`
    ///   2. parse user from `window.Telegram.WebApp.initData` (URL-encoded)
    ///
    /// Returns `None` if Telegram WebApp is not available (e.g. plain browser).
    /// NOTE: `?tgid=` query fallback removed to prevent URL-based impersonation.
    pub fn get_user_id(&self) -> Option<i64> {
        // Returns the id as a STRING (so we don't lose precision on big ints)
        // or empty string if not found.
        let js = r#"(function(){try{
            // 1) initDataUnsafe.user.id
            if(window.Telegram && window.Telegram.WebApp){
                var w = window.Telegram.WebApp;
                if(w.initDataUnsafe && w.initDataUnsafe.user && w.initDataUnsafe.user.id){
                    return String(w.initDataUnsafe.user.id);
                }
                // 2) parse initData query string
                if(w.initData){
                    try{
                        var params = new URLSearchParams(w.initData);
                        var userStr = params.get('user');
                        if(userStr){
                            var u = JSON.parse(userStr);
                            if(u && u.id) return String(u.id);
                        }
                    }catch(e){}
                }
            }
            return '';
        }catch(e){return '';}})()"#;
        let val = js_sys::eval(js).ok()?;
        let s = val.as_string()?;
        if s.is_empty() {
            return None;
        }
        s.parse::<i64>().ok().filter(|id| *id != 0)
    }

    /// Get Telegram username from initDataUnsafe
    pub fn get_username(&self) -> Option<String> {
        let js = r#"(function(){try{
            if(window.Telegram && window.Telegram.WebApp){
                var u = window.Telegram.WebApp.initDataUnsafe;
                if(u && u.user && u.user.username) return u.user.username;
            }
            return '';
        }catch(e){return '';}})()"#;
        let val = js_sys::eval(js).ok()?;
        let s = val.as_string()?;
        if s.is_empty() {
            return None;
        }
        Some(s)
    }

    /// Telegram-client locale code (`"ru"`, `"en"`, `"th"`, …) read from
    /// `initDataUnsafe.user.language_code`. Cycle #71 — used by the
    /// checkout screen to localise error banners on mobile where opening
    /// `?lang=xx` URLs by hand is unfriendly.
    ///
    /// Returns `None` outside the Telegram WebApp (e.g. plain-browser
    /// preview) or when the user object simply doesn't carry the code.
    pub fn get_language_code(&self) -> Option<String> {
        let js = r#"(function(){try{
            if(window.Telegram && window.Telegram.WebApp){
                var u = window.Telegram.WebApp.initDataUnsafe;
                if(u && u.user && u.user.language_code) return u.user.language_code;
            }
            return '';
        }catch(e){return '';}})()"#;
        let val = js_sys::eval(js).ok()?;
        let s = val.as_string()?;
        if s.is_empty() {
            return None;
        }
        Some(s)
    }

    /// Diagnostic dump of what is actually available in Telegram.WebApp.
    /// Used by the admin screen to show why authentication failed.
    pub fn debug_dump(&self) -> String {
        let js = r#"(function(){try{
            var out = {};
            out.hasTelegram = !!window.Telegram;
            out.hasWebApp = !!(window.Telegram && window.Telegram.WebApp);
            if(window.Telegram && window.Telegram.WebApp){
                var w = window.Telegram.WebApp;
                out.version = w.version || null;
                out.platform = w.platform || null;
                out.initDataLen = (w.initData || '').length;
                out.hasInitDataUnsafe = !!w.initDataUnsafe;
                out.hasUser = !!(w.initDataUnsafe && w.initDataUnsafe.user);
                if(w.initDataUnsafe && w.initDataUnsafe.user){
                    out.userId = String(w.initDataUnsafe.user.id || '');
                    out.username = w.initDataUnsafe.user.username || null;
                }
            }
            return JSON.stringify(out);
        }catch(e){return 'err:'+String(e);}})()"#;
        js_sys::eval(js)
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| "eval_failed".to_string())
    }

    /// Get Telegram user data
    /// Note: This is a simplified version. In production, use async JS interop.
    pub fn get_user_data(&self) -> Option<Value> {
        None
    }

    /// Read the `start_param` Telegram passes when the Mini App is opened via a
    /// `t.me/{bot}?startapp=...` deep link. Empty/missing values return `None`.
    pub fn start_param(&self) -> Option<String> {
        let js = r#"(function(){try{
            if(window.Telegram && window.Telegram.WebApp){
                var p = window.Telegram.WebApp.initDataUnsafe;
                if(p && typeof p.start_param === 'string' && p.start_param.length > 0){
                    return p.start_param;
                }
            }
            return '';
        }catch(e){return '';}})()"#;
        let val = js_sys::eval(js).ok()?;
        let s = val.as_string()?;
        if s.is_empty() {
            return None;
        }
        Some(s)
    }

    /// Read the bot username from Telegram initData. Falls back to the
    /// compile-time constant in `share.rs` if the WebApp SDK is unavailable.
    pub fn bot_username(&self) -> Option<String> {
        let js = r#"(function(){try{
            if(window.Telegram && window.Telegram.WebApp){
                var p = window.Telegram.WebApp.initDataUnsafe;
                if(p && p.bot && typeof p.bot.username === 'string' && p.bot.username.length > 0){
                    return p.bot.username;
                }
            }
            return '';
        }catch(e){return '';}})()"#;
        let val = js_sys::eval(js).ok()?;
        let s = val.as_string()?;
        if s.is_empty() {
            return None;
        }
        Some(s)
    }

    /// Read a value from Telegram WebApp CloudStorage.
    /// Returns `None` if the value is missing or CloudStorage is unavailable.
    ///
    /// CloudStorage is async; this helper awaits the JS callback so the WASM
    /// event loop stays free. Do not call from a synchronous context.
    pub async fn cloud_storage_get(&self, key: &str) -> Option<String> {
        let escaped = Self::js_escape(key);
        let js = format!(
            r#"new Promise((resolve) => {{
                try{{
                    if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.CloudStorage){{
                        window.Telegram.WebApp.CloudStorage.getItem("{}", function(err, value){{
                            resolve((!err && typeof value === 'string') ? value : '');
                        }});
                    }} else {{
                        resolve('');
                    }}
                }}catch(e){{ resolve(''); }}
            }})"#,
            escaped
        );
        let promise_val = js_sys::eval(&js).ok()?;
        let promise = promise_val.dyn_into::<js_sys::Promise>().ok()?;
        let result = wasm_bindgen_futures::JsFuture::from(promise).await.ok()?;
        result.as_string().filter(|s| !s.is_empty())
    }

    /// Write a value to Telegram WebApp CloudStorage.
    pub fn cloud_storage_set(&self, key: &str, value: &str) {
        let escaped_key = Self::js_escape(key);
        let escaped_value = Self::js_escape(value);
        let _ = document::eval(&format!(
            r#"if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.CloudStorage){{ window.Telegram.WebApp.CloudStorage.setItem("{}", "{}"); }}"#,
            escaped_key, escaped_value
        ));
    }

    /// Remove a value from Telegram WebApp CloudStorage.
    pub fn cloud_storage_remove(&self, key: &str) {
        let escaped_key = Self::js_escape(key);
        let _ = document::eval(&format!(
            r#"if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.CloudStorage){{ window.Telegram.WebApp.CloudStorage.removeItem("{}"); }}"#,
            escaped_key
        ));
    }

    /// Get Telegram initData string (for server-side validation)
    pub fn get_init_data(&self) -> String {
        let js = r#"(function(){try{
            if(window.Telegram && window.Telegram.WebApp){
                var w = window.Telegram.WebApp;
                var d = w.initData;
                if(typeof d === 'string' && d.length > 0) return d;
                // Fallback: reconstruct from initDataUnsafe if initData string is missing.
                // Some WebViews populate initDataUnsafe before initData. We build the
                // data_check_string the same way Telegram does (sorted keys, user JSON
                // URL-encoded) and append the provided hash so the server can validate.
                var u = w.initDataUnsafe;
                if(u && u.hash){
                    var parts = [];
                    var keys = Object.keys(u).filter(function(k){
                        return k !== 'hash' && k !== 'receiver' && k !== 'chat'
                            && u[k] !== undefined && u[k] !== null;
                    });
                    keys.sort();
                    keys.forEach(function(k){
                        if(k === 'user'){
                            parts.push('user=' + encodeURIComponent(JSON.stringify(u[k])));
                        } else {
                            parts.push(k + '=' + encodeURIComponent(String(u[k])));
                        }
                    });
                    parts.push('hash=' + encodeURIComponent(u.hash));
                    return parts.join('&');
                }
            }
            return "";
        }catch(e){ return "";}})()"#;
        js_sys::eval(js)
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default()
    }
}

/// Hook to access Telegram instance
pub fn use_telegram() -> TelegramApp {
    TelegramApp::init()
}

/// Hook to get current Telegram user ID
pub fn use_telegram_id() -> Option<i64> {
    use_telegram().get_user_id()
}

/// Hook to get current Telegram username
pub fn use_telegram_username() -> Option<String> {
    use_telegram().get_username()
}

/// Hook to get raw initData string for server validation
pub fn use_telegram_init_data() -> String {
    use_telegram().get_init_data()
}

/// Hook that invokes the provided callback when the Telegram MainButton is
/// clicked. The Telegram SDK only exposes a single `MainButton.onClick` JS
/// callback, so we bridge it through a `woody:mainbutton` CustomEvent that the
/// Rust side listens to via `gloo_events`. The JS dispatcher is overwritten on
/// every mount so the active screen always controls the button.
pub fn use_main_button_click<F: FnMut() + 'static>(callback: F) {
    use_hook_with_cleanup(
        move || {
            TelegramApp::init().enable_main_button_click_dispatch();
            let cb = Rc::new(RefCell::new(callback));
            let win = web_sys::window()?;
            let listener =
                EventListener::new(&win, "woody:mainbutton", move |_event: &web_sys::Event| {
                    cb.borrow_mut()();
                });
            Some(Rc::new(listener))
        },
        |_listener: Option<Rc<EventListener>>| {},
    );
}

/// Component that initializes Telegram WebApp on mount
#[component]
pub fn TelegramProvider(children: Element) -> Element {
    let _telegram = use_telegram();
    _telegram.ready();
    _telegram.expand();

    rsx! {
        { children }
    }
}
