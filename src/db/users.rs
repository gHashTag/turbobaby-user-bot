// Historically held tokio_postgres helpers and a SeaORM wrapper that
// duplicated the canonical `Database::get_user_lang` from `db/mod.rs`.
//
// Cycle #79 migrated the canonical methods on `Database` (get_user_lang,
// set_user_lang, set_user_timezone, save_user_name, mark_user_unblocked,
// is_user_blocked) to SeaORM directly, so the wrapper is no longer
// needed. Module kept for module-structure consistency only — future
// per-user logic that doesn't naturally hang off `Database` lives here.
