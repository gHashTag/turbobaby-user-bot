// AR Hunt Screen — GPS-based augmented reality mark collection
use dioxus::prelude::*;

#[derive(Clone, Debug, PartialEq)]
struct ARMark {
    id: String,
    name: String,
    category: String,
    emoji: String,
    color: String,
    distance_meters: f64,
    is_collected: bool,
}

fn mock_ar_marks() -> Vec<ARMark> {
    vec![
        ARMark { id: "ar1".into(), name: "Sunset Beach".into(), category: "beach".into(), emoji: "\u{1F3D6}\u{FE0F}".into(), color: "#00bcd4".into(), distance_meters: 5.2, is_collected: true },
        ARMark { id: "ar2".into(), name: "Mountain View".into(), category: "viewpoint".into(), emoji: "\u{1F304}".into(), color: "#ff9800".into(), distance_meters: 120.0, is_collected: true },
        ARMark { id: "ar3".into(), name: "Thai Temple".into(), category: "temple".into(), emoji: "\u{26E9}\u{FE0F}".into(), color: "#ff5722".into(), distance_meters: 350.0, is_collected: false },
        ARMark { id: "ar4".into(), name: "Reggae Bar".into(), category: "bar".into(), emoji: "\u{1F37A}".into(), color: "#9c27b0".into(), distance_meters: 890.0, is_collected: false },
        ARMark { id: "ar5".into(), name: "Jungle Trail".into(), category: "nature".into(), emoji: "\u{1F333}".into(), color: "#4caf50".into(), distance_meters: 1500.0, is_collected: false },
        ARMark { id: "ar6".into(), name: "Night Market".into(), category: "market".into(), emoji: "\u{1F6CD}\u{FE0F}".into(), color: "#ff6f00".into(), distance_meters: 2100.0, is_collected: false },
    ]
}

fn format_distance(meters: f64) -> String {
    if meters < 1000.0 { format!("{:.0}m", meters) } else { format!("{:.1}km", meters / 1000.0) }
}

fn render_ar_mark(m: ARMark) -> Element {
    let dist = format_distance(m.distance_meters);
    let border_color = if m.is_collected { "#39ff14".to_string() } else { m.color.clone() };

    rsx! {
        div { key: "{m.id}", style: "
            background: #1a1a2e; border: 2px solid {border_color};
            border-radius: 8px; padding: 10px;
            display: flex; align-items: center; gap: 10px;
            box-shadow: 4px 4px 0 #000;
        ",
            div { style: "
                width: 40px; height: 40px; border-radius: 50%;
                background: {m.color}22; border: 2px solid {m.color};
                display: flex; align-items: center; justify-content: center;
                font-size: 10px; flex-shrink: 0;
            ",
                "{m.emoji}"
            }
            div { style: "flex: 1;",
                div { style: "font-size: 14px; font-weight: 700; color: #ffffff; margin-bottom: 3px;",
                    "{m.name}"
                }
                div { style: "display: flex; gap: 8px; align-items: center;",
                    span { style: "font-size: 18px; color: {m.color}; text-transform: uppercase;",
                        "{m.category}"
                    }
                    span { style: "font-size: 18px; color: #8b8b9e;",
                        "\u{1F4CD} {dist}"
                    }
                }
            }
            if m.is_collected {
                span { style: "font-size: 10px;", "\u{2705}" }
            } else if m.distance_meters <= 10.0 {
                button { style: "
                    font-family: 'Press Start 2P', monospace;
                    font-size: 9px; padding: 5px 8px;
                    background: #39ff14; color: #0f0f1a;
                    border: none; border-radius: 8px; cursor: pointer;
                ",
                    "\u{1F3AF} Collect"
                }
            } else {
                span { style: "font-size: 18px; color: #8b8b9e;",
                    "\u{1F512}"
                }
            }
        }
    }
}

#[component]
pub fn ARHuntScreen() -> Element {
    let marks = use_signal(|| mock_ar_marks());
    let total = marks.read().len();
    let collected = marks.read().iter().filter(|m| m.is_collected).count();
    let progress_pct = if total > 0 { (collected as f64 / total as f64) * 100.0 } else { 0.0 };

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            font-family: 'Press Start 2P', monospace;
            padding: 20px 16px;
            padding-bottom: 80px;
        ",
            // Header
            div { style: "text-align: center; margin-bottom: 20px;",
                h1 { style: "font-size: 18px; color: #00bcd4; text-shadow: 0 0 8px rgba(0,188,212,0.5);",
                    "\u{1F30D} AR Hunt"
                }
                p { style: "font-size: 18px; color: #8b8b9e; margin-top: 6px;",
                    "Explore the island, find AR marks, collect rewards!"
                }
            }

            // GPS status
            div { style: "
                background: #1a1a2e; border: 2px solid #00bcd4; border-radius: 8px;
                padding: 10px; margin-bottom: 12px; text-align: center;
                box-shadow: 4px 4px 0 #000;
            ",
                span { style: "font-size: 18px; color: #00bcd4;",
                    "\u{1F4E1} GPS Active \u{2022} {total} marks nearby"
                }
            }

            // Progress
            div { style: "
                background: #1a1a2e; border: 2px solid #2a2a4a; border-radius: 8px;
                padding: 12px; margin-bottom: 16px; box-shadow: 4px 4px 0 #000;
            ",
                div { style: "display: flex; justify-content: space-between; margin-bottom: 8px;",
                    span { style: "font-size: 18px; color: #00bcd4;",
                        "\u{1F3AF} Today's Collection"
                    }
                    span { style: "font-size: 18px; color: #39ff14;",
                        "{collected}/{total}"
                    }
                }
                div { style: "background: #0f0f1a; border-radius: 8px; height: 8px; overflow: hidden;",
                    div { style: "background: linear-gradient(90deg, #00bcd4, #00e5ff); height: 100%; width: {progress_pct}%; border-radius: 8px;" }
                }
            }

            // Auto-collect hint
            div { style: "
                background: rgba(57,255,20,0.05); border: 2px solid #39ff1444;
                border-radius: 6px; padding: 8px; margin-bottom: 12px; text-align: center;
            ",
                span { style: "font-size: 18px; color: #39ff14;",
                    "\u{26A1} Marks within 10m are auto-collected"
                }
            }

            // Mark list
            div { style: "display: flex; flex-direction: column; gap: 8px;",
                {marks.read().clone().into_iter().map(|m| render_ar_mark(m))}
            }
        }
    }
}
