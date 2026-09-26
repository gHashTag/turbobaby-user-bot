#![allow(dead_code)]
use utoipa::OpenApi;

/// GET /api/loyalty/leaderboard -- admin only since 2026-09-26 (R1).
#[utoipa::path(
    get,
    path = "/api/loyalty/leaderboard",
    responses(
        (status = 200, description = "Top loyalty leaderboard (top 20 by total spent), for an admin", body = serde_json::Value),
        (status = 401, description = "Unauthorized: an admin only"),
        (status = 429, description = "Too many failed admin attempts from this address"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "loyalty",
    security(("AdminKey" = []))
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

/// GET /api/quest-places -- closed to customers since 2026-09-26 (R2).
#[utoipa::path(
    get,
    path = "/api/quest-places",
    responses(
        (status = 200, description = "List of quest places, for an admin", body = serde_json::Value),
        (status = 404, description = "Without admin proof: the missing-route answer, as for an unmatched path"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "quests",
    security(("AdminKey" = []))
)]
pub(crate) async fn get_quest_places_doc() {}

/// GET /api/treasure-hunts -- closed to customers since 2026-09-26 (R2).
#[utoipa::path(
    get,
    path = "/api/treasure-hunts",
    responses(
        (status = 200, description = "List of active treasure hunts, for an admin", body = serde_json::Value),
        (status = 404, description = "Without admin proof: the missing-route answer, as for an unmatched path"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "quests",
    security(("AdminKey" = []))
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
        (status = 200, description = "List of loyalty tiers with thresholds (perks: always an empty list)", body = serde_json::Value),
        (status = 500, description = "Internal server error"),
    ),
    tag = "loyalty"
)]
pub(crate) async fn get_loyalty_tiers_doc() {}

// The referral credit (owner, 2026-09-26, R3): 10% of every completed rental of
// an invited friend, recorded by a manager, spent on a rental or paid out by
// hand. Money is whole THB; timestamps are RFC 3339 UTC.

/// GET /api/referral-credit/me/{telegram_id}
#[utoipa::path(
    get,
    path = "/api/referral-credit/me/{telegram_id}",
    params(
        ("telegram_id" = i64, Path, description = "Telegram user ID (the caller's own)"),
        ("X-Telegram-Init-Data" = String, Header, description = "Telegram Mini App initData"),
    ),
    responses(
        (status = 200, description = "Referral balance (signed), the hold of the one open request, what is available, and that request; no entries, friends or orders", body = serde_json::Value),
        (status = 400, description = "Invalid telegram_id"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Another user's balance, or a blocked user"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "referrals"
)]
pub(crate) async fn get_referral_credit_doc() {}

/// POST /api/referral-credit/me/{telegram_id}/requests
#[utoipa::path(
    post,
    path = "/api/referral-credit/me/{telegram_id}/requests",
    params(
        ("telegram_id" = i64, Path, description = "Telegram user ID (the caller's own)"),
        ("X-Telegram-Init-Data" = String, Header, description = "Telegram Mini App initData"),
    ),
    request_body(
        content = serde_json::Value,
        description = "{\"kind\": \"redeem\" | \"payout\"}: hold the whole available balance for a rental or for a payout by hand",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "The open request (already_open when the same kind was open) and the balance", body = serde_json::Value),
        (status = 400, description = "invalid_kind"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Another user, or a blocked user"),
        (status = 409, description = "request_open: a request of the other kind is open"),
        (status = 422, description = "nothing_available"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "referrals"
)]
pub(crate) async fn post_referral_credit_request_doc() {}

/// GET /api/admin/referral-credit/overview
#[utoipa::path(
    get,
    path = "/api/admin/referral-credit/overview",
    responses(
        (status = 200, description = "Open requests, invited friends, recorded rentals and non-zero balances", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 429, description = "Too many failed admin attempts from this address"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "admin",
    security(("AdminKey" = []))
)]
pub(crate) async fn get_referral_credit_overview_doc() {}

/// POST /api/admin/referral-credit/rentals
#[utoipa::path(
    post,
    path = "/api/admin/referral-credit/rentals",
    request_body(
        content = serde_json::Value,
        description = "customer_telegram_id, rental_amount_thb (the rental charge without deposit and delivery), optional order_id and note, idempotency_key",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "The record, the inviter's credit and the redemption it settled (idempotent_replay on a repeated key)", body = serde_json::Value),
        (status = 400, description = "invalid_customer, invalid_rental_amount, invalid_note, invalid_order_id, invalid_idempotency_key"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "order_not_found"),
        (status = 409, description = "order_not_completed, order_already_recorded, order_before_program, idempotency_key_reused"),
        (status = 422, description = "order_of_another_customer, order_has_no_rental_line, recorder_is_inviter, no_referral_effect"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "admin",
    security(("AdminKey" = []))
)]
pub(crate) async fn post_referral_credit_rental_doc() {}

/// POST /api/admin/referral-credit/rentals/{id}/reverse
#[utoipa::path(
    post,
    path = "/api/admin/referral-credit/rentals/{id}/reverse",
    params(
        ("id" = i64, Path, description = "Recorded rental ID"),
    ),
    request_body(
        content = serde_json::Value,
        description = "{\"note\": optional}: reverse the record whole",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "The reversed record and what moved back (already_reversed when repeated)", body = serde_json::Value),
        (status = 400, description = "invalid_note"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "rental_not_found"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "admin",
    security(("AdminKey" = []))
)]
pub(crate) async fn post_referral_credit_reversal_doc() {}

