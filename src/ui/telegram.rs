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

    /// Get Telegram user ID from WebApp
    /// Note: This is a simplified version. In production, use async JS interop.
    pub fn get_user_id(&self) -> Option<i64> {
        // TODO: Implement proper JS interop for getting user ID
        // For now, return None and rely on server to identify user from init data
        None
    }

    /// Get Telegram user data
    /// Note: This is a simplified version. In production, use async JS interop.
    pub fn get_user_data(&self) -> Option<Value> {
        // TODO: Implement proper JS interop for getting user data
        None
    }

    /// Get Telegram initData string (for server-side validation)
    /// Note: This is a simplified version. In production, use async JS interop.
    pub fn get_init_data(&self) -> String {
        // TODO: Implement proper JS interop for getting init data
        String::new()
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
