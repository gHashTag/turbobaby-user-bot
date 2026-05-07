use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::state::{Cart, CartItem};
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::assets;
use crate::ui::api::context::api_base_url;
use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_SETS_TITLE, T_SETS_DESC, T_FILTER_ALL, T_ADD_TO_CART};

#[derive(Debug, Clone, Deserialize)]
struct ApiSet {
    id: String,
    name: String,
    description: Option<String>,
    #[serde(default)]
    description_localized: Option<std::collections::HashMap<String, String>>,
    icon: Option<String>,
    strains: Option<Vec<String>>,
    total_price: f64,
    discount_percent: f64,
    target_mood: Option<String>,
    time_of_day: Option<String>,
    image_url: Option<String>,
    is_available: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SetsResponse {
    sets: Vec<ApiSet>,
}

fn mood_emoji(mood: &str) -> &'static str {
    match mood.to_lowercase().as_str() {
        "energy" => "⚡",
        "party" => "🎉",
        "heavy" => "🏋️",
        "beginner" => "🌱",
        "relax" | "relaxation" => "😌",
        _ => "🎁",
    }
}

fn mood_color(mood: &str) -> &'static str {
    match mood.to_lowercase().as_str() {
        "energy" => "#ffe600",
        "party" => "#ff6b9d",
        "heavy" => "#b388ff",
        "beginner" => "#39ff14",
        "relax" | "relaxation" => "#00e5ff",
        _ => "#8b8b9e",
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum MoodFilter {
    All,
    Energy,
    Party,
    Heavy,
    Beginner,
}

impl MoodFilter {
    fn label(&self) -> &'static str {
        match self {
            Self::All => t(Lang::Russian, T_FILTER_ALL),
            Self::Energy => "⚡ Energy",
            Self::Party => "🎉 Party",
            Self::Heavy => "🏋️ Heavy",
            Self::Beginner => "🌱 Beginner",
        }
    }
    fn matches(&self, mood: &str) -> bool {
        match self {
            Self::All => true,
            Self::Energy => mood == "energy",
            Self::Party => mood == "party",
            Self::Heavy => mood == "heavy",
            Self::Beginner => mood == "beginner",
        }
    }
}

