use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::state::{Cart, CartItem, CartItemType};
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::assets;
use crate::ui::api::context::api_base_url;
use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_SETS_TITLE, T_SETS_DESC, T_FILTER_ALL, T_ADD_TO_CART};

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
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
    #[serde(default)]
    video_url: Option<String>,
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
        _ => "#888",
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
    let cart = use_context::<Signal<Cart>>();
    let mut mood_filter = use_signal(|| MoodFilter::All);

    let sets_title = t(Lang::Russian, T_SETS_TITLE);
    let sets_desc = t(Lang::Russian, T_SETS_DESC);
    let _add_to_cart = t(Lang::Russian, T_ADD_TO_CART);

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
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding-bottom:80px;",

            div { style: "padding:20px 16px 16px;text-align:center;",
                h1 { style: "font-size:24px;font-weight:800;color:#b388ff;text-shadow:3px 3px 0 #000,0 0 10px rgba(179,136,255,0.5);letter-spacing:2px;",
                    "{sets_title}"
                }
                p { style: "font-size:13px;color:#888;margin-top:4px;", "{sets_desc}" }
            }

            div { style: "display:flex;gap:6px;padding:0 16px 12px;overflow-x:auto;",
                for filter in [MoodFilter::All, MoodFilter::Energy, MoodFilter::Party, MoodFilter::Heavy, MoodFilter::Beginner] {
                    {
                        let is_active = mood_filter() == filter;
                        let bg = if is_active { "rgba(179,136,255,0.15)" } else { "transparent" };
                        let color = if is_active { "#b388ff" } else { "#888" };
                        let border = if is_active { "#b388ff" } else { "#2a2a4a" };
                        let label = filter.label();
                        rsx! {
                            button {
                                style: "
                                    font-size:12px;font-weight:600;padding:8px 16px;
                                    background:{bg};color:{color};
                                    border:3px solid {border};border-radius:20px;
                                    cursor:pointer;white-space:nowrap;
                                ",
                                onclick: move |_| mood_filter.set(filter),
                                "{label}"
                            }
                        }
                    }
                }
            }

            div { style: "padding:0 16px 16px;",
                h2 { style: "font-size:13px;font-weight:700;color:#b388ff;text-transform:uppercase;letter-spacing:1px;margin-bottom:12px;text-shadow:2px 2px 0 #000;",
                    "Featured Packs"
                }
                div { style: "display:grid;grid-template-columns:repeat(3,1fr);gap:12px;",
                    div { style: "
                        background:#16213e;border:4px solid #2a2a4a;
                        box-shadow:4px 4px 0 #000;overflow:hidden;
                    ",
                        img { src: "{assets::packs::INDICA}", alt: "Indica Pack", style: "width:100%;aspect-ratio:1;object-fit:cover;" }
                        div { style: "padding:8px;text-align:center;color:#e8e8e8;font-size:13px;font-weight:600;", "Indica Pack" }
                    }
                    div { style: "
                        background:#16213e;border:4px solid #2a2a4a;
                        box-shadow:4px 4px 0 #000;overflow:hidden;
                    ",
                        img { src: "{assets::packs::SATIVA}", alt: "Sativa Pack", style: "width:100%;aspect-ratio:1;object-fit:cover;" }
                        div { style: "padding:8px;text-align:center;color:#e8e8e8;font-size:13px;font-weight:600;", "Sativa Pack" }
                    }
                    div { style: "
                        background:#16213e;border:4px solid #2a2a4a;
                        box-shadow:4px 4px 0 #000;overflow:hidden;
                    ",
                        img { src: "{assets::packs::STARTER}", alt: "Starter Pack", style: "width:100%;aspect-ratio:1;object-fit:cover;" }
                        div { style: "padding:8px;text-align:center;color:#e8e8e8;font-size:13px;font-weight:600;", "Starter Pack" }
                    }
                }
            }

            {
                match &*sets_resource.read() {
                    Some(Ok(sets)) if sets.is_empty() => rsx! {
                        div { style: "text-align:center;padding:48px 16px;",
                            div { style: "font-size:36px;margin-bottom:12px;", "📦" }
                            p { style: "font-size:15px;color:#888;", "No sets available yet" }
                        }
                    },
                    Some(Ok(_)) => rsx! {
                        div { style: "padding:0 16px 8px;",
                            h2 { style: "font-size:13px;font-weight:700;color:#b388ff;text-transform:uppercase;letter-spacing:1px;margin-bottom:12px;text-shadow:2px 2px 0 #000;",
                                "All Sets ({filtered_sets.len()})"
                            }
                        }
                        div { style: "display:flex;flex-direction:column;gap:12px;padding:0 16px;",
                            for set in filtered_sets.iter() {
                                { render_set_card(set.clone(), cart) }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { style: "text-align:center;padding:48px 16px;",
                            div { style: "font-size:36px;margin-bottom:12px;", "⚠️" }
                            p { style: "font-size:15px;color:#ff4757;", "Error: {e}" }
                        }
                    },
                    None => rsx! {
                        div { style: "display:flex;flex-direction:column;gap:12px;padding:0 16px;",
                            div { style: "
                                background:#16213e;border:4px solid #2a2a4a;
                                box-shadow:4px 4px 0 #000;
                                padding:20px;text-align:center;min-height:120px;
                            ",
                                div { style: "font-size:15px;color:#b388ff;margin-bottom:8px;", "Loading sets..." }
                            }
                        }
                    },
                }
            }

            BottomNav {}
        }
    }
}

