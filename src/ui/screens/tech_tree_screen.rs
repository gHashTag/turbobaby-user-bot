// Tech Tree Screen — Grow skill progression
use dioxus::prelude::*;
use crate::ui::game::tech_tree::TECH_TREE;

fn render_tech_node(id: &str, name: &str, cost: u32, unlocked: bool) -> Element {
    let border_color = if unlocked { "#39ff14" } else { "#2a2a4a" };
    let status_text = if unlocked { "\u{2705} Unlocked" } else { "\u{1F512} Locked" };
    let status_color = if unlocked { "#39ff14" } else { "#8b8b9e" };
    let bg = if unlocked { "rgba(57,255,20,0.05)" } else { "#16213e" };

    rsx! {
        div { key: "{id}", style: "
            background: {bg}; border: 2px solid {border_color};
            border-radius: 8px; padding: 12px;
            display: flex; align-items: center; gap: 12px;
            box-shadow: 4px 4px 0 #000;
        ",
            div { style: "
                width: 40px; height: 40px; border-radius: 8px;
                background: {border_color}22; border: 2px solid {border_color};
                display: flex; align-items: center; justify-content: center;
                font-size: 18px; flex-shrink: 0;
            ",
                if unlocked { "\u{1F331}" } else { "\u{1F510}" }
            }
            div { style: "flex: 1;",
                div { style: "font-size: 9px; font-weight: 700; color: #fff; margin-bottom: 3px;",
                    "{name}"
                }
                div { style: "display: flex; gap: 8px; align-items: center;",
                    span { style: "font-size: 6px; color: #ffe600;",
                        "\u{2B50} {cost} pts"
                    }
                    span { style: "font-size: 6px; color: {status_color};",
                        "{status_text}"
                    }
                }
            }
            if !unlocked {
                button { style: "
                    font-family: 'Press Start 2P', monospace;
                    font-size: 5px; padding: 5px 8px;
                    background: #16213e; color: #ffe600;
                    border: 1px solid #ffe600; border-radius: 4px; cursor: pointer;
                ",
                    "Unlock"
                }
            }
        }
    }
}

#[component]
pub fn TechTreeScreen() -> Element {
    let points = use_signal(|| 10u32);
    let total = TECH_TREE.len();
    let unlocked_count = TECH_TREE.iter().filter(|t| t.unlocked).count();

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
            div { style: "text-align: center; margin-bottom: 24px;",
                h1 { style: "font-size: 12px; color: #00e5ff; text-shadow: 0 0 8px rgba(0,229,255,0.5);",
                    "\u{1F333} Tech Tree"
                }
                p { style: "font-size: 7px; color: #8b8b9e; margin-top: 6px;",
                    "Unlock growing skills to improve your garden"
                }
            }

            // Points display
            div { style: "
                background: #1a1a2e; border: 2px solid #ffe600; border-radius: 8px;
                padding: 12px; margin-bottom: 16px; text-align: center;
                box-shadow: 4px 4px 0 #000;
            ",
                span { style: "font-size: 7px; color: #ffe600;",
                    "\u{2B50} {points} Skill Points \u{2022} {unlocked_count}/{total} Unlocked"
                }
            }

            // Progress bar
            div { style: "
                background: #1a1a2e; border: 2px solid #2a2a4a; border-radius: 8px;
                padding: 12px; margin-bottom: 16px; box-shadow: 4px 4px 0 #000;
            ",
                div { style: "background: #0f0f1a; border-radius: 4px; height: 8px; overflow: hidden;",
                    div { style: "background: linear-gradient(90deg, #00e5ff, #39ff14); height: 100%; width: {(unlocked_count * 100 / total)}%; border-radius: 4px;" }
                }
            }

            // Tech nodes
            div { style: "display: flex; flex-direction: column; gap: 10px;",
                {TECH_TREE.iter().map(|t| render_tech_node(t.id, t.name, t.cost, t.unlocked))}
            }
        }
    }
}
