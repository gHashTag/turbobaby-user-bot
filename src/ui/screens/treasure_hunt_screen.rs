// Treasure Hunt Screen — QR-code checkpoint quest with GPS verification
use dioxus::prelude::*;

#[derive(Clone, Debug, PartialEq)]
struct Checkpoint {
    id: String,
    name: String,
    emoji: String,
    collected: bool,
}

fn mock_checkpoints() -> Vec<Checkpoint> {
    vec![
        Checkpoint { id: "cp1".into(), name: "Beach Bar".into(), emoji: "\u{1F3D6}\u{FE0F}".into(), collected: true },
        Checkpoint { id: "cp2".into(), name: "Viewpoint".into(), emoji: "\u{1F304}".into(), collected: true },
        Checkpoint { id: "cp3".into(), name: "Temple".into(), emoji: "\u{26E9}\u{FE0F}".into(), collected: false },
        Checkpoint { id: "cp4".into(), name: "Night Market".into(), emoji: "\u{1F6CD}\u{FE0F}".into(), collected: false },
        Checkpoint { id: "cp5".into(), name: "Secret Beach".into(), emoji: "\u{1F30A}".into(), collected: false },
    ]
}

fn render_checkpoint(cp: Checkpoint) -> Element {
    let border_color = if cp.collected { "#39ff14" } else { "#2a2a4a" };
    let opacity = if cp.collected { "0.7" } else { "1.0" };
    let status_text = if cp.collected { "\u{2705} Collected" } else { "\u{1F4CD} Not found" };
    let status_color = if cp.collected { "#39ff14" } else { "#8b8b9e" };

    rsx! {
        div { key: "{cp.id}", style: "
            background: #1a1a2e; border: 2px solid {border_color};
            border-radius: 8px; padding: 12px; opacity: {opacity};
            display: flex; align-items: center; gap: 12px;
            box-shadow: 4px 4px 0 #000;
        ",
            span { style: "font-size: 28px;", "{cp.emoji}" }
            div { style: "flex: 1;",
                div { style: "font-size: 14px; font-weight: 700; color: #ffffff; margin-bottom: 4px;",
                    "{cp.name}"
                }
                span { style: "font-size: 18px; color: {status_color};",
                    "{status_text}"
                }
            }
            if !cp.collected {
                button { style: "
                    font-family: 'Press Start 2P', monospace;
                    font-size: 10px; padding: 6px 10px;
                    background: #ffe600; color: #0f0f1a;
                    border: none; border-radius: 8px; cursor: pointer;
                ",
                    "\u{1F4F7} Scan"
                }
            }
        }
    }
}

#[component]
pub fn TreasureHuntScreen() -> Element {
    let checkpoints = use_signal(|| mock_checkpoints());
    let total = checkpoints.read().len();
    let collected = checkpoints.read().iter().filter(|c| c.collected).count();
    let progress_pct = if total > 0 { (collected as f64 / total as f64) * 100.0 } else { 0.0 };
    let remaining = total - collected;

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
                h1 { style: "font-size: 18px; color: #ffe600; text-shadow: 0 0 8px rgba(255,230,0,0.5);",
                    "\u{1F3F4}\u{200D}\u{2620}\u{FE0F} Treasure Hunt"
                }
                p { style: "font-size: 18px; color: #8b8b9e; margin-top: 6px;",
                    "Find checkpoints, scan QR codes, collect rewards!"
                }
            }

            // Progress bar
            div { style: "
                background: #1a1a2e; border: 2px solid #2a2a4a; border-radius: 8px;
                padding: 12px; margin-bottom: 16px; box-shadow: 4px 4px 0 #000;
            ",
                div { style: "display: flex; justify-content: space-between; margin-bottom: 8px;",
                    span { style: "font-size: 18px; color: #ffe600;",
                        "\u{1F4AF} Progress"
                    }
                    span { style: "font-size: 18px; color: #39ff14;",
                        "{collected}/{total}"
                    }
                }
                div { style: "background: #0f0f1a; border-radius: 8px; height: 8px; overflow: hidden;",
                    div { style: "background: linear-gradient(90deg, #ffe600, #ff9800); height: 100%; width: {progress_pct}%; border-radius: 8px;" }
                }
            }

            // Status banner
            {if collected == total && total > 0 {
                rsx! {
                    div { style: "
                        background: rgba(57,255,20,0.1); border: 2px solid #39ff14;
                        border-radius: 8px; padding: 12px; text-align: center;
                        margin-bottom: 16px; box-shadow: 4px 4px 0 #000;
                    ",
                        div { style: "font-size: 14px; color: #39ff14; margin-bottom: 4px;",
                            "\u{1F389} All checkpoints collected!"
                        }
                        div { style: "font-size: 18px; color: #8b8b9e;",
                            "Claim your reward at the bar"
                        }
                    }
                }
            } else {
                rsx! {
                    div { style: "
                        background: rgba(255,230,0,0.05); border: 2px solid #ffe600;
                        border-radius: 8px; padding: 12px; text-align: center;
                        margin-bottom: 16px; box-shadow: 4px 4px 0 #000;
                    ",
                        div { style: "font-size: 18px; color: #ffe600; margin-bottom: 4px;",
                            "\u{1F4E1} {remaining} checkpoints remaining"
                        }
                        div { style: "font-size: 18px; color: #8b8b9e;",
                            "Scan QR codes at each location to check in"
                        }
                    }
                }
            }}

            // Checkpoint list
            div { style: "display: flex; flex-direction: column; gap: 8px;",
                {checkpoints.read().clone().into_iter().map(|cp| render_checkpoint(cp))}
            }
        }
    }
}
