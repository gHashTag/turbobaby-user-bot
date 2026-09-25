//! i18n engine for Trios ecosystem

use crate::trios::core::Lang;
use std::collections::HashMap;

/// Translation key
pub type Key = &'static str;

/// Translation value
pub type Value = &'static str;

/// Navigation translations
pub const T_NAV_HOME: Key = "nav.home";
pub const T_NAV_SETS: Key = "nav.sets";
pub const T_NAV_MENU: Key = "nav.menu";
pub const T_NAV_ACCESSORIES: Key = "nav.accessories";
pub const T_NAV_TEA: Key = "nav.tea";
pub const T_NAV_QUEST: Key = "nav.quest";
pub const T_NAV_GAME: Key = "nav.game";
pub const T_NAV_EVENTS: Key = "nav.events";
pub const T_NAV_CART: Key = "nav.cart";
pub const T_NAV_PROFILE: Key = "nav.profile";
pub const T_NAV_MORE: Key = "nav.more";
pub const T_NAV_FLEET: Key = "nav.fleet";
pub const T_NAV_RIDE: Key = "nav.ride";
pub const T_NAV_ORDERS: Key = "nav.orders";

/// The two labels on the idle shop game's own plant/harvest buttons. They
/// were declared in the garden's namespace, but the garden and the game are
/// different mechanics: the garden had tables and endpoints, the game is
/// client-side state. The garden is gone (D5); the game is not.
pub const T_GAME_HARVEST: Key = "game.harvest";
pub const T_GAME_PLANT: Key = "game.plant";

/// Referral social proof. These also lived under `garden.*`, because the
/// garden screen was the only place that rendered them — but the endpoints
/// behind them (`/invitees`, `/milestones`) read `referral_events` and
/// `referral_milestones`, which migration 083 does not touch. The panels
/// moved to the referrals page rather than dying with their old host.
pub const T_REFERRAL_INVITEES_TITLE: Key = "referral.invitees.title";
pub const T_REFERRAL_INVITEES_EMPTY: Key = "referral.invitees.empty";
pub const T_REFERRAL_INVITEE_JOINED: Key = "referral.invitee.joined";
pub const T_REFERRAL_INVITEE_ORDERED: Key = "referral.invitee.ordered";
/// Printed instead of a name only when nothing about the person was ever
/// recorded — see `crate::trios::person::Naming::Anonymous`.
pub const T_REFERRAL_INVITEE_UNKNOWN: Key = "referral.invitee.unknown";
pub const T_REFERRAL_MILESTONE_TITLE: Key = "referral.milestone.title";
pub const T_REFERRAL_MILESTONE_SUBTITLE: Key = "referral.milestone.subtitle";
/// `{0}` friends, `{1}` the money — already formatted by
/// `crate::trios::pricing::format_baht`, symbol included.
///
/// Both translations carried a literal `฿` glued to `{1}` until the panel was
/// rewired. That is a currency decision made in a translation file: the market
/// profile says where the symbol goes (`THB_MARKET.symbol_suffix` is `false`,
/// i.e. `฿500`), and the suffixed glyph disagreed with every other price on
/// the screen. Deciding it once, in the formatter, is D15.
pub const T_REFERRAL_MILESTONE_AWARDED: Key = "referral.milestone.awarded";

/// Generic button translations
pub const T_CLOSE: Key = "btn.close";

/// Telegram button translations
pub const T_BTN_CART: Key = "btn.cart";
pub const T_BTN_CHECKOUT: Key = "btn.checkout";
pub const T_BTN_PAY: Key = "btn.pay";
pub const T_BTN_CHECKIN: Key = "btn.checkin";
pub const T_BTN_ORDER: Key = "btn.order";
pub const T_BTN_ASK: Key = "btn.ask";

/// Coming soon page translations
pub const T_COMING_SOON: Key = "coming_soon.title";
pub const T_COMING_SOON_DESC: Key = "coming_soon.description";
pub const T_COMING_SOON_WORKING: Key = "coming_soon.working";
pub const T_BACK_HOME: Key = "btn.back_home";

/// Category translations
pub const T_CAT_MENU: Key = "cat.menu";
pub const T_CAT_ACCESSORIES: Key = "cat.accessories";
pub const T_CAT_TEA: Key = "cat.tea";

/// Location quest translations
pub const T_TITLE: Key = "title";
pub const T_SUBTITLE: Key = "subtitle";
pub const T_START_BTN: Key = "start_btn";
pub const T_EXPLORE_BTN: Key = "explore_btn";
pub const T_CHECKPOINT_TITLE: Key = "checkpoint_title";
pub const T_CURRENT_CHECKPOINT: Key = "current_checkpoint";
pub const T_PURCHASE_REQUIRED: Key = "purchase_required";
pub const T_PURCHASE_MIN: Key = "purchase_min";
pub const T_SCAN_QR: Key = "scan_qr";
pub const T_SCAN_QR_DESC: Key = "scan_qr_desc";
pub const T_SUBMIT_CHECKIN: Key = "submit_checkin";
pub const T_STATUS_LOCKED: Key = "status_locked";
pub const T_STATUS_ACTIVE: Key = "status_active";
pub const T_STATUS_COMPLETED: Key = "status_completed";
pub const T_CHECKIN_SUCCESS: Key = "checkin_success";
pub const T_NEXT_LOCATION: Key = "next_location";
pub const T_QUEST_COMPLETE: Key = "quest_complete";
pub const T_REWARD_CLAIM: Key = "reward_claim";
pub const T_REWARD: Key = "reward";
pub const T_ERROR_WRONG_ORDER: Key = "error_wrong_order";
pub const T_ERROR_PURCHASE: Key = "error_purchase";
pub const T_ERROR_INVALID_QR: Key = "error_invalid_qr";
pub const T_ERROR_ALREADY_CHECKED: Key = "error_already_checked";
pub const T_ERROR_CHECKIN_FAILED: Key = "error_checkin_failed";

/// Quest point translations
pub const T_POINT_1: Key = "point_1";
pub const T_POINT_2: Key = "point_2";
pub const T_POINT_3: Key = "point_3";
pub const T_POINT_4: Key = "point_4";
pub const T_POINT_5: Key = "point_5";

/// Events calendar translations
pub const T_EVENTS_TITLE: Key = "events.title";
pub const T_EVENTS_SUBTITLE: Key = "events.subtitle";
pub const T_EVENTS_NO_EVENTS: Key = "events.no_events";
pub const T_EVENTS_DATE: Key = "events.date";
pub const T_EVENTS_LOCATION: Key = "events.location";
pub const T_EVENTS_CAPACITY: Key = "events.capacity";
pub const T_EVENTS_PRICE: Key = "events.price";
pub const T_EVENTS_PRICE_STARS: Key = "events.price_stars";
pub const T_EVENTS_BOOK: Key = "events.book";
pub const T_EVENTS_BOOKED: Key = "events.booked";
pub const T_EVENTS_BOOK_FREE: Key = "events.book_free";
pub const T_EVENTS_SOLD_OUT: Key = "events.sold_out";
pub const T_EVENTS_ERROR: Key = "events.error";
pub const T_EVENTS_INSUFFICIENT_STARS: Key = "events.insufficient_stars";
pub const T_EVENTS_REMINDER_BODY: Key = "events.reminder_body";
pub const T_EVENTS_ALREADY_BOOKED: Key = "events.already_booked";
pub const T_EVENTS_VIDEO: Key = "events.video";
pub const T_EVENTS_GALLERY: Key = "events.gallery";
pub const T_EVENTS_WEEKDAY_MON: Key = "events.weekday.mon";
pub const T_EVENTS_WEEKDAY_TUE: Key = "events.weekday.tue";
pub const T_EVENTS_WEEKDAY_WED: Key = "events.weekday.wed";
pub const T_EVENTS_WEEKDAY_THU: Key = "events.weekday.thu";
pub const T_EVENTS_WEEKDAY_FRI: Key = "events.weekday.fri";
pub const T_EVENTS_WEEKDAY_SAT: Key = "events.weekday.sat";
pub const T_EVENTS_WEEKDAY_SUN: Key = "events.weekday.sun";

/// Screen title translations
pub const T_HOME_TITLE: Key = "home.title";
pub const T_HOME_SUBTITLE: Key = "home.subtitle";
pub const T_MENU_TITLE: Key = "menu.title";
pub const T_MENU_DESC: Key = "menu.description";
pub const T_SETS_TITLE: Key = "sets.title";
pub const T_SETS_DESC: Key = "sets.description";
pub const T_ACC_TITLE: Key = "acc.title";
pub const T_ACC_DESC: Key = "acc.description";
pub const T_ACC_CAT_GRINDER: Key = "acc.cat.grinder";
pub const T_ACC_CAT_PAPERS: Key = "acc.cat.papers";
pub const T_ACC_CAT_PIPE: Key = "acc.cat.pipe";
pub const T_ACC_CAT_BONG: Key = "acc.cat.bong";
pub const T_ACC_CAT_STORAGE: Key = "acc.cat.storage";
pub const T_ACC_CAT_LIGHTER: Key = "acc.cat.lighter";
pub const T_ACC_CAT_CLOTHING: Key = "acc.cat.clothing";
pub const T_ACC_CAT_SOUVENIR: Key = "acc.cat.souvenir";
pub const T_ACC_CAT_OTHER: Key = "acc.cat.other";
pub const T_TEA_TITLE: Key = "tea.title";
pub const T_TEA_DESC: Key = "tea.description";
pub const T_CART_TITLE: Key = "cart.title";
pub const T_CART_EMPTY: Key = "cart.empty";
pub const T_CART_EMPTY_DESC: Key = "cart.empty_desc";
pub const T_CART_BROWSE_MENU: Key = "cart.browse_menu";
pub const T_CART_BROWSE_SETS: Key = "cart.browse_sets";
pub const T_CHECKOUT_TITLE: Key = "checkout.title";
pub const T_ORDERS_TITLE: Key = "orders.title";
pub const T_PROFILE_TITLE: Key = "profile.title";
pub const T_YOUR_ORDER: Key = "checkout.your_order";
pub const T_YOUR_INFO: Key = "checkout.your_info";
pub const T_PICKUP_LOCATION: Key = "checkout.pickup_location";
pub const T_DELIVERY: Key = "checkout.delivery";
pub const T_DELIVERY_ZONE: Key = "checkout.delivery_zone";
pub const T_DELIVERY_ETA: Key = "checkout.delivery_eta";
pub const T_DELIVERY_FEE: Key = "checkout.delivery_fee";
pub const T_PAYMENT: Key = "checkout.payment";
pub const T_PLACE_ORDER: Key = "checkout.place_order";
pub const T_BACK: Key = "btn.back";
pub const T_TOTAL: Key = "label.total";
pub const T_CHECKOUT_ERR_NAME: Key = "checkout.error.name_required";
pub const T_CHECKOUT_ERR_NAME_LONG: Key = "checkout.error.name_long";
pub const T_CHECKOUT_ERR_PHONE: Key = "checkout.error.phone_required";
pub const T_CHECKOUT_ERR_PHONE_LONG: Key = "checkout.error.phone_long";
pub const T_CHECKOUT_ERR_PHONE_INVALID: Key = "checkout.error.phone_invalid";
pub const T_CHECKOUT_ERR_ADDRESS: Key = "checkout.error.address_required";
pub const T_CHECKOUT_ERR_ADDRESS_LONG: Key = "checkout.error.address_long";
pub const T_CHECKOUT_ERR_ITEMS: Key = "checkout.error.items_empty";
pub const T_CHECKOUT_ERR_NO_TELEGRAM: Key = "checkout.error.no_telegram";
/// Heading for the list of reasons the order button is not clickable yet.
/// Without it a disabled button gives the customer nothing to act on.
pub const T_CHECKOUT_BLOCKED_TITLE: Key = "checkout.blocked_title";
pub const T_CHECKOUT_FULFILLMENT: Key = "checkout.fulfillment";
pub const T_CHECKOUT_FULFILLMENT_DELIVERY: Key = "checkout.fulfillment.delivery";
pub const T_CHECKOUT_FULFILLMENT_PICKUP: Key = "checkout.fulfillment.pickup";
pub const T_CHECKOUT_ERR_AGE: Key = "checkout.error.age";
pub const T_CHECKOUT_CHANGE: Key = "checkout.change";
pub const T_CHECKOUT_PHONE_FROM_TELEGRAM: Key = "checkout.phone_from_telegram";
pub const T_CHECKOUT_ERR_NETWORK: Key = "checkout.error.network";
pub const T_CHECKOUT_ERR_PARSE: Key = "checkout.error.parse";
pub const T_CHECKOUT_RETRY: Key = "checkout.retry";
pub const T_ADD_TO_CART: Key = "btn.add_to_cart";
pub const T_FULFILLMENT_LABEL: Key = "tea.fulfillment_label";
pub const T_FULFILLMENT_DINE_IN: Key = "tea.fulfillment_dine_in";
pub const T_FULFILLMENT_TAKEAWAY: Key = "tea.fulfillment_takeaway";
pub const T_SHARE: Key = "btn.share";
pub const T_SHARE_MESSAGE: Key = "share.message";
pub const T_REFERRAL_TITLE: Key = "referral.title";
pub const T_REFERRAL_SUBTITLE: Key = "referral.subtitle";
pub const T_REFERRAL_LINK_LABEL: Key = "referral.link_label";
pub const T_REFERRAL_COPY: Key = "referral.copy";
pub const T_REFERRAL_COPIED: Key = "referral.copied";
pub const T_REFERRAL_SHARE: Key = "referral.share";
pub const T_REFERRAL_SHARE_TEXT: Key = "referral.share_text";
pub const T_REFERRAL_STAT_INVITED: Key = "referral.stat.invited";
pub const T_REFERRAL_STAT_CONFIRMED: Key = "referral.stat.confirmed";
pub const T_REFERRAL_STAT_PENDING: Key = "referral.stat.pending";
pub const T_REFERRAL_STAT_BONUS: Key = "referral.stat.bonus";
pub const T_REFERRAL_TOP: Key = "referral.top";
pub const T_REFERRAL_EMPTY_LEADERBOARD: Key = "referral.empty_leaderboard";
pub const T_REFERRAL_ID_MASK: Key = "referral.id_mask";
pub const T_REFERRAL_ROW_META: Key = "referral.row_meta";
pub const T_LOADING: Key = "label.loading";
pub const T_FILTER_ALL: Key = "filter.all";

// Variant C: retention + community
pub const T_TRUST_GACP: Key = "trust.gacp";
pub const T_TRUST_MEDICAL: Key = "trust.medical";
pub const T_TRUST_SUPPORT: Key = "trust.support";
pub const T_TRUST_AGE: Key = "trust.age";
pub const T_REORDER: Key = "btn.reorder";
pub const T_SEARCH_PLACEHOLDER: Key = "search.placeholder";
pub const T_BROADCAST: Key = "broadcast.title";
pub const T_BROADCAST_TEXT: Key = "broadcast.text";
pub const T_BROADCAST_SEND: Key = "broadcast.send";
pub const T_BROADCAST_SENT: Key = "broadcast.sent";
pub const T_BROADCAST_PHOTO: Key = "broadcast.photo";
pub const T_BROADCAST_PHOTO_UPLOAD: Key = "broadcast.photo_upload";
pub const T_BROADCAST_PHOTO_HINT: Key = "broadcast.photo_hint";
pub const T_BROADCAST_PRODUCT: Key = "broadcast.product";
pub const T_BROADCAST_PRODUCT_NONE: Key = "broadcast.product_none";
pub const T_BROADCAST_BUTTON_TEXT: Key = "broadcast.button_text";
pub const T_BROADCAST_PREVIEW: Key = "broadcast.preview";
pub const T_BROADCAST_NO_PRODUCT: Key = "broadcast.no_product";
pub const T_BROADCAST_SELECT_CATALOG: Key = "broadcast.select_catalog";
pub const T_BROADCAST_SEND_TEST: Key = "broadcast.send_test";
pub const T_BROADCAST_TEST_SENT: Key = "broadcast.test_sent";

// Checkout error messages (cycle #69). Mapped from HTTP status by
// `trios::checkout_errors::friendly_order_error`. Unknown statuses
// fall back to an inline `Ошибка сервера: HTTP {n}` since templating
// arbitrary integers through the static key table is more machinery
// than it's worth for one rare path.
pub const T_CHECKOUT_ERR_400: Key = "checkout.err.400";
pub const T_CHECKOUT_ERR_403: Key = "checkout.err.403";
pub const T_CHECKOUT_ERR_404: Key = "checkout.err.404";
pub const T_CHECKOUT_ERR_409: Key = "checkout.err.409";
pub const T_CHECKOUT_ERR_422: Key = "checkout.err.422";
pub const T_CHECKOUT_ERR_429: Key = "checkout.err.429";
pub const T_CHECKOUT_ERR_5XX: Key = "checkout.err.5xx";

// Loop #7: per-field checkout error messages surfaced from the server
// response body (422 with a stable `error` code) so the UI can show a
// sentence instead of the generic price-change hint.
pub const T_CHECKOUT_ERR_AGE_NOT_CONFIRMED: Key = "checkout.err.age_not_confirmed";
pub const T_CHECKOUT_ERR_ZONE_INVALID: Key = "checkout.err.zone_invalid";

// Generic API error messages (cycle #74). Used by
// `trios::api_errors::friendly_response_error` for any non-checkout API
// call. T_API_ERR_401 covers the "Telegram session expired" scenario;
// T_API_ERR_UNKNOWN replaces the hardcoded RU `Ошибка сервера: HTTP {n}`
// fallback so non-RU users don't see Cyrillic on a random 418.
pub const T_API_ERR_401: Key = "api.err.401";
pub const T_API_ERR_UNKNOWN: Key = "api.err.unknown";

// Cart screen hard-coded strings
pub const T_CART_SUBTOTAL: Key = "cart.subtotal";
pub const T_CART_DELIVERY: Key = "cart.delivery";
pub const T_CART_DELIVERY_FREE: Key = "cart.delivery_free";
pub const T_CART_BACK_MENU: Key = "cart.back_menu";
pub const T_CART_CHECKOUT: Key = "cart.checkout";
pub const T_CART_ITEMS: Key = "cart.items";
pub const T_CART_DINE_IN: Key = "cart.dine_in";
pub const T_CART_TAKEAWAY: Key = "cart.takeaway";
pub const T_CART_BONUS_NUDGE: Key = "cart.bonus_nudge";
pub const T_CART_DECREASE_QTY: Key = "cart.decrease_qty";
pub const T_CART_REMOVE: Key = "cart.remove";
pub const T_CART_IMAGE_ALT: Key = "cart.image_alt";
/// `{0}` is the unit price and `{1}` the line total, both already formatted by
/// `crate::trios::pricing::format_baht`.
///
/// The cart row spelled this `"{price_str} each · {line_total_str}"` in Rust
/// until 2026-09-16 — one hard-coded English word on a screen whose every other
/// string goes through `tf`, so a Russian customer read «฿300 each · ฿600».
pub const T_CART_LINE_EACH: Key = "cart.line_each";

