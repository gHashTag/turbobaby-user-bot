// Profile Page - Shows user membership card with tier (brass/silver/gold)
use dioxus::prelude::*;
use crate::ui::api::types::{LoyaltyProfile, LoyaltyTier};

#[component]
pub fn Profile() -> Element {
    let profile = use_memo(move || {
        LoyaltyProfile {
            telegram_id: 123456,
            points: 150,
            tier: LoyaltyTier::Silver,
            bonuses_used: 5,
            total_spent: 3500.0,
        }
    });

    rsx! {
        div { class: "page profile-page",
            ProfileCard { profile: profile.read().clone(), user_data: None }
        }
    }
}

#[component]
fn ProfileCard(profile: LoyaltyProfile, user_data: Option<serde_json::Value>) -> Element {
    let tier_class = match profile.tier {
        LoyaltyTier::None | LoyaltyTier::Brass | LoyaltyTier::Bronze => "brass",
        LoyaltyTier::Silver => "silver",
        LoyaltyTier::Gold => "gold",
    };

    let tier_name = match profile.tier {
        LoyaltyTier::None | LoyaltyTier::Brass | LoyaltyTier::Bronze => "BRASS",
        LoyaltyTier::Silver => "SILVER",
        LoyaltyTier::Gold => "GOLD",
    };

    let bonus_balance = profile.points as f64;
    let total_spent_raw = if profile.total_spent.is_finite() { profile.total_spent.max(0.0) } else { 0.0 };
    let total_spent = (total_spent_raw * 100.0).round() / 100.0;

    rsx! {
        div { class: "profile-container",
            h1 { class: "profile-title", "👤 Мой профиль" }

            div { class: "member-card member-card-{tier_class}",
                div { class: "card-header",
                    h2 { class: "member-tier", "{tier_name}" }
                    img {
                        src: "/assets/images/member-cards/{tier_class}.PNG",
                        class: "card-bg",
                        alt: "{tier_name} card"
                    }
                }

                div { class: "card-body",
                    div { class: "user-info",
                        div { class: "avatar-placeholder",
                            span { class: "avatar-text", "Г" }
                        }
                        div { class: "user-details",
                            h3 { class: "user-name", "Гость" }
                            p { class: "user-id", "ID: {profile.telegram_id}" }
                        }
                    }

                    div { class: "stats-grid",
                        div { class: "stat-item",
                            span { class: "stat-icon", "🌿" }
                            span { class: "stat-value", "{profile.points}" }
                            span { class: "stat-label", "Очки" }
                        }
                        div { class: "stat-item",
                            span { class: "stat-icon", "💰" }
                            span { class: "stat-value", "{bonus_balance:.0} ₽" }
                            span { class: "stat-label", "Бонусы" }
                        }
                        div { class: "stat-item",
                            span { class: "stat-icon", "🛒" }
                            span { class: "stat-value", "{profile.bonuses_used}" }
                            span { class: "stat-label", "Использовано" }
                        }
                        div { class: "stat-item",
                            span { class: "stat-icon", "💵" }
                            span { class: "stat-value", "{total_spent:.0} ₽" }
                            span { class: "stat-label", "Потрачено" }
                        }
                    }
                }
            }

            div { class: "tier-progress",
                h3 { "Прогресс до следующего уровня" }
                div { class: "progress-bar",
                    div { class: "progress-fill progress-{tier_class}",
                        style: "width: {get_progress_percent(&profile.tier, profile.total_spent)}%"
                    }
                }
                p { class: "progress-label",
                    {get_next_tier_info(&profile.tier, profile.total_spent)}
                }
            }
        }
    }
}

fn get_progress_percent(tier: &LoyaltyTier, spent: f64) -> i32 {
    let safe_spent = if spent.is_finite() { spent.max(0.0) } else { 0.0 };
    let (min, max) = match tier {
        LoyaltyTier::None | LoyaltyTier::Brass | LoyaltyTier::Bronze => (0.0, 5000.0),
        LoyaltyTier::Silver => (5000.0, 20000.0),
        LoyaltyTier::Gold => (20000.0, 100000.0),
    };
    let progress = ((safe_spent - min) / (max - min) * 100.0).min(100.0).max(0.0);
    progress as i32
}

fn get_next_tier_info(tier: &LoyaltyTier, spent: f64) -> String {
    let safe_spent = if spent.is_finite() { spent.max(0.0) } else { 0.0 };
    match tier {
        LoyaltyTier::None | LoyaltyTier::Brass | LoyaltyTier::Bronze => {
            let remaining = (5000.0 - safe_spent).max(0.0) as i64;
            format!("До Silver: {} ₽", remaining)
        }
        LoyaltyTier::Silver => {
            let remaining = (20000.0 - safe_spent).max(0.0) as i64;
            format!("До Gold: {} ₽", remaining)
        }
        LoyaltyTier::Gold => {
            "🏆 Максимальный уровень достигнут!".to_string()
        }
    }
}
