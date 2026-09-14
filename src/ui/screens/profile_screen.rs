use crate::trios::core::Lang;
use crate::trios::i18n::{
    t, tf, T_ORDERS_ORDER, T_ORDERS_STATUS_CANCELLED, T_ORDERS_STATUS_CONFIRMED,
    T_ORDERS_STATUS_DELIVERED, T_ORDERS_STATUS_OUT_FOR_DELIVERY, T_ORDERS_STATUS_PENDING,
    T_ORDERS_STATUS_PREPARING, T_ORDERS_STATUS_READY, T_ORDERS_STATUS_UNKNOWN, T_PROFILE_BONUS,
    T_PROFILE_BONUS_ADMIN, T_PROFILE_BONUS_CASHBACK, T_PROFILE_BONUS_DEBIT, T_PROFILE_BONUS_GARDEN,
    T_PROFILE_BONUS_HISTORY, T_PROFILE_BONUS_HISTORY_EMPTY, T_PROFILE_BONUS_OTHER,
    T_PROFILE_BONUS_REFERRAL, T_PROFILE_CASHBACK_LABEL, T_PROFILE_CONTACTS, T_PROFILE_COPY,
    T_PROFILE_COPY_LINK, T_PROFILE_EARN_PER_REF, T_PROFILE_FRIENDS_INVITED, T_PROFILE_INVITED,
    T_PROFILE_LOAD_ERROR, T_PROFILE_MEMBERSHIP, T_PROFILE_MORE_TO_UNLOCK, T_PROFILE_MY_GARDEN,
    T_PROFILE_MY_ORDERS, T_PROFILE_OPEN_MAP, T_PROFILE_ORDER_HISTORY, T_PROFILE_PROGRESS,
    T_PROFILE_QR_CODE, T_PROFILE_QUESTS, T_PROFILE_QUICK_ACTIONS, T_PROFILE_REFERRAL_LINK,
    T_PROFILE_REFERRAL_PROGRAM, T_PROFILE_REORDER, T_PROFILE_RETRY, T_PROFILE_SHARE,
    T_PROFILE_SPENT, T_PROFILE_STARS, T_PROFILE_TIER_BENEFITS, T_PROFILE_TIER_BRONZE,
    T_PROFILE_TIER_GOLD, T_PROFILE_TIER_SILVER, T_PROFILE_TIER_STARTER, T_PROFILE_TITLE,
};
use crate::ui::api::context::api_base_url;
use crate::ui::api::http::{merge_server_cart, post_client_event};
use crate::ui::assets;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::skeleton::{Skeleton, SkeletonShape};
use crate::ui::lang::{current_lang, set_app_lang};
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem, CartItemType};
use crate::ui::telegram::{
    use_telegram_id, use_telegram_init_data, HapticNotification, TelegramApp,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use dioxus::prelude::*;
use qrcode::QrCode;
use serde::Deserialize;
use web_sys;

#[derive(Debug, Clone, Deserialize)]
struct LoyaltyResponse {
    profile: Option<LoyaltyProfileData>,
    config: Option<LoyaltyConfigData>,
}

// API mirror: profile renders most fields but not referral_count — that
// number lives in invited_count (see its doc comment below).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct LoyaltyProfileData {
    total_spent: Option<f64>,
    tier: Option<String>,
    bonus_balance: Option<f64>,
    referral_code: Option<String>,
    referral_count: Option<i32>,
    /// Everyone who followed the link, whatever they did next.
    ///
    /// `referral_count` is a different number: it is written only when an
    /// invited friend's order completes, so under a label that says *invited*
    /// it read zero for anyone whose friends had not yet bought anything.
    #[serde(default)]
    invited_count: Option<i32>,
    orders_count: Option<i32>,
}