// Checkout screen hard-coded strings
pub const T_CHECKOUT_CART_EMPTY: Key = "checkout.cart_empty";
pub const T_CHECKOUT_STEP_CART: Key = "checkout.step.cart";
pub const T_CHECKOUT_STEP_DETAILS: Key = "checkout.step.details";
pub const T_CHECKOUT_STEP_CONFIRM: Key = "checkout.step.confirm";
pub const T_CHECKOUT_SELECT_ZONE: Key = "checkout.select_zone";
pub const T_CHECKOUT_STARS: Key = "checkout.stars";
pub const T_CHECKOUT_STARS_AVAILABLE: Key = "checkout.stars_available";
pub const T_CHECKOUT_STARS_MINUS: Key = "checkout.stars_minus";
pub const T_CHECKOUT_BONUS: Key = "checkout.bonus";
pub const T_CHECKOUT_BONUS_AVAILABLE: Key = "checkout.bonus_available";
pub const T_CHECKOUT_BONUS_APPLIED: Key = "checkout.bonus_applied";
pub const T_CHECKOUT_BONUS_MAX: Key = "checkout.bonus_max";
pub const T_CHECKOUT_NAME_LABEL: Key = "checkout.name_label";
pub const T_CHECKOUT_NAME_PLACEHOLDER: Key = "checkout.name_placeholder";
pub const T_CHECKOUT_PHONE_LABEL: Key = "checkout.phone_label";
pub const T_CHECKOUT_PHONE_PLACEHOLDER: Key = "checkout.phone_placeholder";
pub const T_CHECKOUT_OPEN_MAP: Key = "checkout.open_map";
pub const T_CHECKOUT_ADDRESS_LABEL: Key = "checkout.address_label";
pub const T_CHECKOUT_ADDRESS_PLACEHOLDER: Key = "checkout.address_placeholder";
pub const T_CHECKOUT_USE_MY_LOCATION: Key = "checkout.use_my_location";
pub const T_CHECKOUT_NOTES_LABEL: Key = "checkout.notes_label";
pub const T_CHECKOUT_NOTES_PLACEHOLDER: Key = "checkout.notes_placeholder";
pub const T_CHECKOUT_CASH_ON_DELIVERY: Key = "checkout.cash_on_delivery";
pub const T_CHECKOUT_PAY_ON_RECEIVE: Key = "checkout.pay_on_receive";
pub const T_CHECKOUT_PROCESSING: Key = "checkout.processing";

// Checkout trust + age gate micro-copy
pub const T_CHECKOUT_TRUST_TITLE: Key = "checkout.trust_title";
pub const T_CHECKOUT_TRUST_VERIFIED: Key = "checkout.trust.verified";
pub const T_CHECKOUT_TRUST_COD: Key = "checkout.trust.cod";
pub const T_CHECKOUT_TRUST_SECURE: Key = "checkout.trust.secure";
pub const T_CHECKOUT_AGE_CONFIRM: Key = "checkout.age_confirm";
pub const T_CHECKOUT_AGE_NOTICE: Key = "checkout.age_notice";

// Catalog shared strings
pub const T_CATALOG_EMPTY: Key = "catalog.empty";
pub const T_CATALOG_ERROR: Key = "catalog.error";
pub const T_CATALOG_SORT_DEFAULT: Key = "catalog.sort.default";
pub const T_CATALOG_SORT_POPULAR: Key = "catalog.sort.popular";
pub const T_CATALOG_SORT_PRICE: Key = "catalog.sort.price";
pub const T_CATALOG_SORT_NEW: Key = "catalog.sort.new";
pub const T_CATALOG_SORT_DISCOUNT: Key = "catalog.sort.discount";
pub const T_LOW_STOCK: Key = "catalog.low_stock";
pub const T_WATCH_VIDEO: Key = "catalog.watch_video";

// Sommelier strings

// Game strings
pub const T_GAME_TITLE: Key = "game.title";
pub const T_GAME_TAB_SHOP: Key = "game.tab.shop";
pub const T_GAME_TAB_FARM: Key = "game.tab.farm";
pub const T_GAME_TAB_DJ: Key = "game.tab.dj";
pub const T_GAME_TAB_GRILL: Key = "game.tab.grill";
pub const T_GAME_TABLE_FREE: Key = "game.table.free";
pub const T_GAME_TABLE_WAITING: Key = "game.table.waiting";
pub const T_GAME_TABLE_READY: Key = "game.table.ready";
pub const T_GAME_TABLE_EATING: Key = "game.table.eating";
pub const T_GAME_TABLE_DIRTY: Key = "game.table.dirty";
pub const T_GAME_TABLE_PREPARING: Key = "game.table.preparing";
pub const T_GAME_FARM_EMPTY: Key = "game.farm.empty";
pub const T_GAME_FARM_PLANTED: Key = "game.farm.planted";
pub const T_GAME_FARM_WATERED: Key = "game.farm.watered";
pub const T_GAME_FARM_GROWN: Key = "game.farm.grown";
pub const T_GAME_FARM_WATER: Key = "game.farm.water";
pub const T_GAME_START_PARTY: Key = "game.start_party";
pub const T_GAME_UPGRADE_MAX: Key = "game.upgrade.max";
pub const T_GAME_UPGRADE_LEVEL_COST: Key = "game.upgrade.level_cost";
pub const T_GAME_SERVED: Key = "game.served";
pub const T_GAME_HARVESTED: Key = "game.harvested";
pub const T_GAME_TIP: Key = "game.tip";
pub const T_GAME_UPGRADES: Key = "game.upgrades";
pub const T_GAME_TABLES: Key = "game.tables";
pub const T_GAME_SPEED: Key = "game.speed";
pub const T_GAME_FLOW: Key = "game.flow";
pub const T_GAME_RESET: Key = "game.reset";
pub const T_GAME_CONFIRM_RESET: Key = "game.confirm_reset";
pub const T_GAME_SHOP_TITLE: Key = "game.shop.title";
pub const T_GAME_ORDER: Key = "game.order";
pub const T_GAME_SERVE: Key = "game.serve";
pub const T_GAME_CLEAN: Key = "game.clean";
pub const T_GAME_GRILL: Key = "game.grill";
pub const T_GAME_FARM_TITLE: Key = "game.farm.title";
pub const T_GAME_PARTY_TITLE: Key = "game.party.title";
pub const T_GAME_PARTY_STATUS_ON: Key = "game.party.status_on";
pub const T_GAME_PARTY_STATUS_OFF: Key = "game.party.status_off";
pub const T_GAME_PARTY_START: Key = "game.party.start";
pub const T_GAME_PARTY_ON: Key = "game.party.on";
pub const T_GAME_PARTY_TIP: Key = "game.party.tip";
pub const T_GAME_GRILL_TITLE: Key = "game.grill.title";
pub const T_GAME_GRILL_STOCK: Key = "game.grill.stock";
pub const T_GAME_GRILL_COOK: Key = "game.grill.cook";
pub const T_GAME_GRILL_COOKING: Key = "game.grill.cooking";
pub const T_GAME_GRILL_DESC: Key = "game.grill.desc";
pub const T_GAME_GRILL_TIP: Key = "game.grill.tip";
pub const T_GAME_LOG_NEW_CUSTOMER: Key = "game.log.new_customer";
pub const T_GAME_LOG_FARM_GREW: Key = "game.log.farm_grew";
pub const T_GAME_LOG_RESET: Key = "game.log.reset";
pub const T_GAME_LOG_MOVED_TO_TABLE: Key = "game.log.moved_to_table";
pub const T_GAME_LOG_TAKING_ORDER: Key = "game.log.taking_order";
pub const T_GAME_LOG_SERVING: Key = "game.log.serving";
pub const T_GAME_LOG_CLEANING: Key = "game.log.cleaning";
pub const T_GAME_LOG_READY_AT_TABLE: Key = "game.log.ready_at_table";
pub const T_GAME_LOG_QUICK_GRILL: Key = "game.log.quick_grill";
pub const T_GAME_LOG_GRILLED_LEFT: Key = "game.log.grilled_left";
pub const T_GAME_LOG_CUSTOMER_LEFT: Key = "game.log.customer_left";
pub const T_GAME_LOG_TABLE_CLEANED: Key = "game.log.table_cleaned";
pub const T_GAME_LOG_PARTY_STARTED: Key = "game.log.party_started";
pub const T_GAME_LOG_COOKING_STARTED: Key = "game.log.cooking_started";
pub const T_GAME_LOG_PLANTED_SEED: Key = "game.log.planted_seed";
pub const T_GAME_LOG_WATERING: Key = "game.log.watering";
pub const T_GAME_LOG_HARVEST: Key = "game.log.harvest";
pub const T_GAME_EVENT_RUSH_HOUR: Key = "game.event.rush_hour";
pub const T_GAME_EVENT_BIG_TIP: Key = "game.event.big_tip";
pub const T_GAME_EVENT_HERB_DELIVERY: Key = "game.event.herb_delivery";
pub const T_GAME_EVENT_DJ_ENERGY: Key = "game.event.dj_energy";
pub const T_GAME_EVENT_GRILL_DEMAND: Key = "game.event.grill_demand";
pub const T_GAME_EVENT_DEFAULT: Key = "game.event.default";
pub const T_GAME_UPGRADE_TABLES: Key = "game.upgrade.tables";
pub const T_GAME_UPGRADE_SPEED: Key = "game.upgrade.speed";
pub const T_GAME_UPGRADE_FLOW: Key = "game.upgrade.flow";
pub const T_GAME_VIP: Key = "game.vip";

// Location quest screen strings
pub const T_LOCATION_QUEST_TITLE: Key = "location_quest.title";
pub const T_LOCATION_QUEST_SUBTITLE: Key = "location_quest.subtitle";
pub const T_LOCATION_QUEST_EMPTY: Key = "location_quest.empty";
pub const T_LOCATION_QUEST_EMPTY_DESC: Key = "location_quest.empty_desc";
pub const T_LOCATION_QUEST_EXPLORE: Key = "location_quest.explore";
pub const T_SCAN_QR_PROMPT: Key = "scan_qr_prompt";
pub const T_QUEST_INVALID_QR: Key = "quest.invalid_qr";
pub const T_QUEST_BAD_RESPONSE: Key = "quest.bad_response";
pub const T_QUEST_ERROR_PREFIX: Key = "quest.error_prefix";
pub const T_QUEST_LOADING: Key = "quest.loading";
pub const T_QUEST_REWARD_BAT: Key = "quest.reward_bat";
pub const T_LOCATION_QUEST_DESC: Key = "location_quest.desc";
pub const T_LOCATION_QUEST_LOCATIONS: Key = "location_quest.locations";
pub const T_LOCATION_QUEST_PLACES: Key = "location_quest.places";
pub const T_LOCATION_QUEST_GO: Key = "location_quest.go";
pub const T_LOCATION_QUEST_DEFAULT_DESC: Key = "location_quest.default_desc";

// Success screen hard-coded strings
pub const T_SUCCESS_TITLE: Key = "success.title";
pub const T_SUCCESS_ORDER_RECEIVED: Key = "success.order_received";
pub const T_SUCCESS_CONTACT_SHORTLY: Key = "success.contact_shortly";
pub const T_SUCCESS_DELIVERY_ESTIMATE: Key = "success.delivery_estimate";
pub const T_SUCCESS_STATUS: Key = "success.status";
pub const T_SUCCESS_CONFIRMED: Key = "success.confirmed";
pub const T_SUCCESS_ETA: Key = "success.eta";
pub const T_SUCCESS_ETA_VALUE: Key = "success.eta_value";
pub const T_SUCCESS_PAYMENT: Key = "success.payment";
pub const T_SUCCESS_CASH_ON_DELIVERY: Key = "success.cash_on_delivery";
pub const T_SUCCESS_BACK_MENU: Key = "success.back_menu";
pub const T_SUCCESS_MY_ORDERS: Key = "success.my_orders";
pub const T_SUCCESS_TRACK_ORDER: Key = "success.track_order";
pub const T_SUCCESS_PUSH_REASSURANCE: Key = "success.push_reassurance";
pub const T_SUCCESS_REWARDS_TITLE: Key = "success.rewards.title";
pub const T_SUCCESS_REWARDS_BONUS: Key = "success.rewards.bonus";
pub const T_SUCCESS_CASHBACK_EARNED: Key = "success.cashback_earned";
pub const T_SUCCESS_SHARE_REFERRAL: Key = "success.share_referral";
pub const T_SUCCESS_REORDER: Key = "success.reorder";
pub const T_SUCCESS_STATUS_LOADING: Key = "success.status.loading";
pub const T_SUCCESS_STATUS_ERROR: Key = "success.status.error";
pub const T_SUCCESS_CASHBACK_ERROR: Key = "success.cashback.error";
pub const T_SUCCESS_RETRY: Key = "success.retry";
pub const T_PROFILE_LOAD_ERROR: Key = "profile.load_error";
pub const T_PROFILE_RETRY: Key = "profile.retry";
pub const T_CART_SYNCING: Key = "cart.syncing";
pub const T_REORDER_DEEP_LINK_TITLE: Key = "reorder.deep_link.title";

// Orders screen hard-coded strings
pub const T_ORDERS_HISTORY: Key = "orders.history";
pub const T_ORDERS_NO_ORDERS: Key = "orders.no_orders";
pub const T_ORDERS_BROWSE_BIKES: Key = "orders.browse_bikes";
pub const T_ORDERS_ORDER: Key = "orders.order";
pub const T_ORDERS_CLOSE: Key = "orders.close";
pub const T_ORDERS_STATUS_PENDING: Key = "orders.status.pending";
pub const T_ORDERS_STATUS_CONFIRMED: Key = "orders.status.confirmed";
pub const T_ORDERS_STATUS_PREPARING: Key = "orders.status.preparing";
pub const T_ORDERS_STATUS_READY: Key = "orders.status.ready";
pub const T_ORDERS_STATUS_OUT_FOR_DELIVERY: Key = "orders.status.out_for_delivery";
pub const T_ORDERS_STATUS_DELIVERED: Key = "orders.status.delivered";
pub const T_ORDERS_STATUS_CANCELLED: Key = "orders.status.cancelled";
pub const T_ORDERS_STATUS_UNKNOWN: Key = "orders.status.unknown";
pub const T_ORDERS_FILTER_ALL: Key = "orders.filter.all";
pub const T_ORDERS_FILTER_ACTIVE: Key = "orders.filter.active";
pub const T_ORDERS_FILTER_COMPLETED: Key = "orders.filter.completed";
pub const T_ORDERS_FILTER_CANCELLED: Key = "orders.filter.cancelled";
pub const T_ORDERS_STEP_RECEIVED: Key = "orders.step.received";
pub const T_ORDERS_STEP_CONFIRMED: Key = "orders.step.confirmed";
pub const T_ORDERS_STEP_PREPARING: Key = "orders.step.preparing";
pub const T_ORDERS_STEP_READY: Key = "orders.step.ready";
pub const T_ORDERS_STEP_ON_THE_WAY: Key = "orders.step.on_the_way";
pub const T_ORDERS_STEP_DELIVERED: Key = "orders.step.delivered";

// Order detail screen
pub const T_ORDER_DETAIL_NOT_FOUND: Key = "order.detail.not_found";
pub const T_ORDER_DETAIL_BACK: Key = "order.detail.back";
pub const T_ORDER_DETAIL_TOTAL: Key = "order.detail.total";
pub const T_ORDER_DETAIL_BONUS: Key = "order.detail.bonus";
pub const T_ORDER_DETAIL_STARS: Key = "order.detail.stars";
pub const T_ORDER_DETAIL_LIVE: Key = "order.detail.live";
pub const T_ORDER_DETAIL_CANCEL: Key = "order.detail.cancel";
pub const T_ORDER_DETAIL_CANCEL_CONFIRM: Key = "order.detail.cancel_confirm";
pub const T_ORDER_DETAIL_CANCELLED_BY_USER: Key = "order.detail.cancelled_by_user";
/// A cancellation refused because the order has left pending (409). Copy chosen
/// 2026-09-25 under the owner's delegation; `client_errors.t27` records it.
pub const T_ORDER_DETAIL_CANCEL_REFUSED: Key = "order.detail.cancel_refused";
pub const T_ORDER_REORDER: Key = "order.reorder";

// Profile screen hard-coded strings
pub const T_PROFILE_MEMBERSHIP: Key = "profile.membership";
pub const T_PROFILE_QR_CODE: Key = "profile.qr_code";
pub const T_PROFILE_COPY_LINK: Key = "profile.copy_link";
pub const T_PROFILE_SHARE: Key = "profile.share";
pub const T_PROFILE_FRIENDS_INVITED: Key = "profile.friends_invited";
pub const T_PROFILE_REFERRAL_LINK: Key = "profile.referral_link";
pub const T_PROFILE_COPY: Key = "profile.copy";
pub const T_PROFILE_INVITED: Key = "profile.invited";
/// `{0}` is what a friend is worth, already formatted by
/// `crate::trios::pricing::format_baht`, symbol included.
///
/// Both translations spelled the amount out — `฿100` — while the real figure is
/// `loyalty_config.referral_bonus`, whose own default is 200. The screen was
/// promising half of what the shop pays, in a string no compiler and no test
/// could relate to the number it was about.
pub const T_PROFILE_EARN_PER_REF: Key = "profile.earn_per_ref";
pub const T_PROFILE_QUICK_ACTIONS: Key = "profile.quick_actions";
pub const T_PROFILE_MY_ORDERS: Key = "profile.my_orders";
pub const T_PROFILE_QUESTS: Key = "profile.quests";
pub const T_PROFILE_REFERRAL_PROGRAM: Key = "profile.referral_program";
pub const T_PROFILE_TIER_BENEFITS: Key = "profile.tier_benefits";
pub const T_PROFILE_TIER_STARTER: Key = "profile.tier_starter";
pub const T_PROFILE_TIER_BRONZE: Key = "profile.tier_bronze";
pub const T_PROFILE_TIER_SILVER: Key = "profile.tier_silver";
pub const T_PROFILE_TIER_GOLD: Key = "profile.tier_gold";
pub const T_PROFILE_SPENT: Key = "profile.spent";
pub const T_PROFILE_BONUS: Key = "profile.bonus";
pub const T_PROFILE_STARS: Key = "profile.stars";
pub const T_PROFILE_CASHBACK_LABEL: Key = "profile.cashback_label";
pub const T_PROFILE_PROGRESS: Key = "profile.progress";
pub const T_PROFILE_MORE_TO_UNLOCK: Key = "profile.more_to_unlock";
pub const T_PROFILE_CONTACTS: Key = "profile.contacts";
pub const T_PROFILE_OPEN_MAP: Key = "profile.open_map";
pub const T_PROFILE_BONUS_HISTORY: Key = "profile.bonus_history";
pub const T_PROFILE_BONUS_HISTORY_EMPTY: Key = "profile.bonus_history_empty";
pub const T_PROFILE_BONUS_CREDIT: Key = "profile.bonus_credit";
pub const T_PROFILE_BONUS_DEBIT: Key = "profile.bonus_debit";
pub const T_PROFILE_BONUS_REFERRAL: Key = "profile.bonus_referral";
pub const T_PROFILE_BONUS_GARDEN: Key = "profile.bonus_garden";
pub const T_PROFILE_BONUS_CASHBACK: Key = "profile.bonus_cashback";
pub const T_PROFILE_BONUS_ADMIN: Key = "profile.bonus_admin";
pub const T_PROFILE_BONUS_OTHER: Key = "profile.bonus_other";
pub const T_PROFILE_ORDER_HISTORY: Key = "profile.order_history";
pub const T_PROFILE_REORDER: Key = "profile.reorder";

