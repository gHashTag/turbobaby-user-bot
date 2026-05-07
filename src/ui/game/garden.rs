use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use crate::trios::garden::{GrowthStage, Plant, calculate_progress};
use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_GARDEN_TITLE, T_GARDEN_SUBTITLE, T_BTN_WATER};
use crate::ui::api::context::api_base_url;

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

async fn fetch_plants() -> Result<Vec<Plant>, String> {
    let base = api_base_url();
    reqwest::get(format!("{}/api/garden/plants", base))
        .await
        .map_err(|e| e.to_string())?
        .json::<GardenResponse>()
        .await
        .map_err(|e| e.to_string())
        .map(|r| r.plants.into_iter().map(Into::into).collect())
}

async fn water_plant_api(plant_id: &str) -> Result<Plant, String> {
    let base = api_base_url();
    reqwest::Client::new()
        .post(format!("{}/api/garden/plants/{}/water", base, plant_id))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<ApiPlant>()
        .await
        .map_err(|e| e.to_string())
        .map(Into::into)
}

async fn harvest_plant_api(plant_id: &str) -> Result<Plant, String> {
    let base = api_base_url();
    reqwest::Client::new()
        .post(format!("{}/api/garden/plants/{}/harvest", base, plant_id))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<ApiPlant>()
        .await
        .map_err(|e| e.to_string())
        .map(Into::into)
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

    {
        let mut plants_c = plants.clone();
        let mut loading_c = loading.clone();
        use_future(move || async move {
            match fetch_plants().await {
                Ok(p) => { plants_c.set(p); }
                Err(_) => { plants_c.set(mock_plants()); }
            }
            loading_c.set(false);
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
                        let water_click = move |_| {
                            let mut ps = plants_signal.clone();
                            let mut es = error_signal.clone();
                            let plant_id = pid_for_water.clone();
                            spawn(async move {
                                match water_plant_api(&plant_id).await {
                                    Ok(updated) => {
                                        let mut list = ps.write();
                                        if let Some(p) = list.iter_mut().find(|p| p.id == plant_id) {
                                            *p = updated;
                                        }
                                    }
                                    Err(e) => { es.set(e); }
                                }
                            });
                        };

                        let plants_signal_h = plants.clone();
                        let pid_for_harvest = pid.clone();
                        let harvest_click = move |_| {
                            let mut ps = plants_signal_h.clone();
                            let plant_id = pid_for_harvest.clone();
                            spawn(async move {
                                let _ = harvest_plant_api(&plant_id).await;
                                ps.write().retain(|p| p.id != plant_id);
                            });
                        };

                        rsx! {
                            div {
                                key: "{pid}",
                                style: "background: {bg_card}; border-radius: 8px; padding: 16px; margin-bottom: 16px; border: {border}; {shadow}",

                                div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;",
                                    div { style: "font-size: 14px; color: #e0e0e0;", "{sname}" }
                                    if is_ready {
                                        span { style: "font-size: 10px; padding: 2px 8px; border-radius: 8px; background: rgba(255,215,0,0.2); color: #ffd700;",
                                            "READY TO HARVEST"
                                        }
                                    }
                                }

                                div { style: "display: flex; align-items: center; gap: 12px; margin-bottom: 12px;",
                                    div { style: "
                                        width: 48px; height: 48px; border-radius: 50%;
                                        display: flex; align-items: center; justify-content: center;
                                        font-size: 24px; flex-shrink: 0;
                                        background: {color}15; border: 2px solid {color};
                                    ", "{emoji}" }
                                    div { style: "flex: 1;",
                                        div { style: "font-size: 14px; color: {color}; margin-bottom: 2px;", "{stage_name}" }
                                        div { style: "font-size: 18px; color: #666; margin-bottom: 6px;",
                                            "Water: {wc}/{total_stages}"
                                        }
                                        div { style: "height: 8px; background: rgba(0,0,0,0.4); border-radius: 8px; overflow: hidden;",
                                            div { style: "height: 100%; width: {pct_str}; border-radius: 8px; background: {gradient};" }
                                        }
                                        div { style: "font-size: 18px; color: #555; margin-top: 2px; text-align: right;", "{pct_str}" }
                                    }
                                }

                                div { style: "display: flex; gap: 8px;",
                                    if is_ready {
                                        button {
                                            style: "
                                                flex: 1; padding: 8px; border: none; border-radius: 8px;
                                                font-family: 'Press Start 2P', monospace; font-size: 12px;
                                                cursor: pointer; color: #0a0a0a;
                                                background: linear-gradient(135deg, #ffd700, #ff9500);
                                            ",
                                            onclick: harvest_click,
                                            "🏆 HARVEST"
                                        }
                                    } else if can_w {
                                        button {
                                            style: "
                                                flex: 1; padding: 8px; border: none; border-radius: 8px;
                                                font-family: 'Press Start 2P', monospace; font-size: 12px;
                                                cursor: pointer; color: #0a0a0a;
                                                background: linear-gradient(135deg, #39ff14, #22c55e);
                                            ",
                                            onclick: water_click,
                                            "💧 {wt}"
                                        }
                                    } else {
                                        div { style: "
                                            flex: 1; padding: 8px; border-radius: 8px; text-align: center;
                                            font-size: 18px; color: #555;
                                            background: rgba(255,255,255,0.03);
                                        ",
                                            "⏳ Cooldown..."
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
