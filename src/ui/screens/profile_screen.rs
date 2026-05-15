use dioxus::prelude::*;
use qrcode::QrCode;
use serde::Deserialize;
use crate::ui::routes::Route;
use crate::ui::state::{Cart, use_language, Language};
use crate::ui::assets;
use crate::ui::api::context::api_base_url;
use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_PROFILE_TITLE};
use crate::ui::components::bottom_nav::BottomNav;

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

#[component]
pub fn ProfileScreen() -> Element {
    let cart = use_context::<Signal<Cart>>();
    let cart_count: u32 = cart.read().items.iter().map(|i| i.quantity).sum();

    // Mock telegram_id for development (in real app, get from Telegram WebApp)
    let telegram_id = 123456i64;

    let loyalty_resource = use_resource(move || async move {
        let base = api_base_url();
        let url = format!("{}/api/loyalty/{}", base, telegram_id);
        let client = reqwest::Client::new();
        let resp = client.get(&url).send().await;
        match resp {
            Ok(r) => r.json::<LoyaltyResponse>().await.ok(),
            Err(_) => None,
        }
    });

    let loyalty_data = loyalty_resource.read().clone().flatten().and_then(|r| r.profile.clone());

    let current_tier = loyalty_data.as_ref()
        .and_then(|d| d.tier.as_deref())
        .map(|t| Tier::from_str(t))
        .unwrap_or(Tier::Silver);

    let total_spent = loyalty_data.as_ref()
        .and_then(|d| d.total_spent)
        .unwrap_or(750.0);

    let bonus_balance = loyalty_data.as_ref()
        .and_then(|d| d.bonus_balance)
        .unwrap_or(150.0);

    // Cashback is calculated from tier
    let cashback_pct = current_tier.cashback();

    let referral_code = loyalty_data.as_ref()
        .and_then(|d| d.referral_code.clone())
        .unwrap_or("WOODY-DEMO".to_string());

    let referral_count = loyalty_data.as_ref()
        .and_then(|d| d.referral_count)
        .unwrap_or(3);

    // Orders count would need to come from orders API endpoint
    // For now, using a default value
    let orders_count = 24;

    let next_tier = current_tier.next();
    let progress_pct = if let Some(next) = next_tier {
        let threshold = next.threshold();
        if threshold > 0.0 { ((total_spent / threshold) * 100.0).min(100.0) as i32 } else { 100 }
    } else {
        100
    };
    let remaining = next_tier.map(|t| (t.threshold() - total_spent).max(0.0));

    let profile_title = t(Lang::Russian, T_PROFILE_TITLE);


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
                let qr_value = format!("https://t.me/WoodyWeedBot?start=ref_{}", referral_code);
                let qr_svg = QrCode::new(qr_value.as_bytes())
                    .ok()
                    .map(|code| {
                        let svg_str = code
                            .render::<qrcode::render::svg::Color>()
                            .min_dimensions(200, 200)
                            .dark_color(qrcode::render::svg::Color("#39ff14"))
                            .light_color(qrcode::render::svg::Color("#0f0f1a"))
                            .quiet_zone(false)
                            .build();
                        svg_str
                    })
                    .unwrap_or_default();
                let _qr_title = t(Lang::Russian, T_PROFILE_TITLE);
                let ref_link = format!("https://t.me/WoodyWeedBot?start={}", referral_code);
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
                            div { dangerous_inner_html: "{qr_svg}" }
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
                                    let _ = web_sys::window().map(|_w| {
                                        let share_url = format!(
                                            "https://t.me/share/url?url={}&text={}",
                                            urlencoding::encode(&ref_link_share),
                                            urlencoding::encode("🎁 Get bonus at Woody Weed!")
                                        );
                                        let _ = js_sys::eval(&format!(
                                            "window.open('{}', '_blank')", share_url
                                        ));
                                    });
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
            div { style: "display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; padding: 0 16px 16px;",
                div { style: "text-align: center; padding: 10px 4px; background: #16213e; border: 4px solid #2a2a4a; border-radius: 0; box-shadow: 4px 4px 0 #000;",
                    div { style: "font-size: 20px; font-weight: 800; color: {current_tier.color()}; text-shadow: 2px 2px 0 #000; margin-bottom: 4px;", "฿{total_spent as i32}" }
                    div { style: "font-size: 13px; color: #8b8b9e;", "SPENT" }
                }
                div { style: "text-align: center; padding: 10px 4px; background: #16213e; border: 4px solid #2a2a4a; border-radius: 0; box-shadow: 4px 4px 0 #000;",
                    div { style: "font-size: 20px; font-weight: 800; color: #39ff14; text-shadow: 2px 2px 0 #000; margin-bottom: 4px;", "B{bonus_balance as i32}" }
                    div { style: "font-size: 13px; color: #8b8b9e;", "BONUS" }
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
                            div { style: "font-size: 13px; color: #8b8b9e; margin-top: 6px; text-align: center;",
                                "฿{rem as i32} more to unlock {next.label()}"
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
                    span { style: "font-size: 15px; color: #39ff14; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;", "t.me/WoodyWeedBot?start={referral_code}" }
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
                                    .write_text(&format!("https://t.me/WoodyWeedBot?start={}", referral_code));
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
                                            style: "max-width: 100%; max-height: 100%; object-fit: contain; filter: drop-shadow(0 0 4px {tier.color()}80);"
                                        }
                                    }
                                    div { style: "font-size: 15px; color: {tier.color()}; margin-bottom: 4px;", "{tier.label()}" }
                                    div { style: "font-size: 13px; color: #8b8b9e; margin-bottom: 2px;", "{tier.cashback()}% cashback" }
                                    div { style: "font-size: 15px; color: #8b8b9e;", "฿{tier.threshold() as i32}+" }
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
                { let mut lang = use_language();
                    let current = *lang.read();
                    let langs = vec![
                        (Language::Russian, "\u{1F1F7}\u{1F1FA}", "RU"),
                        (Language::English, "\u{1F1EC}\u{1F1E7}", "EN"),
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
                                            onclick: move |_| {
                                                lang.set(l_c);
                                                #[cfg(target_arch = "wasm32")]
                                                {
                                                    if let Some(window) = web_sys::window() {
                                                        let _ = window.local_storage()
                                                            .ok()
                                                            .flatten()
                                                            .map(|s| s.set_item("wwb_lang", code));
                                                    }
                                                }
                                            },
                                            "{flag} {code}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            BottomNav { cart_count }
        }
    }
}