// Modal hard-coded strings
pub const T_MODAL_CLOSE: Key = "modal.close";
// Shown by screens whose data depends on Telegram identity when the app is
// opened outside Telegram (a plain browser tab has no initData).
pub const T_OPEN_IN_TELEGRAM_TITLE: Key = "notice.open_in_telegram.title";
pub const T_OPEN_IN_TELEGRAM_BODY: Key = "notice.open_in_telegram.body";
pub const T_MODAL_CONFIRM: Key = "modal.confirm";
pub const T_MODAL_CANCEL: Key = "modal.cancel";
pub const T_MODAL_DECREASE_QTY: Key = "modal.decrease_qty";
pub const T_MODAL_INCREASE_QTY: Key = "modal.increase_qty";
pub const T_MODAL_CERTIFICATE: Key = "modal.certificate";

// Menu screen hard-coded strings
pub const T_MENU_FILTER_SATIVA: Key = "menu.filter.sativa";
pub const T_MENU_FILTER_INDICA: Key = "menu.filter.indica";
pub const T_MENU_FILTER_HYBRID: Key = "menu.filter.hybrid";
pub const T_MENU_SORT_TOP: Key = "menu.sort.top";
pub const T_MENU_SORT_PRICE_ASC: Key = "menu.sort.price_asc";
pub const T_MENU_SORT_PRICE_DESC: Key = "menu.sort.price_desc";
pub const T_MENU_SORT_NAME: Key = "menu.sort.name";
pub const T_MENU_NO_RESULTS: Key = "menu.no_results";
pub const T_MENU_NEW_ARRIVALS: Key = "menu.new_arrivals";
pub const T_MENU_PRICE_REQUEST: Key = "menu.price_request";
pub const T_MENU_SOLD_OUT: Key = "menu.sold_out";
pub const T_MENU_SOTD_BADGE: Key = "menu.sotd_badge";
pub const T_MENU_NEW_BADGE: Key = "menu.new_badge";
pub const T_MENU_BEST_BADGE: Key = "menu.best_badge";
pub const T_MENU_SALE_BADGE: Key = "menu.sale_badge";

/// Product card translations
pub const T_SET_BADGE: Key = "set.badge";
pub const T_MENU_SET_LABEL: Key = "menu.set_label";
pub const T_MENU_OFF: Key = "menu.off";
pub const T_MENU_WEIGHT: Key = "menu.weight";
pub const T_MENU_FLAVOR_PREFIX: Key = "menu.flavor_prefix";
pub const T_MENU_PER_GRAM: Key = "menu.per_gram";

// Home screen hard-coded strings
pub const T_HOME_CATEGORIES: Key = "home.categories";
pub const T_HOME_SETS_PACKS: Key = "home.sets_packs";
pub const T_HOME_ADVENTURES: Key = "home.adventures";
pub const T_HOME_DAILY_QUEST: Key = "home.daily_quest";
pub const T_HOME_TREASURE_HUNT: Key = "home.treasure_hunt";
pub const T_HOME_AR_HUNT: Key = "home.ar_hunt";
pub const T_HOME_LOCATION_QUEST: Key = "home.location_quest";
pub const T_HOME_GAME: Key = "home.game";
pub const T_HOME_SHARE: Key = "home.share";
pub const T_HOME_WATCH_VIDEO: Key = "home.watch_video";
pub const T_HOME_REORDER_LAST: Key = "home.reorder.last";
pub const T_HOME_REORDER_STATUS: Key = "home.reorder.status";
pub const T_HOME_REORDER_CTA: Key = "home.reorder.cta";

// Events screen hard-coded strings
pub const T_EVENTS_TIME: Key = "events.time";
pub const T_EVENTS_SOLD_OUT_BADGE: Key = "events.sold_out_badge";
pub const T_EVENTS_SEATS: Key = "events.seats";
pub const T_EVENTS_FREE_BADGE: Key = "events.free_badge";
pub const T_EVENTS_TELEGRAM_REQUIRED: Key = "events.telegram_required";
pub const T_EVENTS_OK: Key = "events.ok";
pub const T_EVENTS_RETRY: Key = "events.retry";
pub const T_EVENTS_SEAT: Key = "events.seat";
pub const T_EVENTS_SELECT_SEATS: Key = "events.select_seats";
pub const T_EVENTS_OPEN_DETAILS: Key = "events.open_details";
pub const T_EVENTS_SHARE_EVENT: Key = "events.share_event";
pub const T_EVENTS_PREV_PHOTO: Key = "events.prev_photo";
pub const T_EVENTS_NEXT_PHOTO: Key = "events.next_photo";
pub const T_EVENTS_PHOTO_N: Key = "events.photo_n";
pub const T_EVENTS_EVENT_NOT_FOUND: Key = "events.event_not_found";
pub const T_EVENTS_MONTH_JAN: Key = "events.month.jan";
pub const T_EVENTS_MONTH_FEB: Key = "events.month.feb";
pub const T_EVENTS_MONTH_MAR: Key = "events.month.mar";
pub const T_EVENTS_MONTH_APR: Key = "events.month.apr";
pub const T_EVENTS_MONTH_MAY: Key = "events.month.may";
pub const T_EVENTS_MONTH_JUN: Key = "events.month.jun";
pub const T_EVENTS_MONTH_JUL: Key = "events.month.jul";
pub const T_EVENTS_MONTH_AUG: Key = "events.month.aug";
pub const T_EVENTS_MONTH_SEP: Key = "events.month.sep";
pub const T_EVENTS_MONTH_OCT: Key = "events.month.oct";
pub const T_EVENTS_MONTH_NOV: Key = "events.month.nov";
pub const T_EVENTS_MONTH_DEC: Key = "events.month.dec";
pub const T_EVENTS_MY_BOOKINGS: Key = "events.my_bookings";
pub const T_EVENTS_NO_BOOKINGS: Key = "events.no_bookings";
pub const T_EVENTS_CANCEL: Key = "events.cancel";

/// Bike catalog and family detail (D13).
///
/// These keys were written against by `src/ui/screens/catalog_screen.rs` and
/// `src/ui/screens/bike_detail.rs` before they existed here — both screens were
/// undeclared in `screens/mod.rs`, so nothing compiled them and nothing noticed
/// the 50 missing constants. Worth knowing when adding more: `t()` falls back to
/// returning the key itself, so a key missing from a table renders the literal
/// string `bike.price.title` on a customer's screen rather than failing a build.
///
/// The wording carries policy, not just labels:
///   * D11 — when no rate is published the copy says a human quotes it. It never
///     emits a number, a "from", an average or a range, and it is never silent.
///   * D9 — an absent money value is `MONEY_DASH`, so nothing here says `0`.
///   * D14 — availability is a count and a colour list, never a unit identity.
///
/// The `{0}`/`{1}` placeholders are positional for `tf()`, and the argument
/// order is fixed by the call sites named beside each key below.
pub const T_BIKE_CATALOG_TITLE: Key = "bike.catalog.title";
pub const T_BIKE_CATALOG_DESC: Key = "bike.catalog.desc";
pub const T_BIKE_FILTER_SCOOTER: Key = "bike.filter.scooter";
pub const T_BIKE_FILTER_MOTORCYCLE: Key = "bike.filter.motorcycle";
pub const T_BIKE_FILTER_FREE_NOW: Key = "bike.filter.free_now";
pub const T_BIKE_SORT_DEFAULT: Key = "bike.sort.default";
pub const T_BIKE_SORT_PRICE_ASC: Key = "bike.sort.price_asc";
pub const T_BIKE_SORT_PRICE_DESC: Key = "bike.sort.price_desc";
/// `{0}` = the active filter's own label.
pub const T_BIKE_NO_RESULTS: Key = "bike.no_results";
pub const T_BIKE_CLASS_SCOOTER: Key = "bike.class.scooter";
pub const T_BIKE_CLASS_MOTORCYCLE: Key = "bike.class.motorcycle";
/// `{0}` = engine displacement in cc.
pub const T_BIKE_CC: Key = "bike.cc";
pub const T_BIKE_DETAILS: Key = "bike.details";
pub const T_BIKE_BOOK: Key = "bike.book";
pub const T_BIKE_PRICE_TITLE: Key = "bike.price.title";
pub const T_BIKE_RATE_PER_DAY: Key = "bike.rate_per_day";
pub const T_BIKE_PER_DAY: Key = "bike.per_day";
/// D11: no published rate. Names a human, emits no number, is not silence.
pub const T_BIKE_PRICE_ON_REQUEST: Key = "bike.price.on_request";
/// `{0}` = the published tariff, already money-formatted. Shown only when a
/// class discount applies, so the customer can see what it was taken off.
pub const T_BIKE_TARIFF_BEFORE_DISCOUNT: Key = "bike.tariff_before_discount";
/// `{0}` = class discount percent.
pub const T_BIKE_CLASS_DISCOUNT: Key = "bike.class_discount";
pub const T_BIKE_DEPOSIT: Key = "bike.deposit";
pub const T_BIKE_MONTHLY_LOW_SEASON: Key = "bike.monthly_low_season";
pub const T_BIKE_QUOTE_NOTE: Key = "bike.quote_note";
pub const T_BIKE_TERMS_TITLE: Key = "bike.terms.title";
pub const T_BIKE_TERMS_NOTE: Key = "bike.terms.note";
pub const T_BIKE_TERM_WEEK: Key = "bike.term.week";
pub const T_BIKE_TERM_TWO_WEEKS: Key = "bike.term.two_weeks";
pub const T_BIKE_TERM_MONTH: Key = "bike.term.month";
/// `{0}` = min days, `{1}` = max days.
pub const T_BIKE_TERM_DAYS: Key = "bike.term.days";
/// `{0}` = min days, no upper bound published.
pub const T_BIKE_TERM_DAYS_OPEN: Key = "bike.term.days_open";
/// `{0}` = the single published percent for this band.
pub const T_BIKE_TERM_DISCOUNT_ONE: Key = "bike.term.discount_one";
/// `{0}` = low percent, `{1}` = high percent.
pub const T_BIKE_TERM_DISCOUNT_RANGE: Key = "bike.term.discount_range";
pub const T_BIKE_UNITS_TITLE: Key = "bike.units.title";
pub const T_BIKE_UNITS_EMPTY: Key = "bike.units.empty";
/// `{0}` = units free, `{1}` = units total.
pub const T_BIKE_AVAILABILITY: Key = "bike.availability";
/// `{0}` = units free; the total was not published.
pub const T_BIKE_AVAILABILITY_FREE: Key = "bike.availability.free";
/// The API said nothing about availability. Fails closed, names a human.
pub const T_BIKE_AVAILABILITY_UNKNOWN: Key = "bike.availability.unknown";
pub const T_BIKE_COLORS_AVAILABLE: Key = "bike.colors.available";
pub const T_BIKE_COLORS_ALL: Key = "bike.colors.all";
pub const T_BIKE_MODEL_YEARS: Key = "bike.model_years";
pub const T_BIKE_SALE_TITLE: Key = "bike.sale.title";
pub const T_BIKE_SALE_PRICE: Key = "bike.sale.price";
pub const T_BIKE_ASK_MANAGER: Key = "bike.ask_manager";
pub const T_BIKE_NOT_OFFERED_TITLE: Key = "bike.not_offered.title";
/// `{0}` = the families offered instead, already joined with ` · ` (D12).
pub const T_BIKE_NOT_OFFERED_ALTERNATIVES: Key = "bike.not_offered.alternatives";
// The five `BookBlock` arms. Issue #9: a family that cannot be booked shows the
// specs and a disabled control **with the reason named**, never a dead button.
pub const T_BIKE_BOOK_BLOCKED_NOT_OFFERED: Key = "bike.book.blocked.not_offered";
pub const T_BIKE_BOOK_BLOCKED_NO_RATE: Key = "bike.book.blocked.no_rate";
pub const T_BIKE_BOOK_BLOCKED_NO_UNITS: Key = "bike.book.blocked.no_units";
pub const T_BIKE_BOOK_BLOCKED_UNKNOWN_AVAILABILITY: Key = "bike.book.blocked.unknown_availability";
pub const T_BIKE_BOOK_BLOCKED_NOT_WIRED: Key = "bike.book.blocked.not_wired";

/// Get translation for a key and language
pub fn t(lang: Lang, key: Key) -> Value {
    match lang {
        Lang::Russian => get_ru_translation(key),
        Lang::English => get_en_translation(key),
        Lang::Thai => get_en_translation(key), // Fallback to English
        Lang::Chinese => get_en_translation(key), // Fallback to English
        Lang::Hebrew => get_en_translation(key), // Fallback to English
        Lang::German => get_en_translation(key), // Fallback to English
        Lang::French => get_en_translation(key), // Fallback to English
        Lang::Spanish => get_en_translation(key), // Fallback to English
    }
}

