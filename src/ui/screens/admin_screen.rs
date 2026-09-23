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
    static ADMIN_TOKEN_CACHE: RefCell<String> = const { RefCell::new(String::new()) };
}

fn set_admin_token_cache(token: &str) {
    ADMIN_TOKEN_CACHE.with(|c| *c.borrow_mut() = token.to_string());
}

fn clear_admin_token_cache() {
    ADMIN_TOKEN_CACHE.with(|c| c.borrow_mut().clear());
}

use crate::trios::api_errors::admin_event_delete_failure;
use crate::trios::i18n::{
    t, T_BROADCAST, T_BROADCAST_BUTTON_TEXT, T_BROADCAST_NO_PRODUCT, T_BROADCAST_PHOTO,
    T_BROADCAST_PHOTO_HINT, T_BROADCAST_PREVIEW, T_BROADCAST_PRODUCT, T_BROADCAST_PRODUCT_NONE,
    T_BROADCAST_SELECT_CATALOG, T_BROADCAST_SEND, T_BROADCAST_SEND_TEST, T_BROADCAST_SENT,
    T_BROADCAST_TEST_SENT, T_BROADCAST_TEXT,
};
use crate::ui::api::context::api_base_url;
use crate::ui::api::types::{
    BroadcastProduct, BroadcastRequest, Event as AdminEvent, EventBooking,
};
use crate::ui::components::{
    EmptyState, Modal, Skeleton, SkeletonShape, Toast, ToastContainer, ToastKind, VideoModal,
};
use crate::ui::screens::events_screen::parse_event_start;
use crate::ui::telegram::{
    use_telegram_id, use_telegram_init_data, HapticNotification, TelegramApp,
};

/// Read the cached admin token. Order of precedence:
/// 1. In-memory cache (populated by CloudStorage loader / login).
/// 2. localStorage fallback (for plain-browser previews / older clients).
///
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

/// serde default for `bikes.offered`, declared `NOT NULL DEFAULT TRUE`:
/// a payload that omits the key describes a family that is on offer.
fn default_true() -> bool {
    true
}

/// A bike family — the catalog unit a customer actually books. The shop
/// assigns the physical machine afterwards, so the family is what carries
/// a price and what a cart line points at (D8).
///
/// Every money field is `Option<f64>` and stays absent when the shop has
/// published no number (D9). None of them is routed through a clamp, a
/// `NOT NULL DEFAULT 0` column or `try_get_warn!`, and none is ever shown
/// as `0`, as an average or as a "from" price — `money_thb` turns an
/// absent number into a dash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdminBike {
    id: String,
    /// Stable family key — `nmax-155`, `xmax-300-new`. Also the cart's
    /// `catalog_id`, which is why it is typed once on create and left
    /// alone by the edit card.
    key: String,
    brand: String,
    model: String,
    /// Separates two generations of one model (`NEW 2023+`). Absent when
    /// the family has a single variant.
    #[serde(default)]
    variant_label: Option<String>,
    /// `scooter` | `motorcycle` — the key into the published class discounts.
    class: String,
    body: String,
    #[serde(default)]
    displacement_cc: Option<i32>,
    /// Published daily tariff **before** the class discount (D11). Every
    /// label that shows it says so: rendering it as the client price
    /// overstates every scooter by 33%.
    #[serde(default)]
    base_rate_thb_day: Option<f64>,
    /// The published class discount as a fraction (scooter `0.25`).
    /// Read-only here — the discount table is not edited from this screen,
    /// and this screen never multiplies it out.
    #[serde(default)]
    class_discount: Option<f64>,
    #[serde(default)]
    deposit_thb: Option<f64>,
    #[serde(default)]
    monthly_low_season_thb: Option<f64>,
    /// Asking price when the family is also for sale. Never derived from
    /// the internal per-unit purchase cost, which is not in this repo (D14).
    #[serde(default)]
    sale_price_thb: Option<f64>,
    /// The client-facing number as the door returned it (D11). `None`
    /// together with `client_rate_source = "unavailable"` is a real state,
    /// not an error: a human quotes that price and the screen shows no
    /// number at all.
    #[serde(default)]
    client_rate_thb_day: Option<f64>,
    #[serde(default)]
    client_rate_source: Option<String>,
    /// `false` closes a family to new rentals without deleting it —
    /// CLICK 125 today (D12).
    #[serde(default = "default_true")]
    offered: bool,
    /// `true` puts the family on the forecourt. Independent of
    /// `sale_price_thb`: for sale with no published asking price is the
    /// honest "price on request" state, and requiring a number here would
    /// force the shop to invent one (D9/D11, issue #11).
    #[serde(default)]
    for_sale: bool,
    #[serde(default)]
    description_ru: Option<String>,
    #[serde(default)]
    description_en: Option<String>,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    sort_order: i32,
    /// Unit counts exactly as the catalog query counted them. `None` means
    /// the payload carried no count — a dash, not zero units.
    #[serde(default)]
    units_total: Option<i64>,
    #[serde(default)]
    units_available: Option<i64>,
}

/// One physical machine. `unit_code` is the shop's own short code; it is
/// **not** a plate number, and plates never enter this repository (D14).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdminBikeUnit {
    id: String,
    bike_id: String,
    unit_code: String,
    #[serde(default)]
    model_year: Option<i32>,
    #[serde(default)]
    color: Option<String>,
    /// Kilometres since the shop bought the machine — the ops sheet's own
    /// column. One unit reads 2900 here and 16433 on the clock, so this is
    /// never labelled odometer or mileage in any surface.
    #[serde(default)]
    km_since_purchase: Option<i32>,
    /// `available` | `rented` | `service` | `retired`.
    status: String,
}

/// A service row for one unit: engine oil, gear oil, ABS, air filter.
///
/// **Admin-only by decision (D6).** Two units read `overdue` right now and
/// a public "serviced" badge nobody can keep accurate is exactly the kind
/// of number the data-honesty rule forbids, so nothing in this struct is
/// rendered anywhere except the Service tab below.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdminServiceRecord {
    id: String,
    bike_unit_id: String,
    service_type: String,
    #[serde(default)]
    current_km: Option<i32>,
    #[serde(default)]
    last_service_km: Option<i32>,
    #[serde(default)]
    interval_km: Option<i32>,
    /// The ops sheet keeps this as its own column rather than deriving it,
    /// so the screen stores what the admin typed and computes nothing.
    #[serde(default)]
    next_km: Option<i32>,
    /// `ok` | `due` | `overdue`, worded as the ops sheet words it.
    status: String,
    #[serde(default)]
    recorded_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BikesResp {
    bikes: Vec<AdminBike>,
}

#[derive(Debug, Deserialize)]
struct BikeUnitsResp {
    units: Vec<AdminBikeUnit>,
}

#[derive(Debug, Deserialize)]
struct ServiceRecordsResp {
    records: Vec<AdminServiceRecord>,
}

// ── Absent-stays-absent helpers (D9) ──────────────────────────

/// A price nobody published renders as a dash. Never `0`, never an
/// average, never a "from" price.
///
/// This was a second implementation until 2026-09-16, and the two had
/// drifted in both directions: it wrote `3000 ฿` where the customer-facing
/// copy writes `฿3,000` (the symbol's side is a property of the market, not
/// of the screen — D15/D18), and it rendered a stored `0.0` as `0 ฿`, which
/// is precisely the `NOT NULL DEFAULT 0` value D9 says is an absent price
/// wearing a number. The rule is one function; the admin gets the same one.
use super::catalog_screen::{finite_money, money_thb, MONEY_DASH};

/// An aggregate, not a price — and the difference is the zero.
///
/// [`money_thb`] reads `Some(0.0)` as an absent price wearing a `NOT NULL
/// DEFAULT 0` (D9), which is right for a tariff nobody published and wrong
/// here: revenue of exactly zero is a measurement, and a shop that has taken
/// no orders today should read `฿0`, not `—`. Only a missing or non-finite
/// total is unknown.
///
/// The symbol still comes from the market profile. The revenue card used to
/// put the currency in its *heading* — "Выручка (Бат)" — and print a bare
/// number, which is a third convention on a screen that already had two.
fn revenue_thb(value: Option<f64>) -> String {
    match value.filter(|v| v.is_finite() && *v >= 0.0) {
        Some(v) => crate::trios::pricing::format_baht(v),
        None => MONEY_DASH.to_string(),
    }
}

/// Same rule for counts: a count the server did not send is unknown, and
/// unknown is not zero.
fn count_or_dash(value: Option<i64>) -> String {
    match value {
        Some(v) => v.to_string(),
        None => "—".to_string(),
    }
}

fn km_or_dash(value: Option<i32>) -> String {
    match value {
        Some(v) => format!("{v} км"),
        None => "—".to_string(),
    }
}

fn year_or_dash(value: Option<i32>) -> String {
    match value {
        Some(v) => v.to_string(),
        None => "—".to_string(),
    }
}

/// Parses a money input that is allowed to be empty. Empty means the admin
/// cleared the field and the number goes back to SQL `NULL` — the point of
/// D9 is that nobody is forced to type a `0` to mean "not published".
///
/// `0` itself is refused: `bikes` declares every money column
/// `CHECK (col IS NULL OR col > 0)`, and a rate of zero reads as free.
fn money_input(raw: &str, label: &str) -> Result<Option<f64>, String> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    crate::trios::validation::parse_finite_float_in_range(raw, label, 0.01, 1_000_000.0).map(Some)
}

/// Pre-fills a money input from a nullable column. An absent price starts
/// life as an empty field, which is what makes "clear it back to nothing"
/// expressible: the admin never has to type a `0` to say "no price".
fn money_edit_value(value: Option<f64>) -> String {
    match value.filter(|v| v.is_finite()) {
        Some(v) => format!("{v:.0}"),
        None => String::new(),
    }
}

/// Same contract for the optional whole numbers (model year, the km
/// columns): empty clears the column instead of storing a zero.
fn int_input(raw: &str, label: &str, min: i32, max: i32) -> Result<Option<i32>, String> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    crate::trios::validation::parse_int_in_range(raw, label, min, max).map(Some)
}

/// `None` has to reach the wire as an explicit JSON `null`, otherwise a
/// cleared field keeps its old number in the database.
fn money_json(value: Option<f64>) -> serde_json::Value {
    match value {
        Some(v) => json!(v),
        None => serde_json::Value::Null,
    }
}

fn int_json(value: Option<i32>) -> serde_json::Value {
    match value {
        Some(v) => json!(v),
        None => serde_json::Value::Null,
    }
}

/// Blank text is absent text, and absent text is `null` — not `""`.
fn text_json(value: &str) -> serde_json::Value {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        serde_json::Value::Null
    } else {
        json!(trimmed)
    }
}

