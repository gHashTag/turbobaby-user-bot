// Historically held tokio_postgres helpers and a SeaORM wrapper that
// duplicated the canonical `Database::get_user_lang` from `db/mod.rs`.
//
// Cycle #79 migrated the canonical methods on `Database` (get_user_lang,
// set_user_lang, set_user_timezone, save_user_name, mark_user_unblocked,
// is_user_blocked) to SeaORM directly, so the wrapper is no longer
// needed. Module kept for module-structure consistency only — future
// per-user logic that doesn't naturally hang off `Database` lives here.

use anyhow::{Context, Result};

/// Loop #21: display name helper for referrer notifications. Falls back to
/// "Friend" when the user has no stored first name or no row at all.
pub(crate) async fn first_name_for(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
) -> Result<String> {
    use crate::db::entities::user::{Column as UserCol, Entity as UserEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let row = UserEntity::find()
        .filter(UserCol::TelegramId.eq(telegram_id))
        .one(orm)
        .await
        .context("first_name_for query")?;
    Ok(row
        .and_then(|m| m.first_name)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Friend".to_string()))
}