/// Get Russian translation
fn get_ru_translation(key: Key) -> Value {
    match key {
        // Navigation
        T_NAV_HOME => "Главная",
        T_NAV_SETS => "Наборы",
        T_NAV_MENU => "Меню",
        T_NAV_ACCESSORIES => "Аксессуары",
        T_NAV_TEA => "Напитки",
        T_NAV_QUEST => "Квест",
        T_NAV_GAME => "Игра",
        T_NAV_EVENTS => "События",
        T_NAV_CART => "Корзина",
        T_NAV_PROFILE => "Профиль",
        T_NAV_MORE => "Ещё",
        T_NAV_FLEET => "Байки",
        T_NAV_RIDE => "Заезд",
        T_NAV_ORDERS => "Заказы",
        T_GAME_HARVEST => "Собрать",
        T_GAME_PLANT => "Посадить",
        T_REFERRAL_INVITEES_TITLE => "🤝 Приглашённые друзья",
        T_REFERRAL_INVITEES_EMPTY => "Пригласи друзей — получайте бонусы вместе",
        T_REFERRAL_INVITEE_JOINED => "присоединился",
        T_REFERRAL_INVITEE_ORDERED => "оформил заказ",
        T_REFERRAL_INVITEE_UNKNOWN => "Друг",
        T_REFERRAL_MILESTONE_TITLE => "Рубежи друзей",
        T_REFERRAL_MILESTONE_SUBTITLE => "{0}/{1} друзей",
        T_REFERRAL_MILESTONE_AWARDED => "🏆 {0} друзей — +{1}",
        T_CLOSE => "Закрыть",
        // Telegram buttons
        T_BTN_CART => "Корзина",
        T_BTN_CHECKOUT => "Оформить",
        T_BTN_PAY => "Оплатить",
        T_BTN_CHECKIN => "Чек-ин",
        T_BTN_ORDER => "Заказать",
        T_BTN_ASK => "Спросить",
        // Coming soon
        T_COMING_SOON => "Скоро появится",
        T_COMING_SOON_DESC => "Мы работаем над этим разделом",
        T_COMING_SOON_WORKING => "Идёт разработка",
        T_BACK_HOME => "Вернуться домой",
        // Categories
        T_CAT_MENU => "Меню",
        T_CAT_ACCESSORIES => "Аксессуары",
        T_CAT_TEA => "Чай",
        // Quest
        T_TITLE => "TurboBaby Island Quest",
        T_SUBTITLE => "Пройди 5 точек на Пхукете",
        T_START_BTN => "Начать квест",
        T_EXPLORE_BTN => "Обзор",
        T_CHECKPOINT_TITLE => "Чекпоинт #{0}",
        T_CURRENT_CHECKPOINT => "Текущий чекпоинт",
        T_PURCHASE_REQUIRED => "Требуется покупка",
        T_PURCHASE_MIN => "Минимальная покупка: 300 бат",
        T_SCAN_QR => "Сканировать QR",
        T_SCAN_QR_DESC => "Отсканируй QR код на этой точке для чек-ина",
        T_SUBMIT_CHECKIN => "Отправить чек-ин",
        T_STATUS_LOCKED => "🔒 Locked",
        T_STATUS_ACTIVE => "🔓 Active",
        T_STATUS_COMPLETED => "✅ Completed",
        T_CHECKIN_SUCCESS => "Чекпоинт открыт!",
        T_NEXT_LOCATION => "Следующая точка открыта!",
        T_QUEST_COMPLETE => "Квест завершён!",
        T_REWARD_CLAIM => "Забери награду:",
        T_REWARD => "🎁 Бонус от TurboBaby",
        T_ERROR_WRONG_ORDER => "Неправильная последовательность",
        T_ERROR_PURCHASE => "Покупка должна быть от 300+ бат",
        T_ERROR_INVALID_QR => "Недействительный QR код",
        T_ERROR_ALREADY_CHECKED => "Уже отмечено",
        T_ERROR_CHECKIN_FAILED => "Ошибка чек-ина",
        T_POINT_1 => "Точка 1: Сливовый залив",
        T_POINT_2 => "Точка 2: Тихая гавань",
        T_POINT_3 => "Точка 3: Джунгли хилл",
        T_POINT_4 => "Точка 4: Пиратская бухта",
        T_POINT_5 => "Точка 5: Триада",
        // Events calendar
        T_EVENTS_TITLE => "📅 События",
        T_EVENTS_SUBTITLE => "Календарь мероприятий TurboBaby",
        T_EVENTS_NO_EVENTS => "На этот день ничего не запланировано",
        T_EVENTS_DATE => "Мероприятие уже началось",
        T_EVENTS_LOCATION => "Локация",
        T_EVENTS_CAPACITY => "Мест",
        T_EVENTS_PRICE => "Цена",
        T_EVENTS_PRICE_STARS => "Цена в Stars",
        T_EVENTS_BOOK => "Забронировать",
        T_EVENTS_BOOKED => "Бронь подтверждена",
        T_EVENTS_BOOK_FREE => "Бесплатно",
        T_EVENTS_SOLD_OUT => "Мест нет",
        T_EVENTS_ERROR => "Не удалось забронировать",
        T_EVENTS_ALREADY_BOOKED => "Уже забронировано. Проверьте Мои брони.",
        T_EVENTS_INSUFFICIENT_STARS => "Недостаточно Stars. Заработайте в играх TurboBaby или пополните баланс.",
        T_EVENTS_REMINDER_BODY => "Напоминаем: вы забронировали мероприятие «{0}».\nНачало: {1}.\n📍 Место: {2}\nЖдём вас!",
        T_EVENTS_VIDEO => "Видео",
        T_EVENTS_GALLERY => "Галерея",
        T_EVENTS_WEEKDAY_MON => "пн",
        T_EVENTS_WEEKDAY_TUE => "вт",
        T_EVENTS_WEEKDAY_WED => "ср",
        T_EVENTS_WEEKDAY_THU => "чт",
        T_EVENTS_WEEKDAY_FRI => "пт",
        T_EVENTS_WEEKDAY_SAT => "сб",
        T_EVENTS_WEEKDAY_SUN => "вс",
        // Screen titles
        T_HOME_TITLE => "Главная",
        T_HOME_SUBTITLE => "Аренда байков на Пхукете · Камала",
        T_MENU_TITLE => "🌿 Меню",
        T_MENU_DESC => "Наши премиальные сорта",
        T_SETS_TITLE => "🎁 Наборы",
        T_SETS_DESC => "Готовые наборы со скидкой",
        T_ACC_TITLE => "🛠️ Аксессуары",
        T_ACC_DESC => "Всё для курения и вейпинга",
        T_ACC_CAT_GRINDER => "Гриндер",
        T_ACC_CAT_PAPERS => "Бумага",
        T_ACC_CAT_PIPE => "Трубка",
        T_ACC_CAT_BONG => "Бонг",
        T_ACC_CAT_STORAGE => "Хранение",
        T_ACC_CAT_LIGHTER => "Зажигалка",
        T_ACC_CAT_CLOTHING => "Одежда",
        T_ACC_CAT_SOUVENIR => "Сувенир",
        T_ACC_CAT_OTHER => "Другое",
        T_TEA_TITLE => "🥤 Напитки",
        T_TEA_DESC => "Чай, кофе и другие напитки",
        T_CART_TITLE => "🛒 Корзина",
        T_CART_EMPTY => "Корзина пуста",
        T_CART_EMPTY_DESC => "Добавьте товары из каталога",
        T_CART_BROWSE_MENU => "🏍 К байкам",
        T_CART_BROWSE_SETS => "🎁 В наборы",
        T_CHECKOUT_TITLE => "🛍️ Оформление",
        T_ORDERS_TITLE => "📋 Заказы",
        T_PROFILE_TITLE => "👤 Профиль",
        T_YOUR_ORDER => "Ваш заказ",
        T_YOUR_INFO => "Ваши данные",
        T_PICKUP_LOCATION => "📍 Точка самовывоза",
        T_DELIVERY => "Доставка",
        T_DELIVERY_ZONE => "Зона доставки",
        T_DELIVERY_ETA => "Время доставки: {0} мин",
        T_DELIVERY_FEE => "Стоимость доставки: {0}",
        T_PAYMENT => "Оплата",
        T_PLACE_ORDER => "Оформить заказ ✓",
        T_CHECKOUT_ERR_NAME => "Укажите ваше имя",
        T_CHECKOUT_ERR_NAME_LONG => "Имя слишком длинное (макс. 200 символов)",
        T_CHECKOUT_ERR_PHONE => "Укажите номер телефона",
        T_CHECKOUT_ERR_PHONE_LONG => "Телефон слишком длинный (макс. 50 символов)",
        T_CHECKOUT_ERR_PHONE_INVALID => "Проверьте номер — например 0812345678 или +66 81 234 5678",
        T_CHECKOUT_BLOCKED_TITLE => "Чтобы оформить заказ:",
        T_CHECKOUT_FULFILLMENT => "Способ получения",
        T_CHECKOUT_FULFILLMENT_DELIVERY => "🛵 Доставка",
        T_CHECKOUT_FULFILLMENT_PICKUP => "🏪 Самовывоз",
        T_CHECKOUT_ERR_AGE => "Подтвердите, что вам есть 20 лет",
        T_CHECKOUT_CHANGE => "изменить",
        T_CHECKOUT_PHONE_FROM_TELEGRAM => "📱 Взять номер из Telegram",
        T_CHECKOUT_ERR_ADDRESS => "Укажите адрес доставки",
        T_CHECKOUT_ERR_ADDRESS_LONG => "Адрес слишком длинный (макс. 500 символов)",
        T_CHECKOUT_ERR_ITEMS => "Корзина пуста — добавьте товары",
        T_CHECKOUT_ERR_NO_TELEGRAM => "Откройте приложение в Telegram, чтобы оформить заказ",
        T_CHECKOUT_ERR_NETWORK => "Ошибка сети. Проверьте соединение и попробуйте снова",
        T_CHECKOUT_ERR_PARSE => "Не удалось обработать ответ сервера. Попробуйте снова",
        T_CHECKOUT_RETRY => "Попробовать снова",
        T_BACK => "← Назад",
        T_TOTAL => "Итого:",
        T_ADD_TO_CART => "В корзину",
        T_FULFILLMENT_LABEL => "Где пить",
        T_FULFILLMENT_DINE_IN => "В заведении",
        T_FULFILLMENT_TAKEAWAY => "С собой",
        T_SHARE => "Поделиться",
        T_SHARE_MESSAGE => "Посмотри {0} в TurboBaby 👇",
        T_REFERRAL_TITLE => "🎁 Реферальная программа",
        T_REFERRAL_SUBTITLE => "Приглашай друзей — получай бонусы",
        T_REFERRAL_LINK_LABEL => "ВАША РЕФЕРАЛЬНАЯ ССЫЛКА",
        T_REFERRAL_COPY => "📋 Копировать",
        T_REFERRAL_COPIED => "✅ Скопировано!",
        T_REFERRAL_SHARE => "📤 Поделиться",
        T_REFERRAL_SHARE_TEXT => "🏍 Присоединяйся к TurboBaby и получай бонусы!",
        T_REFERRAL_STAT_INVITED => "Приглашено",
        T_REFERRAL_STAT_CONFIRMED => "Подтверждено",
        T_REFERRAL_STAT_PENDING => "В ожидании",
        T_REFERRAL_STAT_BONUS => "Бонус",
        T_REFERRAL_TOP => "🏆 Топ рефералов",
        T_REFERRAL_EMPTY_LEADERBOARD => "Пока нет данных — будь первым!",
        T_REFERRAL_ID_MASK => "ID: ⋯{0}",
        T_REFERRAL_ROW_META => "{0} приглашён(а) • {1} заработано",
        T_LOADING => "Загрузка...",
        T_FILTER_ALL => "Все",
        // Variant C
        T_TRUST_GACP => "✅ TURBOBABY CO., LTD. · Камала, Пхукет",
        T_TRUST_MEDICAL => "🪖 Шлем обязателен по закону Таиланда — выдаём с байком.",
        T_TRUST_SUPPORT => "💬 Поддержка",
        T_TRUST_AGE => "🪪 Нужны права",
        T_REORDER => "🔄 Повторить заказ",
        T_SEARCH_PLACEHOLDER => "🔍 Поиск по названию",
        T_BROADCAST => "Рассылка",
        T_BROADCAST_TEXT => "Текст сообщения",
        T_BROADCAST_SEND => "Разослать",
        T_BROADCAST_SENT => "Сообщение разослано",
        T_BROADCAST_PHOTO => "Фото",
        T_BROADCAST_PHOTO_UPLOAD => "Загрузить фото",
        T_BROADCAST_PHOTO_HINT => "Загрузите изображение или оставьте пустым для текстовой рассылки",
        T_BROADCAST_PRODUCT => "Прорекламировать товар",
        T_BROADCAST_PRODUCT_NONE => "Без товара",
        T_BROADCAST_BUTTON_TEXT => "Текст кнопки",
        T_BROADCAST_PREVIEW => "Предпросмотр",
        T_BROADCAST_NO_PRODUCT => "Товар не выбран",
        T_BROADCAST_SELECT_CATALOG => "Выберите каталог",
        T_BROADCAST_SEND_TEST => "Отправить админам (тест)",
        T_BROADCAST_TEST_SENT => "Тестовое сообщение отправлено админам",
        // Checkout error messages (cycle #69)
        T_CHECKOUT_ERR_400 => "Что-то не так с корзиной. Попробуйте очистить её и собрать заново.",
        T_CHECKOUT_ERR_403 => "Аккаунт ограничен. Проверьте Профиль или свяжитесь с поддержкой.",
        T_CHECKOUT_ERR_404 => "Один из товаров больше не доступен. Обновите меню и попробуйте снова.",
        T_CHECKOUT_ERR_409 => "Этот заказ уже создан. Откройте «Мои заказы» — он там.",
        T_CHECKOUT_ERR_422 => "Цены или товары изменились с момента добавления в корзину. Обновите меню и оформите заказ заново.",
        T_CHECKOUT_ERR_AGE_NOT_CONFIRMED => "Для оформления заказа нужно подтвердить, что вам есть 20 лет.",
        T_CHECKOUT_ERR_ZONE_INVALID => "Выбранный район доставки недоступен. Выберите другой.",
        T_CHECKOUT_ERR_429 => "Слишком быстро. Подождите минуту и попробуйте снова.",
        T_CHECKOUT_ERR_5XX => "Сервер сейчас недоступен. Попробуйте через минуту.",
        // Generic API error messages (cycle #74)
        T_API_ERR_401 => "Войдите в Telegram WebApp заново.",
        T_API_ERR_UNKNOWN => "Что-то пошло не так. Попробуйте позже.",
        // Cart screen
        T_CART_SUBTOTAL => "Подытог:",
        T_CART_DELIVERY => "Доставка:",
        T_CART_DELIVERY_FREE => "Бесплатно",
        T_CART_BACK_MENU => "← Меню",
        T_CART_CHECKOUT => "Оформить →",
        T_CART_ITEMS => "{0} товаров",
        T_CART_DINE_IN => "🍽 На месте",
        T_CART_TAKEAWAY => "🥡 С собой",
        T_CART_BONUS_NUDGE => "🎁 У вас {0} бонусов — применим при оформлении",
        T_CART_DECREASE_QTY => "Убавить количество",
        T_CART_REMOVE => "Удалить товар",
        T_CART_IMAGE_ALT => "Фото: {0}",
        T_CART_LINE_EACH => "{0} за шт · {1}",
        // Checkout screen
        T_CHECKOUT_CART_EMPTY => "Корзина пуста",
        T_CHECKOUT_STEP_CART => "Корзина",
        T_CHECKOUT_STEP_DETAILS => "Детали",
        T_CHECKOUT_STEP_CONFIRM => "Подтверждение",
        T_CHECKOUT_SELECT_ZONE => "Выберите зону доставки",
        T_CHECKOUT_STARS => "⭐ Звёзды",
        T_CHECKOUT_STARS_AVAILABLE => "доступно {0}",
        T_CHECKOUT_STARS_MINUS => "−{0}",
        T_CHECKOUT_BONUS => "🎁 Бонусы",
        T_CHECKOUT_BONUS_AVAILABLE => "доступно {0}",
        T_CHECKOUT_BONUS_APPLIED => "−{0}",
        T_CHECKOUT_BONUS_MAX => "макс. {0}",
        T_CHECKOUT_NAME_LABEL => "Имя *",
        T_CHECKOUT_NAME_PLACEHOLDER => "Введите имя",
        T_CHECKOUT_PHONE_LABEL => "Телефон *",
        T_CHECKOUT_PHONE_PLACEHOLDER => "+66 xxx xxx xxxx",
        T_CHECKOUT_OPEN_MAP => "📍 Открыть на карте",
        T_CHECKOUT_ADDRESS_LABEL => "Адрес доставки *",
        T_CHECKOUT_ADDRESS_PLACEHOLDER => "Отель / кондо / улица",
        T_CHECKOUT_USE_MY_LOCATION => "📍 Подставить моё местоположение",
        T_CHECKOUT_NOTES_LABEL => "Комментарий",
        T_CHECKOUT_NOTES_PLACEHOLDER => "Номер комнаты, лобби, встреча у ворот…",
        T_CHECKOUT_CASH_ON_DELIVERY => "Оплата при получении",
        T_CHECKOUT_PAY_ON_RECEIVE => "Оплатите при получении",
        T_CHECKOUT_PROCESSING => "⏳ Оформление...",
        // Checkout trust + age gate micro-copy
        T_CHECKOUT_TRUST_TITLE => "Почему нам доверяют",
        T_CHECKOUT_TRUST_VERIFIED => "🛡️ Проверка возраста (20+)",
        T_CHECKOUT_TRUST_COD => "📦 Оплата при получении",
        T_CHECKOUT_TRUST_SECURE => "🔒 Авторизация через Telegram",
        T_CHECKOUT_AGE_CONFIRM => "Мне исполнилось 20+",
        T_CHECKOUT_AGE_NOTICE => "Оформляя заказ, вы подтверждаете, что вам 20+, и соглашаетесь с правилами медицинского использования.",
        // Catalog shared strings
        T_CATALOG_EMPTY => "Пока нет товаров",
        T_CATALOG_ERROR => "Не удалось загрузить каталог",
        T_CATALOG_SORT_DEFAULT => "По умолчанию",
        T_CATALOG_SORT_POPULAR => "Популярное",
        T_CATALOG_SORT_PRICE => "Цена",
        T_CATALOG_SORT_NEW => "Новинки",
        T_CATALOG_SORT_DISCOUNT => "Скидки",
        T_LOW_STOCK => "⚠ Осталось {0} шт.",
        T_WATCH_VIDEO => "Смотреть видео",
        // Sommelier strings
        // Game strings
        T_GAME_TITLE => "TurboBaby Games",
        T_GAME_TAB_SHOP => "🛒 Магазин",
        T_GAME_TAB_FARM => "🌱 Ферма",
        T_GAME_TAB_DJ => "🎧 DJ",
        T_GAME_TAB_GRILL => "🍖 Гриль",
        T_GAME_TABLE_FREE => "Свободный стол",
        T_GAME_TABLE_WAITING => "Гость ждёт",
        T_GAME_TABLE_READY => "Заказ готов",
        T_GAME_TABLE_EATING => "Гость ест",
        T_GAME_TABLE_DIRTY => "Грязный стол",
        T_GAME_TABLE_PREPARING => "Готовится...",
        T_GAME_FARM_EMPTY => "Пустая грядка",
        T_GAME_FARM_PLANTED => "Росток",
        T_GAME_FARM_WATERED => "Растёт быстро",
        T_GAME_FARM_GROWN => "Готово к сбору!",
        T_GAME_FARM_WATER => "💧 Полить",
        T_GAME_START_PARTY => "Запусти вечеринку, чтобы увеличить доход",
        T_GAME_UPGRADE_MAX => "МАКС",
        T_GAME_UPGRADE_LEVEL_COST => "Ур{0} • {1}🪙",
        T_GAME_SERVED => "Обслужено",
        T_GAME_HARVESTED => "Собрано",
        T_GAME_TIP => "💡 Совет: обслуживай гостей, зарабатывай монеты и прокачивай магазин.",
        T_GAME_UPGRADES => "🆙 УЛУЧШЕНИЯ",
        T_GAME_TABLES => "🪑 Столы",
        T_GAME_SPEED => "⚡ Скорость",
        T_GAME_FLOW => "🚪 Поток",
        T_GAME_RESET => "🔄 Сброс",
        T_GAME_CONFIRM_RESET => "Сбросить весь прогресс? Это нельзя отменить.",
        T_GAME_SHOP_TITLE => "TurboBaby Shop",
        T_GAME_ORDER => "👋 Заказ",
        T_GAME_SERVE => "🤲 Подать",
        T_GAME_CLEAN => "🧽 Убрать",
        T_GAME_GRILL => "🍔 Гриль",
        T_GAME_FARM_TITLE => "🌱 ФЕРМА",
        T_GAME_PARTY_TITLE => "🎧 DJ ЗОНА",
        T_GAME_PARTY_STATUS_ON => "Вечеринка идёт — чаевые +5 🪙 за подачу",
        T_GAME_PARTY_STATUS_OFF => "Запусти вечеринку, чтобы увеличить доход",
        T_GAME_PARTY_START => "🚀 Запустить вечеринку",
        T_GAME_PARTY_ON => "🔥 Вечеринка идёт",
        T_GAME_PARTY_TIP => "Совет: вечеринка добавляет +5 🪙 за подачу",
        T_GAME_GRILL_TITLE => "🍖 ГРИЛЬ",
        T_GAME_GRILL_STOCK => "Запас: {0}",
        T_GAME_GRILL_COOK => "🍳 Готовить",
        T_GAME_GRILL_COOKING => "🔥 Готовится...",
        T_GAME_GRILL_DESC => "Готовь еду. Мгновенно обслуживает голодных гостей в магазине.",
        T_GAME_GRILL_TIP => "Совет: гриль авто-обслуживает голодных гостей",
        T_GAME_LOG_NEW_CUSTOMER => "Новый клиент пришёл",
        T_GAME_LOG_FARM_GREW => "Ферма выросла на шаг",
        T_GAME_LOG_RESET => "Прогресс сброшен",
        T_GAME_LOG_MOVED_TO_TABLE => "Вуди подошёл к столу {0}",
        T_GAME_LOG_TAKING_ORDER => "Принимает заказ у стола {0}",
        T_GAME_LOG_SERVING => "Обслуживает стол {0}",
        T_GAME_LOG_CLEANING => "Убирает стол {0}",
        T_GAME_LOG_READY_AT_TABLE => "{0} готов у стола {1}",
        T_GAME_LOG_QUICK_GRILL => "Быстрый гриль у стола {0}",
        T_GAME_LOG_GRILLED_LEFT => "Гриль-клиент заплатил +{0} 🪙",
        T_GAME_LOG_CUSTOMER_LEFT => "Клиент заплатил +{0} 🪙",
        T_GAME_LOG_TABLE_CLEANED => "Стол убран",
        T_GAME_LOG_PARTY_STARTED => "Вечеринка началась! −{0} 🪙",
        T_GAME_LOG_COOKING_STARTED => "Готовка началась −{0} 🪙",
        T_GAME_LOG_PLANTED_SEED => "Посажено семя −{0} 🪙",
        T_GAME_LOG_WATERING => "Поливаем...",
        T_GAME_LOG_HARVEST => "Урожай! +{0} 🪙",
        T_GAME_EVENT_RUSH_HOUR => "🎉 Час пик! Ещё больше гостей!",
        T_GAME_EVENT_BIG_TIP => "💰 Щедрые чаевые! +20 🪙",
        T_GAME_EVENT_HERB_DELIVERY => "🌿 Доставка травы! Все грядки политы",
        T_GAME_EVENT_DJ_ENERGY => "🎵 DJ-энергия! Вечеринка длится дольше",
        T_GAME_EVENT_GRILL_DEMAND => "🍔 Спрос на гриль! Бесплатный запас еды",
        T_GAME_EVENT_DEFAULT => "🎉 Событие!",
        T_GAME_UPGRADE_TABLES => "🪑 Столы улучшены!",
        T_GAME_UPGRADE_SPEED => "⚡ Обслуживание быстрее!",
        T_GAME_UPGRADE_FLOW => "🚪 Больше клиентов!",
        T_GAME_VIP => "VIP",
        // Location quest screen strings
        T_LOCATION_QUEST_TITLE => "📍 Локационные квесты",
        T_LOCATION_QUEST_SUBTITLE => "Пройди квесты на настоящих локациях острова",
        T_LOCATION_QUEST_EMPTY => "Пока нет квестов",
        T_LOCATION_QUEST_EMPTY_DESC => "Новые квесты появятся здесь, когда станут доступны",
        T_LOCATION_QUEST_EXPLORE => "Исследовать",
        T_SCAN_QR_PROMPT => "Сканируй QR локации",
        T_QUEST_INVALID_QR => "Неверный QR",
        T_QUEST_BAD_RESPONSE => "Некорректный ответ сервера",
        T_QUEST_ERROR_PREFIX => "Ошибка: {0}",
        T_QUEST_LOADING => "Загрузка квеста...",
        T_QUEST_REWARD_BAT => "🎁 +{0} BAT",
        T_LOCATION_QUEST_DESC => "Выполняй квесты в реальных локациях острова",
        T_LOCATION_QUEST_LOCATIONS => "🎯 Локации",
        T_LOCATION_QUEST_PLACES => "{0} мест",
        T_LOCATION_QUEST_GO => "📷 Вперёд",
        T_LOCATION_QUEST_DEFAULT_DESC => "Исследуй эту локацию",
        // Success screen
        T_SUCCESS_TITLE => "Заказ оформлен!",
        T_SUCCESS_ORDER_RECEIVED => "Заказ #{0} получен",
        T_SUCCESS_CONTACT_SHORTLY => "Мы свяжемся с вами в ближайшее время",
        T_SUCCESS_DELIVERY_ESTIMATE => "📦 Ожидаемое время доставки",
        T_SUCCESS_STATUS => "Статус:",
        T_SUCCESS_CONFIRMED => "Подтверждён",
        T_SUCCESS_ETA => "Время доставки:",
        T_SUCCESS_ETA_VALUE => "{0} мин",
        T_SUCCESS_PAYMENT => "Оплата:",
        T_SUCCESS_CASH_ON_DELIVERY => "Наличными при получении",
        T_SUCCESS_BACK_MENU => "🌿 В меню",
        T_SUCCESS_MY_ORDERS => "📋 Мои заказы",
        T_SUCCESS_TRACK_ORDER => "🔔 Отслеживать",
        T_SUCCESS_PUSH_REASSURANCE => "🔔 Push-уведомления о каждом статусе заказа",
        T_SUCCESS_REWARDS_TITLE => "🎁 Что вы получите",
        T_SUCCESS_REWARDS_BONUS => "⭐ Бонусные баллы: {0}",
        T_SUCCESS_CASHBACK_EARNED => "💸 +{0} кешбэка начислено",
        T_SUCCESS_SHARE_REFERRAL => "👥 Пригласить друга",
        T_SUCCESS_REORDER => "🔄 Повторить заказ",
        T_SUCCESS_STATUS_LOADING => "Обновляем статус…",
        T_SUCCESS_STATUS_ERROR => "Не удалось загрузить статус.",
        T_SUCCESS_CASHBACK_ERROR => "Не удалось загрузить кэшбэк.",
        T_SUCCESS_RETRY => "Повторить",
        // Orders screen
        T_ORDERS_HISTORY => "История заказов",
        T_ORDERS_NO_ORDERS => "Пока нет заказов",
        T_ORDERS_BROWSE_BIKES => "🏍 К байкам",
        T_ORDERS_ORDER => "Заказ №{0}",
        T_ORDERS_CLOSE => "Закрыть",
        T_ORDERS_STATUS_PENDING => "⏳ Ожидает",
        T_ORDERS_STATUS_CONFIRMED => "✅ Подтверждён",
        T_ORDERS_STATUS_PREPARING => "🔥 Готовится",
        T_ORDERS_STATUS_READY => "📦 Готов",
        T_ORDERS_STATUS_OUT_FOR_DELIVERY => "🚗 В пути",
        T_ORDERS_STATUS_DELIVERED => "✅ Доставлен",
        T_ORDERS_STATUS_CANCELLED => "❌ Отменён",
        T_ORDERS_STATUS_UNKNOWN => "📋 Неизвестно",
        T_ORDERS_FILTER_ALL => "Все",
        T_ORDERS_FILTER_ACTIVE => "🔄 Активные",
        T_ORDERS_FILTER_COMPLETED => "✅ Завершённые",
        T_ORDERS_FILTER_CANCELLED => "❌ Отменённые",
        T_ORDERS_STEP_RECEIVED => "Получен",
        T_ORDERS_STEP_CONFIRMED => "Подтверждён",
        T_ORDERS_STEP_PREPARING => "Готовится",
        T_ORDERS_STEP_READY => "Готов",
        T_ORDERS_STEP_ON_THE_WAY => "В пути",
        T_ORDERS_STEP_DELIVERED => "Доставлен",
        T_ORDER_DETAIL_NOT_FOUND => "Заказ не найден или недоступен",
        T_ORDER_DETAIL_BACK => "← К заказам",
        T_ORDER_DETAIL_TOTAL => "Итого",
        T_ORDER_DETAIL_BONUS => "Бонусы",
        T_ORDER_DETAIL_STARS => "Звёзды",
        T_ORDER_DETAIL_LIVE => "Обновляется live",
        T_ORDER_DETAIL_CANCEL => "Отменить заказ",
        T_ORDER_DETAIL_CANCEL_CONFIRM => "Отменить заказ? Бонусы и звёзды вернутся на счёт.",
        T_ORDER_DETAIL_CANCELLED_BY_USER => "Вы отменили заказ",
        T_ORDER_DETAIL_CANCEL_REFUSED => "Этот заказ уже в работе, отменить его в приложении нельзя. Напишите менеджеру.",
        T_ORDER_REORDER => "Повторить заказ",
        // Profile screen
        T_PROFILE_MEMBERSHIP => "Ваш статус",
        T_PROFILE_QR_CODE => "Ваш QR-код",
        T_PROFILE_COPY_LINK => "Копировать ссылку",
        T_PROFILE_SHARE => "Поделиться",
        T_PROFILE_FRIENDS_INVITED => "👥 Приглашено друзей: {0}",
        T_PROFILE_REFERRAL_LINK => "🔗 Реферальная ссылка",
        T_PROFILE_COPY => "Копировать",
        T_PROFILE_INVITED => "👥 Приглашено: {0}",
        T_PROFILE_EARN_PER_REF => "Получайте {0} за друга",
        T_PROFILE_QUICK_ACTIONS => "Быстрые действия",
        T_PROFILE_MY_ORDERS => "Мои заказы",
        T_PROFILE_QUESTS => "Квесты",
        T_PROFILE_REFERRAL_PROGRAM => "Реферальная программа",
        T_PROFILE_TIER_BENEFITS => "💎 Привилегии уровня",
        T_PROFILE_TIER_STARTER => "Новичок",
        T_PROFILE_TIER_BRONZE => "Бронзовое колесо",
        T_PROFILE_TIER_SILVER => "Серебряное колесо",
        T_PROFILE_TIER_GOLD => "Золотое колесо",
        T_PROFILE_SPENT => "ПОТРАЧЕНО",
        T_PROFILE_BONUS => "БОНУСЫ",
        T_PROFILE_STARS => "ЗВЁЗДЫ",
        T_PROFILE_CASHBACK_LABEL => "КЭШБЭК",
        T_PROFILE_PROGRESS => "Прогресс до {0}",
        T_PROFILE_MORE_TO_UNLOCK => "Ещё {0} до {1}",
        T_PROFILE_CONTACTS => "📍 Контакты",
        T_PROFILE_OPEN_MAP => "Открыть на карте",
        T_PROFILE_BONUS_HISTORY => "📜 История бонусов",
        T_PROFILE_BONUS_HISTORY_EMPTY => "Пока нет начислений",
        T_PROFILE_BONUS_CREDIT => "Зачисление",
        T_PROFILE_BONUS_DEBIT => "Списание",
        T_PROFILE_LOAD_ERROR => "Не удалось загрузить профиль.",
        T_PROFILE_RETRY => "Повторить",
        T_PROFILE_BONUS_REFERRAL => "Реферальный бонус",
        T_PROFILE_BONUS_GARDEN => "Награда из сада",
        T_PROFILE_BONUS_CASHBACK => "Кэшбэк с заказа",
        T_PROFILE_BONUS_ADMIN => "Админ-начисление",
        T_PROFILE_BONUS_OTHER => "Бонус",
        T_PROFILE_ORDER_HISTORY => "История заказов",
        T_PROFILE_REORDER => "Повторить",
        T_CART_SYNCING => "Синхронизация корзины…",
        T_REORDER_DEEP_LINK_TITLE => "🔄 Ваш последний заказ готов к повтору",
        // Modal
        T_MODAL_CLOSE => "Закрыть",
        T_OPEN_IN_TELEGRAM_TITLE => "Откройте в Telegram",
        T_OPEN_IN_TELEGRAM_BODY => "Это приложение работает внутри Telegram. Откройте бота TurboBaby и нажмите кнопку меню внизу экрана.",
        T_MODAL_CONFIRM => "Да",
        T_MODAL_CANCEL => "Нет",
        T_MODAL_DECREASE_QTY => "Убавить количество",
        T_MODAL_INCREASE_QTY => "Добавить количество",
        T_MODAL_CERTIFICATE => "📄 Сертификат",
        // Menu screen
        T_MENU_FILTER_SATIVA => "☀️ Sativa",
        T_MENU_FILTER_INDICA => "🌙 Indica",
        T_MENU_FILTER_HYBRID => "⚖️ Hybrid",
        T_MENU_SORT_TOP => "✨ Топ",
        T_MENU_SORT_PRICE_ASC => "💰 Цена ↑",
        T_MENU_SORT_PRICE_DESC => "💰 Цена ↓",
        T_MENU_SORT_NAME => "А–Я",
        T_MENU_NO_RESULTS => "Не найдено {0} сортов",
        T_MENU_NEW_ARRIVALS => "🆕 НОВИНКИ",
        T_MENU_PRICE_REQUEST => "Цена по запросу",
        T_MENU_SOLD_OUT => "Нет в наличии",
        T_MENU_SOTD_BADGE => "⭐ СОТД",
        T_MENU_NEW_BADGE => "🆕 НОВИНКА",
        T_MENU_BEST_BADGE => "⭐ ЛУЧШЕЕ",
        T_MENU_SALE_BADGE => "🔥 СКИДКА",
        T_MENU_SET_LABEL => "📦 НАБОР",
        T_SET_BADGE => "📦 НАБОР",
        T_MENU_OFF => "{0}%",
        T_MENU_WEIGHT => "⚖️ {0}",
        T_MENU_FLAVOR_PREFIX => "🍃 {0}",
        T_MENU_PER_GRAM => "/г",
        // Home screen
        T_HOME_CATEGORIES => "Категории",
        T_HOME_SETS_PACKS => "📦 Наборы",
        T_HOME_ADVENTURES => "🎯 Приключения",
        T_HOME_DAILY_QUEST => "Ежедневный квест",
        T_HOME_TREASURE_HUNT => "Охота за сокровищами",
        T_HOME_AR_HUNT => "AR-охота",
        T_HOME_LOCATION_QUEST => "Локационный квест",
        T_HOME_GAME => "Игра",
        T_HOME_SHARE => "Поделиться",
        T_HOME_WATCH_VIDEO => "Смотреть видео",
        T_HOME_REORDER_LAST => "Последний заказ",
        T_HOME_REORDER_STATUS => "Статус",
        T_HOME_REORDER_CTA => "Повторить заказ",
        // Events screen
        T_EVENTS_TIME => "🕒 {0}",
        T_EVENTS_SOLD_OUT_BADGE => "МЕСТ НЕТ",
        T_EVENTS_SEATS => "{0} мест",
        T_EVENTS_FREE_BADGE => "БЕСПЛАТНО",
        T_EVENTS_TELEGRAM_REQUIRED => "Нужен Telegram",
        T_EVENTS_OK => "ОК",
        T_EVENTS_RETRY => "Повторить",
        T_EVENTS_SEAT => "место",
        T_EVENTS_SELECT_SEATS => "Количество мест",
        T_EVENTS_OPEN_DETAILS => "Открыть детали",
        T_EVENTS_SHARE_EVENT => "Поделиться",
        T_EVENTS_PREV_PHOTO => "Предыдущее фото",
        T_EVENTS_NEXT_PHOTO => "Следующее фото",
        T_EVENTS_PHOTO_N => "Фото {0}",
        T_EVENTS_EVENT_NOT_FOUND => "Событие не найдено",
        T_EVENTS_MONTH_JAN => "янв",
        T_EVENTS_MONTH_FEB => "фев",
        T_EVENTS_MONTH_MAR => "мар",
        T_EVENTS_MONTH_APR => "апр",
        T_EVENTS_MONTH_MAY => "май",
        T_EVENTS_MONTH_JUN => "июн",
        T_EVENTS_MONTH_JUL => "июл",
        T_EVENTS_MONTH_AUG => "авг",
        T_EVENTS_MONTH_SEP => "сен",
        T_EVENTS_MONTH_OCT => "окт",
        T_EVENTS_MONTH_NOV => "ноя",
        T_EVENTS_MONTH_DEC => "дек",
        T_EVENTS_MY_BOOKINGS => "Мои бронирования",
        T_EVENTS_NO_BOOKINGS => "Бронирований пока нет",
        T_EVENTS_CANCEL => "Отмена",

        // Bikes
        T_BIKE_CATALOG_TITLE => "Байки",
        T_BIKE_CATALOG_DESC => "Аренда на Пхукете",
        T_BIKE_FILTER_SCOOTER => "Скутеры",
        T_BIKE_FILTER_MOTORCYCLE => "Мотоциклы",
        T_BIKE_FILTER_FREE_NOW => "Свободны сейчас",
        T_BIKE_SORT_DEFAULT => "По умолчанию",
        T_BIKE_SORT_PRICE_ASC => "Сначала дешевле",
        T_BIKE_SORT_PRICE_DESC => "Сначала дороже",
        T_BIKE_NO_RESULTS => "Под фильтр «{0}» ничего не подошло",
        T_BIKE_CLASS_SCOOTER => "Скутер",
        T_BIKE_CLASS_MOTORCYCLE => "Мотоцикл",
        T_BIKE_CC => "{0} см³",
        T_BIKE_DETAILS => "Подробнее",
        T_BIKE_BOOK => "Забронировать",
        T_BIKE_PRICE_TITLE => "Цена",
        T_BIKE_RATE_PER_DAY => "Аренда",
        T_BIKE_PER_DAY => "в сутки",
        T_BIKE_PRICE_ON_REQUEST => "Цену на эту модель называет менеджер — напишите нам",
        T_BIKE_TARIFF_BEFORE_DISCOUNT => "Тариф до скидки: {0}",
        T_BIKE_CLASS_DISCOUNT => "Скидка класса: −{0}%",
        T_BIKE_DEPOSIT => "Залог:",
        T_BIKE_MONTHLY_LOW_SEASON => "Месяц в низкий сезон:",
        T_BIKE_QUOTE_NOTE => "Итоговую сумму подтверждает менеджер: она зависит от срока, сезона и наличия.",
        T_BIKE_TERMS_TITLE => "Сроки и скидки",
        T_BIKE_TERMS_NOTE => "Это опубликованные скидки за срок. Точную сумму подтверждает менеджер.",
        T_BIKE_TERM_WEEK => "Неделя",
        T_BIKE_TERM_TWO_WEEKS => "Две недели",
        T_BIKE_TERM_MONTH => "Месяц",
        T_BIKE_TERM_DAYS => "{0}–{1} дней",
        T_BIKE_TERM_DAYS_OPEN => "от {0} дней",
        T_BIKE_TERM_DISCOUNT_ONE => "−{0}%",
        T_BIKE_TERM_DISCOUNT_RANGE => "−{0}…−{1}%",
        T_BIKE_UNITS_TITLE => "Наличие",
        T_BIKE_UNITS_EMPTY => "Сейчас все байки этой модели заняты.",
        T_BIKE_AVAILABILITY => "Свободно {0} из {1}",
        T_BIKE_AVAILABILITY_FREE => "Свободно: {0}",
        T_BIKE_AVAILABILITY_UNKNOWN => "Наличие уточняет менеджер",
        T_BIKE_COLORS_AVAILABLE => "Свободные цвета:",
        T_BIKE_COLORS_ALL => "Цвета модели:",
        T_BIKE_MODEL_YEARS => "Годы выпуска:",
        T_BIKE_SALE_TITLE => "Выкуп",
        T_BIKE_SALE_PRICE => "Цена выкупа",
        T_BIKE_ASK_MANAGER => "Написать менеджеру",
        T_BIKE_NOT_OFFERED_TITLE => "Эта модель сейчас не сдаётся",
        T_BIKE_NOT_OFFERED_ALTERNATIVES => "Вместо неё: {0}",
        T_BIKE_BOOK_BLOCKED_NOT_OFFERED => "Модель закрыта для новых броней",
        T_BIKE_BOOK_BLOCKED_NO_RATE => "Цена не опубликована — бронь оформляет менеджер",
        T_BIKE_BOOK_BLOCKED_NO_UNITS => "Все байки этой модели заняты",
        T_BIKE_BOOK_BLOCKED_UNKNOWN_AVAILABILITY => "Наличие не подтверждено",
        T_BIKE_BOOK_BLOCKED_NOT_WIRED => "Бронь из приложения ещё не включена — напишите менеджеру",

        _ => key,
    }
}

