use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_postgres::Row;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub telegram_id: Option<i64>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub customer_telegram: Option<String>,
    pub items: Value,
    pub subtotal: f64,
    pub bonus_used: f64,
    pub total: f64,
    pub status: String,
    pub shop_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Order {
    pub fn from_row(row: &Row) -> Self {
        Self {
            id: row.try_get("id").unwrap_or_default(),
            telegram_id: row.try_get("telegram_id").ok().flatten(),
            customer_name: row.try_get("customer_name").ok().flatten(),
            customer_phone: row.try_get("customer_phone").ok().flatten(),
            customer_telegram: row.try_get("customer_telegram").ok().flatten(),
            items: row.try_get("items").unwrap_or(Value::Null),
            subtotal: {
                let v = row.try_get::<_, f64>("subtotal").unwrap_or(0.0);
                if v.is_finite() {
                    v.max(0.0)
                } else {
                    0.0
                }
            },
            bonus_used: {
                let v = row.try_get::<_, f64>("bonus_used").unwrap_or(0.0);
                if v.is_finite() {
                    v.max(0.0)
                } else {
                    0.0
                }
            },
            total: {
                let v = row.try_get::<_, f64>("total").unwrap_or(0.0);
                if v.is_finite() {
                    v.max(0.0)
                } else {
                    0.0
                }
            },
            status: row.try_get("status").unwrap_or_default(),
            shop_id: row.try_get("shop_id").ok().flatten(),
            created_at: row
                .try_get("created_at")
                .unwrap_or_else(|_| chrono::Utc::now()),
        }
    }
}

// Wave 5: SeaORM Entity API path. Старый tokio-postgres код выше — будем удалять в Wave 6+.

