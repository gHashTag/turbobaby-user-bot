use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::state::{Cart, CartItem, CartItemType};
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::api::context::api_base_url;
use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_SOMM_TITLE, T_SOMM_DESC, T_SOMM_MOOD, T_SOMM_TIME, T_SOMM_EXP};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mood {
    Relax,
    Energy,
    Creative,
    Sleep,
    Strong,
    Taste,
}

impl Mood {
    fn label(&self) -> &'static str {
        match self {
            Self::Relax => "😌 Relax",
            Self::Energy => "⚡ Energy",
            Self::Creative => "🎨 Creative",
            Self::Sleep => "😴 Sleep",
            Self::Strong => "💪 Strong",
            Self::Taste => "👅 Taste",
        }
    }
    fn as_str(&self) -> &'static str {
        match self {
            Self::Relax => "relax",
            Self::Energy => "energy",
            Self::Creative => "creative",
            Self::Sleep => "sleep",
            Self::Strong => "strong",
            Self::Taste => "taste",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TimeOfDay {
    Day,
    Evening,
    Any,
}

impl TimeOfDay {
    fn label(&self) -> &'static str {
        match self {
            Self::Day => "☀️ Day",
            Self::Evening => "🌙 Evening",
            Self::Any => "🔄 Any",
        }
    }
    fn as_str(&self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Evening => "evening",
            Self::Any => "any",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Experience {
    Beginner,
    Medium,
    Expert,
}

impl Experience {
    fn label(&self) -> &'static str {
        match self {
            Self::Beginner => "🌱 Beginner",
            Self::Medium => "🌿 Medium",
            Self::Expert => "🔥 Expert",
        }
    }
    fn as_str(&self) -> &'static str {
        match self {
            Self::Beginner => "beginner",
            Self::Medium => "medium",
            Self::Expert => "expert",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct RecommendedStrain {
    id: Option<String>,
    name: String,
    category: Option<String>,
    thc_percent: Option<f64>,
    effect: Option<String>,
    flavor_profile: Option<String>,
    price_per_gram: Option<f64>,
    image_url: Option<String>,
    match_reason: Option<String>,
    match_percent: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct RecommendedSet {
    id: Option<String>,
    name: String,
    icon: Option<String>,
    target_mood: Option<String>,
    total_price: Option<f64>,
    discount_percent: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SommelierResponse {
    recommended_sets: Option<Vec<RecommendedSet>>,
    recommended_strains: Option<Vec<RecommendedStrain>>,
}

fn category_emoji(cat: &str) -> &'static str {
    match cat.to_lowercase().as_str() {
        "sativa" => "☀️",
        "indica" => "🌙",
        "hybrid" => "⚖️",
        _ => "🌿",
    }
}

#[component]
pub fn SommelierScreen() -> Element {
    let mut cart = use_context::<Signal<Cart>>();
    let mut selected_mood = use_signal(|| Option::<Mood>::None);
    let mut selected_time = use_signal(|| TimeOfDay::Any);
    let mut selected_exp = use_signal(|| Experience::Medium);
    let mut show_results = use_signal(|| false);

    let somm_title = t(Lang::Russian, T_SOMM_TITLE);
    let somm_desc = t(Lang::Russian, T_SOMM_DESC);
    let somm_mood_label = format!("🤔 {}?", t(Lang::Russian, T_SOMM_MOOD));
    let somm_time_label = format!("🕐 {}?", t(Lang::Russian, T_SOMM_TIME));
    let somm_exp_label = format!("🎮 {}?", t(Lang::Russian, T_SOMM_EXP));

    let recommendations = use_resource(move || async move {
        if !show_results() { return Ok(SommelierResponse { recommended_sets: None, recommended_strains: None }); }
        let mood = selected_mood().map(|m| m.as_str()).unwrap_or("relax");
        let time = selected_time().as_str();
        let exp = selected_exp().as_str();
        let base = api_base_url();
        let url = format!("{}/api/sommelier/recommend?mood={}&time={}&experience={}", base, mood, time, exp);
        reqwest::Client::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<SommelierResponse>()
            .await
            .map_err(|e| e.to_string())
    });

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: 80px;
        ",
            div { style: "padding: 20px 16px 16px; text-align: center;",
                h1 { style: "font-size: 24px; font-weight: 800; color: #00e5ff; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(0,229,255,0.5); letter-spacing: 2px;", "{somm_title}" }
                p { style: "font-size: 13px; color: #8b8b9e; margin-top: 4px;", "{somm_desc}" }
            }

            if !show_results() {
                // Question mode
                div { style: "
                    margin: 0 16px 16px;
                    background: linear-gradient(135deg, rgba(0,229,255,0.08), rgba(57,255,20,0.08));
                    border: 4px solid #00e5ff;
                    border-radius: 0;
                    padding: 16px;
                    box-shadow: 0 0 16px rgba(0,229,255,0.1), 4px 4px 0 #000;
                ",
                    div { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 12px;", "{somm_mood_label}" }
                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 8px;",
                        for mood in [Mood::Relax, Mood::Energy, Mood::Creative, Mood::Sleep, Mood::Strong, Mood::Taste] {
                            {
                                let is_selected = selected_mood() == Some(mood);
                                let border = if is_selected { "#00e5ff" } else { "#2a2a4a" };
                                let bg = if is_selected { "rgba(0,229,255,0.15)" } else { "#16213e" };
                                let label = mood.label();
                                rsx! {
                                    button {
                                        style: "
                                            font-size: 13px; padding: 10px 8px;
                                            background: {bg}; color: #e8e8e8;
                                            border: 4px solid {border}; border-radius: 20px;
                                            cursor: pointer; text-align: center;
                                        ",
                                        onclick: move |_| selected_mood.set(Some(mood)),
                                        "{label}"
                                    }
                                }
                            }
                        }
                    }

                    div { style: "font-size: 13px; font-weight: 700; color: #ffe600; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin: 14px 0 10px;", "{somm_time_label}" }
                    div { style: "display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 6px;",
                        for time in [TimeOfDay::Day, TimeOfDay::Evening, TimeOfDay::Any] {
                            {
                                let is_selected = selected_time() == time;
                                let border = if is_selected { "#ffe600" } else { "#2a2a4a" };
                                let bg = if is_selected { "rgba(255,230,0,0.15)" } else { "#16213e" };
                                let label = time.label();
                                rsx! {
                                    button {
                                        style: "
                                            font-size: 13px; padding: 8px;
                                            background: {bg}; color: #e8e8e8;
                                            border: 4px solid {border}; border-radius: 20px;
                                            cursor: pointer; text-align: center;
                                        ",
                                        onclick: move |_| selected_time.set(time),
                                        "{label}"
                                    }
                                }
                            }
                        }
                    }

                    div { style: "font-size: 13px; font-weight: 700; color: #b388ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin: 14px 0 10px;", "{somm_exp_label}" }
                    div { style: "display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 6px;",
                        for exp in [Experience::Beginner, Experience::Medium, Experience::Expert] {
                            {
                                let is_selected = selected_exp() == exp;
                                let border = if is_selected { "#b388ff" } else { "#2a2a4a" };
                                let bg = if is_selected { "rgba(179,136,255,0.15)" } else { "#16213e" };
                                let label = exp.label();
                                rsx! {
                                    button {
                                        style: "
                                            font-size: 13px; padding: 8px;
                                            background: {bg}; color: #e8e8e8;
                                            border: 4px solid {border}; border-radius: 20px;
                                            cursor: pointer; text-align: center;
                                        ",
                                        onclick: move |_| selected_exp.set(exp),
                                        "{label}"
                                    }
                                }
                            }
                        }
                    }

                    div { style: "margin-top: 16px;",
                        button {
                            style: "
                                font-size: 14px; font-weight: 700; width: 100%; padding: 12px 20px;
                                background: #39ff14; color: #000;
                                border: 4px solid #2d9e0f; border-radius: 0;
                                cursor: pointer;
                                box-shadow: 3px 3px 0 #000;
                                transition: transform 0.1s, box-shadow 0.1s;
                            ",
                            onclick: move |_| show_results.set(true),
                            "🔮 Get Recommendations"
                        }
                    }
                }
            } else {
                // Results mode
                div { style: "display: flex; justify-content: space-between; align-items: center; padding: 0 16px 12px;",
                    div { style: "font-size: 13px; font-weight: 700; color: #39ff14; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000;", "✨ Recommended for you" }
                    button {
                        style: "
                            font-size: 13px; padding: 4px 8px;
                            background: transparent; color: #ff4757;
                            border: 4px solid #ff4757; border-radius: 0; cursor: pointer;
                            box-shadow: 3px 3px 0 #000;
                        ",
                        onclick: move |_| {
                            show_results.set(false);
                        },
                        "↻ Restart"
                    }
                }

                {
                    match &*recommendations.read() {
                        Some(Ok(resp)) => {
                            let mut elements = Vec::new();

                            if let Some(sets) = &resp.recommended_sets {
                                if !sets.is_empty() {
                                    elements.push(rsx! {
                                        div { style: "font-size: 13px; font-weight: 700; color: #b388ff; padding: 0 16px 8px; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000;", "📦 Recommended Sets" }
                                    });
                                    for set in sets {
                                        let name = set.name.clone();
                                        let icon = set.icon.clone().unwrap_or("🎁".to_string());
                                        let mood = set.target_mood.clone().unwrap_or_default();
                                        let price = set.total_price.unwrap_or(0.0);
                                        let discount = set.discount_percent.unwrap_or(0.0);
                                        let final_price = if discount > 0.0 { price * (1.0 - discount / 100.0) } else { price };
                                        let price_str = format!("฿{}", final_price as i32);

                                        elements.push(rsx! {
                                            div { style: "
                                                background: #16213e; border: 4px solid #b388ff33;
                                                border-radius: 0; padding: 12px; margin: 0 16px 8px;
                                                display: flex; gap: 12px; align-items: center;
                                                box-shadow: 4px 4px 0 #000;
                                            ",
                                                div { style: "font-size: 28px; min-width: 40px; text-align: center;", "{icon}" }
                                                div { style: "flex: 1;",
                                                    div { style: "font-size: 17px; font-weight: 700; margin-bottom: 4px;", "{name}" }
                                                    div { style: "font-size: 13px; color: #b388ff; margin-bottom: 6px;", "{mood}" }
                                                    span { style: "font-size: 20px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000;", "{price_str}" }
                                                }
                                            }
                                        });
                                    }
                                }
                            }

                            if let Some(strains) = &resp.recommended_strains {
                                if !strains.is_empty() {
                                    elements.push(rsx! {
                                        div { style: "font-size: 13px; font-weight: 700; color: #39ff14; padding: 6px 10px 8px; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000;", "🌿 Recommended Strains" }
                                    });
                                    for strain in strains {
                                        let s = strain.clone();
                                        let cat = s.category.as_deref().unwrap_or("Hybrid");
                                        let emoji = category_emoji(cat);
                                        let thc_str = s.thc_percent.map(|t| format!("{}% THC", t as i32)).unwrap_or_default();
                                        let match_pct = s.match_percent.unwrap_or(80);
                                        let match_color = if match_pct >= 90 { "#39ff14" } else if match_pct >= 80 { "#00e5ff" } else { "#ffe600" };
                                        let reason = s.match_reason.clone().unwrap_or_default();
                                        let price = s.price_per_gram.unwrap_or(0.0);
                                        let price_str = format!("฿{}", price as i32);
                                        let s_name = s.name.clone();
                                        let s_id = s.id.clone().unwrap_or_default();
                                        let s_price = price;

                                        elements.push(rsx! {
                                            div { style: "
                                                background: #16213e; border: 4px solid #2a2a4a;
                                                border-radius: 0; padding: 12px; margin: 0 16px 8px;
                                                display: flex; gap: 12px; align-items: center;
                                                box-shadow: 4px 4px 0 #000;
                                            ",
                                                div { style: "font-size: 28px; min-width: 40px; text-align: center;", "{emoji}" }
                                                div { style: "flex: 1;",
                                                    div { style: "font-size: 17px; font-weight: 700; margin-bottom: 4px;", "{s_name}" }
                                                    div { style: "font-size: 13px; color: #00e5ff; margin-bottom: 2px;", "{cat} • {thc_str}" }
                                                    if !reason.is_empty() {
                                                        div { style: "font-size: 13px; color: #8b8b9e; margin-bottom: 6px;", "{reason}" }
                                                    }
                                                    div { style: "display: flex; justify-content: space-between; align-items: center;",
                                                        span { style: "font-size: 20px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000;", "{price_str}" }
                                                        span { style: "font-size: 13px; color: {match_color}; background: {match_color}22; padding: 2px 6px; border-radius: 0;", "{match_pct}% Match" }
                                                    }
                                                }
                                                if !s_id.is_empty() && s_price > 0.0 {
                                                    button {
                                                        style: "
                                                            font-size: 14px; font-weight: 700; padding: 6px 8px;
                                                            background: #39ff14; color: #000;
                                                            border: 4px solid #2d9e0f; border-radius: 0; cursor: pointer;
                                                            box-shadow: 3px 3px 0 #000;
                                                        ",
                                                        onclick: move |_| {
                                                            let mut c = cart.write();
                                                            c.add_item(CartItem {
                                                                id: s_id.clone(),
                                                                name: s_name.clone(),
                                                                price: s_price,
                                                                quantity: 1,
                                                                image_url: None,
                                                                item_type: CartItemType::Strain,
                                                            });
                                                            crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                                                        },
                                                        "+🛒"
                                                    }
                                                }
                                            }
                                        });
                                    }
                                }
                            }

                            if elements.is_empty() {
                                rsx! {
                                    div { style: "text-align: center; padding: 40px 16px;",
                                        div { style: "font-size: 70px; margin-bottom: 12px;", "🍷" }
                                        p { style: "font-size: 13px; color: #8b8b9e;", "No recommendations found. Try different preferences!" }
                                    }
                                }
                            } else {
                                rsx! { for el in elements { {el} } }
                            }
                        },
                        Some(Err(e)) => rsx! {
                            div { style: "text-align: center; padding: 40px 16px;",
                                div { style: "font-size: 70px; margin-bottom: 12px;", "⚠️" }
                                p { style: "font-size: 13px; color: #ff4757;", "Error: {e}" }
                            }
                        },
                        None => rsx! {
                            div { style: "text-align: center; padding: 40px 16px;",
                                div { style: "font-size: 70px; margin-bottom: 8px;", "🔮" }
                                p { style: "font-size: 13px; color: #8b8b9e;", "Consulting the sommelier..." }
                            }
                        },
                    }
                }
            }

            BottomNav {}
        }
    }
}
