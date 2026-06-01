use crate::ui::api::context::api_base_url;
use crate::ui::components::{ErrorBanner, Skeleton, SkeletonShape};
use crate::ui::routes::Route;
use dioxus::prelude::*;
use serde::Deserialize;

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
    let nav = navigator();
    // Parallel data loading — two independent endpoints fetched concurrently.
    // Cycle #29: errors now propagate as `Result<Vec<T>, String>` instead of
    // being swallowed via `.ok()?` → silent empty list. If both endpoints
    // fail, the user sees a clear red banner instead of "no hunts available".
    let hunts_resource = use_resource(move || async move {
        let url = format!("{}/api/treasure-hunts", api_base_url());
        let text = crate::ui::api::http::fetch_text(&url)
            .await
            .map_err(|e| format!("Hunts: {e}"))?;
        let val: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("Hunts parse: {e}"))?;
        let arr = val
            .get("treasure_hunts")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Hunts: server returned no `treasure_hunts` array".to_string())?;
        Ok::<Vec<Hunt>, String>(
            arr.iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect(),
        )
    });

    let places_resource = use_resource(move || async move {
        let url = format!("{}/api/quest-places", api_base_url());
        let text = crate::ui::api::http::fetch_text(&url)
            .await
            .map_err(|e| format!("Places: {e}"))?;
        let val: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("Places parse: {e}"))?;
        let arr = val
            .get("quest_places")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Places: server returned no `quest_places` array".to_string())?;
        Ok::<Vec<Place>, String>(
            arr.iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect(),
        )
    });

    let hunts = match &*hunts_resource.read() {
        Some(Ok(items)) => items.clone(),
        _ => Vec::new(),
    };
    let places = match &*places_resource.read() {
        Some(Ok(items)) => items.clone(),
        _ => Vec::new(),
    };
    let loading = hunts_resource.read().is_none() || places_resource.read().is_none();

    // Show error banner only if **both** endpoints failed — one transient
    // failure shouldn't drown out the other resource's real data. If one is
    // still loading, defer the banner until both settle.
    let combined_error: Option<String> = match (&*hunts_resource.read(), &*places_resource.read()) {
        (Some(Err(e1)), Some(Err(e2))) => Some(format!("{} · {}", e1, e2)),
        _ => None,
    };

    let hunt_count = hunts.len();
    let place_count = places.len();

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

            // Error banner — visible only when both endpoints fail. Distinguishes
            // "API down" from the legitimate-but-misleading "no hunts available".
            ErrorBanner {
                message: combined_error.as_ref().map(|e| format!("Не удалось загрузить квесты: {e}")).unwrap_or_default(),
                margin: "0 0 12px".to_string(),
            }

            if loading {
                div { style: "display: flex; flex-direction: column; gap: 10px;",
                    for _ in 0..3 {
                        div { style: "
                            background: #16213e; border: 4px solid #2a2a4a;
                            box-shadow: 4px 4px 0 #000; padding: 14px;
                            display: flex; flex-direction: column; gap: 8px;
                        ",
                            Skeleton { shape: SkeletonShape::Title, width: Some("50%".into()) }
                            Skeleton { shape: SkeletonShape::Text, width: Some("90%".into()) }
                        }
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
                        {hunts.iter().map(|h| {
                            // Per-hunt clones for the onclick closure (cycle #34).
                            let hunt_id = h.id.clone();
                            rsx! {
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
                                        onclick: move |_| { nav.push(Route::Quest { id: hunt_id.clone() }); },
                                        "\u{1F6B6} Start Hunt"
                                    }
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
                        {places.iter().map(|p| {
                            // Per-place clone for the onclick closure (cycle #34).
                            // "Scan" routes to the Quest screen, which hosts the
                            // working QR-scanner flow established in cycle #17.
                            let place_id = p.id.clone();
                            rsx! {
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
                                        onclick: move |_| { nav.push(Route::Quest { id: place_id.clone() }); },
                                        "\u{1F4F7} Scan"
                                    }
                                }
                            }
                        })}
                    }
                }
            }
        }
    }
}
