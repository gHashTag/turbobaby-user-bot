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
use serde::Deserialize;
use serde_json::json;
use wasm_bindgen::JsCast;
use crate::ui::api::context::api_base_url;
use crate::ui::components::{EmptyState, Modal};
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data, TelegramApp};

// ── Data models ───────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, PartialEq)]
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
    name_en: Option<String>,
    description_en: Option<String>,
    effect_en: Option<String>,
    flavor_profile_en: Option<String>,
    strain_type_en: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Deserialize, PartialEq)]
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

#[derive(Debug, Deserialize)]
struct StrainsResp { strains: Vec<AdminStrain> }
#[derive(Debug, Deserialize)]
struct AccessoriesResp { accessories: Vec<AdminAccessory> }
#[derive(Debug, Deserialize)]
struct TeaResp { tea_products: Vec<AdminTea> }
#[derive(Debug, Deserialize)]
struct AdminCheck { is_admin: bool }

#[derive(Clone, Copy, PartialEq)]
enum Tab { Strains, Accessories, Tea }

// ── File upload helper ─────────────────────────────────────────

async fn upload_image() -> Option<String> {
    let js = r#"
new Promise((resolve) => {
    var input = document.createElement('input');
    input.type = 'file';
    input.accept = 'image/*';
    var resolved = false;
    input.onchange = async (e) => {
        if (resolved) return;
        resolved = true;
        var file = e.target.files[0];
        if (!file) { resolve(''); return; }
        var formData = new FormData();
        formData.append('file', file);
        try {
            var baseUrl = window.location.origin;
            var resp = await fetch(baseUrl + '/api/upload', { method: 'POST', body: formData });
            var data = await resp.json();
            resolve(data.url || '');
        } catch(err) { console.error('Upload error:', err); resolve(''); }
    };
    setTimeout(() => { if (!resolved) { resolved = true; resolve(''); } }, 120000);
    input.click();
})
"#;
    let promise_val = js_sys::eval(js).ok()?;
    let promise = promise_val.dyn_into::<js_sys::Promise>().ok()?;
    let result = wasm_bindgen_futures::JsFuture::from(promise).await.ok()?;
    let url = result.as_string()?;
    if url.is_empty() { return None; }
    Some(url)
}

// ── Main component ────────────────────────────────────────────

#[component]
pub fn AdminScreen() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let active_tab = use_signal(|| Tab::Strains);
    let build_version: &'static str = env!("BUILD_VERSION");
    let init_data = use_telegram_init_data();

    let access = use_resource(move || {
        let init_data = init_data.clone();
        async move {
            let base = api_base_url();
            let url = format!("{}/api/admin/check?telegram_id={}", base, telegram_id);
            let resp = reqwest::Client::new().get(&url)
                .header("X-Telegram-Init-Data", init_data.clone())
                .send().await
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
            match &*access.read_unchecked() {
                None => rsx!(div { style: "color:#888;padding:20px 0;", "Проверка доступа..." }),
                Some(Err(e)) => rsx!(div { style: "color:#ff4757;padding:20px 0;", "Ошибка: {e}" }),
                Some(Ok(false)) => rsx!(AccessDeniedScreen { telegram_id }),
                Some(Ok(true)) => rsx!(AdminPanel { active_tab }),
            }
        }
    }
}

