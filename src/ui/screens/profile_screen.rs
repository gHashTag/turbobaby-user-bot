use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_PROFILE_TITLE};
use crate::ui::api::context::api_base_url;
use crate::ui::assets;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::lang::{current_lang, set_app_lang};
use crate::ui::routes::Route;
use crate::ui::state::Cart;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use dioxus::prelude::*;
use qrcode::QrCode;
use serde::Deserialize;
use web_sys;

#[derive(Debug, Clone, Deserialize)]
struct LoyaltyResponse {
    profile: Option<LoyaltyProfileData>,
}

#[derive(Debug, Clone, Deserialize)]
struct LoyaltyProfileData {
    total_spent: Option<f64>,
    tier: Option<String>,
    bonus_balance: Option<f64>,
    referral_code: Option<String>,
    referral_count: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
struct StarsBalanceResp {
    balance: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // API mirror: telegram_id/min_age_years are used for diagnostics only.
struct UserProfileResp {
    telegram_id: i64,
    age_verified: bool,
    date_of_birth: Option<String>,
    min_age_years: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tier {
    Starter,
    Bronze,
    Silver,
    Gold,
}

impl Tier {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "bronze" | "brass" => Self::Bronze,
            "silver" => Self::Silver,
            "gold" | "platinum" | "diamond" | "woody" => Self::Gold,
            _ => Self::Starter,
        }
    }
    fn label(&self) -> &'static str {
        match self {
            Self::Starter => "Starter",
            Self::Bronze => "Bronze Bud",
            Self::Silver => "Silver Bud",
            Self::Gold => "Gold Bud",
        }
    }
    fn emoji(&self) -> &'static str {
        match self {
            Self::Starter => "🌱",
            Self::Bronze => "🥉",
            Self::Silver => "🥈",
            Self::Gold => "🥇",
        }
    }
    fn full_card_image(&self) -> &'static str {
        match self {
            Self::Starter | Self::Bronze => assets::member_cards::BRASS,
            Self::Silver => assets::member_cards::SILVER,
            Self::Gold => assets::member_cards::GOLD,
        }
    }
    fn bud_image(&self) -> &'static str {
        match self {
            Self::Starter | Self::Bronze => assets::member_cards::BRONZE_WEBP,
            Self::Silver => assets::member_cards::SILVER_WEBP,
            Self::Gold => assets::member_cards::GOLD_WEBP,
        }
    }
    fn cashback(&self) -> f64 {
        match self {
            Self::Starter => 2.0,
            Self::Bronze => 5.0,
            Self::Silver => 8.0,
            Self::Gold => 12.0,
        }
    }
    fn threshold(&self) -> f64 {
        match self {
            Self::Starter => 0.0,
            Self::Bronze => 200.0,
            Self::Silver => 500.0,
            Self::Gold => 1000.0,
        }
    }
    fn next(&self) -> Option<Self> {
        match self {
            Self::Starter => Some(Self::Bronze),
            Self::Bronze => Some(Self::Silver),
            Self::Silver => Some(Self::Gold),
            Self::Gold => None,
        }
    }
    fn color(&self) -> &'static str {
        match self {
            Self::Starter => "#8b8b9e",
            Self::Bronze => "#cd7f32",
            Self::Silver => "#c0c0c0",
            Self::Gold => "#ffd700",
        }
    }
    fn all() -> &'static [Tier] {
        &[Self::Starter, Self::Bronze, Self::Silver, Self::Gold]
    }
}

fn today_iso() -> String {
    chrono::Local::now().naive_local().date().format("%Y-%m-%d").to_string()
}

