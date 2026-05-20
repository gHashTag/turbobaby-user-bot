use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::api::context::api_base_url;

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct Hunt {
    id: String,
    name: String,
    #[allow(dead_code)]
    description: Option<String>,
    #[allow(dead_code)]
    image_url: Option<String>,
    is_active: bool,
    start_name: String,
}

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

#[component]
pub fn TreasureHuntScreen() -> Element {
    let mut hunts = use_signal(Vec::<Hunt>::new);
    let mut places = use_signal(Vec::<Place>::new);
    let mut loading = use_signal(|| true);

    let _ = use_resource(move || async move {
        let base = api_base_url();
        let client = reqwest::Client::new();
        if let Ok(resp) = client.get(format!("{}/api/treasure-hunts", base)).send().await {
            if let Ok(text) = resp.text().await {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(arr) = val.get("treasure_hunts").and_then(|v| v.as_array()) {
                        let items: Vec<Hunt> = arr.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect();
                        hunts.set(items);
                    }
                }
            }
        }
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

    let hunt_count = hunts.read().len();
    let place_count = places.read().len();

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding: 20px 16px;
            padding-bottom: 80px;
        ",
            div { style: "padding: 20px 16px 16px; text-align: center; margin-bottom: 20px;",
                h1 { style: "font-size: 24px; font-weight: 800; color: #ffe600; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(255,230,0,0.5); letter-spacing: 2px;",
                    "\u{1F3F4}\u{200D}\u{2620}\u{FE0F} Treasure Hunt"
                }
                p { style: "font-size: 13px; color: #8b8b9e; margin-top: 6px;",
                    "Find checkpoints, scan QR codes, collect rewards!"
                }
            }

            if *loading.read() {
                div { style: "text-align: center; padding: 40px;",
                    div { style: "font-size: 13px; color: #ffe600;",
                        "Loading..."
                    }
                }
            } else if hunt_count == 0 && place_count == 0 {
                div { style: "
                    background: #16213e; border: 4px solid #2a2a4a; border-radius: 0;
                    padding: 24px; text-align: center; box-shadow: 4px 4px 0 #000;
                ",
                    div { style: "font-size: 70px; margin-bottom: 12px;", "\u{1F5FA}\u{FE0F}" }
                    div { style: "font-size: 15px; color: #ffe600; margin-bottom: 6px;",
                        "No hunts available yet"
                    }
                    div { style: "font-size: 13px; color: #8b8b9e;",
                        "Check back soon for new treasure hunts!"
                    }
                }
            } else {
                // Active Hunts
                if hunt_count > 0 {
                    div { style: "margin-bottom: 16px;",
                        div { style: "font-size: 13px; font-weight: 700; color: #ffe600; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 8px;",
                            "\u{1F3AF} Active Hunts ({hunt_count})"
                        }
                        {hunts.read().iter().map(|h| rsx! {
                            div { key: "{h.id}", style: "
                                background: #16213e; border: 4px solid #ffe600;
                                border-radius: 0; padding: 12px; margin-bottom: 8px;
                                box-shadow: 4px 4px 0 #000;
                            ",
                                div { style: "font-size: 17px; font-weight: 700; color: #fff; margin-bottom: 4px;",
                                    "{h.name}"
                                }
                                if !h.start_name.is_empty() {
                                    div { style: "font-size: 13px; color: #8b8b9e;",
                                        "\u{1F4CD} Start: {h.start_name}"
                                    }
                                }
                                button { style: "
                                    font-size: 14px; font-weight: 700; padding: 8px 16px; margin-top: 8px;
                                    background: #ffe600; color: #000;
                                    border: 4px solid #cca300; border-radius: 0; cursor: pointer;
                                    box-shadow: 3px 3px 0 #000;
                                ",
                                    "\u{1F6B6} Start Hunt"
                                }
                            }
                        })}
                    }
                }

                // Quest Places / Checkpoints
                if place_count > 0 {
                    div { style: "margin-bottom: 16px;",
                        div { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 8px;",
                            "\u{1F4CD} Quest Locations ({place_count})"
                        }
                        {places.read().iter().map(|p| rsx! {
                            div { key: "{p.id}", style: "
                                background: #16213e; border: 4px solid #2a2a4a;
                                border-radius: 0; padding: 10px; margin-bottom: 6px;
                                display: flex; align-items: center; gap: 10px;
                                box-shadow: 4px 4px 0 #000;
                            ",
                                div { style: "
                                    width: 36px; height: 36px; border-radius: 0;
                                    background: #00e5ff15; border: 4px solid #00e5ff44;
                                    display: flex; align-items: center; justify-content: center;
                                    font-size: 14px; flex-shrink: 0;
                                ",
                                    "\u{1F4CD}"
                                }
                                div { style: "flex: 1;",
                                    div { style: "font-size: 17px; font-weight: 700; color: #fff; margin-bottom: 3px;",
                                        "{p.name}"
                                    }
                                    div { style: "font-size: 13px; color: #00e5ff; text-transform: uppercase; margin-bottom: 2px;",
                                        "{p.category}"
                                    }
                                    if let Some(desc) = &p.description {
                                        div { style: "font-size: 13px; color: #8b8b9e;",
                                            "{desc}"
                                        }
                                    }
                                }
                                button { style: "
                                    font-size: 13px; font-weight: 700; padding: 5px 8px;
                                    background: #16213e; color: #ffe600;
                                    border: 4px solid #ffe600; border-radius: 0; cursor: pointer;
                                    box-shadow: 3px 3px 0 #000;
                                ",
                                    "\u{1F4F7} Scan"
                                }
                            }
                        })}
                    }
                }
            }
        }
    }
}
