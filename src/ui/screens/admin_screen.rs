// Admin Mini-App — full CRUD for all product categories.
//
// Architecture: Optimistic UI (like Apollo).
// - Signal<Vec<T>> is the local cache (source of truth for rendering)
// - use_resource populates the cache on initial load
// - Mutations update the cache IMMEDIATELY (optimistic)
// - API requests happen in the background
// - On error, the cache change is rolled back
// - No full refetch after mutations = instant UI

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::cell::RefCell;
use std::sync::LazyLock;
use wasm_bindgen::JsCast;
use web_sys;

// Drop-in replacement for `LazyLock<reqwest::Client>`. The 54 call sites use
// the reqwest builder shape verbatim (.get/.post/.put/.delete().header().json().send())
// which `LocalClient` mirrors, so the diff is single-line. Saves ≈300 KB
// from the wasm bundle vs shipping reqwest itself.
static HTTP_CLIENT: LazyLock<crate::ui::api::local_client::LocalClient> =
    LazyLock::new(crate::ui::api::local_client::LocalClient::new);

// Synchronous in-memory cache for the admin token. Telegram CloudStorage is
// async, but most admin API calls (uploads, events, broadcasts, CRUD) need to
// read the token synchronously from async callbacks / non-render helpers. The
// cache is populated by login, by the CloudStorage loader, and by localStorage
// fallback reads. All reads go through `admin_token()`.
thread_local! {
    static ADMIN_TOKEN_CACHE: RefCell<String> = RefCell::new(String::new());
}

fn set_admin_token_cache(token: &str) {
    ADMIN_TOKEN_CACHE.with(|c| *c.borrow_mut() = token.to_string());
}

fn clear_admin_token_cache() {
    ADMIN_TOKEN_CACHE.with(|c| c.borrow_mut().clear());
}

use crate::trios::i18n::{
    t, T_BROADCAST, T_BROADCAST_BUTTON_TEXT, T_BROADCAST_NO_PRODUCT, T_BROADCAST_PHOTO,
    T_BROADCAST_PHOTO_HINT, T_BROADCAST_PREVIEW, T_BROADCAST_PRODUCT, T_BROADCAST_PRODUCT_NONE,
    T_BROADCAST_SELECT_CATALOG, T_BROADCAST_SEND, T_BROADCAST_SEND_TEST, T_BROADCAST_SENT,
    T_BROADCAST_TEST_SENT, T_BROADCAST_TEXT,
};
use crate::ui::api::context::api_base_url;
use crate::ui::api::types::{
    Accessory, BroadcastProduct, BroadcastRequest, Event as AdminEvent, EventBooking, Set, Strain,
    TeaProduct,
};
use crate::ui::components::{
    EmptyState, IdPicker, Modal, Skeleton, SkeletonShape, Toast, ToastContainer, ToastKind,
    VideoModal,
};
use crate::ui::screens::events_screen::parse_event_start;
use crate::ui::share::{product_deep_link, ProductKind};
use crate::ui::telegram::{
    use_telegram_id, use_telegram_init_data, HapticNotification, TelegramApp,
};

/// Read the cached admin token. Order of precedence:
/// 1. In-memory cache (populated by CloudStorage loader / login).
/// 2. localStorage fallback (for plain-browser previews / older clients).
/// This is used for the first paint so the login screen doesn't flash
/// unnecessarily when a token is already cached in the browser.
/// Telegram Mini App WebViews do not reliably persist localStorage across
/// restarts, so the real auto-login source is CloudStorage, loaded async in
/// `AdminScreen`.
fn admin_token() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let cached = ADMIN_TOKEN_CACHE.with(|c| c.borrow().clone());
        if !cached.is_empty() && cached.len() <= 2048 {
            return cached;
        }
        let token = web_sys::window()
            .and_then(|w| w.local_storage().ok())
            .flatten()
            .and_then(|s| s.get_item("wwb_admin_token").ok())
            .flatten()
            .unwrap_or_default();
        if token.is_empty() || token.len() > 2048 {
            String::new()
        } else {
            ADMIN_TOKEN_CACHE.with(|c| *c.borrow_mut() = token.clone());
            token
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        String::new()
    }
}

/// Persist the admin token to Telegram CloudStorage (primary) and localStorage
/// (fallback for plain-browser previews / older clients where CloudStorage is
/// unavailable). Telegram Mini App WebViews do not reliably persist localStorage
/// across restarts, so CloudStorage is required for auto-login to work.
#[cfg(target_arch = "wasm32")]
fn save_admin_token(token: &str, telegram_id: i64) {
    set_admin_token_cache(token);
    let tg = crate::ui::telegram::TelegramApp;
    tg.cloud_storage_set("wwb_admin_token", token);
    tg.cloud_storage_set("wwb_admin_telegram_id", &telegram_id.to_string());
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.set_item("wwb_admin_token", token);
            let _ = storage.set_item("wwb_admin_telegram_id", &telegram_id.to_string());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_admin_token(_token: &str, _telegram_id: i64) {}

/// Remove the persisted admin token from both CloudStorage and localStorage.
#[cfg(target_arch = "wasm32")]
fn clear_admin_token() {
    clear_admin_token_cache();
    let tg = crate::ui::telegram::TelegramApp;
    tg.cloud_storage_remove("wwb_admin_token");
    tg.cloud_storage_remove("wwb_admin_telegram_id");
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.remove_item("wwb_admin_token");
            let _ = storage.remove_item("wwb_admin_telegram_id");
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn clear_admin_token() {}

#[derive(Clone)]
struct ToastItem {
    id: u64,
    message: String,
    kind: ToastKind,
}

fn push_toast(mut toasts: Signal<Vec<ToastItem>>, message: String, kind: ToastKind) {
    let id = js_sys::Date::now() as u64;
    toasts.write().push(ToastItem { id, message, kind });
    spawn(async move {
        gloo_timers::future::TimeoutFuture::new(4000).await;
        toasts.write().retain(|t| t.id != id);
    });
}

fn render_toasts(mut toasts: Signal<Vec<ToastItem>>) -> Element {
    let items = toasts.read().clone();
    if items.is_empty() {
        return rsx! {};
    }
    rsx! {
        ToastContainer {
            for toast in items {
                Toast {
                    key: "{toast.id}",
                    kind: toast.kind,
                    message: toast.message.clone(),
                    on_close: move |_| { toasts.write().retain(|t| t.id != toast.id); }
                }
            }
        }
    }
}

// ── Data models ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdminStrain {
    id: String,
    name: String,
    category: Option<String>,
    thc_percent: Option<f64>,
    cbd_percent: Option<f64>,
    effect: Option<String>,
    flavor_profile: Option<String>,
    description: Option<String>,
    price_per_gram: f64,
    available_grams: Option<f64>,
    is_available: bool,
    image_url: Option<String>,
    #[serde(default)]
    video_url: Option<String>,
    name_en: Option<String>,
    description_en: Option<String>,
    effect_en: Option<String>,
    flavor_profile_en: Option<String>,
    strain_type_en: Option<String>,
    // Marketing flags (migration 028, TZ #2)
    #[serde(default)]
    is_strain_of_day: bool,
    #[serde(default)]
    strain_of_day_discount: f64,
    #[serde(default)]
    discount_percent: f64,
    #[serde(default)]
    sale_price: Option<f64>,
    #[serde(default)]
    sale_active: bool,
    #[serde(default)]
    sale_until: Option<String>,
    #[serde(default)]
    is_best_seller: bool,
    #[serde(default)]
    is_new_arrival: bool,
    #[serde(default)]
    new_until: Option<String>,
    #[serde(default)]
    display_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdminAccessory {
    id: String,
    name: String,
    category: Option<String>,
    description: Option<String>,
    price: f64,
    stock: Option<i32>,
    is_available: bool,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    video_url: Option<String>,
    name_en: Option<String>,
    description_en: Option<String>,
    category_en: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdminTea {
    id: String,
    name: String,
    subcategory: Option<String>,
    description: Option<String>,
    price: f64,
    stock: Option<i32>,
    is_available: bool,
    image_url: Option<String>,
    #[serde(default)]
    video_url: Option<String>,
    name_en: Option<String>,
    description_en: Option<String>,
    subcategory_en: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdminSet {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    strain_ids: Vec<String>,
    #[serde(default)]
    accessory_ids: Vec<String>,
    total_price: f64,
    #[serde(default)]
    discount_percent: f64,
    is_available: bool,
    #[serde(default)]
    is_deal_of_day: bool,
    /// Migration 034: image upload symmetric with AccessorySet.
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    video_url: Option<String>,
    /// Migration 038: bilingual name/description, symmetric with the other set types.
    #[serde(default)]
    name_en: Option<String>,
    #[serde(default)]
    description_en: Option<String>,
    /// Migration 040 (Packs Phase 1): total pack weight + one promo badge.
    #[serde(default)]
    total_weight_grams: f64,
    #[serde(default)]
    badge: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdminAccessorySet {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    accessories: Vec<String>,
    total_price: f64,
    #[serde(default)]
    discount_percent: f64,
    is_available: bool,
    #[serde(default)]
    is_deal_of_day: bool,
    #[serde(default)]
    name_en: Option<String>,
    #[serde(default)]
    description_en: Option<String>,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    video_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdminTeaSet {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    items: Vec<String>,
    total_price: f64,
    #[serde(default)]
    discount_percent: f64,
    is_available: bool,
    #[serde(default)]
    name_en: Option<String>,
    #[serde(default)]
    description_en: Option<String>,
    /// Migration 034: image upload symmetric with AccessorySet.
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    video_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StrainsResp {
    strains: Vec<AdminStrain>,
}
#[derive(Debug, Deserialize)]
struct AccessoriesResp {
    accessories: Vec<AdminAccessory>,
}
#[derive(Debug, Deserialize)]
struct TeaResp {
    tea_products: Vec<AdminTea>,
}
#[derive(Debug, Deserialize)]
struct SetsResp {
    sets: Vec<AdminSet>,
}
#[derive(Debug, Deserialize)]
struct AccessorySetsResp {
    accessory_sets: Vec<AdminAccessorySet>,
}
#[derive(Debug, Deserialize)]
struct TeaSetsResp {
    tea_sets: Vec<AdminTeaSet>,
}
#[derive(Debug, Deserialize)]
struct AdminCheck {
    is_admin: bool,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Strains,
    Accessories,
    Tea,
    Sets,
    AccessorySets,
    TeaSets,
    Dashboard,
    Orders,
    // Quests hidden until partner locations are configured.
    #[allow(dead_code)]
    Quests,
    Treasures,
    Garden,
    Loyalty,
    Managers,
    Events,
    Broadcast,
}

// ── File upload helper ─────────────────────────────────────────

/// Result of an upload attempt.
/// - `Ok(Some(url))` — success, server returned a URL
/// - `Ok(None)` — user cancelled / didn't pick a file
/// - `Err(msg)` — server or network error; `msg` is a human-readable Russian string
async fn upload_file(accept: &str) -> Result<Option<String>, String> {
    let accept = accept.to_string();
    let init_data = use_telegram_init_data();
    let telegram_id = use_telegram_id().unwrap_or(0).to_string();
    let token = admin_token();
    if init_data.len() > 4096 {
        return Err("Ошибка загрузки: слишком длинные init_data".into());
    }
    if token.len() > 2048 {
        return Err("Ошибка загрузки: слишком длинный admin token".into());
    }
    fn js_escape(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('\'', "\\'")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
            .replace('\0', "\\0")
            .replace("</script>", "<\\/script>")
            .replace("</SCRIPT>", "<\\/SCRIPT>")
    }
    let token_js = js_escape(&token);
    let telegram_id_js = js_escape(&telegram_id);
    let init_data_js = js_escape(&init_data);
    let accept_js = js_escape(&accept);
    // JS resolves one of:
    //   ''                       — user cancelled (no file picked) or timeout
    //   <url>                    — success (plain URL string from server)
    //   'ERROR::<status>::<body>' — HTTP non-2xx; <status> is numeric, <body> truncated server text
    //   'ERROR::0::<message>'     — network/JS exception; <message> is err.message
    let js = format!(
        r#"
new Promise((resolve) => {{
    var input = document.createElement('input');
    input.type = 'file';
    input.accept = '{}';
    input.style.display = 'none';
    document.body.appendChild(input);
    var resolved = false;
    var cleanup = () => {{ try {{ document.body.removeChild(input); }} catch(e) {{}} }};
    input.onchange = async (e) => {{
        if (resolved) return;
        resolved = true;
        var file = e.target.files[0];
        if (!file) {{ cleanup(); resolve(''); return; }}
        var formData = new FormData();
        formData.append('file', file);
        try {{
            var baseUrl = window.location.origin;
            var resp = await fetch(baseUrl + '/api/upload', {{
                method: 'POST',
                body: formData,
                headers: {{
                    'X-Telegram-Init-Data': '{}',
                    'X-Admin-Telegram-Id': '{}',
                    'X-Admin-Token': '{}'
                }}
            }});
            if (!resp.ok) {{
                var bodyText = '';
                try {{ bodyText = await resp.text(); }} catch(_) {{}}
                if (bodyText && bodyText.length > 500) bodyText = bodyText.slice(0, 500) + '...';
                try {{
                    var parsed = JSON.parse(bodyText);
                    if (parsed && (parsed.error || parsed.message)) bodyText = parsed.error || parsed.message;
                }} catch(_) {{}}
                cleanup();
                resolve('ERROR::' + resp.status + '::' + bodyText);
                return;
            }}
            var text = await resp.text();
            var url = '';
            try {{ var data = JSON.parse(text); url = (data && data.url) || ''; }} catch(_) {{}}
            cleanup();
            if (!url) {{
                resolve('ERROR::' + resp.status + '::Пустой ответ сервера');
                return;
            }}
            resolve(url);
        }} catch(err) {{
            cleanup();
            var msg = (err && err.message) ? err.message : String(err);
            if (msg.length > 500) msg = msg.slice(0, 500) + '...';
            resolve('ERROR::0::' + msg);
        }}
    }};
    setTimeout(() => {{ if (!resolved) {{ resolved = true; cleanup(); resolve(''); }} }}, 120000);
    input.click();
}})
"#,
        accept_js, init_data_js, telegram_id_js, token_js
    );
    if js.len() > 100_000 {
        return Err("Ошибка загрузки: внутренний лимит JS".into());
    }
    let promise_val =
        js_sys::eval(&js).map_err(|_| "Ошибка загрузки: не удалось запустить JS".to_string())?;
    let promise = promise_val
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| "Ошибка загрузки: некорректный Promise".to_string())?;
    let result = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|_| "Ошибка загрузки: JS Promise отклонён".to_string())?;
    let raw = result
        .as_string()
        .ok_or_else(|| "Ошибка загрузки: неожиданный тип результата".to_string())?;
    if raw.is_empty() {
        return Ok(None);
    }
    if let Some(rest) = raw.strip_prefix("ERROR::") {
        let mut parts = rest.splitn(2, "::");
        let status = parts.next().unwrap_or("0");
        let body = parts.next().unwrap_or("");
        let trimmed = body.trim();
        return Err(if trimmed.is_empty() {
            format!("Ошибка загрузки: HTTP {}", status)
        } else {
            format!("Ошибка загрузки: HTTP {} — {}", status, trimmed)
        });
    }
    Ok(Some(raw))
}

async fn upload_image() -> Result<Option<String>, String> {
    upload_file("image/*").await
}
async fn upload_video() -> Result<Option<String>, String> {
    upload_file("video/*").await
}

// ── Main component ────────────────────────────────────────────

#[component]
pub fn AdminScreen() -> Element {
    let active_tab = use_signal(|| Tab::Strains);
    let build_version: &'static str = env!("BUILD_VERSION");

    // Password-only admin auth. We no longer rely on Telegram initData for
    // the initial gate — it fails in plain browsers and is confusing in the
    // Telegram WebApp when the user is not yet in ADMIN_IDS. A valid
    // ADMIN_PASSWORD token is enough; Telegram ID is optional metadata.
    let password_token = use_signal(|| admin_token());

    // Cycle #171: load the durable token from Telegram CloudStorage async.
    // WebView localStorage is sandboxed and often does not survive app restart,
    // so the real persistent store for auto-login is CloudStorage. We paint the
    // localStorage-cached token immediately, then upgrade from CloudStorage
    // once it resolves.
    let mut cloud_token = password_token;
    use_future(move || async move {
        #[cfg(target_arch = "wasm32")]
        {
            let tg = TelegramApp;
            if let Some(token) = tg.cloud_storage_get("wwb_admin_token").await {
                if token.len() <= 2048 && !token.is_empty() {
                    set_admin_token_cache(&token);
                    cloud_token.set(token);
                }
            }
        }
    });

    let access_reload = use_signal(|| 0u32);
    let access = use_resource(move || {
        let _ = access_reload.read();
        let token = password_token.read().clone();
        async move {
            if token.is_empty() {
                return Err("no_token".to_string());
            }
            let base = api_base_url();
            // /api/admin/check still requires a telegram_id query param for the
            // diagnostics path (it is not used for auth when X-Admin-Token is
            // present, but Axum rejects the request without it).
            let url = format!("{}/api/admin/check?telegram_id=0", base);
            let resp = HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Admin-Token", token)
                .send()
                .await
                .map_err(|e| format!("send to {url}: {e}"))?;
            let status = resp.status();
            if status.is_success() {
                return resp
                    .json::<AdminCheck>()
                    .await
                    .map_err(|e| format!("json: {e}"));
            }
            let reason = resp
                .json::<AdminCheck>()
                .await
                .ok()
                .and_then(|c| c.reason)
                .unwrap_or_else(|| format!("http {}", status.as_u16()));
            Err(reason)
        }
    });

    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding:16px;padding-bottom:calc(96px + env(safe-area-inset-bottom));",
            h1 { style: "font-size:22px;color:#ff4757;margin-bottom:4px;", "🔧 Admin" }
            div { style: "font-size:10px;color:#444;margin-bottom:16px;font-family:monospace;", "v{build_version}" }
            match &*access.read() {
                Some(Ok(check)) if check.is_admin => rsx!(AdminPanel { active_tab, password_token }),
                _ => rsx!(AdminLoginScreen { password_token, access_reload }),
            }
        }
    }
}

#[component]
fn AdminLoginScreen(mut password_token: Signal<String>, mut access_reload: Signal<u32>) -> Element {
    // Cycle #171: restored the two-field login shape (Telegram ID + password)
    // that admins were used to.  The Telegram ID is optional metadata; the
    // real auth is still the shared ADMIN_PASSWORD.  Wrapped in a <form> with
    // a submit button so Enter/Tap works reliably in Telegram WebApp and
    // mobile browsers — the previous bare button missed taps in some WebViews.
    let mut admin_id = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error = use_signal(String::new);
    let mut logging_in = use_signal(|| false);
    rsx! {
        div { style: "padding:30px 16px;text-align:center;",
            div { style: "font-size:48px;margin-bottom:12px;", "🔧" }
            h2 { style: "color:#39ff14;font-size:18px;margin-bottom:8px;", "Вход в админку" }
            p { style: "color:#888;font-size:13px;line-height:1.5;max-width:300px;margin:0 auto;",
                "Введите Telegram ID и общий пароль администратора." }
            form {
                style: "margin-top:20px;max-width:300px;margin-left:auto;margin-right:auto;display:flex;flex-direction:column;gap:8px;",
                onsubmit: move |_| {
                    let pw = password.read().trim().to_string();
                    let id_str = admin_id.read().trim().to_string();
                    if pw.is_empty() { error.set("Введите пароль".into()); return; }
                    let id_parsed = id_str.parse::<i64>().unwrap_or(0);
                    logging_in.set(true);
                    error.set(String::new());
                    let mut token_signal = password_token;
                    let mut error2 = error;
                    let mut logging_in2 = logging_in;
                    spawn(async move {
                        let url = format!("{}/api/admin/login", api_base_url());
                        let res = HTTP_CLIENT.clone().post(&url)
                            .json(&serde_json::json!({"password": pw, "telegram_id": id_parsed}))
                            .send().await;
                        logging_in2.set(false);
                        match res {
                            Ok(r) if r.status().is_success() => {
                                if let Ok(data) = r.json::<serde_json::Value>().await {
                                    if let Some(token) = data["token"].as_str() {
                                        #[cfg(target_arch = "wasm32")]
                                        save_admin_token(token, id_parsed);
                                        token_signal.set(token.to_string());
                                        let new_reload = access_reload.read().wrapping_add(1);
                                        access_reload.set(new_reload);
                                    }
                                }
                            }
                            _ => { error2.set("Неверный пароль".into()); }
                        }
                    });
                },
                input { style: input_style(), r#type: "text", placeholder: "Ваш Telegram ID (число)",
                    "aria-label": "Telegram ID",
                    value: "{admin_id}", oninput: move |e| admin_id.set(e.value()) }
                input { style: input_style(), r#type: "password", placeholder: "Пароль админа",
                    "aria-label": "Пароль админа",
                    value: "{password}", oninput: move |e| password.set(e.value()) }
                if !error.read().is_empty() {
                    div { style: "color:#ff4757;font-size:12px;margin-top:4px;", "{error}" }
                }
                button {
                    r#type: "submit",
                    style: if *logging_in.read() { submit_btn_disabled_style() } else { submit_btn_style() },
                    disabled: *logging_in.read(),
                    if *logging_in.read() { "⏳..." } else { "🔑 Войти по паролю" }
                }
                button {
                    r#type: "button",
                    style: secondary_btn_style(),
                    onclick: move |_| {
                        #[cfg(target_arch = "wasm32")]
                        clear_admin_token();
                        password_token.set(String::new());
                        password.set(String::new());
                        admin_id.set(String::new());
                        let new_reload = access_reload.read().wrapping_add(1);
                        access_reload.set(new_reload);
                    },
                    "🧹 Сбросить сохранённый вход"
                }
            }
        }
    }
}

// ── Tab mount wrapper — brief skeleton when switching heavy tabs ─

#[component]
fn TabMount(tab: Tab, expected: Tab, children: Element) -> Element {
    let mut ready = use_signal(|| tab != expected);
    use_future(move || async move {
        if tab == expected {
            // Brief yield so the tab bar paints before heavy DOM drops
            gloo_timers::future::TimeoutFuture::new(40).await;
            ready.set(true);
        }
    });
    if tab == expected && ready() {
        rsx! { {children} }
    } else if tab == expected {
        rsx! {
            div { style: "
                min-height: 200px;
                display: flex; flex-direction: column;
                align-items: center; justify-content: center;
                gap: 12px; padding: 24px;
            ",
                div { style: "
                    width: 32px; height: 32px;
                    border: 3px solid #2a2a4a;
                    border-top-color: #39ff14;
                    border-radius: 50%;
                    animation: tab-spin 0.8s linear infinite;
                " }
                div { style: "font-size: 12px; color: #888;", "Загрузка вкладки…" }
                style { {r#"
                    @keyframes tab-spin {
                        to { transform: rotate(360deg); }
                    }
                "#} }
            }
        }
    } else {
        rsx! {}
    }
}

// ── Admin panel with tabs ─────────────────────────────────────

#[component]
fn AdminPanel(active_tab: Signal<Tab>, mut password_token: Signal<String>) -> Element {
    let tab_btn = |t: Tab, label: &str| -> Element {
        let is_active = *active_tab.read() == t;
        let style = if is_active {
            "flex:1;padding:10px;background:#39ff14;color:#000;border:none;font-weight:700;font-size:13px;cursor:pointer;"
        } else {
            "flex:1;padding:10px;background:#1a1a2e;color:#888;border:none;font-size:13px;cursor:pointer;"
        };
        let label = label.to_string();
        rsx! { button { style: "{style}", onclick: move |_| active_tab.set(t), "{label}" } }
    };
    let current = *active_tab.read();
    rsx! {
        div {
            div { style: "display:flex;justify-content:flex-end;margin-bottom:8px;",
                button {
                    style: "padding:6px 12px;background:#2a2a4a;color:#888;border:none;border-radius:4px;font-size:12px;cursor:pointer;",
                    onclick: move |_| {
                        #[cfg(target_arch = "wasm32")]
                        clear_admin_token();
                        password_token.set(String::new());
                    },
                    "🚪 Выйти"
                }
            }
            div { class: "admin-tabs",
                {tab_btn(Tab::Dashboard, "📊 Дашборд")}
                {tab_btn(Tab::Orders, "📦 Заказы")}
                {tab_btn(Tab::Strains, "🌿 Strains")}
                {tab_btn(Tab::Accessories, "⚙️ Gear")}
                {tab_btn(Tab::Tea, "🍵 Tea")}
                {tab_btn(Tab::Sets, "📦 Sets")}
                {tab_btn(Tab::AccessorySets, "🔧 Acc.Sets")}
                {tab_btn(Tab::TeaSets, "🫖 Tea Sets")}
                {tab_btn(Tab::Treasures, "🏴\u{200d}☠️ Сокровища")}
                {tab_btn(Tab::Garden, "🌱 Сад")}
                {tab_btn(Tab::Loyalty, "💎 Лояльность")}
                {tab_btn(Tab::Managers, "👥 Менеджеры")}
                {tab_btn(Tab::Events, "📅 События")}
                {tab_btn(Tab::Broadcast, "📣 Рассылка")}
            }
            match current {
                Tab::Strains => rsx!(TabMount { tab: current, expected: Tab::Strains, StrainsTab {} }),
                Tab::Accessories => rsx!(TabMount { tab: current, expected: Tab::Accessories, AccessoriesTab {} }),
                Tab::Tea => rsx!(TabMount { tab: current, expected: Tab::Tea, TeaTab {} }),
                Tab::Sets => rsx!(TabMount { tab: current, expected: Tab::Sets, SetsTab {} }),
                Tab::AccessorySets => rsx!(TabMount { tab: current, expected: Tab::AccessorySets, AccessorySetsTab {} }),
                Tab::TeaSets => rsx!(TabMount { tab: current, expected: Tab::TeaSets, TeaSetsTab {} }),
                Tab::Dashboard => rsx!(TabMount { tab: current, expected: Tab::Dashboard, DashboardTab {} }),
                Tab::Orders => rsx!(TabMount { tab: current, expected: Tab::Orders, OrdersTab {} }),
                // Quests hidden until partner locations are configured (GH: keep Tab::Quests/QuestsTab code).
                Tab::Quests => rsx!(TabMount { tab: current, expected: Tab::Quests, div {} }),
                Tab::Treasures => rsx!(TabMount { tab: current, expected: Tab::Treasures, TreasuresTab {} }),
                Tab::Garden => rsx!(TabMount { tab: current, expected: Tab::Garden, GardenTab {} }),
                Tab::Loyalty => rsx!(TabMount { tab: current, expected: Tab::Loyalty, LoyaltyTab {} }),
                Tab::Managers => rsx!(TabMount { tab: current, expected: Tab::Managers, ManagersTab {} }),
                Tab::Events => rsx!(TabMount { tab: current, expected: Tab::Events, EventsTab {} }),
                Tab::Broadcast => rsx!(TabMount { tab: current, expected: Tab::Broadcast, BroadcastTab {} }),
            }
        }
    }
}

// ── Strains tab (Optimistic UI) ───────────────────────────────

#[component]
fn StrainsTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut cache: Signal<Vec<AdminStrain>> = use_signal(|| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(json)) = storage.get_item("wwb_admin_strains") {
                        if json.len() <= 1_000_000 {
                            if let Ok(data) = serde_json::from_str::<Vec<AdminStrain>>(&json) {
                                return data;
                            }
                        }
                    }
                }
            }
        }
        Vec::new()
    });
    let mut loading = use_signal(|| true);
    let mut name = use_signal(String::new);
    let mut category = use_signal(|| "hybrid".to_string());
    let mut price = use_signal(String::new);
    let mut thc = use_signal(String::new);
    let mut cbd = use_signal(String::new);
    let mut grams = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut effect = use_signal(String::new);
    let mut flavor_profile = use_signal(String::new);
    let mut image_url = use_signal(String::new);
    let mut video_url = use_signal(String::new);
    let mut name_en = use_signal(String::new);
    let mut description_en = use_signal(String::new);
    let mut effect_en = use_signal(String::new);
    let mut flavor_profile_en = use_signal(String::new);
    let mut strain_type_en = use_signal(String::new);
    let mut status = use_signal(String::new);
    let mut submitting = use_signal(|| false);
    let mut editing_id: Signal<Option<String>> = use_signal(|| None);
    let mut delete_target_id: Signal<Option<String>> = use_signal(|| None);
    let mut search_query = use_signal(String::new);
    let reload = use_signal(|| 0u32);
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);

    // Cycle #134: bulk-marketing selection state. `selected_ids` is
    // the set of strain ids currently checked in the list; the bulk
    // action bar above the list fires POST /api/strains/bulk-marketing
    // for each toggle. Selection cleared after a successful apply.
    let mut selected_ids: Signal<std::collections::HashSet<String>> =
        use_signal(std::collections::HashSet::new);

    // Cycle #136: TЗ #2 §5 self-service — current state of the
    // "hide marketing badges" admin toggle. `None` until first fetch
    // completes (`use_effect` below). `Some(true)` means badges are
    // hidden on the customer menu right now.
    let badges_hidden: Signal<Option<bool>> = use_signal(|| None);

    {
        let init_for_fetch = init_data.read().clone();
        use_effect(move || {
            let init = init_for_fetch.clone();
            let mut state = badges_hidden;
            spawn(async move {
                let url = format!("{}/api/admin/marketing-display", api_base_url());
                if let Ok(r) = HTTP_CLIENT
                    .clone()
                    .get(&url)
                    .header("X-Telegram-Init-Data", init)
                    .header("X-Admin-Token", admin_token())
                    .send()
                    .await
                {
                    if r.status().is_success() {
                        if let Ok(data) = r.json::<serde_json::Value>().await {
                            if let Some(v) = data.get("hidden").and_then(|x| x.as_bool()) {
                                state.set(Some(v));
                            }
                        }
                    }
                }
            });
        });
    }

    use_effect(move || {
        if editing_id.read().is_some() {
            let _ = js_sys::eval("setTimeout(()=>{var el=document.querySelector('[data-editing]');if(el)el.scrollIntoView({behavior:'smooth',block:'center'});},100);");
        }
    });

    // Persist cache to localStorage
    use_effect(move || {
        let data = cache.read().clone();
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item(
                        "wwb_admin_strains",
                        &serde_json::to_string(&data).unwrap_or_default(),
                    );
                }
            }
        }
    });

    // Fetch data into cache (runs on mount + when reload changes)
    let _ = use_resource(move || {
        let _ = reload.read();
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/strains?include_hidden=1", api_base_url());
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                let text = resp.text().await.unwrap_or_default();
                if let Ok(data) = serde_json::from_str::<StrainsResp>(&text) {
                    cache.set(data.strains);
                }
            }
            loading.set(false);
            Some(())
        }
    });

    let filtered: Vec<AdminStrain> = {
        let q = search_query.read().to_lowercase();
        cache
            .read()
            .iter()
            .filter(|s| {
                q.is_empty()
                    || s.name.to_lowercase().contains(&q)
                    || s.category
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q)
            })
            .cloned()
            .collect()
    };

    rsx! {
           div {
               FormCard {
                   title: "Добавить страйн".to_string(),
                   children: rsx!{
                       input { style: input_style(), placeholder: "Название (RU)", value: "{name}",
                           oninput: move |e| name.set(e.value()) }
                       select { style: input_style(), value: "{category}",
                           oninput: move |e| category.set(e.value()),
                           option { value: "sativa", "☀️ Sativa" }
                           option { value: "indica", "🌙 Indica" }
                           option { value: "hybrid", "⚖️ Hybrid" }
                       }
                       input { style: input_style(), placeholder: "Цена ฿/г", value: "{price}", r#type: "number",
                           oninput: move |e| price.set(e.value()) }
                       input { style: input_style(), placeholder: "THC %", value: "{thc}", r#type: "number",
                           oninput: move |e| thc.set(e.value()) }
                       input { style: input_style(), placeholder: "CBD %", value: "{cbd}", r#type: "number",
                           oninput: move |e| cbd.set(e.value()) }
                       input { style: input_style(), placeholder: "Граммы", value: "{grams}", r#type: "number",
                           oninput: move |e| grams.set(e.value()) }
                       textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description}",
                           oninput: move |e| description.set(e.value()) }
                       textarea { style: textarea_style(), placeholder: "Эффект (RU)", value: "{effect}",
                           oninput: move |e| effect.set(e.value()) }
                       textarea { style: textarea_style(), placeholder: "Вкусовой профиль (RU)", value: "{flavor_profile}",
                           oninput: move |e| flavor_profile.set(e.value()) }
                       ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
                       VideoUpload { video_url: video_url.read().clone(), on_change: move |url: String| video_url.set(url) }
                       div { style: en_section_style(), "🇬🇧 English" }
                       input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}",
                           oninput: move |e| name_en.set(e.value()) }
                       textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}",
                           oninput: move |e| description_en.set(e.value()) }
                       textarea { style: textarea_style(), placeholder: "Effect (EN)", value: "{effect_en}",
                           oninput: move |e| effect_en.set(e.value()) }
                       textarea { style: textarea_style(), placeholder: "Flavor (EN)", value: "{flavor_profile_en}",
                           oninput: move |e| flavor_profile_en.set(e.value()) }
                       input { style: input_style(), placeholder: "Type (EN)", value: "{strain_type_en}",
                           oninput: move |e| strain_type_en.set(e.value()) }
                       button {
                           style: if *submitting.read() { submit_btn_disabled_style() } else { submit_btn_style() },
                           disabled: *submitting.read(),
                           r#type: "button",
                           onclick: move |_| {
                               let n = name().trim().to_string(); let c = category();
                               let p = match price.read().trim().parse::<f64>() {
                                   Ok(v) if v > 0.0 && v.is_finite() => v,
                                   _ => { status.set("❌ Цена должна быть числом больше 0".into()); return; }
                               };
                               let t = thc.read().trim().parse::<f64>().ok().filter(|v| v.is_finite());
                               let cb = cbd.read().trim().parse::<f64>().ok().filter(|v| v.is_finite());
                               let g = match grams.read().trim().parse::<f64>() {
                                   Ok(v) if v >= 0.0 && v.is_finite() => v,
                                   _ => { status.set("❌ Граммы должны быть числом ≥ 0".into()); return; }
                               };
                               if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                               let d = description(); let ef = effect(); let fp = flavor_profile();
                               let img = image_url(); let vid = video_url();
                               let ne = name_en(); let de = description_en(); let ee = effect_en();
                               let fpe = flavor_profile_en(); let ste = strain_type_en();
                               submitting.set(true);
                               // ── Optimistic: insert immediately ──
                               let temp_id = format!("temp-{}", uuid::Uuid::new_v4());
                               cache.write().insert(0, AdminStrain {
                                   id: temp_id.clone(), name: n.clone(), category: Some(c.clone()),
                                   thc_percent: t, cbd_percent: cb,
                                   effect: if ef.is_empty() { None } else { Some(ef.clone()) },
                                   flavor_profile: if fp.is_empty() { None } else { Some(fp.clone()) },
                                   description: if d.is_empty() { None } else { Some(d.clone()) },
                                   price_per_gram: p, available_grams: Some(g), is_available: true,
                                   image_url: if img.is_empty() { None } else { Some(img.clone()) },
                                   video_url: if vid.is_empty() { None } else { Some(vid.clone()) },
                                   name_en: if ne.is_empty() { None } else { Some(ne.clone()) },
                                   description_en: if de.is_empty() { None } else { Some(de.clone()) },
                                   effect_en: if ee.is_empty() { None } else { Some(ee.clone()) },
                                   flavor_profile_en: if fpe.is_empty() { None } else { Some(fpe.clone()) },
                                   strain_type_en: if ste.is_empty() { None } else { Some(ste.clone()) },
                                   // TZ #2 marketing flags default-off for new strains; admin
                                   // turns them on via the EditStrainCard marketing block.
                                   is_strain_of_day: false, strain_of_day_discount: 0.0,
                                   discount_percent: 0.0, sale_price: None, sale_active: false,
                                   sale_until: None, is_best_seller: false,
                                   is_new_arrival: false, new_until: None, display_order: 0,
                               });
                               status.set("✅ Добавлен!".into());
                               name.set(String::new()); price.set(String::new());
                               thc.set(String::new()); cbd.set(String::new()); grams.set(String::new());
                               description.set(String::new()); effect.set(String::new()); flavor_profile.set(String::new());
                               image_url.set(String::new()); video_url.set(String::new());
                               name_en.set(String::new()); description_en.set(String::new());
                               effect_en.set(String::new()); flavor_profile_en.set(String::new());
                               strain_type_en.set(String::new());
                               auto_scroll_to_list();
                               spawn(async move {
                                   let body = json!({
                                       "name": n, "category": c, "price_per_gram": p,
                                       "thc_percent": t, "cbd_percent": cb, "available_grams": g, "is_available": true,
                                       "effect": if ef.is_empty() { serde_json::Value::Null } else { ef.into() },
                                       "flavor_profile": if fp.is_empty() { serde_json::Value::Null } else { fp.into() },
                                       "description": if d.is_empty() { serde_json::Value::Null } else { d.into() },
                                       "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                       "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.clone().into() },
                                       "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                       "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                       "effect_en": if ee.is_empty() { serde_json::Value::Null } else { ee.into() },
                                       "flavor_profile_en": if fpe.is_empty() { serde_json::Value::Null } else { fpe.into() },
                                       "strain_type_en": if ste.is_empty() { serde_json::Value::Null } else { ste.into() },
                                   });
                                   let url = format!("{}/api/strains", api_base_url());
                                   let res = HTTP_CLIENT.clone().post(&url)
                                       .header("X-Telegram-Init-Data", init_data.read().clone())
    .header("X-Admin-Token", admin_token())
    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                       .json(&body).send().await;
                                   submitting.set(false);
                                   match res {
                                       Ok(r) if r.status().is_success() => {
                                           if let Ok(data) = r.json::<serde_json::Value>().await {
                                               if let Some(real_id) = data["id"].as_str() {
                                                   if let Some(s) = cache.write().iter_mut().find(|s| s.id == temp_id) { s.id = real_id.to_string(); }
                                               }
                                           }
                                           TelegramApp::init().haptic_notification(HapticNotification::Success);
                                       }
                                       _ => {
                                           cache.write().retain(|s| s.id != temp_id);
                                           status.set("❌ Ошибка добавления".into());
                                           TelegramApp::init().haptic_notification(HapticNotification::Error);
                                       }
                                   }
                               });
                           },
                           if *submitting.read() { "⏳..." } else { "➕ Добавить" }
                       }
                       {render_status(status)}
                   }
               }

               div { style: "display:flex;align-items:center;gap:6px;margin-bottom:6px;",
                   input {
                       style: "flex:1;padding:6px 10px;background:#1a1a2e;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:13px;",
                       placeholder: "🔍 Поиск...",
                       value: "{search_query}",
                       oninput: move |e: Event<FormData>| search_query.set(e.value())
                   }
                   // Cycle #136: TЗ #2 §5 admin-UI toggle for hiding promo
                   // badges (Sale / Best / New) on the customer menu.
                   // Compact icon button so the search + toggle fit one line.
                   {
                       let current = *badges_hidden.read();
                       let (icon, bg, color, border, label, is_disabled) = match current {
                           None => ("⏳", "#2a2a4a", "#888", "#444", "Загрузка состояния меток…", true),
                           Some(true) => ("🚫", "#2a2a4a", "#bbb", "#444", "Показать промо-метки", false),
                           Some(false) => ("👁", "#1a3a1a", "#9efb9e", "#2a5a2a", "Скрыть промо-метки", false),
                       };
                       let init_for_toggle = init_data.read().clone();
                       rsx! {
                           button {
                               style: "min-width:44px;min-height:44px;padding:8px;background:{bg};color:{color};border:1px solid {border};border-radius:4px;font-size:16px;cursor:pointer;line-height:1;display:flex;align-items:center;justify-content:center;",
                               disabled: is_disabled,
                               "aria-label": "{label}",
                               onclick: move |_| {
                                   if let Some(hidden) = current {
                                       let next = !hidden;
                                       let init = init_for_toggle.clone();
                                       let mut state = badges_hidden;
                                       spawn(async move {
                                           let url = format!("{}/api/admin/marketing-display", api_base_url());
                                           let res = HTTP_CLIENT.clone().put(&url)
                                               .header("X-Telegram-Init-Data", init)
                                               .header("X-Admin-Token", admin_token())
                                               .json(&json!({ "hidden": next }))
                                               .send().await;
                                           if matches!(res, Ok(ref r) if r.status().is_success()) {
                                               state.set(Some(next));
                                           }
                                       });
                                   }
                               },
                               "{icon}"
                           }
                       }
                   }
               }
               h3 { style: "font-size:12px;color:#888;margin:0 0 4px 0;", "Страйны ({filtered.len()})" }
               if *loading.read() {
                   div { style: "display:flex;flex-direction:column;gap:4px;",
                       for _ in 0..4 {
                           div { style: "background:#1a1a2e;padding:4px 6px;border-radius:6px;display:flex;align-items:center;gap:4px;",
                               Skeleton { shape: SkeletonShape::Avatar }
                               div { style: "flex:1;display:flex;flex-direction:column;gap:4px;",
                                   Skeleton { shape: SkeletonShape::Text, width: Some("60%".into()) }
                                   Skeleton { shape: SkeletonShape::TextSm, width: Some("40%".into()) }
                               }
                           }
                       }
                   }
               } else if filtered.is_empty() {
                   EmptyState {
                       icon: "🔍",
                       title: "Ничего не найдено",
                       description: "Попробуйте изменить запрос поиска",
                       action: rsx! {
                           button {
                               style: "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:13px;cursor:pointer;",
                               onclick: move |_| search_query.set(String::new()),
                               "Очистить поиск"
                           }
                       },
                   }
               } else {
                   // Cycle #134: bulk-marketing action bar. Renders when any
                   // strain is selected. Three "ON" buttons (BEST / NEW /
                   // SALE) — turning a flag OFF in bulk is rarer; admins
                   // do that via per-strain edit. Disabled while a bulk
                   // request is in flight to prevent N concurrent POSTs.
                   {
                       let selected_count = selected_ids.read().len();
                       if selected_count > 0 {
                           rsx! {
                               div { style: "background:#1a1a2e;border:2px solid #ffe600;border-radius:6px;padding:10px 12px;margin-bottom:8px;display:flex;flex-wrap:wrap;align-items:center;gap:8px;",
                                   span { style: "font-weight:700;color:#ffe600;",
                                       "✓ {selected_count} выбрано"
                                   }
                                   // Cycle #135: six bulk buttons — ON/OFF for each
                                   // of the three promo flags. Same endpoint, value
                                   // differs. ON = orange (action colour), OFF = dark
                                   // grey (de-emphasised — "rare" path).
                                   {[
                                       ("⭐ BEST", "is_best_seller", true),
                                       ("🆕 NEW",  "is_new_arrival", true),
                                       ("🔥 SALE", "sale_active",    true),
                                       ("⭐ BEST", "is_best_seller", false),
                                       ("🆕 NEW",  "is_new_arrival", false),
                                       ("🔥 SALE", "sale_active",    false),
                                   ].iter().map(|(label, flag, value)| {
                                       let flag = *flag;
                                       let label = *label;
                                       let value = *value;
                                       let init_for_bulk = init_data.read().clone();
                                       let btn_style = if value {
                                           "padding:6px 12px;background:#ff9d00;color:#000;border:none;border-radius:4px;font-size:13px;font-weight:700;cursor:pointer;"
                                       } else {
                                           "padding:6px 12px;background:#2a2a4a;color:#bbb;border:none;border-radius:4px;font-size:13px;font-weight:600;cursor:pointer;"
                                       };
                                       let suffix = if value { "ON" } else { "OFF" };
                                       rsx! {
                                           button {
                                               key: "{flag}-{suffix}",
                                               style: "{btn_style}",
                                               onclick: move |_| {
                                                   let ids: Vec<String> = selected_ids.read().iter().cloned().collect();
                                                   if ids.is_empty() { return; }
                                                   let init = init_for_bulk.clone();
                                                   let mut reload_s = reload;
                                                   let mut selected_s = selected_ids;
                                                   let body = json!({ "ids": ids, flag: value });
                                                   spawn(async move {
                                                       let url = format!("{}/api/strains/bulk-marketing", api_base_url());
                                                       let res = HTTP_CLIENT.clone().post(&url)
                                                           .header("X-Telegram-Init-Data", init)
                                                           .header("X-Admin-Token", admin_token())
                                                           .json(&body)
                                                           .send().await;
                                                       if matches!(res, Ok(ref r) if r.status().is_success()) {
                                                           selected_s.set(std::collections::HashSet::new());
                                                           let n = reload_s.read().wrapping_add(1);
                                                           reload_s.set(n);
                                                       }
                                                   });
                                               },
                                               "{label} {suffix}"
                                           }
                                       }
                                   })}
                                   button {
                                       style: "padding:6px 12px;background:transparent;color:#888;border:1px solid #2a2a4a;border-radius:4px;font-size:13px;cursor:pointer;",
                                       onclick: move |_| {
                                           selected_ids.set(std::collections::HashSet::new());
                                       },
                                       "Очистить"
                                   }
                               }
                           }
                       } else {
                           rsx! {}
                       }
                   }
                   div { "data-list": "true", style: "display:flex;flex-direction:column;gap:4px;",
                       for s in filtered {
                           if editing_id.read().as_deref() == Some(s.id.as_str()) {
                               EditStrainCard {
                                   key: "{s.id}",
                                   item: s.clone(),
                                   cache: cache,
                                   on_saved: move |_| editing_id.set(None),
                                   on_cancel: move |_| editing_id.set(None),
                               }
                           } else {
                               // Cycle #134: wrap row with selection checkbox.
                               {
                                   let row_id = s.id.clone();
                                   let row_id_for_checkbox = row_id.clone();
                                   let is_checked = selected_ids.read().contains(&row_id);
                                   rsx! {
                                       div { key: "wrap-{row_id}", style: "display:flex;align-items:center;gap:8px;",
                                           input {
                                               r#type: "checkbox",
                                               style: "width:18px;height:18px;cursor:pointer;flex-shrink:0;",
                                               checked: is_checked,
                                               onchange: move |e| {
                                                   let mut set = selected_ids.write();
                                                   if e.value() == "true" {
                                                       set.insert(row_id_for_checkbox.clone());
                                                   } else {
                                                       set.remove(&row_id_for_checkbox);
                                                   }
                                               },
                                           }
                                           div { style: "flex:1;",
                                               ItemRow {
                                   key: "{s.id}",
                                   name: s.name.clone(),
                                   sub: format!("{} • {}฿/г • {}г", s.category.clone().unwrap_or_default(), s.price_per_gram, s.available_grams.unwrap_or(0.0)),
                                   is_available: s.is_available,
                                   image_url: s.image_url.clone(),
                                   video_url: s.video_url.clone(),
                                   on_edit: {
                                       let id = s.id.clone();
                                       move |_| editing_id.set(Some(id.clone()))
                                   },
                                   on_toggle: {
                                       let id = s.id.clone();
                                       let next_avail = !s.is_available;
                                       move |_| {
                                           let id = id.clone();
                                           if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) { s.is_available = next_avail; }
                                           spawn(async move {
                                               let url = format!("{}/api/strains/{}/availability", api_base_url(), id);
                                               let res = HTTP_CLIENT.clone().put(&url)
                                                   .header("X-Telegram-Init-Data", init_data.read().clone())
                                                   .header("X-Admin-Token", admin_token())
                                                   .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                   .json(&json!({ "is_available": next_avail }))
                                                   .send().await;
                                               match res {
                                                   Ok(r) if r.status().is_success() => { TelegramApp::init().haptic_notification(HapticNotification::Success); }
                                                   _ => {
                                                       if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) { s.is_available = !next_avail; }
                                                       push_toast(toasts, "Не удалось изменить статус страйна".into(), ToastKind::Error);
                                                       TelegramApp::init().haptic_notification(HapticNotification::Error);
                                                   }
                                               }
                                           });
                                       }
                                   },
                                   on_delete: {
                                       let id = s.id.clone();
                                       move |_| delete_target_id.set(Some(id.clone()))
                                   }
                               }
                                           } // close inner div { style:"flex:1;" }
                                       } // close outer wrap div { key:"wrap-..." }
                                   } // close rsx! macro
                               } // close { let row_id...; rsx!{...} } block expression
                           }
                       }
                   }
               }
               DeleteConfirmModal {
                   target: delete_target_id,
                   item_name: "страйн".to_string(),
                   on_confirm: move |id: String| {
                       let deleted = cache.read().iter().find(|s| s.id == id).cloned();
                       cache.write().retain(|s| s.id != id);
                       spawn(async move {
                           let url = format!("{}/api/strains/{}", api_base_url(), id);
                           let res = HTTP_CLIENT.clone().delete(&url)
                               .header("X-Telegram-Init-Data", init_data.read().clone())
                               .header("X-Admin-Token", admin_token())
                               .header("X-Admin-Telegram-Id", telegram_id.to_string())
                               .send().await;
                           match res {
                               Ok(r) if r.status().is_success() => { push_toast(toasts, "Страйн удалён".into(), ToastKind::Success); TelegramApp::init().haptic_notification(HapticNotification::Success); }
                               _ => {
                                   if let Some(item) = deleted { cache.write().push(item); }
                                   push_toast(toasts, "Не удалось удалить страйн".into(), ToastKind::Error);
                                   TelegramApp::init().haptic_notification(HapticNotification::Error);
                               }
                           }
                       });
                   }
               }
               {render_toasts(toasts)}
           }
       }
}

