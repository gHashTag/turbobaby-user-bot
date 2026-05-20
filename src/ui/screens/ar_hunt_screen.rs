use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::api::context::api_base_url;

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct Place {
    id: String,
    name: String,
    category: String,
    lat: f64,
    lon: f64,
    description: Option<String>,
    image_url: Option<String>,
}

fn category_emoji(cat: &str) -> &'static str {
    match cat.to_lowercase().as_str() {
        "beach" => "\u{1F3D6}\u{FE0F}",
        "viewpoint" => "\u{1F304}",
        "temple" => "\u{26E9}\u{FE0F}",
        "bar" => "\u{1F37A}",
        "nature" => "\u{1F333}",
        "market" => "\u{1F6CD}\u{FE0F}",
        "restaurant" => "\u{1F37D}\u{FE0F}",
        "hotel" => "\u{1F3E8}",
        _ => "\u{1F4CD}",
    }
}

fn category_color(cat: &str) -> &'static str {
    match cat.to_lowercase().as_str() {
        "beach" => "#00bcd4",
        "viewpoint" => "#ff9800",
        "temple" => "#ff5722",
        "bar" => "#9c27b0",
        "nature" => "#4caf50",
        "market" => "#ff6f00",
        _ => "#00e5ff",
    }
}

#[component]
pub fn ARHuntScreen() -> Element {
    let places = use_signal(Vec::<Place>::new);
    let loading = use_signal(|| true);

    let _ = use_resource(move || async move {
        let base = api_base_url();
        let client = reqwest::Client::new();
        if let Ok(resp) = client.get(format!("{}/api/quest-places", base)).send().await {
            if let Ok(text) = resp.text().await {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(arr) = val.get("quest_places").and_then(|v| v.as_array()) {
                        let items: Vec<Place> = arr.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect();
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
                h1 { style: "font-size: 24px; font-weight: 800; color: #00bcd4; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(0,188,212,0.5); letter-spacing: 2px;",
                    "\u{1F30D} AR Hunt"
                }
                p { style: "font-size: 13px; color: #8b8b9e; margin-top: 6px;",
                    "Explore the island, find AR marks, collect rewards!"
                }
            }

            div { style: "
                background: #16213e; border: 4px solid #00bcd4; border-radius: 0;
                padding: 10px; margin-bottom: 12px; text-align: center;
                box-shadow: 4px 4px 0 #000;
            ",
                span { style: "font-size: 13px; color: #00bcd4;",
                    "\u{1F4E1} GPS Active \u{2022} {total} marks nearby"
                }
            }

            if *loading.read() {
                div { style: "text-align: center; padding: 40px;",
                    div { style: "font-size: 13px; color: #00bcd4;",
                        "Loading AR marks..."
                    }
                }
            } else if total == 0 {
                div { style: "
                    background: #16213e; border: 4px solid #2a2a4a; border-radius: 0;
                    padding: 24px; text-align: center; box-shadow: 4px 4px 0 #000;
                ",
                    div { style: "font-size: 70px; margin-bottom: 12px;", "\u{1F30D}" }
                    div { style: "font-size: 15px; color: #00bcd4; margin-bottom: 6px;",
                        "No AR marks available yet"
                    }
                    div { style: "font-size: 13px; color: #8b8b9e;",
                        "New marks appear as you explore the island"
                    }
                }
            } else {
                div { style: "
                    background: rgba(57,255,20,0.05); border: 4px solid #39ff1444;
                    border-radius: 0; padding: 8px; margin-bottom: 12px; text-align: center;
                    box-shadow: 4px 4px 0 #000;
                ",
                    span { style: "font-size: 13px; color: #39ff14;",
                        "\u{26A1} Approach a mark and tap Collect"
                    }
                }

                div { style: "display: flex; flex-direction: column; gap: 8px;",
                    {places.read().iter().map(|p| {
                        let emoji = category_emoji(&p.category);
                        let color = category_color(&p.category);
                        let cat = p.category.clone();
                        let desc = p.description.as_deref().unwrap_or("").to_string();
                        rsx! {
                            div { key: "{p.id}", style: "
                                background: #16213e; border: 4px solid {color};
                                border-radius: 0; padding: 10px;
                                display: flex; align-items: center; gap: 10px;
                                box-shadow: 4px 4px 0 #000;
                            ",
                                div { style: "
                                    width: 40px; height: 40px; border-radius: 0;
                                    background: {color}22; border: 4px solid {color};
                                    display: flex; align-items: center; justify-content: center;
                                    font-size: 14px; flex-shrink: 0;
                                ",
                                    "{emoji}"
                                }
                                div { style: "flex: 1;",
                                    div { style: "font-size: 17px; font-weight: 700; color: #ffffff; margin-bottom: 3px;",
                                        "{p.name}"
                                    }
                                    div { style: "display: flex; gap: 8px; align-items: center;",
                                        span { style: "font-size: 13px; color: {color}; text-transform: uppercase;",
                                            "{cat}"
                                        }
                                        if !desc.is_empty() {
                                            span { style: "font-size: 13px; color: #8b8b9e;",
                                                "{desc}"
                                            }
                                        }
                                    }
                                }
                                button { style: "
                                    font-size: 14px; font-weight: 700; padding: 5px 8px;
                                    background: #16213e; color: {color};
                                    border: 4px solid {color}; border-radius: 0; cursor: pointer;
                                    box-shadow: 3px 3px 0 #000;
                                ",
                                    "\u{1F3AF} Go"
                                }
                            }
                        }
                    })}
                }
            }
        }
    }
}