/// POST /api/admin/referral-credit/requests/{id}/resolve
#[utoipa::path(
    post,
    path = "/api/admin/referral-credit/requests/{id}/resolve",
    params(
        ("id" = i64, Path, description = "Request ID"),
    ),
    request_body(
        content = serde_json::Value,
        description = "{\"action\": \"paid\" | \"declined\", \"note\": optional}: paid records a payout made by hand",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "The resolved request and the balance (idempotent_replay when repeated)", body = serde_json::Value),
        (status = 400, description = "invalid_action, invalid_note"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "request_not_found"),
        (status = 409, description = "request_not_open, balance_below_request"),
        (status = 422, description = "redeem_cannot_be_paid"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "admin",
    security(("AdminKey" = []))
)]
pub(crate) async fn post_referral_credit_resolution_doc() {}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "TurboBaby Bot API",
        version = "1.0",
        // The title one line above was renamed; this description was not, and
        // unlike a comment it is *served*: `/api-docs/openapi.json` on the
        // TurboBaby deployment announced "cannabis dispensary" to every client
        // that read the spec. The rebrand sweeps grep source for user-visible
        // strings, and a string is user-visible when the machine hands it out,
        // not only when a screen paints it.
        description = "REST API for TurboBaby Bot — motorbike rental Telegram Mini App. \
                       Provides endpoints for the bike catalog, rentals, orders, loyalty program, and admin management.",
        contact(
            name = "TurboBaby Bot",
            url = "https://turbobaby-bot-production.up.railway.app"
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
        get_referral_credit_doc,
        post_referral_credit_request_doc,
        get_referral_credit_overview_doc,
        post_referral_credit_rental_doc,
        post_referral_credit_reversal_doc,
        post_referral_credit_resolution_doc,
    ),
    tags(
        (name = "loyalty", description = "Loyalty program — profiles, tiers, leaderboard, bonus transactions"),
        (name = "orders", description = "Order creation and management"),
        // D19 keeps the heritage endpoints alive so existing orders stay
        // readable, but their vocabulary does not belong in the published
        // description of a motorbike shop's API.
        (name = "catalog", description = "Catalog — bikes, fleet units, accessories and sets"),
        (name = "quests", description = "Quest places, treasure hunts, location quests"),
        (name = "referrals", description = "Referral credit — 10% of an invited friend's rentals, in THB"),
        (name = "admin", description = "Admin-only endpoints — requires X-Admin-Key header"),
    ),
    servers(
        (url = "https://turbobaby-bot-production.up.railway.app", description = "Production"),
        (url = "http://localhost:3000", description = "Local development"),
    ),
)]
pub(crate) struct ApiDoc;

/// The rebrand's own gate, applied to the one surface the rebrand missed.
///
/// `#2` and `#26` are enforced by grepping source files for the old
/// vocabulary, and that is how `/api-docs/openapi.json` kept serving "REST API
/// for Woody Weed Bot — cannabis dispensary Telegram Mini App" from a
/// turbobaby-bot URL long after every screen had been renamed: the string sat
/// inside an attribute macro, three lines under a `title` that *had* been
/// changed, and no human reading the rendered app could see it.
///
/// So this asserts on the built document rather than on the file. It calls
/// `ApiDoc::openapi()` — the same constructor `src/main.rs` hands to the
/// route — and walks the text the client actually receives. A description
/// added tomorrow in a new `tags(...)` entry is covered without anyone
/// remembering to extend a list.
#[cfg(test)]
mod published_document_tests {
    use super::ApiDoc;
    use utoipa::OpenApi;

    /// Vocabulary that a motorbike shop has no honest reason to publish.
    ///
    /// Deliberately not exhaustive and deliberately not `strain`-only: the
    /// point is to catch the *category* of leak, not to enumerate a banned
    /// dictionary. `gram` is absent on purpose — it is a substring of
    /// "telegram" and "program", and a check that cries wolf gets deleted.
    ///
    /// Widened 2026-09-25, when the owner ruled that nothing cannabis-related
    /// may appear anywhere: the Russian spellings, the other English names
    /// and the certification the old shop advertised. Each is a substring no
    /// honest description of a motorbike rental contains (measured: the
    /// document holds none of them). `indica` is absent for the reason `gram`
    /// is: it is a substring of "indicates".
    const RETIRED: [&str; 13] = [
        "woody",
        "weed",
        "cannabis",
        "dispensary",
        "strain",
        "thc",
        "marijuana",
        "sativa",
        "gacp",
        "каннабис",
        "конопл",
        "марихуан",
        "вуди",
    ];

    fn published_text() -> String {
        serde_json::to_string(&ApiDoc::openapi())
            .expect("the OpenAPI document serializes — main.rs serves exactly this")
            .to_lowercase()
    }

    #[test]
    fn the_served_document_carries_no_cannabis_era_vocabulary() {
        let doc = published_text();
        for word in RETIRED {
            assert!(
                !doc.contains(word),
                "/api-docs/openapi.json still publishes {word:?}; \
                 the rebrand is judged by what the server hands out, not by what the screens paint"
            );
        }
    }

    /// The counterpart claim: the document names this product.
    ///
    /// Without it the test above is satisfied by an empty description, which
    /// is the failure mode of every "contains no X" gate written alone.
    #[test]
    fn the_served_document_names_turbobaby() {
        let doc = published_text();
        assert!(
            doc.contains("turbobaby"),
            "the OpenAPI document no longer names the product — an empty description \
             passes the retired-vocabulary check for the wrong reason"
        );
    }
}
