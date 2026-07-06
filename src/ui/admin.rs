//! Admin status helpers for the Mini App UI.
//!
//! Mirrors the check used by `AdminScreen`: a user is considered an admin if
//! `wwb_admin_token` (from password login) is present in localStorage and the
//! backend `/api/admin/check` endpoint confirms it. Falls back to `false`
//! outside Telegram/WebApp or when no token is stored.

use dioxus::prelude::*;

/// Hook that returns a reactive `bool` indicating whether the current user
/// has an active admin session. The value is fetched once per component mount;
/// call it at screen level and pass the boolean down to card renderers.
pub fn use_admin_status() -> Signal<bool> {
    let mut is_admin = use_signal(|| false);

    use_hook(move || {
        spawn(async move {
            #[cfg(target_arch = "wasm32")]
            {
                let window = match web_sys::window() {
                    Some(w) => w,
                    None => return,
                };
                let storage = match window.local_storage().ok().flatten() {
                    Some(s) => s,
                    None => return,
                };
                let token = match storage.get_item("wwb_admin_token").ok().flatten() {
                    Some(t) if !t.is_empty() && t.len() <= 2048 => t,
                    _ => return,
                };
                let telegram_id = storage
                    .get_item("wwb_admin_telegram_id")
                    .ok()
                    .flatten()
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(0);
                if telegram_id == 0 {
                    return;
                }

                let base = crate::ui::api::context::api_base_url();
                let url = format!("{}/api/admin/check?telegram_id={}", base, telegram_id);
                let resp = crate::ui::api::local_client::LocalClient::new()
                    .get(&url)
                    .header("X-Admin-Token", &token)
                    .send()
                    .await;
                let ok = match resp {
                    Ok(r) if r.status().is_success() => {
                        r.json::<serde_json::Value>()
                            .await
                            .ok()
                            .and_then(|v| v.get("is_admin").and_then(|v| v.as_bool()))
                            .unwrap_or(false)
                    }
                    _ => false,
                };
                is_admin.set(ok);
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = is_admin;
            }
        });
    });

    is_admin
}