/// Get English translation
fn get_en_translation(key: Key) -> Value {
    match key {
        // Navigation
        T_NAV_HOME => "Home",
        T_NAV_SETS => "Sets",
        T_NAV_MENU => "Menu",
        T_NAV_ACCESSORIES => "Accessories",
        T_NAV_TEA => "Drinks",
        T_NAV_QUEST => "Quest",
        T_NAV_GAME => "Game",
        T_NAV_EVENTS => "Events",
        T_NAV_CART => "Cart",
        T_NAV_PROFILE => "Profile",
        T_NAV_MORE => "More",
        T_NAV_FLEET => "Bikes",
        T_NAV_RIDE => "Ride",
        T_NAV_ORDERS => "Orders",
        T_GAME_HARVEST => "Harvest",
        T_GAME_PLANT => "Plant",
        T_REFERRAL_INVITEES_TITLE => "🤝 Friends you invited",
        T_REFERRAL_INVITEES_EMPTY => "Invite friends — earn bonuses together",
        T_REFERRAL_INVITEE_JOINED => "joined",
        T_REFERRAL_INVITEE_ORDERED => "placed an order",
        T_REFERRAL_INVITEE_UNKNOWN => "Friend",
        T_REFERRAL_MILESTONE_TITLE => "Friend milestones",
        T_REFERRAL_MILESTONE_SUBTITLE => "{0}/{1} friends",
        T_REFERRAL_MILESTONE_AWARDED => "🏆 {0} friends — +{1}",
        T_CLOSE => "Close",
        // Telegram buttons
        T_BTN_CART => "Cart",
        T_BTN_CHECKOUT => "Checkout",
        T_BTN_PAY => "Pay",
        T_BTN_CHECKIN => "Check In",
        T_BTN_ORDER => "Order",
        T_BTN_ASK => "Ask",
        // Coming soon
        T_COMING_SOON => "Coming Soon",
        T_COMING_SOON_DESC => "We're working on this section",
        T_COMING_SOON_WORKING => "Under development",
        T_BACK_HOME => "Back to Home",
        // Categories
        T_CAT_MENU => "Menu",
        T_CAT_ACCESSORIES => "Accessories",
        T_CAT_TEA => "Tea",
        // Quest
        T_TITLE => "TurboBaby Island Quest",
        T_SUBTITLE => "Complete 5 locations on Phuket",
        T_START_BTN => "Start Quest",
        T_EXPLORE_BTN => "Explore",
        T_CHECKPOINT_TITLE => "Checkpoint #{0}",
        T_CURRENT_CHECKPOINT => "Current Checkpoint",
        T_PURCHASE_REQUIRED => "Purchase Required",
        T_PURCHASE_MIN => "Minimum purchase: 300 THB",
        T_SCAN_QR => "Scan QR",
        T_SCAN_QR_DESC => "Scan the QR code at this location to check in",
        T_SUBMIT_CHECKIN => "Submit Check-in",
        T_STATUS_LOCKED => "Locked",
        T_STATUS_ACTIVE => "Active",
        T_STATUS_COMPLETED => "Completed",
        T_CHECKIN_SUCCESS => "Checkpoint Unlocked!",
        T_NEXT_LOCATION => "Next location unlocked!",
        T_QUEST_COMPLETE => "Quest Complete!",
        T_REWARD_CLAIM => "Claim your reward:",
        T_REWARD => "🎁 Bonus from TurboBaby",
        T_ERROR_WRONG_ORDER => "Wrong location in sequence",
        T_ERROR_PURCHASE => "Purchase must be 300+ THB",
        T_ERROR_INVALID_QR => "Invalid QR code",
        T_ERROR_ALREADY_CHECKED => "Already checked in",
        T_ERROR_CHECKIN_FAILED => "Check-in failed",
        T_POINT_1 => "Point 1: Plum Bay",
        T_POINT_2 => "Point 2: Quiet Harbor",
        T_POINT_3 => "Point 3: Jungle Hill",
        T_POINT_4 => "Point 4: Pirate Bay",
        T_POINT_5 => "Point 5: Triad",
        // Events calendar
        T_EVENTS_TITLE => "📅 Events",
        T_EVENTS_SUBTITLE => "TurboBaby events calendar",
        T_EVENTS_NO_EVENTS => "Nothing planned for this day",
        T_EVENTS_DATE => "Event already started",
        T_EVENTS_LOCATION => "Location",
        T_EVENTS_CAPACITY => "Capacity",
        T_EVENTS_PRICE => "Price",
        T_EVENTS_PRICE_STARS => "Price in Stars",
        T_EVENTS_BOOK => "Book now",
        T_EVENTS_BOOKED => "Booking confirmed",
        T_EVENTS_BOOK_FREE => "Free",
        T_EVENTS_SOLD_OUT => "Sold out",
        T_EVENTS_ERROR => "Booking failed",
        T_EVENTS_ALREADY_BOOKED => "Already booked. Check My bookings.",
        T_EVENTS_INSUFFICIENT_STARS => "Not enough Stars. Earn them in TurboBaby games or top up your balance.",
        T_EVENTS_REMINDER_BODY => "Reminder: you booked «{0}».\nStarts at: {1}.\n📍 Location: {2}\nSee you there!",
        T_EVENTS_VIDEO => "Video",
        T_EVENTS_GALLERY => "Gallery",
        T_EVENTS_WEEKDAY_MON => "Mon",
        T_EVENTS_WEEKDAY_TUE => "Tue",
        T_EVENTS_WEEKDAY_WED => "Wed",
        T_EVENTS_WEEKDAY_THU => "Thu",
        T_EVENTS_WEEKDAY_FRI => "Fri",
        T_EVENTS_WEEKDAY_SAT => "Sat",
        T_EVENTS_WEEKDAY_SUN => "Sun",
        // Screen titles
        T_HOME_TITLE => "Home",
        T_HOME_SUBTITLE => "Bike rental in Phuket · Kamala",
        T_MENU_TITLE => "🌿 Menu",
        T_MENU_DESC => "Our premium selection",
        T_SETS_TITLE => "🎁 Sets",
        T_SETS_DESC => "Ready-made packs at a discount",
        T_ACC_TITLE => "🛠️ Accessories",
        T_ACC_DESC => "Smoking and vaping gear",
        T_ACC_CAT_GRINDER => "Grinder",
        T_ACC_CAT_PAPERS => "Papers",
        T_ACC_CAT_PIPE => "Pipe",
        T_ACC_CAT_BONG => "Bong",
        T_ACC_CAT_STORAGE => "Storage",
        T_ACC_CAT_LIGHTER => "Lighter",
        T_ACC_CAT_CLOTHING => "Clothing",
        T_ACC_CAT_SOUVENIR => "Souvenir",
        T_ACC_CAT_OTHER => "Other",
        T_TEA_TITLE => "🥤 Drinks",
        T_TEA_DESC => "Tea, coffee & more",
        T_CART_TITLE => "🛒 Cart",
        T_CART_EMPTY => "Your cart is empty",
        T_CART_EMPTY_DESC => "Browse the menu to add items",
        T_CART_BROWSE_MENU => "🏍 Browse bikes",
        T_CART_BROWSE_SETS => "🎁 Browse sets",
        T_CHECKOUT_TITLE => "🛍️ Checkout",
        T_ORDERS_TITLE => "📋 Orders",
        T_PROFILE_TITLE => "👤 Profile",
        T_YOUR_ORDER => "Your Order",
        T_YOUR_INFO => "Your Info",
        T_PICKUP_LOCATION => "Pickup Location",
        T_DELIVERY => "Delivery",
        T_DELIVERY_ZONE => "Delivery Zone",
        T_DELIVERY_ETA => "Delivery time: {0} min",
        T_DELIVERY_FEE => "Delivery fee: {0}",
        T_PAYMENT => "Payment",
        T_PLACE_ORDER => "Place Order ✓",
        T_CHECKOUT_ERR_NAME => "Please enter your name",
        T_CHECKOUT_ERR_NAME_LONG => "Name is too long (max 200 characters)",
        T_CHECKOUT_ERR_PHONE => "Please enter your phone number",
        T_CHECKOUT_ERR_PHONE_LONG => "Phone number is too long (max 50 characters)",
        T_CHECKOUT_ERR_PHONE_INVALID => "Check the number — e.g. 0812345678 or +66 81 234 5678",
        T_CHECKOUT_BLOCKED_TITLE => "To place your order:",
        T_CHECKOUT_FULFILLMENT => "How you get it",
        T_CHECKOUT_FULFILLMENT_DELIVERY => "🛵 Delivery",
        T_CHECKOUT_FULFILLMENT_PICKUP => "🏪 Pickup",
        T_CHECKOUT_ERR_AGE => "Confirm that you are 20 or older",
        T_CHECKOUT_CHANGE => "change",
        T_CHECKOUT_PHONE_FROM_TELEGRAM => "📱 Use my Telegram number",
        T_CHECKOUT_ERR_ADDRESS => "Please enter a delivery address",
        T_CHECKOUT_ERR_ADDRESS_LONG => "Address is too long (max 500 characters)",
        T_CHECKOUT_ERR_ITEMS => "Your cart is empty — add items to continue",
        T_CHECKOUT_ERR_NO_TELEGRAM => "Open the app in Telegram to place an order",
        T_CHECKOUT_ERR_NETWORK => "Network error. Check your connection and try again",
        T_CHECKOUT_ERR_PARSE => "Could not process server response. Please try again",
        T_CHECKOUT_RETRY => "Try again",
        T_BACK => "← Back",
        T_TOTAL => "Total:",
        T_ADD_TO_CART => "Add to Cart",
        T_FULFILLMENT_LABEL => "Where to drink",
        T_FULFILLMENT_DINE_IN => "Dine in",
        T_FULFILLMENT_TAKEAWAY => "Takeaway",
        T_SHARE => "Share",
        T_SHARE_MESSAGE => "Check out {0} in TurboBaby 👇",
        T_REFERRAL_TITLE => "🎁 Referral Program",
        T_REFERRAL_SUBTITLE => "Invite friends — earn bonuses",
        T_REFERRAL_LINK_LABEL => "YOUR REFERRAL LINK",
        T_REFERRAL_COPY => "📋 Copy",
        T_REFERRAL_COPIED => "✅ Copied!",
        T_REFERRAL_SHARE => "📤 Share",
        T_REFERRAL_SHARE_TEXT => "🏍 Join TurboBaby and get bonuses!",
        T_REFERRAL_STAT_INVITED => "Invited",
        T_REFERRAL_STAT_CONFIRMED => "Confirmed",
        T_REFERRAL_STAT_PENDING => "Pending",
        T_REFERRAL_STAT_BONUS => "Bonus",
        T_REFERRAL_TOP => "🏆 Top Referrers",
        T_REFERRAL_EMPTY_LEADERBOARD => "No data yet — be the first!",
        T_REFERRAL_ID_MASK => "ID: ⋯{0}",
        T_REFERRAL_ROW_META => "{0} invited • {1} earned",
        T_LOADING => "Loading...",
        T_FILTER_ALL => "All",
        // Variant C
        T_TRUST_GACP => "✅ TURBOBABY CO., LTD. · Kamala, Phuket",
        T_TRUST_MEDICAL => "🪖 Helmets are required by Thai law — provided with every bike.",
        T_TRUST_SUPPORT => "💬 Support",
        T_TRUST_AGE => "🪪 License required",
        T_REORDER => "🔄 Reorder",
        T_SEARCH_PLACEHOLDER => "🔍 Search by name",
        T_BROADCAST => "Broadcast",
        T_BROADCAST_TEXT => "Message text",
        T_BROADCAST_SEND => "Send broadcast",
        T_BROADCAST_SENT => "Broadcast sent",
        T_BROADCAST_PHOTO => "Photo",
        T_BROADCAST_PHOTO_UPLOAD => "Upload photo",
        T_BROADCAST_PHOTO_HINT => "Upload an image or leave empty for a text-only broadcast",
        T_BROADCAST_PRODUCT => "Advertise a product",
        T_BROADCAST_PRODUCT_NONE => "No product",
        T_BROADCAST_BUTTON_TEXT => "Button text",
        T_BROADCAST_PREVIEW => "Preview",
        T_BROADCAST_NO_PRODUCT => "No product selected",
        T_BROADCAST_SELECT_CATALOG => "Select catalog",
        T_BROADCAST_SEND_TEST => "Send to admins (test)",
        T_BROADCAST_TEST_SENT => "Test message sent to admins",
        // Checkout error messages (cycle #69)
        T_CHECKOUT_ERR_400 => "Something looks wrong with your cart. Try clearing it and adding items again.",
        T_CHECKOUT_ERR_403 => "Account restricted. Check Profile or contact support.",
        T_CHECKOUT_ERR_404 => "One of the items is no longer available. Refresh the menu and try again.",
        T_CHECKOUT_ERR_409 => "This order has already been placed. Open «My Orders» — it's there.",
        T_CHECKOUT_ERR_422 => "Prices or items changed since you added to cart. Refresh the menu and place the order again.",
        T_CHECKOUT_ERR_AGE_NOT_CONFIRMED => "Please confirm you are 20+ to place an order.",
        T_CHECKOUT_ERR_ZONE_INVALID => "Selected delivery zone is unavailable. Please choose another.",
        T_CHECKOUT_ERR_429 => "Too fast. Wait a minute and try again.",
        T_CHECKOUT_ERR_5XX => "Server is currently unavailable. Try again in a minute.",
        // Generic API error messages (cycle #74)
        T_API_ERR_401 => "Sign in to Telegram WebApp again.",
        T_API_ERR_UNKNOWN => "Something went wrong. Please try again later.",
        // Cart screen
        T_CART_SUBTOTAL => "Subtotal:",
        T_CART_DELIVERY => "Delivery:",
        T_CART_DELIVERY_FREE => "Free",
        T_CART_BACK_MENU => "← Menu",
        T_CART_CHECKOUT => "Checkout →",
        T_CART_ITEMS => "{0} items",
        T_CART_DINE_IN => "🍽 Dine-in",
        T_CART_TAKEAWAY => "🥡 Takeaway",
        T_CART_BONUS_NUDGE => "🎁 {0} bonus available at checkout",
        T_CART_DECREASE_QTY => "Decrease quantity",
        T_CART_REMOVE => "Remove item",
        T_CART_IMAGE_ALT => "Photo: {0}",
        T_CART_LINE_EACH => "{0} each · {1}",
        // Checkout screen
        T_CHECKOUT_CART_EMPTY => "Cart is empty",
        T_CHECKOUT_STEP_CART => "Cart",
        T_CHECKOUT_STEP_DETAILS => "Details",
        T_CHECKOUT_STEP_CONFIRM => "Confirm",
        T_CHECKOUT_SELECT_ZONE => "Select delivery zone",
        T_CHECKOUT_STARS => "⭐ Stars",
        T_CHECKOUT_STARS_AVAILABLE => "{0} available",
        T_CHECKOUT_STARS_MINUS => "−{0}",
        T_CHECKOUT_BONUS => "🎁 Bonus",
        T_CHECKOUT_BONUS_AVAILABLE => "{0} available",
        T_CHECKOUT_BONUS_APPLIED => "−{0}",
        T_CHECKOUT_BONUS_MAX => "max {0}",
        T_CHECKOUT_NAME_LABEL => "Name *",
        T_CHECKOUT_NAME_PLACEHOLDER => "Enter your name",
        T_CHECKOUT_PHONE_LABEL => "Phone *",
        T_CHECKOUT_PHONE_PLACEHOLDER => "+66 xxx xxx xxxx",
        T_CHECKOUT_OPEN_MAP => "📍 Open map",
        T_CHECKOUT_ADDRESS_LABEL => "Delivery address *",
        T_CHECKOUT_ADDRESS_PLACEHOLDER => "Hotel / condo / street address",
        T_CHECKOUT_USE_MY_LOCATION => "📍 Use my location",
        T_CHECKOUT_NOTES_LABEL => "Notes",
        T_CHECKOUT_NOTES_PLACEHOLDER => "Room number, lobby, meet at gate…",
        T_CHECKOUT_CASH_ON_DELIVERY => "Cash on Delivery",
        T_CHECKOUT_PAY_ON_RECEIVE => "Pay when you receive",
        T_CHECKOUT_PROCESSING => "⏳ Processing...",
        // Checkout trust + age gate micro-copy
        T_CHECKOUT_TRUST_TITLE => "Fast & discreet",
        T_CHECKOUT_TRUST_VERIFIED => "🛡️ Age-verified orders (20+)",
        T_CHECKOUT_TRUST_COD => "📦 Cash on delivery",
        T_CHECKOUT_TRUST_SECURE => "🔒 Telegram-secured identity",
        T_CHECKOUT_AGE_CONFIRM => "I confirm I am 20+",
        T_CHECKOUT_AGE_NOTICE => "By placing this order, you confirm you are 20+ and agree to medical-use terms.",
        // Catalog shared strings
        T_CATALOG_EMPTY => "No items yet",
        T_CATALOG_ERROR => "Failed to load. Pull to refresh or try again.",
        T_CATALOG_SORT_DEFAULT => "Recommended",
        T_CATALOG_SORT_POPULAR => "Popular",
        T_CATALOG_SORT_PRICE => "Price",
        T_CATALOG_SORT_NEW => "New",
        T_CATALOG_SORT_DISCOUNT => "Discount",
        T_LOW_STOCK => "Only {0} left",
        T_WATCH_VIDEO => "Watch video",
        // Sommelier strings
        // Game strings
        T_GAME_TITLE => "TurboBaby Games",
        T_GAME_TAB_SHOP => "🛒 Shop",
        T_GAME_TAB_FARM => "🌱 Farm",
        T_GAME_TAB_DJ => "🎧 DJ",
        T_GAME_TAB_GRILL => "🍖 Grill",
        T_GAME_TABLE_FREE => "Free table",
        T_GAME_TABLE_WAITING => "Guest waiting",
        T_GAME_TABLE_READY => "Order ready",
        T_GAME_TABLE_EATING => "Guest eating",
        T_GAME_TABLE_DIRTY => "Dirty table",
        T_GAME_TABLE_PREPARING => "Preparing...",
        T_GAME_FARM_EMPTY => "Empty plot",
        T_GAME_FARM_PLANTED => "Sprout",
        T_GAME_FARM_WATERED => "Growing fast",
        T_GAME_FARM_GROWN => "Ready to harvest!",
        T_GAME_FARM_WATER => "💧 Water",
        T_GAME_START_PARTY => "Start party to boost earnings",
        T_GAME_UPGRADE_MAX => "MAX",
        T_GAME_UPGRADE_LEVEL_COST => "Lv{0} • {1}🪙",
        T_GAME_SERVED => "Served",
        T_GAME_HARVESTED => "Harvested",
        T_GAME_TIP => "💡 Tip: serve customers to earn coins, then buy upgrades.",
        T_GAME_UPGRADES => "🆙 UPGRADES",
        T_GAME_TABLES => "🪑 Tables",
        T_GAME_SPEED => "⚡ Speed",
        T_GAME_FLOW => "🚪 Flow",
        T_GAME_RESET => "🔄 Reset",
        T_GAME_CONFIRM_RESET => "Reset all progress? This cannot be undone.",
        T_GAME_SHOP_TITLE => "TurboBaby Shop",
        T_GAME_ORDER => "👋 Order",
        T_GAME_SERVE => "🤲 Serve",
        T_GAME_CLEAN => "🧽 Clean",
        T_GAME_GRILL => "🍔 Grill",
        T_GAME_FARM_TITLE => "🌱 FARM",
        T_GAME_PARTY_TITLE => "🎧 DJ ZONE",
        T_GAME_PARTY_STATUS_ON => "Party ON — tips +5 🪙 per serve",
        T_GAME_PARTY_STATUS_OFF => "Start party to boost shop income",
        T_GAME_PARTY_START => "🚀 Start Party",
        T_GAME_PARTY_ON => "🔥 Party ON",
        T_GAME_PARTY_TIP => "Tip: party adds +5 🪙 per serve while active",
        T_GAME_GRILL_TITLE => "🍖 VERANDA GRILL",
        T_GAME_GRILL_STOCK => "Stock: {0}",
        T_GAME_GRILL_COOK => "🍳 Cook",
        T_GAME_GRILL_COOKING => "🔥 Cooking...",
        T_GAME_GRILL_DESC => "Cook food. Serves hungry customers instantly when in shop.",
        T_GAME_GRILL_TIP => "Tip: grilled food auto-serves hungry customers",
        T_GAME_LOG_NEW_CUSTOMER => "New customer arrived",
        T_GAME_LOG_FARM_GREW => "Farm grew a step",
        T_GAME_LOG_RESET => "Progress reset",
        T_GAME_LOG_MOVED_TO_TABLE => "TurboBaby moved to table {0}",
        T_GAME_LOG_TAKING_ORDER => "Taking order at table {0}",
        T_GAME_LOG_SERVING => "Serving at table {0}",
        T_GAME_LOG_CLEANING => "Cleaning table {0}",
        T_GAME_LOG_READY_AT_TABLE => "{0} ready at table {1}",
        T_GAME_LOG_QUICK_GRILL => "Quick grill serve at table {0}",
        T_GAME_LOG_GRILLED_LEFT => "Grilled customer left +{0} 🪙",
        T_GAME_LOG_CUSTOMER_LEFT => "Customer left +{0} 🪙",
        T_GAME_LOG_TABLE_CLEANED => "Table cleaned",
        T_GAME_LOG_PARTY_STARTED => "Party started! −{0} 🪙",
        T_GAME_LOG_COOKING_STARTED => "Cooking started −{0} 🪙",
        T_GAME_LOG_PLANTED_SEED => "Planted seed −{0} 🪙",
        T_GAME_LOG_WATERING => "Watering...",
        T_GAME_LOG_HARVEST => "Harvest! +{0} 🪙",
        T_GAME_EVENT_RUSH_HOUR => "🎉 Rush hour! More customers coming!",
        T_GAME_EVENT_BIG_TIP => "💰 Big tip! +20 coins",
        T_GAME_EVENT_HERB_DELIVERY => "🌿 Herb delivery! All farm plots watered",
        T_GAME_EVENT_DJ_ENERGY => "🎵 DJ energy up! Party lasts longer",
        T_GAME_EVENT_GRILL_DEMAND => "🍔 Grill demand! Free food stock",
        T_GAME_EVENT_DEFAULT => "🎉 Event!",
        T_GAME_UPGRADE_TABLES => "🪑 Tables upgraded!",
        T_GAME_UPGRADE_SPEED => "⚡ Faster service!",
        T_GAME_UPGRADE_FLOW => "🚪 More customers!",
        T_GAME_VIP => "VIP",
        // Location quest screen strings
        T_LOCATION_QUEST_TITLE => "Location Quest",
        T_LOCATION_QUEST_SUBTITLE => "Find nearby spots and earn bonuses",
        T_LOCATION_QUEST_EMPTY => "No active quests in this area",
        T_LOCATION_QUEST_EMPTY_DESC => "Move around the city or check back later",
        T_LOCATION_QUEST_EXPLORE => "Explore",
        T_SCAN_QR_PROMPT => "Scan location QR",
        T_QUEST_INVALID_QR => "Invalid QR",
        T_QUEST_BAD_RESPONSE => "Bad response",
        T_QUEST_ERROR_PREFIX => "Error: {0}",
        T_QUEST_LOADING => "Loading quest...",
        T_QUEST_REWARD_BAT => "🎁 +{0} BAT",
        T_LOCATION_QUEST_DESC => "Complete quests at real locations around the island",
        T_LOCATION_QUEST_LOCATIONS => "🎯 Locations",
        T_LOCATION_QUEST_PLACES => "{0} places",
        T_LOCATION_QUEST_GO => "📷 Go",
        T_LOCATION_QUEST_DEFAULT_DESC => "Explore this location",
        // Success screen
        T_SUCCESS_TITLE => "Order Placed!",
        T_SUCCESS_ORDER_RECEIVED => "Your order #{0} has been received",
        T_SUCCESS_CONTACT_SHORTLY => "We'll contact you shortly",
        T_SUCCESS_DELIVERY_ESTIMATE => "📦 Delivery Estimate",
        T_SUCCESS_STATUS => "Status:",
        T_SUCCESS_CONFIRMED => "Confirmed",
        T_SUCCESS_ETA => "ETA:",
        T_SUCCESS_ETA_VALUE => "{0} min",
        T_SUCCESS_PAYMENT => "Payment:",
        T_SUCCESS_CASH_ON_DELIVERY => "Cash on delivery",
        T_SUCCESS_BACK_MENU => "Back to Menu",
        T_SUCCESS_MY_ORDERS => "My Orders",
        T_SUCCESS_TRACK_ORDER => "🔔 Track Order",
        T_SUCCESS_PUSH_REASSURANCE => "🔔 Push notifications for every order status",
        T_SUCCESS_REWARDS_TITLE => "🎁 Your Rewards",
        T_SUCCESS_REWARDS_BONUS => "⭐ Bonus points: {0}",
        T_SUCCESS_CASHBACK_EARNED => "💸 +{0} cashback earned",
        T_SUCCESS_SHARE_REFERRAL => "👥 Invite a friend",
        T_SUCCESS_REORDER => "🔄 Reorder",
        T_SUCCESS_STATUS_LOADING => "Updating status…",
        T_SUCCESS_STATUS_ERROR => "Could not load live status.",
        T_SUCCESS_CASHBACK_ERROR => "Could not load cashback details.",
        T_SUCCESS_RETRY => "Retry",
        // Orders screen
        T_ORDERS_HISTORY => "Your order history",
        T_ORDERS_NO_ORDERS => "No orders yet",
        T_ORDERS_BROWSE_BIKES => "🏍 Browse bikes",
        T_ORDERS_ORDER => "Order #{0}",
        T_ORDERS_CLOSE => "Close",
        T_ORDERS_STATUS_PENDING => "⏳ Pending",
        T_ORDERS_STATUS_CONFIRMED => "✅ Confirmed",
        T_ORDERS_STATUS_PREPARING => "🔥 Preparing",
        T_ORDERS_STATUS_READY => "📦 Ready",
        T_ORDERS_STATUS_OUT_FOR_DELIVERY => "🚗 Out for Delivery",
        T_ORDERS_STATUS_DELIVERED => "✅ Delivered",
        T_ORDERS_STATUS_CANCELLED => "❌ Cancelled",
        T_ORDERS_STATUS_UNKNOWN => "📋 Unknown",
        T_ORDERS_FILTER_ALL => "All",
        T_ORDERS_FILTER_ACTIVE => "🔄 Active",
        T_ORDERS_FILTER_COMPLETED => "✅ Completed",
        T_ORDERS_FILTER_CANCELLED => "❌ Cancelled",
        T_ORDERS_STEP_RECEIVED => "Received",
        T_ORDERS_STEP_CONFIRMED => "Confirmed",
        T_ORDERS_STEP_PREPARING => "Preparing",
        T_ORDERS_STEP_READY => "Ready",
        T_ORDERS_STEP_ON_THE_WAY => "On the way",
        T_ORDERS_STEP_DELIVERED => "Delivered",
        T_ORDER_DETAIL_NOT_FOUND => "Order not found or unavailable",
        T_ORDER_DETAIL_BACK => "← Back to orders",
        T_ORDER_DETAIL_TOTAL => "Total",
        T_ORDER_DETAIL_BONUS => "Bonus",
        T_ORDER_DETAIL_STARS => "Stars",
        T_ORDER_DETAIL_LIVE => "Live updates",
        T_ORDER_DETAIL_CANCEL => "Cancel order",
        T_ORDER_DETAIL_CANCEL_CONFIRM => "Cancel order? Bonus and stars will be refunded.",
        T_ORDER_DETAIL_CANCELLED_BY_USER => "You cancelled the order",
        T_ORDER_DETAIL_CANCEL_REFUSED => "This order is already being handled and can't be cancelled in the app. Please message the manager.",
        T_ORDER_REORDER => "Reorder order",
        // Profile screen
        T_PROFILE_MEMBERSHIP => "Your membership status",
        T_PROFILE_QR_CODE => "Your QR Code",
        T_PROFILE_COPY_LINK => "Copy Link",
        T_PROFILE_SHARE => "Share",
        T_PROFILE_FRIENDS_INVITED => "👥 {0} friends invited",
        T_PROFILE_REFERRAL_LINK => "🔗 Referral Link",
        T_PROFILE_COPY => "Copy",
        T_PROFILE_INVITED => "👥 {0} invited",
        T_PROFILE_EARN_PER_REF => "Earn {0} per referral",
        T_PROFILE_QUICK_ACTIONS => "Quick Actions",
        T_PROFILE_MY_ORDERS => "My Orders",
        T_PROFILE_QUESTS => "Quests",
        T_PROFILE_REFERRAL_PROGRAM => "Referral Program",
        T_PROFILE_TIER_BENEFITS => "💎 Tier Benefits",
        T_PROFILE_TIER_STARTER => "Starter",
        T_PROFILE_TIER_BRONZE => "Bronze Wheel",
        T_PROFILE_TIER_SILVER => "Silver Wheel",
        T_PROFILE_TIER_GOLD => "Gold Wheel",
        T_PROFILE_SPENT => "SPENT",
        T_PROFILE_BONUS => "BONUS",
        T_PROFILE_STARS => "STARS",
        T_PROFILE_CASHBACK_LABEL => "CASHBACK",
        T_PROFILE_PROGRESS => "Progress to {0}",
        T_PROFILE_MORE_TO_UNLOCK => "{0} more to unlock {1}",
        T_PROFILE_CONTACTS => "📍 Contacts",
        T_PROFILE_OPEN_MAP => "Open map",
        T_PROFILE_BONUS_HISTORY => "📜 Bonus History",
        T_PROFILE_BONUS_HISTORY_EMPTY => "No transactions yet",
        T_PROFILE_BONUS_CREDIT => "Credit",
        T_PROFILE_BONUS_DEBIT => "Debit",
        T_PROFILE_LOAD_ERROR => "Could not load profile.",
        T_PROFILE_RETRY => "Retry",
        T_PROFILE_BONUS_REFERRAL => "Referral bonus",
        T_PROFILE_BONUS_GARDEN => "Garden reward",
        T_PROFILE_BONUS_CASHBACK => "Order cashback",
        T_PROFILE_BONUS_ADMIN => "Admin grant",
        T_PROFILE_BONUS_OTHER => "Bonus",
        T_PROFILE_ORDER_HISTORY => "Order history",
        T_PROFILE_REORDER => "Reorder",
        T_CART_SYNCING => "Syncing cart…",
        T_REORDER_DEEP_LINK_TITLE => "🔄 Your last order is ready to reorder",
        // Modal
        T_MODAL_CLOSE => "Close",
        T_OPEN_IN_TELEGRAM_TITLE => "Open in Telegram",
        T_OPEN_IN_TELEGRAM_BODY => "This app runs inside Telegram. Open the TurboBaby bot and press the menu button at the bottom of the screen.",
        T_MODAL_CONFIRM => "Yes",
        T_MODAL_CANCEL => "No",
        T_MODAL_DECREASE_QTY => "Decrease quantity",
        T_MODAL_INCREASE_QTY => "Increase quantity",
        T_MODAL_CERTIFICATE => "📄 Certificate",
        // Menu screen
        T_MENU_FILTER_SATIVA => "☀️ Sativa",
        T_MENU_FILTER_INDICA => "🌙 Indica",
        T_MENU_FILTER_HYBRID => "⚖️ Hybrid",
        T_MENU_SORT_TOP => "✨ Top",
        T_MENU_SORT_PRICE_ASC => "💰 ↑",
        T_MENU_SORT_PRICE_DESC => "💰 ↓",
        T_MENU_SORT_NAME => "A–Z",
        T_MENU_NO_RESULTS => "No {0} strains found",
        T_MENU_NEW_ARRIVALS => "🆕 NEW ARRIVALS",
        T_MENU_PRICE_REQUEST => "Price on request",
        T_MENU_SOLD_OUT => "Sold Out",
        T_MENU_SOTD_BADGE => "⭐ SOTD",
        T_MENU_NEW_BADGE => "🆕 NEW",
        T_MENU_BEST_BADGE => "⭐ BEST",
        T_MENU_SALE_BADGE => "🔥 SALE",
        T_MENU_SET_LABEL => "📦 SET",
        T_SET_BADGE => "📦 SET",
        T_MENU_OFF => "{0}% OFF",
        T_MENU_WEIGHT => "⚖️ {0}",
        T_MENU_FLAVOR_PREFIX => "🍃 {0}",
        T_MENU_PER_GRAM => "/g",
        // Home screen
        T_HOME_CATEGORIES => "Categories",
        T_HOME_SETS_PACKS => "📦 Packs",
        T_HOME_ADVENTURES => "🎯 Adventures",
        T_HOME_DAILY_QUEST => "Daily Quest",
        T_HOME_TREASURE_HUNT => "Treasure Hunt",
        T_HOME_AR_HUNT => "AR Hunt",
        T_HOME_LOCATION_QUEST => "Location Quest",
        T_HOME_GAME => "Game",
        T_HOME_SHARE => "Share",
        T_HOME_WATCH_VIDEO => "Watch video",
        T_HOME_REORDER_LAST => "Last order",
        T_HOME_REORDER_STATUS => "Status",
        T_HOME_REORDER_CTA => "Reorder",
        // Events screen
        T_EVENTS_TIME => "🕒 {0}",
        T_EVENTS_SOLD_OUT_BADGE => "SOLD OUT",
        T_EVENTS_SEATS => "{0} seats",
        T_EVENTS_FREE_BADGE => "FREE",
        T_EVENTS_TELEGRAM_REQUIRED => "Telegram required",
        T_EVENTS_OK => "OK",
        T_EVENTS_RETRY => "Retry",
        T_EVENTS_SEAT => "seat",
        T_EVENTS_SELECT_SEATS => "Number of seats",
        T_EVENTS_OPEN_DETAILS => "Open event details",
        T_EVENTS_SHARE_EVENT => "Share event",
        T_EVENTS_PREV_PHOTO => "Previous photo",
        T_EVENTS_NEXT_PHOTO => "Next photo",
        T_EVENTS_PHOTO_N => "Photo {0}",
        T_EVENTS_EVENT_NOT_FOUND => "Event not found",
        T_EVENTS_MONTH_JAN => "Jan",
        T_EVENTS_MONTH_FEB => "Feb",
        T_EVENTS_MONTH_MAR => "Mar",
        T_EVENTS_MONTH_APR => "Apr",
        T_EVENTS_MONTH_MAY => "May",
        T_EVENTS_MONTH_JUN => "Jun",
        T_EVENTS_MONTH_JUL => "Jul",
        T_EVENTS_MONTH_AUG => "Aug",
        T_EVENTS_MONTH_SEP => "Sep",
        T_EVENTS_MONTH_OCT => "Oct",
        T_EVENTS_MONTH_NOV => "Nov",
        T_EVENTS_MONTH_DEC => "Dec",
        T_EVENTS_MY_BOOKINGS => "My bookings",
        T_EVENTS_NO_BOOKINGS => "No bookings yet",
        T_EVENTS_CANCEL => "Cancel",

        // Bikes
        T_BIKE_CATALOG_TITLE => "Bikes",
        T_BIKE_CATALOG_DESC => "Rent on Phuket",
        T_BIKE_FILTER_SCOOTER => "Scooters",
        T_BIKE_FILTER_MOTORCYCLE => "Motorcycles",
        T_BIKE_FILTER_FREE_NOW => "Free now",
        T_BIKE_SORT_DEFAULT => "Default",
        T_BIKE_SORT_PRICE_ASC => "Cheapest first",
        T_BIKE_SORT_PRICE_DESC => "Priciest first",
        T_BIKE_NO_RESULTS => "Nothing matches the “{0}” filter",
        T_BIKE_CLASS_SCOOTER => "Scooter",
        T_BIKE_CLASS_MOTORCYCLE => "Motorcycle",
        T_BIKE_CC => "{0} cc",
        T_BIKE_DETAILS => "Details",
        T_BIKE_BOOK => "Book",
        T_BIKE_PRICE_TITLE => "Price",
        T_BIKE_RATE_PER_DAY => "Rental",
        T_BIKE_PER_DAY => "per day",
        T_BIKE_PRICE_ON_REQUEST => "A manager quotes the price for this model — message us",
        T_BIKE_TARIFF_BEFORE_DISCOUNT => "Tariff before discount: {0}",
        T_BIKE_CLASS_DISCOUNT => "Class discount: −{0}%",
        T_BIKE_DEPOSIT => "Deposit:",
        T_BIKE_MONTHLY_LOW_SEASON => "Monthly, low season:",
        T_BIKE_QUOTE_NOTE => "A manager confirms the final amount: it depends on term, season and availability.",
        T_BIKE_TERMS_TITLE => "Terms and discounts",
        T_BIKE_TERMS_NOTE => "These are the published term discounts. A manager confirms the exact amount.",
        T_BIKE_TERM_WEEK => "Week",
        T_BIKE_TERM_TWO_WEEKS => "Two weeks",
        T_BIKE_TERM_MONTH => "Month",
        T_BIKE_TERM_DAYS => "{0}–{1} days",
        T_BIKE_TERM_DAYS_OPEN => "{0}+ days",
        T_BIKE_TERM_DISCOUNT_ONE => "−{0}%",
        T_BIKE_TERM_DISCOUNT_RANGE => "−{0}…−{1}%",
        T_BIKE_UNITS_TITLE => "Availability",
        T_BIKE_UNITS_EMPTY => "Every bike of this model is out right now.",
        T_BIKE_AVAILABILITY => "{0} of {1} free",
        T_BIKE_AVAILABILITY_FREE => "{0} free",
        T_BIKE_AVAILABILITY_UNKNOWN => "A manager confirms availability",
        T_BIKE_COLORS_AVAILABLE => "Colours free:",
        T_BIKE_COLORS_ALL => "Model colours:",
        T_BIKE_MODEL_YEARS => "Model years:",
        T_BIKE_SALE_TITLE => "Buy-out",
        T_BIKE_SALE_PRICE => "Sale price",
        T_BIKE_ASK_MANAGER => "Message the manager",
        T_BIKE_NOT_OFFERED_TITLE => "This model is not offered right now",
        T_BIKE_NOT_OFFERED_ALTERNATIVES => "Offered instead: {0}",
        T_BIKE_BOOK_BLOCKED_NOT_OFFERED => "Closed to new rentals",
        T_BIKE_BOOK_BLOCKED_NO_RATE => "No published price — a manager takes this booking",
        T_BIKE_BOOK_BLOCKED_NO_UNITS => "Every bike of this model is taken",
        T_BIKE_BOOK_BLOCKED_UNKNOWN_AVAILABILITY => "Availability not confirmed",
        T_BIKE_BOOK_BLOCKED_NOT_WIRED => "In-app booking is not live yet — message the manager",

        _ => key,
    }
}

