// Admin Mini-App — full CRUD for all product categories.
//
// Tabs: 🌿 Strains · ⚙️ Accessories · 🍵 Tea
// Each tab: list + add-form + per-item toggle/delete.
// Auth: reads telegram_id from Telegram WebApp SDK, sends X-Admin-Telegram-Id
// header on every write. Non-admins see a friendly "access denied" screen.

use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::json;
use crate::ui::api::context::api_base_url;
use crate::ui::telegram::{use_telegram_id, TelegramApp};

// ── Data models for list rendering ────────────────────────────

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct AdminStrain {
    id: String,
    name: String,
    category: Option<String>,
    price_per_gram: f64,
    available_grams: Option<f64>,
    is_available: bool,
    // EN fields (migration 016)
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
    price: f64,
    stock: Option<i32>,
    is_available: bool,
    // EN fields (migration 016)
    name_en: Option<String>,
    description_en: Option<String>,
    category_en: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct AdminTea {
    id: String,
    name: String,
    subcategory: Option<String>,
    price: f64,
    stock: Option<i32>,
    is_available: bool,
    // EN fields (migration 016)
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

// ── Main component ────────────────────────────────────────────

#[component]
pub fn AdminScreen() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let active_tab = use_signal(|| Tab::Strains);
    let build_version: &'static str = env!("BUILD_VERSION");

    // Access check. reqwest 0.11 WASM ignores .timeout(), so the browser's
    // own fetch timeout (~30s) applies. We surface detailed error stages
    // to make hangs/failures diagnosable from the UI itself.
    let access = use_resource(move || async move {
        let base = api_base_url();
        let url = format!("{}/api/admin/check?telegram_id={}", base, telegram_id);
        let resp = reqwest::Client::new().get(&url).send().await
            .map_err(|e| format!("send to {url}: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("http {} from {url}", status.as_u16()));
        }
        resp.json::<AdminCheck>().await
            .map(|c| c.is_admin)
            .map_err(|e| format!("json: {e}"))
    });

    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding:16px;padding-bottom:80px;",
            h1 { style: "font-size:22px;color:#ff4757;margin-bottom:4px;", "🔧 Admin Mini-App" }
            div { style: "font-size:11px;color:#666;margin-bottom:4px;",
                "telegram_id: {telegram_id}"
            }
            div { style: "font-size:10px;color:#444;margin-bottom:16px;font-family:monospace;",
                "build: {build_version}"
            }

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
            h2 { style: "color:#ff4757;font-size:18px;margin-bottom:8px;",
                "Доступ закрыт"
            }
            p { style: "color:#888;font-size:13px;line-height:1.5;max-width:300px;margin:0 auto;",
                "У вас нет прав администратора. Чтобы получить доступ — попросите владельца магазина добавить ваш Telegram ID в админы."
            }
            div { style: "margin-top:20px;padding:12px;background:#1a1a2e;border-radius:8px;font-family:monospace;font-size:13px;color:#39ff14;display:inline-block;",
                "Ваш ID: {telegram_id}"
            }
            details { style: "margin-top:16px;text-align:left;max-width:340px;margin-left:auto;margin-right:auto;",
                summary { style: "color:#666;font-size:11px;cursor:pointer;", "debug" }
                pre { style: "font-size:10px;color:#888;background:#1a1a2e;padding:8px;border-radius:6px;white-space:pre-wrap;word-break:break-all;",
                    "{debug}"
                }
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
        rsx! {
            button {
                style: "{style}",
                onclick: move |_| { let mut a = active_tab; a.set(t); },
                "{label}"
            }
        }
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

// ── Strains tab ───────────────────────────────────────────────

#[component]
fn StrainsTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let mut name = use_signal(String::new);
    let mut category = use_signal(|| "hybrid".to_string());
    let mut price = use_signal(String::new);
    let mut thc = use_signal(String::new);
    let mut grams = use_signal(String::new);
    // EN fields
    let mut name_en = use_signal(String::new);
    let mut description_en = use_signal(String::new);
    let mut effect_en = use_signal(String::new);
    let mut flavor_profile_en = use_signal(String::new);
    let mut strain_type_en = use_signal(String::new);
    let mut status = use_signal(String::new);
    let mut reload = use_signal(|| 0u32);
    let mut editing_id: Signal<Option<String>> = use_signal(|| None);

    let items = use_resource(move || async move {
        let _ = reload.read();
        let url = format!("{}/api/strains", api_base_url());
        reqwest::Client::new().get(&url).send().await
            .map_err(|e| e.to_string())?
            .json::<StrainsResp>().await
            .map(|r| r.strains)
            .map_err(|e| e.to_string())
    });

    rsx! {
        div {
            // Add form
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
                    input { style: input_style(), placeholder: "Граммы в наличии", value: "{grams}", r#type: "number",
                        oninput: move |e| grams.set(e.value()) }

                    // ── EN section ──
                    div { style: en_section_style(), "🇬🇧 English (optional)" }
                    input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}",
                        oninput: move |e| name_en.set(e.value()) }
                    input { style: input_style(), placeholder: "Description (EN)", value: "{description_en}",
                        oninput: move |e| description_en.set(e.value()) }
                    input { style: input_style(), placeholder: "Effect (EN)", value: "{effect_en}",
                        oninput: move |e| effect_en.set(e.value()) }
                    input { style: input_style(), placeholder: "Flavor profile (EN)", value: "{flavor_profile_en}",
                        oninput: move |e| flavor_profile_en.set(e.value()) }
                    input { style: input_style(), placeholder: "Strain type (EN, e.g. Hybrid)", value: "{strain_type_en}",
                        oninput: move |e| strain_type_en.set(e.value()) }

                    button { style: submit_btn_style(),
                        onclick: move |_| {
                            let n = name(); let c = category(); let p = price.read().parse::<f64>().unwrap_or(0.0);
                            let t = thc.read().parse::<f64>().ok(); let g = grams.read().parse::<f64>().unwrap_or(0.0);
                            let ne = name_en(); let de = description_en(); let ee = effect_en();
                            let fpe = flavor_profile_en(); let ste = strain_type_en();
                            if n.trim().is_empty() || p <= 0.0 { status.set("❌ Заполните название и цену".into()); return; }
                            spawn(async move {
                                let body = json!({
                                    "name": n, "category": c, "price_per_gram": p,
                                    "thc_percent": t, "available_grams": g, "is_available": true,
                                    "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                    "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                    "effect_en": if ee.is_empty() { serde_json::Value::Null } else { ee.into() },
                                    "flavor_profile_en": if fpe.is_empty() { serde_json::Value::Null } else { fpe.into() },
                                    "strain_type_en": if ste.is_empty() { serde_json::Value::Null } else { ste.into() },
                                });
                                let url = format!("{}/api/strains", api_base_url());
                                let res = reqwest::Client::new().post(&url)
                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        status.set("✓ Страйн добавлен".into());
                                        name.set(String::new()); price.set(String::new());
                                        thc.set(String::new()); grams.set(String::new());
                                        name_en.set(String::new()); description_en.set(String::new());
                                        effect_en.set(String::new()); flavor_profile_en.set(String::new());
                                        strain_type_en.set(String::new());
                                        let next = reload.read().wrapping_add(1); reload.set(next);
                                    }
                                    Ok(r) => status.set(format!("❌ HTTP {}", r.status().as_u16())),
                                    Err(e) => status.set(format!("❌ {}", e)),
                                }
                            });
                        },
                        "Добавить"
                    }
                    if !status.read().is_empty() { div { style: "padding:8px;color:#39ff14;font-size:13px;", "{status}" } }
                }
            }

            // List
            h3 { style: list_title_style(), "Все страйны" }
            match &*items.read_unchecked() {
                None => rsx!(div { style: "color:#888;", "Загрузка..." }),
                Some(Err(e)) => rsx!(div { style: "color:#ff4757;", "Ошибка: {e}" }),
                Some(Ok(list)) => rsx!{
                    div { style: "display:flex;flex-direction:column;gap:8px;",
                        for s in list.clone() {
                            if editing_id.read().as_deref() == Some(s.id.as_str()) {
                                EditStrainCard {
                                    key: "{s.id}",
                                    item: s.clone(),
                                    on_saved: move |_| { editing_id.set(None); let next = reload.read().wrapping_add(1); reload.set(next); },
                                    on_cancel: move |_| editing_id.set(None),
                                }
                            } else {
                                ItemRow {
                                    key: "{s.id}",
                                    name: s.name.clone(),
                                    sub: format!("{} • {}฿/г • {}г", s.category.clone().unwrap_or_default(), s.price_per_gram, s.available_grams.unwrap_or(0.0)),
                                    is_available: s.is_available,
                                    on_edit: {
                                        let id = s.id.clone();
                                        move |_| editing_id.set(Some(id.clone()))
                                    },
                                    on_toggle: {
                                        let id = s.id.clone();
                                        move |_| {
                                            let id = id.clone();
                                            spawn(async move {
                                                let url = format!("{}/api/strains/{}/toggle-availability", api_base_url(), id);
                                                let _ = reqwest::Client::new().post(&url)
                                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                    .send().await;
                                                let next = reload.read().wrapping_add(1); reload.set(next);
                                            });
                                        }
                                    },
                                    on_delete: {
                                        let id = s.id.clone();
                                        move |_| {
                                            let id = id.clone();
                                            spawn(async move {
                                                let url = format!("{}/api/strains/{}", api_base_url(), id);
                                                let _ = reqwest::Client::new().delete(&url)
                                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                    .send().await;
                                                let next = reload.read().wrapping_add(1); reload.set(next);
                                            });
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

// ── Accessories tab ───────────────────────────────────────────

#[component]
fn AccessoriesTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let mut name = use_signal(String::new);
    let mut category = use_signal(|| "other".to_string());
    let mut price = use_signal(String::new);
    let mut stock = use_signal(String::new);
    let mut image_url = use_signal(String::new);
    // EN fields
    let mut name_en = use_signal(String::new);
    let mut description_en = use_signal(String::new);
    let mut category_en = use_signal(String::new);
    let mut status = use_signal(String::new);
    let mut reload = use_signal(|| 0u32);
    let mut editing_id: Signal<Option<String>> = use_signal(|| None);

    let items = use_resource(move || async move {
        let _ = reload.read();
        let url = format!("{}/api/accessories", api_base_url());
        reqwest::Client::new().get(&url).send().await
            .map_err(|e| e.to_string())?
            .json::<AccessoriesResp>().await
            .map(|r| r.accessories)
            .map_err(|e| e.to_string())
    });

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
                    input { style: input_style(), placeholder: "Количество", value: "{stock}", r#type: "number",
                        oninput: move |e| stock.set(e.value()) }
                    input { style: input_style(), placeholder: "URL картинки (опц.)", value: "{image_url}",
                        oninput: move |e| image_url.set(e.value()) }

                    // ── EN section ──
                    div { style: en_section_style(), "🇬🇧 English (optional)" }
                    input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}",
                        oninput: move |e| name_en.set(e.value()) }
                    input { style: input_style(), placeholder: "Description (EN)", value: "{description_en}",
                        oninput: move |e| description_en.set(e.value()) }
                    input { style: input_style(), placeholder: "Category (EN)", value: "{category_en}",
                        oninput: move |e| category_en.set(e.value()) }

                    button { style: submit_btn_style(),
                        onclick: move |_| {
                            let n = name(); let c = category(); let p = price.read().parse::<f64>().unwrap_or(0.0);
                            let s = stock.read().parse::<i32>().unwrap_or(0);
                            let img = image_url();
                            let ne = name_en(); let de = description_en(); let ce = category_en();
                            if n.trim().is_empty() || p <= 0.0 { status.set("❌ Заполните название и цену".into()); return; }
                            spawn(async move {
                                let body = json!({
                                    "name": n, "category": c, "price": p, "stock": s,
                                    "image_url": if img.is_empty() { None } else { Some(img) },
                                    "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                    "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                    "category_en": if ce.is_empty() { serde_json::Value::Null } else { ce.into() },
                                });
                                let url = format!("{}/api/accessories", api_base_url());
                                let res = reqwest::Client::new().post(&url)
                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        status.set("✓ Аксессуар добавлен".into());
                                        name.set(String::new()); price.set(String::new());
                                        stock.set(String::new()); image_url.set(String::new());
                                        name_en.set(String::new()); description_en.set(String::new());
                                        category_en.set(String::new());
                                        let next = reload.read().wrapping_add(1); reload.set(next);
                                    }
                                    Ok(r) => status.set(format!("❌ HTTP {}", r.status().as_u16())),
                                    Err(e) => status.set(format!("❌ {}", e)),
                                }
                            });
                        },
                        "Добавить"
                    }
                    if !status.read().is_empty() { div { style: "padding:8px;color:#39ff14;font-size:13px;", "{status}" } }
                }
            }

            h3 { style: list_title_style(), "Все аксессуары" }
            match &*items.read_unchecked() {
                None => rsx!(div { style: "color:#888;", "Загрузка..." }),
                Some(Err(e)) => rsx!(div { style: "color:#ff4757;", "Ошибка: {e}" }),
                Some(Ok(list)) => rsx!{
                    div { style: "display:flex;flex-direction:column;gap:8px;",
                        for a in list.clone() {
                            if editing_id.read().as_deref() == Some(a.id.as_str()) {
                                EditAccessoryCard {
                                    key: "{a.id}",
                                    item: a.clone(),
                                    on_saved: move |_| { editing_id.set(None); let next = reload.read().wrapping_add(1); reload.set(next); },
                                    on_cancel: move |_| editing_id.set(None),
                                }
                            } else {
                                ItemRow {
                                    key: "{a.id}",
                                    name: a.name.clone(),
                                    sub: format!("{} • {}฿ • {} шт.", a.category.clone().unwrap_or_default(), a.price, a.stock.unwrap_or(0)),
                                    is_available: a.is_available,
                                    on_edit: {
                                        let id = a.id.clone();
                                        move |_| editing_id.set(Some(id.clone()))
                                    },
                                    on_toggle: {
                                        let id = a.id.clone();
                                        move |_| {
                                            let id = id.clone();
                                            spawn(async move {
                                                // No toggle endpoint for accessories — use DELETE which sets is_available=false.
                                                let url = format!("{}/api/accessories/{}", api_base_url(), id);
                                                let _ = reqwest::Client::new().delete(&url)
                                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                    .send().await;
                                                let next = reload.read().wrapping_add(1); reload.set(next);
                                            });
                                        }
                                    },
                                    on_delete: {
                                        let id = a.id.clone();
                                        move |_| {
                                            let id = id.clone();
                                            spawn(async move {
                                                let url = format!("{}/api/accessories/{}", api_base_url(), id);
                                                let _ = reqwest::Client::new().delete(&url)
                                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                    .send().await;
                                                let next = reload.read().wrapping_add(1); reload.set(next);
                                            });
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

// ── Tea tab ───────────────────────────────────────────────────

#[component]
fn TeaTab() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let mut name = use_signal(String::new);
    let mut subcategory = use_signal(|| "green".to_string());
    let mut price = use_signal(String::new);
    let mut stock = use_signal(String::new);
    // EN fields
    let mut name_en = use_signal(String::new);
    let mut description_en = use_signal(String::new);
    let mut subcategory_en = use_signal(String::new);
    let mut status = use_signal(String::new);
    let mut reload = use_signal(|| 0u32);
    let mut editing_id: Signal<Option<String>> = use_signal(|| None);

    let items = use_resource(move || async move {
        let _ = reload.read();
        let url = format!("{}/api/tea-products", api_base_url());
        reqwest::Client::new().get(&url).send().await
            .map_err(|e| e.to_string())?
            .json::<TeaResp>().await
            .map(|r| r.tea_products)
            .map_err(|e| e.to_string())
    });

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
                    input { style: input_style(), placeholder: "Количество", value: "{stock}", r#type: "number",
                        oninput: move |e| stock.set(e.value()) }

                    // ── EN section ──
                    div { style: en_section_style(), "🇬🇧 English (optional)" }
                    input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}",
                        oninput: move |e| name_en.set(e.value()) }
                    input { style: input_style(), placeholder: "Description (EN)", value: "{description_en}",
                        oninput: move |e| description_en.set(e.value()) }
                    input { style: input_style(), placeholder: "Subcategory (EN, e.g. Green Tea)", value: "{subcategory_en}",
                        oninput: move |e| subcategory_en.set(e.value()) }

                    button { style: submit_btn_style(),
                        onclick: move |_| {
                            let n = name(); let sc = subcategory(); let p = price.read().parse::<f64>().unwrap_or(0.0);
                            let s = stock.read().parse::<i32>().unwrap_or(0);
                            let ne = name_en(); let de = description_en(); let sce = subcategory_en();
                            if n.trim().is_empty() || p <= 0.0 { status.set("❌ Заполните название и цену".into()); return; }
                            spawn(async move {
                                let body = json!({
                                    "name": n, "subcategory": sc, "price": p, "stock": s,
                                    "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                    "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                    "subcategory_en": if sce.is_empty() { serde_json::Value::Null } else { sce.into() },
                                });
                                let url = format!("{}/api/tea-products", api_base_url());
                                let res = reqwest::Client::new().post(&url)
                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                    .json(&body).send().await;
                                match res {
                                    Ok(r) if r.status().is_success() => {
                                        status.set("✓ Чай добавлен".into());
                                        name.set(String::new()); price.set(String::new()); stock.set(String::new());
                                        name_en.set(String::new()); description_en.set(String::new());
                                        subcategory_en.set(String::new());
                                        let next = reload.read().wrapping_add(1); reload.set(next);
                                    }
                                    Ok(r) => status.set(format!("❌ HTTP {}", r.status().as_u16())),
                                    Err(e) => status.set(format!("❌ {}", e)),
                                }
                            });
                        },
                        "Добавить"
                    }
                    if !status.read().is_empty() { div { style: "padding:8px;color:#39ff14;font-size:13px;", "{status}" } }
                }
            }

            h3 { style: list_title_style(), "Весь чай" }
            match &*items.read_unchecked() {
                None => rsx!(div { style: "color:#888;", "Загрузка..." }),
                Some(Err(e)) => rsx!(div { style: "color:#ff4757;", "Ошибка: {e}" }),
                Some(Ok(list)) => rsx!{
                    div { style: "display:flex;flex-direction:column;gap:8px;",
                        for t in list.clone() {
                            if editing_id.read().as_deref() == Some(t.id.as_str()) {
                                EditTeaCard {
                                    key: "{t.id}",
                                    item: t.clone(),
                                    on_saved: move |_| { editing_id.set(None); let next = reload.read().wrapping_add(1); reload.set(next); },
                                    on_cancel: move |_| editing_id.set(None),
                                }
                            } else {
                                ItemRow {
                                    key: "{t.id}",
                                    name: t.name.clone(),
                                    sub: format!("{} • {}฿ • {} шт.", t.subcategory.clone().unwrap_or_default(), t.price, t.stock.unwrap_or(0)),
                                    is_available: t.is_available,
                                    on_edit: {
                                        let id = t.id.clone();
                                        move |_| editing_id.set(Some(id.clone()))
                                    },
                                    on_toggle: {
                                        let id = t.id.clone();
                                        move |_| {
                                            let id = id.clone();
                                            spawn(async move {
                                                let url = format!("{}/api/tea-products/{}", api_base_url(), id);
                                                let _ = reqwest::Client::new().delete(&url)
                                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                    .send().await;
                                                let next = reload.read().wrapping_add(1); reload.set(next);
                                            });
                                        }
                                    },
                                    on_delete: {
                                        let id = t.id.clone();
                                        move |_| {
                                            let id = id.clone();
                                            spawn(async move {
                                                let url = format!("{}/api/tea-products/{}", api_base_url(), id);
                                                let _ = reqwest::Client::new().delete(&url)
                                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                    .send().await;
                                                let next = reload.read().wrapping_add(1); reload.set(next);
                                            });
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

// ── Shared UI components ──────────────────────────────────────

#[component]
fn FormCard(title: String, children: Element) -> Element {
    rsx! {
        div { style: "background:#1a1a2e;padding:16px;border-radius:8px;margin-bottom:20px;border:1px solid #2a2a4a;",
            h3 { style: "color:#39ff14;font-size:15px;margin-bottom:12px;", "{title}" }
            div { style: "display:flex;flex-direction:column;gap:8px;",
                {children}
            }
        }
    }
}

#[component]
fn ItemRow(
    name: String, sub: String, is_available: bool,
    on_edit: EventHandler<()>,
    on_toggle: EventHandler<()>, on_delete: EventHandler<()>,
) -> Element {
    let badge = if is_available { ("#39ff14", "ВКЛ") } else { ("#666", "ВЫКЛ") };
    let toggle_label = if is_available { "Скрыть" } else { "Показать" };
    rsx! {
        div { style: "background:#1a1a2e;padding:10px 12px;border-radius:6px;display:flex;align-items:center;gap:6px;cursor:pointer;",
            onclick: move |_| on_edit.call(()),
            div { style: "flex:1;min-width:0;",
                div { style: "font-weight:600;font-size:14px;color:#e8e8e8;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;", "{name}" }
                div { style: "font-size:11px;color:#888;margin-top:2px;", "{sub}" }
            }
            span { style: "font-size:10px;padding:2px 6px;background:{badge.0}20;color:{badge.0};border-radius:10px;font-weight:600;",
                "{badge.1}"
            }
            button { style: "padding:6px 8px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:11px;cursor:pointer;",
                onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_edit.call(()); },
                "✏️"
            }
            button { style: "padding:6px 8px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-size:11px;cursor:pointer;",
                onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_toggle.call(()); },
                "{toggle_label}"
            }
            button { style: "padding:6px 8px;background:#3a1a1a;color:#ff8888;border:none;border-radius:4px;font-size:11px;cursor:pointer;",
                onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_delete.call(()); },
                "🗑"
            }
        }
    }
}

fn cancel_btn_style() -> &'static str {
    "padding:10px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;font-weight:600;font-size:13px;cursor:pointer;margin-top:4px;"
}
fn edit_card_style() -> &'static str {
    "background:#1a1a2e;border:1px solid #6699ff;border-radius:8px;padding:12px;display:flex;flex-direction:column;gap:8px;"
}
fn edit_header_style() -> &'static str {
    "color:#6699ff;font-size:13px;font-weight:700;text-transform:uppercase;letter-spacing:1px;"
}

// ── Inline edit components ───────────────────────────────────

#[component]
fn EditStrainCard(
    item: AdminStrain,
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let mut name = use_signal(|| item.name.clone());
    let mut category = use_signal(|| item.category.clone().unwrap_or_else(|| "hybrid".to_string()));
    let mut price = use_signal(|| item.price_per_gram.to_string());
    let mut grams = use_signal(|| item.available_grams.unwrap_or(0.0).to_string());
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut effect_en = use_signal(|| item.effect_en.clone().unwrap_or_default());
    let mut flavor_profile_en = use_signal(|| item.flavor_profile_en.clone().unwrap_or_default());
    let mut strain_type_en = use_signal(|| item.strain_type_en.clone().unwrap_or_default());
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    let item_is_available = item.is_available;
    rsx! {
        div { style: edit_card_style(),
            div { style: edit_header_style(), "✏️ Редактирование" }
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
            input { style: input_style(), placeholder: "Граммы в наличии", value: "{grams}", r#type: "number",
                oninput: move |e| grams.set(e.value()) }
            div { style: en_section_style(), "🇬🇧 English (optional)" }
            input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}",
                oninput: move |e| name_en.set(e.value()) }
            input { style: input_style(), placeholder: "Description (EN)", value: "{description_en}",
                oninput: move |e| description_en.set(e.value()) }
            input { style: input_style(), placeholder: "Effect (EN)", value: "{effect_en}",
                oninput: move |e| effect_en.set(e.value()) }
            input { style: input_style(), placeholder: "Flavor profile (EN)", value: "{flavor_profile_en}",
                oninput: move |e| flavor_profile_en.set(e.value()) }
            input { style: input_style(), placeholder: "Strain type (EN, e.g. Hybrid)", value: "{strain_type_en}",
                oninput: move |e| strain_type_en.set(e.value()) }
            div { style: "display:flex;gap:8px;",
                button { style: submit_btn_style(),
                    onclick: move |_| {
                        let n = name(); let c = category(); let p = price.read().parse::<f64>().unwrap_or(0.0);
                        let g = grams.read().parse::<f64>().unwrap_or(0.0);
                        let ne = name_en(); let de = description_en(); let ee = effect_en();
                        let fpe = flavor_profile_en(); let ste = strain_type_en();
                        let id = item_id.clone();
                        spawn(async move {
                            let body = json!({
                                "name": n, "category": c, "price_per_gram": p,
                                "available_grams": g, "is_available": item_is_available,
                                "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                "effect_en": if ee.is_empty() { serde_json::Value::Null } else { ee.into() },
                                "flavor_profile_en": if fpe.is_empty() { serde_json::Value::Null } else { fpe.into() },
                                "strain_type_en": if ste.is_empty() { serde_json::Value::Null } else { ste.into() },
                            });
                            let url = format!("{}/api/strains/{}", api_base_url(), id);
                            let res = reqwest::Client::new().put(&url)
                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                .json(&body).send().await;
                            match res {
                                Ok(r) if r.status().is_success() => on_saved.call(()),
                                Ok(r) => status.set(format!("❌ HTTP {}", r.status().as_u16())),
                                Err(e) => status.set(format!("❌ {}", e)),
                            }
                        });
                    },
                    "💾 Сохранить"
                }
                button { style: cancel_btn_style(),
                    onclick: move |_| on_cancel.call(()),
                    "Отмена"
                }
            }
            if !status.read().is_empty() { div { style: "padding:8px;color:#ff4757;font-size:13px;", "{status}" } }
        }
    }
}

#[component]
fn EditAccessoryCard(
    item: AdminAccessory,
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let mut name = use_signal(|| item.name.clone());
    let mut category = use_signal(|| item.category.clone().unwrap_or_else(|| "other".to_string()));
    let mut price = use_signal(|| item.price.to_string());
    let mut stock = use_signal(|| item.stock.map(|s| s.to_string()).unwrap_or_default());
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut category_en = use_signal(|| item.category_en.clone().unwrap_or_default());
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    let item_is_available = item.is_available;
    rsx! {
        div { style: edit_card_style(),
            div { style: edit_header_style(), "✏️ Редактирование" }
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
            input { style: input_style(), placeholder: "Количество", value: "{stock}", r#type: "number",
                oninput: move |e| stock.set(e.value()) }
            div { style: en_section_style(), "🇬🇧 English (optional)" }
            input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}",
                oninput: move |e| name_en.set(e.value()) }
            input { style: input_style(), placeholder: "Description (EN)", value: "{description_en}",
                oninput: move |e| description_en.set(e.value()) }
            input { style: input_style(), placeholder: "Category (EN)", value: "{category_en}",
                oninput: move |e| category_en.set(e.value()) }
            div { style: "display:flex;gap:8px;",
                button { style: submit_btn_style(),
                    onclick: move |_| {
                        let n = name(); let c = category(); let p = price.read().parse::<f64>().unwrap_or(0.0);
                        let s_str = stock();
                        let s_val: serde_json::Value = if s_str.is_empty() { serde_json::Value::Null } else { s_str.parse::<i32>().unwrap_or(0).into() };
                        let ne = name_en(); let de = description_en(); let ce = category_en();
                        let id = item_id.clone();
                        spawn(async move {
                            let body = json!({
                                "name": n, "category": c, "price": p, "stock": s_val,
                                "is_available": item_is_available,
                                "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                "category_en": if ce.is_empty() { serde_json::Value::Null } else { ce.into() },
                            });
                            let url = format!("{}/api/accessories/{}", api_base_url(), id);
                            let res = reqwest::Client::new().put(&url)
                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                .json(&body).send().await;
                            match res {
                                Ok(r) if r.status().is_success() => on_saved.call(()),
                                Ok(r) => status.set(format!("❌ HTTP {}", r.status().as_u16())),
                                Err(e) => status.set(format!("❌ {}", e)),
                            }
                        });
                    },
                    "💾 Сохранить"
                }
                button { style: cancel_btn_style(),
                    onclick: move |_| on_cancel.call(()),
                    "Отмена"
                }
            }
            if !status.read().is_empty() { div { style: "padding:8px;color:#ff4757;font-size:13px;", "{status}" } }
        }
    }
}

#[component]
fn EditTeaCard(
    item: AdminTea,
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let mut name = use_signal(|| item.name.clone());
    let mut subcategory = use_signal(|| item.subcategory.clone().unwrap_or_else(|| "green".to_string()));
    let mut price = use_signal(|| item.price.to_string());
    let mut stock = use_signal(|| item.stock.map(|s| s.to_string()).unwrap_or_default());
    let mut name_en = use_signal(|| item.name_en.clone().unwrap_or_default());
    let mut description_en = use_signal(|| item.description_en.clone().unwrap_or_default());
    let mut subcategory_en = use_signal(|| item.subcategory_en.clone().unwrap_or_default());
    let mut status = use_signal(String::new);
    let item_id = item.id.clone();
    let item_is_available = item.is_available;
    rsx! {
        div { style: edit_card_style(),
            div { style: edit_header_style(), "✏️ Редактирование" }
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
            input { style: input_style(), placeholder: "Количество", value: "{stock}", r#type: "number",
                oninput: move |e| stock.set(e.value()) }
            div { style: en_section_style(), "🇬🇧 English (optional)" }
            input { style: input_style(), placeholder: "Name (EN)", value: "{name_en}",
                oninput: move |e| name_en.set(e.value()) }
            input { style: input_style(), placeholder: "Description (EN)", value: "{description_en}",
                oninput: move |e| description_en.set(e.value()) }
            input { style: input_style(), placeholder: "Subcategory (EN, e.g. Green Tea)", value: "{subcategory_en}",
                oninput: move |e| subcategory_en.set(e.value()) }
            div { style: "display:flex;gap:8px;",
                button { style: submit_btn_style(),
                    onclick: move |_| {
                        let n = name(); let sc = subcategory(); let p = price.read().parse::<f64>().unwrap_or(0.0);
                        let s_str = stock();
                        let s_val: serde_json::Value = if s_str.is_empty() { serde_json::Value::Null } else { s_str.parse::<i32>().unwrap_or(0).into() };
                        let ne = name_en(); let de = description_en(); let sce = subcategory_en();
                        let id = item_id.clone();
                        spawn(async move {
                            let body = json!({
                                "name": n, "subcategory": sc, "price": p, "stock": s_val,
                                "is_available": item_is_available,
                                "name_en": if ne.is_empty() { serde_json::Value::Null } else { ne.into() },
                                "description_en": if de.is_empty() { serde_json::Value::Null } else { de.into() },
                                "subcategory_en": if sce.is_empty() { serde_json::Value::Null } else { sce.into() },
                            });
                            let url = format!("{}/api/tea-products/{}", api_base_url(), id);
                            let res = reqwest::Client::new().put(&url)
                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                .json(&body).send().await;
                            match res {
                                Ok(r) if r.status().is_success() => on_saved.call(()),
                                Ok(r) => status.set(format!("❌ HTTP {}", r.status().as_u16())),
                                Err(e) => status.set(format!("❌ {}", e)),
                            }
                        });
                    },
                    "💾 Сохранить"
                }
                button { style: cancel_btn_style(),
                    onclick: move |_| on_cancel.call(()),
                    "Отмена"
                }
            }
            if !status.read().is_empty() { div { style: "padding:8px;color:#ff4757;font-size:13px;", "{status}" } }
        }
    }
}

fn input_style() -> &'static str {
    "padding:10px 12px;background:#0f0f1a;color:#e8e8e8;border:1px solid #2a2a4a;border-radius:4px;font-size:14px;"
}
fn submit_btn_style() -> &'static str {
    "padding:12px;background:#39ff14;color:#000;border:none;border-radius:4px;font-weight:700;font-size:14px;cursor:pointer;margin-top:4px;"
}
fn list_title_style() -> &'static str {
    "color:#888;font-size:13px;margin:16px 0 8px;text-transform:uppercase;letter-spacing:1px;"
}
fn en_section_style() -> &'static str {
    "padding:6px 0 2px;color:#6699ff;font-size:12px;font-weight:600;border-top:1px solid #2a2a4a;margin-top:4px;"
}
