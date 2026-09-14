use crate::trios::core::Lang;
use crate::trios::i18n::{
    t, tf, T_ADD_TO_CART, T_SOMM_DESC, T_SOMM_EXP, T_SOMM_EXP_BEGINNER, T_SOMM_EXP_EXPERT,
    T_SOMM_EXP_MEDIUM, T_SOMM_GET_RECOMMENDATIONS, T_SOMM_MATCH, T_SOMM_MOOD, T_SOMM_MOOD_CREATIVE,
    T_SOMM_MOOD_ENERGY, T_SOMM_MOOD_RELAX, T_SOMM_MOOD_SLEEP, T_SOMM_MOOD_STRONG,
    T_SOMM_MOOD_TASTE, T_SOMM_NO_RECOMMENDATIONS, T_SOMM_REASON_CREATIVE, T_SOMM_REASON_DEFAULT,
    T_SOMM_REASON_FLAVOR, T_SOMM_REASON_HYBRID_MELLOW, T_SOMM_REASON_HYBRID_UP,
    T_SOMM_REASON_INDICA_RELAX, T_SOMM_REASON_SATIVA_ENERGY, T_SOMM_REASON_SLEEP,
    T_SOMM_REASON_THC, T_SOMM_RECOMMENDED_FOR_YOU, T_SOMM_RECOMMENDED_SETS,
    T_SOMM_RECOMMENDED_STRAINS, T_SOMM_RESTART, T_SOMM_TIME, T_SOMM_TIME_ANY, T_SOMM_TIME_DAY,
    T_SOMM_TIME_EVENING, T_SOMM_TITLE,
};
use crate::ui::api::context::api_base_url;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::skeleton::{Skeleton, SkeletonShape};
use crate::ui::state::{Cart, CartItem, CartItemType};
use dioxus::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mood {
    Relax,
    Energy,
    Creative,
    Sleep,
    Strong,
    Taste,
}