// API mirror — same note as LoyaltyProfileData above.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct LoyaltyConfigData {
    cashback_pct: f64,
    max_bonus_usage_pct: f64,
    next_tier: String,
    next_threshold: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct StarsBalanceResp {
    balance: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct ProfileOrderItem {
    strain_id: Option<String>,
    strain_name: Option<String>,
    accessory_id: Option<String>,
    accessory_name: Option<String>,
    tea_id: Option<String>,
    tea_name: Option<String>,
    set_id: Option<String>,
    set_name: Option<String>,
    quantity: f64,
    #[serde(default)]
    unit_price: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProfileOrder {
    id: String,
    status: String,
    total: f64,
    created_at: String,
    #[serde(default)]
    items: Vec<ProfileOrderItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProfileOrdersResponse {
    orders: Vec<ProfileOrder>,
}

#[derive(Debug, Clone, Deserialize)]
struct BonusHistoryResponse {
    transactions: Vec<BonusTransaction>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // API fields kept for future deep-links / receipts.
struct BonusTransaction {
    id: String,
    amount: f64,
    tx_type: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    related_order_id: Option<String>,
    #[serde(default)]
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
    fn label(&self, lang: Lang) -> String {
        let key = match self {
            Self::Starter => T_PROFILE_TIER_STARTER,
            Self::Bronze => T_PROFILE_TIER_BRONZE,
            Self::Silver => T_PROFILE_TIER_SILVER,
            Self::Gold => T_PROFILE_TIER_GOLD,
        };
        t(lang, key).to_string()
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
    fn rank(&self) -> u8 {
        match self {
            Self::Starter => 0,
            Self::Bronze => 1,
            Self::Silver => 2,
            Self::Gold => 3,
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

fn render_profile_skeleton(_lang: Lang) -> Element {
    rsx! {
        div { style: "padding:0 16px 16px;display:flex;gap:6px;overflow-x:auto;",
            Skeleton { shape: SkeletonShape::AvatarLg }
            Skeleton { shape: SkeletonShape::AvatarLg }
            Skeleton { shape: SkeletonShape::AvatarLg }
            Skeleton { shape: SkeletonShape::AvatarLg }
        }
        div { style: "max-width:380px;margin:0 auto 16px;padding:0 16px;",
            Skeleton { shape: SkeletonShape::MemberCard }
        }
        div { style: "padding:0 16px 16px;",
            div { style: "display:grid;grid-template-columns:repeat(3,1fr);gap:8px;",
                Skeleton { shape: SkeletonShape::Card }
                Skeleton { shape: SkeletonShape::Card }
                Skeleton { shape: SkeletonShape::Card }
            }
        }
        div { style: "padding:0 16px 16px;",
            Skeleton { shape: SkeletonShape::Title, width: Some("60%".into()) }
            div { style: "margin-top:8px;background:#16213e;border:4px solid #2a2a4a;padding:14px;",
                Skeleton { shape: SkeletonShape::Text, width: Some("80%".into()) }
                Skeleton { shape: SkeletonShape::TextSm, width: Some("50%".into()) }
            }
        }
        div { style: "padding:0 16px;display:flex;flex-direction:column;gap:8px;",
            Skeleton { shape: SkeletonShape::Button }
            Skeleton { shape: SkeletonShape::Button }
        }
    }
}

fn profile_order_item_to_cart_item(item: &ProfileOrderItem) -> Option<CartItem> {
    let (id, name, item_type, price_hint) = if let Some(ref sid) = item.strain_id {
        (
            sid.clone(),
            item.strain_name.clone().unwrap_or_else(|| "Strain".into()),
            CartItemType::Strain,
            item.unit_price.unwrap_or(0.0),
        )
    } else if let Some(ref set_id) = item.set_id {
        (
            set_id.clone(),
            item.set_name.clone().unwrap_or_else(|| "Set".into()),
            CartItemType::Set,
            item.unit_price.unwrap_or(0.0),
        )
    } else if let Some(ref aid) = item.accessory_id {
        (
            aid.clone(),
            item.accessory_name
                .clone()
                .unwrap_or_else(|| "Accessory".into()),
            CartItemType::Accessory,
            item.unit_price.unwrap_or(0.0),
        )
    } else if let Some(ref tid) = item.tea_id {
        (
            tid.clone(),
            item.tea_name.clone().unwrap_or_else(|| "Drink".into()),
            CartItemType::Tea,
            item.unit_price.unwrap_or(0.0),
        )
    } else {
        return None;
    };
    Some(CartItem {
        id,
        name,
        price: price_hint,
        quantity: item.quantity.max(1.0) as u32,
        image_url: None,
        item_type,
        fulfillment: None,
    })
}

fn profile_item_name(item: &ProfileOrderItem) -> String {
    item.strain_name
        .clone()
        .or_else(|| item.accessory_name.clone())
        .or_else(|| item.tea_name.clone())
        .or_else(|| item.set_name.clone())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn profile_status_color(status: &str) -> &'static str {
    match status.to_lowercase().as_str() {
        "pending" => "#ffe600",
        "confirmed" => "#00e5ff",
        "preparing" => "#ff9d00",
        "ready" => "#39ff14",
        "out_for_delivery" => "#00e5ff",
        "completed" | "delivered" => "#39ff14",
        "cancelled" | "rejected" => "#ff4757",
        _ => "#8b8b9e",
    }
}

fn profile_status_label_key(status: &str) -> crate::trios::i18n::Key {
    match status.to_lowercase().as_str() {
        "pending" => T_ORDERS_STATUS_PENDING,
        "confirmed" => T_ORDERS_STATUS_CONFIRMED,
        "preparing" => T_ORDERS_STATUS_PREPARING,
        "ready" => T_ORDERS_STATUS_READY,
        "out_for_delivery" => T_ORDERS_STATUS_OUT_FOR_DELIVERY,
        "completed" | "delivered" => T_ORDERS_STATUS_DELIVERED,
        "cancelled" | "rejected" => T_ORDERS_STATUS_CANCELLED,
        _ => T_ORDERS_STATUS_UNKNOWN,
    }
}

fn bonus_tx_label(lang: Lang, tx_type: &str) -> String {
    match tx_type {
        "referral_bonus" => t(lang, T_PROFILE_BONUS_REFERRAL).to_string(),
        "garden_harvest" | "garden_reward" => t(lang, T_PROFILE_BONUS_GARDEN).to_string(),
        "order_cashback" | "cashback" => t(lang, T_PROFILE_BONUS_CASHBACK).to_string(),
        "admin_grant" | "manual_grant" => t(lang, T_PROFILE_BONUS_ADMIN).to_string(),
        "admin_deduction" => t(lang, T_PROFILE_BONUS_DEBIT).to_string(),
        _ => t(lang, T_PROFILE_BONUS_OTHER).to_string(),
    }
}

fn format_bonus_date(iso: Option<&str>) -> String {
    iso.and_then(|s| {
        // Try ISO-8601 timestamp, return "dd.mm.yyyy" in shop timezone.
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.format("%d.%m.%Y").to_string())
    })
    .unwrap_or_default()
}

#[component]
pub fn ProfileScreen() -> Element {
    let cart = use_context::<Signal<Cart>>();
    let cart_count: u32 = cart.read().items.iter().map(|i| i.quantity).sum();

    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_telegram_init_data();
    let init_data_for_loyalty = init_data.clone();
    let init_data_for_stars = init_data.clone();
    let init_data_for_bonus_history = init_data.clone();

    let mut loyalty_retry = use_signal(|| 0u32);
    let loyalty_resource = use_resource(move || {
        let init = init_data_for_loyalty.clone();
        let _ = loyalty_retry();
        async move {
            if telegram_id == 0 {
                return Err("no telegram id".to_string());
            }
            let base = api_base_url();
            let url = format!("{}/api/loyalty/{}", base, telegram_id);
            let client = crate::ui::api::local_client::LocalClient::new();
            match client
                .get(&url)
                .header("X-Telegram-Init-Data", init)
                .send()
                .await
            {
                Ok(r) => match r.json::<LoyaltyResponse>().await {
                    Ok(body) => Ok(body),
                    Err(e) => Err(format!("parse: {e}")),
                },
                Err(e) => Err(format!("network: {e}")),
            }
        }
    });

    let loyalty_result = loyalty_resource
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok().cloned());
    let loyalty_error = loyalty_resource
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().err().cloned());

    let loyalty_data = loyalty_result.as_ref().and_then(|r| r.profile.clone());

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
    let stars_balance = (*stars_balance_res.read()).flatten().unwrap_or(0);

    let bonus_history_res = use_resource(move || {
        let init = init_data_for_bonus_history.clone();
        async move {
            if telegram_id == 0 {
                return None;
            }
            let url = format!(
                "{}/api/loyalty/{}/bonus-history",
                api_base_url(),
                telegram_id
            );
            let client = crate::ui::api::local_client::LocalClient::new();
            let resp = client
                .get(&url)
                .header("X-Telegram-Init-Data", init)
                .send()
                .await;
            match resp {
                Ok(r) => r
                    .json::<BonusHistoryResponse>()
                    .await
                    .ok()
                    .map(|r| r.transactions),
                Err(_) => None,
            }
        }
    });
    let bonus_history = bonus_history_res
        .read()
        .clone()
        .flatten()
        .unwrap_or_default();

    // Loop #16: recent orders mini-list for the profile screen.
    let init_data_for_orders = init_data.clone();
    let orders_resource = use_resource(move || {
        let init = init_data_for_orders.clone();
        async move {
            if telegram_id == 0 {
                return None;
            }
            let url = format!("{}/api/orders/user/{}", api_base_url(), telegram_id);
            let client = crate::ui::api::local_client::LocalClient::new();
            let resp = client
                .get(&url)
                .header("X-Telegram-Init-Data", init)
                .send()
                .await;
            match resp {
                Ok(r) => r
                    .json::<ProfileOrdersResponse>()
                    .await
                    .ok()
                    .map(|r| r.orders),
                Err(_) => None,
            }
        }
    });
    let recent_orders = orders_resource.read().clone().flatten().unwrap_or_default();

    let backend_tier = loyalty_data
        .as_ref()
        .and_then(|d| d.tier.as_deref())
        .map(Tier::from_str);
    let current_tier = backend_tier.unwrap_or(Tier::Starter);

    let total_spent = loyalty_data
        .as_ref()
        .and_then(|d| d.total_spent)
        .filter(|v| v.is_finite())
        .unwrap_or(0.0)
        .max(0.0);

    let bonus_balance = loyalty_data
        .as_ref()
        .and_then(|d| d.bonus_balance)
        .filter(|v| v.is_finite())
        .unwrap_or(0.0)
        .max(0.0);

    // Single source of truth for the ฿ stat (was an inline `as i32` narrowing).
    let total_spent_str = crate::trios::pricing::format_baht(total_spent);

    // Loop #14: cashback and next-tier threshold now come from the same
    // loyalty_config the backend uses to credit cashback on order completion.
    // The Tier enum is kept only for card art / labels.
    let config = loyalty_result.as_ref().and_then(|r| r.config.clone());
    let cashback_pct = config
        .as_ref()
        .map(|c| c.cashback_pct)
        .filter(|v| v.is_finite() && *v >= 0.0)
        .unwrap_or_else(|| current_tier.cashback());

    // Cycle #169E: fallback to raw telegram_id instead of stale "WOODY-DEMO"
    // placeholder so the referral deep-link always carries the user's
    // actual identifier even if the loyalty_profile row hasn't been
    // seeded with a referral_code yet.
    let referral_code = loyalty_data
        .as_ref()
        .and_then(|d| d.referral_code.clone())
        .unwrap_or_else(|| telegram_id.to_string());

    // The label says "invited", so the number has to be the invited one.
    let invited_count = loyalty_data
        .as_ref()
        .and_then(|d| d.invited_count)
        .unwrap_or(0);

    let orders_count = loyalty_data
        .as_ref()
        .and_then(|d| d.orders_count)
        .unwrap_or(0);

    // Loop #14: next-tier threshold from backend config so the progress bar
    // matches the real loyalty program rules instead of hardcoded UI values.
    // The next tier label/color still comes from the Tier enum for card art.
    let next_tier = current_tier.next();
    let next_threshold = config
        .as_ref()
        .map(|c| c.next_threshold)
        .filter(|v| v.is_finite() && *v > 0.0)
        .or_else(|| next_tier.map(|t| t.threshold()));
    let progress_pct = if let Some(threshold) = next_threshold {
        ((total_spent / threshold) * 100.0).min(100.0) as i32
    } else {
        100
    };
    let remaining = next_threshold.map(|t| (t - total_spent).max(0.0));

    let lang = crate::ui::lang::current_lang();
    let profile_title = t(lang, T_PROFILE_TITLE);
    let is_loading = loyalty_resource.read().is_none();

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: calc(96px + env(safe-area-inset-bottom));
        ",
            div { style: "padding: 20px 16px 16px; text-align: center;",
                h1 { style: "font-size: 24px; font-weight: 800; color: #39ff14; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(57,255,20,0.5); letter-spacing: 2px;", "{profile_title}" }
                p { style: "font-size: 13px; color: #8b8b9e; margin-top: 4px;", "{t(lang, T_PROFILE_MEMBERSHIP)}" }
            }

            if is_loading {
                { render_profile_skeleton(lang) }
            } else if let Some(ref _err) = loyalty_error {
                div { style: "padding: 0 16px 16px; text-align: center;",
                    div { style: "background: #16213e; border: 4px solid #ff4757; padding: 14px; color: #ff4757; font-size: 14px; margin-bottom: 12px; box-shadow: 4px 4px 0 #000;",
                        "{t(lang, T_PROFILE_LOAD_ERROR)}"
                    }
                    button {
                        style: "font-size: 14px; font-weight: 700; padding: 12px 20px; background: #39ff14; color: #000; border: 4px solid #2d9e0f; cursor: pointer; box-shadow: 3px 3px 0 #000;",
                        onclick: move |_| { loyalty_retry.set(loyalty_retry() + 1); },
                        "{t(lang, T_PROFILE_RETRY)}"
                    }
                }
            } else {
            // Tier strip — all 4 tiers
            div { style: "display: flex; gap: 6px; padding: 0 16px 16px; overflow-x: auto;",
                for tier in Tier::all() {
                    {
                        let is_current = *tier == current_tier;
                        // Loop #15: when the backend tells us the user's current tier,
                        // trust its rank; fall back to the local threshold only when the
                        // loyalty row is missing (e.g. first-time visitor).
                        let is_unlocked = backend_tier.map_or(
                            tier.threshold() <= total_spent,
                            |bt| bt.rank() >= tier.rank(),
                        );
                        let border = if is_current { tier.color() } else { "#2a2a4a" };
                        let opacity = if is_unlocked { "1.0" } else { "0.4" };
                        let label = tier.label(lang);
                        let _emoji = tier.emoji();
                        let cb = tier.cashback();
                        // For the next tier, show the backend threshold if available so
                        // the progress bar matches the real rules; otherwise use the enum.
                        let next_tier = current_tier.next();
                        let thresh = if Some(*tier) == next_tier {
                            config
                                .as_ref()
                                .map(|c| c.next_threshold)
                                .filter(|v| v.is_finite() && *v > 0.0)
                                .unwrap_or_else(|| tier.threshold())
                        } else {
                            tier.threshold()
                        };
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
                                            alt: "{tier.label(lang)}",
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
                let bot_user = TelegramApp::init().bot_username().unwrap_or_else(|| "Woody_WeedPecker_bot".to_string());
                let qr_value = format!("https://t.me/{bot_user}?start=ref_{referral_code}");
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
                let ref_link = format!("https://t.me/{bot_user}?start=ref_{referral_code}");
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
                                alt: "{t(lang, T_PROFILE_QR_CODE)}"
                            }
                        }
                        div { style: "
                            font-size: 14px; font-weight: 800; color: #39ff14;
                            margin-top: 16px; margin-bottom: 16px;
                            text-shadow: 2px 2px 0 #000;
                        ", "{t(lang, T_PROFILE_QR_CODE)}" }
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
                                "{t(lang, T_PROFILE_COPY_LINK)}"
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
                                "{t(lang, T_PROFILE_SHARE)}"
                            }
                        }
                        div { style: "
                            font-size: 13px; color: #8b8b9e; margin-top: 14px;
                        ", "{tf(lang, T_PROFILE_FRIENDS_INVITED, &[invited_count.to_string()])}" }
                    }
                }
            }

            // Stats row
            div { style: "display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; padding: 0 16px 16px;",
                div { style: "text-align: center; padding: 10px 4px; background: #16213e; border: 4px solid #2a2a4a; border-radius: 0; box-shadow: 4px 4px 0 #000;",
                    div { style: "font-size: 20px; font-weight: 800; color: {current_tier.color()}; text-shadow: 2px 2px 0 #000; margin-bottom: 4px;", "{total_spent_str}" }
                    div { style: "font-size: 13px; color: #8b8b9e;", "{t(lang, T_PROFILE_SPENT)}" }
                }
                div { style: "text-align: center; padding: 10px 4px; background: #16213e; border: 4px solid #2a2a4a; border-radius: 0; box-shadow: 4px 4px 0 #000;",
                    div { style: "font-size: 20px; font-weight: 800; color: #39ff14; text-shadow: 2px 2px 0 #000; margin-bottom: 4px;", "B{bonus_balance as i32}" }
                    div { style: "font-size: 13px; color: #8b8b9e;", "{t(lang, T_PROFILE_BONUS)}" }
                }
                div { style: "text-align: center; padding: 10px 4px; background: #16213e; border: 4px solid #2a2a4a; border-radius: 0; box-shadow: 4px 4px 0 #000;",
                    div { style: "font-size: 20px; font-weight: 800; color: #7dd3fc; text-shadow: 2px 2px 0 #000; margin-bottom: 4px;", "⭐{stars_balance}" }
                    div { style: "font-size: 13px; color: #8b8b9e;", "{t(lang, T_PROFILE_STARS)}" }
                }
                div { style: "text-align: center; padding: 10px 4px; background: #16213e; border: 4px solid #2a2a4a; border-radius: 0; box-shadow: 4px 4px 0 #000;",
                    div { style: "font-size: 20px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000; margin-bottom: 4px;", "{cashback_pct as i32}%" }
                    div { style: "font-size: 13px; color: #8b8b9e;", "{t(lang, T_PROFILE_CASHBACK_LABEL)}" }
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
                            span { style: "color: #8b8b9e;", "{tf(lang, T_PROFILE_PROGRESS, &[next.label(lang).to_string()])}" }
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
                                        "{tf(lang, T_PROFILE_MORE_TO_UNLOCK, &[rem_str.clone(), next.label(lang).to_string()])}"
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
                div { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 8px;", "{t(lang, T_PROFILE_REFERRAL_LINK)}" }
                div { style: "
                    display: flex; gap: 6px; align-items: center;
                    background: #0f0f1a; border: 4px solid #2a2a4a;
                    border-radius: 0; padding: 6px 8px; margin-bottom: 8px;
                ",
                    {
                        let bot_user2 = TelegramApp::init().bot_username().unwrap_or_else(|| "Woody_WeedPecker_bot".to_string());
                        rsx! {
                            span { style: "font-size: 15px; color: #39ff14; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;", "t.me/{bot_user2}?start=ref_{referral_code}" }
                        }
                    }
                    button {
                        style: "
                            font-size: 13px; padding: 4px 8px;
                            background: #00e5ff; color: #000;
                            border: 4px solid #00b8d4; border-radius: 0; cursor: pointer;
                            box-shadow: 3px 3px 0 #000;
                        ",
                        onclick: move |_| {
                            let bot_user = TelegramApp::init().bot_username().unwrap_or_else(|| "Woody_WeedPecker_bot".to_string());
                            let _ = web_sys::window().map(|w| {
                                let _ = w.navigator().clipboard()
                                    .write_text(&format!("https://t.me/{bot_user}?start=ref_{referral_code}"));
                            });
                        },
                        "{t(lang, T_PROFILE_COPY)}"
                    }
                }
                div { style: "display: flex; gap: 12px; font-size: 13px;",
                    span { style: "color: #8b8b9e;", "{tf(lang, T_PROFILE_INVITED, &[invited_count.to_string()])}" }
                    span { style: "color: #39ff14;", "{t(lang, T_PROFILE_EARN_PER_REF)}" }
                }
            }

            // Quick actions
            div { style: "padding: 0 16px;",
                div { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{t(lang, T_PROFILE_QUICK_ACTIONS)}" }
                Link { to: Route::Orders {},
                    div { style: "
                        background: #16213e; border: 4px solid #2a2a4a;
                        border-radius: 0; padding: 12px 14px; margin-bottom: 8px;
                        display: flex; justify-content: space-between; align-items: center;
                        box-shadow: 4px 4px 0 #000; cursor: pointer;
                    ",
                        div { style: "display: flex; align-items: center; gap: 10px;",
                            span { style: "font-size: 14px;", "📋" }
                            span { style: "font-size: 15px;", "{t(lang, T_PROFILE_MY_ORDERS)}" }
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
                            span { style: "font-size: 15px;", "{t(lang, T_PROFILE_MY_GARDEN)}" }
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
                            span { style: "font-size: 15px;", "{t(lang, T_PROFILE_QUESTS)}" }
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
                            span { style: "font-size: 15px;", "{t(lang, T_PROFILE_REFERRAL_PROGRAM)}" }
                        }
                        span { style: "font-size: 15px; color: #8b8b9e;", "→" }
                    }
                }
            }

            // Loop #16: recent order history mini-list
            div { style: "padding: 0 16px 16px;",
                div { style: "font-size: 13px; font-weight: 700; color: #ffe600; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{t(lang, T_PROFILE_ORDER_HISTORY)}" }
                div { style: "background: #16213e; border: 4px solid #2a2a4a; box-shadow: 4px 4px 0 #000; padding: 12px;",
                    if recent_orders.is_empty() {
                        div { style: "font-size: 13px; color: #8b8b9e; text-align: center; padding: 8px 0;", "{t(lang, T_PROFILE_BONUS_HISTORY_EMPTY)}" }
                    } else {
                        div { style: "display: flex; flex-direction: column; gap: 10px;",
                            for order in recent_orders.iter().take(3) {{
                                let o = order.clone();
                                let short_id: String = o.id.chars().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect();
                                let status_color = profile_status_color(&o.status);
                                let status_label = t(lang, profile_status_label_key(&o.status));
                                let total_str = crate::trios::pricing::format_baht(o.total);
                                let date_str = o.created_at.split('T').next().unwrap_or(&o.created_at).to_string();
                                let item_summary = o.items.first().map(profile_item_name).unwrap_or_else(|| "—".to_string());
                                let more_count = o.items.len().saturating_sub(1);
                                let init_for_reorder = init_data.clone();
                                let cart_for_reorder = cart;
                                let nav_for_reorder = navigator();
                                let reorder_label = t(lang, T_PROFILE_REORDER).to_string();
                                rsx! {
                                    div { style: "background: #0f0f1a; border: 3px solid {status_color}; box-shadow: 2px 2px 0 #000; padding: 10px 12px;",
                                        div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;",
                                            span { style: "font-size: 14px; font-weight: 700; color: #e8e8e8;", "{tf(lang, T_ORDERS_ORDER, std::slice::from_ref(&short_id))}" }
                                            span { style: "font-size: 12px; padding: 2px 6px; background: {status_color}22; color: {status_color};", "{status_label}" }
                                        }
                                        div { style: "font-size: 12px; color: #8b8b9e; margin-bottom: 8px;",
                                            if more_count == 0 {
                                                "{item_summary} · {date_str}"
                                            } else {
                                                "{item_summary} +{more_count} · {date_str}"
                                            }
                                        }
                                        div { style: "display: flex; justify-content: space-between; align-items: center;",
                                            span { style: "font-size: 16px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000;", "{total_str}" }
                                            button {
                                                style: "font-size: 12px; font-weight: 700; padding: 6px 10px; background: #39ff14; color: #000; border: 3px solid #2d9e0f; box-shadow: 2px 2px 0 #000; cursor: pointer;",
                                                onclick: move |_| {
                                                    let order_items = o.items.clone();
                                                    let init = init_for_reorder.clone();
                                                    let tid = telegram_id;
                                                    let mut cart_sig = cart_for_reorder;
                                                    let nav = nav_for_reorder;
                                                    spawn(async move {
                                                        let local_items: Vec<CartItem> = order_items.iter().filter_map(profile_order_item_to_cart_item).collect();
                                                        match merge_server_cart(&api_base_url(), &init, tid, &local_items).await {
                                                            Ok(fresh_cart) => { cart_sig.set(fresh_cart); }
                                                            Err(_) => {
                                                                let mut local = Cart::new();
                                                                for item in local_items { local.add_item(item); }
                                                                cart_sig.set(local);
                                                            }
                                                        }
                                                        let _ = post_client_event(&api_base_url(), "reorder_clicked", "profile").await;
                                                        TelegramApp::init().haptic_notification(HapticNotification::Success);
                                                        nav.push(Route::Cart {});
                                                    });
                                                },
                                                "{reorder_label}"
                                            }
                                        }
                                    }
                                }
                            }}
                        }
                    }
                }
            }

            // Bonus transaction history
            div { style: "padding: 0 16px 16px;",
                div { style: "font-size: 13px; font-weight: 700; color: #ffe600; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{t(lang, T_PROFILE_BONUS_HISTORY)}" }
                div { style: "background: #16213e; border: 4px solid #2a2a4a; box-shadow: 4px 4px 0 #000; padding: 12px;",
                    if bonus_history.is_empty() {
                        div { style: "font-size: 13px; color: #8b8b9e; text-align: center; padding: 8px 0;", "{t(lang, T_PROFILE_BONUS_HISTORY_EMPTY)}" }
                    } else {
                        div { style: "display: flex; flex-direction: column; gap: 8px;",
                            for tx in bonus_history.iter().take(20) {
                                {
                                    let amount = if tx.amount.is_finite() { tx.amount } else { 0.0 };
                                    let is_credit = amount >= 0.0;
                                    let sign = if is_credit { "+" } else { "" };
                                    let color = if is_credit { "#39ff14" } else { "#ff4757" };
                                    let label = bonus_tx_label(lang, &tx.tx_type);
                                    let date = format_bonus_date(tx.created_at.as_deref());
                                    let amount_str = crate::trios::pricing::format_baht(amount.abs());
                                    let desc = tx.description.as_deref().unwrap_or("");
                                    rsx! {
                                        div { style: "display: flex; justify-content: space-between; align-items: flex-start; gap: 8px;",
                                            div { style: "min-width: 0;",
                                                div { style: "font-size: 13px; font-weight: 700; color: #e8e8e8;", "{label}" }
                                                if !desc.is_empty() {
                                                    div { style: "font-size: 11px; color: #8b8b9e; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;", "{desc}" }
                                                }
                                                if !date.is_empty() {
                                                    div { style: "font-size: 11px; color: #6b6b7e; margin-top: 2px;", "{date}" }
                                                }
                                            }
                                            div { style: "font-size: 14px; font-weight: 800; color: {color}; white-space: nowrap;", "{sign}{amount_str}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Tier benefits grid
            div { style: "padding: 16px;",
                div { style: "font-size: 13px; font-weight: 700; color: #ffe600; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{t(lang, T_PROFILE_TIER_BENEFITS)}" }
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
                                            alt: "{tier.label(lang)}",
                                            style: "max-width: 100%; max-height: 100%; object-fit: contain; filter: drop-shadow(0 0 4px {tier.color()}80);"
                                        }
                                    }
                                    div { style: "font-size: 15px; color: {tier.color()}; margin-bottom: 4px;", "{tier.label(lang)}" }
                                    div { style: "font-size: 13px; color: #8b8b9e; margin-bottom: 2px;", "{tier.cashback()}% cashback" }
                                    {
                                        let threshold_str = crate::trios::pricing::format_baht(tier.threshold());
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
                div { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{t(lang, T_PROFILE_CONTACTS)}" }
                div { style: "font-size: 15px; margin-bottom: 2px;", "Woody Weed Pecker" }
                div { style: "font-size: 13px; color: #8b8b9e; margin-bottom: 8px;", "Koh Phangan, Thailand" }
                div {
                    style: "cursor: pointer; font-size: 13px; color: #00e5ff; text-decoration: underline;",
                    onclick: move |_| {
                        let _ = web_sys::window().and_then(|w| w.open_with_url_and_target("https://www.google.com/maps/place/Woody+Weed+Pecker/@9.7124562,99.9877309,17z/data=!3m1!4b1!4m6!3m5!1s0x3054ffe9f6df4edf:0xf8735a84f5193e1a!8m2!3d9.7124562!4d99.9877309!16s%2Fg%2F11x314fym6!18m1!1e1?entry=ttu&g_ep=EgoyMDI2MDUxMy4wIKXMDSoASAFQAw%3D%3D", "_blank").ok());
                    },
                    "{t(lang, T_PROFILE_OPEN_MAP)}"
                }
            }
            }

            BottomNav { cart_count }
        }
    }
}
