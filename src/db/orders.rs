use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_postgres::Row;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub telegram_id: i64,
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
            id: row.get("id"),
            telegram_id: row.get("telegram_id"),
            customer_name: row.get("customer_name"),
            customer_phone: row.get("customer_phone"),
            customer_telegram: row.get("customer_telegram"),
            items: row.get("items"),
            subtotal: row.get("subtotal"),
            bonus_used: row.get("bonus_used"),
            total: row.get("total"),
            status: row.get("status"),
            shop_id: row.get("shop_id"),
            created_at: row.get("created_at"),
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
    use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
    use crate::db::entities::order::{Entity, Column};
    Entity::find()
        .filter(Column::TelegramId.eq(telegram_id))
        .all(orm)
        .await
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