/// The rental window as an order line carries it (D8). A half-open window
/// prints the end that exists instead of inventing the other one.
fn rental_window(start: Option<&str>, end: Option<&str>) -> String {
    let start = start.map(str::trim).filter(|v| !v.is_empty());
    let end = end.map(str::trim).filter(|v| !v.is_empty());
    match (start, end) {
        (Some(start), Some(end)) => format!(" · {start} → {end}"),
        (Some(start), None) => format!(" · с {start}"),
        (None, Some(end)) => format!(" · до {end}"),
        (None, None) => String::new(),
    }
}

// ── Label helpers ─────────────────────────────────────────────

fn bike_class_label(class: &str) -> &'static str {
    match class {
        "scooter" => "🛵 Скутер",
        "motorcycle" => "🏍 Мотоцикл",
        _ => "Класс не указан",
    }
}

/// `Honda ADV 350`, or `Yamaha X-MAX 300 · NEW 2023+` when two generations
/// of one model are both in the fleet.
fn bike_title(bike: &AdminBike) -> String {
    match bike
        .variant_label
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        Some(variant) => format!("{} {} · {}", bike.brand, bike.model, variant),
        None => format!("{} {}", bike.brand, bike.model),
    }
}

/// The one-line summary under a family's name. The tariff is announced as
/// pre-discount every single time it is printed (D11).
fn bike_sub(bike: &AdminBike) -> String {
    let cc = match bike.displacement_cc {
        Some(v) => format!("{v} см³"),
        None => "—".to_string(),
    };
    format!(
        "{} • {} • тариф {}/сут до скидки • депозит {} • свободно {} из {}",
        bike_class_label(bike.class.as_str()),
        cc,
        money_thb(bike.base_rate_thb_day),
        money_thb(bike.deposit_thb),
        count_or_dash(bike.units_available),
        count_or_dash(bike.units_total),
    )
}

/// What this screen is allowed to say about the client-facing price (D11).
/// When the door published no number the answer is a sentence, never a
/// number — not exact, not "from", not an average, not a range.
///
/// `finite_money`, not a bare `is_finite`: a client rate of `0.0` is the
/// `NOT NULL DEFAULT 0` the door never filled in, and printing it as `฿0`
/// would be the bot showing a number D11 says only a human may quote. It
/// falls through to the sentence below instead.
fn client_rate_note(bike: &AdminBike) -> String {
    match finite_money(bike.client_rate_thb_day) {
        Some(rate) => {
            let source = match bike.client_rate_source.as_deref() {
                Some("door") => "дверь",
                Some(other) if !other.trim().is_empty() => other,
                _ => "источник не указан",
            };
            // `{}/сут`, matching `bike_sub` twelve lines up. This line spelled
            // the tariff `{rate:.0} ฿/сут` until 2026-09-16 — two conventions
            // for one unit on one card.
            format!(
                "Цена клиенту: {}/сут ({source})",
                crate::trios::pricing::format_baht(rate)
            )
        }
        None => "Цена клиенту: называет человек — бот числа не показывает".to_string(),
    }
}

/// `(label, css badge modifier)` for the four states `bike_units.status`
/// allows. An unrecognised string is reported as unknown instead of being
/// folded into one of the four.
fn unit_status_label(status: &str) -> (&'static str, &'static str) {
    match status {
        "available" => ("Свободен", "success"),
        "rented" => ("В аренде", "info"),
        "service" => ("На сервисе", "warn"),
        "retired" => ("Выведен", "muted"),
        _ => ("Статус неизвестен", "muted"),
    }
}

/// `ok` | `due` | `overdue` — the three words the ops sheet uses.
fn service_status_label(status: &str) -> (&'static str, &'static str) {
    match status {
        "ok" => ("В норме", "success"),
        "due" => ("Пора", "warn"),
        "overdue" => ("Просрочено", "danger"),
        _ => ("Статус неизвестен", "muted"),
    }
}

/// The four service kinds the fleet register tracks (D6). The column is
/// deliberately unconstrained text, so anything else is shown verbatim
/// rather than guessed at.
fn service_type_label(service_type: &str) -> String {
    match service_type {
        "oil" => "Масло двигателя".to_string(),
        "gear_oil" => "Масло редуктора".to_string(),
        "abs" => "ABS".to_string(),
        "air_filter" => "Воздушный фильтр".to_string(),
        other => other.to_string(),
    }
}

#[derive(Debug, Deserialize)]
struct AdminCheck {
    is_admin: bool,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Bikes,
    BikeUnits,
    /// Service records live behind this tab and nowhere else (D6).
    Service,
    Dashboard,
    Orders,
    // Quests hidden until partner locations are configured.
    #[allow(dead_code)]
    Quests,
    Treasures,
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
    let active_tab = use_signal(|| Tab::Bikes);
    let build_version: &'static str = env!("BUILD_VERSION");

    // Password-only admin auth. We no longer rely on Telegram initData for
    // the initial gate — it fails in plain browsers and is confusing in the
    // Telegram WebApp when the user is not yet in ADMIN_IDS. A valid
    // ADMIN_PASSWORD token is enough; Telegram ID is optional metadata.
    let password_token = use_signal(admin_token);

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
                {tab_btn(Tab::Bikes, "🏍 Байки")}
                {tab_btn(Tab::BikeUnits, "🏷 Юниты")}
                {tab_btn(Tab::Service, "🔧 Сервис")}
                {tab_btn(Tab::Treasures, "🏴\u{200d}☠️ Сокровища")}
                {tab_btn(Tab::Loyalty, "💎 Лояльность")}
                {tab_btn(Tab::Managers, "👥 Менеджеры")}
                {tab_btn(Tab::Events, "📅 События")}
                {tab_btn(Tab::Broadcast, "📣 Рассылка")}
            }
            match current {
                Tab::Bikes => rsx!(TabMount { tab: current, expected: Tab::Bikes, BikesTab {} }),
                Tab::BikeUnits => rsx!(TabMount { tab: current, expected: Tab::BikeUnits, BikeUnitsTab {} }),
                Tab::Service => rsx!(TabMount { tab: current, expected: Tab::Service, ServiceTab {} }),
                Tab::Dashboard => rsx!(TabMount { tab: current, expected: Tab::Dashboard, DashboardTab {} }),
                Tab::Orders => rsx!(TabMount { tab: current, expected: Tab::Orders, OrdersTab {} }),
                // Quests hidden until partner locations are configured (GH: keep Tab::Quests/QuestsTab code).
                Tab::Quests => rsx!(TabMount { tab: current, expected: Tab::Quests, div {} }),
                Tab::Treasures => rsx!(TabMount { tab: current, expected: Tab::Treasures, TreasuresTab {} }),
                Tab::Loyalty => rsx!(TabMount { tab: current, expected: Tab::Loyalty, LoyaltyTab {} }),
                Tab::Managers => rsx!(TabMount { tab: current, expected: Tab::Managers, ManagersTab {} }),
                Tab::Events => rsx!(TabMount { tab: current, expected: Tab::Events, EventsTab {} }),
                Tab::Broadcast => rsx!(TabMount { tab: current, expected: Tab::Broadcast, BroadcastTab {} }),
            }
        }
    }
}

// ── Bike families tab (Optimistic UI) ─────────────────────────

