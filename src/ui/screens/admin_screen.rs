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
use wasm_bindgen::JsCast;
use crate::ui::api::context::api_base_url;
use crate::ui::components::{EmptyState, Modal, Toast, ToastKind, ToastContainer, Skeleton, SkeletonShape};
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data, TelegramApp, HapticNotification};

#[derive(Clone)]
struct ToastItem { id: u64, message: String, kind: ToastKind }

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
    if items.is_empty() { return rsx! {}; }
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
    #[serde(default)]
    video_url: Option<String>,
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
    name_en: Option<String>,
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
    name_en: Option<String>,
    description_en: Option<String>,
    #[serde(default)]
    video_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StrainsResp { strains: Vec<AdminStrain> }
#[derive(Debug, Deserialize)]
struct AccessoriesResp { accessories: Vec<AdminAccessory> }
#[derive(Debug, Deserialize)]
struct TeaResp { tea_products: Vec<AdminTea> }
#[derive(Debug, Deserialize)]
struct SetsResp { sets: Vec<AdminSet> }
#[derive(Debug, Deserialize)]
struct AccessorySetsResp { accessory_sets: Vec<AdminAccessorySet> }
#[derive(Debug, Deserialize)]
struct TeaSetsResp { tea_sets: Vec<AdminTeaSet> }
#[derive(Debug, Deserialize)]
struct AdminCheck { is_admin: bool }

#[derive(Clone, Copy, PartialEq)]
enum Tab { Strains, Accessories, Tea, Sets, AccessorySets, TeaSets, Dashboard, Orders, Quests, Treasures, Garden, Loyalty, Managers }

// ── File upload helper ─────────────────────────────────────────

async fn upload_file(accept: &str) -> Option<String> {
    let accept = accept.to_string();
    let js = format!(r#"
new Promise((resolve) => {{
    var input = document.createElement('input');
    input.type = 'file';
    input.accept = '{}';
    var resolved = false;
    input.onchange = async (e) => {{
        if (resolved) return;
        resolved = true;
        var file = e.target.files[0];
        if (!file) {{ resolve(''); return; }}
        var formData = new FormData();
        formData.append('file', file);
        try {{
            var baseUrl = window.location.origin;
            var resp = await fetch(baseUrl + '/api/upload', {{ method: 'POST', body: formData }});
            var data = await resp.json();
            resolve(data.url || '');
        }} catch(err) {{ console.error('Upload error:', err); resolve(''); }}
    }};
    setTimeout(() => {{ if (!resolved) {{ resolved = true; resolve(''); }} }}, 120000);
    input.click();
}})
"#, accept);
    let promise_val = js_sys::eval(&js).ok()?;
    let promise = promise_val.dyn_into::<js_sys::Promise>().ok()?;
    let result = wasm_bindgen_futures::JsFuture::from(promise).await.ok()?;
    let url = result.as_string()?;
    if url.is_empty() { return None; }
    Some(url)
}

async fn upload_image() -> Option<String> { upload_file("image/*").await }
async fn upload_video() -> Option<String> { upload_file("video/*").await }

// ── Main component ────────────────────────────────────────────

#[component]
pub fn AdminScreen() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let active_tab = use_signal(|| Tab::Strains);
    let build_version: &'static str = env!("BUILD_VERSION");
    let init_data = use_telegram_init_data();

    #[cfg(target_arch = "wasm32")]
    {
        web_sys::console::log_1(&format!("[WWB Admin] telegram_id={}, init_data_len={}, init_data_preview={}", telegram_id, init_data.len(), &init_data[..init_data.len().min(80)]).into());
    }

    let password_token = use_signal(|| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("wwb_admin_token") {
                        return token;
                    }
                }
            }
        }
        String::new()
    });

    let access = use_resource(move || {
        let init_data = init_data.clone();
        let token = password_token.read().clone();
        async move {
            let base = api_base_url();
            let url = format!("{}/api/admin/check?telegram_id={}", base, telegram_id);
            let mut req = reqwest::Client::new().get(&url)
                .header("X-Telegram-Init-Data", init_data.clone());
            if !token.is_empty() {
                req = req.header("X-Admin-Token", token);
            }
            let resp = req.send().await
                .map_err(|e| format!("send to {url}: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("http {} from {url}", status.as_u16()));
        }
            resp.json::<AdminCheck>().await
                .map(|c| c.is_admin)
                .map_err(|e| format!("json: {e}"))
        }
    });

    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding:16px;padding-bottom:80px;",
            h1 { style: "font-size:22px;color:#ff4757;margin-bottom:4px;", "🔧 Admin" }
            div { style: "font-size:11px;color:#666;margin-bottom:4px;", "tg: {telegram_id}" }
            div { style: "font-size:10px;color:#444;margin-bottom:16px;font-family:monospace;", "v{build_version}" }
            match &*access.read() {
                None => rsx!(div { style: "color:#888;padding:20px 0;", "Проверка доступа..." }),
                Some(Err(e)) => rsx!(div { style: "color:#ff4757;padding:20px 0;", "Ошибка: {e}" }),
                Some(Ok(false)) => rsx!(AccessDeniedScreen { telegram_id, password_token }),
                Some(Ok(true)) => rsx!(AdminPanel { active_tab, password_token }),
            }
        }
    }
}

