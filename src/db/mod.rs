pub mod strains;
pub mod loyalty;
pub mod orders;
pub mod users;

use anyhow::Result;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::Arc;

pub use strains::*;
pub use loyalty::*;
pub use orders::*;
pub use users::*;

pub struct Database {
    pub pool: PgPool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    // ---------- User language ----------

    pub async fn get_user_lang(&self, telegram_id: i64) -> Option<String> {
        sqlx::query_scalar!("SELECT language FROM user_languages WHERE telegram_id = $1", telegram_id)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
    }

    pub async fn set_user_lang(&self, telegram_id: i64, lang: &str) -> Result<()> {
        sqlx::query!(
            "INSERT INTO user_languages (telegram_id, language) VALUES ($1, $2)
             ON CONFLICT (telegram_id) DO UPDATE SET language = $2, updated_at = NOW()",
            telegram_id, lang
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_user_timezone(&self, telegram_id: i64, tz: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE user_languages SET timezone = $2 WHERE telegram_id = $1",
            telegram_id, tz
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn save_user_name(&self, telegram_id: i64, first_name: &str) -> Result<()> {
        sqlx::query!(
            "INSERT INTO user_languages (telegram_id, first_name) VALUES ($1, $2)
             ON CONFLICT (telegram_id) DO UPDATE SET first_name = $2",
            telegram_id, first_name
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_user_unblocked(&self, telegram_id: i64) -> Result<()> {
        sqlx::query!(
            "UPDATE loyalty_profiles SET is_blocked = false WHERE telegram_id = $1",
            telegram_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ---------- Strains of day ----------

    pub async fn get_strains_of_day(&self) -> Result<Vec<StrainOfDay>> {
        let rows = sqlx::query_as!(StrainOfDay,
            "SELECT s.id, s.name, s.category, s.thc_percent, s.price_per_gram,
                    s.strain_of_day_discount, s.image_url
             FROM strains s WHERE s.is_strain_of_day = true AND s.is_available = true
             ORDER BY s.strain_of_day_set_at DESC LIMIT 5"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ---------- Raw query ----------
    pub fn raw(&self) -> &PgPool { &self.pool }
}
