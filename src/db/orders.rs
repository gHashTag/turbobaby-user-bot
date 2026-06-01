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

#[cfg(test)]
mod tests {
    use super::OrderItem;

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
