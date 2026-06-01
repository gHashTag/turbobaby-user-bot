use crate::ui::api::context::api_base_url;
use crate::ui::components::{ErrorBanner, Skeleton, SkeletonShape};
use crate::ui::routes::Route;
use dioxus::prelude::*;
use serde::Deserialize;

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
    let nav = navigator();
    let mut places = use_signal(Vec::<QuestPlace>::new);
    let mut loading = use_signal(|| true);
    let mut err_msg = use_signal(|| String::new());

    let _ = use_resource(move || async move {
        // Cycle #31: Result-propagated errors so a network drop produces
        // a visible "API недоступен" banner instead of a tear-jerking
        // "no location quests yet" empty state.
        let url = format!("{}/api/quest-places", api_base_url());
        let result: Result<Vec<QuestPlace>, String> = async {
            let text = crate::ui::api::http::fetch_text(&url)
                .await
                .map_err(|e| format!("Network: {e}"))?;
            let val: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| format!("Parse: {e}"))?;
            let arr = val
                .get("quest_places")
                .and_then(|v| v.as_array())
                .ok_or_else(|| "Server returned no `quest_places` array".to_string())?;
            Ok(arr
                .iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect())
        }
        .await;
        match result {
            Ok(items) => {
                places.set(items);
                err_msg.set(String::new());
            }
            Err(e) => {
                err_msg.set(format!("Не удалось загрузить квесты: {e}"));
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

            ErrorBanner { message: err_msg.read().clone(), margin: "0 0 12px".to_string() }

            if *loading.read() {
                div { style: "display: flex; flex-direction: column; gap: 10px;",
                    for _ in 0..4 {
                        div { style: "
                            background: #16213e; border: 4px solid #2a2a4a;
                            box-shadow: 4px 4px 0 #000; padding: 12px;
                            display: flex; align-items: center; gap: 12px;
                        ",
                            Skeleton { shape: SkeletonShape::Avatar }
                            div { style: "flex:1; display:flex; flex-direction:column; gap:6px;",
                                Skeleton { shape: SkeletonShape::Text, width: Some("70%".into()) }
                                Skeleton { shape: SkeletonShape::TextSm, width: Some("50%".into()) }
                            }
                        }
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
                        // cycle #34: "Go" button gets onclick → navigate to Quest
                        // screen with this place id, which hosts the scanner.
                        let place_id = p.id.clone();
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
                                        onclick: move |_| { nav.push(Route::Quest { id: place_id.clone() }); },
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