#[component]
pub fn ProfileScreen() -> Element {
    let cart = use_context::<Signal<Cart>>();
    let cart_count: u32 = cart.read().items.iter().map(|i| i.quantity).sum();

    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_telegram_init_data();
    let init_data_for_loyalty = init_data.clone();
    let init_data_for_stars = init_data.clone();
    let init_data_for_profile = init_data.clone();
    let init_data_for_verify = init_data.clone();

    let loyalty_resource = use_resource(move || {
        let init = init_data_for_loyalty.clone();
        async move {
            if telegram_id == 0 {
                return None;
            }
            let base = api_base_url();
            let url = format!("{}/api/loyalty/{}", base, telegram_id);
            let client = crate::ui::api::local_client::LocalClient::new();
            let resp = client
                .get(&url)
                .header("X-Telegram-Init-Data", init)
                .send()
                .await;
            match resp {
                Ok(r) => r.json::<LoyaltyResponse>().await.ok(),
                Err(_) => None,
            }
        }
    });

    let loyalty_data = loyalty_resource
        .read()
        .clone()
        .flatten()
        .and_then(|r| r.profile.clone());

    let mut age_verified = use_signal(|| false);
    let mut dob_input = use_signal(|| String::new());
    let mut verification_error = use_signal(|| String::new());
    let mut verification_loading = use_signal(|| false);

    let _profile_resource = use_resource(move || {
        let init = init_data_for_profile.clone();
        async move {
            if telegram_id == 0 {
                return;
            }
            let url = format!("{}/api/users/me/{}", api_base_url(), telegram_id);
            let client = crate::ui::api::local_client::LocalClient::new();
            if let Ok(r) = client
                .get(&url)
                .header("X-Telegram-Init-Data", init)
                .send()
                .await
            {
                if let Ok(resp) = r.json::<UserProfileResp>().await {
                    age_verified.set(resp.age_verified);
                    if let Some(d) = resp.date_of_birth {
                        dob_input.set(d);
                    }
                }
            }
        }
    });

    let stars_balance_res = use_resource(move || {
        let init = init_data_for_stars.clone();
        async move {
            if telegram_id == 0 {
                return None;
            }
            let url = format!("{}/api/stars/balance/{}", api_base_url(), telegram_id);
            let client = crate::ui::api::local_client::LocalClient::new();
            let resp = client
                .get(&url)
                .header("X-Telegram-Init-Data", init)
                .send()
                .await;
            match resp {
                Ok(r) => r.json::<StarsBalanceResp>().await.ok().map(|r| r.balance),
                Err(_) => None,
            }
        }
    });
    let stars_balance = stars_balance_res.read().clone().flatten().unwrap_or(0);

    let current_tier = loyalty_data
        .as_ref()
        .and_then(|d| d.tier.as_deref())
        .map(Tier::from_str)
        .unwrap_or(Tier::Silver);

    let total_spent = loyalty_data
        .as_ref()
        .and_then(|d| d.total_spent)
        .filter(|v| v.is_finite())
        .unwrap_or(750.0)
        .max(0.0);

    let bonus_balance = loyalty_data
        .as_ref()
        .and_then(|d| d.bonus_balance)
        .filter(|v| v.is_finite())
        .unwrap_or(150.0)
        .max(0.0);

    // Single source of truth for the ฿ stat (was an inline `as i32` narrowing).
    let total_spent_str = crate::trios::pricing::format_baht(total_spent);

    // Cashback is calculated from tier
    let cashback_pct = current_tier.cashback();

    // Cycle #169E: fallback to raw telegram_id instead of stale "WOODY-DEMO"
    // placeholder so the referral deep-link always carries the user's
    // actual identifier even if the loyalty_profile row hasn't been
    // seeded with a referral_code yet.
    let referral_code = loyalty_data
        .as_ref()
        .and_then(|d| d.referral_code.clone())
        .unwrap_or_else(|| telegram_id.to_string());

    let referral_count = loyalty_data
        .as_ref()
        .and_then(|d| d.referral_count)
        .unwrap_or(3);

    // Orders count would need to come from orders API endpoint
    // For now, using a default value
    let orders_count = 24;

    let next_tier = current_tier.next();
    let progress_pct = if let Some(next) = next_tier {
        let threshold = next.threshold();
        if threshold > 0.0 {
            ((total_spent / threshold) * 100.0).min(100.0) as i32
        } else {
            100
        }
    } else {
        100
    };
    let remaining = next_tier.map(|t| (t.threshold() - total_spent).max(0.0));

    let profile_title = t(crate::ui::lang::current_lang(), T_PROFILE_TITLE);

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: 80px;
        ",
            div { style: "padding: 20px 16px 16px; text-align: center;",
                h1 { style: "font-size: 24px; font-weight: 800; color: #39ff14; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(57,255,20,0.5); letter-spacing: 2px;", "{profile_title}" }
                p { style: "font-size: 13px; color: #8b8b9e; margin-top: 4px;", "Your membership status" }
            }

            // Age verification banner
            {
                let verified = *age_verified.read();
                if verified {
                    rsx! {
                        div { style: "margin: 0 16px 16px; padding: 12px; background: #1a3a1a; border: 2px solid #39ff14; border-radius: 6px; display: flex; align-items: center; gap: 8px;",
                            span { style: "font-size: 16px;", "✅" }
                            span { style: "font-size: 13px; color: #9efb9e;", "Age verified — you can place orders" }
                        }
                    }
                } else {
                    let dob = dob_input.read().clone();
                    let err = verification_error.read().clone();
                    let loading = *verification_loading.read();
                    rsx! {
                        div { style: "margin: 0 16px 16px; padding: 12px; background: #2a1a0f; border: 2px solid #ff9d00; border-radius: 6px;",
                            div { style: "font-size: 13px; color: #ff9d00; margin-bottom: 8px;", "🔞 Age verification required (20+)" }
                            div { style: "display: flex; gap: 8px; align-items: center;",
                                input {
                                    r#type: "date",
                                    style: "flex: 1; padding: 8px; background: #0f0f1a; color: #e8e8e8; border: 1px solid #444; border-radius: 4px; font-size: 13px;",
                                    value: "{dob}",
                                    max: "{today_iso()}",
                                    oninput: move |e| dob_input.set(e.value()),
                                }
                                button {
                                    style: "padding: 8px 14px; background: #ff9d00; color: #000; border: none; border-radius: 4px; font-size: 13px; font-weight: 700; cursor: pointer; min-width: 44px; min-height: 44px;",
                                    disabled: loading,
                                    onclick: move |_| {
                                        let dob = dob_input.read().clone();
                                        if dob.is_empty() { return; }
                                        verification_loading.set(true);
                                        verification_error.set(String::new());
                                        let init = init_data_for_verify.clone();
                                        let tid = telegram_id;
                                        spawn(async move {
                                            let url = format!("{}/api/users/me/{}/verify-age", api_base_url(), tid);
                                            let client = crate::ui::api::local_client::LocalClient::new();
                                            let body = serde_json::json!({ "dob": dob });
                                            match client.post(&url)
                                                .header("X-Telegram-Init-Data", init)
                                                .json(&body)
                                                .send()
                                                .await
                                            {
                                                Ok(r) if r.status().is_success() => {
                                                    if let Ok(resp) = r.json::<serde_json::Value>().await {
                                                        if let Some(true) = resp.get("age_verified").and_then(|v| v.as_bool()) {
                                                            age_verified.set(true);
                                                        } else {
                                                            verification_error.set("You must be 20+ to order".to_string());
                                                        }
                                                    }
                                                }
                                                Ok(r) => {
                                                    verification_error.set(format!("Server error: {}", r.status()));
                                                }
                                                Err(e) => {
                                                    verification_error.set(format!("Network: {}", e));
                                                }
                                            }
                                            verification_loading.set(false);
                                        });
                                    },
                                    if loading { "..." } else { "Verify" }
                                }
                            }
                            if !err.is_empty() {
                                div { style: "font-size: 12px; color: #ff6b7a; margin-top: 8px;", "{err}" }
                            }
                        }
                    }
                }
            }

            // Tier strip — all 4 tiers
            div { style: "display: flex; gap: 6px; padding: 0 16px 16px; overflow-x: auto;",
                for tier in Tier::all() {
                    {
                        let is_current = *tier == current_tier;
                        let is_unlocked = tier.threshold() <= total_spent;
                        let border = if is_current { tier.color() } else { "#2a2a4a" };
                        let opacity = if is_unlocked { "1.0" } else { "0.4" };
                        let label = tier.label();
                        let _emoji = tier.emoji();
                        let cb = tier.cashback();
                        let thresh = tier.threshold();
                        let shadow_val = if is_current {
                            format!("0 0 12px {}44", tier.color())
                        } else {
                            "none".to_string()
                        };

                        rsx! {
                            div { style: "
                                background: #16213e; border: 4px solid {border};
                                border-radius: 0; padding: 10px 8px; min-width: 80px;
                                text-align: center; opacity: {opacity};
                                box-shadow: {shadow_val};
                            ",
                                div { style: "display: flex; justify-content: center; margin-bottom: 6px; height: 32px; align-items: center;",
                                    if is_unlocked {
                                        img {
                                            src: "{tier.bud_image()}",
                                            alt: "{tier.label()}",
                                            style: "max-width: 100%; max-height: 100%; object-fit: contain; filter: drop-shadow(0 0 4px {tier.color()}80);"
                                        }
                                    } else {
                                        div { style: "font-size: 20px;", "🔒" }
                                    }
                                }
                                div { style: "font-size: 15px; color: {tier.color()}; margin-bottom: 2px;", "{label}" }
                                div { style: "font-size: 13px; color: #8b8b9e;", "{cb}% cashback" }
                                if !is_unlocked {
                                    div { style: "font-size: 15px; color: #ff4757; margin-top: 2px;", "฿{thresh}" }
                                }
                            }
                        }
                    }
                }
            }

            // Member card image
            div { style: "max-width: 380px; margin: 0 auto 16px; padding: 0 16px; perspective: 800px;",
                img {
                    class: "comet-card",
                    src: "{current_tier.full_card_image()}",
                    alt: "Member Card",
                    style: "width: 100%; max-width: 320px; border-radius: 12px; display: block; margin: 0 auto; box-shadow: 4px 4px 0 #000, 0 0 24px {current_tier.color()}40; transition: transform 0.1s;",
                }
            }

            // QR Code Card
            {
                let qr_value = format!("https://t.me/Woody_WeedPecker_bot?start=ref_{}", referral_code);
                let qr_data_uri = QrCode::new(qr_value.as_bytes())
                    .ok()
                    .map(|code| {
                        let svg_str = code
                            .render::<qrcode::render::svg::Color>()
                            .min_dimensions(200, 200)
                            .dark_color(qrcode::render::svg::Color("#39ff14"))
                            .light_color(qrcode::render::svg::Color("#0f0f1a"))
                            .quiet_zone(false)
                            .build();
                        format!("data:image/svg+xml;base64,{}", STANDARD.encode(svg_str.as_bytes()))
                    })
                    .unwrap_or_default();
                let _qr_title = t(crate::ui::lang::current_lang(), T_PROFILE_TITLE);
                let ref_link = format!("https://t.me/Woody_WeedPecker_bot?start=ref_{}", referral_code);
                let ref_link_copy = ref_link.clone();
                let ref_link_share = ref_link.clone();
                rsx! {
                    div { style: "
                        margin: 0 16px 16px;
                        background: #16213e; border: 4px solid #39ff14;
                        box-shadow: 4px 4px 0 #000, 0 0 20px rgba(57,255,20,0.1);
                        padding: 28px 16px 24px; text-align: center;
                    ",
                        // QR code with glow border
                        div { style: "
                            display: inline-block;
                            background: #0f0f1a; border: 4px solid #39ff14;
                            border-radius: 0; padding: 20px;
                            box-shadow: inset 0 0 20px rgba(57,255,20,0.05), 0 0 16px rgba(57,255,20,0.15);
                            line-height: 0;
                        ",
                            img {
                                src: "{qr_data_uri}",
                                style: "width: 200px; height: 200px; display: block;",
                                alt: "Referral QR"
                            }
                        }
                        div { style: "
                            font-size: 14px; font-weight: 800; color: #39ff14;
                            margin-top: 16px; margin-bottom: 16px;
                            text-shadow: 2px 2px 0 #000;
                        ", "Your QR Code" }
                        // Action buttons — Copy + Share
                        div { style: "display: flex; gap: 10px; margin-top: 16px;",
                            button {
                                style: "
                                    flex: 1; padding: 12px 8px;
                                    background: #16213e; color: #39ff14;
                                    border: 4px solid #39ff14;
                                    box-shadow: 3px 3px 0 #000;
                                    font-weight: 700; font-size: 14px;
                                    cursor: pointer; transition: transform 0.1s, box-shadow 0.1s;
                                ",
                                onclick: move |_| {
                                    let _ = web_sys::window().map(|w| {
                                        let _ = w.navigator().clipboard()
                                            .write_text(&format!("🎁 Get bonus at Woody Weed!\n{}", ref_link_copy));
                                    });
                                },
                                "Copy Link"
                            }
                            button {
                                style: "
                                    flex: 1; padding: 12px 8px;
                                    background: #39ff14; color: #000;
                                    border: 4px solid #2d9e0f;
                                    box-shadow: 3px 3px 0 #000;
                                    font-weight: 700; font-size: 14px;
                                    cursor: pointer; transition: transform 0.1s, box-shadow 0.1s;
                                ",
                                onclick: move |_| {
                                    let share_url = format!(
                                        "https://t.me/share/url?url={}&text={}",
                                        urlencoding::encode(&ref_link_share),
                                        urlencoding::encode("🎁 Get bonus at Woody Weed!")
                                    );
                                    let _ = web_sys::window().and_then(|w| w.open_with_url_and_target(&share_url, "_blank").ok());
                                },
                                "Share"
                            }
                        }
                        div { style: "
                            font-size: 13px; color: #8b8b9e; margin-top: 14px;
                        ", "👥 {referral_count} friends invited" }
                    }
                }
            }

            // Stats row
            div { style: "display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; padding: 0 16px 16px;",
                div { style: "text-align: center; padding: 10px 4px; background: #16213e; border: 4px solid #2a2a4a; border-radius: 0; box-shadow: 4px 4px 0 #000;",
                    div { style: "font-size: 20px; font-weight: 800; color: {current_tier.color()}; text-shadow: 2px 2px 0 #000; margin-bottom: 4px;", "{total_spent_str}" }
                    div { style: "font-size: 13px; color: #8b8b9e;", "SPENT" }
                }
                div { style: "text-align: center; padding: 10px 4px; background: #16213e; border: 4px solid #2a2a4a; border-radius: 0; box-shadow: 4px 4px 0 #000;",
                    div { style: "font-size: 20px; font-weight: 800; color: #39ff14; text-shadow: 2px 2px 0 #000; margin-bottom: 4px;", "B{bonus_balance as i32}" }
                    div { style: "font-size: 13px; color: #8b8b9e;", "BONUS" }
                }
                div { style: "text-align: center; padding: 10px 4px; background: #16213e; border: 4px solid #2a2a4a; border-radius: 0; box-shadow: 4px 4px 0 #000;",
                    div { style: "font-size: 20px; font-weight: 800; color: #7dd3fc; text-shadow: 2px 2px 0 #000; margin-bottom: 4px;", "⭐{stars_balance}" }
                    div { style: "font-size: 13px; color: #8b8b9e;", "STARS" }
                }
                div { style: "text-align: center; padding: 10px 4px; background: #16213e; border: 4px solid #2a2a4a; border-radius: 0; box-shadow: 4px 4px 0 #000;",
                    div { style: "font-size: 20px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000; margin-bottom: 4px;", "{cashback_pct as i32}%" }
                    div { style: "font-size: 13px; color: #8b8b9e;", "CASHBACK" }
                }
            }

            // Progress to next tier
            if let Some(next) = next_tier {
                div { style: "padding: 0 16px 16px;",
                    div { style: "
                        background: #16213e; border: 4px solid {next.color()}33;
                        border-radius: 0; padding: 12px;
                        box-shadow: 4px 4px 0 #000;
                    ",
                        div { style: "display: flex; justify-content: space-between; font-size: 13px; margin-bottom: 6px;",
                            span { style: "color: #8b8b9e;", "Progress to {next.label()}" }
                            span { style: "color: {next.color()};", "{progress_pct}%" }
                        }
                        div { style: "height: 8px; background: rgba(0,0,0,0.4); border-radius: 0; overflow: hidden;",
                            div { style: "height: 100%; width: {progress_pct}%; border-radius: 0; background: linear-gradient(90deg, {current_tier.color()}, {next.color()}); transition: width 0.3s;" }
                        }
                        if let Some(rem) = remaining {
                            {
                                let rem_str = crate::trios::pricing::format_baht(rem);
                                rsx! {
                                    div { style: "font-size: 13px; color: #8b8b9e; margin-top: 6px; text-align: center;",
                                        "{rem_str} more to unlock {next.label()}"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Referral section
            div { style: "
                margin: 0 16px 16px;
                background: #16213e; border: 4px solid #2a2a4a;
                border-radius: 0; padding: 12px;
                box-shadow: 4px 4px 0 #000;
            ",
                div { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 8px;", "🔗 Referral Link" }
                div { style: "
                    display: flex; gap: 6px; align-items: center;
                    background: #0f0f1a; border: 4px solid #2a2a4a;
                    border-radius: 0; padding: 6px 8px; margin-bottom: 8px;
                ",
                    span { style: "font-size: 15px; color: #39ff14; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;", "t.me/Woody_WeedPecker_bot?start=ref_{referral_code}" }
                    button {
                        style: "
                            font-size: 13px; padding: 4px 8px;
                            background: #00e5ff; color: #000;
                            border: 4px solid #00b8d4; border-radius: 0; cursor: pointer;
                            box-shadow: 3px 3px 0 #000;
                        ",
                        onclick: move |_| {
                            let _ = web_sys::window().map(|w| {
                                let _ = w.navigator().clipboard()
                                    .write_text(&format!("https://t.me/Woody_WeedPecker_bot?start=ref_{}", referral_code));
                            });
                        },
                        "Copy"
                    }
                }
                div { style: "display: flex; gap: 12px; font-size: 13px;",
                    span { style: "color: #8b8b9e;", "👥 {referral_count} invited" }
                    span { style: "color: #39ff14;", "Earn ฿100 per referral" }
                }
            }

            // Quick actions
            div { style: "padding: 0 16px;",
                div { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "Quick Actions" }
                Link { to: Route::Orders {},
                    div { style: "
                        background: #16213e; border: 4px solid #2a2a4a;
                        border-radius: 0; padding: 12px 14px; margin-bottom: 8px;
                        display: flex; justify-content: space-between; align-items: center;
                        box-shadow: 4px 4px 0 #000; cursor: pointer;
                    ",
                        div { style: "display: flex; align-items: center; gap: 10px;",
                            span { style: "font-size: 14px;", "📋" }
                            span { style: "font-size: 15px;", "My Orders" }
                            span { style: "font-size: 13px; color: #8b8b9e;", "({orders_count})" }
                        }
                        span { style: "font-size: 15px; color: #8b8b9e;", "→" }
                    }
                }
                Link { to: Route::Garden {},
                    div { style: "
                        background: #16213e; border: 4px solid #2a2a4a;
                        border-radius: 0; padding: 12px 14px; margin-bottom: 8px;
                        display: flex; justify-content: space-between; align-items: center;
                        box-shadow: 4px 4px 0 #000; cursor: pointer;
                    ",
                        div { style: "display: flex; align-items: center; gap: 10px;",
                            span { style: "font-size: 14px;", "🌱" }
                            span { style: "font-size: 15px;", "My Garden" }
                        }
                        span { style: "font-size: 15px; color: #8b8b9e;", "→" }
                    }
                }
                Link { to: Route::Quest { id: "daily".to_string() },
                    div { style: "
                        background: #16213e; border: 4px solid #2a2a4a;
                        border-radius: 0; padding: 12px 14px; margin-bottom: 8px;
                        display: flex; justify-content: space-between; align-items: center;
                        box-shadow: 4px 4px 0 #000; cursor: pointer;
                    ",
                        div { style: "display: flex; align-items: center; gap: 10px;",
                            span { style: "font-size: 14px;", "🎯" }
                            span { style: "font-size: 15px;", "Quests" }
                        }
                        span { style: "font-size: 15px; color: #8b8b9e;", "→" }
                    }
                }
                Link { to: Route::Referrals {},
                    div { style: "
                        background: #16213e; border: 4px solid #2a2a4a;
                        border-radius: 0; padding: 12px 14px; margin-bottom: 8px;
                        display: flex; justify-content: space-between; align-items: center;
                        box-shadow: 4px 4px 0 #000; cursor: pointer;
                    ",
                        div { style: "display: flex; align-items: center; gap: 10px;",
                            span { style: "font-size: 14px;", "🔗" }
                            span { style: "font-size: 15px;", "Referral Program" }
                        }
                        span { style: "font-size: 15px; color: #8b8b9e;", "→" }
                    }
                }
            }

            // Tier benefits grid
            div { style: "padding: 16px;",
                div { style: "font-size: 13px; font-weight: 700; color: #ffe600; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "💎 Tier Benefits" }
                div { style: "display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 8px;",
                    for tier in [Tier::Bronze, Tier::Silver, Tier::Gold] {
                        {
                            let is_unlocked = tier.threshold() <= total_spent;
                            let opacity = if is_unlocked { "1" } else { "0.5" };
                            rsx! {
                                div { style: "
                                    background: #16213e; border: 4px solid {tier.color()}33;
                                    border-radius: 0; padding: 10px 8px; text-align: center;
                                    opacity: {opacity}; box-shadow: 4px 4px 0 #000;
                                ",
                                    div { style: "display: flex; justify-content: center; margin-bottom: 6px; height: 32px; align-items: center;",
                                        img {
                                            src: "{tier.bud_image()}",
                                            alt: "{tier.label()}",
                                            style: "max-width: 100%; max-height: 100%; object-fit: contain; filter: drop-shadow(0 0 4px {tier.color()}80);"
                                        }
                                    }
                                    div { style: "font-size: 15px; color: {tier.color()}; margin-bottom: 4px;", "{tier.label()}" }
                                    div { style: "font-size: 13px; color: #8b8b9e; margin-bottom: 2px;", "{tier.cashback()}% cashback" }
                                    {
                                        let threshold_str = crate::trios::pricing::format_baht(tier.threshold() as f64);
                                        rsx! { div { style: "font-size: 15px; color: #8b8b9e;", "{threshold_str}+" } }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            div { style: "
                margin: 16px 16px 0; padding: 14px;
                background: #16213e; border: 4px solid #2a2a4a;
                border-radius: 0; box-shadow: 4px 4px 0 #000;
            ",
                div { style: "font-size: 13px; color: #8b8b9e; margin-bottom: 10px;",
                    "\u{1F310} Language / \u{042F}\u{0437}\u{044B}\u{043A}"
                },
                {
                    // Cycle (this commit): switcher now writes to the
                    // shared `crate::ui::lang::APP_LANG` GlobalSignal
                    // via `set_app_lang`. Pre-cycle this wrote to a
                    // separate `Signal<Language>` that no other screen
                    // observed, so EN clicks did nothing visible
                    // outside this card (user-reported bug).
                    let current = current_lang();
                    let langs = vec![
                        (Lang::Russian, "\u{1F1F7}\u{1F1FA}", "RU"),
                        (Lang::English, "\u{1F1EC}\u{1F1E7}", "EN"),
                    ];
                    rsx! {
                        div { style: "display: flex; gap: 8px;",
                            for (l, flag, code) in langs {
                                {
                                    let is_active = current == l;
                                    let bg = if is_active { "#39ff1415" } else { "#2a2a4a" };
                                    let color = if is_active { "#39ff14" } else { "#8b8b9e" };
                                    let border = if is_active { "#39ff14" } else { "#2a2a4a" };
                                    let l_c = l;
                                    rsx! {
                                        button {
                                            key: "{code}",
                                            style: "
                                                flex: 1; font-size: 13px; padding: 8px;
                                                background: {bg}; color: {color};
                                                border: 4px solid {border}; border-radius: 20px; cursor: pointer;
                                            ",
                                            onclick: move |_| set_app_lang(l_c),
                                            "{flag} {code}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Contact / Shop info
            div { style: "
                margin: 0 16px 16px;
                background: #16213e; border: 4px solid #2a2a4a;
                border-radius: 0; padding: 14px;
                box-shadow: 4px 4px 0 #000;
            ",
                div { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "📍 Контакты" }
                div { style: "font-size: 15px; margin-bottom: 2px;", "Woody Weed Pecker" }
                div { style: "font-size: 13px; color: #8b8b9e; margin-bottom: 8px;", "Koh Phangan, Thailand" }
                div {
                    style: "cursor: pointer; font-size: 13px; color: #00e5ff; text-decoration: underline;",
                    onclick: move |_| {
                        let _ = web_sys::window().and_then(|w| w.open_with_url_and_target("https://www.google.com/maps/place/Woody+Weed+Pecker/@9.7124562,99.9877309,17z/data=!3m1!4b1!4m6!3m5!1s0x3054ffe9f6df4edf:0xf8735a84f5193e1a!8m2!3d9.7124562!4d99.9877309!16s%2Fg%2F11x314fym6!18m1!1e1?entry=ttu&g_ep=EgoyMDI2MDUxMy4wIKXMDSoASAFQAw%3D%3D", "_blank").ok());
                    },
                    "Открыть на карте"
                }
            }

            BottomNav { cart_count }
        }
    }
}
