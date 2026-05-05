// Admin Panel Screen — CRUD management for strains, sets, accessories, tea, quests
use dioxus::prelude::*;

#[component]
pub fn AdminScreen() -> Element {
    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            font-family: 'Press Start 2P', monospace;
            padding: 20px 16px;
            padding-bottom: 80px;
        ",
            div { style: "text-align: center; margin-bottom: 20px;",
                h1 { style: "font-size: 12px; color: #ff4757; text-shadow: 0 0 8px rgba(255,71,87,0.5);",
                    "\u{1F527} Admin Panel"
                }
                p { style: "font-size: 7px; color: #8b8b9e; margin-top: 6px;",
                    "Manage strains, sets, accessories, and more"
                }
            }

            // Warning
            div { style: "
                background: rgba(255,71,87,0.08); border: 2px solid #ff475744;
                border-radius: 8px; padding: 10px; text-align: center; margin-bottom: 16px;
            ",
                span { style: "font-size: 6px; color: #ff4757;",
                    "\u{26A0}\u{FE0F} Admin access required \u{2022} Changes affect production"
                }
            }

            // API status
            div { style: "
                background: #1a1a2e; border: 2px solid #2a2a4a; border-radius: 8px;
                padding: 10px; margin-bottom: 16px;
                display: flex; justify-content: space-between; align-items: center;
                box-shadow: 4px 4px 0 #000;
            ",
                span { style: "font-size: 7px; color: #8b8b9e;", "API Status" }
                span { style: "font-size: 7px; color: #39ff14;", "\u{1F7E2} Connected" }
            }

            // Section grid
            div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px;",
                // Strains
                div { style: "background: #16213e; border: 2px solid #39ff1433; border-radius: 8px; padding: 14px; display: flex; align-items: center; gap: 12px; box-shadow: 4px 4px 0 #000;",
                    div { style: "width: 40px; height: 40px; border-radius: 8px; background: #39ff1415; border: 1px solid #39ff1444; display: flex; align-items: center; justify-content: center; font-size: 20px;",
                        "\u{1F33F}"
                    }
                    div { style: "flex: 1;",
                        div { style: "font-size: 10px; font-weight: 700; color: #fff; margin-bottom: 3px;", "Strains" }
                        div { style: "font-size: 6px; color: #8b8b9e;", "/api/strains" }
                    }
                    span { style: "font-size: 6px; color: #39ff14;", "CRUD \u{2192}" }
                }
                // Sets
                div { style: "background: #16213e; border: 2px solid #ffe60033; border-radius: 8px; padding: 14px; display: flex; align-items: center; gap: 12px; box-shadow: 4px 4px 0 #000;",
                    div { style: "width: 40px; height: 40px; border-radius: 8px; background: #ffe60015; border: 1px solid #ffe60044; display: flex; align-items: center; justify-content: center; font-size: 20px;",
                        "\u{1F381}"
                    }
                    div { style: "flex: 1;",
                        div { style: "font-size: 10px; font-weight: 700; color: #fff; margin-bottom: 3px;", "Sets" }
                        div { style: "font-size: 6px; color: #8b8b9e;", "/api/sets" }
                    }
                    span { style: "font-size: 6px; color: #ffe600;", "CRUD \u{2192}" }
                }
                // Accessories
                div { style: "background: #16213e; border: 2px solid #00e5ff33; border-radius: 8px; padding: 14px; display: flex; align-items: center; gap: 12px; box-shadow: 4px 4px 0 #000;",
                    div { style: "width: 40px; height: 40px; border-radius: 8px; background: #00e5ff15; border: 1px solid #00e5ff44; display: flex; align-items: center; justify-content: center; font-size: 20px;",
                        "\u{1F527}"
                    }
                    div { style: "flex: 1;",
                        div { style: "font-size: 10px; font-weight: 700; color: #fff; margin-bottom: 3px;", "Accessories" }
                        div { style: "font-size: 6px; color: #8b8b9e;", "/api/accessories" }
                    }
                    span { style: "font-size: 6px; color: #00e5ff;", "CRUD \u{2192}" }
                }
                // Tea
                div { style: "background: #16213e; border: 2px solid #b388ff33; border-radius: 8px; padding: 14px; display: flex; align-items: center; gap: 12px; box-shadow: 4px 4px 0 #000;",
                    div { style: "width: 40px; height: 40px; border-radius: 8px; background: #b388ff15; border: 1px solid #b388ff44; display: flex; align-items: center; justify-content: center; font-size: 20px;",
                        "\u{1F375}"
                    }
                    div { style: "flex: 1;",
                        div { style: "font-size: 10px; font-weight: 700; color: #fff; margin-bottom: 3px;", "Tea Products" }
                        div { style: "font-size: 6px; color: #8b8b9e;", "/api/tea" }
                    }
                    span { style: "font-size: 6px; color: #b388ff;", "CRUD \u{2192}" }
                }
                // Orders
                div { style: "background: #16213e; border: 2px solid #ff980033; border-radius: 8px; padding: 14px; display: flex; align-items: center; gap: 12px; box-shadow: 4px 4px 0 #000;",
                    div { style: "width: 40px; height: 40px; border-radius: 8px; background: #ff980015; border: 1px solid #ff980044; display: flex; align-items: center; justify-content: center; font-size: 20px;",
                        "\u{1F4E6}"
                    }
                    div { style: "flex: 1;",
                        div { style: "font-size: 10px; font-weight: 700; color: #fff; margin-bottom: 3px;", "Orders" }
                        div { style: "font-size: 6px; color: #8b8b9e;", "/api/orders" }
                    }
                    span { style: "font-size: 6px; color: #ff9800;", "CRUD \u{2192}" }
                }
                // Quests
                div { style: "background: #16213e; border: 2px solid #4caf5033; border-radius: 8px; padding: 14px; display: flex; align-items: center; gap: 12px; box-shadow: 4px 4px 0 #000;",
                    div { style: "width: 40px; height: 40px; border-radius: 8px; background: #4caf5015; border: 1px solid #4caf5044; display: flex; align-items: center; justify-content: center; font-size: 20px;",
                        "\u{1F5FA}\u{FE0F}"
                    }
                    div { style: "flex: 1;",
                        div { style: "font-size: 10px; font-weight: 700; color: #fff; margin-bottom: 3px;", "Quests" }
                        div { style: "font-size: 6px; color: #8b8b9e;", "/api/quest" }
                    }
                    span { style: "font-size: 6px; color: #4caf50;", "CRUD \u{2192}" }
                }
                // Loyalty
                div { style: "background: #16213e; border: 2px solid #ffd70033; border-radius: 8px; padding: 14px; display: flex; align-items: center; gap: 12px; box-shadow: 4px 4px 0 #000;",
                    div { style: "width: 40px; height: 40px; border-radius: 8px; background: #ffd70015; border: 1px solid #ffd70044; display: flex; align-items: center; justify-content: center; font-size: 20px;",
                        "\u{1F451}"
                    }
                    div { style: "flex: 1;",
                        div { style: "font-size: 10px; font-weight: 700; color: #fff; margin-bottom: 3px;", "Loyalty" }
                        div { style: "font-size: 6px; color: #8b8b9e;", "/api/loyalty" }
                    }
                    span { style: "font-size: 6px; color: #ffd700;", "CRUD \u{2192}" }
                }
                // Referrals
                div { style: "background: #16213e; border: 2px solid #e040fb33; border-radius: 8px; padding: 14px; display: flex; align-items: center; gap: 12px; box-shadow: 4px 4px 0 #000;",
                    div { style: "width: 40px; height: 40px; border-radius: 8px; background: #e040fb15; border: 1px solid #e040fb44; display: flex; align-items: center; justify-content: center; font-size: 20px;",
                        "\u{1F91D}"
                    }
                    div { style: "flex: 1;",
                        div { style: "font-size: 10px; font-weight: 700; color: #fff; margin-bottom: 3px;", "Referrals" }
                        div { style: "font-size: 6px; color: #8b8b9e;", "/api/referrals" }
                    }
                    span { style: "font-size: 6px; color: #e040fb;", "CRUD \u{2192}" }
                }
            }

            // Quick actions
            div { style: "margin-top: 20px;",
                div { style: "font-size: 8px; color: #8b8b9e; margin-bottom: 10px;", "Quick Actions" }
                div { style: "display: flex; flex-direction: column; gap: 8px;",
                    button { style: "
                        font-family: 'Press Start 2P', monospace;
                        font-size: 7px; padding: 10px;
                        background: #16213e; color: #e8e8e8;
                        border: 2px solid #2a2a4a; border-radius: 6px;
                        cursor: pointer; text-align: left;
                        box-shadow: 4px 4px 0 #000;
                    ", "\u{1F504} Run Migrations" }
                    button { style: "
                        font-family: 'Press Start 2P', monospace;
                        font-size: 7px; padding: 10px;
                        background: #16213e; color: #e8e8e8;
                        border: 2px solid #2a2a4a; border-radius: 6px;
                        cursor: pointer; text-align: left;
                        box-shadow: 4px 4px 0 #000;
                    ", "\u{1F4CA} View Analytics" }
                    button { style: "
                        font-family: 'Press Start 2P', monospace;
                        font-size: 7px; padding: 10px;
                        background: #16213e; color: #e8e8e8;
                        border: 2px solid #2a2a4a; border-radius: 6px;
                        cursor: pointer; text-align: left;
                        box-shadow: 4px 4px 0 #000;
                    ", "\u{1F4E4} Export Data" }
                }
            }
        }
    }
}
