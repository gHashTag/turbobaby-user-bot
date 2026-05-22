// User helpers are now in db/mod.rs via Database impl
// This file kept for module structure

// Wave 5: SeaORM Entity API path. Старый tokio-postgres код в db/mod.rs — будем удалять в Wave 6+.

/// Returns the language preference for a user via SeaORM Entity API.
/// Looks up the `user_languages` table by `telegram_id` primary key.
#[allow(dead_code)]
pub async fn get_user_lang_seaorm(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
) -> Option<String> {
    use sea_orm::EntityTrait;
    crate::db::entities::user::Entity::find_by_id(telegram_id)
        .one(orm)
        .await
        .ok()
        .flatten()
        .map(|m| m.language)
}