fn mood_label(lang: Lang, mood: Mood) -> String {
    let key = match mood {
        Mood::Relax => T_SOMM_MOOD_RELAX,
        Mood::Energy => T_SOMM_MOOD_ENERGY,
        Mood::Creative => T_SOMM_MOOD_CREATIVE,
        Mood::Sleep => T_SOMM_MOOD_SLEEP,
        Mood::Strong => T_SOMM_MOOD_STRONG,
        Mood::Taste => T_SOMM_MOOD_TASTE,
    };
    t(lang, key).to_string()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TimeOfDay {
    Day,
    Evening,
    Any,
}

fn time_label(lang: Lang, time: TimeOfDay) -> String {
    let key = match time {
        TimeOfDay::Day => T_SOMM_TIME_DAY,
        TimeOfDay::Evening => T_SOMM_TIME_EVENING,
        TimeOfDay::Any => T_SOMM_TIME_ANY,
    };
    t(lang, key).to_string()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Experience {
    Beginner,
    Medium,
    Expert,
}

fn exp_label(lang: Lang, exp: Experience) -> String {
    let key = match exp {
        Experience::Beginner => T_SOMM_EXP_BEGINNER,
        Experience::Medium => T_SOMM_EXP_MEDIUM,
        Experience::Expert => T_SOMM_EXP_EXPERT,
    };
    t(lang, key).to_string()
}

#[derive(Debug, Clone, Deserialize)]
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

/// A strain row from `GET /api/strains`, the input to the rule-based recommender.
#[derive(Debug, Clone, Deserialize)]
struct CatalogStrain {
    #[serde(default)]
    id: Option<String>,
    name: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    thc_percent: Option<f64>,
    #[serde(default)]
    effect: Option<String>,
    #[serde(default)]
    flavor_profile: Option<String>,
    #[serde(default)]
    price_per_gram: Option<f64>,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    is_available: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct StrainsCatalog {
    strains: Vec<CatalogStrain>,
}

/// Rule-based match score (35..=99) + a short reason for a strain given the
/// quiz answers. The AI sommelier (GROK/GLM) is config-gated and off in prod, so
/// this local heuristic makes the section actually recommend real catalog strains
/// instead of always returning "no recommendations". Heuristics: mood → category
/// + effect keywords, time → sativa(day)/indica(evening), experience → THC band.
fn score_strain(
    lang: Lang,
    s: &CatalogStrain,
    mood: Option<Mood>,
    time: TimeOfDay,
    exp: Experience,
) -> (i32, String) {
    let cat = s.category.as_deref().unwrap_or("").to_lowercase();
    let thc = s.thc_percent.unwrap_or(0.0);
    let eff = s.effect.as_deref().unwrap_or("").to_lowercase();
    let has_flavor = s
        .flavor_profile
        .as_deref()
        .map(|f| !f.trim().is_empty())
        .unwrap_or(false);
    let is_sativa = cat.contains("sativa");
    let is_indica = cat.contains("indica");
    let is_hybrid = cat.contains("hybrid");

    let mut score = 50i32;
    let mut reason = String::new();
    if let Some(m) = mood {
        match m {
            Mood::Relax => {
                if is_indica {
                    score += 25;
                    reason = t(lang, T_SOMM_REASON_INDICA_RELAX).to_string();
                } else if is_hybrid {
                    score += 12;
                    reason = t(lang, T_SOMM_REASON_HYBRID_MELLOW).to_string();
                }
                if eff.contains("relax") || eff.contains("calm") || eff.contains("chill") {
                    score += 15;
                }
            }
            Mood::Energy => {
                if is_sativa {
                    score += 25;
                    reason = t(lang, T_SOMM_REASON_SATIVA_ENERGY).to_string();
                } else if is_hybrid {
                    score += 12;
                    reason = t(lang, T_SOMM_REASON_HYBRID_UP).to_string();
                }
                if eff.contains("energ") || eff.contains("uplift") || eff.contains("focus") {
                    score += 15;
                }
            }
            Mood::Creative => {
                if is_sativa || is_hybrid {
                    score += 18;
                    reason = t(lang, T_SOMM_REASON_CREATIVE).to_string();
                }
                if eff.contains("creativ") || eff.contains("focus") || eff.contains("euphor") {
                    score += 18;
                }
            }
            Mood::Sleep => {
                if is_indica {
                    score += 25;
                    reason = t(lang, T_SOMM_REASON_SLEEP).to_string();
                }
                if thc >= 22.0 {
                    score += 10;
                }
                if eff.contains("sleep") || eff.contains("sedat") || eff.contains("relax") {
                    score += 12;
                }
            }
            Mood::Strong => {
                let bonus = ((thc - 18.0).max(0.0) as i32).min(35);
                score += bonus;
                reason = tf(lang, T_SOMM_REASON_THC, &[(thc as i32).to_string()]);
            }
            Mood::Taste => {
                if has_flavor {
                    score += 22;
                    reason = t(lang, T_SOMM_REASON_FLAVOR).to_string();
                }
                score += 8;
            }
        }
    }

    match time {
        TimeOfDay::Day => {
            if is_sativa {
                score += 10;
            }
        }
        TimeOfDay::Evening => {
            if is_indica {
                score += 10;
            }
        }
        TimeOfDay::Any => {}
    }

    match exp {
        Experience::Beginner => {
            if thc <= 20.0 {
                score += 12;
            } else {
                score -= 12;
            }
        }
        Experience::Medium => {}
        Experience::Expert => {
            if thc >= 24.0 {
                score += 10;
            }
        }
    }

    if reason.is_empty() {
        reason = t(lang, T_SOMM_REASON_DEFAULT).to_string();
    }
    (score.clamp(35, 99), reason)
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

    let lang = crate::ui::lang::current_lang();
    let somm_title = t(lang, T_SOMM_TITLE);
    let somm_desc = t(lang, T_SOMM_DESC);
    let somm_mood_label = format!("🤔 {}?", t(lang, T_SOMM_MOOD));
    let somm_time_label = format!("🕐 {}?", t(lang, T_SOMM_TIME));
    let somm_exp_label = format!("🎮 {}?", t(lang, T_SOMM_EXP));

    let recommendations = use_resource(move || async move {
        if !show_results() {
            return Ok(SommelierResponse {
                recommended_sets: None,
                recommended_strains: None,
            });
        }
        // There is no backend /api/sommelier/recommend — the AI sommelier
        // (GROK/GLM) is config-gated and off in prod. Recommend client-side:
        // pull the live catalog and rank it by the quiz answers so the section
        // returns real strains instead of always "no recommendations".
        let mood = selected_mood();
        let time = selected_time();
        let exp = selected_exp();
        let url = format!("{}/api/strains", api_base_url());
        let catalog = crate::ui::api::local_client::LocalClient::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<StrainsCatalog>()
            .await
            .map_err(|e| e.to_string())?;
        let mut scored: Vec<(i32, RecommendedStrain)> = catalog
            .strains
            .into_iter()
            .filter(|s| s.is_available.unwrap_or(true))
            .map(|s| {
                let (score, reason) = score_strain(lang, &s, mood, time, exp);
                (
                    score,
                    RecommendedStrain {
                        id: s.id,
                        name: s.name,
                        category: s.category,
                        thc_percent: s.thc_percent,
                        effect: s.effect,
                        flavor_profile: s.flavor_profile,
                        price_per_gram: s.price_per_gram,
                        image_url: s.image_url,
                        match_reason: Some(reason),
                        match_percent: Some(score),
                    },
                )
            })
            .collect();
        scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        let top: Vec<RecommendedStrain> = scored.into_iter().take(6).map(|(_, r)| r).collect();
        Ok::<SommelierResponse, String>(SommelierResponse {
            recommended_sets: None,
            recommended_strains: if top.is_empty() { None } else { Some(top) },
        })
    });

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: calc(96px + env(safe-area-inset-bottom));
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
                                let label = mood_label(lang, mood);
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
                                let label = time_label(lang, time);
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
                                let label = exp_label(lang, exp);
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
                            "{t(lang, T_SOMM_GET_RECOMMENDATIONS)}"
                        }
                    }
                }
            } else {
                // Results mode
                div { style: "display: flex; justify-content: space-between; align-items: center; padding: 0 16px 12px;",
                    div { style: "font-size: 13px; font-weight: 700; color: #39ff14; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000;", "{t(lang, T_SOMM_RECOMMENDED_FOR_YOU)}" }
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
                        "{t(lang, T_SOMM_RESTART)}"
                    }
                }

                {
                    match &*recommendations.read() {
                        Some(Ok(resp)) => {
                            let mut elements = Vec::new();

                            if let Some(sets) = &resp.recommended_sets {
                                if !sets.is_empty() {
                                    elements.push(rsx! {
                                        div { style: "font-size: 13px; font-weight: 700; color: #b388ff; padding: 0 16px 8px; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000;", "{t(lang, T_SOMM_RECOMMENDED_SETS)}" }
                                    });
                                    for set in sets {
                                        let name = set.name.clone();
                                        let icon = set.icon.clone().unwrap_or("🎁".to_string());
                                        let mood = set.target_mood.clone().unwrap_or_default();
                                        let price = set.total_price.filter(|v| v.is_finite()).unwrap_or(0.0).max(0.0);
                                        let discount = set.discount_percent.filter(|v| v.is_finite()).unwrap_or(0.0).max(0.0);
                                        let final_price = if discount > 0.0 { (price * (1.0 - discount / 100.0)).max(0.0) } else { price };
                                        let price_str = crate::trios::pricing::format_baht(final_price);
                                        let set_id = set.id.clone().unwrap_or_default();
                                        let set_name = name.clone();

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
                                                if !set_id.is_empty() && final_price > 0.0 {
                                                    button {
                                                        style: "
                                                            font-size: 14px; font-weight: 700; padding: 6px 8px;
                                                            background: #39ff14; color: #000;
                                                            border: 4px solid #2d9e0f; border-radius: 0; cursor: pointer;
                                                            box-shadow: 3px 3px 0 #000;
                                                        ",
                                                        "aria-label": "{t(lang, T_ADD_TO_CART)}",
                                                        onclick: move |_| {
                                                            let mut c = cart.write();
                                                            c.add_item(CartItem {
                                                                id: set_id.clone(),
                                                                name: set_name.clone(),
                                                                price: final_price,
                                                                quantity: 1,
                                                                image_url: None,
                                                                item_type: CartItemType::Set,
                                                                fulfillment: None,
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

                            if let Some(strains) = &resp.recommended_strains {
                                if !strains.is_empty() {
                                    elements.push(rsx! {
                                        div { style: "font-size: 13px; font-weight: 700; color: #39ff14; padding: 6px 10px 8px; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000;", "{t(lang, T_SOMM_RECOMMENDED_STRAINS)}" }
                                    });
                                    for strain in strains {
                                        let s = strain.clone();
                                        let cat = s.category.as_deref().unwrap_or("Hybrid");
                                        let emoji = category_emoji(cat);
                                        let thc_str = s.thc_percent.map(|t| format!("{}% THC", t as i32)).unwrap_or_default();
                                        let effect = s.effect.clone().unwrap_or_default();
                                        let flavor = s.flavor_profile.clone().unwrap_or_default();
                                        let img = s.image_url.clone().unwrap_or_default();
                                        let has_img = !img.is_empty()
                                            && (img.starts_with("http://") || img.starts_with("https://")
                                                || (img.starts_with('/') && !img.starts_with("//")));
                                        let match_pct = s.match_percent.unwrap_or(80);
                                        let match_color = if match_pct >= 90 { "#39ff14" } else if match_pct >= 80 { "#00e5ff" } else { "#ffe600" };
                                        let match_label = format!("{}% {}", match_pct, t(lang, T_SOMM_MATCH));
                                        let reason = s.match_reason.clone().unwrap_or_default();
                                        let price = s.price_per_gram.filter(|v| v.is_finite()).unwrap_or(0.0).max(0.0);
                                        let price_str = crate::trios::pricing::format_baht(price);
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
                                                if has_img {
                                                    img {
                                                        src: "{img}",
                                                        alt: "{s_name}",
                                                        loading: "lazy",
                                                        style: "width: 48px; height: 48px; min-width: 48px; object-fit: cover; border: 2px solid #2a2a4a;"
                                                    }
                                                } else {
                                                    div { style: "font-size: 28px; min-width: 40px; text-align: center;", "{emoji}" }
                                                }
                                                div { style: "flex: 1;",
                                                    div { style: "font-size: 17px; font-weight: 700; margin-bottom: 4px;", "{s_name}" }
                                                    div { style: "font-size: 13px; color: #00e5ff; margin-bottom: 2px;", "{cat} • {thc_str}" }
                                                    if !effect.is_empty() {
                                                        div { style: "font-size: 13px; color: #aaa; margin-bottom: 2px;", "{effect}" }
                                                    }
                                                    if !flavor.is_empty() {
                                                        div { style: "font-size: 13px; color: #888; margin-bottom: 2px;", "🍃 {flavor}" }
                                                    }
                                                    if !reason.is_empty() {
                                                        div { style: "font-size: 13px; color: #8b8b9e; margin-bottom: 6px;", "{reason}" }
                                                    }
                                                    div { style: "display: flex; justify-content: space-between; align-items: center;",
                                                        span { style: "font-size: 20px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000;", "{price_str}" }
                                                        span { style: "font-size: 13px; color: {match_color}; background: {match_color}22; padding: 2px 6px; border-radius: 0;", "{match_label}" }
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
                                                        "aria-label": "{t(lang, T_ADD_TO_CART)}",
                                                        onclick: move |_| {
                                                            let mut c = cart.write();
                                                            c.add_item(CartItem {
                                                                id: s_id.clone(),
                                                                name: s_name.clone(),
                                                                price: s_price,
                                                                quantity: 1,
                                                                image_url: None,
                                                                item_type: CartItemType::Strain,
                                                                fulfillment: None,
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
                                        p { style: "font-size: 13px; color: #8b8b9e;", "{t(lang, T_SOMM_NO_RECOMMENDATIONS)}" }
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
                            div { style: "padding:0 16px;display:flex;flex-direction:column;gap:12px;",
                                Skeleton { shape: SkeletonShape::Card }
                                Skeleton { shape: SkeletonShape::Card }
                                Skeleton { shape: SkeletonShape::Card }
                            }
                        },
                    }
                }
            }

            BottomNav {}
        }
    }
}