/// Get translation with formatting
pub fn tf(lang: Lang, key: Key, args: &[String]) -> String {
    let mut result = t(lang, key).to_string();
    for (i, arg) in args.iter().enumerate() {
        result = result.replace(&format!("{{{}}}", i), arg);
    }
    result
}

/// Translation map for JSON serialization
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Translations {
    pub lang: String,
    pub translations: HashMap<String, String>,
}

/// Get all translations for a language
pub fn get_translations(lang: Lang) -> Translations {
    let mut map = HashMap::new();
    let keys = [
        // Navigation
        T_NAV_HOME,
        T_NAV_SETS,
        T_NAV_MENU,
        T_NAV_ACCESSORIES,
        T_NAV_TEA,
        T_NAV_QUEST,
        T_NAV_EVENTS,
        T_NAV_CART,
        T_NAV_PROFILE,
        T_NAV_FLEET,
        T_NAV_RIDE,
        T_NAV_ORDERS,
        T_CLOSE,
        // Telegram buttons
        T_BTN_CART,
        T_BTN_CHECKOUT,
        T_BTN_PAY,
        T_BTN_CHECKIN,
        T_BTN_ORDER,
        T_BTN_ASK,
        // Coming soon
        T_COMING_SOON,
        T_COMING_SOON_DESC,
        T_COMING_SOON_WORKING,
        T_BACK_HOME,
        // Categories
        T_CAT_MENU,
        T_CAT_ACCESSORIES,
        T_CAT_TEA,
        // Quest
        T_TITLE,
        T_SUBTITLE,
        T_START_BTN,
        T_EXPLORE_BTN,
        T_CHECKPOINT_TITLE,
        T_CURRENT_CHECKPOINT,
        T_PURCHASE_REQUIRED,
        T_PURCHASE_MIN,
        T_SCAN_QR,
        T_SCAN_QR_DESC,
        T_SUBMIT_CHECKIN,
        T_STATUS_LOCKED,
        T_STATUS_ACTIVE,
        T_STATUS_COMPLETED,
        T_CHECKIN_SUCCESS,
        T_NEXT_LOCATION,
        T_QUEST_COMPLETE,
        T_REWARD_CLAIM,
        T_REWARD,
        T_ERROR_WRONG_ORDER,
        T_ERROR_PURCHASE,
        T_ERROR_INVALID_QR,
        T_ERROR_ALREADY_CHECKED,
        T_ERROR_CHECKIN_FAILED,
        // Events calendar
        T_EVENTS_TITLE,
        T_EVENTS_SUBTITLE,
        T_EVENTS_NO_EVENTS,
        T_EVENTS_DATE,
        T_EVENTS_LOCATION,
        T_EVENTS_CAPACITY,
        T_EVENTS_PRICE,
        T_EVENTS_PRICE_STARS,
        T_EVENTS_BOOK,
        T_EVENTS_BOOKED,
        T_EVENTS_BOOK_FREE,
        T_EVENTS_SOLD_OUT,
        T_EVENTS_ERROR,
        T_EVENTS_ALREADY_BOOKED,
        T_EVENTS_INSUFFICIENT_STARS,
        T_EVENTS_REMINDER_BODY,
        T_EVENTS_WEEKDAY_MON,
        T_EVENTS_WEEKDAY_TUE,
        T_EVENTS_WEEKDAY_WED,
        T_EVENTS_WEEKDAY_THU,
        T_EVENTS_WEEKDAY_FRI,
        T_EVENTS_WEEKDAY_SAT,
        T_EVENTS_WEEKDAY_SUN,
        // Screen titles
        T_HOME_TITLE,
        T_HOME_SUBTITLE,
        T_MENU_TITLE,
        T_MENU_DESC,
        T_SETS_TITLE,
        T_SETS_DESC,
        T_ACC_TITLE,
        T_ACC_DESC,
        T_TEA_TITLE,
        T_TEA_DESC,
        T_CART_TITLE,
        T_CART_EMPTY,
        T_CART_EMPTY_DESC,
        T_CHECKOUT_TITLE,
        T_ORDERS_TITLE,
        T_PROFILE_TITLE,
        T_YOUR_ORDER,
        T_YOUR_INFO,
        T_PICKUP_LOCATION,
        T_DELIVERY,
        T_DELIVERY_ZONE,
        T_DELIVERY_ETA,
        T_DELIVERY_FEE,
        T_PAYMENT,
        T_PLACE_ORDER,
        T_BACK,
        T_TOTAL,
        T_ADD_TO_CART,
        T_LOADING,
        T_FILTER_ALL,
        T_CHECKOUT_TRUST_TITLE,
        T_CHECKOUT_TRUST_VERIFIED,
        T_CHECKOUT_TRUST_COD,
        T_CHECKOUT_TRUST_SECURE,
        T_CHECKOUT_AGE_CONFIRM,
        T_CHECKOUT_AGE_NOTICE,
        T_CATALOG_EMPTY,
        T_CATALOG_ERROR,
        T_CATALOG_SORT_DEFAULT,
        T_CATALOG_SORT_POPULAR,
        T_CATALOG_SORT_PRICE,
        T_CATALOG_SORT_NEW,
        T_CATALOG_SORT_DISCOUNT,
        T_LOW_STOCK,
        T_WATCH_VIDEO,
        T_GAME_TITLE,
        T_GAME_TAB_SHOP,
        T_GAME_TAB_FARM,
        T_GAME_TAB_DJ,
        T_GAME_TAB_GRILL,
        T_GAME_TABLE_FREE,
        T_GAME_TABLE_WAITING,
        T_GAME_TABLE_READY,
        T_GAME_TABLE_EATING,
        T_GAME_TABLE_DIRTY,
        T_GAME_TABLE_PREPARING,
        T_GAME_FARM_EMPTY,
        T_GAME_FARM_PLANTED,
        T_GAME_FARM_WATERED,
        T_GAME_FARM_GROWN,
        T_GAME_FARM_WATER,
        T_GAME_START_PARTY,
        T_GAME_UPGRADE_MAX,
        T_GAME_UPGRADE_LEVEL_COST,
        T_GAME_SERVED,
        T_GAME_HARVESTED,
        T_GAME_TIP,
        T_GAME_UPGRADES,
        T_GAME_TABLES,
        T_GAME_SPEED,
        T_GAME_FLOW,
        T_GAME_RESET,
        T_GAME_CONFIRM_RESET,
        T_GAME_SHOP_TITLE,
        T_GAME_ORDER,
        T_GAME_SERVE,
        T_GAME_CLEAN,
        T_GAME_GRILL,
        T_GAME_FARM_TITLE,
        T_GAME_PARTY_TITLE,
        T_GAME_PARTY_STATUS_ON,
        T_GAME_PARTY_STATUS_OFF,
        T_GAME_PARTY_START,
        T_GAME_PARTY_ON,
        T_GAME_PARTY_TIP,
        T_GAME_GRILL_TITLE,
        T_GAME_GRILL_STOCK,
        T_GAME_GRILL_COOK,
        T_GAME_GRILL_COOKING,
        T_GAME_GRILL_DESC,
        T_GAME_GRILL_TIP,
        T_GAME_LOG_NEW_CUSTOMER,
        T_GAME_LOG_FARM_GREW,
        T_GAME_LOG_RESET,
        T_GAME_LOG_MOVED_TO_TABLE,
        T_GAME_LOG_TAKING_ORDER,
        T_GAME_LOG_SERVING,
        T_GAME_LOG_CLEANING,
        T_GAME_LOG_READY_AT_TABLE,
        T_GAME_LOG_QUICK_GRILL,
        T_GAME_LOG_GRILLED_LEFT,
        T_GAME_LOG_CUSTOMER_LEFT,
        T_GAME_LOG_TABLE_CLEANED,
        T_GAME_LOG_PARTY_STARTED,
        T_GAME_LOG_COOKING_STARTED,
        T_GAME_LOG_PLANTED_SEED,
        T_GAME_LOG_WATERING,
        T_GAME_LOG_HARVEST,
        T_GAME_EVENT_RUSH_HOUR,
        T_GAME_EVENT_BIG_TIP,
        T_GAME_EVENT_HERB_DELIVERY,
        T_GAME_EVENT_DJ_ENERGY,
        T_GAME_EVENT_GRILL_DEMAND,
        T_GAME_EVENT_DEFAULT,
        T_GAME_UPGRADE_TABLES,
        T_GAME_UPGRADE_SPEED,
        T_GAME_UPGRADE_FLOW,
        T_GAME_VIP,
        T_LOCATION_QUEST_TITLE,
        T_LOCATION_QUEST_SUBTITLE,
        T_LOCATION_QUEST_EMPTY,
        T_LOCATION_QUEST_EMPTY_DESC,
        T_LOCATION_QUEST_EXPLORE,
        T_SCAN_QR_PROMPT,
        T_QUEST_INVALID_QR,
        T_QUEST_BAD_RESPONSE,
        T_QUEST_ERROR_PREFIX,
        T_QUEST_LOADING,
        T_QUEST_REWARD_BAT,
        T_LOCATION_QUEST_DESC,
        T_LOCATION_QUEST_LOCATIONS,
        T_LOCATION_QUEST_PLACES,
        T_LOCATION_QUEST_GO,
        T_LOCATION_QUEST_DEFAULT_DESC,
    ];

    for key in keys {
        map.insert(key.to_string(), t(lang, key).to_string());
    }

    Translations {
        lang: lang.as_str().to_string(),
        translations: map,
    }
}