// ── Accessories tab (Optimistic UI) ───────────────────────────

#[component]
fn AccessoriesTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut cache: Signal<Vec<AdminAccessory>> = use_signal(|| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(json)) = storage.get_item("wwb_admin_accessories") {
                        if json.len() <= 1_000_000 {
                            if let Ok(data) = serde_json::from_str::<Vec<AdminAccessory>>(&json) {
                                return data;
                            }
                        }
                    }
                }
            }
        }
        Vec::new()
    });
    let mut loading = use_signal(|| true);
    let mut name = use_signal(String::new);
    let mut category = use_signal(|| "other".to_string());
    let mut price = use_signal(String::new);
    let mut stock = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut image_url = use_signal(String::new);
    let mut video_url = use_signal(String::new);
    let mut name_en = use_signal(String::new);
    let mut description_en = use_signal(String::new);
    let mut category_en = use_signal(String::new);
    let mut status = use_signal(String::new);
    let mut submitting = use_signal(|| false);
    let mut editing_id: Signal<Option<String>> = use_signal(|| None);
    let mut delete_target_id: Signal<Option<String>> = use_signal(|| None);
    let mut search_query = use_signal(String::new);
    let reload = use_signal(|| 0u32);
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);

    use_effect(move || {
        if editing_id.read().is_some() {
            let _ = js_sys::eval("setTimeout(()=>{var el=document.querySelector('[data-editing]');if(el)el.scrollIntoView({behavior:'smooth',block:'center'});},100);");
        }
    });

    // Persist cache to localStorage
    use_effect(move || {
        let data = cache.read().clone();
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item(
                        "wwb_admin_accessories",
                        &serde_json::to_string(&data).unwrap_or_default(),
                    );
                }
            }
        }
    });

    let _ = use_resource(move || {
        let _ = reload.read();
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/accessories?include_hidden=1", api_base_url());
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                if let Ok(data) = resp.json::<AccessoriesResp>().await {
                    cache.set(data.accessories);
                }
            }
            loading.set(false);
            Some(())
        }
    });

    let filtered: Vec<AdminAccessory> = {
        let q = search_query.read().to_lowercase();
        cache
            .read()
            .iter()
            .filter(|a| {
                q.is_empty()
                    || a.name.to_lowercase().contains(&q)
                    || a.category
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q)
            })
            .cloned()
            .collect()
    };

    rsx! {
           div {
               FormCard {
                   title: "Добавить аксессуар".to_string(),
                   children: rsx!{
                       input { style: input_style(), placeholder: "Название (RU)", value: "{name}",
                           oninput: move |e| name.set(e.value()) }
                       select { style: input_style(), value: "{category}",
                           oninput: move |e| category.set(e.value()),
                           option { value: "grinder", "🌀 Grinder" }
                           option { value: "papers", "📄 Papers" }
                           option { value: "lighter", "🔥 Lighter" }
                           option { value: "pipe", "🚬 Pipe" }
                           option { value: "bong", "💨 Bong" }
                           option { value: "storage", "📦 Storage" }
                           option { value: "clothing", "👕 Clothing" }
                           option { value: "other", "🔧 Other" }
                       }
                       input { style: input_style(), placeholder: "Цена ฿", value: "{price}", r#type: "number",
                           oninput: move |e| price.set(e.value()) }
                       input { style: input_style(), placeholder: "Кол-во", value: "{stock}", r#type: "number",
                           oninput: move |e| stock.set(e.value()) }
                       textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description}",
                           oninput: move |e| description.set(e.value()) }
                       ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
                       VideoUpload { video_url: video_url.read().clone(), on_change: move |url: String| video_url.set(url) }
                       div { style: en_section_style(), "🇬🇧 English" }
                       input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}",
                           oninput: move |e| name_en.set(e.value()) }
                       textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}",
                           oninput: move |e| description_en.set(e.value()) }
                       input { style: input_style(), placeholder: "Category (EN)", value: "{category_en}",
                           oninput: move |e| category_en.set(e.value()) }
                       button {
                           style: if *submitting.read() { submit_btn_disabled_style() } else { submit_btn_style() },
                           disabled: *submitting.read(),
                           onclick: move |_| {
                               let n = name(); let c = category();
                               if n.trim().is_empty() { status.set("❌ Имя обязательно".into()); return; }
                               // Cycle #139: strict parse for price + stock. Pre-#139
                               // a typo silently became 0/0 — only the downstream
                               // `p <= 0.0` guard caught it, and the toast said only
                               // "❌ Name + price" without naming the bad field.
                               let p = match crate::trios::validation::parse_finite_float_in_range(
                                   &price.read(), "Цена", 0.01, 1_000_000.0,
                               ) {
                                   Ok(v) => v,
                                   Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                               };
                               let s_val = match crate::trios::validation::parse_int_in_range(
                                   &stock.read(), "Количество", 0, 1_000_000,
                               ) {
                                   Ok(v) => v,
                                   Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                               };
                               let d = description();
                               // Normalize free-text URLs so a scheme-less paste
                               // (e.g. "bucket-…/x.mov") doesn't 400 server-side.
                               let img = crate::trios::validation::normalize_media_url(&image_url());
                               let vid = crate::trios::validation::normalize_media_url(&video_url());
                               let ne = name_en(); let de = description_en(); let ce = category_en();
                               submitting.set(true);
                               let temp_id = format!("temp-{}", uuid::Uuid::new_v4());
                               cache.write().insert(0, AdminAccessory {
                                   id: temp_id.clone(), name: n.clone(), category: Some(c.clone()),
                                   description: if d.is_empty() { None } else { Some(d.clone()) },
                                   price: p, stock: Some(s_val), is_available: true,
                                   image_url: if img.is_empty() { None } else { Some(img.clone()) },
                                   video_url: if vid.is_empty() { None } else { Some(vid.clone()) },
                                   name_en: if ne.is_empty() { None } else { Some(ne.clone()) },
                                   description_en: if de.is_empty() { None } else { Some(de.clone()) },
                                   category_en: if ce.is_empty() { None } else { Some(ce.clone()) },
                               });
                               status.set("✅ Добавлен!".into());
                               name.set(String::new()); price.set(String::new()); stock.set(String::new());
                               description.set(String::new()); image_url.set(String::new()); video_url.set(String::new());
                               name_en.set(String::new()); description_en.set(String::new()); category_en.set(String::new());
                               auto_scroll_to_list();
                               spawn(async move {
                                   let body = json!({
                                       "name": n, "category": c, "price": p, "stock": s_val,
                                       "description": if d.is_empty() { serde_json::Value::Null } else { d.into() },
                                       "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                       "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.clone().into() },
                                       "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                       "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                       "category_en": if ce.is_empty() { serde_json::Value::Null } else { ce.into() },
                                   });
                                   let url = format!("{}/api/accessories", api_base_url());
                                   let res = HTTP_CLIENT.clone().post(&url)
                                       .header("X-Telegram-Init-Data", init_data.read().clone())
    .header("X-Admin-Token", admin_token())
    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                       .json(&body).send().await;
                                   submitting.set(false);
                                   match res {
                                       Ok(r) if r.status().is_success() => {
                                           if let Ok(data) = r.json::<serde_json::Value>().await {
                                               if let Some(real_id) = data["id"].as_str() {
                                                   if let Some(a) = cache.write().iter_mut().find(|a| a.id == temp_id) { a.id = real_id.to_string(); }
                                               }
                                           }
                                           TelegramApp::init().haptic_notification(HapticNotification::Success);
                                       }
                                       _ => { cache.write().retain(|a| a.id != temp_id); status.set("❌ Error".into()); TelegramApp::init().haptic_notification(HapticNotification::Error); }
                                   }
                               });
                           },
                           if *submitting.read() { "⏳..." } else { "➕ Добавить" }
                       }
                       {render_status(status)}
                   }
               }
               h3 { style: list_title_style(), "Аксессуары ({filtered.len()})" }
               {render_search(search_query)}
               if *loading.read() {
                   div { style: "display:flex;flex-direction:column;gap:8px;",
                       for _ in 0..4 {
                           div { style: "background:#1a1a2e;padding:8px 10px;border-radius:6px;display:flex;align-items:center;gap:6px;",
                               Skeleton { shape: SkeletonShape::Avatar }
                               div { style: "flex:1;display:flex;flex-direction:column;gap:4px;",
                                   Skeleton { shape: SkeletonShape::Text, width: Some("60%".into()) }
                                   Skeleton { shape: SkeletonShape::TextSm, width: Some("40%".into()) }
                               }
                           }
                       }
                   }
               } else if filtered.is_empty() {
                   EmptyState {
                       icon: "🔍",
                       title: "Ничего не найдено",
                       description: "Попробуйте изменить запрос поиска",
                       action: rsx! {
                           button {
                               style: "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:13px;cursor:pointer;",
                               onclick: move |_| search_query.set(String::new()),
                               "Очистить поиск"
                           }
                       },
                   }
               } else {
                   div { "data-list": "true", style: "display:flex;flex-direction:column;gap:8px;",
                       for a in filtered {
                           if editing_id.read().as_deref() == Some(a.id.as_str()) {
                               EditAccessoryCard {
                                   key: "{a.id}",
                                   item: a.clone(),
                                   cache: cache,
                                   on_saved: move |_| editing_id.set(None),
                                   on_cancel: move |_| editing_id.set(None),
                               }
                           } else {
                               ItemRow {
                                   key: "{a.id}",
                                   name: a.name.clone(),
                                   sub: format!("{} • {}฿ • {} шт.", a.category.clone().unwrap_or_default(), a.price, a.stock.unwrap_or(0)),
                                   is_available: a.is_available,
                                   image_url: a.image_url.clone(),
                                   video_url: a.video_url.clone(),
                                   on_edit: { let id = a.id.clone(); move |_| editing_id.set(Some(id.clone())) },
                                   on_toggle: {
                                       let id = a.id.clone(); let next = !a.is_available;
                                       move |_| {
                                           let id = id.clone();
                                           if let Some(a) = cache.write().iter_mut().find(|a| a.id == id) { a.is_available = next; }
                                           spawn(async move {
                                               let url = format!("{}/api/accessories/{}/availability", api_base_url(), id);
                                               let res = HTTP_CLIENT.clone().put(&url)
                                                   .header("X-Telegram-Init-Data", init_data.read().clone())
                                                   .header("X-Admin-Token", admin_token())
                                                   .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                   .json(&json!({ "is_available": next })).send().await;
                                               match res {
                                                   Ok(r) if r.status().is_success() => { TelegramApp::init().haptic_notification(HapticNotification::Success); }
                                                   _ => {
                                                       if let Some(a) = cache.write().iter_mut().find(|a| a.id == id) { a.is_available = !next; }
                                                       push_toast(toasts, "Не удалось изменить статус аксессуара".into(), ToastKind::Error);
                                                       TelegramApp::init().haptic_notification(HapticNotification::Error);
                                                   }
                                               }
                                           });
                                       }
                                   },
                                   on_delete: {
                                       let id = a.id.clone();
                                       move |_| delete_target_id.set(Some(id.clone()))
                                   }
                               }
                           }
                       }
                   }
               }
               DeleteConfirmModal {
                   target: delete_target_id,
                   item_name: "аксессуар".to_string(),
                   on_confirm: move |id: String| {
                       let deleted = cache.read().iter().find(|a| a.id == id).cloned();
                       cache.write().retain(|a| a.id != id);
                       spawn(async move {
                           let url = format!("{}/api/accessories/{}", api_base_url(), id);
                           let res = HTTP_CLIENT.clone().delete(&url)
                               .header("X-Telegram-Init-Data", init_data.read().clone())
                               .header("X-Admin-Token", admin_token())
                               .header("X-Admin-Telegram-Id", telegram_id.to_string())
                               .send().await;
                           match res {
                               Ok(r) if r.status().is_success() => { push_toast(toasts, "Аксессуар удалён".into(), ToastKind::Success); TelegramApp::init().haptic_notification(HapticNotification::Success); }
                               _ => {
                                   if let Some(item) = deleted { cache.write().push(item); }
                                   push_toast(toasts, "Не удалось удалить аксессуар".into(), ToastKind::Error);
                                   TelegramApp::init().haptic_notification(HapticNotification::Error);
                               }
                           }
                       });
                   }
               }
               {render_toasts(toasts)}
           }
       }
}

// ── Tea tab (Optimistic UI) ───────────────────────────────────

#[component]
fn TeaTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut cache: Signal<Vec<AdminTea>> = use_signal(|| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(json)) = storage.get_item("wwb_admin_tea") {
                        if json.len() <= 1_000_000 {
                            if let Ok(data) = serde_json::from_str::<Vec<AdminTea>>(&json) {
                                return data;
                            }
                        }
                    }
                }
            }
        }
        Vec::new()
    });
    let mut loading = use_signal(|| true);
    let mut name = use_signal(String::new);
    // Holds the selected category's canonical KEY, or "__new__" to create one.
    let mut subcategory = use_signal(|| "tea".to_string());
    // Bilingual inputs shown only when "__new__" is selected.
    let mut new_cat_ru = use_signal(String::new);
    let mut new_cat_en = use_signal(String::new);
    let mut price = use_signal(String::new);
    let mut stock = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut image_url = use_signal(String::new);
    let mut video_url = use_signal(String::new);
    let mut name_en = use_signal(String::new);
    let mut description_en = use_signal(String::new);
    let mut status = use_signal(String::new);
    let mut submitting = use_signal(|| false);
    let mut editing_id: Signal<Option<String>> = use_signal(|| None);
    let mut delete_target_id: Signal<Option<String>> = use_signal(|| None);
    let mut search_query = use_signal(String::new);
    let reload = use_signal(|| 0u32);
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);

    use_effect(move || {
        if editing_id.read().is_some() {
            let _ = js_sys::eval("setTimeout(()=>{var el=document.querySelector('[data-editing]');if(el)el.scrollIntoView({behavior:'smooth',block:'center'});},100);");
        }
    });

    // Persist cache to localStorage
    use_effect(move || {
        let data = cache.read().clone();
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item(
                        "wwb_admin_tea",
                        &serde_json::to_string(&data).unwrap_or_default(),
                    );
                }
            }
        }
    });

    let _ = use_resource(move || {
        let _ = reload.read();
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/tea-products?include_hidden=1", api_base_url());
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                if let Ok(data) = resp.json::<TeaResp>().await {
                    cache.set(data.tea_products);
                }
            }
            loading.set(false);
            Some(())
        }
    });

    let filtered: Vec<AdminTea> = {
        let q = search_query.read().to_lowercase();
        cache
            .read()
            .iter()
            .filter(|t| {
                q.is_empty()
                    || t.name.to_lowercase().contains(&q)
                    || t.subcategory
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q)
            })
            .cloned()
            .collect()
    };

    rsx! {
           div {
               FormCard {
                   title: "Добавить чай".to_string(),
                   children: rsx!{
                       input { style: input_style(), placeholder: "Название (RU)", value: "{name}",
                           oninput: move |e| name.set(e.value()) }
                       // Category dropdown — built dynamically from existing
                       // catalog categories (+ built-in starters), with a
                       // "create new" option. New categories auto-appear.
                       {
                           let cats = crate::trios::drink_categories::admin_categories(
                               cache.read().iter().map(|t| (
                                   t.subcategory.as_deref().unwrap_or(""),
                                   t.subcategory_en.as_deref(),
                               )),
                           );
                           rsx! {
                               select { style: input_style(), value: "{subcategory}",
                                   oninput: move |e| subcategory.set(e.value()),
                                   for cat in cats.iter() {
                                       option { value: "{cat.key}",
                                           "{crate::trios::drink_categories::emoji(&cat.key)} {cat.ru} / {cat.en}" }
                                   }
                                   option { value: "__new__", "➕ Новая категория" }
                               }
                           }
                       }
                       if subcategory() == "__new__" {
                           input { style: input_style(), placeholder: "Категория (RU), напр. Смузи", value: "{new_cat_ru}",
                               oninput: move |e| new_cat_ru.set(e.value()) }
                           input { style: input_style(), placeholder: "Category (EN), e.g. Smoothie", value: "{new_cat_en}",
                               oninput: move |e| new_cat_en.set(e.value()) }
                       }
                       input { style: input_style(), placeholder: "Цена ฿", value: "{price}", r#type: "number",
                           oninput: move |e| price.set(e.value()) }
                       input { style: input_style(), placeholder: "Кол-во", value: "{stock}", r#type: "number",
                           oninput: move |e| stock.set(e.value()) }
                       textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description}",
                           oninput: move |e| description.set(e.value()) }
                       ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
                       VideoUpload { video_url: video_url.read().clone(), on_change: move |url: String| video_url.set(url) }
                       div { style: en_section_style(), "🇬🇧 English" }
                       input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}",
                           oninput: move |e| name_en.set(e.value()) }
                       textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}",
                           oninput: move |e| description_en.set(e.value()) }
                       button {
                           style: if *submitting.read() { submit_btn_disabled_style() } else { submit_btn_style() },
                           disabled: *submitting.read(),
                           onclick: move |_| {
                               let n = name();
                               if n.trim().is_empty() { status.set("❌ Имя обязательно".into()); return; }
                               // Resolve the chosen category into bilingual labels
                               // (sc = RU, sce = EN). Both persist on the drink so
                               // a brand-new category is fully defined by its first
                               // item. See trios::drink_categories.
                               let key = subcategory();
                               let (sc, sce) = if key == "__new__" {
                                   let ru = new_cat_ru().trim().to_string();
                                   let en = new_cat_en().trim().to_string();
                                   if ru.is_empty() { status.set("❌ Введите название категории (RU)".into()); return; }
                                   (ru.clone(), if en.is_empty() { ru } else { en })
                               } else {
                                   let cats = crate::trios::drink_categories::admin_categories(
                                       cache.read().iter().map(|t| (
                                           t.subcategory.as_deref().unwrap_or(""),
                                           t.subcategory_en.as_deref(),
                                       )),
                                   );
                                   match cats.iter().find(|c| c.key == key) {
                                       Some(c) => (c.ru.clone(), c.en.clone()),
                                       None => (key.clone(), key.clone()),
                                   }
                               };
                               // Cycle #139: strict parse for price + stock (mirrors
                               // the accessory form). Specific field error replaces
                               // the generic "Name + price" toast.
                               let p = match crate::trios::validation::parse_finite_float_in_range(
                                   &price.read(), "Цена", 0.01, 1_000_000.0,
                               ) {
                                   Ok(v) => v,
                                   Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                               };
                               let s_val = match crate::trios::validation::parse_int_in_range(
                                   &stock.read(), "Количество", 0, 1_000_000,
                               ) {
                                   Ok(v) => v,
                                   Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                               };
                               let d = description(); let img = image_url(); let vid = video_url();
                               let ne = name_en(); let de = description_en();
                               submitting.set(true);
                               let temp_id = format!("temp-{}", uuid::Uuid::new_v4());
                               cache.write().insert(0, AdminTea {
                                   id: temp_id.clone(), name: n.clone(), subcategory: Some(sc.clone()),
                                   description: if d.is_empty() { None } else { Some(d.clone()) },
                                   price: p, stock: Some(s_val), is_available: true,
                                   image_url: if img.is_empty() { None } else { Some(img.clone()) },
                                   video_url: if vid.is_empty() { None } else { Some(vid.clone()) },
                                   name_en: if ne.is_empty() { None } else { Some(ne.clone()) },
                                   description_en: if de.is_empty() { None } else { Some(de.clone()) },
                                   subcategory_en: if sce.is_empty() { None } else { Some(sce.clone()) },
                               });
                               status.set("✅ Добавлен!".into());
                               name.set(String::new()); price.set(String::new()); stock.set(String::new());
                               description.set(String::new()); image_url.set(String::new()); video_url.set(String::new());
                               name_en.set(String::new()); description_en.set(String::new());
                               subcategory.set("tea".to_string()); new_cat_ru.set(String::new()); new_cat_en.set(String::new());
                               auto_scroll_to_list();
                               spawn(async move {
                                   let body = json!({
                                       "name": n, "subcategory": sc, "price": p, "stock": s_val,
                                       "description": if d.is_empty() { serde_json::Value::Null } else { d.into() },
                                       "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                       "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                       "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                       "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                       "subcategory_en": if sce.is_empty() { serde_json::Value::Null } else { sce.into() },
                                   });
                                   let url = format!("{}/api/tea-products", api_base_url());
                                   let res = HTTP_CLIENT.clone().post(&url)
                                       .header("X-Telegram-Init-Data", init_data.read().clone())
    .header("X-Admin-Token", admin_token())
    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                       .json(&body).send().await;
                                   submitting.set(false);
                                   match res {
                                       Ok(r) if r.status().is_success() => {
                                           if let Ok(data) = r.json::<serde_json::Value>().await {
                                               if let Some(real_id) = data["id"].as_str() {
                                                   if let Some(t) = cache.write().iter_mut().find(|t| t.id == temp_id) { t.id = real_id.to_string(); }
                                               }
                                           }
                                           TelegramApp::init().haptic_notification(HapticNotification::Success);
                                       }
                                       _ => { cache.write().retain(|t| t.id != temp_id); status.set("❌ Error".into()); TelegramApp::init().haptic_notification(HapticNotification::Error); }
                                   }
                               });
                           },
                           if *submitting.read() { "⏳..." } else { "➕ Добавить" }
                       }
                       {render_status(status)}
                   }
               }
               h3 { style: list_title_style(), "Чай ({filtered.len()})" }
               {render_search(search_query)}
               if *loading.read() {
                   div { style: "display:flex;flex-direction:column;gap:8px;",
                       for _ in 0..4 {
                           div { style: "background:#1a1a2e;padding:8px 10px;border-radius:6px;display:flex;align-items:center;gap:6px;",
                               Skeleton { shape: SkeletonShape::Avatar }
                               div { style: "flex:1;display:flex;flex-direction:column;gap:4px;",
                                   Skeleton { shape: SkeletonShape::Text, width: Some("60%".into()) }
                                   Skeleton { shape: SkeletonShape::TextSm, width: Some("40%".into()) }
                               }
                           }
                       }
                   }
               } else if filtered.is_empty() {
                   EmptyState {
                       icon: "🔍",
                       title: "Ничего не найдено",
                       description: "Попробуйте изменить запрос поиска",
                       action: rsx! {
                           button {
                               style: "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:13px;cursor:pointer;",
                               onclick: move |_| search_query.set(String::new()),
                               "Очистить поиск"
                           }
                       },
                   }
               } else {
                   div { "data-list": "true", style: "display:flex;flex-direction:column;gap:8px;",
                       for t in filtered {
                           if editing_id.read().as_deref() == Some(t.id.as_str()) {
                               EditTeaCard {
                                   key: "{t.id}",
                                   item: t.clone(),
                                   cache: cache,
                                   on_saved: move |_| editing_id.set(None),
                                   on_cancel: move |_| editing_id.set(None),
                               }
                           } else {
                               ItemRow {
                                   key: "{t.id}",
                                   name: t.name.clone(),
                                   sub: format!("{} • {}฿ • {} шт.", t.subcategory.clone().unwrap_or_default(), t.price, t.stock.unwrap_or(0)),
                                   is_available: t.is_available,
                                   image_url: t.image_url.clone(),
                                   video_url: t.video_url.clone(),
                                   on_edit: { let id = t.id.clone(); move |_| editing_id.set(Some(id.clone())) },
                                   on_toggle: {
                                       let id = t.id.clone(); let next = !t.is_available;
                                       move |_| {
                                           let id = id.clone();
                                           if let Some(t) = cache.write().iter_mut().find(|t| t.id == id) { t.is_available = next; }
                                           spawn(async move {
                                               let url = format!("{}/api/tea-products/{}/availability", api_base_url(), id);
                                               let res = HTTP_CLIENT.clone().put(&url)
                                                   .header("X-Telegram-Init-Data", init_data.read().clone())
                                                   .header("X-Admin-Token", admin_token())
                                                   .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                   .json(&json!({ "is_available": next })).send().await;
                                               match res {
                                                   Ok(r) if r.status().is_success() => { TelegramApp::init().haptic_notification(HapticNotification::Success); }
                                                   _ => {
                                                       if let Some(t) = cache.write().iter_mut().find(|t| t.id == id) { t.is_available = !next; }
                                                       push_toast(toasts, "Не удалось изменить статус чая".into(), ToastKind::Error);
                                                       TelegramApp::init().haptic_notification(HapticNotification::Error);
                                                   }
                                               }
                                           });
                                       }
                                   },
                                   on_delete: {
                                       let id = t.id.clone();
                                       move |_| delete_target_id.set(Some(id.clone()))
                                   }
                               }
                           }
                       }
                   }
               }
               DeleteConfirmModal {
                   target: delete_target_id,
                   item_name: "чай".to_string(),
                   on_confirm: move |id: String| {
                       let deleted = cache.read().iter().find(|t| t.id == id).cloned();
                       cache.write().retain(|t| t.id != id);
                       spawn(async move {
                           let url = format!("{}/api/tea-products/{}", api_base_url(), id);
                           let res = HTTP_CLIENT.clone().delete(&url)
                               .header("X-Telegram-Init-Data", init_data.read().clone())
                               .header("X-Admin-Token", admin_token())
                               .header("X-Admin-Telegram-Id", telegram_id.to_string())
                               .send().await;
                           match res {
                               Ok(r) if r.status().is_success() => { push_toast(toasts, "Чай удалён".into(), ToastKind::Success); TelegramApp::init().haptic_notification(HapticNotification::Success); }
                               _ => {
                                   if let Some(item) = deleted { cache.write().push(item); }
                                   push_toast(toasts, "Не удалось удалить чай".into(), ToastKind::Error);
                                   TelegramApp::init().haptic_notification(HapticNotification::Error);
                               }
                           }
                       });
                   }
               }
               {render_toasts(toasts)}
           }
       }
}