#[component]
pub fn SetsScreen() -> Element {
    let mut cart = use_context::<Signal<Cart>>();
    let mut mood_filter = use_signal(|| MoodFilter::All);

    let sets_title = t(Lang::Russian, T_SETS_TITLE);
    let sets_desc = t(Lang::Russian, T_SETS_DESC);
    let add_to_cart = t(Lang::Russian, T_ADD_TO_CART);

    let sets_resource = use_resource(|| async move {
        let base = api_base_url();
        let url = format!("{}/api/sets", base);
        reqwest::Client::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<SetsResponse>()
            .await
            .map(|r| r.sets)
            .map_err(|e| e.to_string())
    });

    let filtered_sets = match &*sets_resource.read() {
        Some(Ok(sets)) => {
            let f = mood_filter();
            sets.iter()
                .filter(|s| {
                    let mood = s.target_mood.as_deref().unwrap_or("");
                    f.matches(mood)
                })
                .cloned()
                .collect::<Vec<_>>()
        }
        _ => Vec::new(),
    };

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            font-family: 'Press Start 2P', monospace;
            padding-bottom: 80px;
        ",
            div { style: "padding: 20px 16px 12px; text-align: center;",
                h1 { style: "font-size: 18px; color: #b388ff; text-shadow: 0 0 8px rgba(179,136,255,0.5);", "{sets_title}" }
                p { style: "font-size: 11px; color: #8b8b9e; margin-top: 4px;", "{sets_desc}" }
            }

            // Mood filters
            div { style: "display: flex; gap: 6px; padding: 0 16px 12px; overflow-x: auto;",
                for filter in [MoodFilter::All, MoodFilter::Energy, MoodFilter::Party, MoodFilter::Heavy, MoodFilter::Beginner] {
                    {
                        let is_active = mood_filter() == filter;
                        let bg = if is_active { "#b388ff" } else { "transparent" };
                        let color = if is_active { "#0f0f1a" } else { "#8b8b9e" };
                        let border = if is_active { "#b388ff" } else { "#2a2a4a" };
                        let label = filter.label();
                        rsx! {
                            button {
                                style: "
                                    font-family: 'Press Start 2P', monospace;
                                    font-size: 10px; padding: 6px 10px;
                                    background: {bg}; color: {color};
                                    border: 2px solid {border}; border-radius: 8px;
                                    cursor: pointer; white-space: nowrap;
                                ",
                                onclick: move |_| mood_filter.set(filter),
                                "{label}"
                            }
                        }
                    }
                }
            }

            // Featured Packs
            div { style: "padding: 0 16px 16px;",
                h2 { style: "font-size: 14px; color: #b388ff; margin-bottom: 12px; text-transform: uppercase; letter-spacing: 1px;", "Featured Packs" }
                div { style: "display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px;",
                    div { style: "
                        background: #1a1a2e; border: 2px solid #2a2a4a;
                        border-radius: 8px; overflow: hidden; box-shadow: 4px 4px 0 #000;
                    ",
                        img { src: "{assets::packs::INDICA}", alt: "Indica Pack", style: "width: 100%; aspect-ratio: 1; object-fit: cover;" }
                        div { style: "padding: 6px; text-align: center; color: #e8e8e8; font-size: 10px;", "Indica Pack" }
                    }
                    div { style: "
                        background: #1a1a2e; border: 2px solid #2a2a4a;
                        border-radius: 8px; overflow: hidden; box-shadow: 4px 4px 0 #000;
                    ",
                        img { src: "{assets::packs::SATIVA}", alt: "Sativa Pack", style: "width: 100%; aspect-ratio: 1; object-fit: cover;" }
                        div { style: "padding: 6px; text-align: center; color: #e8e8e8; font-size: 10px;", "Sativa Pack" }
                    }
                    div { style: "
                        background: #1a1a2e; border: 2px solid #2a2a4a;
                        border-radius: 8px; overflow: hidden; box-shadow: 4px 4px 0 #000;
                    ",
                        img { src: "{assets::packs::STARTER}", alt: "Starter Pack", style: "width: 100%; aspect-ratio: 1; object-fit: cover;" }
                        div { style: "padding: 6px; text-align: center; color: #e8e8e8; font-size: 10px;", "Starter Pack" }
                    }
                }
            }

            // Sets from API
            {
                match &*sets_resource.read() {
                    Some(Ok(sets)) if sets.is_empty() => rsx! {
                        div { style: "text-align: center; padding: 40px 16px;",
                            div { style: "font-size: 36px; margin-bottom: 12px;", "📦" }
                            p { style: "font-size: 12px; color: #8b8b9e;", "No sets available yet" }
                        }
                    },
                    Some(Ok(_)) => rsx! {
                        div { style: "padding: 0 16px 8px;",
                            h2 { style: "font-size: 14px; color: #b388ff; margin-bottom: 12px; text-transform: uppercase; letter-spacing: 1px;",
                                "All Sets ({filtered_sets.len()})"
                            }
                        }
                        div { style: "display: flex; flex-direction: column; gap: 12px; padding: 0 16px;",
                            for set in filtered_sets.iter() {
                                {
                                    let s = set.clone();
                                    let mood = s.target_mood.as_deref().unwrap_or("party");
                                    let emoji = mood_emoji(mood);
                                    let m_color = mood_color(mood);
                                    let has_discount = s.discount_percent > 0.0;
                                    let discounted_price = if has_discount {
                                        s.total_price * (1.0 - s.discount_percent / 100.0)
                                    } else {
                                        s.total_price
                                    };
                                    let original_price_str = format!("฿{}", s.total_price as i32);
                                    let price_str = format!("฿{}", discounted_price as i32);
                                    let discount_badge = if has_discount {
                                        format!("{}% OFF", s.discount_percent as i32)
                                    } else {
                                        String::new()
                                    };
                                    let set_name = s.name.clone();
                                    let set_id = s.id.clone();
                                    let icon = s.icon.as_deref().unwrap_or("🎁");
                                    let desc = s.description.as_deref().unwrap_or("");
                                    let time_str = s.time_of_day.as_deref().unwrap_or("");
                                    let strains_count = s.strains.as_ref().map(|v| v.len()).unwrap_or(0);
                                    let strains_label = if strains_count > 0 { format!("{} strains", strains_count) } else { String::new() };

                                    rsx! {
                                        div { style: "
                                            background: #1a1a2e;
                                            border: 2px solid {m_color}33;
                                            border-radius: 8px;
                                            overflow: hidden;
                                            box-shadow: 4px 4px 0 #000;
                                        ",
                                            div { style: "
                                                height: 100px;
                                                background: linear-gradient(135deg, #1a1a2e, #1a1a2e);
                                                display: flex; align-items: center; justify-content: center;
                                                font-size: 40px; position: relative;
                                            ",
                                                "{icon}"
                                                if has_discount {
                                                    span { style: "
                                                        position: absolute; top: 6px; right: 6px;
                                                        font-size: 9px; background: {m_color}; color: #0f0f1a;
                                                        padding: 2px 6px; border-radius: 3px;
                                                    ", "{discount_badge}" }
                                                }
                                            }
                                            div { style: "padding: 12px;",
                                                div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px;",
                                                    span { style: "font-size: 14px; font-weight: bold;", "{set_name}" }
                                                    span { style: "font-size: 18px; color: {m_color};", "{emoji} {mood}" }
                                                }
                                                if !desc.is_empty() {
                                                    div { style: "font-size: 18px; color: #8b8b9e; margin-bottom: 6px;", "{desc}" }
                                                }
                                                if !strains_label.is_empty() || !time_str.is_empty() {
                                                    div { style: "font-size: 18px; color: #8b8b9e; margin-bottom: 8px;",
                                                        if !strains_label.is_empty() {
                                                            span { style: "border: 2px solid #2a2a4a; padding: 1px 4px; border-radius: 3px; margin-right: 4px;", "{strains_label}" }
                                                        }
                                                        if !time_str.is_empty() {
                                                            span { style: "border: 2px solid #2a2a4a; padding: 1px 4px; border-radius: 3px;", "🕐 {time_str}" }
                                                        }
                                                    }
                                                }
                                                div { style: "display: flex; justify-content: space-between; align-items: center;",
                                                    div {
                                                        if has_discount {
                                                            span { style: "font-size: 12px; color: #8b8b9e; text-decoration: line-through; margin-right: 6px;", "{original_price_str}" }
                                                        }
                                                        span { style: "font-size: 18px; color: #39ff14;", "{price_str}" }
                                                    }
                                                }
                                            }
                                            div { style: "padding: 0 12px 12px;",
                                                button {
                                                    style: "
                                                        font-family: 'Press Start 2P', monospace;
                                                        font-size: 10px; width: 100%; padding: 8px;
                                                        background: #39ff14; color: #0f0f1a;
                                                        border: none; border-radius: 8px; cursor: pointer;
                                                    ",
                                                    onclick: move |_| {
                                                        let mut c = cart.write();
                                                        c.add_item(CartItem {
                                                            id: set_id.clone(),
                                                            name: set_name.clone(),
                                                            price: discounted_price,
                                                            quantity: 1,
                                                            image_url: None,
                                                        });
                                                    },
                                                    "{add_to_cart}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { style: "text-align: center; padding: 40px 16px;",
                            div { style: "font-size: 36px; margin-bottom: 12px;", "⚠️" }
                            p { style: "font-size: 12px; color: #ff4757;", "Error: {e}" }
                            p { style: "font-size: 18px; color: #8b8b9e; margin-top: 8px;", "Showing offline sets..." }
                        }
                        div { style: "display: flex; flex-direction: column; gap: 12px; padding: 0 16px;",
                            {offline_sets_card("Starter Pack", "3 strains + grinder + papers", 4200.0, 3200.0, &mut cart)}
                            {offline_sets_card("Connoisseur Collection", "5 premium strains + vaporizer + case", 12000.0, 8500.0, &mut cart)}
                            {offline_sets_card("Party Pack", "10 pre-rolls + rolling kit + lighter", 0.0, 2800.0, &mut cart)}
                        }
                    },
                    None => rsx! {
                        div { style: "display: flex; flex-direction: column; gap: 12px; padding: 0 16px;",
                            div { style: "
                                background: #1a1a2e; border: 2px solid #2a2a4a;
                                border-radius: 8px; padding: 20px; text-align: center;
                                box-shadow: 4px 4px 0 #000; min-height: 120px;
                            ",
                                div { style: "font-size: 12px; color: #b388ff; margin-bottom: 8px;", "Loading sets..." }
                            }
                        }
                    },
                }
            }

            BottomNav {}
        }
    }
}

fn offline_sets_card(name: &str, desc: &str, original: f64, price: f64, cart: &mut Signal<Cart>) -> Element {
    let add_to_cart = t(Lang::Russian, T_ADD_TO_CART);
    let has_discount = original > 0.0 && original != price;
    let n = name.to_string();
    let p = price;
    let set_id = format!("set-{}", name.to_lowercase().replace(' ', "-"));
    let original_str = format!("฿{}", original as i32);
    let price_str = format!("฿{}", price as i32);
    let mut c = cart.clone();

    rsx! {
        div { style: "
            background: #1a1a2e; border: 2px solid #2a2a4a;
            border-radius: 8px; overflow: hidden; box-shadow: 4px 4px 0 #000;
        ",
            div { style: "padding: 12px;",
                div { style: "font-size: 14px; font-weight: bold; margin-bottom: 4px;", "{n}" }
                div { style: "font-size: 18px; color: #8b8b9e; margin-bottom: 8px;", "{desc}" }
                div { style: "display: flex; justify-content: space-between; align-items: center;",
                    div {
                        if has_discount {
                            span { style: "font-size: 12px; color: #8b8b9e; text-decoration: line-through; margin-right: 6px;", "{original_str}" }
                        }
                        span { style: "font-size: 18px; color: #39ff14;", "{price_str}" }
                    }
                }
            }
            div { style: "padding: 0 12px 12px;",
                button {
                    style: "
                        font-family: 'Press Start 2P', monospace;
                        font-size: 10px; width: 100%; padding: 8px;
                        background: #39ff14; color: #0f0f1a;
                        border: none; border-radius: 8px; cursor: pointer;
                    ",
                    onclick: move |_| {
                        let mut w = c.write();
                        w.add_item(CartItem {
                            id: set_id.clone(),
                            name: n.clone(),
                            price: p,
                            quantity: 1,
                            image_url: None,
                        });
                    },
                    "{add_to_cart}"
                }
            }
        }
    }
}
