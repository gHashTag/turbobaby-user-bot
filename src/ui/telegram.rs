// Telegram WebApp SDK wrapper for Dioxus 0.6
//
// Provides access to Telegram Mini App features using document::eval()

use dioxus::prelude::*;
use serde_json::Value;

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
        let _ = document::eval("if(window.Telegram && window.Telegram.WebApp) { window.Telegram.WebApp.ready(); }");
    }

    /// Expand Mini App to full height
    pub fn expand(&self) {
        let _ = document::eval("if(window.Telegram && window.Telegram.WebApp) { window.Telegram.WebApp.expand(); }");
    }

    /// Close Mini App
    pub fn close(&self) {
        let _ = document::eval("if(window.Telegram && window.Telegram.WebApp) { window.Telegram.WebApp.close(); }");
    }

    /// Set main button text and show it
    pub fn set_main_button_text(&self, text: &str) {
        let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
        let _ = document::eval(&format!(
            r#"if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.MainButton) {{ window.Telegram.WebApp.MainButton.setText("{}"); window.Telegram.WebApp.MainButton.show(); }}"#,
            escaped
        ));
    }

    /// Hide main button
    pub fn hide_main_button(&self) {
        let _ = document::eval("if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.MainButton) { window.Telegram.WebApp.MainButton.hide(); }");
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
        let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
        let _ = document::eval(&format!(
            r#"if(window.Telegram && window.Telegram.WebApp) {{ window.Telegram.WebApp.showAlert("{}"); }}"#,
            escaped
        ));
    }

    /// Get Telegram user ID from WebApp.
    /// Tries three sources in order:
    ///   1. `window.Telegram.WebApp.initDataUnsafe.user.id`
    ///   2. parse user from `window.Telegram.WebApp.initData` (URL-encoded)
    ///   3. `?tgid=<id>` query parameter (manual fallback)
    /// Returns `None` only if all three fail (e.g. plain browser).
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
            // 3) ?tgid= manual override
            try{
                var qp = new URLSearchParams(window.location.search);
                var manual = qp.get('tgid');
                if(manual) return manual;
            }catch(e){}
            return '';
        }catch(e){return '';}})()"#;
        let val = js_sys::eval(js).ok()?;
        let s = val.as_string()?;
        if s.is_empty() { return None; }
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
        if s.is_empty() { return None; }
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
        js_sys::eval(js).ok().and_then(|v| v.as_string()).unwrap_or_else(|| "eval_failed".to_string())
    }

    /// Get Telegram user data
    /// Note: This is a simplified version. In production, use async JS interop.
    pub fn get_user_data(&self) -> Option<Value> {
        None
    }

    /// Get Telegram initData string (for server-side validation)
    pub fn get_init_data(&self) -> String {
        let js = r#"(function(){try{
            if(window.Telegram && window.Telegram.WebApp){
                var d = window.Telegram.WebApp.initData;
                console.log('[WWB] initData type:', typeof d, 'len:', d ? d.length : 0);
                if(typeof d === 'string' && d.length > 0) return d;
                // Fallback: reconstruct from initDataUnsafe if initData is missing
                var u = window.Telegram.WebApp.initDataUnsafe;
                if(u && u.hash){
                    var parts = [];
                    var keys = Object.keys(u).filter(function(k){
                        return k !== 'hash' && k !== 'receiver' && k !== 'chat' && u[k] !== undefined && u[k] !== null;
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
                    var reconstructed = parts.join('&');
                    console.log('[WWB] reconstructed initData len:', reconstructed.length);
                    return reconstructed;
                }
            }
            console.log('[WWB] Telegram WebApp not available');
            return "";
        }catch(e){console.error('[WWB] get_init_data error:', e); return "";}})()"#;
        js_sys::eval(js).ok().and_then(|v| v.as_string()).unwrap_or_default()
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

/// Component that initializes Telegram WebApp on mount
#[component]
pub fn TelegramProvider(children: Element) -> Element {
    let _telegram = use_telegram();
    let _ = _telegram.ready();
    let _ = _telegram.expand();

    rsx! {
        { children }
    }
}