// ── Sets tab (Optimistic UI) ──────────────────────────────────

#[component]
fn SetsTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut cache: Signal<Vec<AdminSet>> = use_signal(|| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(json)) = storage.get_item("wwb_admin_sets") {
                        if json.len() <= 1_000_000 {
                            if let Ok(data) = serde_json::from_str::<Vec<AdminSet>>(&json) {
                                return data;
                            }
                        }
                    }
                }
            }
        }
        Vec::new()
    });
    let mut loading = use_signal(|| true);
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut icon = use_signal(String::new);
    let mut image_url = use_signal(String::new);
    let mut video_url = use_signal(String::new);
    let mut name_en = use_signal(String::new);
    let mut description_en = use_signal(String::new);
    // Phase 3 / next cycle: IDs are now signals of Vec<String> rather
    // than comma-separated text. The IdPicker component mutates these
    // in-place via checkbox toggles.
    let mut strain_ids: Signal<Vec<String>> = use_signal(Vec::new);
    let mut accessory_ids: Signal<Vec<String>> = use_signal(Vec::new);
    let mut total_price = use_signal(String::new);
    let mut discount_percent = use_signal(String::new);
    let mut is_deal_of_day = use_signal(|| false);
    // Packs Phase 1: total weight (grams) + one promo badge.
    let mut total_weight = use_signal(String::new);
    let mut badge = use_signal(|| "none".to_string());
    let mut status = use_signal(String::new);
    let mut submitting = use_signal(|| false);
    let mut editing_id: Signal<Option<String>> = use_signal(|| None);
    let mut delete_target_id: Signal<Option<String>> = use_signal(|| None);
    let mut search_query = use_signal(String::new);
    let reload = use_signal(|| 0u32);
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);

    // Catalog fetches for the IdPicker — separate from the main sets
    // cache because they read different endpoints. Both are admin
    // views (include_hidden=1) so the picker can target unavailable
    // items if needed.
    let mut strain_catalog: Signal<Vec<(String, String)>> = use_signal(Vec::new);
    let mut accessory_catalog: Signal<Vec<(String, String)>> = use_signal(Vec::new);
    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/strains?include_hidden=1", api_base_url());
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                if let Ok(data) = resp.json::<StrainsResp>().await {
                    strain_catalog.set(data.strains.into_iter().map(|s| (s.id, s.name)).collect());
                }
            }
            Some(())
        }
    });
    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/accessories?include_hidden=1", api_base_url());
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                if let Ok(data) = resp.json::<AccessoriesResp>().await {
                    accessory_catalog.set(
                        data.accessories
                            .into_iter()
                            .map(|a| (a.id, a.name))
                            .collect(),
                    );
                }
            }
            Some(())
        }
    });

    use_effect(move || {
        if editing_id.read().is_some() {
            let _ = js_sys::eval("setTimeout(()=>{var el=document.querySelector('[data-editing]');if(el)el.scrollIntoView({behavior:'smooth',block:'center'});},100);");
        }
    });

    use_effect(move || {
        let data = cache.read().clone();
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item(
                        "wwb_admin_sets",
                        &serde_json::to_string(&data).unwrap_or_default(),
                    );
                }
            }
        }
    });

    let _ = use_resource(move || {
        let _ = reload.read();
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/sets?include_hidden=1", api_base_url());
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                if let Ok(data) = resp.json::<SetsResp>().await {
                    cache.set(data.sets);
                }
            }
            loading.set(false);
            Some(())
        }
    });

    let filtered: Vec<AdminSet> = {
        let q = search_query.read().to_lowercase();
        cache
            .read()
            .iter()
            .filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q))
            .cloned()
            .collect()
    };

    rsx! {
        div {
            FormCard {
                title: "Добавить сет".to_string(),
                children: rsx!{
                    input { style: input_style(), placeholder: "Название", value: "{name}",
                        oninput: move |e| name.set(e.value()) }
                    textarea { style: textarea_style(), placeholder: "Описание", value: "{description}",
                        oninput: move |e| description.set(e.value()) }
                    input { style: input_style(), placeholder: "Иконка (emoji или URL)", value: "{icon}",
                        oninput: move |e| icon.set(e.value()) }
                    ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
                    VideoUpload { video_url: video_url.read().clone(), on_change: move |url: String| video_url.set(url) }
                    input { style: input_style(), placeholder: "Название (EN)", value: "{name_en}",
                        oninput: move |e| name_en.set(e.value()) }
                    textarea { style: textarea_style(), placeholder: "Описание (EN)", value: "{description_en}",
                        oninput: move |e| description_en.set(e.value()) }
                    IdPicker {
                        label: "Сорта".to_string(),
                        options: strain_catalog.read().clone(),
                        selected: strain_ids,
                    }
                    IdPicker {
                        label: "Аксессуары".to_string(),
                        options: accessory_catalog.read().clone(),
                        selected: accessory_ids,
                    }
                    input { style: input_style(), placeholder: "Цена ฿", value: "{total_price}", r#type: "number",
                        oninput: move |e| total_price.set(e.value()) }
                    input { style: input_style(), placeholder: "Скидка %", value: "{discount_percent}", r#type: "number",
                        oninput: move |e| discount_percent.set(e.value()) }
                    input { style: input_style(), placeholder: "Общий вес (г), напр. 10", value: "{total_weight}", r#type: "number",
                        oninput: move |e| total_weight.set(e.value()) }
                    select { style: input_style(), value: "{badge}", oninput: move |e| badge.set(e.value()),
                        option { value: "none", "Без бейджа" }
                        option { value: "sale", "🔴 SALE" }
                        option { value: "special", "🟡 SPECIAL OFFER" }
                        option { value: "limited", "🟣 LIMITED EDITION" }
                    }
                    label { style: "display:flex;align-items:center;gap:8px;font-size:13px;color:#888;",
                        input { r#type: "checkbox", checked: is_deal_of_day(),
                            onchange: move |e| is_deal_of_day.set(e.checked()) }
                        "Deal of the day"
                    }
                    button {
                        style: if *submitting.read() { submit_btn_disabled_style() } else { submit_btn_style() },
                        disabled: *submitting.read(),
                        onclick: move |_| {
                            let n = name().trim().to_string();
                            let p = match total_price.read().trim().parse::<f64>() {
                                Ok(v) if v >= 0.0 && v.is_finite() => v,
                                _ => { status.set("❌ Цена должна быть числом ≥ 0".into()); return; }
                            };
                            let d = match discount_percent.read().trim().parse::<f64>() {
                                Ok(v) if v.is_finite() => v,
                                _ => { status.set("❌ Скидка должна быть числом".into()); return; }
                            };
                            // Weight is optional; empty → 0. Bound-check to the server range.
                            let w = match total_weight.read().trim() {
                                "" => 0.0,
                                s => match s.parse::<f64>() {
                                    Ok(v) if v.is_finite() && (0.0..=100_000.0).contains(&v) => v,
                                    _ => { status.set("❌ Вес: число 0–100000".into()); return; }
                                },
                            };
                            let bdg = badge();
                            if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                            let s_ids: Vec<String> = strain_ids.read().clone();
                            let a_ids: Vec<String> = accessory_ids.read().clone();
                            let desc = description();
                            let ic = icon(); let img = image_url(); let vid = video_url();
                            let ne = name_en(); let de = description_en();
                            let deal = is_deal_of_day();
                            submitting.set(true);
                            let temp_id = format!("temp-{}", uuid::Uuid::new_v4());
                            cache.write().insert(0, AdminSet {
                                id: temp_id.clone(), name: n.clone(),
                                description: if desc.is_empty() { None } else { Some(desc.clone()) },
                                icon: if ic.is_empty() { None } else { Some(ic.clone()) },
                                image_url: if img.is_empty() { None } else { Some(img.clone()) },
                                video_url: if vid.is_empty() { None } else { Some(vid.clone()) },
                                strain_ids: s_ids.clone(), accessory_ids: a_ids.clone(),
                                total_price: p, discount_percent: d,
                                is_available: true, is_deal_of_day: deal,
                                name_en: if ne.is_empty() { None } else { Some(ne.clone()) },
                                description_en: if de.is_empty() { None } else { Some(de.clone()) },
                                total_weight_grams: w, badge: bdg.clone(),
                            });
                            status.set("✅ Добавлен!".into());
                            name.set(String::new()); description.set(String::new()); icon.set(String::new());
                            image_url.set(String::new()); video_url.set(String::new());
                            name_en.set(String::new()); description_en.set(String::new());
                            strain_ids.set(Vec::new()); accessory_ids.set(Vec::new());
                            total_price.set(String::new()); discount_percent.set(String::new());
                            is_deal_of_day.set(false);
                            total_weight.set(String::new()); badge.set("none".to_string());
                            auto_scroll_to_list();
                            spawn(async move {
                                let body = json!({
                                    "name": n, "total_price": p, "discount_percent": d,
                                    "description": if desc.is_empty() { serde_json::Value::Null } else { desc.into() },
                                    "icon": if ic.is_empty() { serde_json::Value::Null } else { ic.into() },
                                    "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                    "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                    "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                    "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                    "strain_ids": s_ids, "accessory_ids": a_ids,
                                    "is_deal_of_day": deal,
                                    "total_weight_grams": w, "badge": bdg,
                                });
                                let url = format!("{}/api/sets", api_base_url());
                                let res = HTTP_CLIENT.clone().post(&url)
                                    .header("X-Telegram-Init-Data", init_data.read().clone())
                                    .header("X-Admin-Token", admin_token())
                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                submitting.set(false);
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        if let Ok(data) = r.json::<serde_json::Value>().await {
                                            if let Some(real_id) = data["id"].as_str() {
                                                if let Some(s) = cache.write().iter_mut().find(|s| s.id == temp_id) { s.id = real_id.to_string(); }
                                            }
                                        }
                                        TelegramApp::init().haptic_notification(HapticNotification::Success);
                                    }
                                    _ => {
                                        cache.write().retain(|s| s.id != temp_id);
                                        status.set("❌ Ошибка добавления".into());
                                        TelegramApp::init().haptic_notification(HapticNotification::Error);
                                    }
                                }
                            });
                        },
                        if *submitting.read() { "⏳..." } else { "➕ Добавить" }
                    }
                    {render_status(status)}
                }
            }
            h3 { style: list_title_style(), "Сеты ({filtered.len()})" }
            {render_search(search_query)}
            if *loading.read() {
                div { style: "display:flex;flex-direction:column;gap:8px;",
                    for _ in 0..4 {
                        div { style: "background:#1a1a2e;padding:8px 10px;border-radius:6px;display:flex;align-items:center;gap:6px;",
                            Skeleton { shape: SkeletonShape::Avatar }
                            div { style: "flex:1;display:flex;flex-direction:column;gap:4px;",
                                Skeleton { shape: SkeletonShape::Text, width: Some("60%".into()) }
                                Skeleton { shape: SkeletonShape::TextSm, width: Some("40%".into()) }
                            }
                        }
                    }
                }
            } else if cache.read().is_empty() && search_query.read().is_empty() {
                EmptyState {
                    icon: "📦",
                    title: "Пока ничего нет",
                    description: "Заполните форму выше — кнопка «➕ Добавить» создаст первую запись.",
                }
            } else if filtered.is_empty() {
                EmptyState {
                    icon: "🔍",
                    title: "Ничего не найдено",
                    description: "Попробуйте изменить запрос поиска",
                    action: rsx! {
                        button {
                            style: "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:13px;cursor:pointer;",
                            onclick: move |_| search_query.set(String::new()),
                            "Очистить поиск"
                        }
                    },
                }
            } else {
                div { "data-list": "true", style: "display:flex;flex-direction:column;gap:8px;",
                    for s in filtered {
                        if editing_id.read().as_deref() == Some(s.id.as_str()) {
                            EditSetCard {
                                key: "{s.id}",
                                item: s.clone(),
                                cache: cache,
                                strain_catalog: strain_catalog.read().clone(),
                                accessory_catalog: accessory_catalog.read().clone(),
                                on_saved: move |_| editing_id.set(None),
                                on_cancel: move |_| editing_id.set(None),
                            }
                        } else {
                            ItemRow {
                                key: "{s.id}",
                                name: s.name.clone(),
                                sub: format!("{} strains • {} accessories • {}฿", s.strain_ids.len(), s.accessory_ids.len(), s.total_price),
                                is_available: s.is_available,
                                image_url: s.icon.clone().filter(|i| i.starts_with("http://") || i.starts_with("https://") || (i.starts_with("/") && !i.starts_with("//"))),
                                video_url: s.video_url.clone(),
                                on_edit: { let id = s.id.clone(); move |_| editing_id.set(Some(id.clone())) },
                                on_toggle: {
                                    let id = s.id.clone(); let next = !s.is_available;
                                    move |_| {
                                        let id = id.clone();
                                        if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) { s.is_available = next; }
                                        spawn(async move {
                                            let url = format!("{}/api/sets/{}/availability", api_base_url(), id);
                                            let res = HTTP_CLIENT.clone().put(&url)
                                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                                .header("X-Admin-Token", admin_token())
                                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                .json(&json!({ "is_available": next })).send().await;
                                            match res {
                                                Ok(r) if r.status().is_success() => { TelegramApp::init().haptic_notification(HapticNotification::Success); }
                                                _ => {
                                                    if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) { s.is_available = !next; }
                                                    push_toast(toasts, "Не удалось изменить статус сета".into(), ToastKind::Error);
                                                    TelegramApp::init().haptic_notification(HapticNotification::Error);
                                                }
                                            }
                                        });
                                    }
                                },
                                on_delete: { let id = s.id.clone(); move |_| delete_target_id.set(Some(id.clone())) }
                            }
                        }
                    }
                }
            }
            DeleteConfirmModal {
                target: delete_target_id,
                item_name: "сет".to_string(),
                on_confirm: move |id: String| {
                    let deleted = cache.read().iter().find(|s| s.id == id).cloned();
                    cache.write().retain(|s| s.id != id);
                    spawn(async move {
                        let url = format!("{}/api/sets/{}", api_base_url(), id);
                        let res = HTTP_CLIENT.clone().delete(&url)
                            .header("X-Telegram-Init-Data", init_data.read().clone())
                            .header("X-Admin-Token", admin_token())
                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                            .send().await;
                        match res {
                            Ok(r) if r.status().is_success() => { push_toast(toasts, "Сет удалён".into(), ToastKind::Success); TelegramApp::init().haptic_notification(HapticNotification::Success); }
                            _ => {
                                if let Some(item) = deleted { cache.write().push(item); }
                                push_toast(toasts, "Не удалось удалить сет".into(), ToastKind::Error);
                                TelegramApp::init().haptic_notification(HapticNotification::Error);
                            }
                        }
                    });
                }
            }
            {render_toasts(toasts)}
        }
    }
}

#[component]
fn EditSetCard(
    item: AdminSet,
    cache: Signal<Vec<AdminSet>>,
    strain_catalog: Vec<(String, String)>,
    accessory_catalog: Vec<(String, String)>,
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut name = use_signal(|| item.name.clone());
    let mut description = use_signal(|| item.description.clone().unwrap_or_default());
    let mut icon = use_signal(|| item.icon.clone().unwrap_or_default());
    let mut image_url = use_signal(|| item.image_url.clone().unwrap_or_default());
    let mut video_url = use_signal(|| item.video_url.clone().unwrap_or_default());
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let strain_ids: Signal<Vec<String>> = use_signal(|| item.strain_ids.clone());
    let accessory_ids: Signal<Vec<String>> = use_signal(|| item.accessory_ids.clone());
    let mut total_price = use_signal(|| item.total_price.to_string());
    let mut discount_percent = use_signal(|| item.discount_percent.to_string());
    let mut is_deal_of_day = use_signal(|| item.is_deal_of_day);
    let mut total_weight = use_signal(|| {
        if item.total_weight_grams > 0.0 {
            item.total_weight_grams.to_string()
        } else {
            String::new()
        }
    });
    let mut badge = use_signal(|| {
        if item.badge.is_empty() {
            "none".to_string()
        } else {
            item.badge.clone()
        }
    });
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    let is_available = item.is_available;
    rsx! {
        div { "data-editing": "true", style: edit_card_style(),
            div { style: edit_header_style(), "✏️ Редактирование" }
            input { style: input_style(), placeholder: "Название", value: "{name}", oninput: move |e| name.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Описание", value: "{description}", oninput: move |e| description.set(e.value()) }
            input { style: input_style(), placeholder: "Иконка", value: "{icon}", oninput: move |e| icon.set(e.value()) }
            ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
            VideoUpload { video_url: video_url.read().clone(), on_change: move |url: String| video_url.set(url) }
            input { style: input_style(), placeholder: "Название (EN)", value: "{name_en}", oninput: move |e| name_en.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Описание (EN)", value: "{description_en}", oninput: move |e| description_en.set(e.value()) }
            IdPicker {
                label: "Сорта".to_string(),
                options: strain_catalog,
                selected: strain_ids,
            }
            IdPicker {
                label: "Аксессуары".to_string(),
                options: accessory_catalog,
                selected: accessory_ids,
            }
            input { style: input_style(), placeholder: "Цена ฿", value: "{total_price}", r#type: "number", oninput: move |e| total_price.set(e.value()) }
            input { style: input_style(), placeholder: "Скидка %", value: "{discount_percent}", r#type: "number", oninput: move |e| discount_percent.set(e.value()) }
            input { style: input_style(), placeholder: "Общий вес (г)", value: "{total_weight}", r#type: "number", oninput: move |e| total_weight.set(e.value()) }
            select { style: input_style(), value: "{badge}", oninput: move |e| badge.set(e.value()),
                option { value: "none", "Без бейджа" }
                option { value: "sale", "🔴 SALE" }
                option { value: "special", "🟡 SPECIAL OFFER" }
                option { value: "limited", "🟣 LIMITED EDITION" }
            }
            label { style: "display:flex;align-items:center;gap:8px;font-size:13px;color:#888;",
                input { r#type: "checkbox", checked: is_deal_of_day(),
                    onchange: move |e| is_deal_of_day.set(e.checked()) }
                "Deal of the day"
            }
            div { style: "display:flex;gap:8px;",
                button { style: submit_btn_style(),
                    onclick: move |_| {
                        let n = name().trim().to_string();
                        let p = match total_price.read().trim().parse::<f64>() {
                            Ok(v) if v >= 0.0 && v.is_finite() => v,
                            _ => { status.set("❌ Цена должна быть числом ≥ 0".into()); return; }
                        };
                        let d = match discount_percent.read().trim().parse::<f64>() {
                            Ok(v) if v.is_finite() => v,
                            _ => { status.set("❌ Скидка должна быть числом".into()); return; }
                        };
                        let w = match total_weight.read().trim() {
                            "" => 0.0,
                            s => match s.parse::<f64>() {
                                Ok(v) if v.is_finite() && (0.0..=100_000.0).contains(&v) => v,
                                _ => { status.set("❌ Вес: число 0–100000".into()); return; }
                            },
                        };
                        let bdg = badge();
                        if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                        let desc = description();
                        let ic = icon(); let img = image_url(); let vid = video_url();
                        let ne = name_en(); let de = description_en();
                        let s_ids: Vec<String> = strain_ids.read().clone();
                        let a_ids: Vec<String> = accessory_ids.read().clone();
                        let deal = is_deal_of_day();
                        let id = item_id.clone();
                        let original = cache.read().iter().find(|s| s.id == id).cloned();
                        if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) {
                            s.name = n.clone();
                            s.description = if desc.is_empty() { None } else { Some(desc.clone()) };
                            s.icon = if ic.is_empty() { None } else { Some(ic.clone()) };
                            s.image_url = if img.is_empty() { None } else { Some(img.clone()) };
                            s.video_url = if vid.is_empty() { None } else { Some(vid.clone()) };
                            s.strain_ids = s_ids.clone();
                            s.accessory_ids = a_ids.clone();
                            s.total_price = p;
                            s.discount_percent = d;
                            s.is_deal_of_day = deal;
                            s.is_available = is_available;
                            s.name_en = if ne.is_empty() { None } else { Some(ne.clone()) };
                            s.description_en = if de.is_empty() { None } else { Some(de.clone()) };
                            s.total_weight_grams = w;
                            s.badge = bdg.clone();
                        }
                        spawn(async move {
                            let body = json!({
                                "name": n, "total_price": p, "discount_percent": d,
                                "description": if desc.is_empty() { serde_json::Value::Null } else { desc.into() },
                                "icon": if ic.is_empty() { serde_json::Value::Null } else { ic.into() },
                                "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                "strain_ids": s_ids, "accessory_ids": a_ids,
                                "is_deal_of_day": deal,
                                "is_available": is_available,
                                "total_weight_grams": w, "badge": bdg,
                            });
                            let url = format!("{}/api/sets/{}", api_base_url(), id);
                            let res = HTTP_CLIENT.clone().put(&url)
                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                .header("X-Admin-Token", admin_token())
                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                .json(&body).send().await;
                            // Surface the real reason (HTTP status + body) instead
                            // of an opaque "Не сохранено", like the accessory/garden fix.
                            let outcome = match res {
                                Ok(r) if r.status().is_success() => Ok(()),
                                Ok(r) => {
                                    let st = r.status().as_u16();
                                    let body = r.text().await.unwrap_or_default();
                                    let b = body.trim();
                                    if b.is_empty() { Err(format!("HTTP {st}")) }
                                    else { Err(format!("HTTP {st}: {}", b.chars().take(80).collect::<String>())) }
                                }
                                Err(_) => Err("сеть/таймаут".to_string()),
                            };
                            match outcome {
                                Ok(()) => {
                                    on_saved.call(());
                                    TelegramApp::init().haptic_notification(HapticNotification::Success);
                                }
                                Err(reason) => {
                                    TelegramApp::init().haptic_notification(HapticNotification::Error);
                                    if let Some(orig) = original { if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) { *s = orig; } }
                                    status.set(format!("❌ Не сохранено ({reason})"));
                                }
                            }
                        });
                    },
                    "💾 Сохранить"
                }
                button { style: cancel_btn_style(), onclick: move |_| on_cancel.call(()), "Отмена" }
            }
            {render_status(status)}
        }
    }
}

// ── Accessory Sets tab (Optimistic UI) ────────────────────────

#[component]
fn AccessorySetsTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut cache: Signal<Vec<AdminAccessorySet>> = use_signal(|| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(json)) = storage.get_item("wwb_admin_accessory_sets") {
                        if json.len() <= 1_000_000 {
                            if let Ok(data) = serde_json::from_str::<Vec<AdminAccessorySet>>(&json)
                            {
                                return data;
                            }
                        }
                    }
                }
            }
        }
        Vec::new()
    });
    let mut loading = use_signal(|| true);
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut icon = use_signal(String::new);
    let mut image_url = use_signal(String::new);
    let mut video_url = use_signal(String::new);
    // Cycle (this commit): textarea-of-comma-separated-IDs replaced by
    // IdPicker checkbox list — same pattern as SetsTab.
    let mut accessories: Signal<Vec<String>> = use_signal(Vec::new);
    let mut total_price = use_signal(String::new);
    let mut discount_percent = use_signal(String::new);
    let mut is_deal_of_day = use_signal(|| false);
    let mut name_en = use_signal(String::new);
    let mut description_en = use_signal(String::new);
    let mut status = use_signal(String::new);
    let mut submitting = use_signal(|| false);
    let mut editing_id: Signal<Option<String>> = use_signal(|| None);
    let mut delete_target_id: Signal<Option<String>> = use_signal(|| None);
    let mut search_query = use_signal(String::new);
    let reload = use_signal(|| 0u32);
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);

    // Catalog fetch for the IdPicker — accessories admin endpoint.
    let mut accessory_catalog: Signal<Vec<(String, String)>> = use_signal(Vec::new);
    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/accessories?include_hidden=1", api_base_url());
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                if let Ok(data) = resp.json::<AccessoriesResp>().await {
                    accessory_catalog.set(
                        data.accessories
                            .into_iter()
                            .map(|a| (a.id, a.name))
                            .collect(),
                    );
                }
            }
            Some(())
        }
    });

    use_effect(move || {
        if editing_id.read().is_some() {
            let _ = js_sys::eval("setTimeout(()=>{var el=document.querySelector('[data-editing]');if(el)el.scrollIntoView({behavior:'smooth',block:'center'});},100);");
        }
    });

    use_effect(move || {
        let data = cache.read().clone();
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item(
                        "wwb_admin_accessory_sets",
                        &serde_json::to_string(&data).unwrap_or_default(),
                    );
                }
            }
        }
    });

    let _ = use_resource(move || {
        let _ = reload.read();
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/accessory-sets?include_hidden=1", api_base_url());
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                if let Ok(data) = resp.json::<AccessorySetsResp>().await {
                    cache.set(data.accessory_sets);
                }
            }
            loading.set(false);
            Some(())
        }
    });

    let filtered: Vec<AdminAccessorySet> = {
        let q = search_query.read().to_lowercase();
        cache
            .read()
            .iter()
            .filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q))
            .cloned()
            .collect()
    };

    rsx! {
        div {
            FormCard {
                title: "Добавить набор аксессуаров".to_string(),
                children: rsx!{
                    input { style: input_style(), placeholder: "Название (RU)", value: "{name}",
                        oninput: move |e| name.set(e.value()) }
                    textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description}",
                        oninput: move |e| description.set(e.value()) }
                    input { style: input_style(), placeholder: "Иконка (emoji или URL)", value: "{icon}",
                        oninput: move |e| icon.set(e.value()) }
                    ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
                    VideoUpload { video_url: video_url.read().clone(), on_change: move |url: String| video_url.set(url) }
                    IdPicker {
                        label: "Аксессуары".to_string(),
                        options: accessory_catalog.read().clone(),
                        selected: accessories,
                    }
                    input { style: input_style(), placeholder: "Цена ฿", value: "{total_price}", r#type: "number",
                        oninput: move |e| total_price.set(e.value()) }
                    input { style: input_style(), placeholder: "Скидка %", value: "{discount_percent}", r#type: "number",
                        oninput: move |e| discount_percent.set(e.value()) }
                    label { style: "display:flex;align-items:center;gap:8px;font-size:13px;color:#888;",
                        input { r#type: "checkbox", checked: is_deal_of_day(),
                            onchange: move |e| is_deal_of_day.set(e.checked()) }
                        "Deal of the day"
                    }
                    div { style: en_section_style(), "🇬🇧 English" }
                    input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}",
                        oninput: move |e| name_en.set(e.value()) }
                    textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}",
                        oninput: move |e| description_en.set(e.value()) }
                    button {
                        style: if *submitting.read() { submit_btn_disabled_style() } else { submit_btn_style() },
                        disabled: *submitting.read(),
                        onclick: move |_| {
                            let n = name().trim().to_string();
                            let p = match total_price.read().trim().parse::<f64>() {
                                Ok(v) if v >= 0.0 && v.is_finite() => v,
                                _ => { status.set("❌ Цена должна быть числом ≥ 0".into()); return; }
                            };
                            let d = match discount_percent.read().trim().parse::<f64>() {
                                Ok(v) if v >= 0.0 && v <= 100.0 && v.is_finite() => v,
                                _ => { status.set("❌ Скидка должна быть числом 0–100".into()); return; }
                            };
                            if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                            let accs: Vec<String> = accessories.read().clone();
                            let desc = description();
                            let ic = icon(); let img = image_url(); let vid = video_url();
                            let deal = is_deal_of_day();
                            let ne = name_en(); let de = description_en();
                            submitting.set(true);
                            let temp_id = format!("temp-{}", uuid::Uuid::new_v4());
                            cache.write().insert(0, AdminAccessorySet {
                                id: temp_id.clone(), name: n.clone(),
                                description: if desc.is_empty() { None } else { Some(desc.clone()) },
                                icon: if ic.is_empty() { None } else { Some(ic.clone()) },
                                accessories: accs.clone(),
                                total_price: p, discount_percent: d,
                                is_available: true, is_deal_of_day: deal,
                                name_en: if ne.is_empty() { None } else { Some(ne.clone()) },
                                description_en: if de.is_empty() { None } else { Some(de.clone()) },
                                image_url: if img.is_empty() { None } else { Some(img.clone()) },
                                video_url: if vid.is_empty() { None } else { Some(vid.clone()) },
                            });
                            status.set("✅ Добавлен!".into());
                            name.set(String::new()); description.set(String::new()); icon.set(String::new()); image_url.set(String::new()); video_url.set(String::new());
                            accessories.set(Vec::new()); total_price.set(String::new()); discount_percent.set(String::new());
                            is_deal_of_day.set(false); name_en.set(String::new()); description_en.set(String::new());
                            auto_scroll_to_list();
                            spawn(async move {
                                let body = json!({
                                    "name": n, "total_price": p, "discount_percent": d,
                                    "description": if desc.is_empty() { serde_json::Value::Null } else { desc.into() },
                                    "icon": if ic.is_empty() { serde_json::Value::Null } else { ic.into() },
                                    "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                    "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                    "accessories": accs,
                                    "is_deal_of_day": deal,
                                    "is_available": true,
                                    "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                    "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                });
                                let url = format!("{}/api/accessory-sets", api_base_url());
                                let res = HTTP_CLIENT.clone().post(&url)
                                    .header("X-Telegram-Init-Data", init_data.read().clone())
                                    .header("X-Admin-Token", admin_token())
                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                submitting.set(false);
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        if let Ok(data) = r.json::<serde_json::Value>().await {
                                            if let Some(real_id) = data["id"].as_str() {
                                                if let Some(s) = cache.write().iter_mut().find(|s| s.id == temp_id) { s.id = real_id.to_string(); }
                                            }
                                        }
                                        TelegramApp::init().haptic_notification(HapticNotification::Success);
                                    }
                                    _ => {
                                        cache.write().retain(|s| s.id != temp_id);
                                        status.set("❌ Ошибка добавления".into());
                                        TelegramApp::init().haptic_notification(HapticNotification::Error);
                                    }
                                }
                            });
                        },
                        if *submitting.read() { "⏳..." } else { "➕ Добавить" }
                    }
                    {render_status(status)}
                }
            }
            h3 { style: list_title_style(), "Наборы аксессуаров ({filtered.len()})" }
            {render_search(search_query)}
            if *loading.read() {
                div { style: "display:flex;flex-direction:column;gap:8px;",
                    for _ in 0..4 {
                        div { style: "background:#1a1a2e;padding:8px 10px;border-radius:6px;display:flex;align-items:center;gap:6px;",
                            Skeleton { shape: SkeletonShape::Avatar }
                            div { style: "flex:1;display:flex;flex-direction:column;gap:4px;",
                                Skeleton { shape: SkeletonShape::Text, width: Some("60%".into()) }
                                Skeleton { shape: SkeletonShape::TextSm, width: Some("40%".into()) }
                            }
                        }
                    }
                }
            } else if cache.read().is_empty() && search_query.read().is_empty() {
                EmptyState {
                    icon: "📦",
                    title: "Пока ничего нет",
                    description: "Заполните форму выше — кнопка «➕ Добавить» создаст первую запись.",
                }
            } else if filtered.is_empty() {
                EmptyState {
                    icon: "🔍",
                    title: "Ничего не найдено",
                    description: "Попробуйте изменить запрос поиска",
                    action: rsx! {
                        button {
                            style: "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:13px;cursor:pointer;",
                            onclick: move |_| search_query.set(String::new()),
                            "Очистить поиск"
                        }
                    },
                }
            } else {
                div { "data-list": "true", style: "display:flex;flex-direction:column;gap:8px;",
                    for s in filtered {
                        if editing_id.read().as_deref() == Some(s.id.as_str()) {
                            EditAccessorySetCard {
                                key: "{s.id}",
                                item: s.clone(),
                                cache: cache,
                                accessory_catalog: accessory_catalog.read().clone(),
                                on_saved: move |_| editing_id.set(None),
                                on_cancel: move |_| editing_id.set(None),
                            }
                        } else {
                            ItemRow {
                                key: "{s.id}",
                                name: s.name.clone(),
                                sub: format!("{} items • {}฿", s.accessories.len(), s.total_price),
                                is_available: s.is_available,
                                image_url: s.image_url.clone().filter(|i| i.starts_with("http://") || i.starts_with("https://") || (i.starts_with("/") && !i.starts_with("//"))).or_else(|| s.icon.clone().filter(|i| i.starts_with("http://") || i.starts_with("https://") || (i.starts_with("/") && !i.starts_with("//")))),
                                video_url: s.video_url.clone(),
                                on_edit: { let id = s.id.clone(); move |_| editing_id.set(Some(id.clone())) },
                                on_toggle: {
                                    let id = s.id.clone(); let next = !s.is_available;
                                    move |_| {
                                        let id = id.clone();
                                        if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) { s.is_available = next; }
                                        spawn(async move {
                                            let url = format!("{}/api/accessory-sets/{}/availability", api_base_url(), id);
                                            let res = HTTP_CLIENT.clone().put(&url)
                                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                                .header("X-Admin-Token", admin_token())
                                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                .json(&json!({ "is_available": next })).send().await;
                                            match res {
                                                Ok(r) if r.status().is_success() => { TelegramApp::init().haptic_notification(HapticNotification::Success); }
                                                _ => {
                                                    if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) { s.is_available = !next; }
                                                    push_toast(toasts, "Не удалось изменить статус".into(), ToastKind::Error);
                                                    TelegramApp::init().haptic_notification(HapticNotification::Error);
                                                }
                                            }
                                        });
                                    }
                                },
                                on_delete: { let id = s.id.clone(); move |_| delete_target_id.set(Some(id.clone())) }
                            }
                        }
                    }
                }
            }
            DeleteConfirmModal {
                target: delete_target_id,
                item_name: "набор".to_string(),
                on_confirm: move |id: String| {
                    let deleted = cache.read().iter().find(|s| s.id == id).cloned();
                    cache.write().retain(|s| s.id != id);
                    spawn(async move {
                        let url = format!("{}/api/accessory-sets/{}", api_base_url(), id);
                        let res = HTTP_CLIENT.clone().delete(&url)
                            .header("X-Telegram-Init-Data", init_data.read().clone())
                            .header("X-Admin-Token", admin_token())
                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                            .send().await;
                        match res {
                            Ok(r) if r.status().is_success() => { push_toast(toasts, "Набор удалён".into(), ToastKind::Success); TelegramApp::init().haptic_notification(HapticNotification::Success); }
                            _ => {
                                if let Some(item) = deleted { cache.write().push(item); }
                                push_toast(toasts, "Не удалось удалить набор".into(), ToastKind::Error);
                                TelegramApp::init().haptic_notification(HapticNotification::Error);
                            }
                        }
                    });
                }
            }
            {render_toasts(toasts)}
        }
    }
}