/// Get supported languages
pub fn supported_languages() -> Vec<Lang> {
    vec![
        Lang::Russian,
        Lang::English,
        Lang::Thai,
        Lang::Chinese,
        Lang::Hebrew,
        Lang::German,
        Lang::French,
        Lang::Spanish,
    ]
}

/// Validate translation key exists
pub fn has_translation(lang: Lang, key: Key) -> bool {
    match lang {
        Lang::Russian => get_ru_translation(key) != key,
        Lang::English => get_en_translation(key) != key,
        _ => get_en_translation(key) != key, // Fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translation_basic() {
        assert_eq!(t(Lang::Russian, T_TITLE), "TurboBaby Island Quest");
        assert_eq!(t(Lang::English, T_TITLE), "TurboBaby Island Quest");
    }

    #[test]
    fn test_translation_different() {
        assert_eq!(t(Lang::Russian, T_SUBTITLE), "Пройди 5 точек на Пхукете");
        assert_eq!(
            t(Lang::English, T_SUBTITLE),
            "Complete 5 locations on Phuket"
        );
    }

    #[test]
    fn test_translation_fallback() {
        // Unknown key returns the key itself
        assert_eq!(t(Lang::English, "unknown_key"), "unknown_key");
    }

    #[test]
    fn test_translation_format() {
        let result = tf(Lang::English, T_CHECKPOINT_TITLE, &[s("5")]);
        assert_eq!(result, "Checkpoint #5");
    }

    fn s(s: &str) -> String {
        s.to_string()
    }

    #[test]
    fn test_get_translations() {
        let ru = get_translations(Lang::Russian);
        assert_eq!(ru.lang, "ru");
        assert!(!ru.translations.is_empty());
        assert_eq!(
            ru.translations.get(T_SUBTITLE),
            Some(&"Пройди 5 точек на Пхукете".to_string())
        );
    }

    /// This file, read back at compile time so the gate below can enumerate
    /// the `T_*` declarations.
    ///
    /// `include_str!` rather than `std::fs`, because this module is also
    /// compiled for `wasm32-unknown-unknown`, where there is no filesystem.
    const THIS_FILE: &str = include_str!("i18n.rs");

    /// Every `pub const T_NAME: Key = "key.literal";` in this file, as
    /// `(constant_name, key_literal)`.
    ///
    /// The declarations are the one place that records which keys exist, so
    /// the gate reads them instead of a hand-written list — a third copy of
    /// 706 names would be the very defect it is trying to catch.
    fn declared_keys() -> Vec<(&'static str, &'static str)> {
        let mut out = Vec::new();
        for line in THIS_FILE.lines() {
            let Some(rest) = line.trim_start().strip_prefix("pub const T_") else {
                continue;
            };
            // `NAME: Key = "key.literal";`
            let Some((name_part, value_part)) = rest.split_once(": Key = ") else {
                continue;
            };
            let value = value_part.trim();
            let Some(literal) = value
                .strip_prefix('"')
                .and_then(|v| v.split_once('"'))
                .map(|(lit, _)| lit)
            else {
                continue;
            };
            out.push((name_part.trim(), literal));
        }
        out
    }

    #[test]
    fn the_declaration_parser_finds_the_whole_table() {
        let keys = declared_keys();
        // A floor, not the exact count: this assertion exists to fail loudly if
        // the parser silently stops matching (a rustfmt change to the `const`
        // layout, say) rather than to be updated every time a key is added. A
        // broken parser would make the parity gate below vacuously green.
        // Lowered from 600 when 46 cannabis-era keys were deleted outright
        // (the sommelier questionnaire, strain badges, lab certificates,
        // THC/CBD labels — #2). The number is only ever a "did the parser
        // break" tripwire: a broken parser yields ~0, so the floor needs to
        // sit far below the real count, not just under it. It is not a target
        // and it is not a census.
        assert!(
            keys.len() > 450,
            "only {} `T_*` declarations parsed out of this file — the parser is \
             broken, not the table. Every key after the break would be exempt \
             from the parity gate.",
            keys.len()
        );
        assert!(
            keys.iter().any(|(_, k)| *k == T_TITLE),
            "a known key is missing from the parse"
        );
    }

    /// Every declared key must resolve in **both** hand-maintained tables.
    ///
    /// `get_ru_translation` and `get_en_translation` are two `match` blocks of
    /// several hundred arms each, both ending `_ => key`. That fallback is why
    /// this gate has to exist: a key missing from a table does not fail a
    /// build, does not log, and does not panic — `t()` returns the key itself,
    /// so the customer reads the literal string `bike.price.title` where the
    /// price was meant to be. The two tables are a restated list, and nothing
    /// until now compared them.
    ///
    /// Written after 50 `T_BIKE_*` keys were added to all three places by hand
    /// (the declarations, the ru arms, the en arms). Three hand-edited lists
    /// that must agree is exactly the shape that needs a test rather than
    /// care.
    ///
    /// One known limitation, stated so it is not mistaken for coverage:
    /// `has_translation` is `get_*_translation(key) != key`, so a translation
    /// whose text happens to equal its own key would read as missing. No
    /// current key has that shape, and the failure direction is safe (a false
    /// alarm, never a silent pass).
    #[test]
    fn every_declared_key_has_a_russian_and_an_english_translation() {
        let mut missing_ru = Vec::new();
        let mut missing_en = Vec::new();
        for (name, key) in declared_keys() {
            if !has_translation(Lang::Russian, key) {
                missing_ru.push(format!("T_{name} = {key:?}"));
            }
            if !has_translation(Lang::English, key) {
                missing_en.push(format!("T_{name} = {key:?}"));
            }
        }
        assert!(
            missing_ru.is_empty() && missing_en.is_empty(),
            "Declared keys with no translation.\n\
             Missing from get_ru_translation ({}): {:#?}\n\
             Missing from get_en_translation ({}): {:#?}\n\
             Each of these renders as its own key on screen, because both \
             tables end `_ => key`.",
            missing_ru.len(),
            missing_ru,
            missing_en.len(),
            missing_en
        );
    }

    #[test]
    fn test_has_translation() {
        assert!(has_translation(Lang::English, T_TITLE));
        assert!(!has_translation(Lang::English, "nonexistent_key"));
    }

    /// Architectural fitness function (Wave loop): every `pub const T_*: Key`
    /// declared in this file MUST have a translation arm in BOTH
    /// `get_ru_translation` and `get_en_translation`. A key with no arm falls
    /// through to `_ => key` and silently renders its raw dotted key
    /// (e.g. "nav.home") in the UI — a class of bug that no compiler warns
    /// about because the const is still "used" via the match's catch-all.
    ///
    /// We parse the source rather than maintain a second list (which would
    /// itself drift). The string *value* of each const is the lookup key, so
    /// `has_translation(lang, value)` exercises the real `t()` path.
    #[test]
    fn every_translation_key_is_translated_in_ru_and_en() {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let path = std::path::Path::new(manifest).join("src/trios/i18n.rs");
        let src = std::fs::read_to_string(path).expect("read src/trios/i18n.rs");

        // Extract the string literal from each `pub const T_*: Key = "value";`.
        let mut keys: Vec<String> = Vec::new();
        for line in src.lines() {
            let line = line.trim();
            if !line.starts_with("pub const T_") || !line.contains(": Key") {
                continue;
            }
            if let (Some(open), Some(_)) = (line.find('"'), line.rfind('"')) {
                if let Some(close) = line[open + 1..].find('"') {
                    keys.push(line[open + 1..open + 1 + close].to_string());
                }
            }
        }
        assert!(
            keys.len() >= 90,
            "parsed only {} translation keys — parser likely broken",
            keys.len()
        );

        let mut missing = Vec::new();
        for k in &keys {
            // `Key` is `&'static str`; leak the parsed value so it satisfies
            // the signature. One-shot, test-process only.
            let leaked: &'static str = Box::leak(k.clone().into_boxed_str());
            if !has_translation(Lang::Russian, leaked) {
                missing.push(format!("{k} (ru)"));
            }
            if !has_translation(Lang::English, leaked) {
                missing.push(format!("{k} (en)"));
            }
        }
        assert!(
            missing.is_empty(),
            "{} translation key(s) have no arm and fall through to `_ => key` \
             (add them to get_ru_translation / get_en_translation):\n  {}",
            missing.len(),
            missing.join("\n  ")
        );
    }
}