#[component]
fn AccessDeniedScreen(telegram_id: i64, mut password_token: Signal<String>) -> Element {
    let debug = TelegramApp::init().debug_dump();
    let mut password = use_signal(String::new);
    let mut error = use_signal(String::new);
    let mut logging_in = use_signal(|| false);
    rsx! {
        div { style: "padding:30px 16px;text-align:center;",
            div { style: "font-size:48px;margin-bottom:12px;", "🔒" }
            h2 { style: "color:#ff4757;font-size:18px;margin-bottom:8px;", "Доступ закрыт" }
            p { style: "color:#888;font-size:13px;line-height:1.5;max-width:300px;margin:0 auto;",
                "Попросите владельца добавить ваш Telegram ID в админы." }
            div { style: "margin-top:20px;padding:12px;background:#1a1a2e;border-radius:8px;font-family:monospace;font-size:13px;color:#39ff14;display:inline-block;",
                "ID: {telegram_id}" }
            div { style: "margin-top:20px;max-width:300px;margin-left:auto;margin-right:auto;",
                input { style: input_style(), r#type: "password", placeholder: "Пароль админа",
                    value: "{password}", oninput: move |e| password.set(e.value()) }
                if !error.read().is_empty() {
                    div { style: "color:#ff4757;font-size:12px;margin-top:4px;", "{error}" }
                }
                button {
                    style: if *logging_in.read() { submit_btn_disabled_style() } else { submit_btn_style() },
                    disabled: *logging_in.read(),
                    onclick: move |_| {
                        let pw = password.read().trim().to_string();
                        if pw.is_empty() { error.set("Введите пароль".into()); return; }
                        logging_in.set(true);
                        error.set(String::new());
                        let mut token_signal = password_token.clone();
                        let mut error2 = error.clone();
                        let mut logging_in2 = logging_in.clone();
                        spawn(async move {
                            let url = format!("{}/api/admin/login", api_base_url());
                            let res = reqwest::Client::new().post(&url)
                                .json(&serde_json::json!({"password": pw}))
                                .send().await;
                            logging_in2.set(false);
                            match res {
                                Ok(r) if r.status().is_success() => {
                                    if let Ok(data) = r.json::<serde_json::Value>().await {
                                        if let Some(token) = data["token"].as_str() {
                                            #[cfg(target_arch = "wasm32")]
                                            {
                                                if let Some(window) = web_sys::window() {
                                                    if let Ok(Some(storage)) = window.local_storage() {
                                                        let _ = storage.set_item("wwb_admin_token", token);
                                                    }
                                                }
                                            }
                                            token_signal.set(token.to_string());
                                        }
                                    }
                                }
                                _ => { error2.set("Неверный пароль".into()); }
                            }
                        });
                    },
                    if *logging_in.read() { "⏳..." } else { "🔑 Войти по паролю" }
                }
            }
            details { style: "margin-top:16px;text-align:left;max-width:340px;margin-left:auto;margin-right:auto;",
                summary { style: "color:#666;font-size:11px;cursor:pointer;", "debug" }
                pre { style: "font-size:10px;color:#888;background:#1a1a2e;padding:8px;border-radius:6px;white-space:pre-wrap;word-break:break-all;",
                    "{debug}" }
            }
        }
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
    rsx! {
        div {
            div { style: "display:flex;justify-content:flex-end;margin-bottom:8px;",
                button {
                    style: "padding:6px 12px;background:#2a2a4a;color:#888;border:none;border-radius:4px;font-size:12px;cursor:pointer;",
                    onclick: move |_| {
                        #[cfg(target_arch = "wasm32")]
                        {
                            if let Some(window) = web_sys::window() {
                                if let Ok(Some(storage)) = window.local_storage() {
                                    let _ = storage.remove_item("wwb_admin_token");
                                }
                            }
                        }
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
                {tab_btn(Tab::Quests, "🗺️ Квесты")}
                {tab_btn(Tab::Treasures, "🏴\u{200d}☠️ Сокровища")}
                {tab_btn(Tab::Garden, "🌱 Сад")}
                {tab_btn(Tab::Loyalty, "💎 Лояльность")}
                {tab_btn(Tab::Managers, "👥 Менеджеры")}
            }
            match *active_tab.read() {
                Tab::Strains => rsx!(StrainsTab {}),
                Tab::Accessories => rsx!(AccessoriesTab {}),
                Tab::Tea => rsx!(TeaTab {}),
                Tab::Sets => rsx!(SetsTab {}),
                Tab::AccessorySets => rsx!(AccessorySetsTab {}),
                Tab::TeaSets => rsx!(TeaSetsTab {}),
                Tab::Dashboard => rsx!(DashboardTab {}),
                Tab::Orders => rsx!(OrdersTab {}),
                Tab::Quests => rsx!(QuestsTab {}),
                Tab::Treasures => rsx!(TreasuresTab {}),
                Tab::Garden => rsx!(GardenTab {}),
                Tab::Loyalty => rsx!(LoyaltyTab {}),
                Tab::Managers => rsx!(ManagersTab {}),
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
                        if let Ok(data) = serde_json::from_str::<Vec<AdminStrain>>(&json) {
                            return data;
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
                    let _ = storage.set_item("wwb_admin_strains", &serde_json::to_string(&data).unwrap_or_default());
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
        if let Ok(resp) = reqwest::Client::new()
            .get(&url)
            .header("X-Telegram-Init-Data", init_data.clone())
            .header("X-Admin-Telegram-Id", telegram_id.to_string())
            .send().await
        {
            if let Ok(data) = resp.json::<StrainsResp>().await {
                cache.set(data.strains);
            }
        }
        loading.set(false);
        Some(())
    }});

    let filtered: Vec<AdminStrain> = {
        let q = search_query.read().to_lowercase();
        cache.read().iter().filter(|s| {
            q.is_empty() || s.name.to_lowercase().contains(&q)
                || s.category.as_deref().unwrap_or("").to_lowercase().contains(&q)
        }).cloned().collect()
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
                    {render_image_upload(image_url)}
                    {render_video_upload(video_url)}
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
                                Ok(v) if v > 0.0 => v,
                                _ => { status.set("❌ Цена должна быть числом больше 0".into()); return; }
                            };
                            let t = thc.read().trim().parse::<f64>().ok();
                            let cb = cbd.read().trim().parse::<f64>().ok();
                            let g = match grams.read().trim().parse::<f64>() {
                                Ok(v) if v >= 0.0 => v,
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
                                    "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                    "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                    "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                    "effect_en": if ee.is_empty() { serde_json::Value::Null } else { ee.into() },
                                    "flavor_profile_en": if fpe.is_empty() { serde_json::Value::Null } else { fpe.into() },
                                    "strain_type_en": if ste.is_empty() { serde_json::Value::Null } else { ste.into() },
                                });
                                let url = format!("{}/api/strains", api_base_url());
                                let res = reqwest::Client::new().post(&url)
                                    .header("X-Telegram-Init-Data", init_data.read().clone())
 .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                submitting.set(false);
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        if let Ok(data) = r.json::<serde_json::Value>().await {
                                            if let Some(real_id) = data["id"].as_str() {
                                                cache.write().iter_mut().find(|s| s.id == temp_id).map(|s| s.id = real_id.to_string());
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

            h3 { style: list_title_style(), "Страйны ({filtered.len()})" }
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
                                        cache.write().iter_mut().find(|s| s.id == id).map(|s| s.is_available = next_avail);
                                        spawn(async move {
                                            let url = format!("{}/api/strains/{}/availability", api_base_url(), id);
                                            let res = reqwest::Client::new().put(&url)
                                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                .json(&json!({ "is_available": next_avail }))
                                                .send().await;
                                            match res {
                                                Ok(r) if r.status().is_success() => { TelegramApp::init().haptic_notification(HapticNotification::Success); }
                                                _ => {
                                                    cache.write().iter_mut().find(|s| s.id == id).map(|s| s.is_available = !next_avail);
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
                        let res = reqwest::Client::new().delete(&url)
                            .header("X-Telegram-Init-Data", init_data.read().clone())
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
                        if let Ok(data) = serde_json::from_str::<Vec<AdminAccessory>>(&json) {
                            return data;
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
                    let _ = storage.set_item("wwb_admin_accessories", &serde_json::to_string(&data).unwrap_or_default());
                }
            }
        }
    });

    let _ = use_resource(move || {
        let _ = reload.read();
        let init_data = init_data.read().clone();
        async move {
        let url = format!("{}/api/accessories?include_hidden=1", api_base_url());
        if let Ok(resp) = reqwest::Client::new()
            .get(&url)
            .header("X-Telegram-Init-Data", init_data.clone())
            .header("X-Admin-Telegram-Id", telegram_id.to_string())
            .send().await
        {
            if let Ok(data) = resp.json::<AccessoriesResp>().await {
                cache.set(data.accessories);
            }
        }
        loading.set(false);
        Some(())
    }});

    let filtered: Vec<AdminAccessory> = {
        let q = search_query.read().to_lowercase();
        cache.read().iter().filter(|a| {
            q.is_empty() || a.name.to_lowercase().contains(&q)
                || a.category.as_deref().unwrap_or("").to_lowercase().contains(&q)
        }).cloned().collect()
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
                    {render_image_upload(image_url)}
                    {render_video_upload(video_url)}
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
                            let n = name(); let c = category(); let p = price.read().parse::<f64>().unwrap_or(0.0);
                            let s_val = stock.read().parse::<i32>().unwrap_or(0);
                            let d = description(); let img = image_url(); let vid = video_url();
                            let ne = name_en(); let de = description_en(); let ce = category_en();
                            if n.trim().is_empty() || p <= 0.0 { status.set("❌ Name + price".into()); return; }
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
                                    "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                    "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                    "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                    "category_en": if ce.is_empty() { serde_json::Value::Null } else { ce.into() },
                                });
                                let url = format!("{}/api/accessories", api_base_url());
                                let res = reqwest::Client::new().post(&url)
                                    .header("X-Telegram-Init-Data", init_data.read().clone())
 .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                submitting.set(false);
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        if let Ok(data) = r.json::<serde_json::Value>().await {
                                            if let Some(real_id) = data["id"].as_str() {
                                                cache.write().iter_mut().find(|a| a.id == temp_id).map(|a| a.id = real_id.to_string());
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
                                        cache.write().iter_mut().find(|a| a.id == id).map(|a| a.is_available = next);
                                        spawn(async move {
                                            let url = format!("{}/api/accessories/{}/availability", api_base_url(), id);
                                            let res = reqwest::Client::new().put(&url)
                                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                .json(&json!({ "is_available": next })).send().await;
                                            match res {
                                                Ok(r) if r.status().is_success() => { TelegramApp::init().haptic_notification(HapticNotification::Success); }
                                                _ => {
                                                    cache.write().iter_mut().find(|a| a.id == id).map(|a| a.is_available = !next);
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
                        let res = reqwest::Client::new().delete(&url)
                            .header("X-Telegram-Init-Data", init_data.read().clone())
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
                        if let Ok(data) = serde_json::from_str::<Vec<AdminTea>>(&json) {
                            return data;
                        }
                    }
                }
            }
        }
        Vec::new()
    });
    let mut loading = use_signal(|| true);
    let mut name = use_signal(String::new);
    let mut subcategory = use_signal(|| "green".to_string());
    let mut price = use_signal(String::new);
    let mut stock = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut image_url = use_signal(String::new);
    let mut video_url = use_signal(String::new);
    let mut name_en = use_signal(String::new);
    let mut description_en = use_signal(String::new);
    let mut subcategory_en = use_signal(String::new);
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
                    let _ = storage.set_item("wwb_admin_tea", &serde_json::to_string(&data).unwrap_or_default());
                }
            }
        }
    });

    let _ = use_resource(move || {
        let _ = reload.read();
        let init_data = init_data.read().clone();
        async move {
        let url = format!("{}/api/tea-products?include_hidden=1", api_base_url());
        if let Ok(resp) = reqwest::Client::new()
            .get(&url)
            .header("X-Telegram-Init-Data", init_data.clone())
            .header("X-Admin-Telegram-Id", telegram_id.to_string())
            .send().await
        {
            if let Ok(data) = resp.json::<TeaResp>().await {
                cache.set(data.tea_products);
            }
        }
        loading.set(false);
        Some(())
    }});

    let filtered: Vec<AdminTea> = {
        let q = search_query.read().to_lowercase();
        cache.read().iter().filter(|t| {
            q.is_empty() || t.name.to_lowercase().contains(&q)
                || t.subcategory.as_deref().unwrap_or("").to_lowercase().contains(&q)
        }).cloned().collect()
    };

    rsx! {
        div {
            FormCard {
                title: "Добавить чай".to_string(),
                children: rsx!{
                    input { style: input_style(), placeholder: "Название (RU)", value: "{name}",
                        oninput: move |e| name.set(e.value()) }
                    select { style: input_style(), value: "{subcategory}",
                        oninput: move |e| subcategory.set(e.value()),
                        option { value: "green", "🍃 Green" }
                        option { value: "black", "🖤 Black" }
                        option { value: "herbal", "🌿 Herbal" }
                        option { value: "oolong", "🍂 Oolong" }
                        option { value: "puer", "🟫 Pu-er" }
                        option { value: "other", "🍵 Other" }
                    }
                    input { style: input_style(), placeholder: "Цена ฿", value: "{price}", r#type: "number",
                        oninput: move |e| price.set(e.value()) }
                    input { style: input_style(), placeholder: "Кол-во", value: "{stock}", r#type: "number",
                        oninput: move |e| stock.set(e.value()) }
                    textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description}",
                        oninput: move |e| description.set(e.value()) }
                    {render_image_upload(image_url)}
                    {render_video_upload(video_url)}
                    div { style: en_section_style(), "🇬🇧 English" }
                    input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}",
                        oninput: move |e| name_en.set(e.value()) }
                    textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}",
                        oninput: move |e| description_en.set(e.value()) }
                    input { style: input_style(), placeholder: "Subcategory (EN)", value: "{subcategory_en}",
                        oninput: move |e| subcategory_en.set(e.value()) }
                    button {
                        style: if *submitting.read() { submit_btn_disabled_style() } else { submit_btn_style() },
                        disabled: *submitting.read(),
                        onclick: move |_| {
                            let n = name(); let sc = subcategory(); let p = price.read().parse::<f64>().unwrap_or(0.0);
                            let s_val = stock.read().parse::<i32>().unwrap_or(0);
                            let d = description(); let img = image_url(); let vid = video_url();
                            let ne = name_en(); let de = description_en(); let sce = subcategory_en();
                            if n.trim().is_empty() || p <= 0.0 { status.set("❌ Name + price".into()); return; }
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
                            name_en.set(String::new()); description_en.set(String::new()); subcategory_en.set(String::new());
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
                                let res = reqwest::Client::new().post(&url)
                                    .header("X-Telegram-Init-Data", init_data.read().clone())
 .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                submitting.set(false);
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        if let Ok(data) = r.json::<serde_json::Value>().await {
                                            if let Some(real_id) = data["id"].as_str() {
                                                cache.write().iter_mut().find(|t| t.id == temp_id).map(|t| t.id = real_id.to_string());
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
                                        cache.write().iter_mut().find(|t| t.id == id).map(|t| t.is_available = next);
                                        spawn(async move {
                                            let url = format!("{}/api/tea-products/{}/availability", api_base_url(), id);
                                            let res = reqwest::Client::new().put(&url)
                                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                .json(&json!({ "is_available": next })).send().await;
                                            match res {
                                                Ok(r) if r.status().is_success() => { TelegramApp::init().haptic_notification(HapticNotification::Success); }
                                                _ => {
                                                    cache.write().iter_mut().find(|t| t.id == id).map(|t| t.is_available = !next);
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
                        let res = reqwest::Client::new().delete(&url)
                            .header("X-Telegram-Init-Data", init_data.read().clone())
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
                        if let Ok(data) = serde_json::from_str::<Vec<AdminSet>>(&json) {
                            return data;
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
    let mut video_url = use_signal(String::new);
    let mut strain_ids = use_signal(String::new);
    let mut accessory_ids = use_signal(String::new);
    let mut total_price = use_signal(String::new);
    let mut discount_percent = use_signal(String::new);
    let mut is_deal_of_day = use_signal(|| false);
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

    use_effect(move || {
        let data = cache.read().clone();
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item("wwb_admin_sets", &serde_json::to_string(&data).unwrap_or_default());
                }
            }
        }
    });

    let _ = use_resource(move || {
        let _ = reload.read();
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/sets?include_hidden=1", api_base_url());
            if let Ok(resp) = reqwest::Client::new()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send().await
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
        cache.read().iter().filter(|s| {
            q.is_empty() || s.name.to_lowercase().contains(&q)
        }).cloned().collect()
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
                    {render_video_upload(video_url)}
                    textarea { style: textarea_style(), placeholder: "Strain IDs (через запятую)", value: "{strain_ids}",
                        oninput: move |e| strain_ids.set(e.value()) }
                    textarea { style: textarea_style(), placeholder: "Accessory IDs (через запятую)", value: "{accessory_ids}",
                        oninput: move |e| accessory_ids.set(e.value()) }
                    input { style: input_style(), placeholder: "Цена ฿", value: "{total_price}", r#type: "number",
                        oninput: move |e| total_price.set(e.value()) }
                    input { style: input_style(), placeholder: "Скидка %", value: "{discount_percent}", r#type: "number",
                        oninput: move |e| discount_percent.set(e.value()) }
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
                                Ok(v) if v >= 0.0 => v,
                                _ => { status.set("❌ Цена должна быть числом ≥ 0".into()); return; }
                            };
                            let d = discount_percent.read().trim().parse::<f64>().unwrap_or(0.0);
                            if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                            let s_ids: Vec<String> = strain_ids().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                            let a_ids: Vec<String> = accessory_ids().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                            let desc = description();
                            let ic = icon(); let vid = video_url();
                            let deal = is_deal_of_day();
                            submitting.set(true);
                            let temp_id = format!("temp-{}", uuid::Uuid::new_v4());
                            cache.write().insert(0, AdminSet {
                                id: temp_id.clone(), name: n.clone(),
                                description: if desc.is_empty() { None } else { Some(desc.clone()) },
                                icon: if ic.is_empty() { None } else { Some(ic.clone()) },
                                video_url: if vid.is_empty() { None } else { Some(vid.clone()) },
                                strain_ids: s_ids.clone(), accessory_ids: a_ids.clone(),
                                total_price: p, discount_percent: d,
                                is_available: true, is_deal_of_day: deal,
                            });
                            status.set("✅ Добавлен!".into());
                            name.set(String::new()); description.set(String::new()); icon.set(String::new());
                            video_url.set(String::new());
                            strain_ids.set(String::new()); accessory_ids.set(String::new());
                            total_price.set(String::new()); discount_percent.set(String::new());
                            is_deal_of_day.set(false);
                            auto_scroll_to_list();
                            spawn(async move {
                                let body = json!({
                                    "name": n, "total_price": p, "discount_percent": d,
                                    "description": if desc.is_empty() { serde_json::Value::Null } else { desc.into() },
                                    "icon": if ic.is_empty() { serde_json::Value::Null } else { ic.into() },
                                    "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                    "strain_ids": s_ids, "accessory_ids": a_ids,
                                    "is_deal_of_day": deal,
                                });
                                let url = format!("{}/api/sets", api_base_url());
                                let res = reqwest::Client::new().post(&url)
                                    .header("X-Telegram-Init-Data", init_data.read().clone())
                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                submitting.set(false);
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        if let Ok(data) = r.json::<serde_json::Value>().await {
                                            if let Some(real_id) = data["id"].as_str() {
                                                cache.write().iter_mut().find(|s| s.id == temp_id).map(|s| s.id = real_id.to_string());
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
                                on_saved: move |_| editing_id.set(None),
                                on_cancel: move |_| editing_id.set(None),
                            }
                        } else {
                            ItemRow {
                                key: "{s.id}",
                                name: s.name.clone(),
                                sub: format!("{} strains • {} accessories • {}฿", s.strain_ids.len(), s.accessory_ids.len(), s.total_price),
                                is_available: s.is_available,
                                image_url: s.icon.clone().filter(|i| i.starts_with("http")),
                                video_url: s.video_url.clone(),
                                on_edit: { let id = s.id.clone(); move |_| editing_id.set(Some(id.clone())) },
                                on_toggle: {
                                    let id = s.id.clone(); let next = !s.is_available;
                                    move |_| {
                                        let id = id.clone();
                                        cache.write().iter_mut().find(|s| s.id == id).map(|s| s.is_available = next);
                                        spawn(async move {
                                            let url = format!("{}/api/sets/{}/availability", api_base_url(), id);
                                            let res = reqwest::Client::new().put(&url)
                                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                .json(&json!({ "is_available": next })).send().await;
                                            match res {
                                                Ok(r) if r.status().is_success() => { TelegramApp::init().haptic_notification(HapticNotification::Success); }
                                                _ => {
                                                    cache.write().iter_mut().find(|s| s.id == id).map(|s| s.is_available = !next);
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
                        let res = reqwest::Client::new().delete(&url)
                            .header("X-Telegram-Init-Data", init_data.read().clone())
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
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut name = use_signal(|| item.name.clone());
    let mut description = use_signal(|| item.description.clone().unwrap_or_default());
    let mut icon = use_signal(|| item.icon.clone().unwrap_or_default());
    let video_url = use_signal(|| item.video_url.clone().unwrap_or_default());
    let mut strain_ids = use_signal(|| item.strain_ids.join(", "));
    let mut accessory_ids = use_signal(|| item.accessory_ids.join(", "));
    let mut total_price = use_signal(|| item.total_price.to_string());
    let mut discount_percent = use_signal(|| item.discount_percent.to_string());
    let mut is_deal_of_day = use_signal(|| item.is_deal_of_day);
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    rsx! {
        div { "data-editing": "true", style: edit_card_style(),
            div { style: edit_header_style(), "✏️ Редактирование" }
            input { style: input_style(), placeholder: "Название", value: "{name}", oninput: move |e| name.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Описание", value: "{description}", oninput: move |e| description.set(e.value()) }
            input { style: input_style(), placeholder: "Иконка", value: "{icon}", oninput: move |e| icon.set(e.value()) }
            {render_video_upload(video_url)}
            textarea { style: textarea_style(), placeholder: "Strain IDs (через запятую)", value: "{strain_ids}", oninput: move |e| strain_ids.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Accessory IDs (через запятую)", value: "{accessory_ids}", oninput: move |e| accessory_ids.set(e.value()) }
            input { style: input_style(), placeholder: "Цена ฿", value: "{total_price}", r#type: "number", oninput: move |e| total_price.set(e.value()) }
            input { style: input_style(), placeholder: "Скидка %", value: "{discount_percent}", r#type: "number", oninput: move |e| discount_percent.set(e.value()) }
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
                            Ok(v) if v >= 0.0 => v,
                            _ => { status.set("❌ Цена должна быть числом ≥ 0".into()); return; }
                        };
                        let d = discount_percent.read().trim().parse::<f64>().unwrap_or(0.0);
                        if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                        let desc = description();
                        let ic = icon(); let vid = video_url();
                        let s_ids: Vec<String> = strain_ids().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                        let a_ids: Vec<String> = accessory_ids().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                        let deal = is_deal_of_day();
                        let id = item_id.clone();
                        let original = cache.read().iter().find(|s| s.id == id).cloned();
                        cache.write().iter_mut().find(|s| s.id == id).map(|s| {
                            s.name = n.clone();
                            s.description = if desc.is_empty() { None } else { Some(desc.clone()) };
                            s.icon = if ic.is_empty() { None } else { Some(ic.clone()) };
                            s.video_url = if vid.is_empty() { None } else { Some(vid.clone()) };
                            s.strain_ids = s_ids.clone();
                            s.accessory_ids = a_ids.clone();
                            s.total_price = p;
                            s.discount_percent = d;
                            s.is_deal_of_day = deal;
                        });
                        on_saved.call(());
                        spawn(async move {
                            let body = json!({
                                "name": n, "total_price": p, "discount_percent": d,
                                "description": if desc.is_empty() { serde_json::Value::Null } else { desc.into() },
                                "icon": if ic.is_empty() { serde_json::Value::Null } else { ic.into() },
                                "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                "strain_ids": s_ids, "accessory_ids": a_ids,
                                "is_deal_of_day": deal,
                            });
                            let url = format!("{}/api/sets/{}", api_base_url(), id);
                            let res = reqwest::Client::new().put(&url)
                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                .json(&body).send().await;
                            let success = match res { Ok(r) => r.status().is_success(), Err(_) => false };
                            if success {
                                TelegramApp::init().haptic_notification(HapticNotification::Success);
                            } else {
                                TelegramApp::init().haptic_notification(HapticNotification::Error);
                                if let Some(orig) = original { cache.write().iter_mut().find(|s| s.id == id).map(|s| *s = orig); }
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
                        if let Ok(data) = serde_json::from_str::<Vec<AdminAccessorySet>>(&json) {
                            return data;
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
    let mut accessories = use_signal(String::new);
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
                    let _ = storage.set_item("wwb_admin_accessory_sets", &serde_json::to_string(&data).unwrap_or_default());
                }
            }
        }
    });

    let _ = use_resource(move || {
        let _ = reload.read();
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/accessory-sets?include_hidden=1", api_base_url());
            if let Ok(resp) = reqwest::Client::new()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send().await
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
        cache.read().iter().filter(|s| {
            q.is_empty() || s.name.to_lowercase().contains(&q)
        }).cloned().collect()
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
                    {render_image_upload(image_url)}
                    {render_video_upload(video_url)}
                    textarea { style: textarea_style(), placeholder: "Accessory IDs (через запятую)", value: "{accessories}",
                        oninput: move |e| accessories.set(e.value()) }
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
                                Ok(v) if v >= 0.0 => v,
                                _ => { status.set("❌ Цена должна быть числом ≥ 0".into()); return; }
                            };
                            let d = discount_percent.read().trim().parse::<f64>().unwrap_or(0.0);
                            if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                            let accs: Vec<String> = accessories().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
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
                            accessories.set(String::new()); total_price.set(String::new()); discount_percent.set(String::new());
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
                                    "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                    "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                });
                                let url = format!("{}/api/accessory-sets", api_base_url());
                                let res = reqwest::Client::new().post(&url)
                                    .header("X-Telegram-Init-Data", init_data.read().clone())
                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                submitting.set(false);
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        if let Ok(data) = r.json::<serde_json::Value>().await {
                                            if let Some(real_id) = data["id"].as_str() {
                                                cache.write().iter_mut().find(|s| s.id == temp_id).map(|s| s.id = real_id.to_string());
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
                                on_saved: move |_| editing_id.set(None),
                                on_cancel: move |_| editing_id.set(None),
                            }
                        } else {
                            ItemRow {
                                key: "{s.id}",
                                name: s.name.clone(),
                                sub: format!("{} items • {}฿", s.accessories.len(), s.total_price),
                                is_available: s.is_available,
                                image_url: s.image_url.clone().filter(|i| i.starts_with("http")).or_else(|| s.icon.clone().filter(|i| i.starts_with("http"))),
                                video_url: s.video_url.clone(),
                                on_edit: { let id = s.id.clone(); move |_| editing_id.set(Some(id.clone())) },
                                on_toggle: {
                                    let id = s.id.clone(); let next = !s.is_available;
                                    move |_| {
                                        let id = id.clone();
                                        cache.write().iter_mut().find(|s| s.id == id).map(|s| s.is_available = next);
                                        spawn(async move {
                                            let url = format!("{}/api/accessory-sets/{}/availability", api_base_url(), id);
                                            let res = reqwest::Client::new().put(&url)
                                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                .json(&json!({ "is_available": next })).send().await;
                                            match res {
                                                Ok(r) if r.status().is_success() => { TelegramApp::init().haptic_notification(HapticNotification::Success); }
                                                _ => {
                                                    cache.write().iter_mut().find(|s| s.id == id).map(|s| s.is_available = !next);
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
                        let res = reqwest::Client::new().delete(&url)
                            .header("X-Telegram-Init-Data", init_data.read().clone())
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
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut name = use_signal(|| item.name.clone());
    let mut description = use_signal(|| item.description.clone().unwrap_or_default());
    let mut icon = use_signal(|| item.icon.clone().unwrap_or_default());
    let image_url = use_signal(|| item.image_url.clone().unwrap_or_default());
    let video_url = use_signal(|| item.video_url.clone().unwrap_or_default());
    let mut accessories = use_signal(|| item.accessories.join(", "));
    let mut total_price = use_signal(|| item.total_price.to_string());
    let mut discount_percent = use_signal(|| item.discount_percent.to_string());
    let mut is_deal_of_day = use_signal(|| item.is_deal_of_day);
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    rsx! {
        div { "data-editing": "true", style: edit_card_style(),
            div { style: edit_header_style(), "✏️ Редактирование" }
            input { style: input_style(), placeholder: "Название (RU)", value: "{name}", oninput: move |e| name.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description}", oninput: move |e| description.set(e.value()) }
            input { style: input_style(), placeholder: "Иконка", value: "{icon}", oninput: move |e| icon.set(e.value()) }
            {render_image_upload(image_url)}
            {render_video_upload(video_url)}
            textarea { style: textarea_style(), placeholder: "Accessory IDs (через запятую)", value: "{accessories}", oninput: move |e| accessories.set(e.value()) }
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
                            Ok(v) if v >= 0.0 => v,
                            _ => { status.set("❌ Цена должна быть числом ≥ 0".into()); return; }
                        };
                        let d = discount_percent.read().trim().parse::<f64>().unwrap_or(0.0);
                        if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                        let accs: Vec<String> = accessories().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                        let desc = description();
                        let ic = icon(); let img = image_url(); let vid = video_url();
                        let deal = is_deal_of_day();
                        let ne = name_en(); let de = description_en();
                        let id = item_id.clone();
                        let original = cache.read().iter().find(|s| s.id == id).cloned();
                        cache.write().iter_mut().find(|s| s.id == id).map(|s| {
                            s.name = n.clone();
                            s.description = if desc.is_empty() { None } else { Some(desc.clone()) };
                            s.icon = if ic.is_empty() { None } else { Some(ic.clone()) };
                            s.image_url = if img.is_empty() { None } else { Some(img.clone()) };
                            s.video_url = if vid.is_empty() { None } else { Some(vid.clone()) };
                            s.accessories = accs.clone();
                            s.total_price = p;
                            s.discount_percent = d;
                            s.is_deal_of_day = deal;
                            s.name_en = if ne.is_empty() { None } else { Some(ne.clone()) };
                            s.description_en = if de.is_empty() { None } else { Some(de.clone()) };
                        });
                        on_saved.call(());
                        spawn(async move {
                            let body = json!({
                                "name": n, "total_price": p, "discount_percent": d,
                                "description": if desc.is_empty() { serde_json::Value::Null } else { desc.into() },
                                "icon": if ic.is_empty() { serde_json::Value::Null } else { ic.into() },
                                "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                "accessories": accs,
                                "is_deal_of_day": deal,
                                "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                            });
                            let url = format!("{}/api/accessory-sets/{}", api_base_url(), id);
                            let res = reqwest::Client::new().put(&url)
                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                .json(&body).send().await;
                            let success = match res { Ok(r) => r.status().is_success(), Err(_) => false };
                            if success {
                                TelegramApp::init().haptic_notification(HapticNotification::Success);
                            } else {
                                TelegramApp::init().haptic_notification(HapticNotification::Error);
                                if let Some(orig) = original { cache.write().iter_mut().find(|s| s.id == id).map(|s| *s = orig); }
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
                        if let Ok(data) = serde_json::from_str::<Vec<AdminTeaSet>>(&json) {
                            return data;
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
    let mut video_url = use_signal(String::new);
    let mut items = use_signal(String::new);
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
                    let _ = storage.set_item("wwb_admin_tea_sets", &serde_json::to_string(&data).unwrap_or_default());
                }
            }
        }
    });

    let _ = use_resource(move || {
        let _ = reload.read();
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/tea-sets?include_hidden=1", api_base_url());
            if let Ok(resp) = reqwest::Client::new()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data.clone())
                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                .send().await
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
        cache.read().iter().filter(|s| {
            q.is_empty() || s.name.to_lowercase().contains(&q)
        }).cloned().collect()
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
                    {render_video_upload(video_url)}
                    textarea { style: textarea_style(), placeholder: "Tea item IDs (через запятую)", value: "{items}",
                        oninput: move |e| items.set(e.value()) }
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
                                Ok(v) if v >= 0.0 => v,
                                _ => { status.set("❌ Цена должна быть числом ≥ 0".into()); return; }
                            };
                            let d = discount_percent.read().trim().parse::<f64>().unwrap_or(0.0);
                            if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                            let tea_items: Vec<String> = items().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                            let desc = description();
                            let ic = icon(); let vid = video_url();
                            let ne = name_en(); let de = description_en();
                            submitting.set(true);
                            let temp_id = format!("temp-{}", uuid::Uuid::new_v4());
                            cache.write().insert(0, AdminTeaSet {
                                id: temp_id.clone(), name: n.clone(),
                                description: if desc.is_empty() { None } else { Some(desc.clone()) },
                                icon: if ic.is_empty() { None } else { Some(ic.clone()) },
                                video_url: if vid.is_empty() { None } else { Some(vid.clone()) },
                                items: tea_items.clone(),
                                total_price: p, discount_percent: d,
                                is_available: true,
                                name_en: if ne.is_empty() { None } else { Some(ne.clone()) },
                                description_en: if de.is_empty() { None } else { Some(de.clone()) },
                            });
                            status.set("✅ Добавлен!".into());
                            name.set(String::new()); description.set(String::new()); icon.set(String::new()); video_url.set(String::new());
                            items.set(String::new()); total_price.set(String::new()); discount_percent.set(String::new());
                            name_en.set(String::new()); description_en.set(String::new());
                            auto_scroll_to_list();
                            spawn(async move {
                                let body = json!({
                                    "name": n, "total_price": p, "discount_percent": d,
                                    "description": if desc.is_empty() { serde_json::Value::Null } else { desc.into() },
                                    "icon": if ic.is_empty() { serde_json::Value::Null } else { ic.into() },
                                    "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                    "items": tea_items,
                                    "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                    "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                });
                                let url = format!("{}/api/tea-sets", api_base_url());
                                let res = reqwest::Client::new().post(&url)
                                    .header("X-Telegram-Init-Data", init_data.read().clone())
                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                submitting.set(false);
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        if let Ok(data) = r.json::<serde_json::Value>().await {
                                            if let Some(real_id) = data["id"].as_str() {
                                                cache.write().iter_mut().find(|s| s.id == temp_id).map(|s| s.id = real_id.to_string());
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
                                on_saved: move |_| editing_id.set(None),
                                on_cancel: move |_| editing_id.set(None),
                            }
                        } else {
                            ItemRow {
                                key: "{s.id}",
                                name: s.name.clone(),
                                sub: format!("{} items • {}฿", s.items.len(), s.total_price),
                                is_available: s.is_available,
                                image_url: s.icon.clone().filter(|i| i.starts_with("http")),
                                video_url: s.video_url.clone(),
                                on_edit: { let id = s.id.clone(); move |_| editing_id.set(Some(id.clone())) },
                                on_toggle: {
                                    let id = s.id.clone(); let next = !s.is_available;
                                    move |_| {
                                        let id = id.clone();
                                        cache.write().iter_mut().find(|s| s.id == id).map(|s| s.is_available = next);
                                        spawn(async move {
                                            let url = format!("{}/api/tea-sets/{}/availability", api_base_url(), id);
                                            let res = reqwest::Client::new().put(&url)
                                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                .json(&json!({ "is_available": next })).send().await;
                                            match res {
                                                Ok(r) if r.status().is_success() => { TelegramApp::init().haptic_notification(HapticNotification::Success); }
                                                _ => {
                                                    cache.write().iter_mut().find(|s| s.id == id).map(|s| s.is_available = !next);
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
                        let res = reqwest::Client::new().delete(&url)
                            .header("X-Telegram-Init-Data", init_data.read().clone())
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
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut name = use_signal(|| item.name.clone());
    let mut description = use_signal(|| item.description.clone().unwrap_or_default());
    let mut icon = use_signal(|| item.icon.clone().unwrap_or_default());
    let video_url = use_signal(|| item.video_url.clone().unwrap_or_default());
    let mut items = use_signal(|| item.items.join(", "));
    let mut total_price = use_signal(|| item.total_price.to_string());
    let mut discount_percent = use_signal(|| item.discount_percent.to_string());
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    rsx! {
        div { "data-editing": "true", style: edit_card_style(),
            div { style: edit_header_style(), "✏️ Редактирование" }
            input { style: input_style(), placeholder: "Название (RU)", value: "{name}", oninput: move |e| name.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description}", oninput: move |e| description.set(e.value()) }
            input { style: input_style(), placeholder: "Иконка", value: "{icon}", oninput: move |e| icon.set(e.value()) }
            {render_video_upload(video_url)}
            textarea { style: textarea_style(), placeholder: "Tea item IDs (через запятую)", value: "{items}", oninput: move |e| items.set(e.value()) }
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
                            Ok(v) if v >= 0.0 => v,
                            _ => { status.set("❌ Цена должна быть числом ≥ 0".into()); return; }
                        };
                        let d = discount_percent.read().trim().parse::<f64>().unwrap_or(0.0);
                        if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                        let tea_items: Vec<String> = items().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                        let desc = description();
                        let ic = icon(); let vid = video_url();
                        let ne = name_en(); let de = description_en();
                        let id = item_id.clone();
                        let original = cache.read().iter().find(|s| s.id == id).cloned();
                        cache.write().iter_mut().find(|s| s.id == id).map(|s| {
                            s.name = n.clone();
                            s.description = if desc.is_empty() { None } else { Some(desc.clone()) };
                            s.icon = if ic.is_empty() { None } else { Some(ic.clone()) };
                            s.video_url = if vid.is_empty() { None } else { Some(vid.clone()) };
                            s.items = tea_items.clone();
                            s.total_price = p;
                            s.discount_percent = d;
                            s.name_en = if ne.is_empty() { None } else { Some(ne.clone()) };
                            s.description_en = if de.is_empty() { None } else { Some(de.clone()) };
                        });
                        on_saved.call(());
                        spawn(async move {
                            let body = json!({
                                "name": n, "total_price": p, "discount_percent": d,
                                "description": if desc.is_empty() { serde_json::Value::Null } else { desc.into() },
                                "icon": if ic.is_empty() { serde_json::Value::Null } else { ic.into() },
                                "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                "items": tea_items,
                                "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                            });
                            let url = format!("{}/api/tea-sets/{}", api_base_url(), id);
                            let res = reqwest::Client::new().put(&url)
                                .header("X-Telegram-Init-Data", init_data.read().clone())
                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                .json(&body).send().await;
                            let success = match res { Ok(r) => r.status().is_success(), Err(_) => false };
                            if success {
                                TelegramApp::init().haptic_notification(HapticNotification::Success);
                            } else {
                                TelegramApp::init().haptic_notification(HapticNotification::Error);
                                if let Some(orig) = original { cache.write().iter_mut().find(|s| s.id == id).map(|s| *s = orig); }
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

// ── Shared UI helpers ─────────────────────────────────────────
// ── Shared UI helpers ─────────────────────────────────────────

fn auto_scroll_to_list() {
    let _ = js_sys::eval("setTimeout(()=>{var el=document.querySelector('[data-list]');if(el)el.scrollIntoView({behavior:'smooth'});},100);");
}

fn render_image_upload(mut image_url: Signal<String>) -> Element {
    let mut uploading = use_signal(|| false);
    rsx! {
        div { style: "display:flex;gap:6px;align-items:center;",
            input { style: "flex:1;{input_style()}", placeholder: "URL картинки", value: "{image_url}",
                oninput: move |e| image_url.set(e.value()) }
            if *uploading.read() {
                div { style: "padding:10px 12px;background:#1a1a2e;color:#6699ff;border:1px dashed #2a2a4a;border-radius:4px;font-size:13px;white-space:nowrap;", "⏳ Загрузка..." }
            } else {
                button { style: upload_btn_style(),
                    onclick: move |_| {
                        uploading.set(true);
                        spawn(async move {
                            let result = upload_image().await;
                            uploading.set(false);
                            if let Some(url) = result { image_url.set(url); }
                        });
                    },
                    "📷 Upload"
                }
            }
        }
        if !image_url.read().is_empty() {
            div { style: "margin-top:4px;",
                img { src: "{image_url}", style: "width:64px;height:64px;object-fit:cover;border-radius:6px;border:1px solid #2a2a4a;cursor:pointer;", onclick: move |_| {
                    let url = image_url.read().clone();
                    let _ = web_sys::window().and_then(|w| w.open_with_url_and_target(&url, "_blank").ok());
                } }
            }
        }
    }
}

fn render_video_upload(mut video_url: Signal<String>) -> Element {
    let mut uploading = use_signal(|| false);
    rsx! {
        div { style: "display:flex;gap:6px;align-items:center;",
            input { style: "flex:1;{input_style()}", placeholder: "URL видео", value: "{video_url}",
                oninput: move |e| video_url.set(e.value()) }
            if *uploading.read() {
                div { style: "padding:10px 12px;background:#1a1a2e;color:#6699ff;border:1px dashed #2a2a4a;border-radius:4px;font-size:13px;white-space:nowrap;", "⏳ Загрузка..." }
            } else {
                button { style: upload_btn_style(),
                    onclick: move |_| {
                        uploading.set(true);
                        spawn(async move {
                            let result = upload_video().await;
                            uploading.set(false);
                            if let Some(url) = result { video_url.set(url); }
                        });
                    },
                    "🎥 Upload"
                }
            }
        }
        if !video_url.read().is_empty() {
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
#[component]
fn ItemRow(
    name: String,
    sub: String,
    is_available: bool,
    image_url: Option<String>,
    #[props(default)]
    video_url: Option<String>,
    on_edit: EventHandler<()>,
    on_toggle: EventHandler<()>,
    on_delete: EventHandler<()>,
    #[props(default)]
    on_sotd: Option<EventHandler<()>>,
) -> Element {
    let badge = if is_available { ("#39ff14", "ВКЛ") } else { ("#666", "ВЫКЛ") };
    let toggle_label = if is_available { "👁️" } else { "🚫" };
    let mut show_video = use_signal(|| false);
    let thumb = match image_url.as_deref() {
        Some(url) if !url.is_empty() => rsx! { img { src: "{url}", style: "width:36px;height:36px;object-fit:cover;border-radius:6px;border:1px solid #2a2a4a;flex-shrink:0;" } },
        _ => rsx! { div { style: "width:36px;height:36px;background:#2a2a4a;border-radius:6px;display:flex;align-items:center;justify-content:center;flex-shrink:0;font-size:16px;", "📦" } },
    };
    rsx! {
        div { style: "background:#1a1a2e;padding:8px 10px;border-radius:6px;display:flex;align-items:center;gap:6px;flex-wrap:nowrap;overflow:hidden;",
            {thumb}
            div { style: "flex:1;min-width:0;overflow:hidden;cursor:pointer;",
                onclick: move |_| on_edit.call(()),
                div { style: "font-weight:600;font-size:13px;color:#e8e8e8;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;", "{name}" }
                div { style: "font-size:10px;color:#888;margin-top:1px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;", "{sub}" }
            }
            span { style: "font-size:9px;padding:2px 5px;background:{badge.0}20;color:{badge.0};border-radius:8px;font-weight:600;flex-shrink:0;", "{badge.1}" }
            {if let Some(ref url) = video_url {
                let url = url.clone();
                rsx! {
                    button { style: "flex-shrink:0;min-width:44px;min-height:44px;padding:8px 10px;background:#1a2a3a;color:#4fc3f7;border:none;border-radius:4px;font-size:16px;cursor:pointer;line-height:1;display:flex;align-items:center;justify-content:center;",
                        onclick: move |e: Event<MouseData>| { e.stop_propagation(); show_video.set(true); }, "▶️" }
                    {if show_video() {
                        rsx! {
                            div { style: "position:fixed;inset:0;background:rgba(0,0,0,0.8);display:flex;align-items:center;justify-content:center;z-index:1000;padding:16px;",
                                onclick: move |_| show_video.set(false),
                                div { style: "background:#1a1a2e;padding:16px;border-radius:8px;max-width:90vw;max-height:80vh;display:flex;flex-direction:column;align-items:center;gap:8px;",
                                    onclick: move |e: Event<MouseData>| e.stop_propagation(),
                                    video { style: "max-width:100%;max-height:60vh;border-radius:6px;", controls: true,
                                        source { src: "{url}", r#type: "video/mp4" }
                                    }
                                    button { style: "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;cursor:pointer;",
                                        onclick: move |_| show_video.set(false), "Закрыть" }
                                }
                            }
                        }
                    } else {
                        rsx! {}
                    }}
                }
            } else {
                rsx! {}
            }}
            button { style: "flex-shrink:0;min-width:44px;min-height:44px;padding:8px 10px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:16px;cursor:pointer;line-height:1;display:flex;align-items:center;justify-content:center;",
                onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_edit.call(()); }, "✏️" }
            button { style: "flex-shrink:0;min-width:44px;min-height:44px;padding:8px 10px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:16px;cursor:pointer;line-height:1;display:flex;align-items:center;justify-content:center;",
                onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_toggle.call(()); }, "{toggle_label}" }
            {if let Some(handler) = on_sotd {
                let handler = handler.clone();
                rsx! {
                    button { style: "flex-shrink:0;min-width:44px;min-height:44px;padding:8px 10px;background:#2a2a1a;color:#ffe600;border:none;border-radius:4px;font-size:16px;cursor:pointer;line-height:1;display:flex;align-items:center;justify-content:center;",
                        onclick: move |e: Event<MouseData>| { e.stop_propagation(); handler.call(()); }, "🌟" }
                }
            } else {
                rsx! {}
            }}
            button { style: "flex-shrink:0;min-width:44px;min-height:44px;padding:8px 10px;background:#3a1a1a;color:#ff8888;border:none;border-radius:4px;font-size:16px;cursor:pointer;line-height:1;display:flex;align-items:center;justify-content:center;",
                onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_delete.call(()); }, "🗑" }
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
    let mut category = use_signal(|| item.category.clone().unwrap_or_else(|| "hybrid".to_string()));
    let mut price = use_signal(|| item.price_per_gram.to_string());
    let mut thc = use_signal(|| item.thc_percent.map(|v| v.to_string()).unwrap_or_default());
    let mut cbd = use_signal(|| item.cbd_percent.map(|v| v.to_string()).unwrap_or_default());
    let mut grams = use_signal(|| item.available_grams.unwrap_or(0.0).to_string());
    let mut description = use_signal(|| item.description.clone().unwrap_or_default());
    let mut effect = use_signal(|| item.effect.clone().unwrap_or_default());
    let mut flavor_profile = use_signal(|| item.flavor_profile.clone().unwrap_or_default());
    let image_url = use_signal(|| item.image_url.clone().unwrap_or_default());
    let video_url = use_signal(|| item.video_url.clone().unwrap_or_default());
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut effect_en = use_signal(|| item.effect_en.clone().unwrap_or_default());
    let mut flavor_profile_en = use_signal(|| item.flavor_profile_en.clone().unwrap_or_default());
    let mut strain_type_en = use_signal(|| item.strain_type_en.clone().unwrap_or_default());
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
            {render_image_upload(image_url)}
            {render_video_upload(video_url)}
            div { style: en_section_style(), "🇬🇧 English" }
            input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}", oninput: move |e| name_en.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}", oninput: move |e| description_en.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Effect (EN)", value: "{effect_en}", oninput: move |e| effect_en.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Flavor (EN)", value: "{flavor_profile_en}", oninput: move |e| flavor_profile_en.set(e.value()) }
            input { style: input_style(), placeholder: "Type (EN)", value: "{strain_type_en}", oninput: move |e| strain_type_en.set(e.value()) }
            div { style: "display:flex;gap:8px;",
                button { style: submit_btn_style(),
                    onclick: move |_| {
                        let n = name().trim().to_string();
                        let c = category();
                        let p = match price.read().trim().parse::<f64>() {
                            Ok(v) if v > 0.0 => v,
                            _ => { status.set("❌ Цена должна быть числом больше 0".into()); return; }
                        };
                        let t = thc.read().trim().parse::<f64>().ok();
                        let cb = cbd.read().trim().parse::<f64>().ok();
                        let g = match grams.read().trim().parse::<f64>() {
                            Ok(v) if v >= 0.0 => v,
                            _ => { status.set("❌ Граммы должны быть числом ≥ 0".into()); return; }
                        };
                        if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                        let d = description(); let ef = effect(); let fp = flavor_profile(); let img = image_url(); let vid = video_url();
                        let ne = name_en(); let de = description_en(); let ee = effect_en();
                        let fpe = flavor_profile_en(); let ste = strain_type_en();
                        let id = item_id.clone();
                        let original = cache.read().iter().find(|s| s.id == id).cloned();
                        // Optimistic update in cache
                        cache.write().iter_mut().find(|s| s.id == id).map(|s| {
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
                        });
                        on_saved.call(());
                        spawn(async move {
                            let body = json!({
                                "name": n, "category": c, "price_per_gram": p, "available_grams": g,
                                "thc_percent": t, "cbd_percent": cb,
                                "description": if d.is_empty() { serde_json::Value::Null } else { d.into() },
                                "effect": if ef.is_empty() { serde_json::Value::Null } else { ef.into() },
                                "flavor_profile": if fp.is_empty() { serde_json::Value::Null } else { fp.into() },
                                "is_available": item.is_available,
                                "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
                                "video_url": if vid.is_empty() { serde_json::Value::Null } else { vid.into() },
                                "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                "effect_en": if ee.is_empty() { serde_json::Value::Null } else { ee.into() },
                                "flavor_profile_en": if fpe.is_empty() { serde_json::Value::Null } else { fpe.into() },
                                "strain_type_en": if ste.is_empty() { serde_json::Value::Null } else { ste.into() },
                            });
                            let url = format!("{}/api/strains/{}", api_base_url(), id);
                            let res = reqwest::Client::new().put(&url)
                                .header("X-Telegram-Init-Data", init_data.read().clone())
 .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                .json(&body).send().await;
                            let success = match res {
                                Ok(r) => r.status().is_success(),
                                Err(_) => false,
                            };
                            if success {
                                TelegramApp::init().haptic_notification(HapticNotification::Success);
                            } else {
                                TelegramApp::init().haptic_notification(HapticNotification::Error);
                                if let Some(orig) = original {
                                    cache.write().iter_mut().find(|s| s.id == id).map(|s| *s = orig);
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
    let image_url = use_signal(|| item.image_url.clone().unwrap_or_default());
    let video_url = use_signal(|| item.video_url.clone().unwrap_or_default());
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
            {render_image_upload(image_url)}
            {render_video_upload(video_url)}
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
                            Ok(v) if v > 0.0 => v,
                            _ => { status.set("❌ Цена должна быть числом больше 0".into()); return; }
                        };
                        let s_str = stock();
                        let s_val: i32 = match s_str.trim().parse() {
                            Ok(v) if v >= 0 => v,
                            _ => { status.set("❌ Количество должно быть числом ≥ 0".into()); return; }
                        };
                        if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                        let d = description(); let img = image_url(); let vid = video_url();
                        let ne = name_en(); let de = description_en(); let ce = category_en();
                        let id = item_id.clone();
                        let original = cache.read().iter().find(|a| a.id == id).cloned();
                        cache.write().iter_mut().find(|a| a.id == id).map(|a| {
                            a.name = n.clone(); a.category = Some(c.clone()); a.price = p; a.stock = Some(s_val);
                            a.description = if d.is_empty() { None } else { Some(d.clone()) };
                            a.image_url = if img.is_empty() { None } else { Some(img.clone()) };
                            a.video_url = if vid.is_empty() { None } else { Some(vid.clone()) };
                            a.name_en = if ne.is_empty() { None } else { Some(ne.clone()) };
                            a.description_en = if de.is_empty() { None } else { Some(de.clone()) };
                            a.category_en = if ce.is_empty() { None } else { Some(ce.clone()) };
                        });
                        on_saved.call(());
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
                            let res = reqwest::Client::new().put(&url)
                                .header("X-Telegram-Init-Data", init_data.read().clone())
 .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                .json(&body).send().await;
                            let success = match res {
                                Ok(r) => r.status().is_success(),
                                Err(_) => false,
                            };
                            if success {
                                TelegramApp::init().haptic_notification(HapticNotification::Success);
                            } else {
                                TelegramApp::init().haptic_notification(HapticNotification::Error);
                                if let Some(orig) = original {
                                    cache.write().iter_mut().find(|a| a.id == id).map(|a| *a = orig);
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
fn EditTeaCard(
    item: AdminTea,
    cache: Signal<Vec<AdminTea>>,
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut name = use_signal(|| item.name.clone());
    let mut subcategory = use_signal(|| item.subcategory.clone().unwrap_or_else(|| "green".to_string()));
    let mut price = use_signal(|| item.price.to_string());
    let mut stock = use_signal(|| item.stock.map(|s| s.to_string()).unwrap_or_default());
    let mut description = use_signal(|| item.description.clone().unwrap_or_default());
    let image_url = use_signal(|| item.image_url.clone().unwrap_or_default());
    let video_url = use_signal(|| item.video_url.clone().unwrap_or_default());
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut subcategory_en = use_signal(|| item.subcategory_en.clone().unwrap_or_default());
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    rsx! {
        div { "data-editing": "true", style: edit_card_style(),
            div { style: edit_header_style(), "✏️ Редактирование" }
            input { style: input_style(), placeholder: "Название", value: "{name}", oninput: move |e| name.set(e.value()) }
            select { style: input_style(), value: "{subcategory}", oninput: move |e| subcategory.set(e.value()),
                option { value: "green", "🍃 Green" } option { value: "black", "🖤 Black" }
                option { value: "herbal", "🌿 Herbal" } option { value: "oolong", "🍂 Oolong" }
                option { value: "puer", "🟫 Pu-er" } option { value: "other", "🍵 Other" } }
            input { style: input_style(), placeholder: "Цена ฿", value: "{price}", r#type: "number", oninput: move |e| price.set(e.value()) }
            input { style: input_style(), placeholder: "Кол-во", value: "{stock}", r#type: "number", oninput: move |e| stock.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Описание (RU)", value: "{description}", oninput: move |e| description.set(e.value()) }
            {render_image_upload(image_url)}
            {render_video_upload(video_url)}
            div { style: en_section_style(), "🇬🇧 English" }
            input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}", oninput: move |e| name_en.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}", oninput: move |e| description_en.set(e.value()) }
            input { style: input_style(), placeholder: "Subcategory (EN)", value: "{subcategory_en}", oninput: move |e| subcategory_en.set(e.value()) }
            div { style: "display:flex;gap:8px;",
                button { style: submit_btn_style(),
                    onclick: move |_| {
                        let n = name().trim().to_string();
                        let sc = subcategory();
                        let p = match price.read().trim().parse::<f64>() {
                            Ok(v) if v > 0.0 => v,
                            _ => { status.set("❌ Цена должна быть числом больше 0".into()); return; }
                        };
                        let s_str = stock();
                        let s_val: i32 = match s_str.trim().parse() {
                            Ok(v) if v >= 0 => v,
                            _ => { status.set("❌ Количество должно быть числом ≥ 0".into()); return; }
                        };
                        if n.is_empty() { status.set("❌ Название обязательно".into()); return; }
                        let d = description(); let img = image_url(); let vid = video_url();
                        let ne = name_en(); let de = description_en(); let sce = subcategory_en();
                        let id = item_id.clone();
                        let original = cache.read().iter().find(|t| t.id == id).cloned();
                        cache.write().iter_mut().find(|t| t.id == id).map(|t| {
                            t.name = n.clone(); t.subcategory = Some(sc.clone()); t.price = p; t.stock = Some(s_val);
                            t.description = if d.is_empty() { None } else { Some(d.clone()) };
                            t.image_url = if img.is_empty() { None } else { Some(img.clone()) };
                            t.video_url = if vid.is_empty() { None } else { Some(vid.clone()) };
                            t.name_en = if ne.is_empty() { None } else { Some(ne.clone()) };
                            t.description_en = if de.is_empty() { None } else { Some(de.clone()) };
                            t.subcategory_en = if sce.is_empty() { None } else { Some(sce.clone()) };
                        });
                        on_saved.call(());
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
                            let res = reqwest::Client::new().put(&url)
                                .header("X-Telegram-Init-Data", init_data.read().clone())
 .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                .json(&body).send().await;
                            let success = match res {
                                Ok(r) => r.status().is_success(),
                                Err(_) => false,
                            };
                            if success {
                                TelegramApp::init().haptic_notification(HapticNotification::Success);
                            } else {
                                TelegramApp::init().haptic_notification(HapticNotification::Error);
                                if let Some(orig) = original {
                                    cache.write().iter_mut().find(|t| t.id == id).map(|t| *t = orig);
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
    let init_data = use_signal(use_telegram_init_data);
    let mut stats: Signal<Option<AdminStats>> = use_signal(|| None);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(String::new);

    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        async move {
            let url = format!("{}/api/admin/stats", api_base_url());
            match reqwest::Client::new()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data)
                .send().await
            {
                Ok(resp) => {
                    if resp.status().is_success() {
                        match resp.json::<AdminStats>().await {
                            Ok(data) => { stats.set(Some(data)); }
                            Err(e) => { error.set(format!("Ошибка разбора: {e}")); }
                        }
                    } else {
                        error.set(format!("HTTP {}", resp.status().as_u16()));
                    }
                }
                Err(e) => { error.set(format!("Сеть: {e}")); }
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
}

#[derive(Debug, Deserialize)]
struct OrdersResp { orders: Vec<AdminOrder> }

#[component]
fn OrderDetailModal(order: AdminOrder, on_close: EventHandler<()>) -> Element {
    let status_label = match order.status.as_str() {
        "pending" => "⏳ Ожидает",
        "confirmed" => "✓ Подтверждён",
        "completed" => "✅ Выполнен",
        "rejected" => "✖ Отменён",
        _ => &order.status,
    };
    let status_cls = match order.status.as_str() {
        "pending" => "admin-badge warn",
        "confirmed" => "admin-badge info",
        "completed" => "admin-badge success",
        "rejected" => "admin-badge danger",
        _ => "admin-badge muted",
    };
    let order_title = format!("Заказ #{}...{}", &order.id[..4], &order.id[order.id.len().saturating_sub(4)..]);
    rsx! {
        div { style: "position:fixed;inset:0;background:rgba(0,0,0,0.8);display:flex;align-items:center;justify-content:center;z-index:2000;padding:16px;",
            onclick: move |_| on_close.call(()),
            div { style: "background:#1a1a2e;border:2px solid #2a2a4a;border-radius:8px;max-width:480px;width:100%;max-height:90vh;overflow-y:auto;padding:20px;display:flex;flex-direction:column;gap:12px;",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),
                div { style: "display:flex;justify-content:space-between;align-items:center;",
                    h3 { style: "margin:0;color:#39ff14;font-size:17px;", "{order_title}" }
                    button { style: "background:none;border:none;color:#888;font-size:20px;cursor:pointer;", onclick: move |_| on_close.call(()), "✕" }
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
                                    format!("{} × {:.0}", name, item.quantity)
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

    let _ = use_resource(move || {
        let init_data = init_data.read().clone();
        let off = *offset.read();
        let lim = *limit.read();
        let _r = *reload.read();
        async move {
            loading.set(true);
            let url = format!("{}/api/orders?limit={}&offset={}", api_base_url(), lim, off);
            match reqwest::Client::new()
                .get(&url)
                .header("X-Telegram-Init-Data", init_data)
                .send().await
            {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(data) = resp.json::<OrdersResp>().await {
                        orders.set(data.orders);
                    }
                }
                Ok(resp) => { error.set(format!("HTTP {}", resp.status().as_u16())); }
                Err(e) => { error.set(format!("Сеть: {e}")); }
            }
            loading.set(false);
            Some(())
        }
    });

    let filtered: Vec<AdminOrder> = {
        let f = filter.read().clone();
        let q = search.read().to_lowercase();
        orders.read().iter().filter(|o| {
            let status_ok = f == "all" || o.status == f;
            let search_ok = q.is_empty()
                || o.id.to_lowercase().contains(&q)
                || o.customer_name.as_deref().unwrap_or("").to_lowercase().contains(&q)
                || o.customer_telegram.as_deref().unwrap_or("").to_lowercase().contains(&q);
            status_ok && search_ok
        }).cloned().collect()
    };

    let count_by = |s: &str| -> usize {
        let s = s.to_string();
        orders.read().iter().filter(|o| o.status == s).count()
    };

    rsx! {
        div {
            {render_toasts(toasts)}
            h3 { class: "admin-card-title", "📦 Заказы" }
            if !error.read().is_empty() {
                div { class: "admin-badge danger", "{error}" }
            }
            // Filter bar
            div { class: "admin-filter-bar",
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
                    class: if *filter.read() == "completed" { "admin-btn primary admin-btn-sm" } else { "admin-btn secondary admin-btn-sm" },
                    onclick: move |_| filter.set("completed".into()), "✅ {count_by(\"completed\")}" }
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
                            let order_id2 = order.id.clone();
                            let order_id3 = order.id.clone();
                            let status = order.status.clone();
                            let status2 = order.status.clone();
                            let status3 = order.status.clone();
                            let init_data2 = init_data.clone();
                            let init_data3 = init_data.clone();
                            let status_label_str = match order.status.as_str() {
                                "pending" => "⏳ Ожидает".to_string(),
                                "confirmed" => "✓ Подтверждён".to_string(),
                                "completed" => "✅ Выполнен".to_string(),
                                "rejected" => "✖ Отменён".to_string(),
                                _ => order.status.clone(),
                            };
                            let status_badge_cls = match order.status.as_str() {
                                "pending" => "admin-badge warn",
                                "confirmed" => "admin-badge info",
                                "completed" => "admin-badge success",
                                "rejected" => "admin-badge danger",
                                _ => "admin-badge muted",
                            };
                            let short_id = if order.id.len() >= 6 { &order.id[order.id.len()-6..] } else { &order.id };
                            let short_id = short_id.to_string();
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
                                    }
                                    div { class: "admin-row-actions",
                                        if status == "pending" {
                                            button {
                                                class: "admin-btn secondary admin-btn-sm",
                                                disabled: updating_id.read().as_deref() == Some(&order_id),
                                                onclick: move |_| {
                                                    let oid = order_id.clone();
                                                    let id2 = init_data2.read().clone();
                                                    updating_id.set(Some(oid.clone()));
                                                    let mut orders2 = orders.clone();
                                                    let mut updating2 = updating_id.clone();
                                                    let toasts2 = toasts.clone();
                                                    spawn(async move {
                                                        let url = format!("{}/api/orders/{}/status", api_base_url(), oid);
                                                        let res = reqwest::Client::new().put(&url)
                                                            .header("X-Telegram-Init-Data", id2)
                                                            .json(&json!({"status": "confirmed"}))
                                                            .send().await;
                                                        updating2.set(None);
                                                        match res {
                                                            Ok(r) if r.status().is_success() => {
                                                                orders2.write().iter_mut().find(|o| o.id == oid).map(|o| o.status = "confirmed".into());
                                                                push_toast(toasts2, "✓ Заказ подтверждён".into(), ToastKind::Success);
                                                            }
                                                            _ => { push_toast(toasts2, "Ошибка подтверждения".into(), ToastKind::Error); }
                                                        }
                                                    });
                                                },
                                                "✓ Подтвердить"
                                            }
                                        }
                                        if status2 == "confirmed" {
                                            button {
                                                class: "admin-btn primary admin-btn-sm",
                                                disabled: updating_id.read().as_deref() == Some(&order_id2),
                                                onclick: move |_| {
                                                    let oid = order_id2.clone();
                                                    let id3 = init_data3.read().clone();
                                                    updating_id.set(Some(oid.clone()));
                                                    let mut orders3 = orders.clone();
                                                    let mut updating3 = updating_id.clone();
                                                    let toasts3 = toasts.clone();
                                                    spawn(async move {
                                                        let url = format!("{}/api/orders/{}/status", api_base_url(), oid);
                                                        let res = reqwest::Client::new().put(&url)
                                                            .header("X-Telegram-Init-Data", id3)
                                                            .json(&json!({"status": "completed"}))
                                                            .send().await;
                                                        updating3.set(None);
                                                        match res {
                                                            Ok(r) if r.status().is_success() => {
                                                                orders3.write().iter_mut().find(|o| o.id == oid).map(|o| o.status = "completed".into());
                                                                push_toast(toasts3, "✅ Заказ выполнен".into(), ToastKind::Success);
                                                            }
                                                            _ => { push_toast(toasts3, "Ошибка выполнения".into(), ToastKind::Error); }
                                                        }
                                                    });
                                                },
                                                "📦 Выполнить"
                                            }
                                        }
                                        if status3 != "rejected" && status3 != "completed" {
                                            button {
                                                class: "admin-btn danger admin-btn-sm",
                                                onclick: move |_| {
                                                    let oid = order_id3.clone();
                                                    let id_c = init_data.read().clone();
                                                    let mut orders_c = orders.clone();
                                                    let toasts_c = toasts.clone();
                                                    spawn(async move {
                                                        let url = format!("{}/api/orders/{}/status", api_base_url(), oid);
                                                        let res = reqwest::Client::new().put(&url)
                                                            .header("X-Telegram-Init-Data", id_c)
                                                            .json(&json!({"status": "rejected"}))
                                                            .send().await;
                                                        match res {
                                                            Ok(r) if r.status().is_success() => {
                                                                orders_c.write().iter_mut().find(|o| o.id == oid).map(|o| o.status = "rejected".into());
                                                                push_toast(toasts_c, "Заказ отменён".into(), ToastKind::Success);
                                                            }
                                                            _ => { push_toast(toasts_c, "Ошибка отмены".into(), ToastKind::Error); }
                                                        }
                                                    });
                                                },
                                                "✖ Отмена"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                {
                    let page = (*offset.read() / *limit.read()) + 1;
                    let lim = *limit.read();
                    let has_prev = *offset.read() > 0;
                    let has_next = filtered.len() >= lim as usize;
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
struct QuestPlacesResp { quest_places: Vec<AdminQuestPlace> }

#[component]
fn QuestsTab() -> Element {
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
            match reqwest::Client::new().get(&url).header("X-Telegram-Init-Data", init_data).send().await {
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
                                    Ok(v) => v,
                                    Err(_) => { error.set("Неверная широта".into()); return; }
                                };
                                let lon = match edit_lon.read().trim().parse::<f64>() {
                                    Ok(v) => v,
                                    Err(_) => { error.set("Неверная долгота".into()); return; }
                                };
                                let cat = edit_cat.read().clone();
                                let desc = edit_desc.read().trim().to_string();
                                let img = edit_img.read().trim().to_string();
                                let iid = item_id.clone();
                                let is_cr = is_creating;
                                let id_data = init_data.read().clone();
                                saving.set(true);
                                error.set(String::new());
                                let _places2 = places.clone();
                                let mut editing2 = editing.clone();
                                let mut saving2 = saving.clone();
                                let toasts2 = toasts.clone();
                                let mut reload2 = reload.clone();
                                spawn(async move {
                                    let body = json!({
                                        "name": n.clone(), "category": cat,
                                        "lat": lat, "lon": lon,
                                        "description": if desc.is_empty() { serde_json::Value::Null } else { desc.clone().into() },
                                        "image_url": if img.is_empty() { serde_json::Value::Null } else { img.clone().into() },
                                    });
                                    let base = api_base_url();
                                    let res = if is_cr {
                                        reqwest::Client::new().post(&format!("{}/api/quest-places", base))
                                            .header("X-Telegram-Init-Data", id_data)
                                            .json(&body).send().await
                                    } else {
                                        reqwest::Client::new().put(&format!("{}/api/quest-places/{}", base, iid))
                                            .header("X-Telegram-Init-Data", id_data)
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
                            let toasts2 = toasts.clone();
                            let _reload2 = reload.clone();
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
                                            onclick: move |_| { is_new.set(false); editing.set(Some(p2.clone())); },
                                            "✏️"
                                        }
                                        button { class: "admin-btn danger admin-btn-sm",
                                            onclick: move |_| {
                                                let pid = p_id.clone();
                                                let id_d = id_del.clone();
                                                let mut places3 = places.clone();
                                                let toasts3 = toasts2.clone();
                                                spawn(async move {
                                                    let url = format!("{}/api/quest-places/{}", api_base_url(), pid);
                                                    let res = reqwest::Client::new().delete(&url)
                                                        .header("X-Telegram-Init-Data", id_d)
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
struct TreasureHuntsResp { treasure_hunts: Vec<AdminTreasureHunt> }

#[component]
fn TreasuresTab() -> Element {
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
            match reqwest::Client::new().get(&url).header("X-Telegram-Init-Data", init_data).send().await {
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
        let mut edit_bm_desc = use_signal(|| item.black_mark_description.clone().unwrap_or_default());
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
                                let start_lat = edit_start_lat.read().trim().parse::<f64>().unwrap_or(0.0);
                                let start_lon = edit_start_lon.read().trim().parse::<f64>().unwrap_or(0.0);
                                let start_name = edit_start_name.read().trim().to_string();
                                let iid = item_id.clone();
                                let is_cr = is_creating;
                                let id_data = init_data.read().clone();
                                saving.set(true);
                                error.set(String::new());
                                let mut editing2 = editing.clone();
                                let mut saving2 = saving.clone();
                                let toasts2 = toasts.clone();
                                let mut reload2 = reload.clone();
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
                                        reqwest::Client::new().post(&format!("{}/api/treasure-hunts", base))
                                            .header("X-Telegram-Init-Data", id_data)
                                            .json(&body).send().await
                                    } else {
                                        reqwest::Client::new().put(&format!("{}/api/treasure-hunts/{}", base, iid))
                                            .header("X-Telegram-Init-Data", id_data)
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
                            let toasts2 = toasts.clone();
                            let _reload2 = reload.clone();
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
                                            onclick: move |_| { is_new.set(false); editing.set(Some(h2.clone())); },
                                            "✏️"
                                        }
                                        button { class: "admin-btn danger admin-btn-sm",
                                        onclick: move |_| {
                                            let hid = h_id.clone();
                                            let id_d = id_del.clone();
                                            let mut hunts2 = hunts.clone();
                                            let toasts3 = toasts2.clone();
                                            spawn(async move {
                                                let url = format!("{}/api/treasure-hunts/{}", api_base_url(), hid);
                                                let res = reqwest::Client::new().delete(&url)
                                                    .header("X-Telegram-Init-Data", id_d)
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

fn default_true() -> bool { true }

#[component]
fn GardenTab() -> Element {
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
            match reqwest::Client::new().get(&url).header("X-Telegram-Init-Data", init_data).send().await {
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
                            let disc = discount.read().trim().parse::<f64>().unwrap_or(10.0);
                            let bp = bonus_points.read().trim().parse::<f64>().unwrap_or(100.0);
                            let ed = expire_days.read().trim().parse::<f64>().unwrap_or(7.0);
                            let id_data = init_data.read().clone();
                            saving.set(true);
                            error.set(String::new());
                            let mut saving2 = saving.clone();
                            let toasts2 = toasts.clone();
                            let mut error2 = error.clone();
                            spawn(async move {
                                let body = json!({
                                    "is_enabled": enabled,
                                    "reward_discount_percent": disc,
                                    "reward_bonus_points": bp,
                                    "reward_expiration_days": ed,
                                });
                                let url = format!("{}/api/garden/config", api_base_url());
                                let res = reqwest::Client::new().put(&url)
                                    .header("X-Telegram-Init-Data", id_data)
                                    .json(&body).send().await;
                                saving2.set(false);
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        push_toast(toasts2, "✓ Настройки сохранены!".into(), ToastKind::Success);
                                    }
                                    _ => {
                                        error2.set("Ошибка сохранения".into());
                                        push_toast(toasts2, "Ошибка сохранения".into(), ToastKind::Error);
                                    }
                                }
                            });
                        },
                        if *saving.read() { "⏳ Сохранение..." } else { "💾 Сохранить настройки" }
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
            if let Ok(resp) = reqwest::Client::new()
                .get(&format!("{}/api/loyalty/tiers", base))
                .header("X-Telegram-Init-Data", init_data.clone())
                .send().await
            {
                if resp.status().is_success() {
                    if let Ok(data) = resp.json::<LoyaltyConfigResp>().await {
                        tiers.set(data.tiers);
                    }
                }
            }
            // Fetch leaderboard
            if let Ok(resp) = reqwest::Client::new()
                .get(&format!("{}/api/loyalty/leaderboard", base))
                .header("X-Telegram-Init-Data", init_data.clone())
                .send().await
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
struct ManagersResp { managers: Vec<AdminManager> }

#[component]
fn ManagersTab() -> Element {
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
            match reqwest::Client::new().get(&url).header("X-Telegram-Init-Data", init_data).send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<ManagersResp>().await {
                        Ok(data) => { managers.set(data.managers); }
                        Err(e) => { error.set(format!("Ошибка разбора: {e}")); }
                    }
                }
                Ok(resp) => { error.set(format!("HTTP {}", resp.status().as_u16())); }
                Err(e) => { error.set(format!("Сеть: {e}")); }
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
                            let id_data = init_data.read().clone();
                            submitting.set(true);
                            let mut submitting2 = submitting.clone();
                            let mut show_form2 = show_form.clone();
                            let toasts2 = toasts.clone();
                            let mut reload2 = reload.clone();
                            let mut form_tg_id2 = form_tg_id.clone();
                            let mut form_name2 = form_name.clone();
                            spawn(async move {
                                let body = json!({
                                    "telegram_id": tg_id,
                                    "name": if name.is_empty() { serde_json::Value::Null } else { name.into() },
                                    "commission_rate": commission,
                                });
                                let url = format!("{}/api/admin/managers", api_base_url());
                                let res = reqwest::Client::new().post(&url)
                                    .header("X-Telegram-Init-Data", id_data)
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
    let mgr = props.manager.clone();
    let mut editing = use_signal(|| false);
    let mut edit_name = use_signal(|| mgr.name.clone().unwrap_or_default());
    let mut edit_ref_code = use_signal(|| mgr.ref_code.clone().unwrap_or_default());
    let mut saving = use_signal(|| false);
    let stats_text = use_signal(|| "Загрузка...".to_string());
    let manager_id = mgr.telegram_id;
    let init_data = props.init_data.clone();

    // Fetch stats (endpoint may not exist yet — OK)
    let stats_signal = stats_text.clone();
    use_effect(move || {
        let url = format!("{}/api/admin/managers/{}/stats", api_base_url(), manager_id);
        spawn(async move {
            match reqwest::Client::new().get(&url).send().await {
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
                                    let mut saving2 = saving.clone();
                                    let mut editing2 = editing.clone();
                                    spawn(async move {
                                        let body = json!({
                                            "name": if name.is_empty() { serde_json::Value::Null } else { name.into() },
                                            "ref_code": if ref_code.is_empty() { serde_json::Value::Null } else { ref_code.into() },
                                        });
                                        let url = format!("{}/api/admin/managers/{}", api_base_url(), tg_id);
                                        let _ = reqwest::Client::new().put(&url)
                                            .header("X-Telegram-Init-Data", id_data)
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

fn input_style() -> &'static str { "padding:10px 12px;background:#0f0f1a;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:14px;" }
fn submit_btn_style() -> &'static str { "padding:12px;background:#39ff14;color:#000;border:none;border-radius:4px;font-weight:700;font-size:14px;cursor:pointer;margin-top:4px;" }
fn submit_btn_disabled_style() -> &'static str { "padding:12px;background:#666;color:#999;border:none;border-radius:4px;font-weight:700;font-size:14px;cursor:not-allowed;margin-top:4px;" }
fn upload_btn_style() -> &'static str { "padding:10px 12px;background:#2a2a4a;color:#6699ff;border:1px dashed #6699ff;border-radius:4px;font-size:13px;cursor:pointer;white-space:nowrap;" }
fn list_title_style() -> &'static str { "color:#888;font-size:13px;margin:16px 0 8px;text-transform:uppercase;letter-spacing:1px;" }
fn en_section_style() -> &'static str { "padding:6px 0 2px;color:#6699ff;font-size:12px;font-weight:600;border-top:1px solid #2a2a4a;margin-top:4px;" }
fn cancel_btn_style() -> &'static str { "padding:10px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-weight:600;font-size:13px;cursor:pointer;margin-top:4px;" }
fn edit_card_style() -> &'static str { "background:#1a1a2e;border:1px solid #6699ff;border-radius:8px;padding:12px;display:flex;flex-direction:column;gap:8px;" }
fn edit_header_style() -> &'static str { "color:#6699ff;font-size:13px;font-weight:700;text-transform:uppercase;letter-spacing:1px;" }
fn textarea_style() -> &'static str { "padding:10px 12px;background:#0f0f1a;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:14px;min-height:80px;resize:vertical;font-family:inherit;" }

