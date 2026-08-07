#![allow(dead_code)]
use utoipa::OpenApi;

/// GET /api/loyalty/leaderboard
#[utoipa::path(
    get,
    path = "/api/loyalty/leaderboard",
    responses(
        (status = 200, description = "Top loyalty leaderboard (top 20 by total spent)", body = serde_json::Value),
        (status = 500, description = "Internal server error"),
    ),
    tag = "loyalty"
)]
pub(crate) async fn get_leaderboard_doc() {}

/// GET /api/loyalty/{telegram_id}
#[utoipa::path(
    get,
    path = "/api/loyalty/{telegram_id}",
    params(
        ("telegram_id" = i64, Path, description = "Telegram user ID"),
    ),
    responses(
        (status = 200, description = "Loyalty profile for user", body = serde_json::Value),
        (status = 404, description = "Profile not found"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "loyalty"
)]
pub(crate) async fn get_loyalty_profile_doc() {}

/// GET /api/loyalty/{telegram_id}/bonus-history
#[utoipa::path(
    get,
    path = "/api/loyalty/{telegram_id}/bonus-history",
    params(
        ("telegram_id" = i64, Path, description = "Telegram user ID"),
        ("limit" = Option<i64>, Query, description = "Page size (max 100, default 50)"),
        ("offset" = Option<i64>, Query, description = "Rows to skip (default 0)"),
    ),
    responses(
        (status = 200, description = "Bonus transaction ledger for user", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "loyalty"
)]
pub(crate) async fn get_bonus_history_doc() {}

/// GET /api/sets
#[utoipa::path(
    get,
    path = "/api/sets",
    params(
        ("include_hidden" = Option<String>, Query, description = "Set to '1' or 'true' to include hidden sets (admin only)"),
    ),
    responses(
        (status = 200, description = "List of available sets (accessory sets + tea sets)", body = serde_json::Value),
        (status = 500, description = "Internal server error"),
    ),
    tag = "catalog"
)]
pub(crate) async fn get_sets_doc() {}

/// GET /api/quest-places
#[utoipa::path(
    get,
    path = "/api/quest-places",
    responses(
        (status = 200, description = "List of quest places", body = serde_json::Value),
        (status = 500, description = "Internal server error"),
    ),
    tag = "quests"
)]
pub(crate) async fn get_quest_places_doc() {}

/// GET /api/treasure-hunts
#[utoipa::path(
    get,
    path = "/api/treasure-hunts",
    responses(
        (status = 200, description = "List of active treasure hunts", body = serde_json::Value),
        (status = 500, description = "Internal server error"),
    ),
    tag = "quests"
)]
pub(crate) async fn get_treasure_hunts_doc() {}

/// POST /api/orders
#[utoipa::path(
    post,
    path = "/api/orders",
    request_body(
        content = serde_json::Value,
        description = "Order payload with customer info and items",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "Order created successfully", body = serde_json::Value),
        (status = 500, description = "Internal server error"),
    ),
    tag = "orders"
)]
pub(crate) async fn create_order_doc() {}

/// GET /api/admin/managers
#[utoipa::path(
    get,
    path = "/api/admin/managers",
    params(
        ("X-Admin-Token" = String, Header, description = "Admin password token"),
    ),
    responses(
        (status = 200, description = "List of managers", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "admin",
    security(("AdminKey" = []))
)]
pub(crate) async fn get_managers_doc() {}

/// GET /api/loyalty/tiers
#[utoipa::path(
    get,
    path = "/api/loyalty/tiers",
    responses(
        (status = 200, description = "List of loyalty tiers with thresholds and perks", body = serde_json::Value),
        (status = 500, description = "Internal server error"),
    ),
    tag = "loyalty"
)]
pub(crate) async fn get_loyalty_tiers_doc() {}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Woody Weed Bot API",
        version = "1.0",
        description = "REST API for Woody Weed Bot — cannabis dispensary Telegram Mini App. \
                       Provides endpoints for loyalty program, orders, catalog, quests, and admin management.",
        contact(
            name = "Woody Weed Bot",
            url = "https://woody-weed-bot-production-370f.up.railway.app"
        ),
    ),
    paths(
        get_leaderboard_doc,
        get_loyalty_profile_doc,
        get_bonus_history_doc,
        get_loyalty_tiers_doc,
        get_sets_doc,
        get_quest_places_doc,
        get_treasure_hunts_doc,
        create_order_doc,
        get_managers_doc,
    ),
    tags(
        (name = "loyalty", description = "Loyalty program — profiles, tiers, leaderboard, bonus transactions"),
        (name = "orders", description = "Order creation and management"),
        (name = "catalog", description = "Catalog — strains, accessories, sets, tea products"),
        (name = "quests", description = "Quest places, treasure hunts, location quests"),
        (name = "admin", description = "Admin-only endpoints — requires X-Admin-Key header"),
    ),
    servers(
        (url = "https://woody-weed-bot-production-370f.up.railway.app", description = "Production"),
        (url = "http://localhost:3000", description = "Local development"),
    ),
)]
pub(crate) struct ApiDoc;
