use crate::trios::garden::{calculate_progress, GrowthStage, Plant};
use crate::trios::i18n::{
    t, tf, T_BTN_WATER, T_CLOSE, T_GARDEN_ACHIEVEMENTS_EMPTY, T_GARDEN_ACHIEVEMENTS_NEW,
    T_GARDEN_ACHIEVEMENTS_TITLE, T_GARDEN_CANCEL, T_GARDEN_CAT_ACCESSORY,
    T_GARDEN_CAT_ACCESSORY_SET, T_GARDEN_CAT_OTHER, T_GARDEN_CAT_SET, T_GARDEN_CAT_STRAIN,
    T_GARDEN_CAT_TEA, T_GARDEN_CAT_TEA_SET, T_GARDEN_CHANGE_PRODUCT, T_GARDEN_CHOOSER_EMPTY,
    T_GARDEN_CHOOSER_ERROR, T_GARDEN_CHOOSER_LOADING, T_GARDEN_CHOOSER_TITLE,
    T_GARDEN_CHOOSE_PRODUCT, T_GARDEN_CHOOSE_PRODUCT_HINT, T_GARDEN_CONFIRM_RESET,
    T_GARDEN_COOLDOWN, T_GARDEN_DIAGNOSTICS_COPIED, T_GARDEN_DIAGNOSTICS_COPY,
    T_GARDEN_DISCOUNT_BADGE, T_GARDEN_EMPTY_CTA, T_GARDEN_EMPTY_LABEL, T_GARDEN_ERROR_COOLDOWN,
    T_GARDEN_ERROR_HARVEST, T_GARDEN_ERROR_PRODUCT_UNAVAILABLE, T_GARDEN_ERROR_RESET,
    T_GARDEN_HARVEST, T_GARDEN_LEADERBOARD_RANK, T_GARDEN_LEADERBOARD_TAB_HARVEST,
    T_GARDEN_LEADERBOARD_TAB_STREAK, T_GARDEN_LEADERBOARD_TITLE, T_GARDEN_LEADERBOARD_YOU,
    T_GARDEN_LOADING, T_GARDEN_NEXT_WATER_IN, T_GARDEN_PLANT_ALT, T_GARDEN_PRODUCT_ALT,
    T_GARDEN_READY, T_GARDEN_RESET_CONFIRM_BODY, T_GARDEN_RESET_CONFIRM_TITLE,
    T_GARDEN_RESET_PROGRESS, T_GARDEN_REWARD_EXPIRES_IN, T_GARDEN_SHARE_CTA,
    T_GARDEN_STREAK_BEST, T_GARDEN_STREAK_DAYS, T_GARDEN_SUBTITLE, T_GARDEN_TITLE,
    T_GARDEN_WATER_NOW,
};
use crate::ui::share::share_garden;
use crate::ui::api::context::api_base_url;
use crate::ui::api::http::post_client_event;
use crate::ui::components::ErrorBanner;
use crate::ui::telegram::{use_telegram, use_telegram_id, use_telegram_init_data};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

#[derive(Debug, Clone, serde::Deserialize)]
struct ApiPlant {
    id: String,
    strain_id: String,
    strain_name: String,
    current_stage: GrowthStage,
    water_count: u32,
    is_completed: bool,
    planted_at: i64,
    #[serde(default)]
    last_watered_at: Option<i64>,
    reward_claimed: bool,
    #[serde(default)]
    streak: u32,
    #[serde(default)]
    max_streak: u32,
    // B3: chosen target product (seed = its photo).
    #[serde(default)]
    target_name: Option<String>,
    #[serde(default)]
    target_image_url: Option<String>,
}

