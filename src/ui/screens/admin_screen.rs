// Admin Panel — products management for Telegram admins.
//
// Reads telegram_id from the WebApp SDK and sends it in the
// `X-Admin-Telegram-Id` header for every write request. The backend
// validates the id against the ADMIN_IDS env list.
//
// Features:
//   • Live access check (GET /api/admin/check)
//   • Strain list with toggle-availability button
//   • New-strain form (name + category + price + thc + grams)

use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::api::context::api_base_url;
use crate::ui::telegram::use_telegram_id;

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct AdminStrain {
    id: String,
    name: String,
    category: Option<String>,
    price_per_gram: f64,
    available_grams: Option<f64>,
    is_available: bool,
}

#[derive(Debug, Deserialize)]
struct StrainsResponse {
    strains: Vec<AdminStrain>,
}

#[derive(Debug, Deserialize)]
struct AdminCheck {
    is_admin: bool,
}

#[component]
pub fn AdminScreen() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);

    // form state
    let mut name = use_signal(String::new);
    let mut category = use_signal(|| "hybrid".to_string());
    let mut price = use_signal(String::new);
    let mut thc = use_signal(String::new);
    let mut grams = use_signal(String::new);
    let mut status_msg = use_signal(String::new);
    let mut reload_token = use_signal(|| 0u32);

    // admin-access check
    let access_check = use_resource(move || async move {
        let base = api_base_url();
        let url = format!("{}/api/admin/check?telegram_id={}", base, telegram_id);
        reqwest::Client::new()
            .get(&url)
            .send().await
            .map_err(|e| e.to_string())?
            .json::<AdminCheck>().await
            .map(|c| c.is_admin)
            .map_err(|e| e.to_string())
    });

    // strain list (reloaded when reload_token changes)
    let strains_resource = use_resource(move || async move {
        let _ = reload_token.read(); // dependency
        let base = api_base_url();
        let url = format!("{}/api/strains", base);
        reqwest::Client::new()
            .get(&url)
            .send().await
            .map_err(|e| e.to_string())?
            .json::<StrainsResponse>().await
            .map(|r| r.strains)
            .map_err(|e| e.to_string())
    });

    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding:16px;padding-bottom:80px;",
            h1 { style: "font-size:24px;color:#ff4757;margin-bottom:8px;",
                "🔧 Admin Panel"
            }
            p { style: "font-size:12px;color:#888;margin-bottom:16px;",
                "Telegram ID: {telegram_id}"
            }

            // ── access status ─────────────────────────────────
            {
                match &*access_check.read() {
                    Some(Ok(true)) => rsx! {
                        div { style: "padding:8px 12px;background:rgba(57,255,20,0.1);border:1px solid #39ff14;color:#39ff14;border-radius:6px;margin-bottom:16px;font-size:13px;",
                            "✓ Доступ разрешён"
                        }
                    },
                    Some(Ok(false)) => rsx! {
                        div { style: "padding:12px;background:rgba(255,71,87,0.1);border:1px solid #ff4757;color:#ff4757;border-radius:6px;margin-bottom:16px;",
                            "⛔ У вас нет прав администратора. Попросите владельца добавить ваш Telegram ID в ADMIN_IDS."
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { style: "padding:12px;background:rgba(255,71,87,0.1);color:#ff4757;border-radius:6px;margin-bottom:16px;font-size:12px;",
                            "Ошибка проверки доступа: {e}"
                        }
                    },
                    None => rsx! {
                        div { style: "padding:12px;color:#888;font-size:13px;", "Проверка доступа..." }
                    },
                }
            }

            // ── add strain form ──────────────────────────────
            div { style: "padding:16px;background:#1a1a2e;border:1px solid #2a2a4a;border-radius:8px;margin-bottom:20px;",
                div { style: "font-size:16px;font-weight:700;color:#39ff14;margin-bottom:12px;", "➕ Добавить товар" }

                input {
                    style: "width:100%;padding:10px;background:#0f0f1a;border:1px solid #2a2a4a;color:#e8e8e8;border-radius:6px;margin-bottom:8px;",
                    placeholder: "Название (например: GORILLA GLUE)",
                    value: "{name}",
                    oninput: move |e| name.set(e.value()),
                }
                select {
                    style: "width:100%;padding:10px;background:#0f0f1a;border:1px solid #2a2a4a;color:#e8e8e8;border-radius:6px;margin-bottom:8px;",
                    value: "{category}",
                    onchange: move |e| category.set(e.value()),
                    option { value: "sativa", "☀️ Sativa" }
                    option { value: "indica", "🌙 Indica" }
                    option { value: "hybrid", "⚖️ Hybrid" }
                }
                div { style: "display:grid;grid-template-columns:1fr 1fr 1fr;gap:8px;margin-bottom:8px;",
                    input {
                        r#type: "number",
                        style: "padding:10px;background:#0f0f1a;border:1px solid #2a2a4a;color:#e8e8e8;border-radius:6px;",
                        placeholder: "Цена ฿/г",
                        value: "{price}",
                        oninput: move |e| price.set(e.value()),
                    }
                    input {
                        r#type: "number",
                        style: "padding:10px;background:#0f0f1a;border:1px solid #2a2a4a;color:#e8e8e8;border-radius:6px;",
                        placeholder: "THC %",
                        value: "{thc}",
                        oninput: move |e| thc.set(e.value()),
                    }
                    input {
                        r#type: "number",
                        style: "padding:10px;background:#0f0f1a;border:1px solid #2a2a4a;color:#e8e8e8;border-radius:6px;",
                        placeholder: "Граммов",
                        value: "{grams}",
                        oninput: move |e| grams.set(e.value()),
                    }
                }

                button {
                    style: "width:100%;padding:12px;background:#39ff14;color:#000;font-weight:700;border:none;border-radius:6px;cursor:pointer;",
                    onclick: move |_| {
                        let name_val = name.read().clone();
                        let cat_val = category.read().clone();
                        let price_val: f64 = price.read().parse().unwrap_or(0.0);
                        let thc_val: Option<f64> = thc.read().parse().ok();
                        let grams_val: Option<f64> = grams.read().parse().ok();
                        let mut status = status_msg.clone();
                        let mut reload = reload_token.clone();
                        let mut nm = name.clone();
                        let mut pr = price.clone();
                        let mut th = thc.clone();
                        let mut gr = grams.clone();
                        if name_val.trim().is_empty() || price_val <= 0.0 {
                            status.set("⚠️ Заполните название и цену".into());
                            return;
                        }
                        spawn(async move {
                            let base = api_base_url();
                            let url = format!("{}/api/strains", base);
                            let body = serde_json::json!({
                                "name": name_val,
                                "category": cat_val,
                                "price_per_gram": price_val,
                                "thc_percent": thc_val,
                                "available_grams": grams_val,
                            });
                            let res = reqwest::Client::new()
                                .post(&url)
                                .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                .json(&body)
                                .send().await;
                            match res {
                                Ok(r) if r.status().is_success() => {
                                    status.set("✅ Товар добавлен".into());
                                    nm.set(String::new());
                                    pr.set(String::new());
                                    th.set(String::new());
                                    gr.set(String::new());
                                    let next = reload.read().wrapping_add(1);
                                    reload.set(next);
                                }
                                Ok(r) => status.set(format!("❌ HTTP {}", r.status().as_u16())),
                                Err(e) => status.set(format!("❌ {}", e)),
                            }
                        });
                    },
                    "Сохранить"
                }

                if !status_msg.read().is_empty() {
                    div { style: "margin-top:8px;font-size:13px;color:#888;", "{status_msg}" }
                }
            }

            // ── strains list ─────────────────────────────────
            div { style: "font-size:16px;font-weight:700;margin-bottom:8px;color:#39ff14;",
                "🌿 Товары"
            }
            {
                match &*strains_resource.read() {
                    Some(Ok(strains)) => rsx! {
                        for s in strains.iter().cloned() {
                            div {
                                key: "{s.id}",
                                style: "display:flex;align-items:center;gap:10px;padding:10px;background:#1a1a2e;border-radius:6px;margin-bottom:6px;",
                                div { style: "flex:1;",
                                    div { style: "font-weight:600;font-size:14px;", "{s.name}" }
                                    div { style: "font-size:11px;color:#888;",
                                        "{s.category.clone().unwrap_or_default()} • {s.price_per_gram as i32}฿/г • {s.available_grams.unwrap_or(0.0) as i32}г"
                                    }
                                }
                                button {
                                    style: if s.is_available {
                                        "padding:6px 10px;background:#39ff14;color:#000;border:none;border-radius:4px;font-size:11px;font-weight:700;cursor:pointer;"
                                    } else {
                                        "padding:6px 10px;background:#444;color:#fff;border:none;border-radius:4px;font-size:11px;cursor:pointer;"
                                    },
                                    onclick: {
                                        let sid = s.id.clone();
                                        let new_val = !s.is_available;
                                        let mut status = status_msg.clone();
                                        let mut reload = reload_token.clone();
                                        move |_| {
                                            let sid = sid.clone();
                                            let mut status = status.clone();
                                            let mut reload = reload.clone();
                                            spawn(async move {
                                                let base = api_base_url();
                                                let url = format!("{}/api/strains/{}/availability", base, sid);
                                                let body = serde_json::json!({ "is_available": new_val });
                                                let res = reqwest::Client::new()
                                                    .put(&url)
                                                    .header("X-Admin-Telegram-Id", telegram_id.to_string())
                                                    .json(&body)
                                                    .send().await;
                                                match res {
                                                    Ok(r) if r.status().is_success() => {
                                                        status.set(format!("✓ {} обновлён", sid));
                                                        let next = reload.read().wrapping_add(1);
                                                        reload.set(next);
                                                    }
                                                    Ok(r) => status.set(format!("❌ HTTP {}", r.status().as_u16())),
                                                    Err(e) => status.set(format!("❌ {}", e)),
                                                }
                                            });
                                        }
                                    },
                                    if s.is_available { "вкл" } else { "выкл" }
                                }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { style: "padding:12px;color:#ff4757;", "Ошибка: {e}" }
                    },
                    None => rsx! { div { style:"color:#888;padding:12px;", "Загрузка..." } },
                }
            }
        }
    }
}
