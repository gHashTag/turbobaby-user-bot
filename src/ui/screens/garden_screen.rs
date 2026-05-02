use dioxus::prelude::*;
use crate::ui::routes::Route;

#[component]
pub fn GardenScreen() -> Element {
    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            font-family: 'Press Start 2P', monospace;
            padding-bottom: 80px;
        ",
            // Header
            div { style: "padding: 20px 16px 12px; text-align: center;",
                h1 { style: "font-size: 12px; color: #39ff14; text-shadow: 0 0 8px rgba(57,255,20,0.5);", "🌱 Garden" }
                p { style: "font-size: 7px; color: #8b8b9e; margin-top: 4px;", "Your virtual plants" }
            }

            // Growth timeline
            div { style: "max-width: 400px; margin: 0 auto; padding: 0 16px; position: relative;",

                // Timeline line
                // Seed stage
                div { style: "display: flex; gap: 16px; margin-bottom: 20px; position: relative; align-items: flex-start;",
                    div { style: "
                        width: 56px; height: 56px; border-radius: 50%;
                        display: flex; align-items: center; justify-content: center;
                        font-size: 24px; flex-shrink: 0; z-index: 1;
                        background: rgba(139,90,43,0.3); border: 2px solid #8b5a2b;
                    ", "🫘" }
                    div { style: "
                        flex: 1; background: #1a1a2e; border-radius: 12px;
                        padding: 16px; border: 2px solid rgba(255,255,255,0.1);
                    ",
                        div { style: "font-size: 10px; color: #8b5a2b; margin-bottom: 4px;", "Seed" }
                        div { style: "font-size: 7px; color: #888; margin-bottom: 8px;", "Planted and waiting to germinate" }
                        div { style: "display: flex; gap: 12px;",
                            div { style: "font-size: 7px; color: #666;", "Day: {\"0\"}" }
                            div { style: "font-size: 7px; color: #666;", "Water: {\"0ml\"}" }
                        }
                        div { style: "height: 8px; background: rgba(0,0,0,0.4); border-radius: 4px; overflow: hidden; margin-top: 8px;",
                            div { style: "height: 100%; width: 0%; border-radius: 4px; background: #8b5a2b;" }
                        }
                    }
                }

                // Sprout stage
                div { style: "display: flex; gap: 16px; margin-bottom: 20px; position: relative; align-items: flex-start;",
                    div { style: "
                        width: 56px; height: 56px; border-radius: 50%;
                        display: flex; align-items: center; justify-content: center;
                        font-size: 24px; flex-shrink: 0; z-index: 1;
                        background: rgba(57,255,20,0.1); border: 2px solid #39ff14;
                    ", "🌱" }
                    div { style: "
                        flex: 1; background: #1a1a2e; border-radius: 12px;
                        padding: 16px; border: 2px solid rgba(255,255,255,0.1);
                    ",
                        div { style: "font-size: 10px; color: #39ff14; margin-bottom: 4px;", "Sprout" }
                        div { style: "font-size: 7px; color: #888; margin-bottom: 8px;", "First leaves emerging from soil" }
                        div { style: "display: flex; gap: 12px;",
                            div { style: "font-size: 7px; color: #666;", "Day: {\"3-7\"}" }
                            div { style: "font-size: 7px; color: #666;", "Water: {\"50ml/day\"}" }
                        }
                        div { style: "height: 8px; background: rgba(0,0,0,0.4); border-radius: 4px; overflow: hidden; margin-top: 8px;",
                            div { style: "height: 100%; width: 25%; border-radius: 4px; background: #39ff14;" }
                        }
                    }
                }

                // Vegetative stage — ACTIVE
                div { style: "display: flex; gap: 16px; margin-bottom: 20px; position: relative; align-items: flex-start;",
                    div { style: "
                        width: 56px; height: 56px; border-radius: 50%;
                        display: flex; align-items: center; justify-content: center;
                        font-size: 24px; flex-shrink: 0; z-index: 1;
                        background: rgba(0,229,255,0.1); border: 2px solid #00e5ff;
                    ", "🌿" }
                    div { style: "
                        flex: 1; background: #1a1a2e; border-radius: 12px;
                        padding: 16px; border: 2px solid #39ff14;
                        box-shadow: 0 0 16px rgba(57,255,20,0.2);
                    ",
                        div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px;",
                            div { style: "font-size: 10px; color: #00e5ff;", "Vegetative" }
                            span { style: "font-size: 6px; color: #39ff14; background: rgba(57,255,20,0.15); padding: 2px 6px; border-radius: 3px;", "CURRENT" }
                        }
                        div { style: "font-size: 7px; color: #888; margin-bottom: 8px;", "Rapid growth, building foliage" }
                        div { style: "display: flex; gap: 12px;",
                            div { style: "font-size: 7px; color: #666;", "Day: {\"14-42\"}" }
                            div { style: "font-size: 7px; color: #666;", "Water: {\"200ml/day\"}" }
                        }
                        div { style: "height: 8px; background: rgba(0,0,0,0.4); border-radius: 4px; overflow: hidden; margin-top: 8px;",
                            div { style: "height: 100%; width: 50%; border-radius: 4px; background: linear-gradient(90deg, #39ff14, #00e5ff);" }
                        }
                    }
                }

                // Flowering stage
                div { style: "display: flex; gap: 16px; margin-bottom: 20px; position: relative; align-items: flex-start;",
                    div { style: "
                        width: 56px; height: 56px; border-radius: 50%;
                        display: flex; align-items: center; justify-content: center;
                        font-size: 24px; flex-shrink: 0; z-index: 1;
                        background: rgba(255,107,53,0.1); border: 2px solid #ff6b35;
                    ", "🌸" }
                    div { style: "
                        flex: 1; background: #1a1a2e; border-radius: 12px;
                        padding: 16px; border: 2px solid rgba(255,255,255,0.1);
                    ",
                        div { style: "font-size: 10px; color: #ff6b35; margin-bottom: 4px;", "Flowering" }
                        div { style: "font-size: 7px; color: #888; margin-bottom: 8px;", "Buds forming, trichomes developing" }
                        div { style: "display: flex; gap: 12px;",
                            div { style: "font-size: 7px; color: #666;", "Day: {\"42-70\"}" }
                            div { style: "font-size: 7px; color: #666;", "Water: {\"150ml/day\"}" }
                        }
                        div { style: "height: 8px; background: rgba(0,0,0,0.4); border-radius: 4px; overflow: hidden; margin-top: 8px;",
                            div { style: "height: 100%; width: 75%; border-radius: 4px; background: linear-gradient(90deg, #00e5ff, #ff6b35);" }
                        }
                    }
                }

                // Harvested stage
                div { style: "display: flex; gap: 16px; margin-bottom: 20px; position: relative; align-items: flex-start;",
                    div { style: "
                        width: 56px; height: 56px; border-radius: 50%;
                        display: flex; align-items: center; justify-content: center;
                        font-size: 24px; flex-shrink: 0; z-index: 1;
                        background: rgba(255,215,0,0.1); border: 2px solid #ffd700;
                    ", "🌾" }
                    div { style: "
                        flex: 1; background: #1a1a2e; border-radius: 12px;
                        padding: 16px; border: 2px solid rgba(255,255,255,0.1);
                    ",
                        div { style: "font-size: 10px; color: #ffd700; margin-bottom: 4px;", "Harvested" }
                        div { style: "font-size: 7px; color: #888; margin-bottom: 8px;", "Ready for drying and curing" }
                        div { style: "display: flex; gap: 12px;",
                            div { style: "font-size: 7px; color: #666;", "Day: {\"70+\"}" }
                            div { style: "font-size: 7px; color: #666;", "Yield: {\"~50g\"}" }
                        }
                        div { style: "height: 8px; background: rgba(0,0,0,0.4); border-radius: 4px; overflow: hidden; margin-top: 8px;",
                            div { style: "height: 100%; width: 100%; border-radius: 4px; background: linear-gradient(90deg, #ff6b35, #ffd700);" }
                        }
                    }
                }
            }

            // Bottom Navigation
            {bottom_nav()}
        }
    }
}

fn bottom_nav() -> Element {
    rsx! {
        nav { style: "
            position: fixed; bottom: 0; left: 0; right: 0;
            background: #1a1a2e; border-top: 2px solid #2a2a4a;
            display: flex; justify-content: space-around;
            padding: 10px 0; z-index: 100;
        ",
            Link { to: Route::Home {},
                div { style: "text-align: center; cursor: pointer;",
                    div { style: "font-size: 20px;", "🏠" }
                    div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Home" }
                }
            }
            Link { to: Route::Menu {},
                div { style: "text-align: center; cursor: pointer;",
                    div { style: "font-size: 20px;", "🌿" }
                    div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Menu" }
                }
            }
            Link { to: Route::Cart {},
                div { style: "text-align: center; cursor: pointer;",
                    div { style: "font-size: 20px;", "🛒" }
                    div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Cart" }
                }
            }
            Link { to: Route::Orders {},
                div { style: "text-align: center; cursor: pointer;",
                    div { style: "font-size: 20px;", "📋" }
                    div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Orders" }
                }
            }
            Link { to: Route::Profile {},
                div { style: "text-align: center; cursor: pointer;",
                    div { style: "font-size: 20px;", "👤" }
                    div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Profile" }
                }
            }
        }
    }
}