/// Returns all orders for a given telegram_id via SeaORM Entity API.
#[allow(dead_code)]
pub async fn get_user_orders_seaorm(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
) -> Result<Vec<crate::db::entities::order::Model>, sea_orm::DbErr> {
    use crate::db::entities::order::{Column, Entity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    Entity::find()
        .filter(Column::TelegramId.eq(telegram_id))
        .all(orm)
        .await
}

/// Atomically mark an order as completed and update the customer's loyalty profile.
/// Returns `Some((telegram_id, is_first_order))` if the order was newly completed,
/// or `None` if it was already completed (idempotent).
pub async fn complete_order_and_update_loyalty(
    pool: &deadpool_postgres::Pool,
    order_id: &str,
) -> Result<Option<(i64, bool)>, Box<dyn std::error::Error + Send + Sync>> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;

    let order_row = tx
        .query_opt(
            "SELECT telegram_id, total::float8, status FROM orders WHERE id = $1 FOR UPDATE",
            &[&order_id],
        )
        .await?;

    let result = if let Some(row) = order_row {
        let cid: Option<i64> = row.try_get("telegram_id").ok().flatten();
        let total: f64 = {
            let v = row.try_get::<_, f64>("total").unwrap_or(0.0);
            if v.is_finite() {
                v.max(0.0)
            } else {
                0.0
            }
        };
        let status: String = row.try_get("status").unwrap_or_default();
        if status != "completed" {
            let loyalty_result = if let Some(cid) = cid {
                // Count BEFORE updating so is_first is accurate
                let count_before = tx.query_one(
                    "SELECT COUNT(*) as cnt FROM orders WHERE telegram_id = $1 AND status = 'completed' AND id != $2",
                    &[&cid, &order_id],
                ).await?.try_get::<_, i64>("cnt").unwrap_or(0);
                let is_first = count_before == 0;

                tx.execute(
                    "UPDATE orders SET status = 'completed' WHERE id = $1",
                    &[&order_id],
                )
                .await?;

                tx.execute(
                    "INSERT INTO loyalty_profiles (telegram_id, total_spent, first_purchase_at) VALUES ($1, $2, NOW())
                     ON CONFLICT (telegram_id) DO UPDATE SET
                       total_spent = COALESCE(loyalty_profiles.total_spent, 0) + EXCLUDED.total_spent,
                       first_purchase_at = COALESCE(loyalty_profiles.first_purchase_at, NOW())",
                    &[&cid, &total],
                ).await?;

                tx.execute(
                    "UPDATE loyalty_profiles SET tier = CASE
                        WHEN loyalty_profiles.total_spent >= (SELECT (config->>'gold_threshold')::float8 FROM loyalty_config WHERE id = 1 LIMIT 1) THEN 'gold'
                        WHEN loyalty_profiles.total_spent >= (SELECT (config->>'silver_threshold')::float8 FROM loyalty_config WHERE id = 1 LIMIT 1) THEN 'silver'
                        WHEN loyalty_profiles.total_spent >= (SELECT (config->>'bronze_threshold')::float8 FROM loyalty_config WHERE id = 1 LIMIT 1) THEN 'bronze'
                        ELSE 'none'
                     END
                     WHERE telegram_id = $1",
                    &[&cid],
                ).await?;

                Some((cid, is_first))
            } else {
                None
            };
            loyalty_result
        } else {
            None
        }
    } else {
        None
    };

    tx.commit().await?;
    Ok(result)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItem {
    pub strain_id: Option<String>,
    pub strain_name: Option<String>,
    pub accessory_id: Option<String>,
    pub accessory_name: Option<String>,
    pub tea_id: Option<String>,
    pub tea_name: Option<String>,
    pub set_id: Option<String>,
    pub set_name: Option<String>,
    pub quantity: f64,
    pub is_set: Option<bool>,
    pub is_accessory: Option<bool>,
    pub is_tea: Option<bool>,
    pub is_tea_set: Option<bool>,
}

// ─── Idempotency-key TTL sweep (cycle #58 / A) ────────────────────────────
//
// migration 029 (`order_idempotency_keys`) is append-only. Without a sweep
// the table grows unbounded; Stripe / AWS / GCP all use a 24 h window for
// the same reason. Real client retry windows are far shorter than that —
// 24 h is the safe default. The pure SQL builder is extracted so a typo in
// the INTERVAL literal cannot slip through to production unnoticed.

/// Build the `DELETE` SQL fragment for the idempotency-key TTL sweep.
/// Extracted as a pure function so the literal is unit-testable; otherwise
/// a typo in `INTERVAL '24 hours'` would only show up in production.
pub(crate) fn idempotency_sweep_sql(retention_hours: u32) -> String {
    format!(
        "DELETE FROM order_idempotency_keys \
         WHERE created_at < NOW() - INTERVAL '{} hours'",
        retention_hours
    )
}

/// Delete `order_idempotency_keys` rows older than `retention_hours`.
/// Returns the number of rows deleted (for metrics / structured logs).
///
/// Idempotent and safe to run concurrently with `create_order` — Postgres
/// handles concurrent DELETE/INSERT on the same table cleanly, and the
/// 24 h cutoff is far older than any in-flight order's retry window.
pub async fn cleanup_old_idempotency_keys(
    pool: &deadpool_postgres::Pool,
    retention_hours: u32,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let client = pool.get().await?;
    let sql = idempotency_sweep_sql(retention_hours);
    let deleted = client.execute(sql.as_str(), &[]).await?;
    Ok(deleted)
}

// ─── Fraud-event audit log (cycle #59) ────────────────────────────────────
//
// Every 422 reject in `create_order` emits a structured `tracing::warn!`,
// but those lines live in stdout — invisible to an admin who only opens
// the Telegram bot. This append-only table mirrors the same data so the
// `/engage` panel can show "Suspicious activity (24h)" without grep.
//
// Inserts are best-effort: a failure here MUST NOT block the reject path
// the customer is already seeing. Worst case we lose a row, not a request.

/// Stable codes for `order_fraud_events.code`. They mirror the JSON error
/// codes the client UI may eventually parse for friendly messages.
pub const FRAUD_CODE_SUBTOTAL_MISMATCH: &str = "subtotal_mismatch";
pub const FRAUD_CODE_UNKNOWN_ITEM: &str = "unknown_item";
pub const FRAUD_CODE_UNAVAILABLE: &str = "unavailable";
pub const FRAUD_CODE_MALFORMED: &str = "malformed";

/// Insert one row into `order_fraud_events`. Returns `Result<(), ...>` so
/// the caller can log a warning, but the caller MUST NOT propagate — the
/// 422 reject path is more important than the audit row landing.
pub async fn record_fraud_event(
    pool: &deadpool_postgres::Pool,
    telegram_id: Option<i64>,
    code: &str,
    catalog: Option<&str>,
    item_id: Option<&str>,
    claimed_subtotal: Option<f64>,
    expected_subtotal: Option<f64>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = pool.get().await?;
    client
        .execute(
            "INSERT INTO order_fraud_events \
             (telegram_id, code, catalog, item_id, claimed_subtotal, expected_subtotal) \
             VALUES ($1, $2, $3, $4, $5, $6)",
            &[
                &telegram_id,
                &code,
                &catalog,
                &item_id,
                &claimed_subtotal,
                &expected_subtotal,
            ],
        )
        .await?;
    Ok(())
}

/// 24-hour aggregate for the `/engage` fraud panel. Keep this struct narrow:
/// /engage's text rendering reads each field once.
#[derive(Debug, Default, Clone)]
pub struct FraudStats24h {
    pub subtotal_mismatch: i64,
    pub unknown_item: i64,
    pub unavailable: i64,
    pub malformed: i64,
    /// telegram_id with the most events in the window. None if no events
    /// had a telegram_id (anonymous-only). String to allow easy `format!`
    /// without an extra cast.
    pub top_offender: Option<String>,
    pub top_offender_count: i64,
}

/// One round-trip aggregate over `order_fraud_events` for the last 24h.
pub async fn fraud_stats_24h(
    pool: &deadpool_postgres::Pool,
) -> Result<FraudStats24h, Box<dyn std::error::Error + Send + Sync>> {
    let client = pool.get().await?;
    // Per-code counts. COUNT(*) FILTER (...) keeps the whole thing in one
    // index scan over `idx_fraud_events_created_at`.
    let row = client
        .query_one(
            "SELECT \
                COUNT(*) FILTER (WHERE code = 'subtotal_mismatch')::bigint AS subtotal_mismatch, \
                COUNT(*) FILTER (WHERE code = 'unknown_item')::bigint        AS unknown_item, \
                COUNT(*) FILTER (WHERE code = 'unavailable')::bigint         AS unavailable, \
                COUNT(*) FILTER (WHERE code = 'malformed')::bigint           AS malformed \
             FROM order_fraud_events WHERE created_at > NOW() - INTERVAL '24 hours'",
            &[],
        )
        .await?;
    let mut s = FraudStats24h {
        subtotal_mismatch: row.try_get("subtotal_mismatch").unwrap_or(0),
        unknown_item: row.try_get("unknown_item").unwrap_or(0),
        unavailable: row.try_get("unavailable").unwrap_or(0),
        malformed: row.try_get("malformed").unwrap_or(0),
        top_offender: None,
        top_offender_count: 0,
    };
    // Top offender — separate cheap query because it's bounded LIMIT 1.
    if let Ok(Some(top)) = client
        .query_opt(
            "SELECT telegram_id::text AS tid, COUNT(*)::bigint AS n \
             FROM order_fraud_events \
             WHERE created_at > NOW() - INTERVAL '24 hours' \
               AND telegram_id IS NOT NULL \
             GROUP BY telegram_id ORDER BY n DESC LIMIT 1",
            &[],
        )
        .await
    {
        s.top_offender = top.try_get::<_, Option<String>>("tid").ok().flatten();
        s.top_offender_count = top.try_get("n").unwrap_or(0);
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::{idempotency_sweep_sql, OrderItem};

    #[test]
    fn idempotency_sweep_uses_correct_interval_literal() {
        let sql = idempotency_sweep_sql(24);
        assert!(sql.contains("DELETE FROM order_idempotency_keys"));
        assert!(sql.contains("INTERVAL '24 hours'"));
    }

    #[test]
    fn idempotency_sweep_accepts_arbitrary_retention() {
        // Cycle ships with 24h, but the builder must work for any value
        // (staging / tests may want a shorter window).
        let sql = idempotency_sweep_sql(1);
        assert!(sql.contains("INTERVAL '1 hours'"));
    }

    #[test]
    fn test_order_item_serde_roundtrip() {
        let item = OrderItem {
            strain_id: Some("s1".into()),
            strain_name: Some("Indica".into()),
            accessory_id: None,
            accessory_name: None,
            tea_id: None,
            tea_name: None,
            set_id: None,
            set_name: None,
            quantity: 2.5,
            is_set: Some(false),
            is_accessory: None,
            is_tea: None,
            is_tea_set: None,
        };
        let json = serde_json::to_value(&item).unwrap();
        let back: OrderItem = serde_json::from_value(json).unwrap();
        assert_eq!(back.strain_id, Some("s1".into()));
        assert_eq!(back.quantity, 2.5);
    }

    #[test]
    fn test_order_item_defaults() {
        let item = OrderItem {
            strain_id: None,
            strain_name: None,
            accessory_id: None,
            accessory_name: None,
            tea_id: None,
            tea_name: None,
            set_id: None,
            set_name: None,
            quantity: 1.0,
            is_set: None,
            is_accessory: None,
            is_tea: None,
            is_tea_set: None,
        };
        let json = serde_json::to_value(&item).unwrap();
        assert!(json.get("strain_id").is_some());
    }
}