#[component]
fn AccessDeniedScreen(telegram_id: i64) -> Element {
    let debug = TelegramApp::init().debug_dump();
    rsx! {
        div { style: "padding:30px 16px;text-align:center;",
            div { style: "font-size:48px;margin-bottom:12px;", "🔒" }
            h2 { style: "color:#ff4757;font-size:18px;margin-bottom:8px;", "Доступ закрыт" }
            p { style: "color:#888;font-size:13px;line-height:1.5;max-width:300px;margin:0 auto;",
                "Попросите владельца добавить ваш Telegram ID в админы." }
            div { style: "margin-top:20px;padding:12px;background:#1a1a2e;border-radius:8px;font-family:monospace;font-size:13px;color:#39ff14;display:inline-block;",
                "ID: {telegram_id}" }
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
fn AdminPanel(active_tab: Signal<Tab>) -> Element {
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
            div { style: "display:flex;gap:2px;margin-bottom:16px;border-radius:4px;overflow:hidden;",
                {tab_btn(Tab::Strains, "🌿 Strains")}
                {tab_btn(Tab::Accessories, "⚙️ Gear")}
                {tab_btn(Tab::Tea, "🍵 Tea")}
            }
            match *active_tab.read() {
                Tab::Strains => rsx!(StrainsTab {}),
                Tab::Accessories => rsx!(AccessoriesTab {}),
                Tab::Tea => rsx!(TeaTab {}),
            }
        }
    }
}

// ── Strains tab (Optimistic UI) ───────────────────────────────

#[component]
fn StrainsTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut cache: Signal<Vec<AdminStrain>> = use_signal(Vec::new);
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

    // Fetch data into cache (runs on mount + when reload changes)
    let _ = use_resource(move || async move {
        let _ = reload.read();
        let url = format!("{}/api/strains?include_hidden=1", api_base_url());
        if let Ok(resp) = reqwest::Client::new().get(&url).send().await {
            if let Ok(data) = resp.json::<StrainsResp>().await {
                cache.set(data.strains);
            }
        }
        loading.set(false);
        Some(())
    });

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
                            let img = image_url();
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
                            image_url.set(String::new());
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
                                    }
                                    _ => {
                                        // Rollback on error
                                        cache.write().retain(|s| s.id != temp_id);
                                        status.set("❌ Ошибка добавления".into());
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
                div { style: "color:#888;", "⏳ Загрузка..." }
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
                                on_edit: {
                                    let id = s.id.clone();
                                    move |_| editing_id.set(Some(id.clone()))
                                },
                                on_toggle: {
                                    let id = s.id.clone();
                                    let next_avail = !s.is_available;
                                    move |_| {
                                        let id = id.clone();
                                        // Optimistic toggle
                                        cache.write().iter_mut().find(|s| s.id == id).map(|s| s.is_available = next_avail);
                                        spawn(async move {
                                            let url = format!("{}/api/strains/{}/availability", api_base_url(), id);
                                            let _ = reqwest::Client::new().put(&url)
                                                .header("X-Telegram-Init-Data", init_data.read().clone())
 .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                .json(&json!({ "is_available": next_avail }))
                                                .send().await;
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
                    cache.write().retain(|s| s.id != id);
                    spawn(async move {
                        let url = format!("{}/api/strains/{}", api_base_url(), id);
                        let _ = reqwest::Client::new().delete(&url)
                            .header("X-Telegram-Init-Data", init_data.read().clone())
                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                            .send().await;
                    });
                }
            }
        }
    }
}

// ── Accessories tab (Optimistic UI) ───────────────────────────

#[component]
fn AccessoriesTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut cache: Signal<Vec<AdminAccessory>> = use_signal(Vec::new);
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

    let _ = use_resource(move || async move {
        let _ = reload.read();
        let url = format!("{}/api/accessories?include_hidden=1", api_base_url());
        if let Ok(resp) = reqwest::Client::new().get(&url).send().await {
            if let Ok(data) = resp.json::<AccessoriesResp>().await {
                cache.set(data.accessories);
            }
        }
        loading.set(false);
        Some(())
    });

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
                    input { style: input_style(), placeholder: "Видео URL", value: "{video_url}",
                        oninput: move |e| video_url.set(e.value()) }
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
                                    }
                                    _ => { cache.write().retain(|a| a.id != temp_id); status.set("❌ Error".into()); }
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
                div { style: "color:#888;", "⏳ Загрузка..." }
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
                                on_edit: { let id = a.id.clone(); move |_| editing_id.set(Some(id.clone())) },
                                on_toggle: {
                                    let id = a.id.clone(); let next = !a.is_available;
                                    move |_| {
                                        let id = id.clone();
                                        cache.write().iter_mut().find(|a| a.id == id).map(|a| a.is_available = next);
                                        spawn(async move {
                                            let url = format!("{}/api/accessories/{}/availability", api_base_url(), id);
                                            let _ = reqwest::Client::new().put(&url)
                                                .header("X-Telegram-Init-Data", init_data.read().clone())
 .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                .json(&json!({ "is_available": next })).send().await;
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
                    cache.write().retain(|a| a.id != id);
                    spawn(async move {
                        let url = format!("{}/api/accessories/{}", api_base_url(), id);
                        let _ = reqwest::Client::new().delete(&url)
                            .header("X-Telegram-Init-Data", init_data.read().clone())
                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                            .send().await;
                    });
                }
            }
        }
    }
}

