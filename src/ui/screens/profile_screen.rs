use dioxus::prelude::*;
use qrcode::QrCode;
use serde::Deserialize;
use crate::ui::routes::Route;
use crate::ui::state::Cart;
use crate::ui::assets;
use crate::ui::api::context::api_base_url;
use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_PROFILE_TITLE};
use crate::ui::components::bottom_nav::BottomNav;

#[derive(Debug, Clone, Deserialize)]
struct LoyaltyResponse {
    tier: Option<String>,
    bonus_balance: Option<f64>,
    total_spent: Option<f64>,
    cashback_pct: Option<f64>,
    referral_code: Option<String>,
    referral_count: Option<i32>,
    orders_count: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct BonusHistoryResponse {
    transactions: Vec<BonusTx>,
}

#[derive(Debug, Clone, Deserialize)]
struct BonusTx {
    amount: f64,
    description: Option<String>,
    created_at: Option<String>,
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
    fn card_image(&self) -> &'static str {
        match self {
            Self::Starter => assets::member_cards::BRONZE_WEBP,
            Self::Bronze => assets::member_cards::BRONZE_WEBP,
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
    let mut cart = use_context::<Signal<Cart>>();
    let cart_count: u32 = cart.read().items.iter().map(|i| i.quantity).sum();

    let loyalty_resource = use_resource(|| async move {
        let base = api_base_url();
        let url = format!("{}/api/loyalty/me", base);
        let client = reqwest::Client::new();
        let resp = client.get(&url).send().await;
        match resp {
            Ok(r) => r.json::<LoyaltyResponse>().await.ok(),
            Err(_) => None,
        }
    });

    let loyalty_data = loyalty_resource.read().clone().flatten();

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

    let cashback_pct = loyalty_data.as_ref()
        .and_then(|d| d.cashback_pct)
        .unwrap_or_else(|| current_tier.cashback());

    let referral_code = loyalty_data.as_ref()
        .and_then(|d| d.referral_code.clone())
        .unwrap_or("WOODY-DEMO".to_string());

    let referral_count = loyalty_data.as_ref()
        .and_then(|d| d.referral_count)
        .unwrap_or(3);

    let orders_count = loyalty_data.as_ref()
        .and_then(|d| d.orders_count)
        .unwrap_or(24);

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
            font-family: 'Press Start 2P', monospace;
            padding-bottom: 80px;
        ",
            div { style: "padding: 20px 16px 12px; text-align: center;",
                h1 { style: "font-size: 18px; color: #39ff14; text-shadow: 0 0 8px rgba(57,255,20,0.5);", "{profile_title}" }
                p { style: "font-size: 11px; color: #8b8b9e; margin-top: 4px;", "Your membership status" }
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
                        let emoji = tier.emoji();
                        let cb = tier.cashback();
                        let thresh = tier.threshold();
                        let shadow_val = if is_current {
                            format!("0 0 12px {}44", tier.color())
                        } else {
                            "none".to_string()
                        };

                        rsx! {
                            div { style: "
                                background: #1a1a2e; border: 2px solid {border};
                                border-radius: 8px; padding: 10px 8px; min-width: 80px;
                                text-align: center; opacity: {opacity};
                                box-shadow: {shadow_val};
                            ",
                                div { style: "font-size: 10px; margin-bottom: 4px;",
                                    if is_unlocked { "{emoji}" } else { "🔒" }
                                }
                                div { style: "font-size: 18px; color: {tier.color()}; margin-bottom: 2px;", "{label}" }
                                div { style: "font-size: 9px; color: #8b8b9e;", "{cb}% cashback" }
                                if !is_unlocked {
                                    div { style: "font-size: 16px; color: #ff4757; margin-top: 2px;", "฿{thresh}" }
                                }
                            }
                        }
                    }
                }
            }

            // Member card image
            div { style: "max-width: 380px; margin: 0 auto 16px; padding: 0 16px;",
                img {
                    src: "{current_tier.card_image()}",
                    alt: "Member Card",
                    style: "width: 100%; max-width: 320px; border-radius: 16px; display: block; margin: 0 auto; box-shadow: 0 0 24px {current_tier.color()}40;",
                }
            }

