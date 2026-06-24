use crate::trios::garden::{calculate_progress, GrowthStage, Plant};
use crate::trios::i18n::{
    t, T_BTN_WATER, T_GARDEN_EMPTY_CTA, T_GARDEN_EMPTY_LABEL, T_GARDEN_LOADING, T_GARDEN_SUBTITLE,
    T_GARDEN_TITLE,
};
use crate::ui::api::context::api_base_url;
use crate::ui::components::ErrorBanner;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

#[derive(Debug, Clone, serde::Deserialize)]
struct ApiPlant {
    id: String,
    strain_id: String,
    strain_name: String,
    current_stage: GrowthStage,
    water_count: u32,
    is_completed: bool,
    planted_at: i64,
    #[serde(default)]
    last_watered_at: Option<i64>,
    reward_claimed: bool,
    // B3: chosen target product (seed = its photo).
    #[serde(default)]
    target_name: Option<String>,
    #[serde(default)]
    target_image_url: Option<String>,
}

impl From<&ApiPlant> for Plant {
    fn from(a: &ApiPlant) -> Self {
        Plant {
            id: a.id.clone(),
            user_id: String::new(),
            strain_id: a.strain_id.clone(),
            strain_name: a.strain_name.clone(),
            current_stage: a.current_stage,
            planted_at: a.planted_at,
            is_completed: a.is_completed,
            harvested_at: None,
            reward_claimed: a.reward_claimed,
            water_count: a.water_count,
            last_watered_at: a.last_watered_at,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct GardenProduct {
    catalog: String,
    id: String,
    name: String,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    price: f64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct GardenProductsResponse {
    products: Vec<GardenProduct>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct GardenResponse {
    plants: Vec<ApiPlant>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct WaterPlantResponse {
    success: bool,
    #[serde(default)]
    water_count: u32,
    #[serde(default)]
    current_stage: Option<String>,
    #[serde(default)]
    is_completed: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    next_water_at: Option<i64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct HarvestPlantResponse {
    success: bool,
    #[serde(default)]
    error: Option<String>,
}

async fn fetch_plants(telegram_id: i64, init_data: &str) -> Result<Vec<ApiPlant>, String> {
    let base = api_base_url();
    let url = format!("{}/api/garden/plants?telegram_id={}", base, telegram_id);
    let text = crate::ui::api::http::fetch_text_authed(&url, init_data).await?;
    serde_json::from_str::<GardenResponse>(&text)
        .map_err(|e| format!("Parse error: {e}"))
        .map(|r| r.plants)
}

async fn fetch_garden_products() -> Result<Vec<GardenProduct>, String> {
    let base = api_base_url();
    let url = format!("{}/api/garden/products", base);
    let text = crate::ui::api::http::fetch_text(&url).await?;
    serde_json::from_str::<GardenProductsResponse>(&text)
        .map_err(|e| format!("Parse error: {e}"))
        .map(|r| r.products)
}

/// POST a chosen product to plant it. Returns Ok(()) or a server error code.
async fn choose_plant_api(
    telegram_id: i64,
    catalog: &str,
    product_id: &str,
    init_data: &str,
) -> Result<(), String> {
    let base = api_base_url();
    let url = format!("{}/api/garden/plants/choose", base);
    let body = serde_json::json!({
        "telegram_id": telegram_id,
        "catalog": catalog,
        "product_id": product_id,
    })
    .to_string();
    let text = crate::ui::api::http::post_json_authed(&url, init_data, &body).await?;
    let resp: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Parse error: {e}"))?;
    if resp
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err(resp
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("choose_failed")
            .to_string())
    }
}

async fn water_plant_api(plant_id: &str, init_data: &str) -> Result<WaterPlantResponse, String> {
    let base = api_base_url();
    let url = format!(
        "{}/api/garden/plants/{}/water",
        base,
        urlencoding::encode(plant_id)
    );
    let text = crate::ui::api::http::post_json_authed(&url, init_data, "").await?;
    serde_json::from_str::<WaterPlantResponse>(&text).map_err(|e| format!("Parse error: {e}"))
}

async fn harvest_plant_api(plant_id: &str, init_data: &str) -> Result<(), String> {
    let base = api_base_url();
    let url = format!(
        "{}/api/garden/plants/{}/harvest",
        base,
        urlencoding::encode(plant_id)
    );
    let text = crate::ui::api::http::post_json_authed(&url, init_data, "").await?;
    let resp: HarvestPlantResponse =
        serde_json::from_str(&text).map_err(|e| format!("Parse error: {e}"))?;
    if resp.success {
        Ok(())
    } else {
        Err(resp.error.unwrap_or_else(|| "Harvest failed".into()))
    }
}

fn mock_plants() -> Vec<ApiPlant> {
    let now = chrono::Utc::now().timestamp_millis();
    vec![ApiPlant {
        id: "mock-1".into(),
        strain_id: "banana-fritter".into(),
        strain_name: "Banana Fritter".into(),
        current_stage: GrowthStage::BigVeg,
        planted_at: now.saturating_sub(86400000 * 5),
        is_completed: false,
        reward_claimed: false,
        water_count: 5,
        last_watered_at: Some(now.saturating_sub(120000)),
        target_name: None,
        target_image_url: None,
    }]
}

/// Returns the plant growth stage image URL based on stage index (0-13).
/// Images are at /assets/game/{1..14}.png (single canonical path; the
/// orphan /assets/images/game/ copy was removed in cycle #14).
fn stage_image_url(stage_index: usize) -> String {
    let n = (stage_index + 1).clamp(1, 14);
    format!("/assets/game/{}.png", n)
}

fn stage_color(stage: &GrowthStage) -> &'static str {
    match stage {
        GrowthStage::Seed => "#8b5a2b",
        GrowthStage::Sprout => "#39ff14",
        GrowthStage::FirstLeaf => "#4ade80",
        GrowthStage::YoungBush => "#22c55e",
        GrowthStage::VegStart => "#16a34a",
        GrowthStage::BigVeg => "#00e5ff",
        GrowthStage::PreFlower => "#a855f7",
        GrowthStage::SmallBuds => "#f472b6",
        GrowthStage::BigBuds => "#ff6b35",
        GrowthStage::Trimming => "#fb923c",
        GrowthStage::Curing => "#a78bfa",
        GrowthStage::Lab => "#60a5fa",
        GrowthStage::Delivery => "#34d399",
        GrowthStage::Final => "#ffd700",
    }
}

fn progress_bar_gradient(stage: &GrowthStage) -> &'static str {
    match stage {
        GrowthStage::Seed => "#8b5a2b",
        GrowthStage::Sprout | GrowthStage::FirstLeaf => "#39ff14",
        GrowthStage::YoungBush | GrowthStage::VegStart | GrowthStage::BigVeg => {
            "linear-gradient(90deg, #39ff14, #00e5ff)"
        }
        GrowthStage::PreFlower | GrowthStage::SmallBuds => {
            "linear-gradient(90deg, #00e5ff, #a855f7)"
        }
        GrowthStage::BigBuds | GrowthStage::Trimming => "linear-gradient(90deg, #a855f7, #ff6b35)",
        GrowthStage::Curing | GrowthStage::Lab => "linear-gradient(90deg, #ff6b35, #a78bfa)",
        GrowthStage::Delivery | GrowthStage::Final => "linear-gradient(90deg, #a78bfa, #ffd700)",
    }
}

#[component]
pub fn Garden() -> Element {
    let plants = use_signal(Vec::<ApiPlant>::new);
    let loading = use_signal(|| true);
    let error_msg = use_signal(String::new);
    let now_ms = use_signal(|| chrono::Utc::now().timestamp_millis());
    let mut show_chooser = use_signal(|| false);
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_telegram_init_data();

    {
        let mut plants_c = plants;
        let mut loading_c = loading;
        let mut error_c = error_msg;
        let tid = telegram_id;
        let init = init_data.clone();
        use_future(move || {
            let value = init.clone();
            async move {
                if tid == 0 {
                    plants_c.set(mock_plants());
                    loading_c.set(false);
                    return;
                }
                // Just reflect server state. An EMPTY garden no longer auto-seeds
                // a stage-0 plant (that was the "посадить семечку опять начинается
                // с нуля" bug — it re-created a fresh seed on every empty mount);
                // the empty state shows the "Выбрать товар" chooser so the player
                // plants intentionally. A grown plant now persists (the backend
                // `get_user_plants` was changed to return un-harvested plants).
                match fetch_plants(tid, &value).await {
                    Ok(p) => {
                        plants_c.set(p);
                    }
                    Err(e) => {
                        error_c.set(format!("Не удалось загрузить сад: {}", e));
                    }
                }
                loading_c.set(false);
            }
        });
    }

    {
        let mut now_c = now_ms;
        use_future(move || async move {
            loop {
                TimeoutFuture::new(5000).await;
                now_c.set(chrono::Utc::now().timestamp_millis());
            }
        });
    }

    let plant_list = plants.read().clone();
    let now = *now_ms.read();
    let is_loading = *loading.read();
    let err = error_msg.read().clone();
    let init_for_closures = init_data.clone();

    let bg = "#0f0f1a";
    let bg_card = "#1a1a2e";
    let border_subtle = "rgba(255,255,255,0.1)";
    let title_text = t(crate::ui::lang::current_lang(), T_GARDEN_TITLE);
    let subtitle_text = t(crate::ui::lang::current_lang(), T_GARDEN_SUBTITLE);
    let water_text = t(crate::ui::lang::current_lang(), T_BTN_WATER);
    let loading_text = t(crate::ui::lang::current_lang(), T_GARDEN_LOADING);
    let empty_label = t(crate::ui::lang::current_lang(), T_GARDEN_EMPTY_LABEL);
    let empty_cta = t(crate::ui::lang::current_lang(), T_GARDEN_EMPTY_CTA);

    rsx! {
        div { style: "min-height: 100vh; background: {bg}; color: #e8e8e8; font-family: 'Press Start 2P', monospace; padding-bottom: 80px;",

            div { style: "padding: 20px 16px 12px; text-align: center;",
                h1 { style: "font-size: 18px; color: #39ff14; text-shadow: 0 0 8px rgba(57,255,20,0.5);",
                    "{title_text}"
                }
                p { style: "font-size: 11px; color: #8b8b9e; margin-top: 4px;",
                    "{subtitle_text}"
                }
            }

            ErrorBanner {
                message: err.clone(),
                margin: "0 auto 12px".to_string(),
            }

            // Always-visible chooser: pick (or change) the product to grow a
            // discount for — works even when a plant already exists (it replaces).
            if !is_loading {
                div { style: "text-align:center;padding:0 16px 14px;",
                    button {
                        style: "padding:10px 18px;background:#39ff14;color:#000;border:4px solid #2d9e0f;box-shadow:3px 3px 0 #000;font-size:13px;font-weight:700;cursor:pointer;",
                        onclick: move |_| show_chooser.set(true),
                        if plant_list.is_empty() { "🌱 Выбрать товар для скидки" } else { "🔄 Сменить товар" }
                    }
                }
            }

            if is_loading {
                div { style: "text-align: center; padding: 60px 20px;",
                    div { style: "font-size: 36px; margin-bottom: 12px;", "🌱" }
                    p { style: "font-size: 12px; color: #8b8b9e;", "{loading_text}" }
                }
            } else if plant_list.is_empty() {
                div { style: "text-align: center; padding: 40px 20px;",
                    div { style: "font-size: 48px; margin-bottom: 16px;", "🌱" }
                    p { style: "font-size: 11px; color: #8b8b9e; margin-bottom: 8px;", "{empty_label}" }
                    p { style: "font-size: 13px; color: #8b8b9e;", "{empty_cta}" }
                    p { style: "font-size: 13px; color: #39ff14; margin-top: 12px;", "↑ Нажми «Выбрать товар» вверху" }
                }
            } else {
                div { style: "max-width: 400px; margin: 0 auto; padding: 0 16px;",
                    {plant_list.into_iter().map(|plant| {
                        let core: Plant = (&plant).into();
                        let progress = calculate_progress(&core, now);
                        let pid = plant.id.clone();
                        // Prefer the chosen product's name + photo (the seed); fall
                        // back to the legacy strain + the generic stage sprite.
                        let sname = plant.target_name.clone()
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| plant.strain_name.clone());
                        let wc = plant.water_count;
                        let total_stages = GrowthStage::TOTAL_STAGES;
                        let color = stage_color(&progress.stage).to_string();
                        let gradient = progress_bar_gradient(&progress.stage).to_string();
                        let stage_name = progress.stage_name.clone();
                        let emoji = progress.stage_emoji.clone();
                        let img_url = {
                            // Main hero is ALWAYS the growing-bush sprite (seed →
                            // sprout → … → bud) so the player SEES it grow. The
                            // chosen product shows as a small thumbnail (below).
                            stage_image_url(progress.stage_index)
                        };
                        let product_thumb = {
                            let ti = plant.target_image_url.clone().unwrap_or_default();
                            if ti.starts_with("http://") || ti.starts_with("https://")
                                || (ti.starts_with('/') && !ti.starts_with("//")) {
                                Some(ti)
                            } else {
                                None
                            }
                        };
                        let total_pct = progress.total_progress;
                        let pct_str = format!("{}%", total_pct);
                        let is_active = progress.stage_index > 0 && !progress.is_ready_to_harvest;
                        let can_w = progress.can_water;
                        let is_ready = progress.is_ready_to_harvest;

                        let border = if is_ready {
                            "1px solid #ffd700".to_string()
                        } else if is_active {
                            format!("1px solid {}", color)
                        } else {
                            format!("1px solid {}", border_subtle)
                        };
                        let shadow = if is_ready {
                            "box-shadow: 0 0 16px rgba(255,215,0,0.3);".to_string()
                        } else if is_active {
                            format!("box-shadow: 0 0 16px {}40;", &color)
                        } else {
                            String::new()
                        };

                        let wt = water_text.to_string();

                        let plants_signal = plants;
                        let error_signal = error_msg;
                        let pid_for_water = pid.clone();
                        let init_water = init_for_closures.clone();
                        let water_click = move |_| {
                            let mut ps = plants_signal;
                            let mut es = error_signal;
                            let plant_id = pid_for_water.clone();
                            let init = init_water.clone();
                            spawn(async move {
                                match water_plant_api(&plant_id, &init).await {
                                    Ok(resp) => {
                                        if resp.success {
                                            let mut list = ps.write();
                                            if let Some(p) = list.iter_mut().find(|p| p.id == plant_id) {
                                                p.water_count = resp.water_count;
                                                if let Some(stage_str) = resp.current_stage {
                                                    if let Ok(stage) = serde_json::from_value::<GrowthStage>(serde_json::Value::String(stage_str)) {
                                                        p.current_stage = stage;
                                                    }
                                                }
                                                p.is_completed = resp.is_completed;
                                            }
                                        } else {
                                            es.set(resp.error.unwrap_or_else(|| "Water failed".into()));
                                        }
                                    }
                                    Err(e) => { es.set(e); }
                                }
                            });
                        };

                        let plants_signal_h = plants;
                        let error_signal_h = error_msg;
                        let pid_for_harvest = pid.clone();
                        let init_harvest = init_for_closures.clone();
                        let harvest_click = move |_| {
                            let mut ps = plants_signal_h;
                            let mut es = error_signal_h;
                            let plant_id = pid_for_harvest.clone();
                            let init = init_harvest.clone();
                            spawn(async move {
                                match harvest_plant_api(&plant_id, &init).await {
                                    Ok(()) => { ps.write().retain(|p| p.id != plant_id); }
                                    Err(e) => { es.set(format!("Не удалось собрать урожай: {}", e)); }
                                }
                            });
                        };

                        rsx! {
                            div {
                                key: "{pid}",
                                style: "background: {bg_card}; border-radius: 12px; margin-bottom: 16px; border: {border}; overflow: hidden; {shadow}",

                                // ── Hero image ──────────────────────────
                                div { style: "position: relative; width: 100%; aspect-ratio: 1/1; overflow: hidden; background: #111;",
                                    img {
                                        src: "{img_url}",
                                        alt: "Растение",
                                        style: "width: 100%; height: 100%; object-fit: cover;",
                                    }
                                    // Target product thumbnail (top-left): what
                                    // discount this bush is growing.
                                    if let Some(timg) = product_thumb.clone() {
                                        div { style: "position:absolute;top:12px;left:12px;display:flex;align-items:center;gap:6px;background:rgba(0,0,0,0.65);padding:4px 8px;border-radius:8px;z-index:2;",
                                            img { src: "{timg}", alt: "Товар", style: "width:34px;height:34px;object-fit:cover;border-radius:6px;border:1px solid #39ff14;" }
                                            span { style: "font-size:10px;color:#39ff14;font-weight:700;", "🎯 скидка" }
                                        }
                                    }
                                    // Gradient overlay at bottom
                                    div { style: "position: absolute; bottom: 0; left: 0; right: 0; height: 50%; background: linear-gradient(transparent, rgba(15,15,26,0.95));" }

                                    // Plant name overlay
                                    div { style: "position: absolute; bottom: 12px; left: 16px; right: 16px;",
                                        div { style: "font-size: 16px; font-weight: 700; color: #fff; text-shadow: 0 1px 4px rgba(0,0,0,0.8);", "{sname}" }
                                        div { style: "display: flex; align-items: center; gap: 6px; margin-top: 4px;",
                                            span { style: "font-size: 14px;", "{emoji}" }
                                            span { style: "font-size: 12px; color: {color}; font-weight: 600;", "{stage_name}" }
                                        }
                                    }

                                    // Ready badge
                                    if is_ready {
                                        div { style: "position: absolute; top: 12px; right: 12px; font-size: 10px; padding: 4px 10px; border-radius: 12px; background: rgba(255,215,0,0.9); color: #000; font-weight: 700;",
                                            "🏆 READY"
                                        }
                                    }
                                }

                                // ── Progress + controls ─────────────────
                                div { style: "padding: 12px 16px 16px;",

                                    // Water count + progress bar
                                    div { style: "margin-bottom: 10px;",
                                        div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;",
                                            span { style: "font-size: 11px; color: #8b8b9e;",
                                                "💧 {wc}/{total_stages}"
                                            }
                                            span { style: "font-size: 11px; color: #8b8b9e;",
                                                "{pct_str}"
                                            }
                                        }
                                        div { style: "height: 6px; background: rgba(255,255,255,0.08); border-radius: 6px; overflow: hidden;",
                                            div { style: "height: 100%; width: {pct_str}; border-radius: 6px; background: {gradient}; transition: width 0.3s ease;" }
                                        }
                                    }

                                    // Action buttons
                                    div { style: "display: flex; gap: 8px;",
                                        if is_ready {
                                            button {
                                                style: "
                                                    flex: 1; padding: 10px; border: none; border-radius: 8px;
                                                    font-family: 'Inter', sans-serif; font-size: 14px;
                                                    font-weight: 700; cursor: pointer; color: #0a0a0a;
                                                    background: linear-gradient(135deg, #ffd700, #ff9500);
                                                ",
                                                onclick: harvest_click,
                                                "🏆 HARVEST"
                                            }
                                        } else if can_w {
                                            button {
                                                style: "
                                                    flex: 1; padding: 10px; border: none; border-radius: 8px;
                                                    font-family: 'Inter', sans-serif; font-size: 14px;
                                                    font-weight: 700; cursor: pointer; color: #0a0a0a;
                                                    background: linear-gradient(135deg, #39ff14, #22c55e);
                                                ",
                                                onclick: water_click,
                                                "💧 {wt}"
                                            }
                                        } else {
                                            div { style: "
                                                flex: 1; padding: 10px; border-radius: 8px; text-align: center;
                                                font-size: 13px; color: #555;
                                                background: rgba(255,255,255,0.03);
                                            ",
                                                "⏳ Cooldown..."
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    })}
                }
            }

            if show_chooser() {
                GardenChooser { telegram_id, init_data: init_data.clone(), plants, open: show_chooser }
            }
        }
    }
}

/// Modal: pick a live, garden-eligible product to grow a discount for. On
/// success the chosen product's photo becomes the plant's seed image.
#[component]
fn GardenChooser(
    telegram_id: i64,
    init_data: String,
    plants: Signal<Vec<ApiPlant>>,
    open: Signal<bool>,
) -> Element {
    let products = use_resource(|| async move { fetch_garden_products().await });
    let mut busy = use_signal(|| false);
    let mut err = use_signal(String::new);

    let cat_label = |c: &str| match c {
        "strain" => "🌿 Сорта",
        "accessory" => "💨 Аксессуары",
        "tea" => "🥤 Напитки",
        "set" => "📦 Наборы",
        "accessory_set" => "🔧 Сеты аксессуаров",
        "tea_set" => "🫖 Сеты напитков",
        _ => "Прочее",
    };

    rsx! {
        div {
            style: "position:fixed;inset:0;z-index:1000;background:rgba(0,0,0,0.75);display:flex;align-items:flex-end;justify-content:center;",
            onclick: move |_| open.set(false),
            div {
                style: "background:#0f0f1a;width:100%;max-width:520px;max-height:85vh;overflow:auto;border-top:4px solid #39ff14;padding:16px;",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),
                div { style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:12px;",
                    div { style: "font-size:15px;font-weight:700;color:#39ff14;", "Выбери товар для скидки" }
                    button { style: "width:44px;height:44px;background:transparent;border:none;color:#8b8b9e;font-size:20px;cursor:pointer;",
                        "aria-label": "Закрыть", onclick: move |_| open.set(false), "✕" }
                }
                if !err.read().is_empty() {
                    div { style: "background:#2a1a1a;color:#ff6b7a;font-size:13px;padding:8px;margin-bottom:10px;border:1px solid #ff4757;", "{err}" }
                }
                {
                    match &*products.read() {
                        Some(Ok(list)) if !list.is_empty() => {
                            let items = list.clone();
                            rsx! {
                                div { style: "display:grid;grid-template-columns:1fr 1fr;gap:10px;",
                                    for p in items.into_iter() {
                                        {
                                            let img = p.image_url.clone().unwrap_or_default();
                                            let has_img = img.starts_with("http") || img.starts_with('/');
                                            let cat = p.catalog.clone();
                                            let pid = p.id.clone();
                                            let name = p.name.clone();
                                            let badge = cat_label(&p.catalog);
                                            let price = if p.price.is_finite() { p.price.max(0.0) } else { 0.0 };
                                            let init = init_data.clone();
                                            rsx! {
                                                div {
                                                    style: "background:#16213e;border:3px solid #2a2a4a;box-shadow:3px 3px 0 #000;overflow:hidden;cursor:pointer;",
                                                    onclick: move |_| {
                                                        if *busy.read() { return; }
                                                        busy.set(true);
                                                        err.set(String::new());
                                                        let cat = cat.clone(); let pid = pid.clone(); let init = init.clone();
                                                        spawn(async move {
                                                            match choose_plant_api(telegram_id, &cat, &pid, &init).await {
                                                                Ok(()) => {
                                                                    if let Ok(p2) = fetch_plants(telegram_id, &init).await {
                                                                        plants.set(p2);
                                                                    }
                                                                    open.set(false);
                                                                }
                                                                Err(code) => {
                                                                    let msg = match code.as_str() {
                                                                        "harvest_cooldown" => "Скидку можно растить раз в сутки — подожди после прошлого сбора.",
                                                                        "product_not_available" => "Товар недоступен.",
                                                                        other => other,
                                                                    };
                                                                    err.set(msg.to_string());
                                                                    busy.set(false);
                                                                }
                                                            }
                                                        });
                                                    },
                                                    div { style: "min-height:90px;background:#111;display:flex;align-items:center;justify-content:center;font-size:32px;",
                                                        if has_img {
                                                            img { src: "{img}", alt: "{name}", style: "width:100%;height:auto;object-fit:contain;display:block;" }
                                                        } else { "🎁" }
                                                    }
                                                    div { style: "padding:8px;",
                                                        div { style: "font-size:10px;color:#8b8b9e;", "{badge}" }
                                                        div { style: "font-size:13px;font-weight:700;color:#e8e8e8;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;", "{name}" }
                                                        div { style: "font-size:12px;color:#ffe600;", "{price} ฿" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Some(Ok(_)) => rsx! { div { style: "text-align:center;padding:30px;color:#8b8b9e;", "Нет доступных товаров" } },
                        Some(Err(e)) => rsx! { div { style: "text-align:center;padding:30px;color:#ff6b7a;", "Ошибка: {e}" } },
                        None => rsx! { div { style: "text-align:center;padding:30px;color:#8b8b9e;", "Загрузка..." } },
                    }
                }
            }
        }
    }
}