#[component]
fn EditAccessorySetCard(
    item: AdminAccessorySet,
    cache: Signal<Vec<AdminAccessorySet>>,
    accessory_catalog: Vec<(String, String)>,
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut name = use_signal(|| item.name.clone());
    let mut description = use_signal(|| item.description.clone().unwrap_or_default());
    let mut icon = use_signal(|| item.icon.clone().unwrap_or_default());
    let mut image_url = use_signal(|| item.image_url.clone().unwrap_or_default());
    let mut video_url = use_signal(|| item.video_url.clone().unwrap_or_default());
    let accessories: Signal<Vec<String>> = use_signal(|| item.accessories.clone());
    let mut total_price = use_signal(|| item.total_price.to_string());
    let mut discount_percent = use_signal(|| item.discount_percent.to_string());
    let mut is_deal_of_day = use_signal(|| item.is_deal_of_day);
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    let is_available = item.is_available;
    rsx! {
        div { "data-editing": "true", style: edit_card_style(),
            div { style: edit_header_style(), "✏️ Редактирование" }
            input { style: input_style(), placeholder: "Название (RU)", value: "{name}", oninput: move |e| name.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description}", oninput: move |e| description.set(e.value()) }
            input { style: input_style(), placeholder: "Иконка", value: "{icon}", oninput: move |e| icon.set(e.value()) }
            ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
            VideoUpload { video_url: video_url.read().clone(), on_change: move |url: String| video_url.set(url) }
            IdPicker {
                label: "Аксессуары".to_string(),
                options: accessory_catalog,
                selected: accessories,
            }
            input { style: input_style(), placeholder: "Цена ฿", value: "{total_price}", r#type: "number", oninput: move |e| total_price.set(e.value()) }
            input { style: input_style(), placeholder: "Скидка %", value: "{discount_percent}", r#type: "number", oninput: move |e| discount_percent.set(e.value()) }
            label { style: "display:flex;align-items:center;gap:8px;font-size:13px;color:#888;",
                input { r#type: "checkbox", checked: is_deal_of_day(),
                    onchange: move |e| is_deal_of_day.set(e.checked()) }
                "Deal of the day"
            }
            div { style: en_section_style(), "🇬🇧 English" }
            input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}", oninput: move |e| name_en.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}", oninput: move |e| description_en.set(e.value()) }
            div { style: "display:flex;gap:8px;",
                button { style: submit_btn_style(),
                    onclick: move |_| {
                        let n = name().trim().to_string();
                        let p = match total_price.read().trim().parse::<f64>() {
                            Ok(v) if v >= 0.0 && v.is_finite() => v,
                            _ => { status.set("❌ Цена должна быть числом ≥ 0".into()); return; }
                        };
                        let d = match discount_percent.read().trim().parse::<f64>() {
                            Ok(v) if v >= 0.0 && v <= 100.0 && v.is_finite() => v,
                            _ => { status.set("❌ Скидка должна быть числом 0–100".into()); return; }
                        };
                        if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                        let accs: Vec<String> = accessories.read().clone();
                        let desc = description();
                        let ic = icon(); let img = image_url(); let vid = video_url();
                        let deal = is_deal_of_day();
                        let ne = name_en(); let de = description_en();
                        let id = item_id.clone();
                        let original = cache.read().iter().find(|s| s.id == id).cloned();
                        if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) {
                            s.name = n.clone();
                            s.description = if desc.is_empty() { None } else { Some(desc.clone()) };
                            s.icon = if ic.is_empty() { None } else { Some(ic.clone()) };
                            s.image_url = if img.is_empty() { None } else { Some(img.clone()) };
                            s.video_url = if vid.is_empty() { None } else { Some(vid.clone()) };
                            s.accessories = accs.clone();
                            s.total_price = p;
                            s.discount_percent = d;
                            s.is_deal_of_day = deal;
                            s.is_available = is_available;
                            s.name_en = if ne.is_empty() { None } else { Some(ne.clone()) };
                            s.description_en = if de.is_empty() { None } else { Some(de.clone()) };
                        }
                        spawn(async move {
                            let body = json!({
                                "name": n, "total_price": p, "discount_percent": d,
                                "description": if desc.is_empty() { serde_json::Value::Null } else { desc.into() },
                                "icon": if ic.is_empty() { serde_json::Value::Null } else { ic.into() },
                                "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                "accessories": accs,
                                "is_deal_of_day": deal,
                                "is_available": is_available,
                                "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                            });
                            let url = format!("{}/api/accessory-sets/{}", api_base_url(), id);
                            let res = HTTP_CLIENT.clone().put(&url)
                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                .header("X-Admin-Token", admin_token())
                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                .json(&body).send().await;
                            let outcome = match res {
                                Ok(r) if r.status().is_success() => Ok(()),
                                Ok(r) => {
                                    let st = r.status().as_u16();
                                    let body = r.text().await.unwrap_or_default();
                                    let b = body.trim();
                                    if b.is_empty() { Err(format!("HTTP {st}")) }
                                    else { Err(format!("HTTP {st}: {}", b.chars().take(80).collect::<String>())) }
                                }
                                Err(_) => Err("сеть/таймаут".to_string()),
                            };
                            match outcome {
                                Ok(()) => {
                                    on_saved.call(());
                                    status.set("✅ Сохранено".into());
                                    TelegramApp::init().haptic_notification(HapticNotification::Success);
                                }
                                Err(reason) => {
                                    TelegramApp::init().haptic_notification(HapticNotification::Error);
                                    if let Some(orig) = original { if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) { *s = orig; } }
                                    status.set(format!("❌ Не сохранено ({reason})"));
                                }
                            }
                        });
                    },
                    "💾 Сохранить"
                }
                button { style: cancel_btn_style(), onclick: move |_| on_cancel.call(()), "Отмена" }
            }
            {render_status(status)}
        }
    }
}

// ── Tea Sets tab (Optimistic UI) ──────────────────────────────

#[component]
fn TeaSetsTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut cache: Signal<Vec<AdminTeaSet>> = use_signal(|| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(json)) = storage.get_item("wwb_admin_tea_sets") {
                        if json.len() <= 1_000_000 {
                            if let Ok(data) = serde_json::from_str::<Vec<AdminTeaSet>>(&json) {
                                return data;
                            }
                        }
                    }
                }
            }
        }
        Vec::new()
    });
    let mut loading = use_signal(|| true);
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut icon = use_signal(String::new);
    let mut image_url = use_signal(String::new);
    let mut video_url = use_signal(String::new);
    // Cycle (this commit): same upgrade as SetsTab/AccessorySetsTab.
    let mut items: Signal<Vec<String>> = use_signal(Vec::new);
    let mut total_price = use_signal(String::new);
    let mut discount_percent = use_signal(String::new);
    let mut name_en = use_signal(String::new);
    let mut description_en = use_signal(String::new);
    let mut status = use_signal(String::new);
    let mut submitting = use_signal(|| false);
    let mut editing_id: Signal<Option<String>> = use_signal(|| None);
    let mut delete_target_id: Signal<Option<String>> = use_signal(|| None);
    let mut search_query = use_signal(String::new);
    let reload = use_signal(|| 0u32);
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);

    // Catalog fetch for the IdPicker — tea-products admin endpoint.
    let mut tea_catalog: Signal<Vec<(String, String)>> = use_signal(Vec::new);
    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/tea-products?include_hidden=1", api_base_url());
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                if let Ok(data) = resp.json::<TeaResp>().await {
                    tea_catalog.set(
                        data.tea_products
                            .into_iter()
                            .map(|t| (t.id, t.name))
                            .collect(),
                    );
                }
            }
            Some(())
        }
    });

    use_effect(move || {
        if editing_id.read().is_some() {
            let _ = js_sys::eval("setTimeout(()=>{var el=document.querySelector('[data-editing]');if(el)el.scrollIntoView({behavior:'smooth',block:'center'});},100);");
        }
    });

    use_effect(move || {
        let data = cache.read().clone();
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item(
                        "wwb_admin_tea_sets",
                        &serde_json::to_string(&data).unwrap_or_default(),
                    );
                }
            }
        }
    });

    let _ = use_resource(move || {
        let _ = reload.read();
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/tea-sets?include_hidden=1", api_base_url());
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                if let Ok(data) = resp.json::<TeaSetsResp>().await {
                    cache.set(data.tea_sets);
                }
            }
            loading.set(false);
            Some(())
        }
    });

    let filtered: Vec<AdminTeaSet> = {
        let q = search_query.read().to_lowercase();
        cache
            .read()
            .iter()
            .filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q))
            .cloned()
            .collect()
    };

    rsx! {
        div {
            FormCard {
                title: "Добавить набор чая".to_string(),
                children: rsx!{
                    input { style: input_style(), placeholder: "Название (RU)", value: "{name}",
                        oninput: move |e| name.set(e.value()) }
                    textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description}",
                        oninput: move |e| description.set(e.value()) }
                    input { style: input_style(), placeholder: "Иконка (emoji или URL)", value: "{icon}",
                        oninput: move |e| icon.set(e.value()) }
                    ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
                    VideoUpload { video_url: video_url.read().clone(), on_change: move |url: String| video_url.set(url) }
                    IdPicker {
                        label: "Чай".to_string(),
                        options: tea_catalog.read().clone(),
                        selected: items,
                    }
                    input { style: input_style(), placeholder: "Цена ฿", value: "{total_price}", r#type: "number",
                        oninput: move |e| total_price.set(e.value()) }
                    input { style: input_style(), placeholder: "Скидка %", value: "{discount_percent}", r#type: "number",
                        oninput: move |e| discount_percent.set(e.value()) }
                    div { style: en_section_style(), "🇬🇧 English" }
                    input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}",
                        oninput: move |e| name_en.set(e.value()) }
                    textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}",
                        oninput: move |e| description_en.set(e.value()) }
                    button {
                        style: if *submitting.read() { submit_btn_disabled_style() } else { submit_btn_style() },
                        disabled: *submitting.read(),
                        onclick: move |_| {
                            let n = name().trim().to_string();
                            let p = match total_price.read().trim().parse::<f64>() {
                                Ok(v) if v >= 0.0 && v.is_finite() => v,
                                _ => { status.set("❌ Цена должна быть числом ≥ 0".into()); return; }
                            };
                            let d = match discount_percent.read().trim().parse::<f64>() {
                                Ok(v) if v.is_finite() => v,
                                _ => { status.set("❌ Скидка должна быть числом".into()); return; }
                            };
                            if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                            let tea_items: Vec<String> = items.read().clone();
                            let desc = description();
                            let ic = icon(); let img = image_url(); let vid = video_url();
                            let ne = name_en(); let de = description_en();
                            submitting.set(true);
                            let temp_id = format!("temp-{}", uuid::Uuid::new_v4());
                            cache.write().insert(0, AdminTeaSet {
                                id: temp_id.clone(), name: n.clone(),
                                description: if desc.is_empty() { None } else { Some(desc.clone()) },
                                icon: if ic.is_empty() { None } else { Some(ic.clone()) },
                                image_url: if img.is_empty() { None } else { Some(img.clone()) },
                                video_url: if vid.is_empty() { None } else { Some(vid.clone()) },
                                items: tea_items.clone(),
                                total_price: p, discount_percent: d,
                                is_available: true,
                                name_en: if ne.is_empty() { None } else { Some(ne.clone()) },
                                description_en: if de.is_empty() { None } else { Some(de.clone()) },
                            });
                            status.set("✅ Добавлен!".into());
                            name.set(String::new()); description.set(String::new()); icon.set(String::new());
                            image_url.set(String::new()); video_url.set(String::new());
                            items.set(Vec::new()); total_price.set(String::new()); discount_percent.set(String::new());
                            name_en.set(String::new()); description_en.set(String::new());
                            auto_scroll_to_list();
                            spawn(async move {
                                let body = json!({
                                    "name": n, "total_price": p, "discount_percent": d,
                                    "description": if desc.is_empty() { serde_json::Value::Null } else { desc.into() },
                                    "icon": if ic.is_empty() { serde_json::Value::Null } else { ic.into() },
                                    "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                    "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                    "items": tea_items,
                                    "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                    "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                });
                                let url = format!("{}/api/tea-sets", api_base_url());
                                let res = HTTP_CLIENT.clone().post(&url)
                                    .header("X-Telegram-Init-Data", init_data.read().clone())
                                    .header("X-Admin-Token", admin_token())
                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                submitting.set(false);
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        if let Ok(data) = r.json::<serde_json::Value>().await {
                                            if let Some(real_id) = data["id"].as_str() {
                                                if let Some(s) = cache.write().iter_mut().find(|s| s.id == temp_id) { s.id = real_id.to_string(); }
                                            }
                                        }
                                        TelegramApp::init().haptic_notification(HapticNotification::Success);
                                    }
                                    _ => {
                                        cache.write().retain(|s| s.id != temp_id);
                                        status.set("❌ Ошибка добавления".into());
                                        TelegramApp::init().haptic_notification(HapticNotification::Error);
                                    }
                                }
                            });
                        },
                        if *submitting.read() { "⏳..." } else { "➕ Добавить" }
                    }
                    {render_status(status)}
                }
            }
            h3 { style: list_title_style(), "Наборы чая ({filtered.len()})" }
            {render_search(search_query)}
            if *loading.read() {
                div { style: "display:flex;flex-direction:column;gap:8px;",
                    for _ in 0..4 {
                        div { style: "background:#1a1a2e;padding:8px 10px;border-radius:6px;display:flex;align-items:center;gap:6px;",
                            Skeleton { shape: SkeletonShape::Avatar }
                            div { style: "flex:1;display:flex;flex-direction:column;gap:4px;",
                                Skeleton { shape: SkeletonShape::Text, width: Some("60%".into()) }
                                Skeleton { shape: SkeletonShape::TextSm, width: Some("40%".into()) }
                            }
                        }
                    }
                }
            } else if cache.read().is_empty() && search_query.read().is_empty() {
                EmptyState {
                    icon: "📦",
                    title: "Пока ничего нет",
                    description: "Заполните форму выше — кнопка «➕ Добавить» создаст первую запись.",
                }
            } else if filtered.is_empty() {
                EmptyState {
                    icon: "🔍",
                    title: "Ничего не найдено",
                    description: "Попробуйте изменить запрос поиска",
                    action: rsx! {
                        button {
                            style: "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:13px;cursor:pointer;",
                            onclick: move |_| search_query.set(String::new()),
                            "Очистить поиск"
                        }
                    },
                }
            } else {
                div { "data-list": "true", style: "display:flex;flex-direction:column;gap:8px;",
                    for s in filtered {
                        if editing_id.read().as_deref() == Some(s.id.as_str()) {
                            EditTeaSetCard {
                                key: "{s.id}",
                                item: s.clone(),
                                cache: cache,
                                tea_catalog: tea_catalog.read().clone(),
                                on_saved: move |_| editing_id.set(None),
                                on_cancel: move |_| editing_id.set(None),
                            }
                        } else {
                            ItemRow {
                                key: "{s.id}",
                                name: s.name.clone(),
                                sub: format!("{} items • {}฿", s.items.len(), s.total_price),
                                is_available: s.is_available,
                                image_url: s.icon.clone().filter(|i| i.starts_with("http://") || i.starts_with("https://") || (i.starts_with("/") && !i.starts_with("//"))),
                                video_url: s.video_url.clone(),
                                on_edit: { let id = s.id.clone(); move |_| editing_id.set(Some(id.clone())) },
                                on_toggle: {
                                    let id = s.id.clone(); let next = !s.is_available;
                                    move |_| {
                                        let id = id.clone();
                                        if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) { s.is_available = next; }
                                        spawn(async move {
                                            let url = format!("{}/api/tea-sets/{}/availability", api_base_url(), id);
                                            let res = HTTP_CLIENT.clone().put(&url)
                                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                                .header("X-Admin-Token", admin_token())
                                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                .json(&json!({ "is_available": next })).send().await;
                                            match res {
                                                Ok(r) if r.status().is_success() => { TelegramApp::init().haptic_notification(HapticNotification::Success); }
                                                _ => {
                                                    if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) { s.is_available = !next; }
                                                    push_toast(toasts, "Не удалось изменить статус".into(), ToastKind::Error);
                                                    TelegramApp::init().haptic_notification(HapticNotification::Error);
                                                }
                                            }
                                        });
                                    }
                                },
                                on_delete: { let id = s.id.clone(); move |_| delete_target_id.set(Some(id.clone())) }
                            }
                        }
                    }
                }
            }
            DeleteConfirmModal {
                target: delete_target_id,
                item_name: "набор чая".to_string(),
                on_confirm: move |id: String| {
                    let deleted = cache.read().iter().find(|s| s.id == id).cloned();
                    cache.write().retain(|s| s.id != id);
                    spawn(async move {
                        let url = format!("{}/api/tea-sets/{}", api_base_url(), id);
                        let res = HTTP_CLIENT.clone().delete(&url)
                            .header("X-Telegram-Init-Data", init_data.read().clone())
                            .header("X-Admin-Token", admin_token())
                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                            .send().await;
                        match res {
                            Ok(r) if r.status().is_success() => { push_toast(toasts, "Набор чая удалён".into(), ToastKind::Success); TelegramApp::init().haptic_notification(HapticNotification::Success); }
                            _ => {
                                if let Some(item) = deleted { cache.write().push(item); }
                                push_toast(toasts, "Не удалось удалить набор чая".into(), ToastKind::Error);
                                TelegramApp::init().haptic_notification(HapticNotification::Error);
                            }
                        }
                    });
                }
            }
            {render_toasts(toasts)}
        }
    }
}

#[component]
fn EditTeaSetCard(
    item: AdminTeaSet,
    cache: Signal<Vec<AdminTeaSet>>,
    tea_catalog: Vec<(String, String)>,
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut name = use_signal(|| item.name.clone());
    let mut description = use_signal(|| item.description.clone().unwrap_or_default());
    let mut icon = use_signal(|| item.icon.clone().unwrap_or_default());
    let mut image_url = use_signal(|| item.image_url.clone().unwrap_or_default());
    let mut video_url = use_signal(|| item.video_url.clone().unwrap_or_default());
    let items: Signal<Vec<String>> = use_signal(|| item.items.clone());
    let mut total_price = use_signal(|| item.total_price.to_string());
    let mut discount_percent = use_signal(|| item.discount_percent.to_string());
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    let is_available = item.is_available;
    rsx! {
        div { "data-editing": "true", style: edit_card_style(),
            div { style: edit_header_style(), "✏️ Редактирование" }
            input { style: input_style(), placeholder: "Название (RU)", value: "{name}", oninput: move |e| name.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description}", oninput: move |e| description.set(e.value()) }
            input { style: input_style(), placeholder: "Иконка", value: "{icon}", oninput: move |e| icon.set(e.value()) }
            ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
            VideoUpload { video_url: video_url.read().clone(), on_change: move |url: String| video_url.set(url) }
            IdPicker {
                label: "Чай".to_string(),
                options: tea_catalog,
                selected: items,
            }
            input { style: input_style(), placeholder: "Цена ฿", value: "{total_price}", r#type: "number", oninput: move |e| total_price.set(e.value()) }
            input { style: input_style(), placeholder: "Скидка %", value: "{discount_percent}", r#type: "number", oninput: move |e| discount_percent.set(e.value()) }
            div { style: en_section_style(), "🇬🇧 English" }
            input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}", oninput: move |e| name_en.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}", oninput: move |e| description_en.set(e.value()) }
            div { style: "display:flex;gap:8px;",
                button { style: submit_btn_style(),
                    onclick: move |_| {
                        let n = name().trim().to_string();
                        let p = match total_price.read().trim().parse::<f64>() {
                            Ok(v) if v >= 0.0 && v.is_finite() => v,
                            _ => { status.set("❌ Цена должна быть числом ≥ 0".into()); return; }
                        };
                        let d = match discount_percent.read().trim().parse::<f64>() {
                            Ok(v) if v.is_finite() => v,
                            _ => { status.set("❌ Скидка должна быть числом".into()); return; }
                        };
                        if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                        let tea_items: Vec<String> = items.read().clone();
                        let desc = description();
                        let ic = icon(); let img = image_url(); let vid = video_url();
                        let ne = name_en(); let de = description_en();
                        let id = item_id.clone();
                        let original = cache.read().iter().find(|s| s.id == id).cloned();
                        if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) {
                            s.name = n.clone();
                            s.description = if desc.is_empty() { None } else { Some(desc.clone()) };
                            s.icon = if ic.is_empty() { None } else { Some(ic.clone()) };
                            s.image_url = if img.is_empty() { None } else { Some(img.clone()) };
                            s.video_url = if vid.is_empty() { None } else { Some(vid.clone()) };
                            s.items = tea_items.clone();
                            s.total_price = p;
                            s.discount_percent = d;
                            s.is_available = is_available;
                            s.name_en = if ne.is_empty() { None } else { Some(ne.clone()) };
                            s.description_en = if de.is_empty() { None } else { Some(de.clone()) };
                        }
                        spawn(async move {
                            let body = json!({
                                "name": n, "total_price": p, "discount_percent": d,
                                "description": if desc.is_empty() { serde_json::Value::Null } else { desc.into() },
                                "icon": if ic.is_empty() { serde_json::Value::Null } else { ic.into() },
                                "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                "items": tea_items,
                                "is_available": is_available,
                                "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                            });
                            let url = format!("{}/api/tea-sets/{}", api_base_url(), id);
                            let res = HTTP_CLIENT.clone().put(&url)
                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                .header("X-Admin-Token", admin_token())
                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                .json(&body).send().await;
                            let outcome = match res {
                                Ok(r) if r.status().is_success() => Ok(()),
                                Ok(r) => {
                                    let st = r.status().as_u16();
                                    let body = r.text().await.unwrap_or_default();
                                    let b = body.trim();
                                    if b.is_empty() { Err(format!("HTTP {st}")) }
                                    else { Err(format!("HTTP {st}: {}", b.chars().take(80).collect::<String>())) }
                                }
                                Err(_) => Err("сеть/таймаут".to_string()),
                            };
                            match outcome {
                                Ok(()) => {
                                    on_saved.call(());
                                    status.set("✅ Сохранено".into());
                                    TelegramApp::init().haptic_notification(HapticNotification::Success);
                                }
                                Err(reason) => {
                                    TelegramApp::init().haptic_notification(HapticNotification::Error);
                                    if let Some(orig) = original { if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) { *s = orig; } }
                                    status.set(format!("❌ Не сохранено ({reason})"));
                                }
                            }
                        });
                    },
                    "💾 Сохранить"
                }
                button { style: cancel_btn_style(), onclick: move |_| on_cancel.call(()), "Отмена" }
            }
            {render_status(status)}
        }
    }
}

// ── Shared UI helpers ─────────────────────────────────────────
// ── Shared UI helpers ─────────────────────────────────────────

fn auto_scroll_to_list() {
    let _ = js_sys::eval("setTimeout(()=>{var el=document.querySelector('[data-list]');if(el)el.scrollIntoView({behavior:'smooth'});},100);");
}

fn upload_error_style() -> &'static str {
    "margin-top:6px;padding:8px 10px;background:#2a0f15;color:#ff6b7a;border:1px solid #ff4757;border-radius:4px;font-size:12px;line-height:1.4;word-break:break-word;"
}

#[component]
fn ImageUpload(image_url: String, on_change: EventHandler<String>) -> Element {
    let mut uploading = use_signal(|| false);
    let mut error_msg = use_signal(String::new);
    let img_url = image_url.clone();
    rsx! {
        div { style: "display:flex;gap:6px;align-items:center;",
            input { style: "flex:1;{input_style()}", placeholder: "URL картинки", value: "{image_url}",
                oninput: move |e| on_change.call(e.value()) }
            if *uploading.read() {
                div { style: "padding:10px 12px;background:#1a1a2e;color:#6699ff;border:1px dashed #2a2a4a;border-radius:4px;font-size:13px;white-space:nowrap;", "⏳ Загрузка..." }
            } else {
                button { style: upload_btn_style(),
                    onclick: move |_| {
                        uploading.set(true);
                        error_msg.set(String::new());
                        spawn(async move {
                            let result = upload_image().await;
                            uploading.set(false);
                            match result {
                                Ok(Some(url)) => on_change.call(url),
                                Ok(None) => {}
                                Err(msg) => error_msg.set(msg),
                            }
                        });
                    },
                    "📷 Upload"
                }
            }
        }
        if !error_msg.read().is_empty() {
            div { style: upload_error_style(),
                "❌ {error_msg}"
                button {
                    style: "margin-left:8px;background:transparent;color:#ff6b7a;border:1px solid #ff4757;border-radius:3px;font-size:11px;padding:1px 6px;cursor:pointer;",
                    "aria-label": "Закрыть",
                    onclick: move |_| error_msg.set(String::new()),
                    "✕"
                }
            }
        }
        if !image_url.is_empty() && (image_url.starts_with("http://") || image_url.starts_with("https://") || (image_url.starts_with("/") && !image_url.starts_with("//"))) {
            div { style: "margin-top:4px;",
                img { src: "{image_url}", alt: "Превью", style: "width:64px;height:64px;object-fit:cover;border-radius:6px;border:1px solid #2a2a4a;cursor:pointer;", onclick: move |_| {
                    if img_url.starts_with("http://") || img_url.starts_with("https://") || (img_url.starts_with("/") && !img_url.starts_with("//")) {
                        let _ = web_sys::window().and_then(|w| w.open_with_url_and_target(&img_url, "_blank").ok());
                    }
                } }
            }
        }
    }
}

#[component]
fn VideoUpload(video_url: String, on_change: EventHandler<String>) -> Element {
    let mut uploading = use_signal(|| false);
    let mut error_msg = use_signal(String::new);
    rsx! {
        div { style: "display:flex;gap:6px;align-items:center;",
            input { style: "flex:1;{input_style()}", placeholder: "URL видео", value: "{video_url}",
                oninput: move |e| on_change.call(e.value()) }
            if *uploading.read() {
                div { style: "padding:10px 12px;background:#1a1a2e;color:#6699ff;border:1px dashed #2a2a4a;border-radius:4px;font-size:13px;white-space:nowrap;", "⏳ Загрузка..." }
            } else {
                button { style: upload_btn_style(),
                    onclick: move |_| {
                        uploading.set(true);
                        error_msg.set(String::new());
                        spawn(async move {
                            let result = upload_video().await;
                            uploading.set(false);
                            match result {
                                Ok(Some(url)) => on_change.call(url),
                                Ok(None) => {}
                                Err(msg) => error_msg.set(msg),
                            }
                        });
                    },
                    "🎥 Upload"
                }
            }
        }
        if !error_msg.read().is_empty() {
            div { style: upload_error_style(),
                "❌ {error_msg}"
                button {
                    style: "margin-left:8px;background:transparent;color:#ff6b7a;border:1px solid #ff4757;border-radius:3px;font-size:11px;padding:1px 6px;cursor:pointer;",
                    "aria-label": "Закрыть",
                    onclick: move |_| error_msg.set(String::new()),
                    "✕"
                }
            }
        }
        if !video_url.is_empty() {
            div { style: "margin-top:4px;",
                video { src: "{video_url}", controls: true, style: "width:120px;height:80px;object-fit:cover;border-radius:6px;border:1px solid #2a2a4a;" }
            }
        }
    }
}

fn render_search(mut search_query: Signal<String>) -> Element {
    rsx! {
        input {
            style: "width:100%;padding:8px 12px;background:#1a1a2e;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:13px;margin-bottom:8px;",
            placeholder: "🔍 Поиск...",
            value: "{search_query}",
            oninput: move |e: Event<FormData>| search_query.set(e.value())
        }
    }
}

fn render_status(mut status: Signal<String>) -> Element {
    rsx! {
        if !status.read().is_empty() {
            {
                let msg = status.read().clone();
                let color = if msg.starts_with("✅") { "#39ff14" } else if msg.starts_with("❌") { "#ff4757" } else { "#39ff14" };
                rsx! {
                    div {
                        style: "padding:10px;background:{color}10;color:{color};font-size:13px;border-radius:4px;border:1px solid {color}30;display:flex;justify-content:space-between;align-items:center;margin-top:4px;",
                        span { "{msg}" }
                        button { style: "background:none;border:none;color:{color};cursor:pointer;font-size:16px;padding:0 4px;",
                            "aria-label": "Закрыть",
                            onclick: move |_| status.set(String::new()), "✕" }
                    }
                }
            }
        }
    }
}

#[component]
fn DeleteConfirmModal(
    target: Signal<Option<String>>,
    item_name: String,
    on_confirm: EventHandler<String>,
) -> Element {
    rsx! {
        Modal {
            open: target.read().is_some(),
            title: Some("Подтвердите удаление".to_string()),
            show_close: true,
            on_close: move |_| target.set(None),
            div { style: "padding:16px;text-align:center;",
                div { style: "font-size:32px;margin-bottom:8px;", "🗑️" }
                div { style: "color:#e8e8e8;font-size:14px;margin-bottom:16px;",
                    "Этот {item_name} будет удалён навсегда. Продолжить?"
                }
                div { style: "display:flex;gap:8px;justify-content:center;",
                    button { style: "padding:10px 20px;background:#ff4757;color:#fff;border:none;border-radius:4px;font-size:14px;cursor:pointer;font-weight:600;",
                        onclick: move |_| {
                            if let Some(id) = target.read().as_ref() {
                                on_confirm.call(id.clone());
                            }
                            target.set(None);
                        },
                        "Удалить"
                    }
                    button { style: cancel_btn_style(),
                        onclick: move |_| target.set(None),
                        "Отмена"
                    }
                }
            }
        }
    }
}

#[component]
fn FormCard(title: String, children: Element) -> Element {
    rsx! {
        div { style: "background:#1a1a2e;padding:16px;border-radius:8px;margin-bottom:20px;border:1px solid #2a2a4a;",
            h3 { style: "color:#39ff14;font-size:15px;margin-bottom:12px;", "{title}" }
            div { style: "display:flex;flex-direction:column;gap:8px;", {children} }
        }
    }
}

#[component]
fn ItemRow(
    name: String,
    sub: String,
    is_available: bool,
    image_url: Option<String>,
    #[props(default)] video_url: Option<String>,
    on_edit: EventHandler<()>,
    on_toggle: EventHandler<()>,
    on_delete: EventHandler<()>,
    #[props(default)] on_sotd: Option<EventHandler<()>>,
) -> Element {
    let badge = if is_available {
        ("#39ff14", "ВКЛ")
    } else {
        ("#666", "ВЫКЛ")
    };
    let toggle_icon = if is_available { "👁️" } else { "🚫" };
    let toggle_label = if is_available {
        "Скрыть"
    } else {
        "Показать"
    };
    let mut show_video = use_signal(|| false);
    let mut menu_open = use_signal(|| false);
    let thumb = match image_url.as_deref() {
        Some(url)
            if !url.is_empty()
                && (url.starts_with("http://")
                    || url.starts_with("https://")
                    || (url.starts_with("/") && !url.starts_with("//"))) =>
        {
            rsx! { img { src: "{url}", alt: "{name}", style: "width:28px;height:28px;object-fit:cover;border-radius:4px;border:1px solid #2a2a4a;flex-shrink:0;" } }
        }
        _ => {
            rsx! { div { style: "width:28px;height:28px;background:#2a2a4a;border-radius:4px;display:flex;align-items:center;justify-content:center;flex-shrink:0;font-size:14px;", "📦" } }
        }
    };
    rsx! {
        div { style: "background:#1a1a2e;padding:4px 6px;border-radius:6px;display:flex;align-items:center;gap:4px;flex-wrap:nowrap;overflow:hidden;",
            {thumb}
            div { style: "flex:1;min-width:0;overflow:hidden;cursor:pointer;",
                onclick: move |_| on_edit.call(()),
                div { style: "font-weight:600;font-size:12px;color:#e8e8e8;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;", "{name}" }
                div { style: "font-size:10px;color:#888;margin-top:1px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;", "{sub}" }
            }
            span { style: "font-size:9px;padding:1px 4px;background:{badge.0}20;color:{badge.0};border-radius:8px;font-weight:600;flex-shrink:0;", "{badge.1}" }
            button { style: "flex-shrink:0;min-width:44px;min-height:44px;padding:8px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:16px;cursor:pointer;line-height:1;display:flex;align-items:center;justify-content:center;",
                "aria-label": "{toggle_label}",
                onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_toggle.call(()); }, "{toggle_icon}" }
            button { style: "flex-shrink:0;min-width:44px;min-height:44px;padding:8px;background:#1a2a3a;color:#e8e8e8;border:none;border-radius:4px;font-size:18px;cursor:pointer;line-height:1;display:flex;align-items:center;justify-content:center;",
                "aria-label": "Действия",
                onclick: move |e: Event<MouseData>| { e.stop_propagation(); menu_open.set(true); }, "⋮" }
        }
        Modal {
            open: menu_open(),
            title: Some(name.clone()),
            show_close: true,
            on_close: move |_| menu_open.set(false),
            div { style: "display:flex;flex-direction:column;gap:8px;padding:8px 0;",
                button { style: "width:100%;padding:10px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:14px;cursor:pointer;text-align:left;",
                    onclick: move |_| { menu_open.set(false); on_edit.call(()); },
                    "✏️ Редактировать" }
                {if let Some(handler) = on_sotd {
                    rsx! {
                        button { style: "width:100%;padding:10px;background:#2a2a1a;color:#ffe600;border:none;border-radius:4px;font-size:14px;cursor:pointer;text-align:left;",
                            onclick: move |e: Event<MouseData>| { e.stop_propagation(); menu_open.set(false); handler.call(()); },
                            "🌟 Сорт дня" }
                    }
                } else {
                    rsx! {}
                }}
                {if let Some(ref url) = video_url {
                    let url = url.clone();
                    rsx! {
                        button { style: "width:100%;padding:10px;background:#1a2a3a;color:#4fc3f7;border:none;border-radius:4px;font-size:14px;cursor:pointer;text-align:left;",
                            onclick: move |e: Event<MouseData>| { e.stop_propagation(); menu_open.set(false); show_video.set(true); },
                            "▶️ Смотреть видео" }
                        {show_video().then(|| rsx! {
                            VideoModal { url: url.clone(), on_close: move |_| show_video.set(false) }
                        })}
                    }
                } else {
                    rsx! {}
                }}
                button { style: "width:100%;padding:10px;background:#3a1a1a;color:#ff8888;border:none;border-radius:4px;font-size:14px;cursor:pointer;text-align:left;",
                    onclick: move |_| { menu_open.set(false); on_delete.call(()); },
                    "🗑 Удалить" }
            }
        }
    }
}

// ── Edit components (write directly to cache) ─────────────────