            // QR Code Card — matching old repo ProfilePage
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
                let qr_title = t(Lang::Russian, T_PROFILE_TITLE);
                let ref_link = format!("https://t.me/WoodyWeedBot?start={}", referral_code);
                let ref_link_copy = ref_link.clone();
                let ref_link_share = ref_link.clone();
                rsx! {
                    div { style: "
                        margin: 0 16px 16px;
                        background: #1a1a2e; border: 4px solid #39ff14;
                        box-shadow: 4px 4px 0 #000, 0 0 20px rgba(57,255,20,0.1);
                        padding: 28px 16px 24px; text-align: center;
                    ",
                        // QR code with glow border
                        div { style: "
                            display: inline-block;
                            background: #0f0f1a; border: 3px solid #39ff14;
                            border-radius: 8px; padding: 20px;
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
                                    background: #1a1a2e; color: #39ff14;
                                    border: 3px solid #39ff14;
                                    box-shadow: 3px 3px 0 #000;
                                    font-family: 'Press Start 2P', monospace;
                                    font-weight: 700; font-size: 11px;
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
                                    border: 3px solid #2d9e0f;
                                    box-shadow: 3px 3px 0 #000;
                                    font-family: 'Press Start 2P', monospace;
                                    font-weight: 700; font-size: 11px;
                                    cursor: pointer; transition: transform 0.1s, box-shadow 0.1s;
                                ",
                                onclick: move |_| {
                                    let _ = web_sys::window().map(|w| {
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
                            font-size: 11px; color: #8b8b9e; margin-top: 14px;
                        ", "👥 {referral_count} friends invited" }
                    }
                }
            }

            // Stats row
            div { style: "display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; padding: 0 16px 16px;",
                div { style: "text-align: center; padding: 10px 4px; background: #1a1a2e; border: 2px solid #2a2a4a; border-radius: 8px;",
                    div { style: "font-size: 18px; color: {current_tier.color()}; margin-bottom: 4px;", "฿{total_spent as i32}" }
                    div { style: "font-size: 18px; color: #8b8b9e;", "SPENT" }
                }
                div { style: "text-align: center; padding: 10px 4px; background: #1a1a2e; border: 2px solid #2a2a4a; border-radius: 8px;",
                    div { style: "font-size: 18px; color: #39ff14; margin-bottom: 4px;", "B{bonus_balance as i32}" }
                    div { style: "font-size: 18px; color: #8b8b9e;", "BONUS" }
                }
                div { style: "text-align: center; padding: 10px 4px; background: #1a1a2e; border: 2px solid #2a2a4a; border-radius: 8px;",
                    div { style: "font-size: 18px; color: #ffe600; margin-bottom: 4px;", "{cashback_pct as i32}%" }
                    div { style: "font-size: 18px; color: #8b8b9e;", "CASHBACK" }
                }
            }

            // Progress to next tier
            if let Some(next) = next_tier {
                div { style: "padding: 0 16px 16px;",
                    div { style: "
                        background: #1a1a2e; border: 2px solid {next.color()}33;
                        border-radius: 8px; padding: 12px;
                    ",
                        div { style: "display: flex; justify-content: space-between; font-size: 10px; margin-bottom: 6px;",
                            span { style: "color: #8b8b9e;", "Progress to {next.label()}" }
                            span { style: "color: {next.color()};", "{progress_pct}%" }
                        }
                        div { style: "height: 8px; background: rgba(0,0,0,0.4); border-radius: 8px; overflow: hidden;",
                            div { style: "height: 100%; width: {progress_pct}%; border-radius: 8px; background: linear-gradient(90deg, {current_tier.color()}, {next.color()}); transition: width 0.3s;" }
                        }
                        if let Some(rem) = remaining {
                            div { style: "font-size: 18px; color: #8b8b9e; margin-top: 6px; text-align: center;",
                                "฿{rem as i32} more to unlock {next.label()}"
                            }
                        }
                    }
                }
            }

            // Referral section
            div { style: "
                margin: 0 16px 16px;
                background: #1a1a2e; border: 2px solid #2a2a4a;
                border-radius: 8px; padding: 12px;
                box-shadow: 4px 4px 0 #000;
            ",
                div { style: "font-size: 12px; color: #00e5ff; margin-bottom: 8px;", "🔗 Referral Link" }
                div { style: "
                    display: flex; gap: 6px; align-items: center;
                    background: #0f0f1a; border: 2px solid #2a2a4a;
                    border-radius: 8px; padding: 6px 8px; margin-bottom: 8px;
                ",
                    span { style: "font-size: 18px; color: #39ff14; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;", "t.me/WoodyWeedBot?start={referral_code}" }
                    button {
                        style: "
                            font-family: 'Press Start 2P', monospace;
                            font-size: 10px; padding: 4px 8px;
                            background: #00e5ff; color: #0f0f1a;
                            border: none; border-radius: 3px; cursor: pointer;
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
                div { style: "display: flex; gap: 12px; font-size: 10px;",
                    span { style: "color: #8b8b9e;", "👥 {referral_count} invited" }
                    span { style: "color: #39ff14;", "Earn ฿100 per referral" }
                }
            }

            // Quick actions
            div { style: "padding: 0 16px;",
                div { style: "font-size: 12px; color: #00e5ff; margin-bottom: 10px; text-transform: uppercase;", "Quick Actions" }
                Link { to: Route::Orders {},
                    div { style: "
                        background: #1a1a2e; border: 2px solid #2a2a4a;
                        border-radius: 8px; padding: 12px 14px; margin-bottom: 8px;
                        display: flex; justify-content: space-between; align-items: center;
                        box-shadow: 4px 4px 0 #000; cursor: pointer;
                    ",
                        div { style: "display: flex; align-items: center; gap: 10px;",
                            span { style: "font-size: 14px;", "📋" }
                            span { style: "font-size: 12px;", "My Orders" }
                            span { style: "font-size: 18px; color: #8b8b9e;", "({orders_count})" }
                        }
                        span { style: "font-size: 18px; color: #8b8b9e;", "→" }
                    }
                }
                Link { to: Route::Garden {},
                    div { style: "
                        background: #1a1a2e; border: 2px solid #2a2a4a;
                        border-radius: 8px; padding: 12px 14px; margin-bottom: 8px;
                        display: flex; justify-content: space-between; align-items: center;
                        box-shadow: 4px 4px 0 #000; cursor: pointer;
                    ",
                        div { style: "display: flex; align-items: center; gap: 10px;",
                            span { style: "font-size: 14px;", "🌱" }
                            span { style: "font-size: 12px;", "My Garden" }
                        }
                        span { style: "font-size: 18px; color: #8b8b9e;", "→" }
                    }
                }
                Link { to: Route::Quest { id: "daily".to_string() },
                    div { style: "
                        background: #1a1a2e; border: 2px solid #2a2a4a;
                        border-radius: 8px; padding: 12px 14px; margin-bottom: 8px;
                        display: flex; justify-content: space-between; align-items: center;
                        box-shadow: 4px 4px 0 #000; cursor: pointer;
                    ",
                        div { style: "display: flex; align-items: center; gap: 10px;",
                            span { style: "font-size: 14px;", "🎯" }
                            span { style: "font-size: 12px;", "Quests" }
                        }
                        span { style: "font-size: 18px; color: #8b8b9e;", "→" }
                    }
                }
                Link { to: Route::Referrals {},
                    div { style: "
                        background: #1a1a2e; border: 2px solid #2a2a4a;
                        border-radius: 8px; padding: 12px 14px; margin-bottom: 8px;
                        display: flex; justify-content: space-between; align-items: center;
                        box-shadow: 4px 4px 0 #000; cursor: pointer;
                    ",
                        div { style: "display: flex; align-items: center; gap: 10px;",
                            span { style: "font-size: 14px;", "🔗" }
                            span { style: "font-size: 12px;", "Referral Program" }
                        }
                        span { style: "font-size: 18px; color: #8b8b9e;", "→" }
                    }
                }
            }

            // Tier benefits grid
            div { style: "padding: 16px;",
                div { style: "font-size: 12px; color: #ffe600; margin-bottom: 10px; text-transform: uppercase;", "💎 Tier Benefits" }
                div { style: "display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 8px;",
                    for tier in [Tier::Bronze, Tier::Silver, Tier::Gold] {
                        {
                            let is_unlocked = tier.threshold() <= total_spent;
                            let opacity = if is_unlocked { "1" } else { "0.5" };
                            rsx! {
                                div { style: "
                                    background: #1a1a2e; border: 2px solid {tier.color()}33;
                                    border-radius: 8px; padding: 10px 8px; text-align: center;
                                    opacity: {opacity};
                                ",
                                    div { style: "font-size: 12px; margin-bottom: 4px;", "{tier.emoji()}" }
                                    div { style: "font-size: 18px; color: {tier.color()}; margin-bottom: 4px;", "{tier.label()}" }
                                    div { style: "font-size: 9px; color: #8b8b9e; margin-bottom: 2px;", "{tier.cashback()}% cashback" }
                                    div { style: "font-size: 16px; color: #8b8b9e;", "฿{tier.threshold() as i32}+" }
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