#[component]
fn BikesTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut cache: Signal<Vec<AdminBike>> = use_signal(|| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(json)) = storage.get_item("tb_admin_bikes") {
                        if json.len() <= 1_000_000 {
                            if let Ok(data) = serde_json::from_str::<Vec<AdminBike>>(&json) {
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
    let mut family_key = use_signal(String::new);
    let mut brand = use_signal(String::new);
    let mut model = use_signal(String::new);
    let mut variant_label = use_signal(String::new);
    let mut bike_class = use_signal(|| "scooter".to_string());
    let mut bike_body = use_signal(|| "scooter".to_string());
    let mut displacement_cc = use_signal(String::new);
    let mut base_rate = use_signal(String::new);
    let mut deposit = use_signal(String::new);
    let mut monthly = use_signal(String::new);
    let mut sale_price = use_signal(String::new);
    let mut description_ru = use_signal(String::new);
    let mut description_en = use_signal(String::new);
    let mut image_url = use_signal(String::new);
    let mut status = use_signal(String::new);
    let mut submitting = use_signal(|| false);
    let mut editing_id: Signal<Option<String>> = use_signal(|| None);
    let mut delete_target_id: Signal<Option<String>> = use_signal(|| None);
    let mut search_query = use_signal(String::new);
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
                        "tb_admin_bikes",
                        &serde_json::to_string(&data).unwrap_or_default(),
                    );
                }
            }
        }
    });

    // Fetch data into cache (runs on mount). The admin list is its own route,
    // not the public one with a flag: the public catalog hides CLICK 125
    // (D12) and the admin has to see it to re-open it, but no query string an
    // anonymous caller can type may reach a hidden family. This used to point
    // at `/api/bikes?include_unoffered=true`, which was never a route — the
    // flag was read by nothing and the screen has always been looking at the
    // offered families only.
    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/admin/bikes", api_base_url());
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
                if let Ok(data) = serde_json::from_str::<BikesResp>(&text) {
                    cache.set(data.bikes);
                }
            }
            loading.set(false);
            Some(())
        }
    });

    let filtered: Vec<AdminBike> = {
        let q = search_query.read().to_lowercase();
        cache
            .read()
            .iter()
            .filter(|b| {
                q.is_empty()
                    || b.key.to_lowercase().contains(&q)
                    || bike_title(b).to_lowercase().contains(&q)
                    || b.class.to_lowercase().contains(&q)
                    || b.body.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    };

    rsx! {
           div {
               FormCard {
                   title: "Добавить модель".to_string(),
                   children: rsx!{
                       input { style: input_style(), placeholder: "Ключ (nmax-155)", value: "{family_key}",
                           oninput: move |e| family_key.set(e.value()) }
                       input { style: input_style(), placeholder: "Бренд (Yamaha)", value: "{brand}",
                           oninput: move |e| brand.set(e.value()) }
                       input { style: input_style(), placeholder: "Модель (NMAX 155)", value: "{model}",
                           oninput: move |e| model.set(e.value()) }
                       input { style: input_style(), placeholder: "Вариант (NEW 2023+) — можно пусто", value: "{variant_label}",
                           oninput: move |e| variant_label.set(e.value()) }
                       select { style: input_style(), value: "{bike_class}",
                           "aria-label": "Класс",
                           oninput: move |e| bike_class.set(e.value()),
                           option { value: "scooter", "🛵 Скутер" }
                           option { value: "motorcycle", "🏍 Мотоцикл" }
                       }
                       select { style: input_style(), value: "{bike_body}",
                           "aria-label": "Тип кузова",
                           oninput: move |e| bike_body.set(e.value()),
                           option { value: "scooter", "Скутер" }
                           option { value: "maxi-scooter", "Макси-скутер" }
                           option { value: "adventure-scooter", "Адвенчер-скутер" }
                           option { value: "naked", "Нейкед" }
                           option { value: "sport", "Спорт" }
                           option { value: "cruiser", "Круизер" }
                       }
                       input { style: input_style(), placeholder: "Объём, см³", value: "{displacement_cc}", r#type: "number",
                           oninput: move |e| displacement_cc.set(e.value()) }
                       input { style: input_style(), placeholder: "Тариф ฿/сут ДО скидки — пусто = прочерк", value: "{base_rate}", r#type: "number",
                           oninput: move |e| base_rate.set(e.value()) }
                       input { style: input_style(), placeholder: "Депозит ฿ — пусто = прочерк", value: "{deposit}", r#type: "number",
                           oninput: move |e| deposit.set(e.value()) }
                       input { style: input_style(), placeholder: "Месяц, низкий сезон ฿ — пусто = прочерк", value: "{monthly}", r#type: "number",
                           oninput: move |e| monthly.set(e.value()) }
                       input { style: input_style(), placeholder: "Цена продажи ฿ — пусто = не продаётся", value: "{sale_price}", r#type: "number",
                           oninput: move |e| sale_price.set(e.value()) }
                       textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description_ru}",
                           oninput: move |e| description_ru.set(e.value()) }
                       ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
                       div { style: en_section_style(), "🇬🇧 English" }
                       textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}",
                           oninput: move |e| description_en.set(e.value()) }
                       button {
                           style: if *submitting.read() { submit_btn_disabled_style() } else { submit_btn_style() },
                           disabled: *submitting.read(),
                           r#type: "button",
                           onclick: move |_| {
                               let k = family_key().trim().to_lowercase();
                               let br = brand().trim().to_string();
                               let md = model().trim().to_string();
                               if k.is_empty() || br.is_empty() || md.is_empty() {
                                   status.set("❌ Ключ, бренд и модель обязательны".into()); return;
                               }
                               if !k.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
                                   status.set("❌ Ключ: строчные латинские буквы, цифры и дефис".into()); return;
                               }
                               let cc = match crate::trios::validation::parse_int_in_range(
                                   &displacement_cc.read(), "Объём", 49, 2000,
                               ) {
                                   Ok(v) => v,
                                   Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                               };
                               // D9: each money field may stay empty, and empty
                               // travels to the column as an explicit null. A
                               // typed 0 is refused — the column forbids it and
                               // a rate of 0 reads as free.
                               let rate = match money_input(&base_rate.read(), "Тариф") {
                                   Ok(v) => v,
                                   Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                               };
                               let dep = match money_input(&deposit.read(), "Депозит") {
                                   Ok(v) => v,
                                   Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                               };
                               let mon = match money_input(&monthly.read(), "Месячная цена") {
                                   Ok(v) => v,
                                   Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                               };
                               let sale = match money_input(&sale_price.read(), "Цена продажи") {
                                   Ok(v) => v,
                                   Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                               };
                               let cls = bike_class(); let bd = bike_body();
                               let vl = variant_label().trim().to_string();
                               let dru = description_ru(); let den = description_en();
                               let img = crate::trios::validation::normalize_media_url(&image_url());
                               submitting.set(true); status.set(String::new());
                               // ── Optimistic: insert immediately ──
                               let temp_id = format!("temp-{}", uuid::Uuid::new_v4());
                               cache.write().insert(0, AdminBike {
                                   id: temp_id.clone(), key: k.clone(),
                                   brand: br.clone(), model: md.clone(),
                                   variant_label: if vl.is_empty() { None } else { Some(vl.clone()) },
                                   class: cls.clone(), body: bd.clone(),
                                   displacement_cc: Some(cc),
                                   base_rate_thb_day: rate, class_discount: None,
                                   deposit_thb: dep, monthly_low_season_thb: mon,
                                   sale_price_thb: sale,
                                   // The door is asked by the catalog, not from
                                   // here, so the client price is unknown to this
                                   // row until the next fetch — a dash, not a
                                   // copy of the tariff (D11).
                                   client_rate_thb_day: None, client_rate_source: None,
                                   offered: true, for_sale: false,
                                   description_ru: if dru.is_empty() { None } else { Some(dru.clone()) },
                                   description_en: if den.is_empty() { None } else { Some(den.clone()) },
                                   image_url: if img.is_empty() { None } else { Some(img.clone()) },
                                   sort_order: 0,
                                   // Nobody has counted units for a family that
                                   // was created a millisecond ago.
                                   units_total: None, units_available: None,
                               });
                               auto_scroll_to_list();
                               spawn(async move {
                                   let body = json!({
                                       "key": k, "brand": br, "model": md,
                                       "variant_label": text_json(&vl),
                                       "class": cls, "body": bd,
                                       "displacement_cc": cc,
                                       "base_rate_thb_day": money_json(rate),
                                       "deposit_thb": money_json(dep),
                                       "monthly_low_season_thb": money_json(mon),
                                       "sale_price_thb": money_json(sale),
                                       "offered": true, "for_sale": false,
                                       "description_ru": text_json(&dru),
                                       "description_en": text_json(&den),
                                       "image_url": text_json(&img),
                                   });
                                   let url = format!("{}/api/admin/bikes", api_base_url());
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
                                                   if let Some(b) = cache.write().iter_mut().find(|b| b.id == temp_id) { b.id = real_id.to_string(); }
                                               }
                                           }
                                           status.set("✅ Добавлена!".into());
                                           family_key.set(String::new()); brand.set(String::new()); model.set(String::new());
                                           variant_label.set(String::new()); displacement_cc.set(String::new());
                                           base_rate.set(String::new()); deposit.set(String::new());
                                           monthly.set(String::new()); sale_price.set(String::new());
                                           description_ru.set(String::new()); description_en.set(String::new());
                                           image_url.set(String::new());
                                           TelegramApp::init().haptic_notification(HapticNotification::Success);
                                       }
                                       other => {
                                           cache.write().retain(|b| b.id != temp_id);
                                           status.set(format!("❌ Ошибка добавления ({})", admin_add_failure_reason(other).await));
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

               div { style: "background:#1a1a2e;border:1px solid #2a2a4a;border-radius:6px;padding:8px 10px;margin-bottom:8px;font-size:11px;color:#8b8b9e;line-height:1.5;",
                   "Тариф — публикуемая ставка ДО скидки класса (скутер −25%, мотоцикл −15%); экран её не перемножает. Клиентскую цену называет дверь, и если дверь молчит — числа нет вообще. Пустое поле цены остаётся прочерком, а не нулём."
               }

               h3 { style: list_title_style(), "Модели ({filtered.len()})" }
               {render_search(search_query)}
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
                       icon: "🏍",
                       title: "Моделей нет",
                       description: "Добавьте семейство байков или измените запрос поиска",
                       action: rsx! {
                           button {
                               style: "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:13px;cursor:pointer;",
                               onclick: move |_| search_query.set(String::new()),
                               "Очистить поиск"
                           }
                       },
                   }
               } else {
                   div { "data-list": "true", style: "display:flex;flex-direction:column;gap:4px;",
                       for b in filtered {
                           if editing_id.read().as_deref() == Some(b.id.as_str()) {
                               EditBikeCard {
                                   key: "{b.id}",
                                   item: b.clone(),
                                   cache: cache,
                                   on_saved: move |_| editing_id.set(None),
                                   on_cancel: move |_| editing_id.set(None),
                               }
                           } else {
                               ItemRow {
                                   key: "{b.id}",
                                   name: bike_title(&b),
                                   sub: bike_sub(&b),
                                   is_available: b.offered,
                                   image_url: b.image_url.clone(),
                                   on_edit: {
                                       let id = b.id.clone();
                                       move |_| editing_id.set(Some(id.clone()))
                                   },
                                   on_toggle: {
                                       let id = b.id.clone();
                                       let next_offered = !b.offered;
                                       move |_| {
                                           let id = id.clone();
                                           if let Some(b) = cache.write().iter_mut().find(|b| b.id == id) { b.offered = next_offered; }
                                           spawn(async move {
                                               let url = format!("{}/api/admin/bikes/{}/offered", api_base_url(), id);
                                               let res = HTTP_CLIENT.clone().put(&url)
                                                   .header("X-Telegram-Init-Data", init_data.read().clone())
                                                   .header("X-Admin-Token", admin_token())
                                                   .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                   .json(&json!({ "offered": next_offered }))
                                                   .send().await;
                                               match res {
                                                   Ok(r) if r.status().is_success() => { TelegramApp::init().haptic_notification(HapticNotification::Success); }
                                                   _ => {
                                                       if let Some(b) = cache.write().iter_mut().find(|b| b.id == id) { b.offered = !next_offered; }
                                                       push_toast(toasts, "Не удалось изменить статус модели".into(), ToastKind::Error);
                                                       TelegramApp::init().haptic_notification(HapticNotification::Error);
                                                   }
                                               }
                                           });
                                       }
                                   },
                                   on_delete: {
                                       let id = b.id.clone();
                                       move |_| delete_target_id.set(Some(id.clone()))
                                   }
                               }
                           }
                       }
                   }
               }
               DeleteConfirmModal {
                   target: delete_target_id,
                   item_name: "модель".to_string(),
                   on_confirm: move |id: String| {
                       let deleted = cache.read().iter().find(|b| b.id == id).cloned();
                       cache.write().retain(|b| b.id != id);
                       spawn(async move {
                           let url = format!("{}/api/admin/bikes/{}", api_base_url(), id);
                           let res = HTTP_CLIENT.clone().delete(&url)
                               .header("X-Telegram-Init-Data", init_data.read().clone())
                               .header("X-Admin-Token", admin_token())
                               .header("X-Admin-Telegram-Id", telegram_id.to_string())
                               .send().await;
                           match res {
                               Ok(r) if r.status().is_success() => { push_toast(toasts, "Модель удалена".into(), ToastKind::Success); TelegramApp::init().haptic_notification(HapticNotification::Success); }
                               _ => {
                                   if let Some(item) = deleted { cache.write().push(item); }
                                   push_toast(toasts, "Не удалось удалить модель".into(), ToastKind::Error);
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

// ── Bike units tab (Optimistic UI) ────────────────────────────
//
// A unit is a physical machine. Nothing identifying rides along with it:
// no plate, no key code, no renter, no purchase cost (D14). `unit_code` is
// the shop's own short code and `km_since_purchase` is labelled as what it
// is — kilometres since purchase, not an odometer reading.

#[component]
fn BikeUnitsTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut cache: Signal<Vec<AdminBikeUnit>> = use_signal(|| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(json)) = storage.get_item("tb_admin_bike_units") {
                        if json.len() <= 1_000_000 {
                            if let Ok(data) = serde_json::from_str::<Vec<AdminBikeUnit>>(&json) {
                                return data;
                            }
                        }
                    }
                }
            }
        }
        Vec::new()
    });
    let mut families: Signal<Vec<AdminBike>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut bike_id = use_signal(String::new);
    let mut unit_code = use_signal(String::new);
    let mut model_year = use_signal(String::new);
    let mut color = use_signal(String::new);
    let mut km_since_purchase = use_signal(String::new);
    let mut unit_status = use_signal(|| "available".to_string());
    let mut status = use_signal(String::new);
    let mut submitting = use_signal(|| false);
    let mut editing_id: Signal<Option<String>> = use_signal(|| None);
    let mut delete_target_id: Signal<Option<String>> = use_signal(|| None);
    let mut search_query = use_signal(String::new);
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);

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
                        "tb_admin_bike_units",
                        &serde_json::to_string(&data).unwrap_or_default(),
                    );
                }
            }
        }
    });

    // Families are fetched for the picker and for the row subtitles: a unit
    // carries `bike_id` only, and inventing a model name from the unit code
    // would be a guess.
    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/admin/bikes", api_base_url());
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
                if let Ok(data) = serde_json::from_str::<BikesResp>(&text) {
                    families.set(data.bikes);
                }
            }
            Some(())
        }
    });

    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/admin/bike-units", api_base_url());
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
                if let Ok(data) = serde_json::from_str::<BikeUnitsResp>(&text) {
                    cache.set(data.units);
                }
            }
            loading.set(false);
            Some(())
        }
    });

    // Each row is paired with its family label up front, so the render pass
    // does no lookups and an orphan unit says so instead of showing nothing.
    let filtered: Vec<(AdminBikeUnit, String)> = {
        let q = search_query.read().to_lowercase();
        let fams = families.read().clone();
        cache
            .read()
            .iter()
            .map(|u| {
                let label = match fams.iter().find(|b| b.id == u.bike_id) {
                    Some(b) => format!("{} · {}", bike_title(b), b.key),
                    None => "Модель не найдена".to_string(),
                };
                (u.clone(), label)
            })
            .filter(|(u, label)| {
                q.is_empty()
                    || u.unit_code.to_lowercase().contains(&q)
                    || label.to_lowercase().contains(&q)
                    || u.status.to_lowercase().contains(&q)
                    || u.color.as_deref().unwrap_or("").to_lowercase().contains(&q)
            })
            .collect()
    };
    let family_options = families.read().clone();

    rsx! {
        div {
            FormCard {
                title: "Добавить юнит".to_string(),
                children: rsx!{
                    select { style: input_style(), value: "{bike_id}",
                        "aria-label": "Модель юнита",
                        oninput: move |e| bike_id.set(e.value()),
                        option { value: "", "— выберите модель —" }
                        for f in family_options.iter() {
                            option { key: "{f.id}", value: "{f.id}", "{bike_title(f)} · {f.key}" }
                        }
                    }
                    input { style: input_style(), placeholder: "Код юнита (не номер!)", value: "{unit_code}",
                        oninput: move |e| unit_code.set(e.value()) }
                    input { style: input_style(), placeholder: "Год модели — можно пусто", value: "{model_year}", r#type: "number",
                        oninput: move |e| model_year.set(e.value()) }
                    input { style: input_style(), placeholder: "Цвет — можно пусто", value: "{color}",
                        oninput: move |e| color.set(e.value()) }
                    input { style: input_style(), placeholder: "Км с покупки — можно пусто", value: "{km_since_purchase}", r#type: "number",
                        oninput: move |e| km_since_purchase.set(e.value()) }
                    select { style: input_style(), value: "{unit_status}",
                        "aria-label": "Статус юнита",
                        oninput: move |e| unit_status.set(e.value()),
                        option { value: "available", "Свободен" }
                        option { value: "rented", "В аренде" }
                        option { value: "service", "На сервисе" }
                        option { value: "retired", "Выведен" }
                    }
                    button {
                        style: if *submitting.read() { submit_btn_disabled_style() } else { submit_btn_style() },
                        disabled: *submitting.read(),
                        r#type: "button",
                        onclick: move |_| {
                            let bid = bike_id().trim().to_string();
                            let code = unit_code().trim().to_string();
                            if bid.is_empty() { status.set("❌ Выберите модель".into()); return; }
                            if code.is_empty() { status.set("❌ Код юнита обязателен".into()); return; }
                            let year = match int_input(&model_year.read(), "Год модели", 1980, 2100) {
                                Ok(v) => v,
                                Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                            };
                            // Km is optional and stays absent when unknown: a
                            // zero here would claim a machine straight off the
                            // truck.
                            let km = match int_input(&km_since_purchase.read(), "Км с покупки", 0, 1_000_000) {
                                Ok(v) => v,
                                Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                            };
                            let col = color().trim().to_string();
                            let st = unit_status();
                            submitting.set(true); status.set(String::new());
                            let temp_id = format!("temp-{}", uuid::Uuid::new_v4());
                            cache.write().insert(0, AdminBikeUnit {
                                id: temp_id.clone(), bike_id: bid.clone(), unit_code: code.clone(),
                                model_year: year,
                                color: if col.is_empty() { None } else { Some(col.clone()) },
                                km_since_purchase: km, status: st.clone(),
                            });
                            auto_scroll_to_list();
                            spawn(async move {
                                let body = json!({
                                    "bike_id": bid, "unit_code": code,
                                    "model_year": int_json(year),
                                    "color": text_json(&col),
                                    "km_since_purchase": int_json(km),
                                    "status": st,
                                });
                                let url = format!("{}/api/admin/bike-units", api_base_url());
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
                                                if let Some(u) = cache.write().iter_mut().find(|u| u.id == temp_id) { u.id = real_id.to_string(); }
                                            }
                                        }
                                        status.set("✅ Добавлен!".into());
                                        unit_code.set(String::new()); model_year.set(String::new());
                                        color.set(String::new()); km_since_purchase.set(String::new());
                                        TelegramApp::init().haptic_notification(HapticNotification::Success);
                                    }
                                    other => {
                                        cache.write().retain(|u| u.id != temp_id);
                                        status.set(format!("❌ Ошибка добавления ({})", admin_add_failure_reason(other).await));
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

            h3 { style: list_title_style(), "Юниты ({filtered.len()})" }
            {render_search(search_query)}
            if *loading.read() {
                div { style: "display:flex;flex-direction:column;gap:4px;",
                    for _ in 0..4 {
                        div { style: "background:#1a1a2e;padding:8px 10px;border-radius:6px;display:flex;flex-direction:column;gap:4px;",
                            Skeleton { shape: SkeletonShape::Text, width: Some("50%".into()) }
                            Skeleton { shape: SkeletonShape::TextSm, width: Some("70%".into()) }
                        }
                    }
                }
            } else if filtered.is_empty() {
                EmptyState {
                    icon: "🏷",
                    title: "Юнитов нет",
                    description: "Добавьте машину к одной из моделей или измените запрос поиска",
                    action: rsx! {
                        button {
                            style: "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:13px;cursor:pointer;",
                            onclick: move |_| search_query.set(String::new()),
                            "Очистить поиск"
                        }
                    },
                }
            } else {
                div { "data-list": "true", style: "display:flex;flex-direction:column;gap:4px;",
                    for (u, label) in filtered {
                        if editing_id.read().as_deref() == Some(u.id.as_str()) {
                            EditBikeUnitCard {
                                key: "{u.id}",
                                item: u.clone(),
                                families: families.read().clone(),
                                cache: cache,
                                on_saved: move |_| editing_id.set(None),
                                on_cancel: move |_| editing_id.set(None),
                            }
                        } else {
                            {
                                let (badge_label, badge_class) = unit_status_label(u.status.as_str());
                                let year = year_or_dash(u.model_year);
                                let color_text = u.color.clone().unwrap_or_else(|| "—".to_string());
                                let km_text = km_or_dash(u.km_since_purchase);
                                let code = u.unit_code.clone();
                                let id_for_status = u.id.clone();
                                let id_for_edit = u.id.clone();
                                let id_for_delete = u.id.clone();
                                let previous_status = u.status.clone();
                                let current_status = u.status.clone();
                                rsx! {
                                    div { key: "{u.id}", style: "background:#1a1a2e;padding:8px 10px;border-radius:6px;display:flex;flex-direction:column;gap:6px;",
                                        div { style: "display:flex;align-items:center;gap:8px;",
                                            div { style: "flex:1;min-width:0;",
                                                div { style: "font-weight:700;font-size:13px;color:#e8e8e8;", "{code}" }
                                                div { style: "font-size:11px;color:#888;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;", "{label}" }
                                            }
                                            span { class: "admin-badge {badge_class}", "{badge_label}" }
                                        }
                                        div { style: "font-size:11px;color:#8b8b9e;",
                                            "Год: {year} • Цвет: {color_text} • С покупки: {km_text}"
                                        }
                                        div { style: "display:flex;align-items:center;gap:6px;",
                                            select {
                                                style: "flex:1;padding:8px 10px;background:#0f0f1a;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:13px;cursor:pointer;",
                                                value: "{current_status}",
                                                "aria-label": "Статус юнита {code}",
                                                onchange: move |e: Event<FormData>| {
                                                    let next = e.value();
                                                    let previous = previous_status.clone();
                                                    if next == previous { return; }
                                                    let id = id_for_status.clone();
                                                    if let Some(u) = cache.write().iter_mut().find(|u| u.id == id) { u.status = next.clone(); }
                                                    spawn(async move {
                                                        let url = format!("{}/api/admin/bike-units/{}/status", api_base_url(), id);
                                                        let res = HTTP_CLIENT.clone().put(&url)
                                                            .header("X-Telegram-Init-Data", init_data.read().clone())
                                                            .header("X-Admin-Token", admin_token())
                                                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                            .json(&json!({ "status": next }))
                                                            .send().await;
                                                        match res {
                                                            Ok(r) if r.status().is_success() => { TelegramApp::init().haptic_notification(HapticNotification::Success); }
                                                            _ => {
                                                                if let Some(u) = cache.write().iter_mut().find(|u| u.id == id) { u.status = previous.clone(); }
                                                                push_toast(toasts, "Не удалось изменить статус юнита".into(), ToastKind::Error);
                                                                TelegramApp::init().haptic_notification(HapticNotification::Error);
                                                            }
                                                        }
                                                    });
                                                },
                                                option { value: "available", "Свободен" }
                                                option { value: "rented", "В аренде" }
                                                option { value: "service", "На сервисе" }
                                                option { value: "retired", "Выведен" }
                                            }
                                            button {
                                                style: "flex-shrink:0;min-width:44px;min-height:44px;padding:8px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:16px;cursor:pointer;line-height:1;display:flex;align-items:center;justify-content:center;",
                                                "aria-label": "Редактировать юнит {code}",
                                                onclick: move |_| editing_id.set(Some(id_for_edit.clone())),
                                                "✏️"
                                            }
                                            button {
                                                style: "flex-shrink:0;min-width:44px;min-height:44px;padding:8px;background:#3a1a1a;color:#ff8888;border:none;border-radius:4px;font-size:16px;cursor:pointer;line-height:1;display:flex;align-items:center;justify-content:center;",
                                                "aria-label": "Удалить юнит {code}",
                                                onclick: move |_| delete_target_id.set(Some(id_for_delete.clone())),
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
            DeleteConfirmModal {
                target: delete_target_id,
                item_name: "юнит".to_string(),
                on_confirm: move |id: String| {
                    let deleted = cache.read().iter().find(|u| u.id == id).cloned();
                    cache.write().retain(|u| u.id != id);
                    spawn(async move {
                        let url = format!("{}/api/admin/bike-units/{}", api_base_url(), id);
                        let res = HTTP_CLIENT.clone().delete(&url)
                            .header("X-Telegram-Init-Data", init_data.read().clone())
                            .header("X-Admin-Token", admin_token())
                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                            .send().await;
                        // The server's own sentence is shown when it sends
                        // one. Deleting a unit that has service records is
                        // refused with a 409 explaining that «Выведен» is
                        // almost certainly the status wanted instead — advice
                        // a generic "не удалось" would throw away, leaving the
                        // owner pressing the same button again.
                        let reason = match res {
                            Ok(r) if r.status().is_success() => None,
                            Ok(r) => {
                                let code = r.status().as_u16();
                                let body = r.text().await.unwrap_or_default();
                                let b = body.trim();
                                Some(if b.is_empty() {
                                    format!("Не удалось удалить юнит (HTTP {code})")
                                } else {
                                    b.chars().take(160).collect()
                                })
                            }
                            Err(_) => Some("Не удалось удалить юнит: сеть/таймаут".to_string()),
                        };
                        match reason {
                            None => { push_toast(toasts, "Юнит удалён".into(), ToastKind::Success); TelegramApp::init().haptic_notification(HapticNotification::Success); }
                            Some(reason) => {
                                if let Some(item) = deleted { cache.write().push(item); }
                                push_toast(toasts, reason, ToastKind::Error);
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

// ── Service records tab (Optimistic UI) ───────────────────────
//
// D6: this tab is the only surface that shows service state, and the only
// writer of `bike_service_records` — the table ships empty. Two units read
// `overdue` today; a public "serviced" badge nobody can keep accurate is
// exactly the kind of number the data-honesty rule forbids, so nothing
// here reaches the customer catalog.
//
// Records are appended and deleted, not edited: a service entry is a log
// line about a day that has already happened.

#[component]
fn ServiceTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut cache: Signal<Vec<AdminServiceRecord>> = use_signal(Vec::new);
    let mut units: Signal<Vec<AdminBikeUnit>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut bike_unit_id = use_signal(String::new);
    let mut service_type = use_signal(|| "oil".to_string());
    let mut service_type_custom = use_signal(String::new);
    let mut current_km = use_signal(String::new);
    let mut last_service_km = use_signal(String::new);
    let mut interval_km = use_signal(String::new);
    let mut next_km = use_signal(String::new);
    let mut record_status = use_signal(|| "ok".to_string());
    let mut status = use_signal(String::new);
    let mut submitting = use_signal(|| false);
    let mut delete_target_id: Signal<Option<String>> = use_signal(|| None);
    let mut search_query = use_signal(String::new);
    let toasts: Signal<Vec<ToastItem>> = use_signal(Vec::new);

    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/admin/bike-units", api_base_url());
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
                if let Ok(data) = serde_json::from_str::<BikeUnitsResp>(&text) {
                    units.set(data.units);
                }
            }
            Some(())
        }
    });

    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/admin/bike-service-records", api_base_url());
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
                if let Ok(data) = serde_json::from_str::<ServiceRecordsResp>(&text) {
                    cache.set(data.records);
                }
            }
            loading.set(false);
            Some(())
        }
    });

    let filtered: Vec<(AdminServiceRecord, String)> = {
        let q = search_query.read().to_lowercase();
        let known_units = units.read().clone();
        cache
            .read()
            .iter()
            .map(|r| {
                let code = match known_units.iter().find(|u| u.id == r.bike_unit_id) {
                    Some(u) => u.unit_code.clone(),
                    None => "Юнит не найден".to_string(),
                };
                (r.clone(), code)
            })
            .filter(|(r, code)| {
                q.is_empty()
                    || code.to_lowercase().contains(&q)
                    || r.status.to_lowercase().contains(&q)
                    || service_type_label(r.service_type.as_str())
                        .to_lowercase()
                        .contains(&q)
            })
            .collect()
    };
    // A count of what is loaded, described as exactly that.
    let overdue_loaded = cache
        .read()
        .iter()
        .filter(|r| r.status == "overdue")
        .count();
    let unit_options = units.read().clone();
    let custom_type = *service_type.read() == "__custom__";

    rsx! {
        div {
            div { style: "background:#2a1a0f;border:1px solid #ff9d00;border-radius:6px;padding:8px 10px;margin-bottom:12px;font-size:11px;color:#ffc470;line-height:1.5;",
                "🔒 Только для админа. Сервисные записи не попадают в публичный каталог: значок «обслужен», который невозможно держать точным, — это как раз то число, которое запрещено показывать."
            }
            FormCard {
                title: "Добавить сервисную запись".to_string(),
                children: rsx!{
                    select { style: input_style(), value: "{bike_unit_id}",
                        "aria-label": "Юнит",
                        oninput: move |e| bike_unit_id.set(e.value()),
                        option { value: "", "— выберите юнит —" }
                        for u in unit_options.iter() {
                            option { key: "{u.id}", value: "{u.id}", "{u.unit_code}" }
                        }
                    }
                    select { style: input_style(), value: "{service_type}",
                        "aria-label": "Тип обслуживания",
                        oninput: move |e| service_type.set(e.value()),
                        option { value: "oil", "Масло двигателя" }
                        option { value: "gear_oil", "Масло редуктора" }
                        option { value: "abs", "ABS" }
                        option { value: "air_filter", "Воздушный фильтр" }
                        option { value: "__custom__", "➕ Другое" }
                    }
                    if custom_type {
                        input { style: input_style(), placeholder: "Свой тип (латиницей, например brake_pads)", value: "{service_type_custom}",
                            oninput: move |e| service_type_custom.set(e.value()) }
                    }
                    input { style: input_style(), placeholder: "Текущий км — можно пусто", value: "{current_km}", r#type: "number",
                        oninput: move |e| current_km.set(e.value()) }
                    input { style: input_style(), placeholder: "Км последнего ТО — можно пусто", value: "{last_service_km}", r#type: "number",
                        oninput: move |e| last_service_km.set(e.value()) }
                    input { style: input_style(), placeholder: "Интервал, км — можно пусто", value: "{interval_km}", r#type: "number",
                        oninput: move |e| interval_km.set(e.value()) }
                    input { style: input_style(), placeholder: "Следующее ТО, км — можно пусто", value: "{next_km}", r#type: "number",
                        oninput: move |e| next_km.set(e.value()) }
                    div { style: "font-size:11px;color:#8b8b9e;",
                        "Следующее ТО не считается автоматически: экран сохраняет то, что стоит в сервисной ведомости, и не придумывает числа."
                    }
                    select { style: input_style(), value: "{record_status}",
                        "aria-label": "Статус обслуживания",
                        oninput: move |e| record_status.set(e.value()),
                        option { value: "ok", "В норме" }
                        option { value: "due", "Пора" }
                        option { value: "overdue", "Просрочено" }
                    }
                    button {
                        style: if *submitting.read() { submit_btn_disabled_style() } else { submit_btn_style() },
                        disabled: *submitting.read(),
                        r#type: "button",
                        onclick: move |_| {
                            let uid = bike_unit_id().trim().to_string();
                            if uid.is_empty() { status.set("❌ Выберите юнит".into()); return; }
                            let kind = if *service_type.read() == "__custom__" {
                                service_type_custom().trim().to_lowercase()
                            } else {
                                service_type()
                            };
                            if kind.is_empty() { status.set("❌ Укажите тип обслуживания".into()); return; }
                            let cur = match int_input(&current_km.read(), "Текущий км", 0, 1_000_000) {
                                Ok(v) => v,
                                Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                            };
                            let last = match int_input(&last_service_km.read(), "Км последнего ТО", 0, 1_000_000) {
                                Ok(v) => v,
                                Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                            };
                            let interval = match int_input(&interval_km.read(), "Интервал", 1, 100_000) {
                                Ok(v) => v,
                                Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                            };
                            let next = match int_input(&next_km.read(), "Следующее ТО", 0, 1_000_000) {
                                Ok(v) => v,
                                Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                            };
                            let st = record_status();
                            submitting.set(true);
                            let temp_id = format!("temp-{}", uuid::Uuid::new_v4());
                            cache.write().insert(0, AdminServiceRecord {
                                id: temp_id.clone(), bike_unit_id: uid.clone(),
                                service_type: kind.clone(), current_km: cur,
                                last_service_km: last, interval_km: interval,
                                next_km: next, status: st.clone(),
                                // The server stamps the time; until it answers
                                // this row has no timestamp and shows a dash.
                                recorded_at: None,
                            });
                            status.set("✅ Записано!".into());
                            current_km.set(String::new()); last_service_km.set(String::new());
                            interval_km.set(String::new()); next_km.set(String::new());
                            service_type_custom.set(String::new());
                            auto_scroll_to_list();
                            spawn(async move {
                                let body = json!({
                                    "bike_unit_id": uid,
                                    "service_type": kind,
                                    "current_km": int_json(cur),
                                    "last_service_km": int_json(last),
                                    "interval_km": int_json(interval),
                                    "next_km": int_json(next),
                                    "status": st,
                                });
                                let url = format!("{}/api/admin/bike-service-records", api_base_url());
                                let res = HTTP_CLIENT.clone().post(&url)
                                    .header("X-Telegram-Init-Data", init_data.read().clone())
                                    .header("X-Admin-Token", admin_token())
                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                submitting.set(false);
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        if let Ok(data) = r.json::<serde_json::Value>().await {
                                            let real_id = data["id"].as_str().map(|s| s.to_string());
                                            let stamp = data["recorded_at"].as_str().map(|s| s.to_string());
                                            if let Some(rec) = cache.write().iter_mut().find(|rec| rec.id == temp_id) {
                                                if let Some(real_id) = real_id { rec.id = real_id; }
                                                rec.recorded_at = stamp;
                                            }
                                        }
                                        TelegramApp::init().haptic_notification(HapticNotification::Success);
                                    }
                                    _ => {
                                        cache.write().retain(|rec| rec.id != temp_id);
                                        status.set("❌ Ошибка записи".into());
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

            h3 { style: list_title_style(), "Сервис ({filtered.len()})" }
            div { style: "font-size:11px;color:#8b8b9e;margin-bottom:8px;",
                "Просрочено среди загруженных записей: {overdue_loaded}"
            }
            {render_search(search_query)}
            if *loading.read() {
                div { style: "display:flex;flex-direction:column;gap:4px;",
                    for _ in 0..4 {
                        div { style: "background:#1a1a2e;padding:8px 10px;border-radius:6px;display:flex;flex-direction:column;gap:4px;",
                            Skeleton { shape: SkeletonShape::Text, width: Some("40%".into()) }
                            Skeleton { shape: SkeletonShape::TextSm, width: Some("80%".into()) }
                        }
                    }
                }
            } else if filtered.is_empty() {
                EmptyState {
                    icon: "🔧",
                    title: "Записей нет",
                    description: "Сервисная ведомость заполняется здесь — добавьте первую запись",
                    action: rsx! {
                        button {
                            style: "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:13px;cursor:pointer;",
                            onclick: move |_| search_query.set(String::new()),
                            "Очистить поиск"
                        }
                    },
                }
            } else {
                div { "data-list": "true", style: "display:flex;flex-direction:column;gap:4px;",
                    for (rec, code) in filtered {
                        {
                            let (badge_label, badge_class) = service_status_label(rec.status.as_str());
                            let kind = service_type_label(rec.service_type.as_str());
                            let cur = km_or_dash(rec.current_km);
                            let last = km_or_dash(rec.last_service_km);
                            let interval = km_or_dash(rec.interval_km);
                            let next = km_or_dash(rec.next_km);
                            let stamp = rec.recorded_at.clone().unwrap_or_else(|| "—".to_string());
                            let id_for_delete = rec.id.clone();
                            rsx! {
                                div { key: "{rec.id}", style: "background:#1a1a2e;padding:8px 10px;border-radius:6px;display:flex;flex-direction:column;gap:6px;",
                                    div { style: "display:flex;align-items:center;gap:8px;",
                                        div { style: "flex:1;min-width:0;",
                                            div { style: "font-weight:700;font-size:13px;color:#e8e8e8;", "{code} • {kind}" }
                                            div { style: "font-size:11px;color:#888;", "Записано: {stamp}" }
                                        }
                                        span { class: "admin-badge {badge_class}", "{badge_label}" }
                                    }
                                    div { style: "font-size:11px;color:#8b8b9e;line-height:1.5;",
                                        "Сейчас: {cur} • Последнее ТО: {last} • Интервал: {interval} • Следующее: {next}"
                                    }
                                    div { style: "display:flex;justify-content:flex-end;",
                                        button {
                                            style: "min-width:44px;min-height:44px;padding:8px;background:#3a1a1a;color:#ff8888;border:none;border-radius:4px;font-size:16px;cursor:pointer;line-height:1;display:flex;align-items:center;justify-content:center;",
                                            "aria-label": "Удалить запись {code} {kind}",
                                            onclick: move |_| delete_target_id.set(Some(id_for_delete.clone())),
                                            "🗑"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            DeleteConfirmModal {
                target: delete_target_id,
                item_name: "сервисную запись".to_string(),
                on_confirm: move |id: String| {
                    let deleted = cache.read().iter().find(|rec| rec.id == id).cloned();
                    cache.write().retain(|rec| rec.id != id);
                    spawn(async move {
                        let url = format!("{}/api/admin/bike-service-records/{}", api_base_url(), id);
                        let res = HTTP_CLIENT.clone().delete(&url)
                            .header("X-Telegram-Init-Data", init_data.read().clone())
                            .header("X-Admin-Token", admin_token())
                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                            .send().await;
                        match res {
                            Ok(r) if r.status().is_success() => { push_toast(toasts, "Запись удалена".into(), ToastKind::Success); TelegramApp::init().haptic_notification(HapticNotification::Success); }
                            _ => {
                                if let Some(item) = deleted { cache.write().push(item); }
                                push_toast(toasts, "Не удалось удалить запись".into(), ToastKind::Error);
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
fn EditBikeCard(
    item: AdminBike,
    cache: Signal<Vec<AdminBike>>,
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut brand = use_signal(|| item.brand.clone());
    let mut model = use_signal(|| item.model.clone());
    let mut variant_label = use_signal(|| item.variant_label.clone().unwrap_or_default());
    let mut bike_class = use_signal(|| item.class.clone());
    let mut bike_body = use_signal(|| item.body.clone());
    let mut displacement_cc = use_signal(|| {
        item.displacement_cc
            .map(|v| v.to_string())
            .unwrap_or_default()
    });
    // Every money input starts empty when the column is NULL, and an empty
    // input saves back as NULL. That is the whole point of D9 here: clearing
    // a price must not force the admin to type a 0.
    let mut base_rate = use_signal(|| money_edit_value(item.base_rate_thb_day));
    let mut deposit = use_signal(|| money_edit_value(item.deposit_thb));
    let mut monthly = use_signal(|| money_edit_value(item.monthly_low_season_thb));
    let mut sale_price = use_signal(|| money_edit_value(item.sale_price_thb));
    let mut for_sale = use_signal(|| item.for_sale);
    let mut description_ru = use_signal(|| item.description_ru.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut image_url = use_signal(|| item.image_url.clone().unwrap_or_default());
    let mut sort_order = use_signal(|| item.sort_order.to_string());
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    let item_key = item.key.clone();
    // Read-only context, computed once: the key is the cart's `catalog_id`
    // and is not editable, and the client price belongs to the door (D11).
    let rate_note = client_rate_note(&item);
    let discount_note = match item.class_discount.filter(|v| v.is_finite()) {
        // A percentage of the published fraction — a unit change, not a new
        // number. The screen still never multiplies it into the tariff.
        Some(d) => format!("Скидка класса: {:.0}%", d * 100.0),
        None => "Скидка класса: —".to_string(),
    };
    rsx! {
           div { "data-editing": "true", style: edit_card_style(),
               div { style: edit_header_style(), "✏️ Редактирование" }
               div { style: "font-size:11px;color:#8b8b9e;line-height:1.5;",
                   "Ключ: {item_key} — не меняется, на него ссылаются корзины. {discount_note}. {rate_note}"
               }
               input { style: input_style(), placeholder: "Бренд", value: "{brand}", oninput: move |e| brand.set(e.value()) }
               input { style: input_style(), placeholder: "Модель", value: "{model}", oninput: move |e| model.set(e.value()) }
               input { style: input_style(), placeholder: "Вариант (NEW 2023+)", value: "{variant_label}", oninput: move |e| variant_label.set(e.value()) }
               select { style: input_style(), value: "{bike_class}", "aria-label": "Класс", oninput: move |e| bike_class.set(e.value()),
                   option { value: "scooter", "🛵 Скутер" } option { value: "motorcycle", "🏍 Мотоцикл" } }
               select { style: input_style(), value: "{bike_body}", "aria-label": "Тип кузова", oninput: move |e| bike_body.set(e.value()),
                   option { value: "scooter", "Скутер" } option { value: "maxi-scooter", "Макси-скутер" }
                   option { value: "adventure-scooter", "Адвенчер-скутер" } option { value: "naked", "Нейкед" }
                   option { value: "sport", "Спорт" } option { value: "cruiser", "Круизер" } }
               input { style: input_style(), placeholder: "Объём, см³", value: "{displacement_cc}", r#type: "number", oninput: move |e| displacement_cc.set(e.value()) }
               input { style: input_style(), placeholder: "Тариф ฿/сут ДО скидки — пусто = прочерк", value: "{base_rate}", r#type: "number", oninput: move |e| base_rate.set(e.value()) }
               input { style: input_style(), placeholder: "Депозит ฿ — пусто = прочерк", value: "{deposit}", r#type: "number", oninput: move |e| deposit.set(e.value()) }
               input { style: input_style(), placeholder: "Месяц, низкий сезон ฿ — пусто = прочерк", value: "{monthly}", r#type: "number", oninput: move |e| monthly.set(e.value()) }
               input { style: input_style(), placeholder: "Цена продажи ฿ — пусто = цена по запросу", value: "{sale_price}", r#type: "number", oninput: move |e| sale_price.set(e.value()) }
               // The forecourt flag, separate from the asking price on purpose:
               // "продаём, цена по запросу" is a real state and the shop must be
               // able to say it without inventing a number (D9/D11, issue #11).
               // Before this the two were one field — the placeholder above used
               // to read "пусто = не продаётся" — so a bike could only be
               // advertised by publishing a price for it.
               label { style: "display:flex;align-items:center;gap:8px;font-size:13px;color:#d8d8e4;cursor:pointer;",
                   input {
                       r#type: "checkbox",
                       checked: "{for_sale}",
                       onchange: move |e| for_sale.set(e.checked()),
                   }
                   "Продаётся"
               }
               div { style: "font-size:11px;color:#8b8b9e;",
                   "Пустое поле цены сохраняется как отсутствие цены и показывается прочерком. Ноль вводить не нужно и он не принимается. «Продаётся» без цены — это «цена по запросу»."
               }
               input { style: input_style(), placeholder: "Порядок сортировки", value: "{sort_order}", r#type: "number", oninput: move |e| sort_order.set(e.value()) }
               textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description_ru}", oninput: move |e| description_ru.set(e.value()) }
               ImageUpload { image_url: image_url.read().clone(), on_change: move |url: String| image_url.set(url) }
               div { style: en_section_style(), "🇬🇧 English" }
               textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}", oninput: move |e| description_en.set(e.value()) }
               div { style: "display:flex;gap:8px;",
                   button { style: submit_btn_style(),
                       onclick: move |_| {
                           let br = brand().trim().to_string();
                           let md = model().trim().to_string();
                           if br.is_empty() || md.is_empty() { status.set("❌ Бренд и модель обязательны".into()); return; }
                           let cc = match int_input(&displacement_cc.read(), "Объём", 49, 2000) {
                               Ok(v) => v,
                               Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                           };
                           let rate = match money_input(&base_rate.read(), "Тариф") {
                               Ok(v) => v,
                               Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                           };
                           let dep = match money_input(&deposit.read(), "Депозит") {
                               Ok(v) => v,
                               Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                           };
                           let mon = match money_input(&monthly.read(), "Месячная цена") {
                               Ok(v) => v,
                               Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                           };
                           let sale = match money_input(&sale_price.read(), "Цена продажи") {
                               Ok(v) => v,
                               Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                           };
                           let order = match crate::trios::validation::parse_int_in_range(&sort_order.read(), "Порядок", 0, 10_000) {
                               Ok(v) => v,
                               Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                           };
                           let cls = bike_class(); let bd = bike_body();
                           let vl = variant_label().trim().to_string();
                           let dru = description_ru(); let den = description_en();
                           let img = crate::trios::validation::normalize_media_url(&image_url());
                           let offered = item.offered;
                           let for_sale = for_sale();
                           let id = item_id.clone();
                           let original = cache.read().iter().find(|b| b.id == id).cloned();
                           if let Some(b) = cache.write().iter_mut().find(|b| b.id == id) {
                               b.brand = br.clone(); b.model = md.clone();
                               b.variant_label = if vl.is_empty() { None } else { Some(vl.clone()) };
                               b.class = cls.clone(); b.body = bd.clone();
                               b.displacement_cc = cc;
                               b.base_rate_thb_day = rate; b.deposit_thb = dep;
                               b.monthly_low_season_thb = mon; b.sale_price_thb = sale;
                               b.for_sale = for_sale;
                               b.sort_order = order;
                               b.description_ru = if dru.is_empty() { None } else { Some(dru.clone()) };
                               b.description_en = if den.is_empty() { None } else { Some(den.clone()) };
                               b.image_url = if img.is_empty() { None } else { Some(img.clone()) };
                               // The client price is the door's answer, not a
                               // consequence of this edit: leave it untouched
                               // rather than re-deriving it from the tariff.
                           }
                           spawn(async move {
                               let body = json!({
                                   "brand": br, "model": md,
                                   "variant_label": text_json(&vl),
                                   "class": cls, "body": bd,
                                   "displacement_cc": int_json(cc),
                                   "base_rate_thb_day": money_json(rate),
                                   "deposit_thb": money_json(dep),
                                   "monthly_low_season_thb": money_json(mon),
                                   "sale_price_thb": money_json(sale),
                                   "offered": offered, "for_sale": for_sale,
                                   "sort_order": order,
                                   "description_ru": text_json(&dru),
                                   "description_en": text_json(&den),
                                   "image_url": text_json(&img),
                               });
                               let url = format!("{}/api/admin/bikes/{}", api_base_url(), id);
                               let res = HTTP_CLIENT.clone().put(&url)
                                   .header("X-Telegram-Init-Data", init_data.read().clone())
    .header("X-Admin-Token", admin_token())
    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                   .json(&body).send().await;
                               // Same diagnosable outcome as the other edit
                               // cards: HTTP status and reason body beat an
                               // opaque "не сохранено".
                               let outcome = match res {
                                   Ok(r) if r.status().is_success() => Ok(()),
                                   Ok(r) => {
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
                                           if let Some(b) = cache.write().iter_mut().find(|b| b.id == id) { *b = orig; }
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
fn EditBikeUnitCard(
    item: AdminBikeUnit,
    families: Vec<AdminBike>,
    cache: Signal<Vec<AdminBikeUnit>>,
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut bike_id = use_signal(|| item.bike_id.clone());
    let mut unit_code = use_signal(|| item.unit_code.clone());
    let mut model_year = use_signal(|| item.model_year.map(|v| v.to_string()).unwrap_or_default());
    let mut color = use_signal(|| item.color.clone().unwrap_or_default());
    let mut km_since_purchase = use_signal(|| {
        item.km_since_purchase
            .map(|v| v.to_string())
            .unwrap_or_default()
    });
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    // Status is owned by the row's own select, so it travels through the save
    // unchanged instead of being edited in two places.
    let item_status = item.status.clone();
    rsx! {
           div { "data-editing": "true", style: edit_card_style(),
               div { style: edit_header_style(), "✏️ Редактирование" }
               select { style: input_style(), value: "{bike_id}", "aria-label": "Модель юнита", oninput: move |e| bike_id.set(e.value()),
                   option { value: "", "— выберите модель —" }
                   for f in families.iter() {
                       option { key: "{f.id}", value: "{f.id}", "{bike_title(f)} · {f.key}" }
                   }
               }
               input { style: input_style(), placeholder: "Код юнита (не номер!)", value: "{unit_code}", oninput: move |e| unit_code.set(e.value()) }
               input { style: input_style(), placeholder: "Год модели", value: "{model_year}", r#type: "number", oninput: move |e| model_year.set(e.value()) }
               input { style: input_style(), placeholder: "Цвет", value: "{color}", oninput: move |e| color.set(e.value()) }
               input { style: input_style(), placeholder: "Км с покупки", value: "{km_since_purchase}", r#type: "number", oninput: move |e| km_since_purchase.set(e.value()) }
               div { style: "font-size:11px;color:#8b8b9e;line-height:1.5;",
                   "Км с покупки — то, что записано в ведомости, а не показание одометра: у одной машины это 2900, а на приборе 16433. Пусто = неизвестно."
               }
               div { style: "display:flex;gap:8px;",
                   button { style: submit_btn_style(),
                       onclick: move |_| {
                           let bid = bike_id().trim().to_string();
                           let code = unit_code().trim().to_string();
                           if bid.is_empty() { status.set("❌ Выберите модель".into()); return; }
                           if code.is_empty() { status.set("❌ Код юнита обязателен".into()); return; }
                           let year = match int_input(&model_year.read(), "Год модели", 1980, 2100) {
                               Ok(v) => v,
                               Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                           };
                           let km = match int_input(&km_since_purchase.read(), "Км с покупки", 0, 1_000_000) {
                               Ok(v) => v,
                               Err(msg) => { status.set(format!("❌ {}", msg)); return; }
                           };
                           let col = color().trim().to_string();
                           let st = item_status.clone();
                           let id = item_id.clone();
                           let original = cache.read().iter().find(|u| u.id == id).cloned();
                           if let Some(u) = cache.write().iter_mut().find(|u| u.id == id) {
                               u.bike_id = bid.clone(); u.unit_code = code.clone();
                               u.model_year = year;
                               u.color = if col.is_empty() { None } else { Some(col.clone()) };
                               u.km_since_purchase = km;
                           }
                           spawn(async move {
                               let body = json!({
                                   "bike_id": bid, "unit_code": code,
                                   "model_year": int_json(year),
                                   "color": text_json(&col),
                                   "km_since_purchase": int_json(km),
                                   "status": st,
                               });
                               let url = format!("{}/api/admin/bike-units/{}", api_base_url(), id);
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
                                           if let Some(u) = cache.write().iter_mut().find(|u| u.id == id) { *u = orig; }
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

// ══════════════════════════════════════════════════════════════
// ── 7 NEW TABS ────────────────────────────────────────────────
// ══════════════════════════════════════════════════════════════

// ─── Dashboard ───────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct AdminStats {
    /// Every counter is optional and renders as a dash when the payload does
    /// not carry it (D9). A stats endpoint that dropped a field has not
    /// measured zero orders, and saying "0" would be a claim nobody made.
    #[serde(default)]
    total_orders: Option<i64>,
    #[serde(default)]
    total_revenue: Option<f64>,
    #[serde(default)]
    top_bikes: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    offered_families: Option<i64>,
    #[serde(default)]
    units_available: Option<i64>,
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
                    {stat_card("Всего заказов", count_or_dash(s.total_orders), "cyan")}
                    {stat_card("Выручка", revenue_thb(s.total_revenue), "")}
                    {stat_card("Моделей в прокате", count_or_dash(s.offered_families), "yellow")}
                    {stat_card("Свободных юнитов", count_or_dash(s.units_available), "")}
                }
                if let Some(top) = s.top_bikes.clone() {
                    if !top.is_empty() {
                        div { class: "admin-card",
                            h4 { class: "admin-card-meta", "🏍 Топ модели" }
                            for item in top {
                                {
                                    let name = item["name"].as_str().unwrap_or("—").to_string();
                                    // A row without a count is a row whose
                                    // count we were not told — a dash, not 0.
                                    let count = count_or_dash(item["count"].as_i64());
                                    rsx! {
                                        div { class: "admin-row",
                                            div { class: "admin-row-main", "{name}" }
                                            div { class: "admin-badge success", "{count}" }
                                        }
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
    subtotal: Option<f64>,
    #[serde(default)]
    bonus_used: Option<f64>,
    #[serde(default)]
    stars_used: Option<i64>,
    total: Option<f64>,
    status: String,
    #[serde(default)]
    shop_id: Option<String>,
    #[serde(rename = "created_at", default)]
    created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdminOrderItem {
    /// A line points at a bike **family**, because that is what a customer
    /// books (D8: `catalog_id` is the family key). The unit fields fill in
    /// later, when the shop has assigned a machine, and stay absent until
    /// then rather than guessing one.
    #[serde(default)]
    bike_id: Option<String>,
    #[serde(default)]
    bike_name: Option<String>,
    #[serde(default)]
    bike_unit_id: Option<String>,
    /// The shop's own short code for the assigned machine — never a plate (D14).
    #[serde(default)]
    bike_unit_code: Option<String>,
    #[serde(default)]
    accessory_id: Option<String>,
    #[serde(default)]
    accessory_name: Option<String>,
    #[serde(default)]
    quantity: f64,
    /// The rental window carried by the line (D8). Absent on a sale line.
    #[serde(default)]
    rental_start: Option<String>,
    #[serde(default)]
    rental_end: Option<String>,
    /// Rental or sale, spelled with two l's to match `OrderItem` (D7).
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
        order.id.get(0..4).unwrap_or(&order.id),
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
                                    let name = item.bike_name.clone()
                                        .or(item.accessory_name.clone())
                                        .unwrap_or_else(|| "Неизвестно".into());
                                    // The assigned machine appears only once
                                    // there is one: the customer booked a
                                    // family, not this unit (D8).
                                    let unit = match item.bike_unit_code.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
                                        Some(code) => format!(" · юнит {code}"),
                                        None => String::new(),
                                    };
                                    let f = match item.fulfillment.as_deref() {
                                        Some("rental") | Some("bike_rental") => " 🛞 аренда".to_string(),
                                        Some("sale") | Some("bike_sale") => " 🏷 продажа".to_string(),
                                        // An unknown value is shown as it came
                                        // rather than folded into one of the two.
                                        Some(other) if !other.trim().is_empty() => format!(" {other}"),
                                        _ => String::new(),
                                    };
                                    let window = rental_window(item.rental_start.as_deref(), item.rental_end.as_deref());
                                    format!("{} × {:.0}{}{}{}", name, item.quantity, unit, f, window)
                                }
                            }
                        }
                    }
                }
                div { style: "border-top:1px solid #2a2a4a;padding-top:12px;display:flex;flex-direction:column;gap:4px;",
                    div { style: "display:flex;justify-content:space-between;font-size:13px;color:#888;",
                        span { "Подытог" }
                        span { "{crate::trios::pricing::order_total_text(order.subtotal, crate::ui::components::bike_card::DASH)}" }
                    }
                    if order.bonus_used.is_none_or(|b| b > 0.0) {
                        div { style: "display:flex;justify-content:space-between;font-size:13px;color:#ffe600;",
                            span { "Бонусы" }
                            span { "{admin_discount_text(order.bonus_used)}" }
                        }
                    }
                    if order.stars_used.is_none_or(|s| s > 0) {
                        div { style: "display:flex;justify-content:space-between;font-size:13px;color:#7dd3fc;",
                            span { "⭐ Stars" }
                            span { "{admin_discount_text(order.stars_used.map(|s| s as f64))}" }
                        }
                    }
                    div { style: "display:flex;justify-content:space-between;font-size:16px;font-weight:700;color:#39ff14;",
                        span { "Итого" }
                        span { "{crate::trios::pricing::order_total_text(order.total, crate::ui::components::bike_card::DASH)}" }
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
                    i.bike_name
                        .clone()
                        .or(i.accessory_name.clone())
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
                admin_csv_money(o.subtotal),
                admin_csv_money(o.bonus_used),
                admin_csv_count(o.stars_used),
                admin_csv_money(o.total),
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
                                        "{admin_badge_total(order.total)}"
                                        if order.bonus_used.is_none_or(|b| b > 0.0) {
                                            span { class: "admin-badge warn",
                                                "{admin_badge_discount(order.bonus_used)} бонусов"
                                            }
                                        }
                                        if order.stars_used.is_none_or(|s| s > 0) {
                                            span { class: "admin-badge info",
                                                "{admin_badge_stars(order.stars_used)}⭐"
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
                    div { class: "admin-label", "🏴\u{200d}☠️ Стартовая точка (TurboBaby)" }
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
    let lang = crate::ui::lang::current_lang();
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
    // Not `mut`: the click handler copies the signal (`let mut status = …`) and
    // sets it through that copy, which writes the same underlying value.
    let send_list_status = use_signal(String::new);

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
        // Admin input is interpreted on the declared market's clock (D18), not
        // browser local time. The conversion is shared and tested
        // (`trios::calendar`) rather than a `format!` here — a datetime-local
        // value carrying seconds used to be concatenated into an unparseable
        // timestamp and rejected by the server with an error nobody could see.
        //
        // The offset was a literal, the second of the two sites D18's own list
        // of them missed. The DST fallback lives in `trios::market`, not here:
        // a default is a policy about the market.
        let shop_offset = crate::trios::market::MARKET.rfc3339_offset_or_utc();
        let shop_offset = shop_offset.as_str();
        let starts_at_api =
            match crate::trios::calendar::datetime_local_to_rfc3339(&starts_at.read(), shop_offset)
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
            match crate::trios::calendar::datetime_local_to_rfc3339(&ends_at_raw, shop_offset) {
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

    // The one copy of the failed-delete label. It used to be three arms' worth
    // of the same literal; the wording is unchanged, and what follows the colon
    // is what now tells the three failures apart.
    const DELETE_FAILED: &str = "Ошибка удаления";

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
                    // The server explains itself on a refused delete: 409
                    // `paid_bookings_exist` carries the count and the total it
                    // measured, and 409 `acknowledgement_is_stale` carries the
                    // count the caller answered for as well. Collapsing those
                    // into one generic toast made "three people paid for
                    // this event" and "the connection dropped" the same
                    // sentence, and the owner's next move is different for
                    // each. The reading is `admin_event_delete_failure`
                    // (src/trios/api_errors.rs), which is a trio because
                    // nothing under src/ui is compiled by cargo test.
                    Ok(r) => {
                        let status = r.status().as_u16();
                        let body = r.text().await.unwrap_or_default();
                        let msg = match admin_event_delete_failure(lang, status, &body) {
                            Some(detail) => format!("{DELETE_FAILED} ({status}): {detail}"),
                            None => format!("{DELETE_FAILED} ({status})"),
                        };
                        push_toast(toasts, msg, ToastKind::Error);
                    }
                    // Nothing was answered at all, which is the case the
                    // arm above used to be mistaken for.
                    Err(e) => {
                        push_toast(toasts, format!("{DELETE_FAILED}: {e}"), ToastKind::Error);
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
                                crate::trios::pricing::format_baht(p)
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
            title: if editing_id.read().as_deref() == Some("") { Some("Новое событие".to_string()) } else { Some("Редактировать событие".to_string()) },
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

/// One catalog now that the shop sells and rents one thing. It stays an enum
/// because `BroadcastProduct` carries a `kind` string on the wire and the
/// picker still has to send one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BroadcastCatalog {
    Bikes,
}

impl BroadcastCatalog {
    fn from_value(s: &str) -> Option<Self> {
        match s {
            "bike" => Some(Self::Bikes),
            _ => None,
        }
    }
    fn value(self) -> &'static str {
        match self {
            Self::Bikes => "bike",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Bikes => "🏍 Байки",
        }
    }
}

/// Loads the picker rows. Only families that are on offer are listed: a
/// broadcast about a model the shop has closed (D12) advertises something
/// nobody can book. The label is the model name and nothing else — a price
/// in a broadcast is a price this screen would be inventing (D11).
async fn load_broadcast_products(
    catalog: BroadcastCatalog,
) -> Result<Vec<BroadcastPickerProduct>, String> {
    let base = api_base_url();
    let path = match catalog {
        BroadcastCatalog::Bikes => "/api/bikes",
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
        BroadcastCatalog::Bikes => {
            #[derive(Deserialize)]
            struct R {
                bikes: Vec<AdminBike>,
            }
            resp.json::<R>()
                .await
                .map_err(|e| format!("json: {e}"))?
                .bikes
                .into_iter()
                .filter(|b| b.offered)
                .map(|b| BroadcastPickerProduct {
                    name: bike_title(&b),
                    id: b.id,
                    image_url: b.image_url.unwrap_or_default(),
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
    let mut text = use_signal(String::new);
    let mut photo_url = use_signal(String::new);
    let mut catalog = use_signal(|| "none".to_string());
    let mut selected_product = use_signal(|| None::<BroadcastPickerProduct>);
    let mut button_text = use_signal(String::new);
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
                    option { value: "{BroadcastCatalog::Bikes.value()}", {BroadcastCatalog::Bikes.label()} }
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
                        { {
                            let label = {
                                let txt = button_text();
                                let s = txt.trim();
                                if s.is_empty() { "Открыть в магазине".to_string() } else { s.to_string() }
                            };
                            // No bike deep link exists yet, and printing a URL
                            // the bot cannot open would be a preview of
                            // something that will not happen. The chosen model
                            // is named instead of a link being guessed.
                            let picked = p.name.clone();
                            rsx! {
                                div { style: "margin-top:10px;display:flex;flex-direction:column;gap:6px;",
                                    button {
                                        style: "padding:10px 14px;background:#00e5ff;color:#000;border:none;border-radius:6px;font-weight:700;font-size:13px;cursor:default;",
                                        disabled: true,
                                        "{label}"
                                    }
                                    div { style: "font-size:11px;color:#8b8b9e;word-break:break-all;", "Кнопка ведёт в каталог: {picked}" }
                                }
                            }
                        } }
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

// Legacy admin broadcast form: it threads the ten signals it touches by hand,
// which is ugly but honest. Bundling them into a params struct is a separate
// refactor, not a lint-sweep change.
#[allow(clippy::too_many_arguments)]
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

// T27 defect 2 (D9): the order endpoints send `null` for a figure the server
// withheld (`src/db/orders.rs`, `money_withheld`), so the admin card, badges
// and CSV print the dash or an empty cell, never a substituted zero. These sit
// at the end of the file so that no line cited above them moves.

/// A card discount row: the stored amount with a minus sign, or the dash. The
/// star row passes its count here too and so prints it through the baht
/// formatter, exactly as it did before this change (order_money.t27 records it).
fn admin_discount_text(amount: Option<f64>) -> String {
    match crate::trios::pricing::measured_money(amount) {
        Some(v) => format!("-{}", crate::trios::pricing::format_baht(v)),
        None => crate::ui::components::bike_card::DASH.to_string(),
    }
}

/// The list's total badge: whole baht, or the dash.
fn admin_badge_total(amount: Option<f64>) -> String {
    match crate::trios::pricing::measured_money(amount) {
        Some(v) => format!("{v:.0}Б"),
        None => crate::ui::components::bike_card::DASH.to_string(),
    }
}

/// The list's bonus badge: whole baht with a minus sign, or the dash.
fn admin_badge_discount(amount: Option<f64>) -> String {
    match crate::trios::pricing::measured_money(amount) {
        Some(v) => format!("-{v:.0}Б"),
        None => crate::ui::components::bike_card::DASH.to_string(),
    }
}

/// The list's star badge: the count with a minus sign, or the dash.
fn admin_badge_stars(count: Option<i64>) -> String {
    match count.filter(|s| *s >= 0) {
        Some(s) => format!("-{s}"),
        None => crate::ui::components::bike_card::DASH.to_string(),
    }
}

/// A CSV money cell: the measured figure, or an empty cell -- never a zero.
fn admin_csv_money(amount: Option<f64>) -> String {
    crate::trios::pricing::measured_money(amount).map_or_else(String::new, |v| v.to_string())
}

/// A CSV star cell: the count, or an empty cell -- never a zero.
fn admin_csv_count(count: Option<i64>) -> String {
    count
        .filter(|s| *s >= 0)
        .map_or_else(String::new, |v| v.to_string())
}

/// Why an admin Add was refused, for the line under the form (epic #31 AC4,
/// 2026-09-24). Both Add buttons used to show «❌ Ошибка добавления» and nothing
/// else, although the server answers a duplicate key with a 409 and a sentence
/// of its own (`src/api/bikes.rs`, `admin_create_bike`). The words are the edit
/// cards' own -- `HTTP {st}: {snip}`, `HTTP {st}`, `сеть/таймаут` -- so no new
/// copy enters the screen. Called only for an answer that is not a 2xx. Sits at
/// the end of the file so that no line cited above it moves.
async fn admin_add_failure_reason(
    res: Result<
        crate::ui::api::local_client::LocalResponse,
        crate::ui::api::local_client::LocalError,
    >,
) -> String {
    match res {
        Ok(r) => {
            let st = r.status().as_u16();
            let body = r.text().await.unwrap_or_default();
            let b = body.trim();
            if b.is_empty() {
                format!("HTTP {st}")
            } else {
                let snip: String = b.chars().take(100).collect();
                format!("HTTP {st}: {snip}")
            }
        }
        Err(_) => "сеть/таймаут".to_string(),
    }
}