fn render_set_card(set: ApiSet, mut cart: Signal<Cart>) -> Element {
    let add_to_cart = t(Lang::Russian, T_ADD_TO_CART).to_string();
    let mood = set.target_mood.as_deref().unwrap_or("party");
    let emoji = mood_emoji(mood);
    let m_color = mood_color(mood);
    let has_discount = set.discount_percent > 0.0;
    let discounted_price = if has_discount {
        set.total_price * (1.0 - set.discount_percent / 100.0)
    } else {
        set.total_price
    };
    let original_price_str = format!("฿{}", set.total_price as i32);
    let price_str = format!("฿{}", discounted_price as i32);
    let discount_badge = if has_discount {
        format!("{}% OFF", set.discount_percent as i32)
    } else {
        String::new()
    };
    let set_name = set.name.clone();
    let set_id = set.id.clone();
    let icon = set.icon.as_deref().unwrap_or("🎁");
    let desc = set.description.as_deref().unwrap_or("");
    let time_str = set.time_of_day.as_deref().unwrap_or("");
    let strains_count = set.strains.as_ref().map(|v| v.len()).unwrap_or(0);
    let strains_label = if strains_count > 0 { format!("{} strains", strains_count) } else { String::new() };
    let img_url = set.image_url.clone().unwrap_or_default();
    let has_image = !img_url.is_empty();
    let video_url = set.video_url.clone().unwrap_or_default();
    let has_video = !video_url.is_empty();
    let mut show_video = use_signal(|| false);

    rsx! {
        div { style: "
            background:#16213e;
            border:4px solid {m_color}33;
            box-shadow:4px 4px 0 #000;
            overflow:hidden;
            position:relative;
        ",
            div { style: "
                height:100px;
                background:linear-gradient(135deg,#1a1a2e,#16213e);
                display:flex;align-items:center;justify-content:center;
                font-size:40px;position:relative;overflow:hidden;
            ",
                if has_image {
                    img { src: "{img_url}", style: "width:100%;height:100%;object-fit:cover;position:absolute;inset:0;" }
                } else {
                    "{icon}"
                }
                if has_discount {
                    span { style: "
                        position:absolute;top:8px;right:8px;
                        font-size:13px;font-weight:700;background:{m_color};color:#000;
                        padding:4px 8px;box-shadow:2px 2px 0 #000;z-index:2;
                    ", "{discount_badge}" }
                }
                if has_video {
                    button { style: "position:absolute;bottom:8px;right:8px;width:32px;height:32px;border-radius:50%;background:rgba(0,0,0,0.6);border:1px solid #fff;color:#fff;font-size:14px;display:flex;align-items:center;justify-content:center;cursor:pointer;z-index:2;",
                        onclick: move |e: Event<MouseData>| { e.stop_propagation(); show_video.set(true); }, "▶️" }
                }
                if show_video() {
                    div { style: "position:fixed;inset:0;background:rgba(0,0,0,0.85);display:flex;align-items:center;justify-content:center;z-index:1000;padding:16px;",
                        onclick: move |_| show_video.set(false),
                        div { style: "background:#1a1a2e;padding:16px;border-radius:8px;max-width:90vw;max-height:80vh;display:flex;flex-direction:column;align-items:center;gap:8px;",
                            onclick: move |e: Event<MouseData>| e.stop_propagation(),
                            video { style: "max-width:100%;max-height:60vh;border-radius:6px;", controls: true, src: "{video_url}" }
                            button { style: "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;cursor:pointer;",
                                onclick: move |_| show_video.set(false), "Закрыть" }
                        }
                    }
                }
            }
            div { style: "padding:14px;",
                div { style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:4px;",
                    span { style: "font-size:16px;font-weight:700;text-shadow:2px 2px 0 #000;", "{set_name}" }
                    span { style: "font-size:13px;color:{m_color};", "{emoji} {mood}" }
                }
                if !desc.is_empty() {
                    div { style: "font-size:13px;color:#888;margin-bottom:6px;", "{desc}" }
                }
                if !strains_label.is_empty() || !time_str.is_empty() {
                    div { style: "font-size:13px;color:#888;margin-bottom:8px;",
                        if !strains_label.is_empty() {
                            span { style: "border:2px solid #2a2a4a;padding:2px 8px;margin-right:4px;", "{strains_label}" }
                        }
                        if !time_str.is_empty() {
                            span { style: "border:2px solid #2a2a4a;padding:2px 8px;", "🕐 {time_str}" }
                        }
                    }
                }
                div { style: "display:flex;justify-content:space-between;align-items:center;",
                    div {
                        if has_discount {
                            span { style: "font-size:13px;color:#888;text-decoration:line-through;margin-right:6px;", "{original_price_str}" }
                        }
                        span { style: "font-size:22px;font-weight:800;color:#ffe600;text-shadow:2px 2px 0 #000;", "{price_str}" }
                    }
                }
            }
            div { style: "padding:0 14px 14px;",
                button {
                    style: "
                        font-size:14px;font-weight:700;width:100%;padding:12px 20px;
                        background:#39ff14;color:#000;
                        border:4px solid #2d9e0f;
                        box-shadow:3px 3px 0 #000;
                        cursor:pointer;
                    ",
                    onclick: move |_| {
                        let mut c = cart.write();
                        c.add_item(CartItem {
                            id: set_id.clone(),
                            name: set_name.clone(),
                            price: discounted_price,
                            quantity: 1,
                            image_url: None,
                            item_type: CartItemType::Set,
                        });
                        crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                    },
                    "{add_to_cart}"
                }
            }
        }
    }
}