#[component]
fn EditStrainCard(
    item: AdminStrain,
    cache: Signal<Vec<AdminStrain>>,
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut name = use_signal(|| item.name.clone());
    let mut category = use_signal(|| {
        item.category
            .clone()
            .unwrap_or_else(|| "hybrid".to_string())
    });
    let mut price = use_signal(|| item.price_per_gram.to_string());
    let mut thc = use_signal(|| item.thc_percent.map(|v| v.to_string()).unwrap_or_default());
    let mut cbd = use_signal(|| item.cbd_percent.map(|v| v.to_string()).unwrap_or_default());
    let mut grams = use_signal(|| item.available_grams.unwrap_or(0.0).to_string());
    let mut description = use_signal(|| item.description.clone().unwrap_or_default());
    let mut effect = use_signal(|| item.effect.clone().unwrap_or_default());
    let mut flavor_profile = use_signal(|| item.flavor_profile.clone().unwrap_or_default());
    let mut image_url = use_signal(|| item.image_url.clone().unwrap_or_default());
    let mut video_url = use_signal(|| item.video_url.clone().unwrap_or_default());
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut effect_en = use_signal(|| item.effect_en.clone().unwrap_or_default());
    let mut flavor_profile_en = use_signal(|| item.flavor_profile_en.clone().unwrap_or_default());
    let mut strain_type_en = use_signal(|| item.strain_type_en.clone().unwrap_or_default());
    // TZ #2 marketing signals — pre-seeded from the row, dispatched via the
    // same `update_strain` body so admin saves once and gets all updates.
    let mut is_sotd = use_signal(|| item.is_strain_of_day);
    let mut sotd_discount = use_signal(|| {
        if item.strain_of_day_discount > 0.0 {
            item.strain_of_day_discount.to_string()
        } else {
            String::new()
        }
    });
    let mut sale_active = use_signal(|| item.sale_active);
    let mut discount_percent = use_signal(|| {
        if item.discount_percent > 0.0 {
            item.discount_percent.to_string()
        } else {
            String::new()
        }
    });
    let mut sale_price = use_signal(|| item.sale_price.map(|v| v.to_string()).unwrap_or_default());
    // sale_until / new_until inputs use HTML <input type="datetime-local">.
    // That widget gives a value like "2026-07-15T18:30" (no timezone, no
    // seconds). We append ":00Z" on submit so the server parses it as RFC3339.
    let mut sale_until = use_signal(|| {
        item.sale_until
            .clone()
            .and_then(|s| s.get(..16).map(str::to_string))
            .unwrap_or_default()
    });
    let mut is_best_seller = use_signal(|| item.is_best_seller);
    let mut is_new_arrival = use_signal(|| item.is_new_arrival);
    let mut new_until = use_signal(|| {
        item.new_until
            .clone()
            .and_then(|s| s.get(..16).map(str::to_string))
            .unwrap_or_default()
    });
    let mut display_order = use_signal(|| item.display_order.to_string());
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    rsx! {
           div { "data-editing": "true", style: edit_card_style(),
               div { style: edit_header_style(), "✏️ Редактирование" }
               input { style: input_style(), placeholder: "Название", value: "{name}", oninput: move |e| name.set(e.value()) }
               select { style: input_style(), value: "{category}", oninput: move |e| category.set(e.value()),
                   option { value: "sativa", "☀️ Sativa" } option { value: "indica", "🌙 Indica" } option { value: "hybrid", "⚖️ Hybrid" } }
               input { style: input_style(), placeholder: "Цена ฿/г", value: "{price}", r#type: "number", oninput: move |e| price.set(e.value()) }
               input { style: input_style(), placeholder: "THC %", value: "{thc}", r#type: "number", oninput: move |e| thc.set(e.value()) }
               input { style: input_style(), placeholder: "CBD %", value: "{cbd}", r#type: "number", oninput: move |e| cbd.set(e.value()) }
               input { style: input_style(), placeholder: "Граммы", value: "{grams}", r#type: "number", oninput: move |e| grams.set(e.value()) }
               textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description}", oninput: move |e| description.set(e.value()) }
               textarea { style: textarea_style(), placeholder: "Эффект (RU)", value: "{effect}", oninput: move |e| effect.set(e.value()) }
               textarea { style: textarea_style(), placeholder: "Вкусовой профиль (RU)", value: "{flavor_profile}", oninput: move |e| flavor_profile.set(e.value()) }
               ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
               VideoUpload { video_url: video_url.read().clone(), on_change: move |url: String| video_url.set(url) }
               div { style: en_section_style(), "🇬🇧 English" }
               input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}", oninput: move |e| name_en.set(e.value()) }
               textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}", oninput: move |e| description_en.set(e.value()) }
               textarea { style: textarea_style(), placeholder: "Effect (EN)", value: "{effect_en}", oninput: move |e| effect_en.set(e.value()) }
               textarea { style: textarea_style(), placeholder: "Flavor (EN)", value: "{flavor_profile_en}", oninput: move |e| flavor_profile_en.set(e.value()) }
               input { style: input_style(), placeholder: "Type (EN)", value: "{strain_type_en}", oninput: move |e| strain_type_en.set(e.value()) }

               // ── Marketing controls (TZ #2) ────────────────────────────
               // All four flags + display order go in one block, submitted
               // by the same Save button below. Date inputs use datetime-local;
               // empty = no expiry. The hero blocks in menu_screen pick these
               // up via priority CASE / `is_active_until`.
               div { style: en_section_style(), "📣 Маркетинг" }
               div { style: "display:flex;flex-direction:column;gap:8px;background:#1a1a2e;padding:10px;border-radius:6px;",
                   // Strain of the Day toggle
                   div { style: "display:flex;gap:8px;align-items:center;flex-wrap:wrap;",
                       button {
                           r#type: "button",
                           style: if *is_sotd.read() {
                               "padding:8px 14px;background:#ffe600;color:#000;border:none;border-radius:6px;font-size:13px;font-weight:700;cursor:pointer;".to_string()
                           } else {
                               "padding:8px 14px;background:#2a2a4a;color:#888;border:none;border-radius:6px;font-size:13px;cursor:pointer;".to_string()
                           },
                           onclick: move |_| { let next = !*is_sotd.read(); is_sotd.set(next); },
                           if *is_sotd.read() { "⭐ STRAIN OF DAY ON" } else { "⭐ STRAIN OF DAY off" }
                       }
                       input {
                           style: "padding:8px 10px;background:#0f0f1a;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:13px;width:90px;",
                           placeholder: "SOTD %",
                           r#type: "number",
                           value: "{sotd_discount}",
                           oninput: move |e| sotd_discount.set(e.value()),
                       }
                   }
                   // Sale toggle + discount % + sale_price + sale_until
                   div { style: "display:flex;gap:8px;align-items:center;flex-wrap:wrap;",
                       button {
                           r#type: "button",
                           style: if *sale_active.read() {
                               "padding:8px 14px;background:#ff4757;color:#fff;border:none;border-radius:6px;font-size:13px;font-weight:700;cursor:pointer;".to_string()
                           } else {
                               "padding:8px 14px;background:#2a2a4a;color:#888;border:none;border-radius:6px;font-size:13px;cursor:pointer;".to_string()
                           },
                           onclick: move |_| { let next = !*sale_active.read(); sale_active.set(next); },
                           if *sale_active.read() { "🔥 SALE ON" } else { "🔥 SALE off" }
                       }
                       input {
                           style: "padding:8px 10px;background:#0f0f1a;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:13px;width:90px;",
                           placeholder: "Скидка %",
                           r#type: "number",
                           value: "{discount_percent}",
                           oninput: move |e| discount_percent.set(e.value()),
                       }
                       input {
                           style: "padding:8px 10px;background:#0f0f1a;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:13px;width:110px;",
                           placeholder: "Цена ฿",
                           r#type: "number",
                           value: "{sale_price}",
                           oninput: move |e| sale_price.set(e.value()),
                       }
                   }
                   div { style: "display:flex;gap:8px;align-items:center;",
                       span { style: "font-size:12px;color:#888;min-width:90px;", "Sale до:" }
                       input {
                           style: "padding:8px 10px;background:#0f0f1a;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:13px;flex:1;",
                           r#type: "datetime-local",
                           value: "{sale_until}",
                           oninput: move |e| sale_until.set(e.value()),
                       }
                   }
                   // Best seller
                   button {
                       r#type: "button",
                       style: if *is_best_seller.read() {
                           "padding:8px 14px;background:#ff9d00;color:#000;border:none;border-radius:6px;font-size:13px;font-weight:700;cursor:pointer;".to_string()
                       } else {
                           "padding:8px 14px;background:#2a2a4a;color:#888;border:none;border-radius:6px;font-size:13px;cursor:pointer;".to_string()
                       },
                       onclick: move |_| { let next = !*is_best_seller.read(); is_best_seller.set(next); },
                       if *is_best_seller.read() { "⭐ BEST SELLER ON" } else { "⭐ BEST SELLER off" }
                   }
                   // New arrival + expiry
                   button {
                       r#type: "button",
                       style: if *is_new_arrival.read() {
                           "padding:8px 14px;background:#00e5ff;color:#000;border:none;border-radius:6px;font-size:13px;font-weight:700;cursor:pointer;".to_string()
                       } else {
                           "padding:8px 14px;background:#2a2a4a;color:#888;border:none;border-radius:6px;font-size:13px;cursor:pointer;".to_string()
                       },
                       onclick: move |_| { let next = !*is_new_arrival.read(); is_new_arrival.set(next); },
                       if *is_new_arrival.read() { "🆕 NEW ARRIVAL ON" } else { "🆕 NEW ARRIVAL off" }
                   }
                   div { style: "display:flex;gap:8px;align-items:center;",
                       span { style: "font-size:12px;color:#888;min-width:90px;", "Новинка до:" }
                       input {
                           style: "padding:8px 10px;background:#0f0f1a;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:13px;flex:1;",
                           r#type: "datetime-local",
                           value: "{new_until}",
                           oninput: move |e| new_until.set(e.value()),
                       }
                   }
                   // Display order — lower number shows first within priority group
                   div { style: "display:flex;gap:8px;align-items:center;",
                       span { style: "font-size:12px;color:#888;min-width:90px;", "Порядок:" }
                       input {
                           style: "padding:8px 10px;background:#0f0f1a;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:13px;width:90px;",
                           r#type: "number",
                           value: "{display_order}",
                           oninput: move |e| display_order.set(e.value()),
                       }
                   }
               }

               div { style: "display:flex;gap:8px;",
                   button { style: submit_btn_style(),
                       onclick: move |_| {
                           let n = name().trim().to_string();
                           let c = category();
                           let p = match price.read().trim().parse::<f64>() {
                               Ok(v) if v > 0.0 && v.is_finite() => v,
                               _ => { status.set("❌ Цена должна быть числом больше 0".into()); return; }
                           };
                           let t = thc.read().trim().parse::<f64>().ok().filter(|v| v.is_finite());
                           let cb = cbd.read().trim().parse::<f64>().ok().filter(|v| v.is_finite());
                           let g = match grams.read().trim().parse::<f64>() {
                               Ok(v) if v >= 0.0 && v.is_finite() => v,
                               _ => { status.set("❌ Граммы должны быть числом ≥ 0".into()); return; }
                           };
                           if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                           let d = description(); let ef = effect(); let fp = flavor_profile(); let img = image_url(); let vid = video_url();
                           let ne = name_en(); let de = description_en(); let ee = effect_en();
                           let fpe = flavor_profile_en(); let ste = strain_type_en();
                           // Marketing values (TZ #2). datetime-local widget returns
                           // "YYYY-MM-DDTHH:MM" without timezone — server expects RFC3339,
                           // so we append ":00Z" only when the field is non-empty.
                           let sotd_b = *is_sotd.read();
                           // Clamp to the server's accepted range (0..=100). An
                           // out-of-range value made `extract_discount` reject the
                           // whole request with 400, so the discount silently never
                           // landed.
                           let sotd_pct = sotd_discount.read().trim().parse::<f64>().ok()
                               .filter(|v| v.is_finite())
                               .map(|v| v.clamp(0.0, 100.0))
                               .unwrap_or(0.0);
                           let sale_b = *sale_active.read();
                           let dp = discount_percent.read().trim().parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0);
                           let sp = sale_price.read().trim().parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0);
                           let su = sale_until().trim().to_string();
                           let su_rfc = if su.is_empty() { None } else { Some(format!("{}:00Z", su)) };
                           let best_b = *is_best_seller.read();
                           let new_b = *is_new_arrival.read();
                           let nu = new_until().trim().to_string();
                           let nu_rfc = if nu.is_empty() { None } else { Some(format!("{}:00Z", nu)) };
                           let ord = display_order.read().trim().parse::<i32>().ok().unwrap_or(0);
                           let id = item_id.clone();
                           let original = cache.read().iter().find(|s| s.id == id).cloned();
                           // Optimistic update in cache
                           if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) {
                               s.name = n.clone(); s.category = Some(c.clone()); s.price_per_gram = p;
                               s.thc_percent = t; s.cbd_percent = cb;
                               s.description = if d.is_empty() { None } else { Some(d.clone()) };
                               s.effect = if ef.is_empty() { None } else { Some(ef.clone()) };
                               s.flavor_profile = if fp.is_empty() { None } else { Some(fp.clone()) };
                               s.available_grams = Some(g);
                               s.image_url = if img.is_empty() { None } else { Some(img.clone()) };
                               s.video_url = if vid.is_empty() { None } else { Some(vid.clone()) };
                               s.name_en = if ne.is_empty() { None } else { Some(ne.clone()) };
                               s.description_en = if de.is_empty() { None } else { Some(de.clone()) };
                               s.effect_en = if ee.is_empty() { None } else { Some(ee.clone()) };
                               s.flavor_profile_en = if fpe.is_empty() { None } else { Some(fpe.clone()) };
                               s.strain_type_en = if ste.is_empty() { None } else { Some(ste.clone()) };
                               s.is_strain_of_day = sotd_b;
                               s.strain_of_day_discount = sotd_pct;
                               s.discount_percent = dp.unwrap_or(0.0);
                               s.sale_price = sp;
                               s.sale_active = sale_b;
                               s.sale_until = su_rfc.clone();
                               s.is_best_seller = best_b;
                               s.is_new_arrival = new_b;
                               s.new_until = nu_rfc.clone();
                               s.display_order = ord;
                           }
                           spawn(async move {
                               let body = json!({
                                   "name": n, "category": c, "price_per_gram": p, "available_grams": g,
                                   "thc_percent": t, "cbd_percent": cb,
                                   "description": if d.is_empty() { serde_json::Value::Null } else { d.into() },
                                   "effect": if ef.is_empty() { serde_json::Value::Null } else { ef.into() },
                                   "flavor_profile": if fp.is_empty() { serde_json::Value::Null } else { fp.into() },
                                   "is_available": item.is_available,
                                   "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                   "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.clone().into() },
                                   "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                   "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                   "effect_en": if ee.is_empty() { serde_json::Value::Null } else { ee.into() },
                                   "flavor_profile_en": if fpe.is_empty() { serde_json::Value::Null } else { fpe.into() },
                                   "strain_type_en": if ste.is_empty() { serde_json::Value::Null } else { ste.into() },
                                   // TZ #2 marketing fields. Send them all every save so the
                                   // server treats this as an authoritative state snapshot.
                                   "discount_percent": dp.unwrap_or(0.0),
                                   "sale_price": sp,
                                   "sale_active": sale_b,
                                   "sale_until": su_rfc.clone(),
                                   "is_best_seller": best_b,
                                   "is_new_arrival": new_b,
                                   "new_until": nu_rfc.clone(),
                                   "display_order": ord,
                               });
                               // SOTD toggle goes through its dedicated endpoint
                               // (preserves the strain_of_day_set_at audit column).
                               //
                               // Previously this was gated on `sotd_b != item.is_strain_of_day`
                               // and fire-and-forget. Two bugs followed:
                               //   * editing ONLY the discount of an already-featured strain
                               //     sent no request at all — the new percent was written to
                               //     the local cache, shown as "✅ Сохранено", and lost on reload;
                               //   * a rejected request (409 `sotd_limit` when 3 strains are
                               //     already featured, 400, 401) was swallowed, so "сорт дня"
                               //     appeared to be set but never was.
                               // Now: always send when the flag is on or is being turned off,
                               // and surface the server's answer.
                               let mut sotd_error: Option<String> = None;
                               if sotd_b || item.is_strain_of_day {
                                   let sotd_url = format!("{}/api/strains/{}/strain-of-day", api_base_url(), id);
                                   let sotd_body = if sotd_b {
                                       json!({ "is_strain_of_day": true, "discount": sotd_pct })
                                   } else {
                                       json!({ "is_strain_of_day": false, "discount": 0 })
                                   };
                                   let init_for_sotd = init_data.read().clone();
                                   let sotd_res = HTTP_CLIENT.clone().put(&sotd_url)
                                       .header("X-Telegram-Init-Data", init_for_sotd)
                                       .header("X-Admin-Token", admin_token())
                                       .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                       .json(&sotd_body).send().await;
                                   sotd_error = match sotd_res {
                                       Ok(r) if r.status().is_success() => None,
                                       Ok(r) => {
                                           let code = r.status().as_u16();
                                           let msg = r.json::<serde_json::Value>().await.ok()
                                               .and_then(|v| v.get("message")
                                                   .or_else(|| v.get("error"))
                                                   .and_then(|m| m.as_str())
                                                   .map(|s| s.to_string()));
                                           Some(msg.unwrap_or_else(|| format!("сорт дня не сохранён ({code})")))
                                       }
                                       Err(_) => Some("сорт дня не сохранён (нет связи)".into()),
                                   };
                               }
                               let url = format!("{}/api/strains/{}", api_base_url(), id);
                               let res = HTTP_CLIENT.clone().put(&url)
                                   .header("X-Telegram-Init-Data", init_data.read().clone())
    .header("X-Admin-Token", admin_token())
    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                   .json(&body).send().await;
                               let _ = &res;
                               let success = match res {
                                   Ok(r) => r.status().is_success(),
                                   Err(_) => false,
                               };
                               if success {
                                   on_saved.call(());
                                   match &sotd_error {
                                       None => {
                                           status.set("✅ Сохранено".into());
                                           TelegramApp::init().haptic_notification(HapticNotification::Success);
                                       }
                                       Some(err) => {
                                           // The strain itself saved, but the SOTD flag/discount
                                           // did not — roll the featured state back in the cache
                                           // so the UI stops showing a value the server rejected.
                                           if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) {
                                               s.is_strain_of_day = item.is_strain_of_day;
                                               s.strain_of_day_discount = item.strain_of_day_discount;
                                           }
                                           status.set(format!("⚠️ Сохранено, но {err}"));
                                           TelegramApp::init().haptic_notification(HapticNotification::Error);
                                       }
                                   }
                               } else {
                                   TelegramApp::init().haptic_notification(HapticNotification::Error);
                                   if let Some(orig) = original {
                                       if let Some(s) = cache.write().iter_mut().find(|s| s.id == id) { *s = orig; }
                                   }
                                   status.set("❌ Не сохранено. Попробуйте снова".into());
                               }
                           });
                       },
                       "💾 Сохранить"
                   }
                   button { style: cancel_btn_style(), onclick: move |_| on_cancel.call(()), "Отмена" }
               }
               {render_status(status)}
           }
       }
}

#[component]
fn EditAccessoryCard(
    item: AdminAccessory,
    cache: Signal<Vec<AdminAccessory>>,
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut name = use_signal(|| item.name.clone());
    let mut category = use_signal(|| item.category.clone().unwrap_or_else(|| "other".to_string()));
    let mut price = use_signal(|| item.price.to_string());
    let mut stock = use_signal(|| item.stock.map(|s| s.to_string()).unwrap_or_default());
    let mut description = use_signal(|| item.description.clone().unwrap_or_default());
    let mut image_url = use_signal(|| item.image_url.clone().unwrap_or_default());
    let mut video_url = use_signal(|| item.video_url.clone().unwrap_or_default());
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut category_en = use_signal(|| item.category_en.clone().unwrap_or_default());
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    rsx! {
           div { "data-editing": "true", style: edit_card_style(),
               div { style: edit_header_style(), "✏️ Редактирование" }
               input { style: input_style(), placeholder: "Название", value: "{name}", oninput: move |e| name.set(e.value()) }
               select { style: input_style(), value: "{category}", oninput: move |e| category.set(e.value()),
                   option { value: "grinder", "🌀 Grinder" } option { value: "papers", "📄 Papers" }
                   option { value: "lighter", "🔥 Lighter" } option { value: "pipe", "🚬 Pipe" }
                   option { value: "bong", "💨 Bong" } option { value: "storage", "📦 Storage" }
                   option { value: "clothing", "👕 Clothing" } option { value: "other", "🔧 Other" } }
               input { style: input_style(), placeholder: "Цена ฿", value: "{price}", r#type: "number", oninput: move |e| price.set(e.value()) }
               input { style: input_style(), placeholder: "Кол-во", value: "{stock}", r#type: "number", oninput: move |e| stock.set(e.value()) }
               textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description}", oninput: move |e| description.set(e.value()) }
               ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
               VideoUpload { video_url: video_url.read().clone(), on_change: move |url: String| video_url.set(url) }
               div { style: en_section_style(), "🇬🇧 English" }
               input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}", oninput: move |e| name_en.set(e.value()) }
               textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}", oninput: move |e| description_en.set(e.value()) }
               input { style: input_style(), placeholder: "Category (EN)", value: "{category_en}", oninput: move |e| category_en.set(e.value()) }
               div { style: "display:flex;gap:8px;",
                   button { style: submit_btn_style(),
                       onclick: move |_| {
                           let n = name().trim().to_string();
                           let c = category();
                           let p = match price.read().trim().parse::<f64>() {
                               Ok(v) if v > 0.0 && v.is_finite() => v,
                               _ => { status.set("❌ Цена должна быть числом больше 0".into()); return; }
                           };
                           let s_str = stock();
                           let s_val: i32 = match s_str.trim().parse() {
                               Ok(v) if v >= 0 => v,
                               _ => { status.set("❌ Количество должно быть числом ≥ 0".into()); return; }
                           };
                           if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                           let d = description();
                           // Normalize free-text URLs so a scheme-less paste
                           // (e.g. "bucket-…/x.mov") doesn't 400 server-side.
                           let img = crate::trios::validation::normalize_media_url(&image_url());
                           let vid = crate::trios::validation::normalize_media_url(&video_url());
                           let ne = name_en(); let de = description_en(); let ce = category_en();
                           let id = item_id.clone();
                           let original = cache.read().iter().find(|a| a.id == id).cloned();
                           if let Some(a) = cache.write().iter_mut().find(|a| a.id == id) {
                               a.name = n.clone(); a.category = Some(c.clone()); a.price = p; a.stock = Some(s_val);
                               a.description = if d.is_empty() { None } else { Some(d.clone()) };
                               a.image_url = if img.is_empty() { None } else { Some(img.clone()) };
                               a.video_url = if vid.is_empty() { None } else { Some(vid.clone()) };
                               a.name_en = if ne.is_empty() { None } else { Some(ne.clone()) };
                               a.description_en = if de.is_empty() { None } else { Some(de.clone()) };
                               a.category_en = if ce.is_empty() { None } else { Some(ce.clone()) };
                           }
                           spawn(async move {
                               let body = json!({
                                   "name": n, "category": c, "price": p, "stock": s_val, "is_available": item.is_available,
                                   "description": if d.is_empty() { serde_json::Value::Null } else { d.into() },
                                   "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                   "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                   "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                   "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                   "category_en": if ce.is_empty() { serde_json::Value::Null } else { ce.into() },
                               });
                               let url = format!("{}/api/accessories/{}", api_base_url(), id);
                               let res = HTTP_CLIENT.clone().put(&url)
                                   .header("X-Telegram-Init-Data", init_data.read().clone())
    .header("X-Admin-Token", admin_token())
    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                   .json(&body).send().await;
                               // Surface the real reason so a save failure is
                               // diagnosable (HTTP status vs network) instead of an
                               // opaque "не сохранено".
                               let outcome = match res {
                                   Ok(r) if r.status().is_success() => Ok(()),
                                   Ok(r) => {
                                       // Surface the server's reason body (e.g. which field
                                       // validation rejected) so a 400 is self-diagnosable
                                       // from the client without server-log access.
                                       let st = r.status().as_u16();
                                       let body = r.text().await.unwrap_or_default();
                                       let b = body.trim();
                                       if b.is_empty() {
                                           Err(format!("HTTP {st}"))
                                       } else {
                                           let snip: String = b.chars().take(100).collect();
                                           Err(format!("HTTP {st}: {snip}"))
                                       }
                                   }
                                   Err(_) => Err("сеть/таймаут".to_string()),
                               };
                               match outcome {
                                   Ok(()) => {
                                       on_saved.call(());
                                       status.set("✅ Сохранено".into());
                                       TelegramApp::init().haptic_notification(HapticNotification::Success);
                                   }
                                   Err(reason) => {
                                       TelegramApp::init().haptic_notification(HapticNotification::Error);
                                       if let Some(orig) = original {
                                           if let Some(a) = cache.write().iter_mut().find(|a| a.id == id) { *a = orig; }
                                       }
                                       status.set(format!("❌ Не сохранено ({reason}). Попробуйте снова"));
                                   }
                               }
                           });
                       },
                       "💾 Сохранить"
                   }
                   button { style: cancel_btn_style(), onclick: move |_| on_cancel.call(()), "Отмена" }
               }
               {render_status(status)}
           }
       }
}

#[component]
fn EditTeaCard(
    item: AdminTea,
    cache: Signal<Vec<AdminTea>>,
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut name = use_signal(|| item.name.clone());
    // Preselect the item's current category by its canonical key.
    let mut subcategory = use_signal(|| {
        crate::trios::drink_categories::canonical_key(
            item.subcategory.as_deref().unwrap_or(""),
            item.subcategory_en.as_deref(),
        )
    });
    let mut new_cat_ru = use_signal(String::new);
    let mut new_cat_en = use_signal(String::new);
    let mut price = use_signal(|| item.price.to_string());
    let mut stock = use_signal(|| item.stock.map(|s| s.to_string()).unwrap_or_default());
    let mut description = use_signal(|| item.description.clone().unwrap_or_default());
    let mut image_url = use_signal(|| item.image_url.clone().unwrap_or_default());
    let mut video_url = use_signal(|| item.video_url.clone().unwrap_or_default());
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    rsx! {
           div { "data-editing": "true", style: edit_card_style(),
               div { style: edit_header_style(), "✏️ Редактирование" }
               input { style: input_style(), placeholder: "Название", value: "{name}", oninput: move |e| name.set(e.value()) }
               {
                   let cats = crate::trios::drink_categories::admin_categories(
                       cache.read().iter().map(|t| (
                           t.subcategory.as_deref().unwrap_or(""),
                           t.subcategory_en.as_deref(),
                       )),
                   );
                   rsx! {
                       select { style: input_style(), value: "{subcategory}", oninput: move |e| subcategory.set(e.value()),
                           for cat in cats.iter() {
                               option { value: "{cat.key}",
                                   "{crate::trios::drink_categories::emoji(&cat.key)} {cat.ru} / {cat.en}" }
                           }
                           option { value: "__new__", "➕ Новая категория" }
                       }
                   }
               }
               if subcategory() == "__new__" {
                   input { style: input_style(), placeholder: "Категория (RU)", value: "{new_cat_ru}", oninput: move |e| new_cat_ru.set(e.value()) }
                   input { style: input_style(), placeholder: "Category (EN)", value: "{new_cat_en}", oninput: move |e| new_cat_en.set(e.value()) }
               }
               input { style: input_style(), placeholder: "Цена ฿", value: "{price}", r#type: "number", oninput: move |e| price.set(e.value()) }
               input { style: input_style(), placeholder: "Кол-во", value: "{stock}", r#type: "number", oninput: move |e| stock.set(e.value()) }
               textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description}", oninput: move |e| description.set(e.value()) }
               ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
               VideoUpload { video_url: video_url.read().clone(), on_change: move |url: String| video_url.set(url) }
               div { style: en_section_style(), "🇬🇧 English" }
               input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}", oninput: move |e| name_en.set(e.value()) }
               textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}", oninput: move |e| description_en.set(e.value()) }
               div { style: "display:flex;gap:8px;",
                   button { style: submit_btn_style(),
                       onclick: move |_| {
                           let n = name().trim().to_string();
                           if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                           // Resolve the chosen category into bilingual labels.
                           let key = subcategory();
                           let (sc, sce) = if key == "__new__" {
                               let ru = new_cat_ru().trim().to_string();
                               let en = new_cat_en().trim().to_string();
                               if ru.is_empty() { status.set("❌ Введите название категории (RU)".into()); return; }
                               (ru.clone(), if en.is_empty() { ru } else { en })
                           } else {
                               let cats = crate::trios::drink_categories::admin_categories(
                                   cache.read().iter().map(|t| (
                                       t.subcategory.as_deref().unwrap_or(""),
                                       t.subcategory_en.as_deref(),
                                   )),
                               );
                               match cats.iter().find(|c| c.key == key) {
                                   Some(c) => (c.ru.clone(), c.en.clone()),
                                   None => (key.clone(), key.clone()),
                               }
                           };
                           let p = match price.read().trim().parse::<f64>() {
                               Ok(v) if v > 0.0 && v.is_finite() => v,
                               _ => { status.set("❌ Цена должна быть числом больше 0".into()); return; }
                           };
                           let s_str = stock();
                           let s_val: i32 = match s_str.trim().parse() {
                               Ok(v) if v >= 0 => v,
                               _ => { status.set("❌ Количество должно быть числом ≥ 0".into()); return; }
                           };
                           let d = description(); let img = image_url(); let vid = video_url();
                           let ne = name_en(); let de = description_en();
                           let id = item_id.clone();
                           let original = cache.read().iter().find(|t| t.id == id).cloned();
                           if let Some(t) = cache.write().iter_mut().find(|t| t.id == id) {
                               t.name = n.clone(); t.subcategory = Some(sc.clone()); t.price = p; t.stock = Some(s_val);
                               t.description = if d.is_empty() { None } else { Some(d.clone()) };
                               t.image_url = if img.is_empty() { None } else { Some(img.clone()) };
                               t.video_url = if vid.is_empty() { None } else { Some(vid.clone()) };
                               t.name_en = if ne.is_empty() { None } else { Some(ne.clone()) };
                               t.description_en = if de.is_empty() { None } else { Some(de.clone()) };
                               t.subcategory_en = if sce.is_empty() { None } else { Some(sce.clone()) };
                           }
                           spawn(async move {
                               let body = json!({
                                   "name": n, "subcategory": sc, "price": p, "stock": s_val, "is_available": item.is_available,
                                   "description": if d.is_empty() { serde_json::Value::Null } else { d.into() },
                                   "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                   "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                   "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                   "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                   "subcategory_en": if sce.is_empty() { serde_json::Value::Null } else { sce.into() },
                               });
                               let url = format!("{}/api/tea-products/{}", api_base_url(), id);
                               let res = HTTP_CLIENT.clone().put(&url)
                                   .header("X-Telegram-Init-Data", init_data.read().clone())
    .header("X-Admin-Token", admin_token())
    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                   .json(&body).send().await;
                               let success = match res {
                                   Ok(r) => r.status().is_success(),
                                   Err(_) => false,
                               };
                               if success {
                                   on_saved.call(());
                                   status.set("✅ Сохранено".into());
                                   TelegramApp::init().haptic_notification(HapticNotification::Success);
                               } else {
                                   TelegramApp::init().haptic_notification(HapticNotification::Error);
                                   if let Some(orig) = original {
                                       if let Some(t) = cache.write().iter_mut().find(|t| t.id == id) { *t = orig; }
                                   }
                                   status.set("❌ Не сохранено. Попробуйте снова".into());
                               }
                           });
                       },
                       "💾 Сохранить"
                   }
                   button { style: cancel_btn_style(), onclick: move |_| on_cancel.call(()), "Отмена" }
               }
               {render_status(status)}
           }
       }
}

// ══════════════════════════════════════════════════════════════
// ── 7 NEW TABS ────────────────────────────────────────────────
// ══════════════════════════════════════════════════════════════

// ─── Dashboard ───────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct AdminStats {
    #[serde(default)]
    total_orders: i64,
    #[serde(default)]
    total_revenue: Option<f64>,
    #[serde(default)]
    top_strains: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    active_strains: Option<i64>,
}

