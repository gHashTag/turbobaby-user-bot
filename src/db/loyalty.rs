use serde::{Deserialize, Serialize};
use tokio_postgres::Row;

#[allow(dead_code)] // Used for future loyalty profile operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoyaltyProfile {
    pub telegram_id: i64,
    pub total_spent: Option<f64>,
    pub bonus_balance: f64,
    pub tier: String,
    pub referral_code: Option<String>,
    pub referred_by: Option<i64>,
    pub referral_count: i32,
    pub first_purchase_at: Option<chrono::DateTime<chrono::Utc>>,
    pub manager_telegram_id: Option<i64>,
    pub is_blocked: bool,
}

// total_spent / bonus_balance в БД могут быть NUMERIC или DOUBLE PRECISION в зависимости от истории миграций.
// В выборках всегда кастим через ::float8, но ровно на всякий случай используем try_get.
impl LoyaltyProfile {
    #[allow(dead_code)] // Используется в будущих cache invalidation путях
    pub fn from_row(row: &Row) -> Self {
        Self {
            telegram_id: row.try_get("telegram_id").unwrap_or(0),
            total_spent: row.try_get::<_, f64>("total_spent").ok().or_else(|| row.try_get::<_, Option<f64>>("total_spent").ok().flatten()),
            bonus_balance: row.try_get::<_, f64>("bonus_balance").unwrap_or(0.0),
            tier: row.try_get("tier").unwrap_or_default(),
            referral_code: row.try_get("referral_code").ok().flatten(),
            referred_by: row.try_get("referred_by").ok().flatten(),
            referral_count: row.try_get("referral_count").unwrap_or(0),
            first_purchase_at: row.try_get("first_purchase_at").ok().flatten(),
            manager_telegram_id: row.try_get("manager_telegram_id").ok().flatten(),
            is_blocked: row.try_get("is_blocked").unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct BonusTransaction {
    pub id: String,
    pub telegram_id: i64,
    pub amount: f64,
    pub tx_type: String,
    pub description: Option<String>,
    pub related_order_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct LoyaltyConfig {
    pub bronze_threshold: f64,
    pub silver_threshold: f64,
    pub gold_threshold: f64,
    pub bronze_cashback_pct: f64,
    pub silver_cashback_pct: f64,
    pub gold_cashback_pct: f64,
    pub referral_bonus: f64,
    pub max_bonus_usage_pct: f64,
    pub progressive_cashback: Vec<f64>,
    pub happy_hour_start: i32,
    pub happy_hour_end: i32,
    pub happy_hour_enabled: bool,
    pub happy_hour_discount: f64,
}

#[allow(dead_code)]
pub fn calculate_tier(total_spent: f64, order_count: i64, config: &LoyaltyConfig) -> (&'static str, f64) {
    let (tier, tier_pct) = if total_spent >= config.gold_threshold {
        ("gold", config.gold_cashback_pct)
    } else if total_spent >= config.silver_threshold {
        ("silver", config.silver_cashback_pct)
    } else if total_spent >= config.bronze_threshold {
        ("bronze", config.bronze_cashback_pct)
    } else {
        ("none", 0.0)
    };
    let idx = (order_count.saturating_sub(1) as usize).min(config.progressive_cashback.len().saturating_sub(1));
    let progressive_pct = if order_count > 0 {
        config.progressive_cashback.get(idx).copied().unwrap_or(0.0)
    } else { 0.0 };
    (tier, tier_pct.max(progressive_pct))
}