// ── Tea tab (Optimistic UI) ───────────────────────────────────

#[component]
fn TeaTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_signal(use_telegram_init_data);
    let mut cache: Signal<Vec<AdminTea>> = use_signal(Vec::new);
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

    let _ = use_resource(move || async move {
        let _ = reload.read();
        let url = format!("{}/api/tea-products?include_hidden=1", api_base_url());
        if let Ok(resp) = reqwest::Client::new().get(&url).send().await {
            if let Ok(data) = resp.json::<TeaResp>().await {
                cache.set(data.tea_products);
            }
        }
        loading.set(false);
        Some(())
    });

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
                    input { style: input_style(), placeholder: "Видео URL", value: "{video_url}",
                        oninput: move |e| video_url.set(e.value()) }
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
                                    }
                                    _ => { cache.write().retain(|t| t.id != temp_id); status.set("❌ Error".into()); }
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
                div { style: "color:#888;", "⏳ Загрузка..." }
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
                                on_edit: { let id = t.id.clone(); move |_| editing_id.set(Some(id.clone())) },
                                on_toggle: {
                                    let id = t.id.clone(); let next = !t.is_available;
                                    move |_| {
                                        let id = id.clone();
                                        cache.write().iter_mut().find(|t| t.id == id).map(|t| t.is_available = next);
                                        spawn(async move {
                                            let url = format!("{}/api/tea-products/{}/availability", api_base_url(), id);
                                            let _ = reqwest::Client::new().put(&url)
                                                .header("X-Telegram-Init-Data", init_data.read().clone())
 .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                .json(&json!({ "is_available": next })).send().await;
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
                    cache.write().retain(|t| t.id != id);
                    spawn(async move {
                        let url = format!("{}/api/tea-products/{}", api_base_url(), id);
                        let _ = reqwest::Client::new().delete(&url)
                            .header("X-Telegram-Init-Data", init_data.read().clone())
                            .header("X-Admin-Telegram-Id", telegram_id.to_string())
                            .send().await;
                    });
                }
            }
        }
    }
}

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
                    let _ = js_sys::eval("window.open('"); // placeholder for expand
                } }
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
fn ItemRow(
    name: String, sub: String, is_available: bool, image_url: Option<String>,
    on_edit: EventHandler<()>, on_toggle: EventHandler<()>, on_delete: EventHandler<()>,
) -> Element {
    let badge = if is_available { ("#39ff14", "ВКЛ") } else { ("#666", "ВЫКЛ") };
    let toggle_label = if is_available { "⬇️" } else { "⬆️" };
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
            button { style: "flex-shrink:0;padding:5px 6px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:14px;cursor:pointer;line-height:1;",
                onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_edit.call(()); }, "✏️" }
            button { style: "flex-shrink:0;padding:5px 6px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:14px;cursor:pointer;line-height:1;",
                onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_toggle.call(()); }, "{toggle_label}" }
            button { style: "flex-shrink:0;padding:5px 6px;background:#3a1a1a;color:#ff8888;border:none;border-radius:4px;font-size:14px;cursor:pointer;line-height:1;",
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
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut effect_en = use_signal(|| item.effect_en.clone().unwrap_or_default());
    let mut flavor_profile_en = use_signal(|| item.flavor_profile_en.clone().unwrap_or_default());
    let mut strain_type_en = use_signal(|| item.strain_type_en.clone().unwrap_or_default());
    let status = use_signal(String::new);
    let item_id = item.id.clone();
    rsx! {
        div { style: edit_card_style(),
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
            div { style: en_section_style(), "🇬🇧 English" }
            input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}", oninput: move |e| name_en.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}", oninput: move |e| description_en.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Effect (EN)", value: "{effect_en}", oninput: move |e| effect_en.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Flavor (EN)", value: "{flavor_profile_en}", oninput: move |e| flavor_profile_en.set(e.value()) }
            input { style: input_style(), placeholder: "Type (EN)", value: "{strain_type_en}", oninput: move |e| strain_type_en.set(e.value()) }
            div { style: "display:flex;gap:8px;",
                button { style: submit_btn_style(),
                    onclick: move |_| {
                        let n = name(); let c = category(); let p = price.read().parse::<f64>().unwrap_or(0.0);
                        let t = thc.read().parse::<f64>().ok(); let cb = cbd.read().parse::<f64>().ok();
                        let g = grams.read().parse::<f64>().unwrap_or(0.0);
                        let d = description(); let ef = effect(); let fp = flavor_profile(); let img = image_url();
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
                                "is_available": true,
                                "image_url": if img.is_empty() { serde_json::Value::Null } else { img.into() },
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
                            if !success {
                                if let Some(orig) = original {
                                    cache.write().iter_mut().find(|s| s.id == id).map(|s| *s = orig);
                                }
                            }
                        });
                    },
                    "💾 Сохранить"
                }
                button { style: cancel_btn_style(), onclick: move |_| on_cancel.call(()), "Отмена" }
            }
            if !status.read().is_empty() { div { style: "padding:8px;color:#ff4757;font-size:13px;", "{status}" } }
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
    let mut video_url = use_signal(|| item.video_url.clone().unwrap_or_default());
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut category_en = use_signal(|| item.category_en.clone().unwrap_or_default());
    let status = use_signal(String::new);
    let item_id = item.id.clone();
    rsx! {
        div { style: edit_card_style(),
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
            input { style: input_style(), placeholder: "Видео URL", value: "{video_url}", oninput: move |e| video_url.set(e.value()) }
            div { style: en_section_style(), "🇬🇧 English" }
            input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}", oninput: move |e| name_en.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}", oninput: move |e| description_en.set(e.value()) }
            input { style: input_style(), placeholder: "Category (EN)", value: "{category_en}", oninput: move |e| category_en.set(e.value()) }
            div { style: "display:flex;gap:8px;",
                button { style: submit_btn_style(),
                    onclick: move |_| {
                        let n = name(); let c = category(); let p = price.read().parse::<f64>().unwrap_or(0.0);
                        let s_str = stock(); let s_val: i32 = s_str.parse().unwrap_or(0);
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
                                "name": n, "category": c, "price": p, "stock": s_val, "is_available": true,
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
                            if !success {
                                if let Some(orig) = original {
                                    cache.write().iter_mut().find(|a| a.id == id).map(|a| *a = orig);
                                }
                            }
                        });
                    },
                    "💾 Сохранить"
                }
                button { style: cancel_btn_style(), onclick: move |_| on_cancel.call(()), "Отмена" }
            }
            if !status.read().is_empty() { div { style: "padding:8px;color:#ff4757;font-size:13px;", "{status}" } }
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
    let mut video_url = use_signal(|| item.video_url.clone().unwrap_or_default());
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut subcategory_en = use_signal(|| item.subcategory_en.clone().unwrap_or_default());
    let status = use_signal(String::new);
    let item_id = item.id.clone();
    rsx! {
        div { style: edit_card_style(),
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
            input { style: input_style(), placeholder: "Видео URL", value: "{video_url}", oninput: move |e| video_url.set(e.value()) }
            div { style: en_section_style(), "🇬🇧 English" }
            input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}", oninput: move |e| name_en.set(e.value()) }
            textarea { style: textarea_style(), placeholder: "Description (EN)", value: "{description_en}", oninput: move |e| description_en.set(e.value()) }
            input { style: input_style(), placeholder: "Subcategory (EN)", value: "{subcategory_en}", oninput: move |e| subcategory_en.set(e.value()) }
            div { style: "display:flex;gap:8px;",
                button { style: submit_btn_style(),
                    onclick: move |_| {
                        let n = name(); let sc = subcategory(); let p = price.read().parse::<f64>().unwrap_or(0.0);
                        let s_str = stock(); let s_val: i32 = s_str.parse().unwrap_or(0);
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
                                "name": n, "subcategory": sc, "price": p, "stock": s_val, "is_available": true,
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
                            if !success {
                                if let Some(orig) = original {
                                    cache.write().iter_mut().find(|t| t.id == id).map(|t| *t = orig);
                                }
                            }
                        });
                    },
                    "💾 Сохранить"
                }
                button { style: cancel_btn_style(), onclick: move |_| on_cancel.call(()), "Отмена" }
            }
            if !status.read().is_empty() { div { style: "padding:8px;color:#ff4757;font-size:13px;", "{status}" } }
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

