use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::api::context::api_base_url;

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct TechNode {
    id: String,
    name: String,
    description: String,
    category: String,
    icon: String,
    status: String,
    xp_required: i64,
    xp_reward: i64,
    #[allow(dead_code)]
    dependencies: Vec<String>,
    #[allow(dead_code)]
    unlocks: Vec<String>,
    features: Vec<String>,
    #[allow(dead_code)]
    estimated_hours: i64,
    #[allow(dead_code)]
    priority: i64,
}

fn category_color(cat: &str) -> &'static str {
    match cat {
        "core" => "#39ff14",
        "wasm" => "#00e5ff",
        "ai" => "#b388ff",
        "commerce" => "#ffd700",
        "analytics" => "#ff9800",
        "social" => "#e040fb",
        "experience" => "#00bcd4",
        "future" => "#ff4757",
        _ => "#8b8b9e",
    }
}

#[component]
pub fn TechTreeScreen() -> Element {
    let nodes = use_signal(Vec::<TechNode>::new);
    let loading = use_signal(|| true);

    let _ = use_resource(move || async move {
        let base = api_base_url();
        let client = reqwest::Client::new();
        if let Ok(resp) = client.get(format!("{}/api/tech-tree/nodes", base)).send().await {
            if let Ok(text) = resp.text().await {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(arr) = val.get("nodes").and_then(|v| v.as_array()) {
                        let items: Vec<TechNode> = arr.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect();
                        nodes.set(items);
                    }
                }
            }
        }
        loading.set(false);
        Some(())
    });

    let total = nodes.read().len();
    let completed = nodes.read().iter().filter(|n| n.status == "completed").count();
    let available = nodes.read().iter().filter(|n| n.status == "available").count();
    let progress_pct = if total > 0 { (completed * 100) / total } else { 0 };

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding: 20px 16px;
            padding-bottom: 80px;
        ",
            div { style: "padding: 20px 16px 16px; text-align: center; margin-bottom: 24px;",
                h1 { style: "font-size: 24px; font-weight: 800; color: #00e5ff; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(0,229,255,0.5); letter-spacing: 2px;",
                    "\u{1F333} Tech Tree"
                }
                p { style: "font-size: 13px; color: #8b8b9e; margin-top: 6px;",
                    "Unlock growing skills to improve your garden"
                }
            }

            div { style: "
                background: #16213e; border: 4px solid #ffe600; border-radius: 0;
                padding: 12px; margin-bottom: 16px; text-align: center;
                box-shadow: 4px 4px 0 #000;
            ",
                span { style: "font-size: 13px; color: #ffe600;",
                    "\u{2B50} {completed}/{total} Completed \u{2022} {available} Available"
                }
            }

            div { style: "
                background: #16213e; border: 4px solid #2a2a4a; border-radius: 0;
                padding: 12px; margin-bottom: 16px; box-shadow: 4px 4px 0 #000;
            ",
                div { style: "background: #0f0f1a; border-radius: 0; height: 8px; overflow: hidden;",
                    div { style: "background: linear-gradient(90deg, #00e5ff, #39ff14); height: 100%; width: {progress_pct}%; border-radius: 0;" }
                }
            }

            if *loading.read() {
                div { style: "text-align: center; padding: 40px;",
                    div { style: "font-size: 13px; color: #00e5ff;", "Loading tech tree..." }
                }
            } else {
                div { style: "display: flex; flex-direction: column; gap: 10px;",
                    {nodes.read().iter().map(|n| {
                        let is_completed = n.status == "completed";
                        let is_available = n.status == "available";
                        let accent = category_color(&n.category);
                        let border = if is_completed { "#39ff14" } else if is_available { accent } else { "#2a2a4a" };
                        let bg = if is_completed { "rgba(57,255,20,0.05)" } else if is_available { "rgba(0,229,255,0.03)" } else { "#16213e" };
                        let status_text = if is_completed { "\u{2705} Done" } else if is_available { "\u{1F7E2} Available" } else { "\u{1F512} Locked" };
                        let status_color = if is_completed { "#39ff14" } else if is_available { accent } else { "#8b8b9e" };
                        let features_str = n.features.join(", ");

                        rsx! {
                            div { key: "{n.id}", style: "
                                background: {bg}; border: 4px solid {border};
                                border-radius: 0; padding: 12px;
                                display: flex; align-items: center; gap: 12px;
                                box-shadow: 4px 4px 0 #000;
                            ",
                                div { style: "
                                    width: 40px; height: 40px; border-radius: 0;
                                    background: {border}22; border: 4px solid {border};
                                    display: flex; align-items: center; justify-content: center;
                                    font-size: 14px; flex-shrink: 0;
                                ",
                                    "{n.icon}"
                                }
                                div { style: "flex: 1;",
                                    div { style: "font-size: 15px; font-weight: 700; color: #fff; margin-bottom: 3px;",
                                        "{n.name}"
                                    }
                                    if !n.description.is_empty() {
                                        div { style: "font-size: 13px; color: #c9c9d4; margin-bottom: 4px;",
                                            "{n.description}"
                                        }
                                    }
                                    div { style: "display: flex; gap: 8px; align-items: center; flex-wrap: wrap;",
                                        span { style: "font-size: 13px; color: #ffe600;",
                                            "\u{2B50} {n.xp_required}xp"
                                        }
                                        span { style: "font-size: 13px; color: {status_color};",
                                            "{status_text}"
                                        }
                                        if !features_str.is_empty() {
                                            span { style: "font-size: 13px; color: #555;",
                                                "{features_str}"
                                            }
                                        }
                                    }
                                }
                                if is_available {
                                    button { style: "
                                        font-size: 14px; font-weight: 700; padding: 5px 8px;
                                        background: {accent}22; color: {accent};
                                        border: 4px solid {accent}; border-radius: 0; cursor: pointer;
                                        box-shadow: 3px 3px 0 #000;
                                    ",
                                        "Unlock"
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
