use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use crate::trios::garden::{GrowthStage, Plant, calculate_progress};
use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_GARDEN_TITLE, T_GARDEN_SUBTITLE, T_BTN_WATER};
use crate::ui::api::context::api_base_url;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};

#[derive(Debug, Clone, serde::Deserialize)]
struct ApiPlant {
    id: String,
    strain_id: String,
    strain_name: String,
    current_stage: GrowthStage,
    water_count: u32,
    is_completed: bool,
    planted_at: i64,
    last_watered_at: Option<i64>,
    reward_claimed: bool,
}

impl From<ApiPlant> for Plant {
    fn from(a: ApiPlant) -> Self {
        Plant {
            id: a.id,
            user_id: String::new(),
            strain_id: a.strain_id,
            strain_name: a.strain_name,
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

async fn fetch_plants(telegram_id: i64, init_data: &str) -> Result<Vec<Plant>, String> {
    let base = api_base_url();
    reqwest::Client::new()
        .get(format!("{}/api/garden/plants?telegram_id={}", base, telegram_id))
        .header("X-Telegram-Init-Data", init_data)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<GardenResponse>()
        .await
        .map_err(|e| e.to_string())
        .map(|r| r.plants.into_iter().map(Into::into).collect())
}

async fn water_plant_api(plant_id: &str, init_data: &str) -> Result<WaterPlantResponse, String> {
    let base = api_base_url();
    reqwest::Client::new()
        .post(format!("{}/api/garden/plants/{}/water", base, plant_id))
        .header("X-Telegram-Init-Data", init_data)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<WaterPlantResponse>()
        .await
        .map_err(|e| e.to_string())
}

async fn harvest_plant_api(plant_id: &str, init_data: &str) -> Result<(), String> {
    let base = api_base_url();
    let resp: HarvestPlantResponse = reqwest::Client::new()
        .post(format!("{}/api/garden/plants/{}/harvest", base, plant_id))
        .header("X-Telegram-Init-Data", init_data)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    if resp.success {
        Ok(())
    } else {
        Err(resp.error.unwrap_or_else(|| "Harvest failed".into()))
    }
}

fn mock_plants() -> Vec<Plant> {
    let now = chrono::Utc::now().timestamp_millis();
    vec![
        Plant {
            id: "mock-1".into(),
            user_id: "user".into(),
            strain_id: "banana-fritter".into(),
            strain_name: "Banana Fritter".into(),
            current_stage: GrowthStage::BigVeg,
            planted_at: now - 86400000 * 5,
            is_completed: false,
            harvested_at: None,
            reward_claimed: false,
            water_count: 5,
            last_watered_at: Some(now - 120000),
        },
        Plant {
            id: "mock-2".into(),
            user_id: "user".into(),
            strain_id: "super-lemon-haze".into(),
            strain_name: "Super Lemon Haze".into(),
            current_stage: GrowthStage::Seed,
            planted_at: now - 60000,
            is_completed: false,
            harvested_at: None,
            reward_claimed: false,
            water_count: 0,
            last_watered_at: None,
        },
    ]
}

/// Returns the plant growth stage image URL based on stage index (0-13).
/// Images are at /assets/images/game/{1..14}.png
fn stage_image_url(stage_index: usize) -> String {
    let n = (stage_index + 1).clamp(1, 14);
    format!("/assets/images/game/{}.png", n)
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
        GrowthStage::YoungBush | GrowthStage::VegStart | GrowthStage::BigVeg => "linear-gradient(90deg, #39ff14, #00e5ff)",
        GrowthStage::PreFlower | GrowthStage::SmallBuds => "linear-gradient(90deg, #00e5ff, #a855f7)",
        GrowthStage::BigBuds | GrowthStage::Trimming => "linear-gradient(90deg, #a855f7, #ff6b35)",
        GrowthStage::Curing | GrowthStage::Lab => "linear-gradient(90deg, #ff6b35, #a78bfa)",
        GrowthStage::Delivery | GrowthStage::Final => "linear-gradient(90deg, #a78bfa, #ffd700)",
    }
}

#[component]
pub fn Garden() -> Element {
    let plants = use_signal(Vec::<Plant>::new);
    let loading = use_signal(|| true);
    let error_msg = use_signal(|| String::new());
    let now_ms = use_signal(|| chrono::Utc::now().timestamp_millis());
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_telegram_init_data();

    {
        let mut plants_c = plants.clone();
        let mut loading_c = loading.clone();
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
                match fetch_plants(tid, &value).await {
                    Ok(p) => { plants_c.set(p); }
                    Err(_) => { plants_c.set(mock_plants()); }
                }
                loading_c.set(false);
            }
        });
    }

    {
        let mut now_c = now_ms.clone();
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
    let title_text = t(Lang::Russian, T_GARDEN_TITLE);
    let subtitle_text = t(Lang::Russian, T_GARDEN_SUBTITLE);
    let water_text = t(Lang::Russian, T_BTN_WATER);

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

            if !err.is_empty() {
                div { style: "max-width: 400px; margin: 0 auto 12px; padding: 6px 10px; background: rgba(255,71,87,0.1); border: 2px solid #ff4757; border-radius: 8px; font-size: 18px; color: #ff4757;",
                    "{err}"
                }
            }

            if is_loading {
                div { style: "text-align: center; padding: 60px 20px;",
                    div { style: "font-size: 36px; margin-bottom: 12px;", "🌱" }
                    p { style: "font-size: 12px; color: #8b8b9e;", "Loading your garden..." }
                }
            } else if plant_list.is_empty() {
                div { style: "text-align: center; padding: 60px 20px;",
                    div { style: "font-size: 48px; margin-bottom: 16px;", "🌱" }
                    p { style: "font-size: 11px; color: #8b8b9e; margin-bottom: 8px;", "No plants yet" }
                    p { style: "font-size: 18px; color: #555577;", "Order a strain to get your first seed!" }
                }
            } else {
                div { style: "max-width: 400px; margin: 0 auto; padding: 0 16px;",
                    {plant_list.into_iter().map(|plant| {
                        let progress = calculate_progress(&plant, now);
                        let pid = plant.id.clone();
                        let sname = plant.strain_name.clone();
                        let wc = plant.water_count;
                        let total_stages = GrowthStage::TOTAL_STAGES;
                        let color = stage_color(&progress.stage).to_string();
                        let gradient = progress_bar_gradient(&progress.stage).to_string();
                        let stage_name = progress.stage_name.clone();
                        let emoji = progress.stage_emoji.clone();
                        let img_url = stage_image_url(progress.stage_index);
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

                        let plants_signal = plants.clone();
                        let error_signal = error_msg.clone();
                        let pid_for_water = pid.clone();
                        let init_water = init_for_closures.clone();
                        let water_click = move |_| {
                            let mut ps = plants_signal.clone();
                            let mut es = error_signal.clone();
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
                                                    if let Ok(stage) = serde_json::from_str::<GrowthStage>(&format!("\"{}\"", stage_str)) {
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

                        let plants_signal_h = plants.clone();
                        let pid_for_harvest = pid.clone();
                        let init_harvest = init_for_closures.clone();
                        let harvest_click = move |_| {
                            let mut ps = plants_signal_h.clone();
                            let plant_id = pid_for_harvest.clone();
                            let init = init_harvest.clone();
                            spawn(async move {
                                if let Ok(()) = harvest_plant_api(&plant_id, &init).await {
                                    ps.write().retain(|p| p.id != plant_id);
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
                                        style: "width: 100%; height: 100%; object-fit: cover;",
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
        }
    }
}