#[component]
fn DashboardTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut stats: Signal<Option<AdminStats>> = use_signal(|| None);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(String::new);

    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/admin/stats", api_base_url());
            match HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data)
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                Ok(resp) => {
                    if resp.status().is_success() {
                        match resp.json::<AdminStats>().await {
                            Ok(data) => {
                                stats.set(Some(data));
                            }
                            Err(e) => {
                                error.set(format!("Ошибка разбора: {e}"));
                            }
                        }
                    } else {
                        error.set(format!("HTTP {}", resp.status().as_u16()));
                    }
                }
                Err(e) => {
                    error.set(format!("Сеть: {e}"));
                }
            }
            loading.set(false);
            Some(())
        }
    });

    let stat_card = |label: &str, value: String, color_cls: &str| -> Element {
        let (label, color_cls) = (label.to_string(), color_cls.to_string());
        rsx! {
            div { class: "admin-stat-card {color_cls}",
                div { class: "value", "{value}" }
                div { class: "label", "{label}" }
            }
        }
    };

    rsx! {
        div {
            h3 { class: "admin-card-title", "📊 Статистика" }
            if *loading.read() {
                div { class: "admin-stats-grid",
                    for _ in 0..3 {
                        div { class: "skeleton-stat-card",
                            div { class: "skeleton-loader skeleton-label" }
                            div { class: "skeleton-loader skeleton-value" }
                        }
                    }
                }
            } else if !error.read().is_empty() {
                div { class: "admin-badge danger", "Ошибка: {error}" }
            } else if let Some(s) = stats.read().clone() {
                div { class: "admin-stats-grid",
                    {stat_card("Всего заказов", s.total_orders.to_string(), "cyan")}
                    {stat_card("Выручка (Бат)", s.total_revenue.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".to_string()), "")}
                    {stat_card("Активных страйнов", s.active_strains.unwrap_or(0).to_string(), "yellow")}
                }
                if let Some(top) = s.top_strains.clone() {
                    if !top.is_empty() {
                        div { class: "admin-card",
                            h4 { class: "admin-card-meta", "🌿 Топ страйны" }
                            for item in top {
                                div { class: "admin-row",
                                    div { class: "admin-row-main",
                                        "{item[\"name\"].as_str().unwrap_or(\"-\")}"
                                    }
                                    div { class: "admin-badge success",
                                        "{item[\"count\"].as_i64().unwrap_or(0)}"
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                div { class: "admin-empty", "Данных нет" }
            }
        }
    }
}

// ─── Orders ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdminOrder {
    id: String,
    #[serde(default)]
    telegram_id: Option<i64>,
    #[serde(default)]
    customer_name: Option<String>,
    #[serde(default)]
    customer_phone: Option<String>,
    #[serde(default)]
    customer_telegram: Option<String>,
    #[serde(default)]
    items: Vec<AdminOrderItem>,
    #[serde(default)]
    subtotal: f64,
    #[serde(default)]
    bonus_used: f64,
    #[serde(default)]
    stars_used: i64,
    total: f64,
    status: String,
    #[serde(default)]
    shop_id: Option<String>,
    #[serde(rename = "created_at", default)]
    created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdminOrderItem {
    #[serde(default)]
    strain_id: Option<String>,
    #[serde(default)]
    strain_name: Option<String>,
    #[serde(default)]
    accessory_id: Option<String>,
    #[serde(default)]
    accessory_name: Option<String>,
    #[serde(default)]
    tea_id: Option<String>,
    #[serde(default)]
    tea_name: Option<String>,
    #[serde(default)]
    set_id: Option<String>,
    #[serde(default)]
    set_name: Option<String>,
    #[serde(default)]
    quantity: f64,
    /// A3: per-drink dine-in/takeaway.
    #[serde(default)]
    fulfillment: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrdersResp {
    orders: Vec<AdminOrder>,
}

/// Full delivery pipeline statuses. `completed` is kept as a legacy alias
/// for `delivered`.
const ORDER_PIPELINE: &[&str] = &[
    "pending",
    "confirmed",
    "preparing",
    "ready",
    "out_for_delivery",
    "delivered",
];

fn admin_status_label(status: &str) -> &str {
    match status {
        "pending" => "⏳ Ожидает",
        "confirmed" => "✓ Подтверждён",
        "preparing" => "🔥 Готовится",
        "ready" => "📦 Готов",
        "out_for_delivery" => "🚗 В доставке",
        "delivered" | "completed" => "✅ Выполнен",
        "cancelled" | "rejected" => "✖ Отменён",
        _ => status,
    }
}

fn admin_status_badge_class(status: &str) -> &'static str {
    match status {
        "pending" => "admin-badge warn",
        "confirmed" => "admin-badge info",
        "preparing" => "admin-badge info",
        "ready" => "admin-badge info",
        "out_for_delivery" => "admin-badge info",
        "delivered" | "completed" => "admin-badge success",
        "cancelled" | "rejected" => "admin-badge danger",
        _ => "admin-badge muted",
    }
}

/// Next status in the delivery pipeline, if the order is not terminal.
fn next_pipeline_status(status: &str) -> Option<&'static str> {
    let pos = ORDER_PIPELINE.iter().position(|&s| s == status);
    pos.and_then(|i| ORDER_PIPELINE.get(i + 1).copied())
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "delivered" | "completed" | "rejected" | "cancelled")
}

#[component]
fn OrderDetailModal(order: AdminOrder, on_close: EventHandler<()>) -> Element {
    let status_label = admin_status_label(&order.status);
    let status_cls = admin_status_badge_class(&order.status);
    let suffix: String = order
        .id
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let order_title = format!(
        "Заказ #{}...{}",
        &order.id.get(0..4).unwrap_or(&order.id),
        suffix
    );
    rsx! {
        div { style: "position:fixed;inset:0;background:rgba(0,0,0,0.8);display:flex;align-items:center;justify-content:center;z-index:2000;padding:16px;",
            onclick: move |_| on_close.call(()),
            div { style: "background:#1a1a2e;border:2px solid #2a2a4a;border-radius:8px;max-width:480px;width:100%;max-height:90vh;overflow-y:auto;padding:20px;display:flex;flex-direction:column;gap:12px;",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),
                div { style: "display:flex;justify-content:space-between;align-items:center;",
                    h3 { style: "margin:0;color:#39ff14;font-size:17px;", "{order_title}" }
                    button { style: "background:none;border:none;color:#888;font-size:20px;cursor:pointer;", "aria-label": "Закрыть", onclick: move |_| on_close.call(()), "✕" }
                }
                div { style: "display:flex;align-items:center;gap:8px;",
                    span { class: "{status_cls}", "{status_label}" }
                    if let Some(ref dt) = order.created_at {
                        span { style: "font-size:12px;color:#666;", "{dt}" }
                    }
                }
                if let Some(ref name) = order.customer_name {
                    div { style: "display:flex;flex-direction:column;gap:4px;",
                        div { style: "font-size:14px;font-weight:600;color:#e8e8e8;", "👤 {name}" }
                        if let Some(ref phone) = order.customer_phone {
                            div { style: "font-size:13px;color:#888;", "📞 {phone}" }
                        }
                        if let Some(ref tg) = order.customer_telegram {
                            div { style: "font-size:13px;color:#888;", "📱 @{tg}" }
                        }
                    }
                }
                div { style: "border-top:1px solid #2a2a4a;padding-top:12px;display:flex;flex-direction:column;gap:8px;",
                    h4 { style: "margin:0;color:#00e5ff;font-size:14px;", "📋 Товары" }
                    for item in order.items.clone() {
                        div { style: "display:flex;justify-content:space-between;align-items:center;background:#0f0f1a;padding:8px 10px;border-radius:6px;",
                            span { style: "font-size:13px;color:#e8e8e8;",
                                {
                                    let name = item.strain_name.clone()
                                        .or(item.accessory_name.clone())
                                        .or(item.tea_name.clone())
                                        .or(item.set_name.clone())
                                        .unwrap_or_else(|| "Неизвестно".into());
                                    let f = match item.fulfillment.as_deref() {
                                        Some("dine_in") => " 🍽 на месте",
                                        Some("takeaway") => " 🥡 с собой",
                                        _ => "",
                                    };
                                    format!("{} × {:.0}{}", name, item.quantity, f)
                                }
                            }
                        }
                    }
                }
                div { style: "border-top:1px solid #2a2a4a;padding-top:12px;display:flex;flex-direction:column;gap:4px;",
                    div { style: "display:flex;justify-content:space-between;font-size:13px;color:#888;",
                        span { "Подытог" }
                        span { "{order.subtotal:.0} Бат" }
                    }
                    if order.bonus_used > 0.0 {
                        div { style: "display:flex;justify-content:space-between;font-size:13px;color:#ffe600;",
                            span { "Бонусы" }
                            span { "-{order.bonus_used:.0} Бат" }
                        }
                    }
                    if order.stars_used > 0 {
                        div { style: "display:flex;justify-content:space-between;font-size:13px;color:#7dd3fc;",
                            span { "⭐ Stars" }
                            span { "-{order.stars_used} ฿" }
                        }
                    }
                    div { style: "display:flex;justify-content:space-between;font-size:16px;font-weight:700;color:#39ff14;",
                        span { "Итого" }
                        span { "{order.total:.0} Бат" }
                    }
                }
                if let Some(ref shop) = order.shop_id {
                    div { style: "font-size:12px;color:#666;", "🏪 Магазин: {shop}" }
                }
            }
        }
    }
}

#[component]
fn OrdersTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut orders: Signal<Vec<AdminOrder>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut filter = use_signal(|| "all".to_string());
    let mut search = use_signal(String::new);
    let mut updating_id: Signal<Option<String>> = use_signal(|| None);
    let mut error = use_signal(String::new);
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);
    let mut selected_order: Signal<Option<AdminOrder>> = use_signal(|| None);
    let mut offset = use_signal(|| 0i64);
    let mut limit = use_signal(|| 20i64);
    let reload = use_signal(|| 0u32);
    let mut auto_refresh = use_signal(|| true);
    let mut tick = use_signal(|| 0u32);
    let mut prev_pending_count = use_signal(|| 0usize);

    let _poll = use_resource(move || {
        let enabled = auto_refresh();
        async move {
            if !enabled {
                return Some(());
            }
            loop {
                gloo_timers::future::TimeoutFuture::new(10000).await;
                tick.set(tick() + 1);
            }
        }
    });

    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        let off = *offset.read();
        let lim = *limit.read();
        let _r = *reload.read();
        let _t = *tick.read();
        let toasts2 = toasts;
        async move {
            loading.set(true);
            let url = format!("{}/api/orders?limit={}&offset={}", api_base_url(), lim, off);
            match HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data)
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(data) = resp.json::<OrdersResp>().await {
                        let new_pending =
                            data.orders.iter().filter(|o| o.status == "pending").count();
                        let old_pending = *prev_pending_count.read();
                        if new_pending > old_pending && old_pending > 0 {
                            let diff = new_pending - old_pending;
                            push_toast(
                                toasts2,
                                format!("🛎️ {} новых заказов!", diff),
                                ToastKind::Info,
                            );
                        }
                        prev_pending_count.set(new_pending);
                        orders.set(data.orders);
                    }
                }
                Ok(resp) => {
                    error.set(format!("HTTP {}", resp.status().as_u16()));
                }
                Err(e) => {
                    error.set(format!("Сеть: {e}"));
                }
            }
            loading.set(false);
            Some(())
        }
    });

    let filtered: Vec<AdminOrder> = {
        let f = filter.read().clone();
        let q = search.read().to_lowercase();
        orders
            .read()
            .iter()
            .filter(|o| {
                let status_ok = f == "all" || o.status == f;
                let search_ok = q.is_empty()
                    || o.id.to_lowercase().contains(&q)
                    || o.customer_name
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q)
                    || o.customer_telegram
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q);
                status_ok && search_ok
            })
            .cloned()
            .collect()
    };

    let count_by = |s: &str| -> usize {
        let s = s.to_string();
        orders.read().iter().filter(|o| o.status == s).count()
    };

    let export_csv = move || {
        let mut csv =
            "ID,Дата,Статус,Клиент,Телефон,Telegram,Товары,Подытог,Бонусы,Итого\n".to_string();
        for o in orders.read().iter() {
            let items_str = o
                .items
                .iter()
                .map(|i| {
                    i.strain_name
                        .clone()
                        .or(i.accessory_name.clone())
                        .or(i.tea_name.clone())
                        .or(i.set_name.clone())
                        .unwrap_or_else(|| "Неизвестно".into())
                })
                .collect::<Vec<_>>()
                .join("; ");
            let status_ru = admin_status_label(&o.status);
            csv.push_str(&format!(
                "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",{},{},{},{}\n",
                o.id.replace('"', "\"\""),
                o.created_at.as_deref().unwrap_or(""),
                status_ru,
                o.customer_name
                    .as_deref()
                    .unwrap_or("")
                    .replace('"', "\"\""),
                o.customer_phone.as_deref().unwrap_or(""),
                o.customer_telegram.as_deref().unwrap_or(""),
                items_str.replace('"', "\"\""),
                o.subtotal,
                o.bonus_used,
                o.stars_used,
                o.total,
            ));
        }
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                let arr = js_sys::Array::new();
                arr.push(&js_sys::JsString::from(csv).into());
                if let Ok(blob) = web_sys::Blob::new_with_str_sequence(&arr) {
                    if let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) {
                        if let Some(document) = window.document() {
                            if let Ok(a) = document.create_element("a") {
                                let _ = a.set_attribute("href", &url);
                                let _ = a.set_attribute("download", "orders.csv");
                                let _ = a.dyn_into::<web_sys::HtmlElement>().map(|el| el.click());
                            }
                        }
                        let _ = web_sys::Url::revoke_object_url(&url);
                    }
                }
            }
        }
    };

    rsx! {
        div {
            {render_toasts(toasts)}
            div { style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:12px;",
                h3 { class: "admin-card-title", style: "margin:0;", "📦 Заказы" }
                div { style: "display:flex;align-items:center;gap:8px;",
                    button {
                        style: "padding:6px 12px;background:#1a3a1a;color:#39ff14;border:1px solid #2a5a2a;border-radius:4px;font-size:12px;cursor:pointer;",
                        onclick: move |_| export_csv(),
                        "📥 CSV"
                    }
                    label { style: "display:flex;align-items:center;gap:6px;cursor:pointer;font-size:12px;color:#888;",
                        input { r#type: "checkbox", checked: auto_refresh(),
                            onchange: move |e| auto_refresh.set(e.checked()) }
                        "🔄 Авто"
                    }
                }
            }
            if !error.read().is_empty() {
                div { class: "admin-badge danger", "{error}" }
            }
            // Filter bar
            div { class: "admin-filter-bar", style: "flex-wrap: wrap;",
                button {
                    class: if *filter.read() == "all" { "admin-btn primary admin-btn-sm" } else { "admin-btn secondary admin-btn-sm" },
                    onclick: move |_| filter.set("all".into()), "Все ({orders.read().len()})" }
                button {
                    class: if *filter.read() == "pending" { "admin-btn primary admin-btn-sm" } else { "admin-btn secondary admin-btn-sm" },
                    onclick: move |_| filter.set("pending".into()), "⏳ {count_by(\"pending\")}" }
                button {
                    class: if *filter.read() == "confirmed" { "admin-btn primary admin-btn-sm" } else { "admin-btn secondary admin-btn-sm" },
                    onclick: move |_| filter.set("confirmed".into()), "✓ {count_by(\"confirmed\")}" }
                button {
                    class: if *filter.read() == "preparing" { "admin-btn primary admin-btn-sm" } else { "admin-btn secondary admin-btn-sm" },
                    onclick: move |_| filter.set("preparing".into()), "🔥 {count_by(\"preparing\")}" }
                button {
                    class: if *filter.read() == "ready" { "admin-btn primary admin-btn-sm" } else { "admin-btn secondary admin-btn-sm" },
                    onclick: move |_| filter.set("ready".into()), "📦 {count_by(\"ready\")}" }
                button {
                    class: if *filter.read() == "out_for_delivery" { "admin-btn primary admin-btn-sm" } else { "admin-btn secondary admin-btn-sm" },
                    onclick: move |_| filter.set("out_for_delivery".into()), "🚗 {count_by(\"out_for_delivery\")}" }
                button {
                    class: if *filter.read() == "delivered" { "admin-btn primary admin-btn-sm" } else { "admin-btn secondary admin-btn-sm" },
                    onclick: move |_| filter.set("delivered".into()), "✅ {count_by(\"delivered\")}" }
                button {
                    class: if *filter.read() == "rejected" { "admin-btn primary admin-btn-sm" } else { "admin-btn secondary admin-btn-sm" },
                    onclick: move |_| filter.set("rejected".into()), "✖ {count_by(\"rejected\")}" }
            }
            input {
                class: "admin-input",
                placeholder: "🔍 Поиск по ID, имени, telegram...",
                value: "{search}",
                oninput: move |e| search.set(e.value())
            }
            if *loading.read() {
                div { style: "display:flex;flex-direction:column;gap:8px;",
                    for _ in 0..4 {
                        div { style: "background:#1a1a2e;padding:8px 10px;border-radius:6px;display:flex;align-items:center;gap:6px;",
                            Skeleton { shape: SkeletonShape::Avatar }
                            div { style: "flex:1;display:flex;flex-direction:column;gap:4px;",
                                Skeleton { shape: SkeletonShape::Text, width: Some("60%".into()) }
                                Skeleton { shape: SkeletonShape::TextSm, width: Some("40%".into()) }
                            }
                        }
                    }
                }
            } else if filtered.is_empty() {
                EmptyState {
                    icon: "📦",
                    title: if *filter.read() == "all" { "Заказов нет".to_string() } else { "Нет заказов с таким статусом".to_string() },
                    description: "Заказы будут отображаться здесь",
                }
            } else {
                div {
                    for order in filtered.iter().cloned() {
                        {
                            let order_id = order.id.clone();
                            let order_id3 = order.id.clone();
                            let order_id_href = order.id.clone();
                            let status = order.status.clone();
                            let status3 = order.status.clone();
                            let init_data2 = init_data;
                            let status_label_str = admin_status_label(&order.status).to_string();
                            let status_badge_cls = admin_status_badge_class(&order.status);
                            let short_id: String = order.id.chars().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect();
                            rsx! {
                                div { class: "admin-card", style: "cursor:pointer;",
                                    onclick: {
                                        let o = order.clone();
                                        move |_| selected_order.set(Some(o.clone()))
                                    },
                                    div { class: "admin-row",
                                        div { class: "admin-row-main",
                                            div { class: "admin-card-title", "#{short_id}" }
                                            if let Some(name) = order.customer_name.clone() {
                                                div { class: "admin-card-meta", "{name}"
                                                    if let Some(tg) = order.customer_telegram.clone() {
                                                        span { class: "admin-badge info", " @{tg}" }
                                                    }
                                                }
                                            }
                                        }
                                        span { class: "{status_badge_cls}",
                                            "{status_label_str}"
                                        }
                                    }
                                    div { class: "admin-badge success",
                                        "{order.total:.0}Б"
                                        if order.bonus_used > 0.0 {
                                            span { class: "admin-badge warn",
                                                "-{order.bonus_used:.0}Б бонусов"
                                            }
                                        }
                                        if order.stars_used > 0 {
                                            span { class: "admin-badge info",
                                                "-{order.stars_used}⭐"
                                            }
                                        }
                                    }
                                    div { class: "admin-row-actions",
                                        if let Some(next) = next_pipeline_status(&status) {
                                            button {
                                                class: "admin-btn secondary admin-btn-sm",
                                                disabled: updating_id.read().as_deref() == Some(&order_id),
                                                onclick: move |_| {
                                                    let oid = order_id.clone();
                                                    let id2 = init_data2.read().clone();
                                                    let next_status = next.to_string();
                                                    let next_label = admin_status_label(next).to_string();
                                                    updating_id.set(Some(oid.clone()));
                                                    let mut orders2 = orders;
                                                    let mut updating2 = updating_id;
                                                    let toasts2 = toasts;
                                                    spawn(async move {
                                                        let url = format!("{}/api/orders/{}/status", api_base_url(), oid);
                                                        let res = HTTP_CLIENT.clone().put(&url)
                                                            .header("X-Telegram-Init-Data", id2)
                                                            .header("X-Admin-Token", admin_token())
                                                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                            .json(&json!({"status": next_status}))
                                                            .send().await;
                                                        updating2.set(None);
                                                        match res {
                                                            Ok(r) if r.status().is_success() => {
                                                                if let Some(o) = orders2.write().iter_mut().find(|o| o.id == oid) { o.status = next_status.clone(); }
                                                                push_toast(toasts2, format!("→ {next_label}"), ToastKind::Success);
                                                            }
                                                            _ => { push_toast(toasts2, "Ошибка смены статуса".into(), ToastKind::Error); }
                                                        }
                                                    });
                                                },
                                                "→ {admin_status_label(next)}"
                                            }
                                        }
                                        if !is_terminal_status(&status3) {
                                            button {
                                                class: "admin-btn danger admin-btn-sm",
                                                onclick: move |_| {
                                                    let oid = order_id3.clone();
                                                    let id_c = init_data.read().clone();
                                                    let mut orders_c = orders;
                                                    let toasts_c = toasts;
                                                    spawn(async move {
                                                        let url = format!("{}/api/orders/{}/status", api_base_url(), oid);
                                                        let res = HTTP_CLIENT.clone().put(&url)
                                                            .header("X-Telegram-Init-Data", id_c)
                                                            .header("X-Admin-Token", admin_token())
                                                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                            .json(&json!({"status": "rejected"}))
                                                            .send().await;
                                                        match res {
                                                            Ok(r) if r.status().is_success() => {
                                                                if let Some(o) = orders_c.write().iter_mut().find(|o| o.id == oid) { o.status = "rejected".into(); }
                                                                push_toast(toasts_c, "Заказ отменён".into(), ToastKind::Success);
                                                            }
                                                            _ => { push_toast(toasts_c, "Ошибка отмены".into(), ToastKind::Error); }
                                                        }
                                                    });
                                                },
                                                "✖ Отмена"
                                            }
                                        }
                                        a {
                                            style: "padding:6px 12px;background:#1a3a1a;color:#39ff14;border:1px solid #2a5a2a;border-radius:4px;font-size:12px;text-decoration:none;cursor:pointer;",
                                            href: "{api_base_url()}/api/orders/{order_id_href}/promptpay-qr",
                                            target: "_blank",
                                            "QR"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                {
                    let page = (*offset.read() / (*limit.read()).max(1)) + 1;
                    let lim = *limit.read();
                    let has_prev = *offset.read() > 0;
                    let has_next = orders.read().len() >= lim as usize;
                    rsx! {
                        div { style: "display:flex;justify-content:center;align-items:center;gap:12px;margin-top:16px;",
                            button {
                                style: if !has_prev { "padding:8px 16px;background:#2a2a4a;color:#666;border:none;border-radius:4px;font-size:13px;cursor:not-allowed;" } else { "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:13px;cursor:pointer;" },
                                disabled: !has_prev,
                                onclick: move |_| {
                                    let new_off = (*offset.read() - *limit.read()).max(0);
                                    offset.set(new_off);
                                },
                                "← Назад"
                            }
                            span { style: "font-size:13px;color:#888;", "Страница {page} (по {lim})" }
                            select {
                                style: "padding:6px 10px;background:#1a1a2e;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:13px;",
                                value: "{lim}",
                                onchange: move |e| {
                                    if let Ok(v) = e.value().parse::<i64>() {
                                        limit.set(v);
                                        offset.set(0);
                                    }
                                },
                                option { value: "20", "20" }
                                option { value: "50", "50" }
                                option { value: "100", "100" }
                            }
                            button {
                                style: if !has_next { "padding:8px 16px;background:#2a2a4a;color:#666;border:none;border-radius:4px;font-size:13px;cursor:not-allowed;" } else { "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:13px;cursor:pointer;" },
                                disabled: !has_next,
                                onclick: move |_| {
                                    let current = *offset.read();
                                    offset.set(current + *limit.read());
                                },
                                "Вперёд →"
                            }
                        }
                    }
                }
            }
            if let Some(o) = selected_order.read().clone() {
                OrderDetailModal { order: o.clone(), on_close: move |_| selected_order.set(None) }
            }
        }
    }
}

// ─── Quests ───────────────────────────────────────────────────

// Quests tab hidden until partner locations are configured.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
struct AdminQuestPlace {
    #[serde(default)]
    id: String,
    name: String,
    #[serde(default)]
    category: String,
    lat: f64,
    lon: f64,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    is_available: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct QuestPlacesResp {
    quest_places: Vec<AdminQuestPlace>,
}

#[component]
#[allow(dead_code)]
fn QuestsTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut places: Signal<Vec<AdminQuestPlace>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut editing: Signal<Option<AdminQuestPlace>> = use_signal(|| None);
    let mut is_new = use_signal(|| false);
    let mut saving = use_signal(|| false);
    let mut error = use_signal(String::new);
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);

    let reload = use_signal(|| 0u32);
    let _ = use_resource(move || {
        let _ = reload.read();
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/quest-places", api_base_url());
            match HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data)
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(data) = resp.json::<QuestPlacesResp>().await {
                        places.set(data.quest_places);
                    }
                }
                _ => {}
            }
            loading.set(false);
            Some(())
        }
    });

    if let Some(ref item) = editing.read().clone() {
        let item = item.clone();
        let item_id = item.id.clone();
        let is_creating = *is_new.read();

        let mut edit_name = use_signal(|| item.name.clone());
        let mut edit_cat = use_signal(|| item.category.clone());
        let mut edit_lat = use_signal(|| item.lat.to_string());
        let mut edit_lon = use_signal(|| item.lon.to_string());
        let mut edit_desc = use_signal(|| item.description.clone().unwrap_or_default());
        let mut edit_img = use_signal(|| item.image_url.clone().unwrap_or_default());

        return rsx! {
            div {
                {render_toasts(toasts)}
                div { class: "admin-card",
                    div { class: "admin-modal-title",
                        if is_creating { "➕ Новая квест-точка" } else { "✏️ Редактировать" }
                    }
                    input { class: "admin-input", placeholder: "Название", value: "{edit_name}",
                        oninput: move |e| edit_name.set(e.value()) }
                    select { class: "admin-select", value: "{edit_cat}",
                        oninput: move |e| edit_cat.set(e.value()),
                        option { value: "beach", "🏖️ Beach" }
                        option { value: "viewpoint", "🌄 Viewpoint" }
                        option { value: "restaurant", "🍽️ Restaurant" }
                        option { value: "bar", "🍻 Bar" }
                        option { value: "temple", "🛭️ Temple" }
                        option { value: "nature", "🌿 Nature" }
                    }
                    div { class: "admin-form-row-2col",
                        input { class: "admin-input", placeholder: "Lat", value: "{edit_lat}", r#type: "number",
                            oninput: move |e| edit_lat.set(e.value()) }
                        input { class: "admin-input", placeholder: "Lon", value: "{edit_lon}", r#type: "number",
                            oninput: move |e| edit_lon.set(e.value()) }
                    }
                    textarea { class: "admin-textarea", placeholder: "Описание", value: "{edit_desc}",
                        oninput: move |e| edit_desc.set(e.value()) }
                    input { class: "admin-input", placeholder: "URL изображения", value: "{edit_img}",
                        oninput: move |e| edit_img.set(e.value()) }
                    if !error.read().is_empty() {
                        div { class: "admin-badge danger", "{error}" }
                    }
                    div { class: "admin-modal-footer",
                        button {
                            class: if *saving.read() { "admin-btn" } else { "admin-btn primary" },
                            disabled: *saving.read(),
                            onclick: move |_| {
                                let n = edit_name.read().trim().to_string();
                                if n.is_empty() { error.set("Название обязательно".into()); return; }
                                let lat = match edit_lat.read().trim().parse::<f64>() {
                                    Ok(v) if v.is_finite() => v,
                                    _ => { error.set("Неверная широта".into()); return; }
                                };
                                let lon = match edit_lon.read().trim().parse::<f64>() {
                                    Ok(v) if v.is_finite() => v,
                                    _ => { error.set("Неверная долгота".into()); return; }
                                };
                                let cat = edit_cat.read().clone();
                                let desc = edit_desc.read().trim().to_string();
                                let img = edit_img.read().trim().to_string();
                                let iid = item_id.clone();
                                let is_cr = is_creating;
                                let id_data = init_data.read().clone();
                                saving.set(true);
                                error.set(String::new());
                                let _places2 = places;
                                let mut editing2 = editing;
                                let mut saving2 = saving;
                                let toasts2 = toasts;
                                let mut reload2 = reload;
                                spawn(async move {
                                    let body = json!({
                                        "name": n.clone(), "category": cat,
                                        "lat": lat, "lon": lon,
                                        "description": if desc.is_empty() { serde_json::Value::Null } else { desc.clone().into() },
                                        "image_url": if img.is_empty() { serde_json::Value::Null } else { img.clone().into() },
                                    });
                                    let base = api_base_url();
                                    let res = if is_cr {
                                        HTTP_CLIENT.clone().post(&format!("{}/api/quest-places", base))
                                            .header("X-Telegram-Init-Data", id_data)
                                            .header("X-Admin-Token", admin_token())
                                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                            .json(&body).send().await
                                    } else {
                                        HTTP_CLIENT.clone().put(&format!("{}/api/quest-places/{}", base, iid))
                                            .header("X-Telegram-Init-Data", id_data)
                                            .header("X-Admin-Token", admin_token())
                                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                            .json(&body).send().await
                                    };
                                    saving2.set(false);
                                    match res {
                                        Ok(r) if r.status().is_success() => {
                                            editing2.set(None);
                                            { let v = reload2.read().wrapping_add(1); reload2.set(v); }
                                            push_toast(toasts2, "✓ Сохранено".into(), ToastKind::Success);
                                        }
                                        _ => { push_toast(toasts2, "Ошибка сохранения".into(), ToastKind::Error); }
                                    }
                                });
                            },
                            "💾 Сохранить"
                        }
                        button { class: "admin-btn secondary", onclick: move |_| { editing.set(None); error.set(String::new()); }, "Отмена" }
                    }
                }
            }
        };
    }

    rsx! {
        div {
            {render_toasts(toasts)}
            h3 { class: "admin-card-title", "🗺️ Квест-точки" }
            button {
                class: "admin-btn primary",
                class: "admin-btn-full",
                onclick: move |_| {
                    is_new.set(true);
                    editing.set(Some(AdminQuestPlace {
                        id: String::new(), name: String::new(), category: "beach".into(),
                        lat: 0.0, lon: 0.0, description: None, image_url: None, is_available: true,
                    }));
                },
                "+ Добавить точку"
            }
            if *loading.read() {
                EmptyState { icon: "⏳".to_string(), title: "Загрузка...".to_string(), description: "Получаем данные с сервера".to_string() }
            } else if places.read().is_empty() {
                div { class: "admin-empty", "Нет точек" }
            } else {
                div {
                    for place in places.read().clone() {
                        {
                            let p2 = place.clone();
                            let _p3 = place.clone();
                            let p_id = place.id.clone();
                            let id_del = init_data.read().clone();
                            let toasts2 = toasts;
                            let _reload2 = reload;
                            rsx! {
                                div { class: "admin-row",
                                    div { class: "admin-row-main",
                                        div { class: "admin-card-title", "{place.name}" }
                                        div { class: "admin-card-meta",
                                            "{place.category} • {place.lat:.4}, {place.lon:.4}"
                                        }
                                    }
                                    div { class: "admin-row-actions",
                                        button { class: "admin-btn secondary admin-btn-sm",
                                            "aria-label": "Редактировать",
                                            onclick: move |_| { is_new.set(false); editing.set(Some(p2.clone())); },
                                            "✏️"
                                        }
                                        button { class: "admin-btn danger admin-btn-sm",
                                            "aria-label": "Удалить",
                                            onclick: move |_| {
                                                let pid = p_id.clone();
                                                let id_d = id_del.clone();
                                                let mut places3 = places;
                                                let toasts3 = toasts2;
                                                spawn(async move {
                                                    let url = format!("{}/api/quest-places/{}", api_base_url(), pid);
                                                    let res = HTTP_CLIENT.clone().delete(&url)
                                                        .header("X-Telegram-Init-Data", id_d)
                                                        .header("X-Admin-Token", admin_token())
                                                        .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                        .send().await;
                                                    match res {
                                                        Ok(r) if r.status().is_success() => {
                                                            places3.write().retain(|p| p.id != pid);
                                                            push_toast(toasts3, "Удалено".into(), ToastKind::Success);
                                                        }
                                                        _ => { push_toast(toasts3, "Ошибка удаления".into(), ToastKind::Error); }
                                                    }
                                                });
                                            },
                                            "🗑"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Treasures ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdminTreasureHunt {
    #[serde(default)]
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    black_mark_title: String,
    #[serde(default)]
    black_mark_description: Option<String>,
    #[serde(default)]
    black_mark_image_url: Option<String>,
    #[serde(default)]
    start_lat: f64,
    #[serde(default)]
    start_lon: f64,
    #[serde(default)]
    start_name: String,
    #[serde(default)]
    is_active: bool,
}

#[derive(Debug, Deserialize)]
struct TreasureHuntsResp {
    treasure_hunts: Vec<AdminTreasureHunt>,
}

#[component]
fn TreasuresTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut hunts: Signal<Vec<AdminTreasureHunt>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut editing: Signal<Option<AdminTreasureHunt>> = use_signal(|| None);
    let mut is_new = use_signal(|| false);
    let mut saving = use_signal(|| false);
    let mut error = use_signal(String::new);
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);
    let reload = use_signal(|| 0u32);

    let _ = use_resource(move || {
        let _ = reload.read();
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/treasure-hunts", api_base_url());
            match HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data)
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(data) = resp.json::<TreasureHuntsResp>().await {
                        hunts.set(data.treasure_hunts);
                    }
                }
                _ => {}
            }
            loading.set(false);
            Some(())
        }
    });

    if let Some(ref item) = editing.read().clone() {
        let item = item.clone();
        let item_id = item.id.clone();
        let is_creating = *is_new.read();

        let mut edit_name = use_signal(|| item.name.clone());
        let mut edit_desc = use_signal(|| item.description.clone().unwrap_or_default());
        let mut edit_img = use_signal(|| item.image_url.clone().unwrap_or_default());
        let mut edit_bm_title = use_signal(|| item.black_mark_title.clone());
        let mut edit_bm_desc =
            use_signal(|| item.black_mark_description.clone().unwrap_or_default());
        let mut edit_bm_img = use_signal(|| item.black_mark_image_url.clone().unwrap_or_default());
        let mut edit_start_lat = use_signal(|| item.start_lat.to_string());
        let mut edit_start_lon = use_signal(|| item.start_lon.to_string());
        let mut edit_start_name = use_signal(|| item.start_name.clone());

        return rsx! {
            div {
                {render_toasts(toasts)}
                div { class: "admin-card",
                    div { class: "admin-modal-title",
                        if is_creating { "➕ Новый квест" } else { "✏️ Редактировать" }
                    }
                    input { class: "admin-input", placeholder: "Название квеста", value: "{edit_name}",
                        oninput: move |e| edit_name.set(e.value()) }
                    textarea { class: "admin-textarea", placeholder: "Описание", value: "{edit_desc}",
                        oninput: move |e| edit_desc.set(e.value()) }
                    input { class: "admin-input", placeholder: "URL изображения", value: "{edit_img}",
                        oninput: move |e| edit_img.set(e.value()) }
                    div { class: "admin-label", "☠️ Чёрная Метка (финальная награда)" }
                    input { class: "admin-input", placeholder: "Заголовок", value: "{edit_bm_title}",
                        oninput: move |e| edit_bm_title.set(e.value()) }
                    textarea { class: "admin-textarea", placeholder: "Описание чёрной метки", value: "{edit_bm_desc}",
                        oninput: move |e| edit_bm_desc.set(e.value()) }
                    input { class: "admin-input", placeholder: "URL постера", value: "{edit_bm_img}",
                        oninput: move |e| edit_bm_img.set(e.value()) }
                    div { class: "admin-label", "🏴\u{200d}☠️ Стартовая точка (Woody)" }
                    input { class: "admin-input", placeholder: "Название", value: "{edit_start_name}",
                        oninput: move |e| edit_start_name.set(e.value()) }
                    div { class: "admin-form-row-2col",
                        input { class: "admin-input", placeholder: "Lat", value: "{edit_start_lat}", r#type: "number",
                            oninput: move |e| edit_start_lat.set(e.value()) }
                        input { class: "admin-input", placeholder: "Lon", value: "{edit_start_lon}", r#type: "number",
                            oninput: move |e| edit_start_lon.set(e.value()) }
                    }
                    if !error.read().is_empty() {
                        div { class: "admin-badge danger", "{error}" }
                    }
                    div { class: "admin-modal-footer",
                        button {
                            class: if *saving.read() { "admin-btn" } else { "admin-btn primary" },
                            disabled: *saving.read(),
                            onclick: move |_| {
                                let n = edit_name.read().trim().to_string();
                                if n.is_empty() { error.set("Название обязательно".into()); return; }
                                let bm_title = edit_bm_title.read().trim().to_string();
                                if bm_title.is_empty() { error.set("Заголовок Чёрной Метки обязателен".into()); return; }
                                let desc = edit_desc.read().trim().to_string();
                                let img = edit_img.read().trim().to_string();
                                let bm_desc = edit_bm_desc.read().trim().to_string();
                                let bm_img = edit_bm_img.read().trim().to_string();
                                // Cycle #137: strict parse — empty / typo / out-of-range
                                // would silently `unwrap_or(0.0)` previously and ship
                                // "Gulf of Guinea" coordinates.
                                let start_lat = match crate::trios::validation::parse_finite_float_in_range(
                                    &edit_start_lat.read(), "Широта", -90.0, 90.0,
                                ) {
                                    Ok(v) => v,
                                    Err(msg) => { error.set(msg); return; }
                                };
                                let start_lon = match crate::trios::validation::parse_finite_float_in_range(
                                    &edit_start_lon.read(), "Долгота", -180.0, 180.0,
                                ) {
                                    Ok(v) => v,
                                    Err(msg) => { error.set(msg); return; }
                                };
                                let start_name = edit_start_name.read().trim().to_string();
                                let iid = item_id.clone();
                                let is_cr = is_creating;
                                let id_data = init_data.read().clone();
                                saving.set(true);
                                error.set(String::new());
                                let mut editing2 = editing;
                                let mut saving2 = saving;
                                let toasts2 = toasts;
                                let mut reload2 = reload;
                                spawn(async move {
                                    let body = json!({
                                        "name": n,
                                        "description": if desc.is_empty() { serde_json::Value::Null } else { desc.into() },
                                        "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                        "black_mark_title": bm_title,
                                        "black_mark_description": if bm_desc.is_empty() { serde_json::Value::Null } else { bm_desc.into() },
                                        "black_mark_image_url": if bm_img.is_empty() { serde_json::Value::Null } else { bm_img.into() },
                                        "start_lat": start_lat, "start_lon": start_lon, "start_name": start_name,
                                    });
                                    let base = api_base_url();
                                    let res = if is_cr {
                                        HTTP_CLIENT.clone().post(&format!("{}/api/treasure-hunts", base))
                                            .header("X-Telegram-Init-Data", id_data)
                                            .header("X-Admin-Token", admin_token())
                                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                            .json(&body).send().await
                                    } else {
                                        HTTP_CLIENT.clone().put(&format!("{}/api/treasure-hunts/{}", base, iid))
                                            .header("X-Telegram-Init-Data", id_data)
                                            .header("X-Admin-Token", admin_token())
                                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                            .json(&body).send().await
                                    };
                                    saving2.set(false);
                                    match res {
                                        Ok(r) if r.status().is_success() => {
                                            editing2.set(None);
                                            { let v = reload2.read().wrapping_add(1); reload2.set(v); }
                                            push_toast(toasts2, "✓ Сохранено".into(), ToastKind::Success);
                                        }
                                        _ => { push_toast(toasts2, "Ошибка сохранения".into(), ToastKind::Error); }
                                    }
                                });
                            },
                            "💾 Сохранить"
                        }
                        button { class: "admin-btn secondary", onclick: move |_| { editing.set(None); error.set(String::new()); }, "Отмена" }
                    }
                }
            }
        };
    }

    rsx! {
        div {
            {render_toasts(toasts)}
            h3 { class: "admin-card-title", "🏴\u{200d}☠️ Поиск Сокровищ" }
            button {
                class: "admin-btn primary",
                class: "admin-btn-full",
                onclick: move |_| {
                    is_new.set(true);
                    editing.set(Some(AdminTreasureHunt {
                        id: String::new(), name: String::new(), description: None, image_url: None,
                        black_mark_title: String::new(), black_mark_description: None, black_mark_image_url: None,
                        start_lat: 0.0, start_lon: 0.0, start_name: String::new(), is_active: true,
                    }));
                },
                "+ Создать квест"
            }
            if *loading.read() {
                EmptyState { icon: "⏳".to_string(), title: "Загрузка...".to_string(), description: "Получаем данные с сервера".to_string() }
            } else if hunts.read().is_empty() {
                div { class: "admin-empty", "Нет квестов" }
            } else {
                div {
                    for hunt in hunts.read().clone() {
                        {
                            let h2 = hunt.clone();
                            let h_id = hunt.id.clone();
                            let id_del = init_data.read().clone();
                            let toasts2 = toasts;
                            let _reload2 = reload;
                            rsx! {
                                div { class: "admin-row",
                                    div { class: "admin-row-main",
                                        div { class: "admin-card-title",
                                            "🏴\u{200d}☠️ {hunt.name}"
                                            if hunt.is_active {
                                                span { class: "admin-badge success", "Активен" }
                                            }
                                        }
                                        div { class: "admin-card-meta",
                                            "Чёрная метка: {hunt.black_mark_title}"
                                        }
                                    }
                                    div { class: "admin-row-actions",
                                        button { class: "admin-btn secondary admin-btn-sm",
                                            "aria-label": "Редактировать",
                                            onclick: move |_| { is_new.set(false); editing.set(Some(h2.clone())); },
                                            "✏️"
                                        }
                                        button { class: "admin-btn danger admin-btn-sm",
                                        "aria-label": "Удалить",
                                        onclick: move |_| {
                                            let hid = h_id.clone();
                                            let id_d = id_del.clone();
                                            let mut hunts2 = hunts;
                                            let toasts3 = toasts2;
                                            spawn(async move {
                                                let url = format!("{}/api/treasure-hunts/{}", api_base_url(), hid);
                                                let res = HTTP_CLIENT.clone().delete(&url)
                                                    .header("X-Telegram-Init-Data", id_d)
                                                    .header("X-Admin-Token", admin_token())
                                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                    .send().await;
                                                match res {
                                                    Ok(r) if r.status().is_success() => {
                                                        hunts2.write().retain(|h| h.id != hid);
                                                        push_toast(toasts3, "Удалено".into(), ToastKind::Success);
                                                    }
                                                    _ => { push_toast(toasts3, "Ошибка удаления".into(), ToastKind::Error); }
                                                }
                                            });
                                        },
                                            "🗑"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Garden ───────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct GardenConfig {
    #[serde(default = "default_true")]
    is_enabled: bool,
    #[serde(default)]
    reward_discount_percent: f64,
    #[serde(default)]
    reward_bonus_points: f64,
    #[serde(default)]
    reward_expiration_days: f64,
}

fn default_true() -> bool {
    true
}

#[component]
fn GardenTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut config: Signal<Option<GardenConfig>> = use_signal(|| None);
    let mut loading = use_signal(|| true);
    let mut saving = use_signal(|| false);
    let mut is_enabled = use_signal(|| true);
    let mut discount = use_signal(|| "10".to_string());
    let mut bonus_points = use_signal(|| "100".to_string());
    let mut expire_days = use_signal(|| "7".to_string());
    let mut error = use_signal(String::new);
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);

    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/garden/config", api_base_url());
            match HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data)
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(cfg) = resp.json::<GardenConfig>().await {
                        is_enabled.set(cfg.is_enabled);
                        discount.set(cfg.reward_discount_percent.to_string());
                        bonus_points.set(cfg.reward_bonus_points.to_string());
                        expire_days.set(cfg.reward_expiration_days.to_string());
                        config.set(Some(cfg));
                    }
                }
                _ => {}
            }
            loading.set(false);
            Some(())
        }
    });

    rsx! {
        div {
            {render_toasts(toasts)}
            h3 { class: "admin-card-title", "🌱 Сад — Настройки" }
            if *loading.read() {
                EmptyState { icon: "⏳".to_string(), title: "Загрузка...".to_string(), description: "Получаем данные с сервера".to_string() }
            } else {
                div {
                    // Toggle enabled
                    div { class: "admin-card",
                        div { class: "admin-row",
                            div { class: "admin-row-main",
                                div { class: "admin-card-title", "Игра активна" }
                                div { class: "admin-card-meta", "Включить/выключить игру Сад" }
                            }
                            button {
                                class: if *is_enabled.read() { "admin-btn primary" } else { "admin-btn danger" },
                                onclick: move |_| { let v = !*is_enabled.read(); is_enabled.set(v); },
                                if *is_enabled.read() { "✓ Вкл" } else { "✖ Выкл" }
                            }
                        }
                    }
                    // Discount
                    div { class: "admin-form-group",
                        label { class: "admin-label", "Скидка за награду (%)" }
                        input { class: "admin-input", r#type: "number",
                            value: "{discount}", oninput: move |e| discount.set(e.value()) }
                    }
                    // Bonus points
                    div { class: "admin-form-group",
                        label { class: "admin-label", "Бонусные баллы за урожай" }
                        input { class: "admin-input", r#type: "number",
                            value: "{bonus_points}", oninput: move |e| bonus_points.set(e.value()) }
                    }
                    // Expiration days
                    div { class: "admin-form-group",
                        label { class: "admin-label", "Срок действия награды (дней)" }
                        input { class: "admin-input", r#type: "number",
                            value: "{expire_days}", oninput: move |e| expire_days.set(e.value()) }
                    }
                    if !error.read().is_empty() {
                        div { class: "admin-badge danger", "{error}" }
                    }
                    button {
                        class: if *saving.read() { "admin-btn" } else { "admin-btn primary" },
                        disabled: *saving.read(),
                        onclick: move |_| {
                            let enabled = *is_enabled.read();
                            // The backend wants integers (u32); serde_json will NOT
                            // coerce a JSON float like `10.0` into u32, so sending
                            // floats here yielded a silent 422 ("не сохранялось").
                            // Parse + round to integers and bound-check to match the
                            // server's validate_garden_config_update ranges.
                            let disc = match discount.read().trim().parse::<f64>() {
                                Ok(v) if v.is_finite() && (0.0..=100.0).contains(&v) => v.round() as u32,
                                _ => { error.set("Скидка: число 0–100".into()); return; }
                            };
                            let bp = match bonus_points.read().trim().parse::<f64>() {
                                Ok(v) if v.is_finite() && (0.0..=1_000_000.0).contains(&v) => v.round() as u32,
                                _ => { error.set("Бонусы: число 0–1000000".into()); return; }
                            };
                            let ed = match expire_days.read().trim().parse::<f64>() {
                                Ok(v) if v.is_finite() && (1.0..=365.0).contains(&v) => v.round() as u32,
                                _ => { error.set("Срок: число 1–365 дней".into()); return; }
                            };
                            let id_data = init_data.read().clone();
                            saving.set(true);
                            error.set(String::new());
                            let mut saving2 = saving;
                            let toasts2 = toasts;
                            let mut error2 = error;
                            spawn(async move {
                                let body = json!({
                                    "is_enabled": enabled,
                                    "reward_discount_percent": disc,
                                    "reward_bonus_points": bp,
                                    "reward_expiration_days": ed,
                                });
                                let url = format!("{}/api/garden/config", api_base_url());
                                let res = HTTP_CLIENT.clone().put(&url)
                                    .header("X-Telegram-Init-Data", id_data)
                                    .header("X-Admin-Token", admin_token())
                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                saving2.set(false);
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        push_toast(toasts2, "✓ Настройки сохранены!".into(), ToastKind::Success);
                                    }
                                    Ok(r) => {
                                        let st = r.status().as_u16();
                                        let body = r.text().await.unwrap_or_default();
                                        let b = body.trim();
                                        let msg = if b.is_empty() { format!("Ошибка сохранения (HTTP {st})") }
                                            else { format!("Ошибка сохранения (HTTP {st}): {}", b.chars().take(80).collect::<String>()) };
                                        error2.set(msg.clone());
                                        push_toast(toasts2, msg, ToastKind::Error);
                                    }
                                    Err(_) => {
                                        error2.set("Ошибка сети/таймаут".into());
                                        push_toast(toasts2, "Ошибка сети/таймаут".into(), ToastKind::Error);
                                    }
                                }
                            });
                        },
                        if *saving.read() { "⏳ Сохранение..." } else { "💾 Сохранить настройки" }
                    }
                    GardenEligibility {}
                }
            }
        }
    }
}

// ─── Garden eligibility (B5: opt products in/out of the garden chooser) ───

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct EligProduct {
    catalog: String,
    id: String,
    name: String,
    #[serde(default = "default_true")]
    garden_eligible: bool,
}

#[derive(Debug, Deserialize)]
struct EligResp {
    #[serde(default)]
    products: Vec<EligProduct>,
}

fn garden_catalog_label(c: &str) -> &'static str {
    match c {
        "strain" => "🌿 Сорта",
        "accessory" => "💨 Аксессуары",
        "tea" => "🥤 Напитки",
        "set" => "📦 Наборы",
        "accessory_set" => "🔧 Сеты аксессуаров",
        "tea_set" => "🫖 Сеты напитков",
        _ => "Прочее",
    }
}

#[component]
fn GardenEligibility() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut items: Signal<Vec<EligProduct>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);

    let _ = use_resource(move || {
        let init = init_data.read().clone();
        async move {
            let url = format!("{}/api/garden/eligibility", api_base_url());
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init)
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(data) = resp.json::<EligResp>().await {
                        items.set(data.products);
                    }
                }
            }
            loading.set(false);
            Some(())
        }
    });

    rsx! {
        div { style: "margin-top:20px;",
            h3 { class: "admin-card-title", "🌱 Участвуют в саду" }
            div { class: "admin-card-meta", style: "margin-bottom:10px;",
                "Выключи товар — и его нельзя будет выбрать для выращивания скидки." }
            if *loading.read() {
                EmptyState { icon: "⏳".to_string(), title: "Загрузка...".to_string(), description: "".to_string() }
            } else {
                for p in items.read().clone().into_iter() {
                    {
                        let on = p.garden_eligible;
                        let cat = p.catalog.clone();
                        let pid = p.id.clone();
                        let name = p.name.clone();
                        let badge = garden_catalog_label(&p.catalog);
                        let id_data = init_data.read().clone();
                        rsx! {
                            div { style: "display:flex;justify-content:space-between;align-items:center;gap:8px;background:#1a1a2e;padding:8px 10px;border-radius:6px;margin-bottom:6px;",
                                div { style: "min-width:0;",
                                    div { style: "font-size:10px;color:#8b8b9e;", "{badge}" }
                                    div { style: "font-size:13px;color:#e8e8e8;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;", "{name}" }
                                }
                                button {
                                    class: if on { "admin-btn primary" } else { "admin-btn danger" },
                                    onclick: move |_| {
                                        let new_val = !on;
                                        let cat = cat.clone(); let pid = pid.clone(); let id_data = id_data.clone();
                                        let mut items2 = items;
                                        spawn(async move {
                                            let url = format!("{}/api/garden/eligible", api_base_url());
                                            let body = json!({ "catalog": cat, "product_id": pid, "eligible": new_val });
                                            let res = HTTP_CLIENT.clone().put(&url)
                                                .header("X-Telegram-Init-Data", id_data)
                                                .header("X-Admin-Token", admin_token())
                                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                .json(&body).send().await;
                                            if matches!(res, Ok(ref r) if r.status().is_success()) {
                                                if let Some(it) = items2.write().iter_mut().find(|x| x.id == pid && x.catalog == cat) {
                                                    it.garden_eligible = new_val;
                                                }
                                                TelegramApp::init().haptic_notification(HapticNotification::Success);
                                            } else {
                                                TelegramApp::init().haptic_notification(HapticNotification::Error);
                                            }
                                        });
                                    },
                                    if on { "✓ Вкл" } else { "✖ Выкл" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Loyalty ──────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct LoyaltyTier {
    tier: String,
    name: String,
    min_points: i64,
    discount_percent: i64,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    color: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct LeaderboardEntry {
    #[serde(default)]
    telegram_id: Option<i64>,
    #[serde(default)]
    first_name: Option<String>,
    #[serde(default)]
    total_spent: Option<f64>,
    #[serde(default)]
    tier: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoyaltyConfigResp {
    #[serde(default)]
    tiers: Vec<LoyaltyTier>,
}

#[derive(Debug, Deserialize)]
struct LeaderboardResp {
    #[serde(default)]
    leaderboard: Vec<LeaderboardEntry>,
}

#[component]
fn LoyaltyTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut tiers: Signal<Vec<LoyaltyTier>> = use_signal(Vec::new);
    let mut leaderboard: Signal<Vec<LeaderboardEntry>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut active_sub = use_signal(|| "config".to_string());
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);

    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        async move {
            let base = api_base_url();
            // Fetch config/tiers
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&format!("{}/api/loyalty/tiers", base))
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(data) = resp.json::<LoyaltyConfigResp>().await {
                        tiers.set(data.tiers);
                    }
                }
            }
            // Fetch leaderboard
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&format!("{}/api/loyalty/leaderboard", base))
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(data) = resp.json::<LeaderboardResp>().await {
                        leaderboard.set(data.leaderboard);
                    }
                }
            }
            loading.set(false);
            Some(())
        }
    });

    rsx! {
        div {
            {render_toasts(toasts)}
            h3 { class: "admin-card-title", "💎 Лояльность" }
            div { class: "admin-subtabs",
                button {
                    class: if *active_sub.read() == "config" { "admin-subtab active" } else { "admin-subtab" },
                    onclick: move |_| active_sub.set("config".into()),
                    "🎟️ Тиры"
                }
                button {
                    class: if *active_sub.read() == "leaderboard" { "admin-subtab active" } else { "admin-subtab" },
                    onclick: move |_| active_sub.set("leaderboard".into()),
                    "🏆 Лидерборд"
                }
            }
            if *loading.read() {
                EmptyState { icon: "⏳".to_string(), title: "Загрузка...".to_string(), description: "Получаем данные с сервера".to_string() }
            } else if *active_sub.read() == "config" {
                if tiers.read().is_empty() {
                    div { class: "admin-empty", "Нет тиров" }
                } else {
                    div {
                        for tier in tiers.read().clone() {
                            div { class: "admin-row",
                                div { class: "admin-emoji-lg",
                                    "{tier.icon.clone().unwrap_or_else(|| \"💎\".to_string())}"
                                }
                                div { class: "admin-row-main",
                                    div { class: "admin-card-title", "{tier.name}" }
                                    div { class: "admin-card-meta",
                                        "От {tier.min_points} баллов • Скидка {tier.discount_percent}%"
                                    }
                                }
                                span { class: "admin-badge success",
                                    "{tier.tier}"
                                }
                            }
                        }
                    }
                }
            } else {
                // Leaderboard
                if leaderboard.read().is_empty() {
                    div { class: "admin-empty", "Лидерборд пустой" }
                } else {
                    div {
                        for (idx, entry) in leaderboard.read().clone().iter().enumerate() {
                            {
                                let medal = match idx {
                                    0 => "🥇",
                                    1 => "🥈",
                                    2 => "🥉",
                                    _ => "",
                                };
                                let name = entry.first_name.clone().unwrap_or_else(|| "Unknown".to_string());
                                let spent = entry.total_spent.unwrap_or(0.0);
                                let tier_label = entry.tier.clone().unwrap_or_default();
                                rsx! {
                                    div { class: "admin-row",
                                        div { class: "admin-medal",
                                            if medal.is_empty() {
                                                span { class: "admin-badge muted", "{idx+1}" }
                                            } else {
                                                "{medal}"
                                            }
                                        }
                                        div { class: "admin-row-main",
                                            div { class: "admin-card-title", "{name}" }
                                            div { class: "admin-card-meta", "{tier_label}" }
                                        }
                                        div { class: "admin-badge success",
                                            "{spent:.0}Б"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Managers ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdminManager {
    telegram_id: i64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    ref_code: Option<String>,
    #[serde(default)]
    commission_rate: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ManagersResp {
    managers: Vec<AdminManager>,
}

#[component]
fn ManagersTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut managers: Signal<Vec<AdminManager>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(String::new);
    let mut show_form = use_signal(|| false);
    let mut form_tg_id = use_signal(String::new);
    let mut form_name = use_signal(String::new);
    let mut form_commission = use_signal(|| "10".to_string());
    let mut submitting = use_signal(|| false);
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);
    let reload = use_signal(|| 0u32);
    let mut selected_manager: Signal<Option<AdminManager>> = use_signal(|| None);

    let _ = use_resource(move || {
        let _ = reload.read();
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/admin/managers", api_base_url());
            match HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data)
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => match resp.json::<ManagersResp>().await {
                    Ok(data) => {
                        managers.set(data.managers);
                    }
                    Err(e) => {
                        error.set(format!("Ошибка разбора: {e}"));
                    }
                },
                Ok(resp) => {
                    error.set(format!("HTTP {}", resp.status().as_u16()));
                }
                Err(e) => {
                    error.set(format!("Сеть: {e}"));
                }
            }
            loading.set(false);
            Some(())
        }
    });

    rsx! {
        div {
            {render_toasts(toasts)}
            h3 { class: "admin-card-title", "👥 Менеджеры" }
            if !error.read().is_empty() {
                div { class: "admin-badge danger", "{error}" }
            }
            button {
                class: "admin-btn primary",
                class: "admin-btn-full",
                onclick: move |_| { let v = !*show_form.read(); show_form.set(v); },
                if *show_form.read() { "✖ Скрыть форму" } else { "+ Добавить менеджера" }
            }
            if *show_form.read() {
                div { class: "admin-card",
                    div { class: "admin-modal-title", "➕ Новый менеджер" }
                    div { class: "admin-form-group",
                        input { class: "admin-input", placeholder: "Telegram ID", value: "{form_tg_id}",
                            r#type: "number", oninput: move |e| form_tg_id.set(e.value()) }
                    }
                    div { class: "admin-form-group",
                        input { class: "admin-input", placeholder: "Имя", value: "{form_name}",
                            oninput: move |e| form_name.set(e.value()) }
                    }
                    div { class: "admin-form-group",
                        input { class: "admin-input", placeholder: "Commission % (напр. 10)", value: "{form_commission}",
                            r#type: "number", oninput: move |e| form_commission.set(e.value()) }
                    }
                    button {
                        class: if *submitting.read() { "admin-btn" } else { "admin-btn primary" },
                        disabled: *submitting.read(),
                        onclick: move |_| {
                            let tg_id_str = form_tg_id.read().trim().to_string();
                            let tg_id = match tg_id_str.parse::<i64>() {
                                Ok(v) => v,
                                Err(_) => { push_toast(toasts, "Неверный Telegram ID".into(), ToastKind::Error); return; }
                            };
                            let name = form_name.read().trim().to_string();
                            let commission = form_commission.read().trim().parse::<f64>().unwrap_or(10.0);
                            if !commission.is_finite() { push_toast(toasts, "Неверное значение комиссии".into(), ToastKind::Error); return; }
                            let id_data = init_data.read().clone();
                            submitting.set(true);
                            let mut submitting2 = submitting;
                            let mut show_form2 = show_form;
                            let toasts2 = toasts;
                            let mut reload2 = reload;
                            let mut form_tg_id2 = form_tg_id;
                            let mut form_name2 = form_name;
                            spawn(async move {
                                let body = json!({
                                    "telegram_id": tg_id,
                                    "name": if name.is_empty() { serde_json::Value::Null } else { name.into() },
                                    "commission_rate": commission,
                                });
                                let url = format!("{}/api/admin/managers", api_base_url());
                                let res = HTTP_CLIENT.clone().post(&url)
                                    .header("X-Telegram-Init-Data", id_data)
                                    .header("X-Admin-Token", admin_token())
                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                submitting2.set(false);
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        show_form2.set(false);
                                        form_tg_id2.set(String::new());
                                        form_name2.set(String::new());
                                        { let v = reload2.read().wrapping_add(1); reload2.set(v); }
                                        push_toast(toasts2, "✓ Менеджер добавлен".into(), ToastKind::Success);
                                    }
                                    _ => { push_toast(toasts2, "Ошибка сохранения".into(), ToastKind::Error); }
                                }
                            });
                        },
                        if *submitting.read() { "⏳ Сохранение..." } else { "💾 Сохранить" }
                    }
                }
            }
            if *loading.read() {
                EmptyState { icon: "⏳".to_string(), title: "Загрузка...".to_string(), description: "Получаем данные с сервера".to_string() }
            } else if managers.read().is_empty() {
                div { class: "admin-empty", "Нет менеджеров" }
            } else {
                div {
                    for mgr in managers.read().clone() {
                        {
                            let mgr_clone = mgr.clone();
                            rsx! {
                                div {
                                    class: "admin-card",
                                    style: "cursor:pointer;",
                                    onclick: move |_| { selected_manager.set(Some(mgr_clone.clone())); },
                                    div { class: "admin-card-title",
                                        "👤 {mgr.name.clone().unwrap_or_else(|| \"—\".to_string())}"
                                        if let Some(uname) = mgr.username.clone() {
                                            span { class: "admin-badge info",
                                                "(@{uname})"
                                            }
                                        }
                                    }
                                    div { class: "admin-card-meta",
                                        "Telegram ID: {mgr.telegram_id}"
                                        if let Some(rate) = mgr.commission_rate {
                                            span { class: "admin-badge warn", " • {rate}% комиссия" }
                                        }
                                        if let Some(ref_code) = mgr.ref_code.clone() {
                                            div { class: "admin-badge info",
                                                "Реф. код: {ref_code}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(mgr) = selected_manager.read().clone() {
                ManagerDetailModal {
                    manager: mgr,
                    on_close: move |_| { selected_manager.set(None); },
                    init_data: init_data.read().clone(),
                }
            }
        }
    }
}

// ── ManagerDetailModal ────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
struct ManagerDetailModalProps {
    manager: AdminManager,
    on_close: EventHandler<()>,
    init_data: String,
}

#[component]
fn ManagerDetailModal(props: ManagerDetailModalProps) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let mgr = props.manager.clone();
    let mut editing = use_signal(|| false);
    let mut edit_name = use_signal(|| mgr.name.clone().unwrap_or_default());
    let mut edit_ref_code = use_signal(|| mgr.ref_code.clone().unwrap_or_default());
    let mut saving = use_signal(|| false);
    let stats_text = use_signal(|| "Загрузка...".to_string());
    let manager_id = mgr.telegram_id;
    let init_data = props.init_data.clone();

    // Fetch stats (endpoint may not exist yet — OK)
    let stats_signal = stats_text;
    let init_data2 = init_data.clone();
    use_effect(move || {
        let url = format!("{}/api/admin/managers/{}/stats", api_base_url(), manager_id);
        let id_data = init_data2.clone();
        spawn(async move {
            match HTTP_CLIENT
                .clone()
                .get(&url)
                .header("X-Telegram-Init-Data", id_data)
                .header("X-Admin-Token", admin_token())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send()
                .await
            {
                Ok(r) if r.status().is_success() => {
                    if let Ok(text) = r.text().await {
                        stats_signal.clone().set(text);
                    }
                }
                _ => {}
            }
        });
    });

    rsx! {
        div {
            class: "admin-overlay",
            onclick: move |_| { props.on_close.call(()); },
            div {
                class: "admin-modal",
                onclick: move |e| { e.stop_propagation(); },
                // Header
                div { class: "admin-modal-header",
                    span { class: "admin-modal-title", "👤 Карточка менеджера" }
                    button {
                        class: "admin-modal-close",
                        "aria-label": "Закрыть",
                        onclick: move |_| { props.on_close.call(()); },
                        "✖"
                    }
                }
                // Info section
                if !*editing.read() {
                    div { class: "admin-form-group",
                        div { class: "admin-card-meta", "Telegram ID: {mgr.telegram_id}" }
                        div { class: "admin-card-meta",
                            "Имя: {mgr.name.clone().unwrap_or_else(|| \"—\".to_string())}"
                        }
                        div { class: "admin-card-meta",
                            "@{mgr.username.clone().unwrap_or_else(|| \"—\".to_string())}"
                        }
                        div { class: "admin-card-meta",
                            "Реф-код: {mgr.ref_code.clone().unwrap_or_else(|| \"—\".to_string())}"
                        }
                    }
                    // Stats section
                    div { class: "admin-card-title", "📊 Статистика" }
                    div { class: "admin-card-meta", "{stats_text}" }
                    // Actions
                    button {
                        class: "admin-btn secondary",
                        onclick: move |_| { editing.set(true); },
                        "✏️ Редактировать"
                    }
                } else {
                    // Edit mode
                    div { class: "admin-form-group",
                        input {
                            class: "admin-input",
                            placeholder: "Имя",
                            value: "{edit_name}",
                            oninput: move |e| edit_name.set(e.value()),
                        }
                    }
                    div { class: "admin-form-group",
                        input {
                            class: "admin-input",
                            placeholder: "Реф-код",
                            value: "{edit_ref_code}",
                            oninput: move |e| edit_ref_code.set(e.value()),
                        }
                    }
                    div { class: "admin-row",
                        button {
                            class: if *saving.read() { "admin-btn" } else { "admin-btn primary" },
                            disabled: *saving.read(),
                            onclick: {
                                let init_data = init_data.clone();
                                let tg_id = mgr.telegram_id;
                                move |_| {
                                    let name = edit_name.read().trim().to_string();
                                    let ref_code = edit_ref_code.read().trim().to_string();
                                    let id_data = init_data.clone();
                                    saving.set(true);
                                    let mut saving2 = saving;
                                    let mut editing2 = editing;
                                    spawn(async move {
                                        let body = json!({
                                            "name": if name.is_empty() { serde_json::Value::Null } else { name.into() },
                                            "ref_code": if ref_code.is_empty() { serde_json::Value::Null } else { ref_code.into() },
                                        });
                                        let url = format!("{}/api/admin/managers/{}", api_base_url(), tg_id);
                                        let _ = HTTP_CLIENT.clone().put(&url)
                                            .header("X-Telegram-Init-Data", id_data)
                                            .header("X-Admin-Token", admin_token())
                                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                            .json(&body).send().await;
                                        saving2.set(false);
                                        editing2.set(false);
                                    });
                                }
                            },
                            if *saving.read() { "⏳ Сохранение..." } else { "💾 Сохранить" }
                        }
                        button {
                            class: "admin-btn",
                            onclick: move |_| { editing.set(false); },
                            "❌ Отмена"
                        }
                    }
                }
            }
        }
    }
}

// ── Styles ────────────────────────────────────────────────────

fn input_style() -> &'static str {
    "padding:10px 12px;background:#0f0f1a;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:14px;"
}
fn submit_btn_style() -> &'static str {
    "padding:12px;background:#39ff14;color:#000;border:none;border-radius:4px;font-weight:700;font-size:14px;cursor:pointer;margin-top:4px;"
}
fn submit_btn_disabled_style() -> &'static str {
    "padding:12px;background:#666;color:#999;border:none;border-radius:4px;font-weight:700;font-size:14px;cursor:not-allowed;margin-top:4px;"
}
fn upload_btn_style() -> &'static str {
    "padding:10px 12px;background:#2a2a4a;color:#6699ff;border:1px dashed #6699ff;border-radius:4px;font-size:13px;cursor:pointer;white-space:nowrap;"
}
fn list_title_style() -> &'static str {
    "color:#888;font-size:13px;margin:16px 0 8px;text-transform:uppercase;letter-spacing:1px;"
}
fn en_section_style() -> &'static str {
    "padding:6px 0 2px;color:#6699ff;font-size:12px;font-weight:600;border-top:1px solid #2a2a4a;margin-top:4px;"
}
fn cancel_btn_style() -> &'static str {
    "padding:10px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-weight:600;font-size:13px;cursor:pointer;margin-top:4px;"
}
fn secondary_btn_style() -> &'static str {
    "padding:10px;background:#1a1a2e;color:#888;border:1px solid #2a2a4a;border-radius:4px;font-weight:600;font-size:13px;cursor:pointer;margin-top:8px;"
}
fn danger_btn_style() -> &'static str {
    "padding:10px;background:#ff4757;color:#fff;border:none;border-radius:4px;font-weight:600;font-size:13px;cursor:pointer;margin-top:4px;"
}
fn edit_card_style() -> &'static str {
    "background:#1a1a2e;border:1px solid #6699ff;border-radius:8px;padding:12px;display:flex;flex-direction:column;gap:8px;"
}
fn edit_header_style() -> &'static str {
    "color:#6699ff;font-size:13px;font-weight:700;text-transform:uppercase;letter-spacing:1px;"
}
fn textarea_style() -> &'static str {
    "padding:10px 12px;background:#0f0f1a;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:14px;min-height:80px;resize:vertical;font-family:inherit;"
}

// ── Events admin tab ─────────────────────────────────────────────

#[component]
fn EventsTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut cache: Signal<Vec<AdminEvent>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut reload = use_signal(|| 0u32);
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);
    let mut search_query = use_signal(String::new);

    let mut title = use_signal(String::new);
    let mut title_en = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut description_en = use_signal(String::new);
    let mut starts_at = use_signal(String::new);
    let mut ends_at = use_signal(String::new);
    let mut location_text = use_signal(String::new);
    let mut image_url = use_signal(String::new);
    let mut video_url = use_signal(String::new);
    let mut photos: Signal<Vec<String>> = use_signal(Vec::new);
    let mut max_seats = use_signal(String::new);
    let mut price_baht = use_signal(String::new);
    let mut price_stars = use_signal(String::new);
    let mut is_public = use_signal(|| true);
    let mut editing_id: Signal<Option<String>> = use_signal(|| None);
    let mut delete_target_id: Signal<Option<String>> = use_signal(|| None);
    let mut submitting = use_signal(|| false);

    let mut booking_target: Signal<Option<AdminEvent>> = use_signal(|| None);
    let mut bookings: Signal<Vec<EventBooking>> = use_signal(Vec::new);
    let mut bookings_loading = use_signal(|| false);
    // Result of the "send guest list to Telegram" action, shown under the button.
    let mut send_list_status = use_signal(String::new);

    fn admin_event_headers(init: String, telegram_id: i64) -> Vec<(&'static str, String)> {
        vec![
            ("X-Telegram-Init-Data", init),
            ("X-Admin-Token", admin_token()),
            ("X-Admin-Telegram-Id", telegram_id.to_string()),
        ]
    }

    let clear_form = use_callback(move |()| {
        title.set(String::new());
        title_en.set(String::new());
        description.set(String::new());
        description_en.set(String::new());
        starts_at.set(String::new());
        ends_at.set(String::new());
        location_text.set(String::new());
        image_url.set(String::new());
        video_url.set(String::new());
        photos.set(Vec::new());
        max_seats.set(String::new());
        price_baht.set(String::new());
        price_stars.set(String::new());
        is_public.set(true);
        editing_id.set(None);
    });

    let _ = use_resource(move || {
        let _ = reload.read();
        let init = init_data.read().clone();
        async move {
            let url = format!("{}/api/admin/events", api_base_url());
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&url)
                .headers(admin_event_headers(init, telegram_id))
                .send()
                .await
            {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    let events = data
                        .get("events")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                .collect::<Vec<AdminEvent>>()
                        })
                        .unwrap_or_default();
                    cache.set(events);
                }
            }
            loading.set(false);
            Some(())
        }
    });

    let load_bookings = use_callback(move |event_id: String| {
        bookings_loading.set(true);
        let init = init_data.read().clone();
        spawn(async move {
            let url = format!(
                "{}/api/admin/events/{}/bookings",
                api_base_url(),
                urlencoding::encode(&event_id)
            );
            let mut list = Vec::new();
            if let Ok(resp) = HTTP_CLIENT
                .clone()
                .get(&url)
                .headers(admin_event_headers(init, telegram_id))
                .send()
                .await
            {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    list = data
                        .get("bookings")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                .collect::<Vec<EventBooking>>()
                        })
                        .unwrap_or_default();
                }
            }
            bookings.set(list);
            bookings_loading.set(false);
        });
    });

    let create_or_update = move |_| {
        if title.read().trim().is_empty() || starts_at.read().trim().is_empty() {
            push_toast(
                toasts,
                "Название и дата обязательны".into(),
                ToastKind::Error,
            );
            return;
        }
        // Build the timestamps BEFORE `submitting` is latched: an early return
        // past that point would leave the Save button disabled for good.
        //
        // Admin input is interpreted as Asia/Bangkok (UTC+7), not browser local
        // time. The conversion is shared and tested (`trios::calendar`) rather
        // than a `format!` here — a datetime-local value carrying seconds used
        // to be concatenated into an unparseable timestamp and rejected by the
        // server with an error nobody could see.
        const SHOP_OFFSET: &str = "+07:00";
        let starts_at_api =
            match crate::trios::calendar::datetime_local_to_rfc3339(&starts_at.read(), SHOP_OFFSET)
            {
                Some(s) => s,
                None => {
                    push_toast(
                        toasts,
                        format!("Не понял дату начала: «{}»", starts_at.read()),
                        ToastKind::Error,
                    );
                    return;
                }
            };
        let ends_at_raw = ends_at.read().trim().to_string();
        let ends_at_api = if ends_at_raw.is_empty() {
            None
        } else {
            match crate::trios::calendar::datetime_local_to_rfc3339(&ends_at_raw, SHOP_OFFSET) {
                Some(s) => Some(s),
                None => {
                    push_toast(
                        toasts,
                        format!("Не понял дату окончания: «{ends_at_raw}»"),
                        ToastKind::Error,
                    );
                    return;
                }
            }
        };

        submitting.set(true);
        let init = init_data.read().clone();
        // "Add" opens the modal with editing_id = Some(""), so treat an empty id as create.
        let id_opt = editing_id.read().clone().filter(|id| !id.is_empty());
        // Snapshot form values for optimistic cache update after API success.
        let title_clone = title.read().clone();
        let title_en_clone = title_en.read().clone();
        let description_clone = description.read().clone();
        let description_en_clone = description_en.read().clone();
        let location_text_clone = location_text.read().clone();
        let image_url_clone = image_url.read().clone();
        let video_url_clone = video_url.read().clone();
        let photos_clone = photos.read().clone();
        let max_seats_clone = max_seats.read().clone();
        let price_baht_clone = price_baht.read().clone();
        let price_stars_clone = price_stars.read().clone();
        let is_public_clone = *is_public.read();
        let body = json!({
            "title": title.read().clone(),
            "title_en": if title_en.read().trim().is_empty() { serde_json::Value::Null } else { title_en.read().clone().into() },
            "description": if description.read().trim().is_empty() { serde_json::Value::Null } else { description.read().clone().into() },
            "description_en": if description_en.read().trim().is_empty() { serde_json::Value::Null } else { description_en.read().clone().into() },
            // Reuse the values validated above — building them a second time
            // here let the request and the optimistic cache entry drift apart.
            "starts_at": starts_at_api.clone(),
            "ends_at": match ends_at_api.clone() { Some(s) => serde_json::Value::String(s), None => serde_json::Value::Null },
            "location_text": if location_text.read().trim().is_empty() { serde_json::Value::Null } else { location_text.read().clone().into() },
            "image_url": if image_url.read().trim().is_empty() { serde_json::Value::Null } else { image_url.read().clone().into() },
            "video_url": if video_url.read().trim().is_empty() { serde_json::Value::Null } else { video_url.read().clone().into() },
            "photos": serde_json::Value::Array(photos.read().iter().map(|s| serde_json::Value::String(s.clone())).collect()),
            "max_seats": if max_seats.read().trim().is_empty() { serde_json::Value::Null } else { max_seats.read().parse::<i32>().unwrap_or(0).into() },
            "price_baht": if price_baht.read().trim().is_empty() { serde_json::Value::Null } else { price_baht.read().parse::<f64>().unwrap_or(0.0).into() },
            "price_stars": if price_stars.read().trim().is_empty() { serde_json::Value::Null } else { price_stars.read().parse::<i64>().unwrap_or(0).into() },
            "is_public": *is_public.read(),
        });
        spawn(async move {
            let base = api_base_url();
            let res = if let Some(ref id) = id_opt {
                let url = format!("{}/api/admin/events/{}", base, urlencoding::encode(id));
                HTTP_CLIENT
                    .clone()
                    .put(&url)
                    .headers(admin_event_headers(init, telegram_id))
                    .json(&body)
                    .send()
                    .await
            } else {
                let url = format!("{}/api/admin/events", base);
                HTTP_CLIENT
                    .clone()
                    .post(&url)
                    .headers(admin_event_headers(init, telegram_id))
                    .json(&body)
                    .send()
                    .await
            };
            submitting.set(false);
            match res {
                Ok(r) if r.status().is_success() => {
                    let is_create = id_opt.is_none();
                    let returned_id = if is_create {
                        r.json::<serde_json::Value>()
                            .await
                            .ok()
                            .and_then(|v| v.get("id").and_then(|i| i.as_str()).map(String::from))
                    } else {
                        id_opt.clone()
                    };
                    clear_form.call(());
                    // Optimistic refresh: rebuild the cache entry from form values
                    // so the event appears immediately without waiting for use_resource.
                    cache.with_mut(|list| {
                        let empty_or_none = |s: &str| {
                            if s.trim().is_empty() {
                                None
                            } else {
                                Some(s.to_string())
                            }
                        };
                        let new_event = AdminEvent {
                            id: returned_id.unwrap_or_default(),
                            title: title_clone.clone(),
                            title_en: empty_or_none(&title_en_clone),
                            description: empty_or_none(&description_clone),
                            description_en: empty_or_none(&description_en_clone),
                            starts_at: starts_at_api.clone(),
                            ends_at: ends_at_api.clone().filter(|s| !s.is_empty()),
                            location_text: empty_or_none(&location_text_clone),
                            image_url: empty_or_none(&image_url_clone),
                            video_url: empty_or_none(&video_url_clone),
                            photos: photos_clone.clone(),
                            max_seats: max_seats_clone.parse::<i32>().ok(),
                            price_baht: price_baht_clone.parse::<f64>().ok(),
                            price_stars: price_stars_clone.parse::<i64>().ok(),
                            is_public: is_public_clone,
                            seats_taken: 0,
                            seats_available: max_seats_clone.parse::<i32>().ok(),
                            created_at: None,
                        };
                        if let Some(existing) = id_opt {
                            if let Some(idx) = list.iter().position(|e| e.id == existing) {
                                list[idx] = new_event;
                                return;
                            }
                        }
                        list.push(new_event);
                    });
                    let next = *reload.read() + 1;
                    reload.set(next);
                    push_toast(toasts, "Сохранено".into(), ToastKind::Success);
                }
                // The server already explains itself — `validate_event_request`
                // returns messages like "starts_at не ISO-8601" or
                // "photos[2] невалиден". Collapsing every failure into a bare
                // "Ошибка сохранения" threw that away and left a failed save
                // undiagnosable from either end. Show status + reason.
                Ok(r) => {
                    let status = r.status().as_u16();
                    let body = r.text().await.unwrap_or_default();
                    let reason = body.trim();
                    let msg = if reason.is_empty() {
                        format!("Ошибка сохранения ({status})")
                    } else {
                        // Bodies are short validation strings; cap anyway so a
                        // stray HTML error page cannot fill the screen.
                        let short: String = reason.chars().take(300).collect();
                        format!("Ошибка сохранения ({status}): {short}")
                    };
                    push_toast(toasts, msg, ToastKind::Error);
                }
                Err(e) => {
                    push_toast(
                        toasts,
                        format!("Сеть недоступна, событие не сохранено: {e}"),
                        ToastKind::Error,
                    );
                }
            }
        });
    };

    let confirm_delete = move |_| {
        let id = delete_target_id.read().clone();
        if let Some(id) = id {
            let init = init_data.read().clone();
            delete_target_id.set(None);
            spawn(async move {
                let url = format!(
                    "{}/api/admin/events/{}",
                    api_base_url(),
                    urlencoding::encode(&id)
                );
                match HTTP_CLIENT
                    .clone()
                    .delete(&url)
                    .headers(admin_event_headers(init, telegram_id))
                    .send()
                    .await
                {
                    Ok(r) if r.status().is_success() => {
                        let next = *reload.read() + 1;
                        reload.set(next);
                        push_toast(toasts, "Удалено".into(), ToastKind::Success);
                    }
                    _ => {
                        push_toast(toasts, "Ошибка удаления".into(), ToastKind::Error);
                    }
                }
            });
        }
    };

    let filtered: Vec<AdminEvent> = {
        let q = search_query.read().to_lowercase();
        cache
            .read()
            .iter()
            .filter(|e| {
                e.title.to_lowercase().contains(&q)
                    || e.location_text
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q)
            })
            .cloned()
            .collect()
    };

    let mut open_bookings = move |ev: AdminEvent| {
        load_bookings.call(ev.id.clone());
        booking_target.set(Some(ev));
    };

    let mut start_edit = move |ev: AdminEvent| {
        title.set(ev.title.clone());
        title_en.set(ev.title_en.clone().unwrap_or_default());
        description.set(ev.description.clone().unwrap_or_default());
        description_en.set(ev.description_en.clone().unwrap_or_default());
        starts_at.set(
            parse_event_start(&ev.starts_at)
                .map(|dt| dt.format("%Y-%m-%dT%H:%M").to_string())
                .unwrap_or_default(),
        );
        ends_at.set(
            ev.ends_at
                .as_deref()
                .and_then(parse_event_start)
                .map(|dt| dt.format("%Y-%m-%dT%H:%M").to_string())
                .unwrap_or_default(),
        );
        location_text.set(ev.location_text.clone().unwrap_or_default());
        image_url.set(ev.image_url.clone().unwrap_or_default());
        video_url.set(ev.video_url.clone().unwrap_or_default());
        photos.set(ev.photos.clone());
        max_seats.set(ev.max_seats.map(|v| v.to_string()).unwrap_or_default());
        price_baht.set(ev.price_baht.map(|v| format!("{}", v)).unwrap_or_default());
        price_stars.set(ev.price_stars.map(|v| v.to_string()).unwrap_or_default());
        is_public.set(ev.is_public);
        editing_id.set(Some(ev.id.clone()));
    };

    let booking_rows = {
        let list = bookings.read().clone();
        if list.is_empty() {
            rsx! { p { style: "color:#888;font-size:13px;", "Бронирований пока нет" } }
        } else {
            rsx! {
                div { style: "display:flex;flex-direction:column;gap:8px;",
                    {
                        list.into_iter().map(move |b| {
                            let cancel_id = b.id.clone();
                            let event_id = b.event_id.clone();
                            // Show a handle the owner can tap to message the person,
                            // not a bare telegram_id that identifies nobody.
                            let who = crate::trios::attendees::attendee_link(
                                b.username.as_deref(),
                                b.first_name.as_deref(),
                                b.telegram_id,
                            );
                            let who_color = if who.reachable_by_handle { "#39ff14" } else { "#8b8b9e" };
                            rsx! {
                                div { key: "{b.id}", style: "display:flex;justify-content:space-between;align-items:center;gap:8px;background:#1a1a2e;padding:8px;border:1px solid #2a2a4a;",
                                    div { style: "font-size:13px;min-width:0;",
                                        // Only a real @handle gets a link. `tg://user?id=…` renders
                                        // as an "Open link?" prompt in the Telegram webview and then
                                        // does nothing, so a link there would be a lie — use the
                                        // "send to Telegram" button below, where the same id becomes
                                        // a working inline mention.
                                        if who.reachable_by_handle {
                                            a {
                                                href: "{who.url}",
                                                target: "_blank",
                                                style: "color:{who_color};font-weight:700;text-decoration:underline;word-break:break-all;",
                                                "{who.label}"
                                            }
                                        } else {
                                            span { style: "color:{who_color};font-weight:700;word-break:break-all;", "{who.label}" }
                                        }
                                        span { style: "color:#888;margin-left:8px;", "{b.status}" }
                                        span { style: "color:#39ff14;margin-left:8px;", "+{b.seats}" }
                                    }
                                    if b.status == "confirmed" {
                                        button {
                                            style: "padding:6px 10px;background:#ff4757;color:#fff;border:none;border-radius:4px;font-size:12px;cursor:pointer;",
                                            onclick: move |_| {
                                                let init = init_data.read().clone();
                                                let cid = cancel_id.clone();
                                                let eid = event_id.clone();
                                                spawn(async move {
                                                    let url = format!("{}/api/admin/events/{}/bookings/{}/cancel", api_base_url(), urlencoding::encode(&eid), urlencoding::encode(&cid));
                                                    match HTTP_CLIENT.clone().put(&url).headers(admin_event_headers(init, telegram_id)).send().await {
                                                        Ok(r) if r.status().is_success() => {
                                                            load_bookings.call(eid.clone());
                                                            push_toast(toasts, "Бронь отменена".into(), ToastKind::Success);
                                                        }
                                                        _ => {
                                                            push_toast(toasts, "Ошибка отмены".into(), ToastKind::Error);
                                                        }
                                                    }
                                                });
                                            },
                                            "Отменить"
                                        }
                                    }
                                }
                            }
                        })
                    }
                }
            }
        }
    };

    let event_rows = {
        let list = filtered.clone();
        if list.is_empty() {
            rsx! { p { style: "color:#888;font-size:13px;", "Нет событий" } }
        } else {
            rsx! {
                div { style: "display:flex;flex-direction:column;gap:8px;",
                    {
                        list.into_iter().map(move |ev| {
                            let ev_id = ev.id.clone();
                            let ev_id_for_key = ev_id.clone();
                            let ev_clone = ev.clone();
                            let ev_clone2 = ev.clone();
                            let date_label = parse_event_start(&ev.starts_at).map(|s| s.format("%d %b %Y %H:%M").to_string()).unwrap_or_else(|| ev.starts_at.clone());
                            let cap_label = ev.max_seats.map(|cap| format!("{}/{}", ev.seats_taken, cap)).unwrap_or_else(|| "∞".to_string());
                            let price_label = if let Some(s) = ev.price_stars.filter(|s| *s > 0) {
                                format!("{} ⭐", s)
                            } else if let Some(p) = ev.price_baht.filter(|p| *p > 0.0) {
                                format!("{:.0} ฿", p)
                            } else {
                                "бесплатно".to_string()
                            };
                            let public_label = if ev.is_public { "публично" } else { "скрыто" };
                            rsx! {
                                div { key: "{ev_id_for_key}", style: "background:#1a1a2e;border:1px solid #2a2a4a;border-radius:6px;padding:10px;display:flex;justify-content:space-between;align-items:center;",
                                    div { style: "display:flex;flex-direction:column;gap:2px;",
                                        span { style: "color:#e8e8e8;font-weight:700;font-size:14px;", "{ev.title}" }
                                        span { style: "color:#888;font-size:12px;", "{date_label} • {cap_label} • {price_label} • {public_label}" }
                                        if let Some(ref loc) = ev.location_text {
                                            span { style: "color:#6699ff;font-size:12px;", "📍 {loc}" }
                                        }
                                    }
                                    div { style: "display:flex;gap:6px;",
                                        button {
                                            style: "padding:6px 10px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:12px;cursor:pointer;",
                                            onclick: move |_| open_bookings(ev_clone.clone()),
                                            "Брони"
                                        }
                                        button {
                                            style: "padding:6px 10px;background:#2a2a4a;color:#39ff14;border:none;border-radius:4px;font-size:12px;cursor:pointer;",
                                            "aria-label": "Редактировать",
                                            onclick: move |_| start_edit(ev_clone2.clone()),
                                            "✎"
                                        }
                                        button {
                                            style: "padding:6px 10px;background:#2a2a4a;color:#ff4757;border:none;border-radius:4px;font-size:12px;cursor:pointer;",
                                            "aria-label": "Удалить",
                                            onclick: move |_| delete_target_id.set(Some(ev_id.clone())),
                                            "🗑"
                                        }
                                    }
                                }
                            }
                        })
                    }
                }
            }
        }
    };

    let modal_open = editing_id.read().is_some();

    if let Some((msg, kind)) = take_global_toast() {
        push_toast(toasts, msg, kind);
    }

    rsx! {
        div { style: "padding:12px;display:flex;flex-direction:column;gap:12px;",
            { render_toasts(toasts) }
            div { style: "display:flex;gap:8px;",
                input {
                    style: input_style(),
                    r#type: "text",
                    placeholder: "Поиск событий...",
                    value: "{search_query}",
                    oninput: move |evt| search_query.set(evt.value()),
                }
                button {
                    style: "padding:8px 16px;background:#39ff14;color:#000;border:none;border-radius:4px;font-weight:700;font-size:13px;cursor:pointer;",
                    onclick: move |_| {
                        clear_form.call(());
                        editing_id.set(Some(String::new()));
                    },
                    "➕ Событие"
                }
            }
            if *loading.read() {
                div { style: "display:flex;flex-direction:column;gap:8px;",
                    for _ in 0..3 {
                        div { style: "background:#1a1a2e;padding:10px;border-radius:6px;", Skeleton { shape: SkeletonShape::Text, width: Some("80%".into()) } }
                    }
                }
            } else {
                { event_rows }
            }
        }

        Modal {
            open: modal_open,
            title: if editing_id.read().as_deref() == Some(&String::new()) { Some("Новое событие".to_string()) } else { Some("Редактировать событие".to_string()) },
            show_close: true,
            on_close: move |_| {
                clear_form.call(());
                editing_id.set(None);
            },
            div { style: "display:flex;flex-direction:column;gap:10px;",
                input { style: input_style(), r#type: "text", placeholder: "Название", value: "{title}", oninput: move |evt| title.set(evt.value()) }
                input { style: input_style(), r#type: "text", placeholder: "Название (EN)", value: "{title_en}", oninput: move |evt| title_en.set(evt.value()) }
                textarea { style: textarea_style(), placeholder: "Описание", value: "{description}", oninput: move |evt| description.set(evt.value()) }
                textarea { style: textarea_style(), placeholder: "Описание (EN)", value: "{description_en}", oninput: move |evt| description_en.set(evt.value()) }
                label { style: "font-size:12px;color:#888;", "Начало (дата и время)" }
                input { style: input_style(), r#type: "datetime-local", value: "{starts_at}", oninput: move |evt| starts_at.set(evt.value()) }
                label { style: "font-size:12px;color:#888;", "Окончание (необязательно)" }
                input { style: input_style(), r#type: "datetime-local", value: "{ends_at}", oninput: move |evt| ends_at.set(evt.value()) }
                input { style: input_style(), r#type: "text", placeholder: "Место", value: "{location_text}", oninput: move |evt| location_text.set(evt.value()) }
                div { style: "margin-bottom:4px;",
                    div { style: "font-size:12px;color:#888;margin-bottom:4px;", "Изображение (обложка)" }
                    ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
                }
                div { style: "margin-bottom:4px;",
                    div { style: "font-size:12px;color:#888;margin-bottom:4px;", "Видео" }
                    VideoUpload { video_url: video_url.read().clone(), on_change: move |url: String| video_url.set(url) }
                }
                div { style: "margin-bottom:4px;",
                    div { style: "font-size:12px;color:#888;margin-bottom:4px;", "Галерея фото" }
                    PhotoGalleryEditor { photos: photos, on_change: move |next: Vec<String>| photos.set(next) }
                }
                div { style: "display:flex;gap:10px;",
                    input { style: input_style(), r#type: "number", placeholder: "Мест", value: "{max_seats}", oninput: move |evt| max_seats.set(evt.value()) }
                    input { style: input_style(), r#type: "number", placeholder: "Цена (бат)", value: "{price_baht}", oninput: move |evt| price_baht.set(evt.value()) }
                    input { style: input_style(), r#type: "number", placeholder: "Цена (Stars)", value: "{price_stars}", oninput: move |evt| price_stars.set(evt.value()) }
                }
                label { style: "display:flex;align-items:center;gap:8px;font-size:13px;color:#e8e8e8;",
                    input { r#type: "checkbox", checked: *is_public.read(), onchange: move |evt| is_public.set(evt.checked()) }
                    "Публичное событие"
                }
                div { style: "display:flex;gap:8px;justify-content:flex-end;",
                    button { style: cancel_btn_style(), onclick: move |_| { clear_form.call(()); editing_id.set(None); }, "Отмена" }
                    button {
                        style: submit_btn_style(),
                        disabled: *submitting.read(),
                        onclick: create_or_update,
                        if *submitting.read() { "⏳" } else { "Сохранить" }
                    }
                }
            }
        }

        if let Some(ref ev) = booking_target.read().clone() {
            Modal {
                open: true,
                title: Some(format!("Брони: {}", ev.title)),
                show_close: true,
                on_close: move |_| booking_target.set(None),
                div { style: "display:flex;flex-direction:column;gap:10px;",
                    if *bookings_loading.read() {
                        Skeleton { shape: SkeletonShape::Text, width: Some("60%".into()) }
                    } else {
                        { booking_rows }
                        // The only way to reach a guest who has no @username:
                        // in a bot message `tg://user?id=…` is a real inline
                        // mention and opens the profile, whereas in this
                        // webview it does nothing.
                        {
                            let send_event_id = ev.id.clone();
                            rsx! {
                                button {
                                    style: "width:100%;padding:10px;background:#39ff14;color:#000;border:3px solid #2d9e0f;font-size:13px;font-weight:700;cursor:pointer;box-shadow:2px 2px 0 #000;",
                                    onclick: move |_| {
                                        let init = init_data.read().clone();
                                        let eid = send_event_id.clone();
                                        let mut status = send_list_status;
                                        spawn(async move {
                                            status.set("Отправляю…".to_string());
                                            let url = format!(
                                                "{}/api/admin/events/{}/bookings/send",
                                                api_base_url(), eid
                                            );
                                            let res = HTTP_CLIENT.clone().post(&url)
                                                .headers(admin_event_headers(init, telegram_id))
                                                .send().await;
                                            match res {
                                                Ok(r) if r.status().is_success() =>
                                                    status.set("✅ Список отправлен вам в Telegram".to_string()),
                                                Ok(r) =>
                                                    status.set(format!("Не отправилось ({})", r.status().as_u16())),
                                                Err(_) => status.set("Не отправилось (нет связи)".to_string()),
                                            }
                                        });
                                    },
                                    "📨 Прислать список в Telegram"
                                }
                            }
                        }
                        if !send_list_status.read().is_empty() {
                            p { style: "font-size:12px;color:#8b8b9e;margin:0;", "{send_list_status}" }
                        }
                    }
                }
            }
        }

        if delete_target_id.read().is_some() {
            Modal {
                open: true,
                title: Some("Удалить событие?".to_string()),
                show_close: true,
                on_close: move |_| delete_target_id.set(None),
                div { style: "display:flex;flex-direction:column;gap:12px;",
                    p { style: "color:#e8e8e8;font-size:14px;", "Это также отменит все бронирования. Продолжить?" }
                    div { style: "display:flex;gap:8px;justify-content:flex-end;",
                        button { style: cancel_btn_style(), onclick: move |_| delete_target_id.set(None), "Отмена" }
                        button { style: danger_btn_style(), onclick: confirm_delete, "Удалить" }
                    }
                }
            }
        }
    }
}

#[component]
fn PhotoGalleryEditor(
    photos: Signal<Vec<String>>,
    on_change: EventHandler<Vec<String>>,
) -> Element {
    let mut add_url = use_signal(String::new);
    rsx! {
        div { style: "display:flex;flex-direction:column;gap:8px;",
            div { style: "display:flex;gap:6px;align-items:center;",
                input { style: "flex:1;{input_style()}", placeholder: "URL фото", value: "{add_url}",
                    oninput: move |e| add_url.set(e.value()) }
                button { style: upload_btn_style(),
                    onclick: move |_| {
                        let url = add_url.read().trim().to_string();
                        if !url.is_empty() {
                            let mut next = photos.read().clone();
                            next.push(url);
                            on_change.call(next);
                            add_url.set(String::new());
                        }
                    },
                    "➕ Добавить"
                }
            }
            button { style: secondary_btn_style(),
                onclick: move |_| {
                    spawn(async move {
                        match upload_image().await {
                            Ok(Some(url)) => {
                                let mut next = photos.read().clone();
                                next.push(url);
                                on_change.call(next);
                            }
                            Ok(None) => {}
                            Err(msg) => { push_toast_global(msg, ToastKind::Error); }
                        }
                    });
                },
                "📷 Загрузить фото"
            }
            if !photos.read().is_empty() {
                div { style: "display:flex;flex-wrap:wrap;gap:6px;",
                    {
                        let list = photos.read().clone();
                        list.into_iter().enumerate().map(move |(i, url)| {
                            let url_for_img = url.clone();
                            let url_for_open = url.clone();
                            let url_for_del = url.clone();
                            let url_for_up = url.clone();
                            rsx! {
                                div { key: "{i}_{url_for_img}", style: "position:relative;width:64px;height:64px;",
                                    if url_for_img.starts_with("http://") || url_for_img.starts_with("https://") || (url_for_img.starts_with("/") && !url_for_img.starts_with("//")) {
                                        img { src: "{url_for_img}", alt: "", style: "width:64px;height:64px;object-fit:cover;border-radius:6px;border:1px solid #2a2a4a;cursor:pointer;",
                                            onclick: move |_| {
                                                let u = url_for_open.clone();
                                                let _ = web_sys::window().and_then(|w| w.open_with_url_and_target(&u, "_blank").ok());
                                            }
                                        }
                                    } else {
                                        div { style: "width:64px;height:64px;background:#1a1a2e;border-radius:6px;border:1px solid #2a2a4a;display:flex;align-items:center;justify-content:center;font-size:11px;color:#888;", "📷" }
                                    }
                                    div { style: "position:absolute;top:-4px;right:-4px;display:flex;gap:2px;",
                                        button { style: "width:18px;height:18px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:50%;font-size:10px;cursor:pointer;padding:0;",
                                            onclick: move |_| {
                                                let u = url_for_up.clone();
                                                let mut next = photos.read().clone();
                                                if let Some(pos) = next.iter().position(|x| x == &u) {
                                                    if pos > 0 {
                                                        next.swap(pos, pos - 1);
                                                        on_change.call(next);
                                                    }
                                                }
                                            },
                                            "↑"
                                        }
                                        button {
                                            style: "width:18px;height:18px;background:#ff4757;color:#fff;border:none;border-radius:50%;font-size:10px;cursor:pointer;padding:0;",
                                            "aria-label": "Удалить фото",
                                            onclick: move |_| {
                                                let u = url_for_del.clone();
                                                let mut next = photos.read().clone();
                                                next.retain(|x| x != &u);
                                                on_change.call(next);
                                            },
                                            "✕"
                                        }
                                    }
                                }
                            }
                        })
                    }
                }
            }
        }
    }
}

/// Send a toast from a component that doesn't own the toasts signal.
/// Uses a small module-level mutex so PhotoGalleryEditor can surface
/// upload errors without threading the full ToastContainer signal down.
/// The owner loop should call take_global_toast() each render.
static GLOBAL_TOAST: std::sync::OnceLock<std::sync::Mutex<Option<(String, ToastKind)>>> =
    std::sync::OnceLock::new();

fn push_toast_global(message: String, kind: ToastKind) {
    if let Ok(mut guard) = GLOBAL_TOAST.get_or_init(Default::default).lock() {
        *guard = Some((message, kind));
    }
}

fn take_global_toast() -> Option<(String, ToastKind)> {
    GLOBAL_TOAST
        .get_or_init(Default::default)
        .lock()
        .ok()
        .and_then(|mut guard| guard.take())
}

// ── Telegram Broadcast tab ───────────────────────────────────

#[derive(Clone)]
struct BroadcastPickerProduct {
    id: String,
    name: String,
    image_url: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BroadcastCatalog {
    Strains,
    Accessories,
    Tea,
    Sets,
}

impl BroadcastCatalog {
    fn from_value(s: &str) -> Option<Self> {
        match s {
            "strain" => Some(Self::Strains),
            "accessory" => Some(Self::Accessories),
            "tea" => Some(Self::Tea),
            "set" => Some(Self::Sets),
            _ => None,
        }
    }
    fn value(self) -> &'static str {
        match self {
            Self::Strains => "strain",
            Self::Accessories => "accessory",
            Self::Tea => "tea",
            Self::Sets => "set",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Strains => "🌿 Штаммы",
            Self::Accessories => "💨 Аксессуары",
            Self::Tea => "🍵 Чай",
            Self::Sets => "📦 Наборы",
        }
    }
    fn product_kind(self) -> ProductKind {
        match self {
            Self::Strains => ProductKind::Strain,
            Self::Accessories => ProductKind::Accessory,
            Self::Tea => ProductKind::Tea,
            Self::Sets => ProductKind::Set,
        }
    }
}

async fn load_broadcast_products(
    catalog: BroadcastCatalog,
) -> Result<Vec<BroadcastPickerProduct>, String> {
    let base = api_base_url();
    let path = match catalog {
        BroadcastCatalog::Strains => "/api/strains",
        BroadcastCatalog::Accessories => "/api/accessories",
        BroadcastCatalog::Tea => "/api/tea-products",
        BroadcastCatalog::Sets => "/api/sets",
    };
    let url = format!("{base}{path}");
    let resp = HTTP_CLIENT
        .clone()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;
    if !resp.status().is_success() {
        return Err("Ошибка загрузки каталога".into());
    }
    Ok(match catalog {
        BroadcastCatalog::Strains => {
            #[derive(Deserialize)]
            struct R {
                strains: Vec<Strain>,
            }
            resp.json::<R>()
                .await
                .map_err(|e| format!("json: {e}"))?
                .strains
                .into_iter()
                .map(|s| BroadcastPickerProduct {
                    id: s.id,
                    name: s.name,
                    image_url: s.image_url,
                })
                .collect()
        }
        BroadcastCatalog::Accessories => {
            #[derive(Deserialize)]
            struct R {
                accessories: Vec<Accessory>,
            }
            resp.json::<R>()
                .await
                .map_err(|e| format!("json: {e}"))?
                .accessories
                .into_iter()
                .map(|a| BroadcastPickerProduct {
                    id: a.id,
                    name: a.name,
                    image_url: a.image_url,
                })
                .collect()
        }
        BroadcastCatalog::Tea => {
            #[derive(Deserialize)]
            struct R {
                #[serde(default)]
                products: Vec<TeaProduct>,
                #[serde(default)]
                tea_products: Vec<TeaProduct>,
            }
            let r = resp.json::<R>().await.map_err(|e| format!("json: {e}"))?;
            let items = if !r.products.is_empty() {
                r.products
            } else {
                r.tea_products
            };
            items
                .into_iter()
                .map(|t| BroadcastPickerProduct {
                    id: t.id,
                    name: t.name,
                    image_url: t.image_url,
                })
                .collect()
        }
        BroadcastCatalog::Sets => {
            #[derive(Deserialize)]
            struct R {
                sets: Vec<Set>,
            }
            resp.json::<R>()
                .await
                .map_err(|e| format!("json: {e}"))?
                .sets
                .into_iter()
                .map(|s| BroadcastPickerProduct {
                    id: s.id,
                    name: s.name,
                    image_url: String::new(),
                })
                .collect()
        }
    })
}

#[component]
fn BroadcastTab() -> Element {
    let lang = crate::ui::lang::current_lang();
    let _init_data = use_telegram_init_data();
    let _token = admin_token();
    let mut text = use_signal(|| String::new());
    let mut photo_url = use_signal(|| String::new());
    let mut catalog = use_signal(|| "none".to_string());
    let mut selected_product = use_signal(|| None::<BroadcastPickerProduct>);
    let mut button_text = use_signal(|| String::new());
    let mut sending = use_signal(|| false);
    let mut sent = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let mut result = use_signal(|| Option::<serde_json::Value>::None);

    let products = use_resource(move || {
        let cat_val = catalog.read().clone();
        async move {
            match BroadcastCatalog::from_value(&cat_val) {
                Some(c) => load_broadcast_products(c).await,
                None => Ok(vec![]),
            }
        }
    });

    let select_style = "width:100%;padding:10px 12px;background:#0f0f1a;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:14px;cursor:pointer;";

    rsx! {
        div { style: "padding: 16px;",
            div { style: "font-size: 18px; font-weight: 800; color: #39ff14; margin-bottom: 12px; text-shadow: 2px 2px 0 #000;",
                {t(lang, T_BROADCAST)}
            }
            div { style: "font-size: 13px; color: #8b8b9e; margin-bottom: 12px;",
                " Рассылка с фото и кнопкой на товар всем пользователям бота. "
            }

            // Text
            div { style: "margin-bottom: 12px;",
                div { style: "font-size: 13px; color: #e8e8e8; margin-bottom: 6px;", {t(lang, T_BROADCAST_TEXT)} }
                textarea {
                    style: "width: 100%; min-height: 100px; background: #1a1a2e; border: 2px solid #2a2a4a; color: #e8e8e8; padding: 10px; font-size: 14px; resize: vertical;",
                    value: "{text()}",
                    oninput: move |e: Event<FormData>| {
                        text.set(e.value().clone());
                        error.set(None);
                        result.set(None);
                    },
                }
            }

            // Photo
            div { style: "margin-bottom: 12px;",
                div { style: "font-size: 13px; color: #e8e8e8; margin-bottom: 6px;", {t(lang, T_BROADCAST_PHOTO)} }
                ImageUpload { image_url: photo_url.read().clone(), on_change: move |url: String| photo_url.set(url) }
                div { style: "font-size: 12px; color: #8b8b9e; margin-top: 4px;", {t(lang, T_BROADCAST_PHOTO_HINT)} }
            }

            // Product catalog picker
            div { style: "margin-bottom: 12px;",
                div { style: "font-size: 13px; color: #e8e8e8; margin-bottom: 6px;", {t(lang, T_BROADCAST_PRODUCT)} }
                div { style: "font-size: 12px; color: #8b8b9e; margin-bottom: 6px;", {t(lang, T_BROADCAST_SELECT_CATALOG)} }
                select {
                    style: "{select_style}",
                    value: "{catalog()}",
                    onchange: move |e: Event<FormData>| {
                        catalog.set(e.value().clone());
                        selected_product.set(None);
                        error.set(None);
                        result.set(None);
                    },
                    option { value: "none", {t(lang, T_BROADCAST_PRODUCT_NONE)} }
                    option { value: "{BroadcastCatalog::Strains.value()}", {BroadcastCatalog::Strains.label()} }
                    option { value: "{BroadcastCatalog::Accessories.value()}", {BroadcastCatalog::Accessories.label()} }
                    option { value: "{BroadcastCatalog::Tea.value()}", {BroadcastCatalog::Tea.label()} }
                    option { value: "{BroadcastCatalog::Sets.value()}", {BroadcastCatalog::Sets.label()} }
                }

                if catalog() != "none" {
                    match &*products.read() {
                        Some(Ok(opts)) => rsx! {
                            if opts.is_empty() {
                                div { style: "margin-top:8px;padding:8px;background:#1a1a2e;border:1px solid #2a2a4a;border-radius:4px;color:#8b8b9e;font-size:13px;", { "Каталог пуст" } }
                            } else {
                                select {
                                    style: "{select_style} margin-top:8px;",
                                    value: "{selected_product().as_ref().map(|p| p.id.clone()).unwrap_or_default()}",
                                    onchange: move |e: Event<FormData>| {
                                        let id = e.value();
                                        if let Some(Ok(opts)) = products.read().as_ref() {
                                            selected_product.set(opts.iter().find(|p| p.id == id).cloned());
                                        }
                                        error.set(None);
                                        result.set(None);
                                    },
                                    option { value: "", {t(lang, T_BROADCAST_NO_PRODUCT)} }
                                    for p in opts {
                                        option { value: "{p.id}", "{p.name}" }
                                    }
                                }
                            }
                        },
                        Some(Err(msg)) => rsx! {
                            div { style: "margin-top:8px;padding:8px;background:#2a0f15;border:1px solid #ff4757;border-radius:4px;color:#ff6b7a;font-size:13px;", "❌ {msg}" }
                        },
                        None => rsx! {
                            div { style: "margin-top:8px;", Skeleton { shape: SkeletonShape::Text, width: Some("100%".into()) } }
                        },
                    }
                }
            }

            // Button text
            div { style: "margin-bottom: 12px;",
                div { style: "font-size: 13px; color: #e8e8e8; margin-bottom: 6px;", {t(lang, T_BROADCAST_BUTTON_TEXT)} }
                input {
                    style: "{input_style()} width:100%; box-sizing:border-box;",
                    r#type: "text",
                    placeholder: "Открыть в магазине",
                    value: "{button_text()}",
                    oninput: move |e: Event<FormData>| {
                        button_text.set(e.value().clone());
                        error.set(None);
                        result.set(None);
                    },
                }
            }

            // Preview
            div { style: "margin-bottom: 12px;",
                div { style: "font-size: 13px; color: #e8e8e8; margin-bottom: 6px;", {t(lang, T_BROADCAST_PREVIEW)} }
                div { style: "background:#1a1a2e;border:2px solid #2a2a4a;border-radius:8px;padding:12px;",
                    if !photo_url().is_empty() {
                        img { src: "{photo_url()}", alt: "Preview", style: "width:100%;max-height:240px;object-fit:cover;border-radius:6px;margin-bottom:8px;" }
                    }
                    if !text().trim().is_empty() {
                        div { style: "font-size:14px;color:#e8e8e8;white-space:pre-wrap;", "{text()}" }
                    } else {
                        div { style: "font-size:13px;color:#8b8b9e;", "Текст сообщения…" }
                    }
                    if let Some(ref p) = selected_product() {
                        if let Some(cat) = BroadcastCatalog::from_value(&catalog()) {
                            { {
                                let label = {
                                    let txt = button_text();
                                    let s = txt.trim();
                                    if s.is_empty() { "Открыть в магазине".to_string() } else { s.to_string() }
                                };
                                let link = product_deep_link(cat.product_kind(), &p.id);
                                rsx! {
                                    div { style: "margin-top:10px;display:flex;flex-direction:column;gap:6px;",
                                        button {
                                            style: "padding:10px 14px;background:#00e5ff;color:#000;border:none;border-radius:6px;font-weight:700;font-size:13px;cursor:default;",
                                            disabled: true,
                                            "{label}"
                                        }
                                        div { style: "font-size:11px;color:#6699ff;word-break:break-all;", "{link}" }
                                    }
                                }
                            } }
                        }
                    }
                }
            }

            if let Some(ref msg) = error() {
                div { style: "padding: 12px; background: #2a0f15; border: 2px solid #ff4757; color: #ff6b7a; font-size: 14px; margin-bottom: 12px;",
                    "❌ {msg}"
                }
            }
            if sent() {
                div { style: "padding: 12px; background: #1a2e1a; border: 2px solid #39ff14; color: #39ff14; font-size: 14px; margin-bottom: 12px;",
                    {t(lang, T_BROADCAST_SENT)}
                }
            }
            { {
                if let Some(ref r) = result() {
                    let sent_n = r.get("sent").and_then(|v| v.as_u64());
                    let recipients_n = r.get("recipients").and_then(|v| v.as_u64());
                    let is_test = r.get("test").and_then(|v| v.as_bool()).unwrap_or(false);
                    if let (Some(sent_n), Some(recipients_n)) = (sent_n, recipients_n) {
                        if is_test {
                            rsx! {
                                div { style: "padding: 12px; background: #1a2e1a; border: 2px solid #39ff14; color: #39ff14; font-size: 14px; margin-bottom: 12px;",
                                    "🧪 "
                                    {t(lang, T_BROADCAST_TEST_SENT)}
                                    " ({sent_n}/{recipients_n})"
                                }
                            }
                        } else {
                            rsx! {
                                div { style: "padding: 12px; background: #1a2e1a; border: 2px solid #39ff14; color: #39ff14; font-size: 14px; margin-bottom: 12px;",
                                    "✅ Отправлено {sent_n} из {recipients_n} пользователей"
                                }
                            }
                        }
                    } else {
                        rsx! {}
                    }
                } else {
                    rsx! {}
                }
            } }
            div { style: "display:flex;gap:10px;flex-wrap:wrap;",
                button {
                    style: "font-size: 14px; font-weight: 700; padding: 12px 24px; background: #00e5ff; color: #000; border: 4px solid #008ba3; box-shadow: 3px 3px 0 #000; cursor: pointer;",
                    disabled: sending() || text().trim().is_empty(),
                    onclick: move |e: Event<MouseData>| {
                        e.stop_propagation();
                        send_broadcast(&mut sending, &mut sent, &mut text, &mut photo_url, &mut catalog, &mut selected_product, &mut button_text, &mut error, &mut result, false);
                    },
                    {t(lang, T_BROADCAST_SEND)}
                }
                button {
                    style: "font-size: 14px; font-weight: 700; padding: 12px 24px; background: #ffaa00; color: #000; border: 4px solid #cc8800; box-shadow: 3px 3px 0 #000; cursor: pointer;",
                    disabled: sending() || text().trim().is_empty(),
                    onclick: move |e: Event<MouseData>| {
                        e.stop_propagation();
                        send_broadcast(&mut sending, &mut sent, &mut text, &mut photo_url, &mut catalog, &mut selected_product, &mut button_text, &mut error, &mut result, true);
                    },
                    {t(lang, T_BROADCAST_SEND_TEST)}
                }
            }
        }
    }
}

fn send_broadcast(
    sending: &mut Signal<bool>,
    sent: &mut Signal<bool>,
    text: &mut Signal<String>,
    photo_url: &mut Signal<String>,
    catalog: &mut Signal<String>,
    selected_product: &mut Signal<Option<BroadcastPickerProduct>>,
    button_text: &mut Signal<String>,
    error: &mut Signal<Option<String>>,
    result: &mut Signal<Option<serde_json::Value>>,
    test_only: bool,
) {
    error.set(None);
    result.set(None);
    let catalog_val = catalog().clone();
    let product = selected_product().as_ref().and_then(|p| {
        BroadcastCatalog::from_value(&catalog_val).map(|cat| BroadcastProduct {
            kind: cat.value().to_string(),
            id: p.id.clone(),
            name: p.name.clone(),
            image_url: if p.image_url.is_empty() {
                None
            } else {
                Some(p.image_url.clone())
            },
        })
    });
    let photo = {
        let s = photo_url().trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    };
    let btn = {
        let s = button_text().trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    };
    let body = BroadcastRequest {
        text: text().trim().to_string(),
        photo_url: photo,
        product,
        button_text: btn,
    };
    let client = crate::ui::api::local_client::LocalClient::new();
    let base = api_base_url();
    let url = if test_only {
        format!("{base}/api/admin/broadcast/test")
    } else {
        format!("{base}/api/admin/broadcast")
    };
    let init = use_telegram_init_data();
    let tok = admin_token();
    let mut sending = *sending;
    let mut sent = *sent;
    let mut text = *text;
    let mut photo_url = *photo_url;
    let mut catalog = *catalog;
    let mut selected_product = *selected_product;
    let mut button_text = *button_text;
    let mut error = *error;
    let mut result = *result;
    spawn(async move {
        sending.set(true);
        let res = client
            .post(&url)
            .header("X-Telegram-Init-Data", init)
            .header("X-Admin-Token", tok)
            .json(&body)
            .send()
            .await;
        sending.set(false);
        match res {
            Ok(resp) if resp.status().is_success() => {
                let data = resp.json::<serde_json::Value>().await.ok();
                result.set(data);
                sent.set(true);
                if !test_only {
                    text.set(String::new());
                    photo_url.set(String::new());
                    catalog.set("none".to_string());
                    selected_product.set(None);
                    button_text.set(String::new());
                }
            }
            Ok(resp) => {
                let err_text = resp
                    .json::<serde_json::Value>()
                    .await
                    .ok()
                    .and_then(|v| {
                        v.get("error")
                            .and_then(|e| e.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "Ошибка рассылки".into());
                error.set(Some(err_text));
            }
            Err(_) => {
                error.set(Some("Сеть недоступна".into()));
            }
        }
    });
}
