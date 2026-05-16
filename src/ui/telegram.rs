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
    /// Reads `window.Telegram.WebApp.initDataUnsafe.user.id` synchronously via
    /// `js_sys::eval`. Returns `None` if SDK is missing or user is not present
    /// (e.g. page opened in a regular browser).
    pub fn get_user_id(&self) -> Option<i64> {
        let js = r#"(function(){try{
            if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.initDataUnsafe && window.Telegram.WebApp.initDataUnsafe.user){
                return window.Telegram.WebApp.initDataUnsafe.user.id || 0;
            }
            return 0;
        }catch(e){return 0;}})()"#;
        let val = js_sys::eval(js).ok()?;
        let n = val.as_f64()?;
        let id = n as i64;
        if id == 0 { None } else { Some(id) }
    }

    /// Get Telegram user data
    /// Note: This is a simplified version. In production, use async JS interop.
    pub fn get_user_data(&self) -> Option<Value> {
        None
    }

    /// Get Telegram initData string (for server-side validation)
    pub fn get_init_data(&self) -> String {
        let js = r#"(function(){try{
            if(window.Telegram && window.Telegram.WebApp && window.Telegram.WebApp.initData){
                return window.Telegram.WebApp.initData || "";
            }
            return "";
        }catch(e){return "";}})()"#;
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