impl From<&ApiPlant> for Plant {
    fn from(a: &ApiPlant) -> Self {
        Plant {
            id: a.id.clone(),
            user_id: String::new(),
            strain_id: a.strain_id.clone(),
            strain_name: a.strain_name.clone(),
            current_stage: a.current_stage,
            planted_at: a.planted_at,
            is_completed: a.is_completed,
            harvested_at: None,
            reward_claimed: a.reward_claimed,
            water_count: a.water_count,
            last_watered_at: a.last_watered_at,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct GardenProduct {
    catalog: String,
    id: String,
    name: String,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    price: f64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct GardenProductsResponse {
    products: Vec<GardenProduct>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct GardenResponse {
    plants: Vec<ApiPlant>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct GardenStreakResponse {
    has_plant: bool,
    #[serde(default)]
    plant_id: String,
    #[serde(default)]
    strain_name: String,
    #[serde(default)]
    streak: u32,
    #[serde(default)]
    max_streak: u32,
    #[serde(default)]
    next_water_at: i64,
    #[serde(default)]
    is_ready_to_harvest: bool,
    #[serde(default)]
    reward_expires_at: Option<i64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct WaterPlantResponse {
    success: bool,
    #[serde(default)]
    water_count: u32,
    #[serde(default)]
    current_stage: Option<String>,
    #[serde(default)]
    is_completed: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    next_water_at: Option<i64>,
    #[serde(default)]
    streak: u32,
    #[serde(default)]
    max_streak: u32,
    #[serde(default)]
    new_achievements: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ResetPlantResponse {
    success: bool,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct HarvestPlantResponse {
    success: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    new_achievements: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct Achievement {
    id: String,
    name: String,
    description: String,
    icon: String,
    xp_reward: i32,
    requirement: String,
    category: String,
    #[serde(default)]
    unlocked_at: Option<i64>,
    #[serde(default)]
    notified: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct AchievementsResponse {
    achievements: Vec<Achievement>,
    #[serde(default)]
    unnotified: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct LeaderboardEntry {
    rank: i64,
    display_name: String,
    score: i64,
    #[serde(default)]
    is_you: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct LeaderboardResponse {
    kind: String,
    entries: Vec<LeaderboardEntry>,
    #[serde(default)]
    user: Option<serde_json::Value>,
}

/// Fetch the user's plants. On failure the `Err` carries the HTTP status so the
/// caller can distinguish an auth failure (`401` → stale/empty Telegram
/// initData, worth a one-time re-auth retry) from other errors. A network or
/// parse failure reports status `0`.
async fn fetch_plants(telegram_id: i64, init_data: &str) -> Result<Vec<ApiPlant>, u16> {
    let base = api_base_url();
    let url = format!("{}/api/garden/plants?telegram_id={}", base, telegram_id);
    let (status, body) = crate::ui::api::http::fetch_text_authed_full(&url, init_data)
        .await
        .map_err(|_| 0u16)?;
    if !(200..300).contains(&status) {
        return Err(status);
    }
    serde_json::from_str::<GardenResponse>(&body)
        .map(|r| r.plants)
        .map_err(|_| 0u16)
}

async fn fetch_garden_streak(telegram_id: i64, init_data: &str) -> Option<GardenStreakResponse> {
    let base = api_base_url();
    let url = format!("{}/api/garden/streak?telegram_id={}", base, telegram_id);
    let (status, body) = crate::ui::api::http::fetch_text_authed_full(&url, init_data)
        .await
        .ok()?;
    if !(200..300).contains(&status) {
        return None;
    }
    serde_json::from_str::<GardenStreakResponse>(&body).ok()
}

/// Ask the backend WHY it rejected this initData via the debug endpoint added on
/// main (`GET /api/debug/validate-init-data`). Returns a "\nserver=…" suffix for
/// the error banner, or an empty string on failure. The endpoint never echoes
/// the bot token or full initData — only a machine-readable reason.
async fn fetch_validation_reason(init_data: &str) -> String {
    let base = api_base_url();
    let url = format!("{}/api/debug/validate-init-data", base);
    match crate::ui::api::http::fetch_text_authed(&url, init_data).await {
        Ok(text) => format!("\nserver={}", text.chars().take(300).collect::<String>()),
        Err(_) => String::new(),
    }
}

async fn fetch_garden_products() -> Result<Vec<GardenProduct>, String> {
    let base = api_base_url();
    let url = format!("{}/api/garden/products", base);
    let text = crate::ui::api::http::fetch_text(&url).await?;
    serde_json::from_str::<GardenProductsResponse>(&text)
        .map_err(|e| format!("Parse error: {e}"))
        .map(|r| r.products)
}

async fn fetch_achievements(
    telegram_id: i64,
    init_data: &str,
) -> Result<AchievementsResponse, String> {
    let base = api_base_url();
    let url = format!("{}/api/garden/achievements?telegram_id={}", base, telegram_id);
    let text = crate::ui::api::http::fetch_text_authed(&url, init_data).await?;
    serde_json::from_str::<AchievementsResponse>(&text).map_err(|e| format!("Parse error: {e}"))
}

async fn fetch_leaderboard(
    telegram_id: i64,
    kind: &str,
    init_data: &str,
) -> Result<LeaderboardResponse, String> {
    let base = api_base_url();
    let url = format!(
        "{}/api/garden/leaderboard?telegram_id={}&kind={}",
        base,
        telegram_id,
        urlencoding::encode(kind)
    );
    let text = crate::ui::api::http::fetch_text_authed(&url, init_data).await?;
    serde_json::from_str::<LeaderboardResponse>(&text).map_err(|e| format!("Parse error: {e}"))
}

async fn mark_achievements_notified(telegram_id: i64, init_data: &str) {
    let base = api_base_url();
    let url = format!("{}/api/garden/achievements/notified", base);
    let body = serde_json::json!({ "telegram_id": telegram_id }).to_string();
    let _ = crate::ui::api::http::post_json_authed(&url, init_data, &body).await;
}

async fn log_share_event(
    telegram_id: i64,
    init_data: &str,
    content_kind: &str,
    content_id: &str,
) {
    let base = api_base_url();
    let url = format!("{}/api/garden/share-events", base);
    let body = serde_json::json!({
        "telegram_id": telegram_id,
        "channel": "telegram_chat",
        "content_kind": content_kind,
        "content_id": content_id,
    })
    .to_string();
    let _ = crate::ui::api::http::post_json_authed(&url, init_data, &body).await;
}

/// Copy text to the browser clipboard. Returns true on apparent success.
fn copy_to_clipboard(text: &str) -> bool {
    let js = format!(
        r#"(function(){{
            try {{
                navigator.clipboard.writeText({});
                return true;
            }} catch(e) {{
                return false;
            }}
        }})()"#,
        serde_json::Value::String(text.to_string())
    );
    js_sys::eval(&js)
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// POST a chosen product to plant it. Returns Ok(()) or a server error code.
async fn choose_plant_api(
    telegram_id: i64,
    catalog: &str,
    product_id: &str,
    init_data: &str,
) -> Result<(), String> {
    let base = api_base_url();
    let url = format!("{}/api/garden/plants/choose", base);
    let body = serde_json::json!({
        "telegram_id": telegram_id,
        "catalog": catalog,
        "product_id": product_id,
    })
    .to_string();
    let text = crate::ui::api::http::post_json_authed(&url, init_data, &body).await?;
    let resp: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Parse error: {e}"))?;
    if resp
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err(resp
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("choose_failed")
            .to_string())
    }
}

async fn water_plant_api(plant_id: &str, init_data: &str) -> Result<WaterPlantResponse, String> {
    let base = api_base_url();
    let url = format!(
        "{}/api/garden/plants/{}/water",
        base,
        urlencoding::encode(plant_id)
    );
    let text = crate::ui::api::http::post_json_authed(&url, init_data, "").await?;
    serde_json::from_str::<WaterPlantResponse>(&text).map_err(|e| format!("Parse error: {e}"))
}

async fn harvest_plant_api(plant_id: &str, init_data: &str) -> Result<HarvestPlantResponse, String> {
    let base = api_base_url();
    let url = format!(
        "{}/api/garden/plants/{}/harvest",
        base,
        urlencoding::encode(plant_id)
    );
    let text = crate::ui::api::http::post_json_authed(&url, init_data, "").await?;
    let resp: HarvestPlantResponse =
        serde_json::from_str(&text).map_err(|e| format!("Parse error: {e}"))?;
    if resp.success {
        Ok(resp)
    } else {
        Err(resp.error.unwrap_or_else(|| "Harvest failed".into()))
    }
}

async fn reset_plant_api(plant_id: &str, init_data: &str) -> Result<(), String> {
    let base = api_base_url();
    let url = format!(
        "{}/api/garden/plants/{}/reset",
        base,
        urlencoding::encode(plant_id)
    );
    let text = crate::ui::api::http::post_json_authed(&url, init_data, "").await?;
    let resp: ResetPlantResponse =
        serde_json::from_str(&text).map_err(|e| format!("Parse error: {e}"))?;
    if resp.success {
        Ok(())
    } else {
        Err(resp.error.unwrap_or_else(|| "Reset failed".into()))
    }
}

fn mock_plants() -> Vec<ApiPlant> {
    let now = chrono::Utc::now().timestamp_millis();
    vec![ApiPlant {
        id: "mock-1".into(),
        strain_id: "banana-fritter".into(),
        strain_name: "Banana Fritter".into(),
        current_stage: GrowthStage::BigVeg,
        planted_at: now.saturating_sub(86400000 * 5),
        is_completed: false,
        reward_claimed: false,
        water_count: 5,
        last_watered_at: Some(now.saturating_sub(120000)),
        streak: 2,
        max_streak: 3,
        target_name: None,
        target_image_url: None,
    }]
}

/// Returns the plant growth stage image URL based on stage index (0-13).
/// Images are at /assets/game/{1..14}.png (single canonical path; the
/// orphan /assets/images/game/ copy was removed in cycle #14).
fn stage_image_url(stage_index: usize) -> String {
    let n = (stage_index + 1).clamp(1, 14);
    format!("/assets/game/{}.png", n)
}

/// Loop #17: format remaining millis as "Xh Ym" (or "0m" if under a minute).
fn format_countdown_ms(remaining_ms: i64) -> String {
    if remaining_ms <= 0 {
        return "0m".to_string();
    }
    let total_minutes = remaining_ms / (60 * 1000);
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

fn stage_color(stage: &GrowthStage) -> &'static str {
    match stage {
        GrowthStage::Seed => "#8b5a2b",
        GrowthStage::Sprout => "#39ff14",
        GrowthStage::FirstLeaf => "#4ade80",
        GrowthStage::YoungBush => "#22c55e",
        GrowthStage::VegStart => "#16a34a",
        GrowthStage::BigVeg => "#00e5ff",
        GrowthStage::PreFlower => "#a855f7",
        GrowthStage::SmallBuds => "#f472b6",
        GrowthStage::BigBuds => "#ff6b35",
        GrowthStage::Trimming => "#fb923c",
        GrowthStage::Curing => "#a78bfa",
        GrowthStage::Lab => "#60a5fa",
        GrowthStage::Delivery => "#34d399",
        GrowthStage::Final => "#ffd700",
    }
}

fn progress_bar_gradient(stage: &GrowthStage) -> &'static str {
    match stage {
        GrowthStage::Seed => "#8b5a2b",
        GrowthStage::Sprout | GrowthStage::FirstLeaf => "#39ff14",
        GrowthStage::YoungBush | GrowthStage::VegStart | GrowthStage::BigVeg => {
            "linear-gradient(90deg, #39ff14, #00e5ff)"
        }
        GrowthStage::PreFlower | GrowthStage::SmallBuds => {
            "linear-gradient(90deg, #00e5ff, #a855f7)"
        }
        GrowthStage::BigBuds | GrowthStage::Trimming => "linear-gradient(90deg, #a855f7, #ff6b35)",
        GrowthStage::Curing | GrowthStage::Lab => "linear-gradient(90deg, #ff6b35, #a78bfa)",
        GrowthStage::Delivery | GrowthStage::Final => "linear-gradient(90deg, #a78bfa, #ffd700)",
    }
}

#[component]
pub fn Garden() -> Element {
    let plants = use_signal(Vec::<ApiPlant>::new);
    let loading = use_signal(|| true);
    let error_msg = use_signal(String::new);
    let now_ms = use_signal(|| chrono::Utc::now().timestamp_millis());
    let streak = use_signal(|| None::<GardenStreakResponse>);
    let mut show_chooser = use_signal(|| false);
    let mut show_reset_warning = use_signal(|| false);
    let mut show_achievements = use_signal(|| false);
    let mut show_leaderboard = use_signal(|| false);
    let leaderboard_kind = use_signal(|| "streak".to_string());
    let new_achievements = use_signal(Vec::<String>::new);
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_telegram_init_data();
    let tg = use_telegram();

    // Loop #17: screen-open metric. Fire once after initial mount; don't block
    // on failure.
    {
        let base = api_base_url();
        use_effect(move || {
            let b = base.clone();
            spawn(async move {
                let _ = post_client_event(&b, "garden_screen_opened", "garden_tab").await;
            });
        });
    }

    {
        let mut plants_c = plants;
        let mut loading_c = loading;
        let mut error_c = error_msg;
        let mut streak_c = streak;
        let tid = telegram_id;
        let init = init_data.clone();
        use_future(move || {
            let value = init.clone();
            async move {
                if tid == 0 {
                    plants_c.set(mock_plants());
                    loading_c.set(false);
                    return;
                }
                // Wait up to ~5s for Telegram WebApp to expose non-empty initData.
                // On some iOS/Android WebViews initDataUnsafe.user.id is available
                // before initData string itself, so the first read can be empty and
                // the server returns 401. Retry before giving up.
                let mut attempt_init = value.clone();
                for _ in 0..12 {
                    if !attempt_init.is_empty() {
                        break;
                    }
                    TimeoutFuture::new(420).await;
                    attempt_init = tg.get_init_data();
                }
                let lang = crate::ui::lang::current_lang();
                if let Some(s) = fetch_garden_streak(tid, &value).await {
                    streak_c.set(Some(s));
                }
                match fetch_plants(tid, &attempt_init).await {
                    Ok(p) => {
                        plants_c.set(p);
                    }
                    // A 401 means the initData we sent was empty, stale (>24h) or
                    // reconstructed with a hash the server rejected. Re-arm the
                    // WebApp, read a fresh initData and retry ONCE before giving up.
                    Err(status) if crate::trios::api_errors::should_retry_reauth(status) => {
                        tg.ready();
                        TimeoutFuture::new(500).await;
                        let fresh_init = tg.get_init_data();
                        match fetch_plants(tid, &fresh_init).await {
                            Ok(p) => {
                                plants_c.set(p);
                            }
                            Err(_) => {
                                // Still unauthorised: show the friendly RU message and
                                // append the server-side validation reason (debug
                                // endpoint) so we can diagnose without client logs.
                                let detail = fetch_validation_reason(&fresh_init).await;
                                error_c.set(format!(
                                    "{}{}",
                                    crate::trios::api_errors::friendly_response_error(lang, 401),
                                    detail
                                ));
                            }
                        }
                    }
                    Err(status) => {
                        // Network/parse (status 0) → treat as a transient server
                        // problem for messaging; otherwise map the real status.
                        let s = if status == 0 { 503 } else { status };
                        error_c.set(crate::trios::api_errors::friendly_response_error(lang, s));
                    }
                }
                loading_c.set(false);
            }
        });
    }

    {
        let mut now_c = now_ms;
        use_future(move || async move {
            loop {
                TimeoutFuture::new(5000).await;
                now_c.set(chrono::Utc::now().timestamp_millis());
            }
        });
    }

    let plant_list = plants.read().clone();
    let now = *now_ms.read();
    let is_loading = *loading.read();
    let err = error_msg.read().clone();
    let streak_opt = streak.read().clone();
    let init_for_closures = init_data.clone();
    let lang = crate::ui::lang::current_lang();

    let bg = "#0f0f1a";
    let bg_card = "#1a1a2e";
    let border_subtle = "rgba(255,255,255,0.1)";
    let title_text = t(lang, T_GARDEN_TITLE);
    let subtitle_text = t(lang, T_GARDEN_SUBTITLE);
    let water_text = t(lang, T_BTN_WATER);
    let loading_text = t(lang, T_GARDEN_LOADING);
    let empty_label = t(lang, T_GARDEN_EMPTY_LABEL);
    let empty_cta = t(lang, T_GARDEN_EMPTY_CTA);
    let choose_product_text = t(lang, T_GARDEN_CHOOSE_PRODUCT);
    let choose_product_hint = t(lang, T_GARDEN_CHOOSE_PRODUCT_HINT);
    let change_product_text = t(lang, T_GARDEN_CHANGE_PRODUCT);
    let reset_progress_text = t(lang, T_GARDEN_RESET_PROGRESS);
    let reset_confirm_title = t(lang, T_GARDEN_RESET_CONFIRM_TITLE);
    let reset_confirm_body = t(lang, T_GARDEN_RESET_CONFIRM_BODY);
    let cancel_text = t(lang, T_GARDEN_CANCEL);
    let confirm_reset_text = t(lang, T_GARDEN_CONFIRM_RESET);
    let diagnostics_copy = t(lang, T_GARDEN_DIAGNOSTICS_COPY);
    let diagnostics_copied = t(lang, T_GARDEN_DIAGNOSTICS_COPIED);
    let plant_alt = t(lang, T_GARDEN_PLANT_ALT);
    let product_alt = t(lang, T_GARDEN_PRODUCT_ALT);
    let discount_badge = t(lang, T_GARDEN_DISCOUNT_BADGE);
    let ready_text = t(lang, T_GARDEN_READY);
    let cooldown_text = t(lang, T_GARDEN_COOLDOWN);
    let harvest_text = t(lang, T_GARDEN_HARVEST);
    let water_now_text = t(lang, T_GARDEN_WATER_NOW).to_string();
    let _next_water_in_text = t(lang, T_GARDEN_NEXT_WATER_IN).to_string();
    let streak_days_text = t(lang, T_GARDEN_STREAK_DAYS).to_string();
    let streak_best_text = t(lang, T_GARDEN_STREAK_BEST).to_string();
    let _reward_expires_in_text = t(lang, T_GARDEN_REWARD_EXPIRES_IN).to_string();
    let share_cta = t(lang, T_GARDEN_SHARE_CTA).to_string();
    let achievements_title = t(lang, T_GARDEN_ACHIEVEMENTS_TITLE).to_string();
    let leaderboard_title = t(lang, T_GARDEN_LEADERBOARD_TITLE).to_string();

    let init_share = init_for_closures.clone();
    let share_tid = telegram_id;
    let share_click = move |_| {
        let init = init_share.clone();
        spawn(async move {
            log_share_event(share_tid, &init, "garden", "garden").await;
            share_garden();
        });
    };

    let new_achievements_signal = new_achievements;
    let dismiss_achievement = move |id: String| {
        let mut ns = new_achievements_signal;
        move |_| {
            ns.write().retain(|a| a != &id);
        }
    };

    rsx! {
        div { style: "min-height: 100vh; background: {bg}; color: #e8e8e8; font-family: 'Press Start 2P', monospace; padding-bottom: 80px;",

            div { style: "padding: 20px 16px 12px; text-align: center;",
                h1 { style: "font-size: 18px; color: #39ff14; text-shadow: 0 0 8px rgba(57,255,20,0.5); display:flex; align-items:center; justify-content:center; gap:8px;",
                    "{title_text}"
                    if let Some(ref s) = streak_opt {
                        if s.has_plant {
                            span { style: "font-size:12px;background:#ff4757;color:#fff;padding:2px 8px;border-radius:10px;white-space:nowrap;", "🔥 {s.streak}" }
                        }
                    }
                }
                p { style: "font-size: 11px; color: #8b8b9e; margin-top: 4px;",
                    "{subtitle_text}"
                }

                // Loop #18: social actions.
                div { style: "display:flex; gap:8px; justify-content:center; margin-top: 12px; flex-wrap: wrap;",
                    button {
                        style: "padding:8px 12px;background:#1a1a2e;color:#e8e8e8;border:2px solid #39ff14;border-radius:20px;font-size:11px;font-weight:700;cursor:pointer;min-height:44px;display:flex;align-items:center;gap:4px;",
                        onclick: share_click,
                        "{share_cta}"
                    }
                    button {
                        style: "width:44px;height:44px;background:#1a1a2e;color:#ffd700;border:2px solid #ffd700;border-radius:50%;font-size:18px;cursor:pointer;",
                        "aria-label": "{achievements_title}",
                        onclick: move |_| show_achievements.set(true),
                        "🏅"
                    }
                    button {
                        style: "width:44px;height:44px;background:#1a1a2e;color:#00e5ff;border:2px solid #00e5ff;border-radius:50%;font-size:18px;cursor:pointer;",
                        "aria-label": "{leaderboard_title}",
                        onclick: move |_| show_leaderboard.set(true),
                        "🏆"
                    }
                }

                // Loop #18: achievement unlock toasts.
                {
                    let toasts: Vec<String> = new_achievements.read().iter().cloned().collect();
                    if !toasts.is_empty() {
                        rsx! {
                            div { style: "display:flex; flex-direction:column; gap:6px; margin-top:12px;",
                                for id in toasts {
                                    div { style: "background:#ffd700;color:#000;padding:8px 12px;border-radius:8px;font-size:12px;font-weight:700;display:flex;align-items:center;justify-content:center;gap:6px;",
                                        "{t(lang, T_GARDEN_ACHIEVEMENTS_NEW)} {id}"
                                        button { style: "background:transparent;border:none;font-weight:700;cursor:pointer;", "aria-label": "{t(lang, T_CLOSE)}", onclick: dismiss_achievement(id.clone()), "✕" }
                                    }
                                }
                            }
                        }
                    } else {
                        rsx! {}
                    }
                }
            }

            ErrorBanner {
                message: err.clone(),
                margin: "0 auto 12px".to_string(),
            }

            // Cycle #171: when initData diagnostics are available, let the user
            // copy the full server JSON with one tap so they can paste it into
            // support/dev chat without touching the browser console.
            if err.contains("server={") {
                div { style: "text-align:center;margin:0 auto 12px;",
                    button {
                        style: "padding:8px 14px;background:#ff4757;color:#fff;border:4px solid #c0392b;box-shadow:3px 3px 0 #000;font-size:11px;cursor:pointer;",
                        onclick: move |_| {
                            if copy_to_clipboard(&err) {
                                let alert_js = format!("alert({});", serde_json::Value::String(diagnostics_copied.to_string()));
                                let _ = js_sys::eval(&alert_js);
                            }
                        },
                        {diagnostics_copy}
                    }
                }
            }

            // Always-visible chooser: pick (or change) the product to grow a
            // discount for. Since cycle #173 choosing no longer resets progress.
            if !is_loading {
                div { style: "text-align:center;padding:0 16px 14px;display:flex;gap:8px;justify-content:center;",
                    button {
                        style: "padding:10px 16px;background:#39ff14;color:#000;border:4px solid #2d9e0f;box-shadow:3px 3px 0 #000;font-size:12px;font-weight:700;cursor:pointer;",
                        onclick: move |_| {
                            let _ = post_client_event(&api_base_url(), "garden_choose_product_tapped", "");
                            show_chooser.set(true);
                        },
                        if plant_list.is_empty() { "🌱 {choose_product_text}" } else { "🔄 {change_product_text}" }
                    }
                    if !plant_list.is_empty() {
                        button {
                            style: "padding:10px 16px;background:#ff4757;color:#fff;border:4px solid #c0392b;box-shadow:3px 3px 0 #000;font-size:12px;font-weight:700;cursor:pointer;",
                            onclick: move |_| {
                                let _ = post_client_event(&api_base_url(), "garden_reset_tapped", "");
                                show_reset_warning.set(true);
                            },
                            "⏪ {reset_progress_text}"
                        }
                    }
                }
            }

            if is_loading {
                div { style: "text-align: center; padding: 60px 20px;",
                    div { style: "font-size: 36px; margin-bottom: 12px;", "🌱" }
                    p { style: "font-size: 12px; color: #8b8b9e;", "{loading_text}" }
                }
            } else if plant_list.is_empty() {
                div { style: "text-align: center; padding: 40px 20px;",
                    div { style: "font-size: 48px; margin-bottom: 16px;", "🌱" }
                    p { style: "font-size: 11px; color: #8b8b9e; margin-bottom: 8px;", "{empty_label}" }
                    p { style: "font-size: 13px; color: #8b8b9e;", "{empty_cta}" }
                    p { style: "font-size: 13px; color: #39ff14; margin-top: 12px;", "↑ {choose_product_hint}" }
                }
            } else {
                div { style: "max-width: 400px; margin: 0 auto; padding: 0 16px;",
                    {plant_list.into_iter().map(|plant| {
                        let core: Plant = (&plant).into();
                        let progress = calculate_progress(&core, now);
                        let pid = plant.id.clone();
                        // Prefer the chosen product's name + photo (the seed); fall
                        // back to the legacy strain + the generic stage sprite.
                        let sname = plant.target_name.clone()
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| plant.strain_name.clone());
                        let wc = plant.water_count;
                        let total_stages = GrowthStage::TOTAL_STAGES;
                        let color = stage_color(&progress.stage).to_string();
                        let gradient = progress_bar_gradient(&progress.stage).to_string();
                        let stage_name = progress.stage_name.clone();
                        let emoji = progress.stage_emoji.clone();
                        let img_url = {
                            // Main hero is ALWAYS the growing-bush sprite (seed →
                            // sprout → … → bud) so the player SEES it grow. The
                            // chosen product shows as a small thumbnail (below).
                            stage_image_url(progress.stage_index)
                        };
                        let product_thumb = {
                            let ti = plant.target_image_url.clone().unwrap_or_default();
                            if ti.starts_with("http://") || ti.starts_with("https://")
                                || (ti.starts_with('/') && !ti.starts_with("//")) {
                                Some(ti)
                            } else {
                                None
                            }
                        };
                        let total_pct = progress.total_progress;
                        let pct_str = format!("{}%", total_pct);
                        let is_active = progress.stage_index > 0 && !progress.is_ready_to_harvest;
                        let can_w = progress.can_water;
                        let is_ready = progress.is_ready_to_harvest;
                        let status_countdown_text = if is_ready {
                            ready_text.to_string()
                        } else if can_w {
                            water_now_text.clone()
                        } else {
                            let remaining = progress.next_water_at.saturating_sub(now);
                            let cd = format_countdown_ms(remaining);
                            tf(lang, T_GARDEN_NEXT_WATER_IN, &[cd])
                        };
                        let reward_expiry_text = streak_opt.as_ref().and_then(|s| s.reward_expires_at).and_then(|expires_at| {
                            let remaining_ms = expires_at.saturating_sub(now);
                            if remaining_ms > 0 {
                                Some(tf(lang, T_GARDEN_REWARD_EXPIRES_IN, &[format_countdown_ms(remaining_ms)]))
                            } else {
                                None
                            }
                        });

                        let border = if is_ready {
                            "1px solid #ffd700".to_string()
                        } else if is_active {
                            format!("1px solid {}", color)
                        } else {
                            format!("1px solid {}", border_subtle)
                        };
                        let shadow = if is_ready {
                            "box-shadow: 0 0 16px rgba(255,215,0,0.3);".to_string()
                        } else if is_active {
                            format!("box-shadow: 0 0 16px {}40;", &color)
                        } else {
                            String::new()
                        };

                        let wt = water_text.to_string();

                        let plants_signal = plants;
                        let error_signal = error_msg;
                        let mut ach_signal_water = new_achievements;
                        let pid_for_water = pid.clone();
                        let init_water = init_for_closures.clone();
                        let water_click = move |_| {
                            let mut ps = plants_signal;
                            let mut es = error_signal;
                            let plant_id = pid_for_water.clone();
                            let init = init_water.clone();
                            spawn(async move {
                                match water_plant_api(&plant_id, &init).await {
                                    Ok(resp) => {
                                        if resp.success {
                                            let mut list = ps.write();
                                            if let Some(p) = list.iter_mut().find(|p| p.id == plant_id) {
                                                p.water_count = resp.water_count;
                                                if let Some(stage_str) = resp.current_stage {
                                                    if let Ok(stage) = serde_json::from_value::<GrowthStage>(serde_json::Value::String(stage_str)) {
                                                        p.current_stage = stage;
                                                    }
                                                }
                                                p.is_completed = resp.is_completed;
                                                p.streak = resp.streak;
                                                p.max_streak = resp.max_streak;
                                                // Update last_watered_at locally so the UI
                                                // immediately disables the water button for the
                                                // 24h cooldown instead of staying enabled until
                                                // the next 5s tick / refetch.
                                                p.last_watered_at = Some(chrono::Utc::now().timestamp_millis());
                                            }
                                            crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                                            let _ = post_client_event(&api_base_url(), "garden_water_tapped", "").await;
                                            if !resp.new_achievements.is_empty() {
                                                ach_signal_water.write().extend(resp.new_achievements);
                                            }
                                        } else {
                                            es.set(resp.error.unwrap_or_else(|| "Water failed".into()));
                                        }
                                    }
                                    Err(e) => { es.set(e); }
                                }
                            });
                        };

                        let plants_signal_h = plants;
                        let error_signal_h = error_msg;
                        let mut ach_signal_harvest = new_achievements;
                        let pid_for_harvest = pid.clone();
                        let init_harvest = init_for_closures.clone();
                        let harvest_click = move |_| {
                            let mut ps = plants_signal_h;
                            let mut es = error_signal_h;
                            let plant_id = pid_for_harvest.clone();
                            let init = init_harvest.clone();
                            spawn(async move {
                                match harvest_plant_api(&plant_id, &init).await {
                                    Ok(resp) => {
                                        ps.write().retain(|p| p.id != plant_id);
                                        crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                                        let _ = post_client_event(&api_base_url(), "garden_harvest_tapped", "").await;
                                        if !resp.new_achievements.is_empty() {
                                            ach_signal_harvest.write().extend(resp.new_achievements);
                                        }
                                    }
                                    Err(e) => { es.set(tf(lang, T_GARDEN_ERROR_HARVEST, &[e])); }
                                }
                            });
                        };

                        rsx! {
                            div {
                                key: "{pid}",
                                style: "background: {bg_card}; border-radius: 12px; margin-bottom: 16px; border: {border}; overflow: hidden; {shadow}",

                                // ── Hero image ──────────────────────────
                                div { style: "position: relative; width: 100%; aspect-ratio: 1/1; overflow: hidden; background: #111;",
                                    img {
                                        src: "{img_url}",
                                        alt: "{plant_alt}",
                                        style: "width: 100%; height: 100%; object-fit: cover;",
                                    }
                                    // Target product thumbnail (top-left): what
                                    // discount this bush is growing.
                                    if let Some(timg) = product_thumb.clone() {
                                        div { style: "position:absolute;top:12px;left:12px;display:flex;align-items:center;gap:6px;background:rgba(0,0,0,0.65);padding:4px 8px;border-radius:8px;z-index:2;",
                                            img { src: "{timg}", alt: "{product_alt}", style: "width:34px;height:34px;object-fit:cover;border-radius:6px;border:1px solid #39ff14;" }
                                            span { style: "font-size:10px;color:#39ff14;font-weight:700;", "{discount_badge}" }
                                        }
                                    }
                                    // Gradient overlay at bottom
                                    div { style: "position: absolute; bottom: 0; left: 0; right: 0; height: 50%; background: linear-gradient(transparent, rgba(15,15,26,0.95));" }

                                    // Plant name overlay
                                    div { style: "position: absolute; bottom: 12px; left: 16px; right: 16px;",
                                        div { style: "font-size: 16px; font-weight: 700; color: #fff; text-shadow: 0 1px 4px rgba(0,0,0,0.8);", "{sname}" }
                                        div { style: "display: flex; align-items: center; gap: 6px; margin-top: 4px;",
                                            span { style: "font-size: 14px;", "{emoji}" }
                                            span { style: "font-size: 12px; color: {color}; font-weight: 600;", "{stage_name}" }
                                        }
                                    }

                                    // Ready badge
                                    if is_ready {
                                        div { style: "position: absolute; top: 12px; right: 12px; font-size: 10px; padding: 4px 10px; border-radius: 12px; background: rgba(255,215,0,0.9); color: #000; font-weight: 700;",
                                            "{ready_text}"
                                        }
                                    }
                                }

                                // ── Progress + controls ─────────────────
                                div { style: "padding: 12px 16px 16px;",

                                    // Water count + progress bar
                                    div { style: "margin-bottom: 10px;",
                                        div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;",
                                            span { style: "font-size: 11px; color: #8b8b9e;",
                                                "💧 {wc}/{total_stages}"
                                            }
                                            span { style: "font-size: 11px; color: #8b8b9e;",
                                                "{pct_str}"
                                            }
                                        }
                                        div { style: "height: 6px; background: rgba(255,255,255,0.08); border-radius: 6px; overflow: hidden;",
                                            div { style: "height: 100%; width: {pct_str}; border-radius: 6px; background: {gradient}; transition: width 0.3s ease;" }
                                        }
                                    }

                                    // Loop #17: status + streak + reward expiry line.
                                    div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; font-size: 11px; color: #8b8b9e;",
                                        span { "{status_countdown_text}" }
                                        span { "🔥 {plant.streak} {streak_days_text} · {streak_best_text} {plant.max_streak}" }
                                    }
                                    if let Some(exp_text) = reward_expiry_text.clone() {
                                        div { style: "margin-bottom: 10px; padding: 6px 10px; background: rgba(255,71,87,0.15); border: 1px solid rgba(255,71,87,0.4); border-radius: 8px; font-size: 11px; color: #ff6b7a; text-align: center;",
                                            "{exp_text}"
                                        }
                                    }

                                    // Action buttons
                                    div { style: "display: flex; gap: 8px;",
                                        if is_ready {
                                            button {
                                                style: "
                                                    flex: 1; padding: 10px; border: none; border-radius: 8px;
                                                    font-family: 'Inter', sans-serif; font-size: 14px;
                                                    font-weight: 700; cursor: pointer; color: #0a0a0a;
                                                    background: linear-gradient(135deg, #ffd700, #ff9500);
                                                ",
                                                onclick: harvest_click,
                                                "{harvest_text}"
                                            }
                                        } else if can_w {
                                            button {
                                                style: "
                                                    flex: 1; padding: 10px; border: none; border-radius: 8px;
                                                    font-family: 'Inter', sans-serif; font-size: 14px;
                                                    font-weight: 700; cursor: pointer; color: #0a0a0a;
                                                    background: linear-gradient(135deg, #39ff14, #22c55e);
                                                ",
                                                onclick: water_click,
                                                "💧 {wt}"
                                            }
                                        } else {
                                            div { style: "
                                                flex: 1; padding: 10px; border-radius: 8px; text-align: center;
                                                font-size: 13px; color: #555;
                                                background: rgba(255,255,255,0.03);
                                            ",
                                                "{cooldown_text}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    })}
                }
            }

            if show_chooser() {
                GardenChooser { telegram_id, init_data: init_data.clone(), plants, open: show_chooser }
            }

            // Cycle #173: explicit reset warning — only the dedicated reset
            // button resets progress. Choosing a product now preserves water_count.
            if show_reset_warning() {
                div {
                    style: "position:fixed;inset:0;z-index:1001;background:rgba(0,0,0,0.8);display:flex;align-items:center;justify-content:center;padding:16px;",
                    onclick: move |_| show_reset_warning.set(false),
                    div {
                        style: "background:#1a1a2e;max-width:340px;width:100%;border:4px solid #ff4757;box-shadow:4px 4px 0 #000;padding:20px;border-radius:12px;text-align:center;",
                        onclick: move |e: Event<MouseData>| e.stop_propagation(),
                        div { style: "font-size:36px;margin-bottom:12px;", "⏪" }
                        div { style: "font-size:15px;font-weight:700;color:#ff6b7a;margin-bottom:10px;", "{reset_confirm_title}" }
                        p { style: "font-size:12px;color:#8b8b9e;line-height:1.5;margin-bottom:16px;",
                            "{reset_confirm_body}"
                        }
                        div { style: "display:flex;gap:10px;justify-content:center;",
                            button {
                                style: "padding:10px 16px;background:transparent;color:#8b8b9e;border:2px solid #2a2a4a;border-radius:8px;font-size:12px;font-weight:700;cursor:pointer;",
                                onclick: move |_| show_reset_warning.set(false),
                                "{cancel_text}"
                            }
                            button {
                                style: "padding:10px 16px;background:#ff4757;color:#fff;border:2px solid #c0392b;border-radius:8px;font-size:12px;font-weight:700;cursor:pointer;box-shadow:2px 2px 0 #000;",
                                onclick: move |_| {
                                    show_reset_warning.set(false);
                                    let pid = plants.read().first().map(|p| p.id.clone()).unwrap_or_default();
                                    let init = init_for_closures.clone();
                                    let mut ps = plants;
                                    let mut es = error_msg;
                                    spawn(async move {
                                        match reset_plant_api(&pid, &init).await {
                                            Ok(()) => {
                                                if let Ok(p2) = fetch_plants(telegram_id, &init).await {
                                                    ps.set(p2);
                                                }
                                            }
                                            Err(e) => { es.set(tf(lang, T_GARDEN_ERROR_RESET, &[e])); }
                                        }
                                    });
                                },
                                "{confirm_reset_text}"
                            }
                        }
                    }
                }
            }

            // Loop #18: social modals.
            if show_achievements() {
                AchievementsModal { telegram_id, init_data: init_data.clone(), open: show_achievements }
            }

            if show_leaderboard() {
                LeaderboardModal { telegram_id, init_data: init_data.clone(), open: show_leaderboard, kind: leaderboard_kind }
            }
        }
    }
}

/// Modal: pick a live, garden-eligible product to grow a discount for. On
/// success the chosen product's photo becomes the plant's seed image.
#[component]
fn GardenChooser(
    telegram_id: i64,
    init_data: String,
    plants: Signal<Vec<ApiPlant>>,
    open: Signal<bool>,
) -> Element {
    let products = use_resource(|| async move { fetch_garden_products().await });
    let mut busy = use_signal(|| false);
    let mut err = use_signal(String::new);
    let lang = crate::ui::lang::current_lang();

    let cat_label = |c: &str| match c {
        "strain" => t(lang, T_GARDEN_CAT_STRAIN),
        "accessory" => t(lang, T_GARDEN_CAT_ACCESSORY),
        "tea" => t(lang, T_GARDEN_CAT_TEA),
        "set" => t(lang, T_GARDEN_CAT_SET),
        "accessory_set" => t(lang, T_GARDEN_CAT_ACCESSORY_SET),
        "tea_set" => t(lang, T_GARDEN_CAT_TEA_SET),
        _ => t(lang, T_GARDEN_CAT_OTHER),
    };

    rsx! {
        div {
            style: "position:fixed;inset:0;z-index:1000;background:rgba(0,0,0,0.75);display:flex;align-items:flex-end;justify-content:center;",
            onclick: move |_| open.set(false),
            div {
                style: "background:#0f0f1a;width:100%;max-width:520px;max-height:85vh;overflow:auto;border-top:4px solid #39ff14;padding:16px;",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),
                div { style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:12px;",
                    div { style: "font-size:15px;font-weight:700;color:#39ff14;", "{t(lang, T_GARDEN_CHOOSER_TITLE)}" }
                    button { style: "width:44px;height:44px;background:transparent;border:none;color:#8b8b9e;font-size:20px;cursor:pointer;",
                        "aria-label": "{t(lang, T_CLOSE)}", onclick: move |_| open.set(false), "✕" }
                }
                if !err.read().is_empty() {
                    div { style: "background:#2a1a1a;color:#ff6b7a;font-size:13px;padding:8px;margin-bottom:10px;border:1px solid #ff4757;", "{err}" }
                }
                {
                    match &*products.read() {
                        Some(Ok(list)) if !list.is_empty() => {
                            let items = list.clone();
                            rsx! {
                                div { style: "display:grid;grid-template-columns:1fr 1fr;gap:10px;",
                                    for p in items.into_iter() {
                                        {
                                            let img = p.image_url.clone().unwrap_or_default();
                                            let has_img = img.starts_with("http") || img.starts_with('/');
                                            let cat = p.catalog.clone();
                                            let pid = p.id.clone();
                                            let name = p.name.clone();
                                            let badge = cat_label(&p.catalog);
                                            let price = if p.price.is_finite() { p.price.max(0.0) } else { 0.0 };
                                            let init = init_data.clone();
                                            rsx! {
                                                div {
                                                    style: "background:#16213e;border:3px solid #2a2a4a;box-shadow:3px 3px 0 #000;overflow:hidden;cursor:pointer;",
                                                    onclick: move |_| {
                                                        if *busy.read() { return; }
                                                        busy.set(true);
                                                        err.set(String::new());
                                                        let cat = cat.clone(); let pid = pid.clone(); let init = init.clone();
                                                        spawn(async move {
                                                            match choose_plant_api(telegram_id, &cat, &pid, &init).await {
                                                                Ok(()) => {
                                                                    if let Ok(p2) = fetch_plants(telegram_id, &init).await {
                                                                        plants.set(p2);
                                                                    }
                                                                    open.set(false);
                                                                }
                                                                Err(code) => {
                                                                    let msg = match code.as_str() {
                                                                        "harvest_cooldown" => t(lang, T_GARDEN_ERROR_COOLDOWN).to_string(),
                                                                        "product_not_available" => t(lang, T_GARDEN_ERROR_PRODUCT_UNAVAILABLE).to_string(),
                                                                        other => other.to_string(),
                                                                    };
                                                                    err.set(msg);
                                                                    busy.set(false);
                                                                }
                                                            }
                                                        });
                                                    },
                                                    div { style: "min-height:90px;background:#111;display:flex;align-items:center;justify-content:center;font-size:32px;",
                                                        if has_img {
                                                            img { src: "{img}", alt: "{name}", style: "width:100%;height:auto;object-fit:contain;display:block;" }
                                                        } else { "🎁" }
                                                    }
                                                    div { style: "padding:8px;",
                                                        div { style: "font-size:10px;color:#8b8b9e;", "{badge}" }
                                                        div { style: "font-size:13px;font-weight:700;color:#e8e8e8;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;", "{name}" }
                                                        div { style: "font-size:12px;color:#ffe600;", "{price} ฿" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Some(Ok(_)) => rsx! { div { style: "text-align:center;padding:30px;color:#8b8b9e;", "{t(lang, T_GARDEN_CHOOSER_EMPTY)}" } },
                        Some(Err(e)) => rsx! { div { style: "text-align:center;padding:30px;color:#ff6b7a;", "{tf(lang, T_GARDEN_CHOOSER_ERROR, &[e.to_string()])}" } },
                        None => rsx! { div { style: "text-align:center;padding:30px;color:#8b8b9e;", "{t(lang, T_GARDEN_CHOOSER_LOADING)}" } },
                    }
                }
            }
        }
    }
}

/// Loop #18: modal showing all garden achievements and recent unlocks.
#[component]
fn AchievementsModal(
    telegram_id: i64,
    init_data: String,
    open: Signal<bool>,
) -> Element {
    let init_for_resource = init_data.clone();
    let data = use_resource(move || {
        let init = init_for_resource.clone();
        async move { fetch_achievements(telegram_id, &init).await }
    });
    let lang = crate::ui::lang::current_lang();

    // Mark all garden achievements as notified when the modal is opened,
    // so the same unlocks don't keep flashing every time the user returns.
    {
        let init = init_data.clone();
        use_effect(move || {
            let init = init.clone();
            spawn(async move {
                mark_achievements_notified(telegram_id, &init).await;
            });
        });
    }

    let content: Element = match &*data.read() {
        Some(Ok(resp)) if !resp.achievements.is_empty() => {
            rsx! {
                div { style: "display:flex;flex-direction:column;gap:10px;",
                    {
                        resp.achievements.iter().map(|a| {
                            let border = if a.unlocked_at.is_some() {
                                "#ffd700"
                            } else {
                                "#2a2a4a"
                            };
                            let opacity = if a.unlocked_at.is_some() { "1" } else { "0.45" };
                            let style = format!(
                                "display:flex;align-items:center;gap:12px;background:#1a1a2e;border:2px solid {border};border-radius:10px;padding:12px;opacity:{opacity};"
                            );
                            let unlocked = a.unlocked_at.is_some();
                            let xp = a.xp_reward;
                            let icon = a.icon.clone();
                            let name = a.name.clone();
                            let desc = a.description.clone();
                            rsx! {
                                div { style: "{style}",
                                    div { style: "font-size:28px;", "{icon}" }
                                    div { style: "flex:1;",
                                        div { style: "font-size:13px;font-weight:700;color:#e8e8e8;", "{name}" }
                                        div { style: "font-size:11px;color:#8b8b9e;margin-top:2px;", "{desc}" }
                                    }
                                    if unlocked {
                                        div { style: "font-size:11px;color:#ffd700;font-weight:700;", "+{xp} XP" }
                                    }
                                }
                            }
                        })
                    }
                }
            }
        }
        Some(Ok(_)) => rsx! {
            div { style: "text-align:center;padding:40px;color:#8b8b9e;",
                "{t(lang, T_GARDEN_ACHIEVEMENTS_EMPTY)}"
            }
        },
        Some(Err(_)) => rsx! { div { style: "text-align:center;padding:40px;color:#ff6b7a;", "Load failed" } },
        None => rsx! { div { style: "text-align:center;padding:40px;color:#8b8b9e;", "Loading..." } },
    };

    rsx! {
        div {
            style: "position:fixed;inset:0;z-index:1002;background:rgba(0,0,0,0.85);display:flex;align-items:flex-end;justify-content:center;",
            onclick: move |_| open.set(false),
            div {
                style: "background:#0f0f1a;width:100%;max-width:520px;max-height:85vh;overflow:auto;border-top:4px solid #ffd700;padding:16px;",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),
                div { style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:12px;",
                    div { style: "font-size:15px;font-weight:700;color:#ffd700;", "{t(lang, T_GARDEN_ACHIEVEMENTS_TITLE)}" }
                    button { style: "width:44px;height:44px;background:transparent;border:none;color:#8b8b9e;font-size:20px;cursor:pointer;",
                        "aria-label": "{t(lang, T_CLOSE)}", onclick: move |_| open.set(false), "✕" }
                }
                { content }
            }
        }
    }
}

/// Loop #18: modal showing the garden leaderboard by streak or harvest.
#[component]
fn LeaderboardModal(
    telegram_id: i64,
    init_data: String,
    open: Signal<bool>,
    kind: Signal<String>,
) -> Element {
    let kind_read = kind.read().clone();
    let data = use_resource(move || {
        let init = init_data.clone();
        let k = kind_read.clone();
        async move { fetch_leaderboard(telegram_id, &k, &init).await }
    });
    let lang = crate::ui::lang::current_lang();

    let content: Element = match &*data.read() {
        Some(Ok(resp)) if !resp.entries.is_empty() => {
            let user_row: Element = if let Some(ref u) = resp.user {
                if let (Some(rank), Some(score)) = (
                    u.get("rank").and_then(|v| v.as_i64()),
                    u.get("score").and_then(|v| v.as_i64()),
                ) {
                    let you_label = t(lang, T_GARDEN_LEADERBOARD_YOU).to_string();
                    let rank_label = t(lang, T_GARDEN_LEADERBOARD_RANK).to_string();
                    rsx! {
                        div { style: "margin-top:12px;padding:10px;background:#16213e;border:2px solid #00e5ff;border-radius:10px;text-align:center;font-size:12px;color:#e8e8e8;",
                            "{you_label} — {rank_label}{rank} — {score}"
                        }
                    }
                } else {
                    rsx! {}
                }
            } else {
                rsx! {}
            };
            rsx! {
                div { style: "display:flex;flex-direction:column;gap:8px;",
                    {
                        resp.entries.iter().map(|e| {
                            let bg = if e.is_you { "#16213e" } else { "#1a1a2e" };
                            let border = if e.is_you { "#00e5ff" } else { "#2a2a4a" };
                            let style = format!(
                                "display:flex;align-items:center;gap:10px;background:{bg};border:2px solid {border};border-radius:10px;padding:10px 12px;"
                            );
                            let name = if e.is_you {
                                t(lang, T_GARDEN_LEADERBOARD_YOU).to_string()
                            } else {
                                e.display_name.clone()
                            };
                            let rank = e.rank;
                            let score = e.score;
                            let rank_label = t(lang, T_GARDEN_LEADERBOARD_RANK).to_string();
                            rsx! {
                                div { style: "{style}",
                                    div { style: "font-size:13px;font-weight:700;color:#8b8b9e;min-width:30px;text-align:center;",
                                        "{rank_label}{rank}"
                                    }
                                    div { style: "flex:1;font-size:13px;font-weight:700;color:#e8e8e8;", "{name}" }
                                    div { style: "font-size:13px;font-weight:700;color:#39ff14;", "{score}" }
                                }
                            }
                        })
                    }
                    { user_row }
                }
            }
        }
        Some(Ok(_)) => rsx! { div { style: "text-align:center;padding:40px;color:#8b8b9e;", "No entries yet" } },
        Some(Err(_)) => rsx! { div { style: "text-align:center;padding:40px;color:#ff6b7a;", "Load failed" } },
        None => rsx! { div { style: "text-align:center;padding:40px;color:#8b8b9e;", "Loading..." } },
    };

    rsx! {
        div {
            style: "position:fixed;inset:0;z-index:1002;background:rgba(0,0,0,0.85);display:flex;align-items:flex-end;justify-content:center;",
            onclick: move |_| open.set(false),
            div {
                style: "background:#0f0f1a;width:100%;max-width:520px;max-height:85vh;overflow:auto;border-top:4px solid #00e5ff;padding:16px;",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),
                div { style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:12px;",
                    div { style: "font-size:15px;font-weight:700;color:#00e5ff;", "{t(lang, T_GARDEN_LEADERBOARD_TITLE)}" }
                    button { style: "width:44px;height:44px;background:transparent;border:none;color:#8b8b9e;font-size:20px;cursor:pointer;",
                        "aria-label": "{t(lang, T_CLOSE)}", onclick: move |_| open.set(false), "✕" }
                }

                // Tabs
                div { style: "display:flex;gap:8px;margin-bottom:12px;",
                    button {
                        style: if kind.read().as_str() == "streak" {
                            "flex:1;padding:10px;background:#00e5ff;color:#000;border:2px solid #00e5ff;border-radius:8px;font-size:12px;font-weight:700;cursor:pointer;"
                        } else {
                            "flex:1;padding:10px;background:transparent;color:#8b8b9e;border:2px solid #2a2a4a;border-radius:8px;font-size:12px;font-weight:700;cursor:pointer;"
                        },
                        onclick: move |_| kind.set("streak".to_string()),
                        "{t(lang, T_GARDEN_LEADERBOARD_TAB_STREAK)}"
                    }
                    button {
                        style: if kind.read().as_str() == "harvest" {
                            "flex:1;padding:10px;background:#39ff14;color:#000;border:2px solid #39ff14;border-radius:8px;font-size:12px;font-weight:700;cursor:pointer;"
                        } else {
                            "flex:1;padding:10px;background:transparent;color:#8b8b9e;border:2px solid #2a2a4a;border-radius:8px;font-size:12px;font-weight:700;cursor:pointer;"
                        },
                        onclick: move |_| kind.set("harvest".to_string()),
                        "{t(lang, T_GARDEN_LEADERBOARD_TAB_HARVEST)}"
                    }
                }

                { content }
            }
        }
    }
}
