use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::api::context::api_base_url;

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct QuestPlace {
    id: String,
    name: String,
    category: String,
    lat: f64,
    lon: f64,
    description: Option<String>,
    image_url: Option<String>,
}

fn category_icon(cat: &str) -> &'static str {
    match cat.to_lowercase().as_str() {
        "beach" => "\u{1F33F}",
        "viewpoint" => "\u{1F304}",
        "temple" => "\u{26E9}\u{FE0F}",
        "bar" => "\u{1F37A}",
        "nature" => "\u{1F333}",
        "market" => "\u{1F6CD}\u{FE0F}",
        _ => "\u{1F4CD}",
    }
}

fn category_accent(cat: &str) -> &'static str {
    match cat.to_lowercase().as_str() {
        "beach" => "#00bcd4",
        "viewpoint" => "#ff9800",
        "temple" => "#ff5722",
        "bar" => "#9c27b0",
        "nature" => "#4caf50",
        "market" => "#ff6f00",
        _ => "#4caf50",
    }
}

#[component]
pub fn LocationQuestScreen() -> Element {
    let places = use_signal(Vec::<QuestPlace>::new);
    let loading = use_signal(|| true);

    let _ = use_resource(move || async move {
        let base = api_base_url();
        let client = reqwest::Client::new();
        if let Ok(resp) = client.get(format!("{}/api/quest-places", base)).send().await {
            if let Ok(text) = resp.text().await {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(arr) = val.get("quest_places").and_then(|v| v.as_array()) {
                        let items: Vec<QuestPlace> = arr.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect();
                        places.set(items);
                    }
                }
            }
        }
        loading.set(false);
        Some(())
    });

    let total = places.read().len();

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding: 20px 16px;
            padding-bottom: 80px;
        ",
            div { style: "padding: 20px 16px 16px; text-align: center; margin-bottom: 20px;",
                h1 { style: "font-size: 24px; font-weight: 800; color: #4caf50; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(76,175,80,0.5); letter-spacing: 2px;",
                    "\u{1F4CD} Location Quests"
                }
                p { style: "font-size: 13px; color: #8b8b9e; margin-top: 6px;",
                    "Complete quests at real locations around the island"
                }
            }

            if *loading.read() {
                div { style: "text-align: center; padding: 40px;",
                    div { style: "font-size: 13px; color: #4caf50;",
                        "Loading quests..."
                    }
                }
            } else if total == 0 {
                div { style: "
                    background: #16213e; border: 4px solid #2a2a4a; border-radius: 0;
                    padding: 24px; text-align: center; box-shadow: 4px 4px 0 #000;
                ",
                    div { style: "font-size: 70px; margin-bottom: 12px;", "\u{1F5FA}\u{FE0F}" }
                    div { style: "font-size: 15px; color: #4caf50; margin-bottom: 6px;",
                        "No location quests yet"
                    }
                    div { style: "font-size: 13px; color: #8b8b9e;",
                        "New quests will appear here when available"
                    }
                }
            } else {
                div { style: "
                    background: #16213e; border: 4px solid #2a2a4a; border-radius: 0;
                    padding: 12px; margin-bottom: 16px; box-shadow: 4px 4px 0 #000;
                ",
                    div { style: "display: flex; justify-content: space-between; margin-bottom: 8px;",
                        span { style: "font-size: 13px; color: #4caf50;", "\u{1F3AF} Locations" }
                        span { style: "font-size: 13px; color: #39ff14;", "{total} places" }
                    }
                    div { style: "background: #0f0f1a; border-radius: 0; height: 8px; overflow: hidden;",
                        div { style: "background: linear-gradient(90deg, #4caf50, #8bc34a); height: 100%; width: 100%; border-radius: 0;" }
                    }
                }

                div { style: "display: flex; flex-direction: column; gap: 10px;",
                    {places.read().iter().map(|p| {
                        let icon = category_icon(&p.category);
                        let accent = category_accent(&p.category);
                        let desc = p.description.as_deref().unwrap_or("Explore this location").to_string();
                        let cat = p.category.clone();
                        rsx! {
                            div { key: "{p.id}", style: "
                                background: #16213e; border: 4px solid {accent}33;
                                border-radius: 0; padding: 12px; box-shadow: 4px 4px 0 #000;
                            ",
                                div { style: "display: flex; align-items: center; gap: 10px;",
                                    span { style: "font-size: 28px;", "{icon}" }
                                    div { style: "flex: 1;",
                                        div { style: "font-size: 17px; font-weight: 700; color: #fff; margin-bottom: 4px;",
                                            "{p.name}"
                                        }
                                        div { style: "font-size: 13px; color: #c9c9d4; margin-bottom: 4px;",
                                            "{desc}"
                                        }
                                        div { style: "display: flex; gap: 8px; align-items: center;",
                                            span { style: "font-size: 13px; color: {accent}; text-transform: uppercase;",
                                                "{cat}"
                                            }
                                            span { style: "font-size: 13px; color: #8b8b9e;",
                                                "\u{1F4CD} {p.lat:.2}, {p.lon:.2}"
                                            }
                                        }
                                    }
                                    button { style: "
                                        font-size: 14px; font-weight: 700; padding: 8px 12px;
                                        background: {accent}22; color: {accent};
                                        border: 4px solid {accent}; border-radius: 0; cursor: pointer;
                                        box-shadow: 3px 3px 0 #000;
                                    ",
                                        "\u{1F4F7} Go"
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
