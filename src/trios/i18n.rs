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
pub const T_NAV_GARDEN: Key = "nav.garden";
pub const T_NAV_ACCESSORIES: Key = "nav.accessories";
pub const T_NAV_TEA: Key = "nav.tea";
pub const T_NAV_QUEST: Key = "nav.quest";
pub const T_NAV_GAME: Key = "nav.game";
pub const T_NAV_EVENTS: Key = "nav.events";
pub const T_NAV_CART: Key = "nav.cart";
pub const T_NAV_PROFILE: Key = "nav.profile";
pub const T_NAV_MORE: Key = "nav.more";

/// Garden translations
pub const T_GARDEN_TITLE: Key = "garden.title";
pub const T_GARDEN_SUBTITLE: Key = "garden.subtitle";
pub const T_GARDEN_EMPTY_LABEL: Key = "garden.empty_label";
pub const T_GARDEN_EMPTY_CTA: Key = "garden.empty_cta";
pub const T_GARDEN_LOADING: Key = "garden.loading";
pub const T_GARDEN_CHANGE_PRODUCT: Key = "garden.change_product";
pub const T_GARDEN_RESET_PROGRESS: Key = "garden.reset_progress";
pub const T_GARDEN_RESET_CONFIRM_TITLE: Key = "garden.reset_confirm_title";
pub const T_GARDEN_RESET_CONFIRM_BODY: Key = "garden.reset_confirm_body";
pub const T_GARDEN_CANCEL: Key = "garden.cancel";
pub const T_GARDEN_CONFIRM_RESET: Key = "garden.confirm_reset";
pub const T_GARDEN_TAB_GARDEN: Key = "garden.tab.garden";
pub const T_GARDEN_TAB_GAME: Key = "garden.tab.game";
pub const T_GARDEN_HARVEST: Key = "garden.harvest";
pub const T_GARDEN_PLANT: Key = "garden.plant";
pub const T_GARDEN_ONBOARD_TITLE: Key = "garden.onboard.title";
pub const T_GARDEN_ONBOARD_STEP1: Key = "garden.onboard.step1";
pub const T_GARDEN_ONBOARD_STEP2: Key = "garden.onboard.step2";
pub const T_GARDEN_ONBOARD_STEP3: Key = "garden.onboard.step3";
pub const T_GARDEN_ONBOARD_CTA: Key = "garden.onboard.cta";
pub const T_GARDEN_DIAGNOSTICS_COPY: Key = "garden.diagnostics.copy";
pub const T_GARDEN_DIAGNOSTICS_COPIED: Key = "garden.diagnostics.copied";
pub const T_GARDEN_CHOOSE_PRODUCT: Key = "garden.choose_product";
pub const T_GARDEN_CHOOSE_PRODUCT_HINT: Key = "garden.choose_product_hint";
pub const T_GARDEN_PLANT_ALT: Key = "garden.plant_alt";
pub const T_GARDEN_PRODUCT_ALT: Key = "garden.product_alt";
pub const T_GARDEN_DISCOUNT_BADGE: Key = "garden.discount_badge";
pub const T_GARDEN_READY: Key = "garden.ready";
pub const T_GARDEN_COOLDOWN: Key = "garden.cooldown";
pub const T_GARDEN_CHOOSER_TITLE: Key = "garden.chooser.title";
pub const T_GARDEN_CHOOSER_EMPTY: Key = "garden.chooser.empty";
pub const T_GARDEN_CHOOSER_LOADING: Key = "garden.chooser.loading";
pub const T_GARDEN_CHOOSER_ERROR: Key = "garden.chooser.error";
pub const T_GARDEN_ERROR_HARVEST: Key = "garden.error.harvest";
pub const T_GARDEN_ERROR_RESET: Key = "garden.error.reset";
pub const T_GARDEN_ERROR_COOLDOWN: Key = "garden.error.cooldown";
pub const T_GARDEN_ERROR_PRODUCT_UNAVAILABLE: Key = "garden.error.product_unavailable";
pub const T_GARDEN_CAT_STRAIN: Key = "garden.cat.strain";
pub const T_GARDEN_CAT_ACCESSORY: Key = "garden.cat.accessory";
pub const T_GARDEN_CAT_TEA: Key = "garden.cat.tea";
pub const T_GARDEN_CAT_SET: Key = "garden.cat.set";
pub const T_GARDEN_CAT_ACCESSORY_SET: Key = "garden.cat.accessory_set";
pub const T_GARDEN_CAT_TEA_SET: Key = "garden.cat.tea_set";
pub const T_GARDEN_CAT_OTHER: Key = "garden.cat.other";

/// Generic button translations
pub const T_CLOSE: Key = "btn.close";

/// Telegram button translations
pub const T_BTN_CART: Key = "btn.cart";
pub const T_BTN_CHECKOUT: Key = "btn.checkout";
pub const T_BTN_PAY: Key = "btn.pay";
pub const T_BTN_WATER: Key = "btn.water";
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
pub const T_SOMM_TITLE: Key = "somm.title";
pub const T_SOMM_DESC: Key = "somm.description";
pub const T_SOMM_REASON_INDICA_RELAX: Key = "somm.reason.indica_relax";
pub const T_SOMM_REASON_HYBRID_MELLOW: Key = "somm.reason.hybrid_mellow";
pub const T_SOMM_REASON_SATIVA_ENERGY: Key = "somm.reason.sativa_energy";
pub const T_SOMM_REASON_HYBRID_UP: Key = "somm.reason.hybrid_up";
pub const T_SOMM_REASON_CREATIVE: Key = "somm.reason.creative";
pub const T_SOMM_REASON_SLEEP: Key = "somm.reason.sleep";
pub const T_SOMM_REASON_FLAVOR: Key = "somm.reason.flavor";
pub const T_SOMM_REASON_THC: Key = "somm.reason.thc";
pub const T_SOMM_REASON_DEFAULT: Key = "somm.reason.default";
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
pub const T_REVIEWS_TITLE: Key = "reviews.title";
pub const T_REVIEWS_AVG: Key = "reviews.avg";
pub const T_REVIEWS_EMPTY: Key = "reviews.empty";
pub const T_REVIEW_LEAVE: Key = "review.leave";
pub const T_REVIEW_RATING: Key = "review.rating";
pub const T_REVIEW_COMMENT: Key = "review.comment";
pub const T_REVIEW_SUBMIT: Key = "review.submit";
pub const T_REVIEW_THANKS: Key = "review.thanks";
pub const T_LAB_CERTS: Key = "lab_certs.title";
pub const T_LAB_CERT_THC: Key = "lab_certs.thc";
pub const T_LAB_CERT_CBD: Key = "lab_certs.cbd";
pub const T_LAB_CERT_TESTED: Key = "lab_certs.tested";
pub const T_LAB_CERT_EMPTY: Key = "lab_certs.empty";
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
pub const T_SOMM_MOOD: Key = "somm.mood";
pub const T_SOMM_TIME: Key = "somm.time";
pub const T_SOMM_EXP: Key = "somm.experience";
pub const T_SOMM_RESULT: Key = "somm.result";

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

// Checkout screen hard-coded strings
pub const T_CHECKOUT_CART_EMPTY: Key = "checkout.cart_empty";
pub const T_CHECKOUT_STEP_CART: Key = "checkout.step.cart";
pub const T_CHECKOUT_STEP_DETAILS: Key = "checkout.step.details";
pub const T_CHECKOUT_STEP_CONFIRM: Key = "checkout.step.confirm";
pub const T_CHECKOUT_SELECT_ZONE: Key = "checkout.select_zone";
pub const T_CHECKOUT_GARDEN_DISCOUNT: Key = "checkout.garden_discount";
pub const T_CHECKOUT_GARDEN_DISCOUNT_PCT: Key = "checkout.garden_discount_pct";
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
pub const T_SOMM_GET_RECOMMENDATIONS: Key = "somm.get_recommendations";
pub const T_SOMM_RECOMMENDED_FOR_YOU: Key = "somm.recommended_for_you";
pub const T_SOMM_RESTART: Key = "somm.restart";
pub const T_SOMM_RECOMMENDED_SETS: Key = "somm.recommended_sets";
pub const T_SOMM_RECOMMENDED_STRAINS: Key = "somm.recommended_strains";
pub const T_SOMM_NO_RECOMMENDATIONS: Key = "somm.no_recommendations";
pub const T_SOMM_MATCH: Key = "somm.match";
pub const T_SOMM_MOOD_RELAX: Key = "somm.mood.relax";
pub const T_SOMM_MOOD_ENERGY: Key = "somm.mood.energy";
pub const T_SOMM_MOOD_CREATIVE: Key = "somm.mood.creative";
pub const T_SOMM_MOOD_SLEEP: Key = "somm.mood.sleep";
pub const T_SOMM_MOOD_STRONG: Key = "somm.mood.strong";
pub const T_SOMM_MOOD_TASTE: Key = "somm.mood.taste";
pub const T_SOMM_TIME_DAY: Key = "somm.time.day";
pub const T_SOMM_TIME_EVENING: Key = "somm.time.evening";
pub const T_SOMM_TIME_ANY: Key = "somm.time.any";
pub const T_SOMM_EXP_BEGINNER: Key = "somm.exp.beginner";
pub const T_SOMM_EXP_MEDIUM: Key = "somm.exp.medium";
pub const T_SOMM_EXP_EXPERT: Key = "somm.exp.expert";

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
pub const T_SUCCESS_REWARDS_GARDEN: Key = "success.rewards.garden";
pub const T_SUCCESS_REWARDS_BONUS: Key = "success.rewards.bonus";
pub const T_SUCCESS_CASHBACK_EARNED: Key = "success.cashback_earned";
pub const T_SUCCESS_SHARE_REFERRAL: Key = "success.share_referral";
pub const T_SUCCESS_REORDER: Key = "success.reorder";

// Orders screen hard-coded strings
pub const T_ORDERS_HISTORY: Key = "orders.history";
pub const T_ORDERS_NO_ORDERS: Key = "orders.no_orders";
pub const T_ORDERS_BROWSE_SETS: Key = "orders.browse_sets";
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

// Profile screen hard-coded strings
pub const T_PROFILE_MEMBERSHIP: Key = "profile.membership";
pub const T_PROFILE_QR_CODE: Key = "profile.qr_code";
pub const T_PROFILE_COPY_LINK: Key = "profile.copy_link";
pub const T_PROFILE_SHARE: Key = "profile.share";
pub const T_PROFILE_FRIENDS_INVITED: Key = "profile.friends_invited";
pub const T_PROFILE_REFERRAL_LINK: Key = "profile.referral_link";
pub const T_PROFILE_COPY: Key = "profile.copy";
pub const T_PROFILE_INVITED: Key = "profile.invited";
pub const T_PROFILE_EARN_PER_REF: Key = "profile.earn_per_ref";
pub const T_PROFILE_QUICK_ACTIONS: Key = "profile.quick_actions";
pub const T_PROFILE_MY_ORDERS: Key = "profile.my_orders";
pub const T_PROFILE_MY_GARDEN: Key = "profile.my_garden";
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

// Modal hard-coded strings
pub const T_MODAL_CLOSE: Key = "modal.close";
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
pub const T_MENU_SORT_THC: Key = "menu.sort.thc";
pub const T_MENU_NO_RESULTS: Key = "menu.no_results";
pub const T_MENU_SOTD_HERO: Key = "menu.sotd_hero";
pub const T_MENU_NEW_ARRIVALS: Key = "menu.new_arrivals";
pub const T_MENU_PRICE_REQUEST: Key = "menu.price_request";
pub const T_MENU_SOLD_OUT: Key = "menu.sold_out";
pub const T_MENU_THC: Key = "menu.thc";
pub const T_MENU_CBD: Key = "menu.cbd";
pub const T_MENU_SOTD_BADGE: Key = "menu.sotd_badge";
pub const T_MENU_NEW_BADGE: Key = "menu.new_badge";
pub const T_MENU_BEST_BADGE: Key = "menu.best_badge";
pub const T_MENU_SALE_BADGE: Key = "menu.sale_badge";

/// Product card translations
pub const T_STRAIN_BADGE_SOTD: Key = "strain.badge.sotd";
pub const T_STRAIN_BADGE_NEW: Key = "strain.badge.new";
pub const T_STRAIN_BADGE_BEST: Key = "strain.badge.best";
pub const T_STRAIN_BADGE_SALE: Key = "strain.badge.sale";
pub const T_SET_BADGE: Key = "set.badge";
pub const T_MENU_SET_LABEL: Key = "menu.set_label";
pub const T_MENU_OFF: Key = "menu.off";
pub const T_MENU_WEIGHT: Key = "menu.weight";
pub const T_MENU_FLAVOR_PREFIX: Key = "menu.flavor_prefix";
pub const T_MENU_PER_GRAM: Key = "menu.per_gram";

// Home screen hard-coded strings
pub const T_HOME_CATEGORIES: Key = "home.categories";
pub const T_HOME_SETS_PACKS: Key = "home.sets_packs";
pub const T_HOME_SOTD: Key = "home.sotd";
pub const T_HOME_NO_SOTD: Key = "home.no_sotd";
pub const T_HOME_ADVENTURES: Key = "home.adventures";
pub const T_HOME_DAILY_QUEST: Key = "home.daily_quest";
pub const T_HOME_TREASURE_HUNT: Key = "home.treasure_hunt";
pub const T_HOME_AR_HUNT: Key = "home.ar_hunt";
pub const T_HOME_LOCATION_QUEST: Key = "home.location_quest";
pub const T_HOME_SOMMELIER: Key = "home.sommelier";
pub const T_HOME_GAME: Key = "home.game";
pub const T_HOME_SHARE: Key = "home.share";
pub const T_HOME_WATCH_VIDEO: Key = "home.watch_video";
pub const T_HOME_GARDEN_TITLE: Key = "home.garden.title";
pub const T_HOME_GARDEN_WATER: Key = "home.garden.water";
pub const T_HOME_GARDEN_HARVEST: Key = "home.garden.harvest";
pub const T_HOME_GARDEN_GROWING: Key = "home.garden.growing";
pub const T_HOME_GARDEN_EMPTY: Key = "home.garden.empty";
pub const T_HOME_GARDEN_CTA: Key = "home.garden.cta";

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
        T_NAV_GARDEN => "Сад",
        T_NAV_ACCESSORIES => "Аксессуары",
        T_NAV_TEA => "Напитки",
        T_NAV_QUEST => "Квест",
        T_NAV_GAME => "Игра",
        T_NAV_EVENTS => "События",
        T_NAV_CART => "Корзина",
        T_NAV_PROFILE => "Профиль",
        T_NAV_MORE => "Ещё",
        // Garden
        T_GARDEN_TITLE => "Мой сад",
        T_GARDEN_SUBTITLE => "Выращивайте и собирайте урожай",
        T_GARDEN_EMPTY_LABEL => "Пока нет растений",
        T_GARDEN_EMPTY_CTA => "Закажите любой товар, чтобы получить первое семечко!",
        T_GARDEN_LOADING => "Загружаем ваш сад...",
        T_GARDEN_CHANGE_PRODUCT => "Сменить товар",
        T_GARDEN_RESET_PROGRESS => "Начать сначала",
        T_GARDEN_RESET_CONFIRM_TITLE => "Начать сначала?",
        T_GARDEN_RESET_CONFIRM_BODY => "Текущий прогресс полива будет сброшен. Растение вернётся к семечку (0/14), но целевой товар останется прежним.",
        T_GARDEN_CANCEL => "Отмена",
        T_GARDEN_CONFIRM_RESET => "Сбросить прогресс",
        T_GARDEN_TAB_GARDEN => "Сад",
        T_GARDEN_TAB_GAME => "Игра",
        T_GARDEN_HARVEST => "Собрать",
        T_GARDEN_PLANT => "Посадить",
        T_GARDEN_ONBOARD_TITLE => "Как работает сад",
        T_GARDEN_ONBOARD_STEP1 => "1. Посадите семечко из заказа",
        T_GARDEN_ONBOARD_STEP2 => "2. Поливайте растение, чтобы оно росло",
        T_GARDEN_ONBOARD_STEP3 => "3. Соберите урожай и получите награды",
        T_GARDEN_ONBOARD_CTA => "Понятно",
        T_GARDEN_DIAGNOSTICS_COPY => "📋 Скопировать диагностику",
        T_GARDEN_DIAGNOSTICS_COPIED => "📋 Диагностика скопирована",
        T_GARDEN_CHOOSE_PRODUCT => "🌱 Выбрать товар",
        T_GARDEN_CHOOSE_PRODUCT_HINT => "↑ Нажми «Выбрать товар» вверху",
        T_GARDEN_PLANT_ALT => "Растение",
        T_GARDEN_PRODUCT_ALT => "Товар",
        T_GARDEN_DISCOUNT_BADGE => "🎯 скидка",
        T_GARDEN_READY => "🏆 ГОТОВО",
        T_GARDEN_COOLDOWN => "⏳ Перерыв...",
        T_GARDEN_CHOOSER_TITLE => "Выбери товар для скидки",
        T_GARDEN_CHOOSER_EMPTY => "Нет доступных товаров",
        T_GARDEN_CHOOSER_LOADING => "Загрузка...",
        T_GARDEN_CHOOSER_ERROR => "Ошибка: {0}",
        T_GARDEN_ERROR_HARVEST => "Не удалось собрать урожай: {0}",
        T_GARDEN_ERROR_RESET => "Не удалось сбросить: {0}",
        T_GARDEN_ERROR_COOLDOWN => "Скидку можно растить раз в сутки — подожди после прошлого сбора.",
        T_GARDEN_ERROR_PRODUCT_UNAVAILABLE => "Товар недоступен.",
        T_GARDEN_CAT_STRAIN => "🌿 Сорта",
        T_GARDEN_CAT_ACCESSORY => "💨 Аксессуары",
        T_GARDEN_CAT_TEA => "🥤 Напитки",
        T_GARDEN_CAT_SET => "📦 Наборы",
        T_GARDEN_CAT_ACCESSORY_SET => "🔧 Сеты аксессуаров",
        T_GARDEN_CAT_TEA_SET => "🫖 Сеты напитков",
        T_GARDEN_CAT_OTHER => "Прочее",
        T_CLOSE => "Закрыть",
        // Telegram buttons
        T_BTN_CART => "Корзина",
        T_BTN_CHECKOUT => "Оформить",
        T_BTN_PAY => "Оплатить",
        T_BTN_WATER => "Полить",
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
        T_TITLE => "Woody Island Quest",
        T_SUBTITLE => "Пройди 5 точек на Пангане",
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
        T_REWARD => "🌿 Косячок от Woody",
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
        T_EVENTS_SUBTITLE => "Календарь мероприятий Woody",
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
        T_EVENTS_INSUFFICIENT_STARS => "Недостаточно Stars. Заработайте в играх Woody или пополните баланс.",
        T_EVENTS_REMINDER_BODY => "Напоминаем: вы забронировали мероприятие «{0}». Начало: {1}. Ждём вас!",
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
        T_HOME_SUBTITLE => "Премиум каннабис на острове Панган",
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
        T_SOMM_TITLE => "🍷 Сомелье",
        T_SOMM_DESC => "Подберём сорт под настроение",
        T_SOMM_REASON_INDICA_RELAX => "Indica · расслабляющий",
        T_SOMM_REASON_HYBRID_MELLOW => "Hybrid · мягкий",
        T_SOMM_REASON_SATIVA_ENERGY => "Sativa · бодрящий",
        T_SOMM_REASON_HYBRID_UP => "Hybrid · приподнимающий",
        T_SOMM_REASON_CREATIVE => "Творческий & сфокусированный",
        T_SOMM_REASON_SLEEP => "Indica · для сна",
        T_SOMM_REASON_FLAVOR => "Насыщенный вкус",
        T_SOMM_REASON_THC => "Высокий THC · {0}%",
        T_SOMM_REASON_DEFAULT => "Хорошее совпадение",
        T_CART_TITLE => "🛒 Корзина",
        T_CART_EMPTY => "Корзина пуста",
        T_CART_EMPTY_DESC => "Добавьте товары из каталога",
        T_CART_BROWSE_MENU => "🌿 В меню",
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
        T_DELIVERY_FEE => "Стоимость доставки: {0} ฿",
        T_PAYMENT => "Оплата",
        T_PLACE_ORDER => "Оформить заказ ✓",
        T_CHECKOUT_ERR_NAME => "Укажите ваше имя",
        T_CHECKOUT_ERR_NAME_LONG => "Имя слишком длинное (макс. 200 символов)",
        T_CHECKOUT_ERR_PHONE => "Укажите номер телефона",
        T_CHECKOUT_ERR_PHONE_LONG => "Телефон слишком длинный (макс. 50 символов)",
        T_CHECKOUT_ERR_PHONE_INVALID => "Введите корректный номер (мин. 5 цифр)",
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
        T_SHARE_MESSAGE => "Посмотри {0} в Woody Weed 👇",
        T_REFERRAL_TITLE => "🎁 Реферальная программа",
        T_REFERRAL_SUBTITLE => "Приглашай друзей — получай бонусы",
        T_REFERRAL_LINK_LABEL => "ВАША РЕФЕРАЛЬНАЯ ССЫЛКА",
        T_REFERRAL_COPY => "📋 Копировать",
        T_REFERRAL_COPIED => "✅ Скопировано!",
        T_REFERRAL_SHARE => "📤 Поделиться",
        T_REFERRAL_SHARE_TEXT => "🪵 Присоединяйся к Woody Weed и получай бонусы!",
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
        T_REVIEWS_TITLE => "Отзывы",
        T_REVIEWS_AVG => "Средняя оценка: {0}",
        T_REVIEWS_EMPTY => "Пока нет отзывов. Будьте первым!",
        T_REVIEW_LEAVE => "Оставить отзыв",
        T_REVIEW_RATING => "Оценка",
        T_REVIEW_COMMENT => "Комментарий",
        T_REVIEW_SUBMIT => "Отправить",
        T_REVIEW_THANKS => "Спасибо за отзыв!",
        T_LAB_CERTS => "Лабораторные тесты",
        T_LAB_CERT_THC => "THC",
        T_LAB_CERT_CBD => "CBD",
        T_LAB_CERT_TESTED => "Тестировано",
        T_LAB_CERT_EMPTY => "Нет сертификатов",
        T_TRUST_GACP => "✅ GACP-сертифицированная продукция",
        T_TRUST_MEDICAL => "🏥 Только медицинское применение. Перед употреблением проконсультируйтесь с врачом.",
        T_TRUST_SUPPORT => "💬 Поддержка",
        T_TRUST_AGE => "🔞 20+",
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
        T_SOMM_MOOD => "Настроение",
        T_SOMM_TIME => "Время суток",
        T_SOMM_EXP => "Опыт",
        T_SOMM_RESULT => "Рекомендации",
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
        T_CART_BONUS_NUDGE => "🎁 У вас {0} ฿ бонусов — применим при оформлении",
        T_CART_DECREASE_QTY => "Убавить количество",
        T_CART_REMOVE => "Удалить товар",
        T_CART_IMAGE_ALT => "Фото: {0}",
        // Checkout screen
        T_CHECKOUT_CART_EMPTY => "Корзина пуста",
        T_CHECKOUT_STEP_CART => "Корзина",
        T_CHECKOUT_STEP_DETAILS => "Детали",
        T_CHECKOUT_STEP_CONFIRM => "Подтверждение",
        T_CHECKOUT_SELECT_ZONE => "Выберите зону доставки",
        T_CHECKOUT_GARDEN_DISCOUNT => "🌱 Скидка из сада",
        T_CHECKOUT_GARDEN_DISCOUNT_PCT => "{0}% на {1}",
        T_CHECKOUT_STARS => "⭐ Звёзды",
        T_CHECKOUT_STARS_AVAILABLE => "доступно {0}",
        T_CHECKOUT_STARS_MINUS => "−{0} ฿",
        T_CHECKOUT_BONUS => "🎁 Бонусы",
        T_CHECKOUT_BONUS_AVAILABLE => "доступно {0} ฿",
        T_CHECKOUT_BONUS_APPLIED => "−{0} ฿",
        T_CHECKOUT_BONUS_MAX => "макс. {0} ฿",
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
        T_SOMM_GET_RECOMMENDATIONS => "🔮 Подобрать",
        T_SOMM_RECOMMENDED_FOR_YOU => "✨ Рекомендации для вас",
        T_SOMM_RESTART => "↻ Заново",
        T_SOMM_RECOMMENDED_SETS => "📦 Рекомендуемые наборы",
        T_SOMM_RECOMMENDED_STRAINS => "🌿 Рекомендуемые сорта",
        T_SOMM_NO_RECOMMENDATIONS => "Ничего не нашлось. Попробуйте другие варианты!",
        T_SOMM_MATCH => "совпадение",
        T_SOMM_MOOD_RELAX => "😌 Расслабиться",
        T_SOMM_MOOD_ENERGY => "⚡ Энергия",
        T_SOMM_MOOD_CREATIVE => "🎨 Творчество",
        T_SOMM_MOOD_SLEEP => "😴 Сон",
        T_SOMM_MOOD_STRONG => "💪 Крепкое",
        T_SOMM_MOOD_TASTE => "👅 Вкус",
        T_SOMM_TIME_DAY => "☀️ День",
        T_SOMM_TIME_EVENING => "🌙 Вечер",
        T_SOMM_TIME_ANY => "🔄 Любое",
        T_SOMM_EXP_BEGINNER => "🌱 Новичок",
        T_SOMM_EXP_MEDIUM => "🌿 Средний",
        T_SOMM_EXP_EXPERT => "🔥 Эксперт",
        // Game strings
        T_GAME_TITLE => "WOODY SHOP",
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
        T_GAME_SHOP_TITLE => "WOODY SHOP",
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
        T_SUCCESS_REWARDS_GARDEN => "🌱 Семя сада за strain-заказ",
        T_SUCCESS_REWARDS_BONUS => "⭐ Бонусные баллы: {0}",
        T_SUCCESS_CASHBACK_EARNED => "💸 +{0} ฿ кешбэка начислено",
        T_SUCCESS_SHARE_REFERRAL => "👥 Пригласить друга",
        T_SUCCESS_REORDER => "🔄 Повторить заказ",
        // Orders screen
        T_ORDERS_HISTORY => "История заказов",
        T_ORDERS_NO_ORDERS => "Пока нет заказов",
        T_ORDERS_BROWSE_SETS => "Смотреть наборы 🎁",
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
        // Profile screen
        T_PROFILE_MEMBERSHIP => "Ваш статус",
        T_PROFILE_QR_CODE => "Ваш QR-код",
        T_PROFILE_COPY_LINK => "Копировать ссылку",
        T_PROFILE_SHARE => "Поделиться",
        T_PROFILE_FRIENDS_INVITED => "👥 Приглашено друзей: {0}",
        T_PROFILE_REFERRAL_LINK => "🔗 Реферальная ссылка",
        T_PROFILE_COPY => "Копировать",
        T_PROFILE_INVITED => "👥 Приглашено: {0}",
        T_PROFILE_EARN_PER_REF => "Получайте ฿100 за друга",
        T_PROFILE_QUICK_ACTIONS => "Быстрые действия",
        T_PROFILE_MY_ORDERS => "Мои заказы",
        T_PROFILE_MY_GARDEN => "Мой сад",
        T_PROFILE_QUESTS => "Квесты",
        T_PROFILE_REFERRAL_PROGRAM => "Реферальная программа",
        T_PROFILE_TIER_BENEFITS => "💎 Привилегии уровня",
        T_PROFILE_TIER_STARTER => "Новичок",
        T_PROFILE_TIER_BRONZE => "Бронзовый бутон",
        T_PROFILE_TIER_SILVER => "Серебряный бутон",
        T_PROFILE_TIER_GOLD => "Золотой бутон",
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
        T_PROFILE_BONUS_REFERRAL => "Реферальный бонус",
        T_PROFILE_BONUS_GARDEN => "Награда из сада",
        T_PROFILE_BONUS_CASHBACK => "Кэшбэк с заказа",
        T_PROFILE_BONUS_ADMIN => "Админ-начисление",
        T_PROFILE_BONUS_OTHER => "Бонус",
        // Modal
        T_MODAL_CLOSE => "Закрыть",
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
        T_MENU_SORT_THC => "🔥 THC",
        T_MENU_NO_RESULTS => "Не найдено {0} сортов",
        T_MENU_SOTD_HERO => "🔥 СОРТ ДНЯ",
        T_MENU_NEW_ARRIVALS => "🆕 НОВИНКИ",
        T_MENU_PRICE_REQUEST => "Цена по запросу",
        T_MENU_SOLD_OUT => "Нет в наличии",
        T_MENU_THC => "THC {0}%",
        T_MENU_CBD => "CBD {0}%",
        T_MENU_SOTD_BADGE => "⭐ СОТД",
        T_MENU_NEW_BADGE => "🆕 НОВИНКА",
        T_MENU_BEST_BADGE => "⭐ ЛУЧШЕЕ",
        T_MENU_SALE_BADGE => "🔥 СКИДКА",
        T_MENU_SET_LABEL => "📦 НАБОР",
        T_STRAIN_BADGE_SOTD => "🌟 Сорт дня",
        T_STRAIN_BADGE_NEW => "🆕 Новинка",
        T_STRAIN_BADGE_BEST => "⭐ Лучшее",
        T_STRAIN_BADGE_SALE => "🔥 Скидка",
        T_SET_BADGE => "📦 НАБОР",
        T_MENU_OFF => "{0}%",
        T_MENU_WEIGHT => "⚖️ {0}",
        T_MENU_FLAVOR_PREFIX => "🍃 {0}",
        T_MENU_PER_GRAM => "/г",
        // Home screen
        T_HOME_CATEGORIES => "Категории",
        T_HOME_SETS_PACKS => "📦 Наборы",
        T_HOME_SOTD => "⭐ Сорт дня",
        T_HOME_NO_SOTD => "Сорт дня пока не выбран",
        T_HOME_ADVENTURES => "🎯 Приключения",
        T_HOME_DAILY_QUEST => "Ежедневный квест",
        T_HOME_TREASURE_HUNT => "Охота за сокровищами",
        T_HOME_AR_HUNT => "AR-охота",
        T_HOME_LOCATION_QUEST => "Локационный квест",
        T_HOME_SOMMELIER => "Сомелье",
        T_HOME_GAME => "Игра",
        T_HOME_SHARE => "Поделиться",
        T_HOME_WATCH_VIDEO => "Смотреть видео",
        T_HOME_GARDEN_TITLE => "🌱 Мой сад",
        T_HOME_GARDEN_WATER => "💧 Полить",
        T_HOME_GARDEN_HARVEST => "🏆 Собрать",
        T_HOME_GARDEN_GROWING => "растёт",
        T_HOME_GARDEN_EMPTY => "Посадите семя из заказа — получите скидку",
        T_HOME_GARDEN_CTA => "В сад",
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
        T_NAV_GARDEN => "Garden",
        T_NAV_ACCESSORIES => "Accessories",
        T_NAV_TEA => "Drinks",
        T_NAV_QUEST => "Quest",
        T_NAV_GAME => "Game",
        T_NAV_EVENTS => "Events",
        T_NAV_CART => "Cart",
        T_NAV_PROFILE => "Profile",
        T_NAV_MORE => "More",
        // Garden
        T_GARDEN_TITLE => "My Garden",
        T_GARDEN_SUBTITLE => "Grow and harvest your plants",
        T_GARDEN_EMPTY_LABEL => "No plants yet",
        T_GARDEN_EMPTY_CTA => "Order any product to get your first seed!",
        T_GARDEN_LOADING => "Loading your garden...",
        T_GARDEN_CHANGE_PRODUCT => "Change product",
        T_GARDEN_RESET_PROGRESS => "Start over",
        T_GARDEN_RESET_CONFIRM_TITLE => "Start over?",
        T_GARDEN_RESET_CONFIRM_BODY => "Current watering progress will be reset. The plant returns to seed (0/14), but the chosen product stays the same.",
        T_GARDEN_CANCEL => "Cancel",
        T_GARDEN_CONFIRM_RESET => "Reset progress",
        T_GARDEN_TAB_GARDEN => "Garden",
        T_GARDEN_TAB_GAME => "Game",
        T_GARDEN_HARVEST => "Harvest",
        T_GARDEN_PLANT => "Plant",
        T_GARDEN_ONBOARD_TITLE => "How Garden works",
        T_GARDEN_ONBOARD_STEP1 => "1. Plant a seed from your order",
        T_GARDEN_ONBOARD_STEP2 => "2. Water it to grow through stages",
        T_GARDEN_ONBOARD_STEP3 => "3. Harvest and earn rewards",
        T_GARDEN_ONBOARD_CTA => "Got it",
        T_GARDEN_DIAGNOSTICS_COPY => "📋 Copy diagnostics",
        T_GARDEN_DIAGNOSTICS_COPIED => "📋 Diagnostics copied",
        T_GARDEN_CHOOSE_PRODUCT => "🌱 Choose product",
        T_GARDEN_CHOOSE_PRODUCT_HINT => "↑ Tap Choose product above",
        T_GARDEN_PLANT_ALT => "Plant",
        T_GARDEN_PRODUCT_ALT => "Product",
        T_GARDEN_DISCOUNT_BADGE => "🎯 discount",
        T_GARDEN_READY => "🏆 READY",
        T_GARDEN_COOLDOWN => "⏳ Cooldown...",
        T_GARDEN_CHOOSER_TITLE => "Pick a product for discount",
        T_GARDEN_CHOOSER_EMPTY => "No products available",
        T_GARDEN_CHOOSER_LOADING => "Loading...",
        T_GARDEN_CHOOSER_ERROR => "Error: {0}",
        T_GARDEN_ERROR_HARVEST => "Failed to harvest: {0}",
        T_GARDEN_ERROR_RESET => "Failed to reset: {0}",
        T_GARDEN_ERROR_COOLDOWN => "Discount can be grown once a day — wait until after the last harvest.",
        T_GARDEN_ERROR_PRODUCT_UNAVAILABLE => "Product unavailable.",
        T_GARDEN_CAT_STRAIN => "🌿 Strains",
        T_GARDEN_CAT_ACCESSORY => "💨 Accessories",
        T_GARDEN_CAT_TEA => "🥤 Drinks",
        T_GARDEN_CAT_SET => "📦 Sets",
        T_GARDEN_CAT_ACCESSORY_SET => "🔧 Accessory sets",
        T_GARDEN_CAT_TEA_SET => "🫖 Drink sets",
        T_GARDEN_CAT_OTHER => "Other",
        T_CLOSE => "Close",
        // Telegram buttons
        T_BTN_CART => "Cart",
        T_BTN_CHECKOUT => "Checkout",
        T_BTN_PAY => "Pay",
        T_BTN_WATER => "Water",
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
        T_TITLE => "Woody Island Quest",
        T_SUBTITLE => "Complete 5 locations on Koh Phangan",
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
        T_REWARD => "🌿 Pre-roll from Woody",
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
        T_EVENTS_SUBTITLE => "Woody events calendar",
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
        T_EVENTS_INSUFFICIENT_STARS => "Not enough Stars. Earn them in Woody games or top up your balance.",
        T_EVENTS_REMINDER_BODY => "Reminder: you booked «{0}». Starts at: {1}. See you there!",
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
        T_HOME_SUBTITLE => "Premium Cannabis on Koh Phangan",
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
        T_SOMM_TITLE => "🍷 Sommelier",
        T_SOMM_DESC => "Find your perfect strain",
        T_SOMM_REASON_INDICA_RELAX => "Indica · relaxing",
        T_SOMM_REASON_HYBRID_MELLOW => "Hybrid · mellow",
        T_SOMM_REASON_SATIVA_ENERGY => "Sativa · energizing",
        T_SOMM_REASON_HYBRID_UP => "Hybrid · uplifting",
        T_SOMM_REASON_CREATIVE => "Creative & focused",
        T_SOMM_REASON_SLEEP => "Indica · sleep",
        T_SOMM_REASON_FLAVOR => "Rich flavor",
        T_SOMM_REASON_THC => "High THC · {0}%",
        T_SOMM_REASON_DEFAULT => "Good match",
        T_CART_TITLE => "🛒 Cart",
        T_CART_EMPTY => "Your cart is empty",
        T_CART_EMPTY_DESC => "Browse the menu to add items",
        T_CART_BROWSE_MENU => "🌿 Browse menu",
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
        T_DELIVERY_FEE => "Delivery fee: {0} ฿",
        T_PAYMENT => "Payment",
        T_PLACE_ORDER => "Place Order ✓",
        T_CHECKOUT_ERR_NAME => "Please enter your name",
        T_CHECKOUT_ERR_NAME_LONG => "Name is too long (max 200 characters)",
        T_CHECKOUT_ERR_PHONE => "Please enter your phone number",
        T_CHECKOUT_ERR_PHONE_LONG => "Phone number is too long (max 50 characters)",
        T_CHECKOUT_ERR_PHONE_INVALID => "Please enter a valid phone number (min 5 digits)",
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
        T_SHARE_MESSAGE => "Check out {0} in Woody Weed 👇",
        T_REFERRAL_TITLE => "🎁 Referral Program",
        T_REFERRAL_SUBTITLE => "Invite friends — earn bonuses",
        T_REFERRAL_LINK_LABEL => "YOUR REFERRAL LINK",
        T_REFERRAL_COPY => "📋 Copy",
        T_REFERRAL_COPIED => "✅ Copied!",
        T_REFERRAL_SHARE => "📤 Share",
        T_REFERRAL_SHARE_TEXT => "🪵 Join Woody Weed and get bonuses!",
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
        T_REVIEWS_TITLE => "Reviews",
        T_REVIEWS_AVG => "Average rating: {0}",
        T_REVIEWS_EMPTY => "No reviews yet. Be the first!",
        T_REVIEW_LEAVE => "Leave a review",
        T_REVIEW_RATING => "Rating",
        T_REVIEW_COMMENT => "Comment",
        T_REVIEW_SUBMIT => "Submit",
        T_REVIEW_THANKS => "Thanks for your review!",
        T_LAB_CERTS => "Lab tests",
        T_LAB_CERT_THC => "THC",
        T_LAB_CERT_CBD => "CBD",
        T_LAB_CERT_TESTED => "Tested",
        T_LAB_CERT_EMPTY => "No certificates",
        T_TRUST_GACP => "✅ GACP-certified products",
        T_TRUST_MEDICAL => "🏥 Medical use only. Consult a physician before use.",
        T_TRUST_SUPPORT => "💬 Support",
        T_TRUST_AGE => "🔞 20+",
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
        T_SOMM_MOOD => "Mood",
        T_SOMM_TIME => "Time of Day",
        T_SOMM_EXP => "Experience",
        T_SOMM_RESULT => "Recommendations",
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
        T_CART_BONUS_NUDGE => "🎁 {0} ฿ bonus available at checkout",
        T_CART_DECREASE_QTY => "Decrease quantity",
        T_CART_REMOVE => "Remove item",
        T_CART_IMAGE_ALT => "Photo: {0}",
        // Checkout screen
        T_CHECKOUT_CART_EMPTY => "Cart is empty",
        T_CHECKOUT_STEP_CART => "Cart",
        T_CHECKOUT_STEP_DETAILS => "Details",
        T_CHECKOUT_STEP_CONFIRM => "Confirm",
        T_CHECKOUT_SELECT_ZONE => "Select delivery zone",
        T_CHECKOUT_GARDEN_DISCOUNT => "🌱 Garden discount",
        T_CHECKOUT_GARDEN_DISCOUNT_PCT => "{0}% off {1}",
        T_CHECKOUT_STARS => "⭐ Stars",
        T_CHECKOUT_STARS_AVAILABLE => "{0} available",
        T_CHECKOUT_STARS_MINUS => "−{0} ฿",
        T_CHECKOUT_BONUS => "🎁 Bonus",
        T_CHECKOUT_BONUS_AVAILABLE => "{0} ฿ available",
        T_CHECKOUT_BONUS_APPLIED => "−{0} ฿",
        T_CHECKOUT_BONUS_MAX => "max {0} ฿",
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
        T_SOMM_GET_RECOMMENDATIONS => "Get recommendations",
        T_SOMM_RECOMMENDED_FOR_YOU => "Recommended for you",
        T_SOMM_RESTART => "Start over",
        T_SOMM_RECOMMENDED_SETS => "Recommended sets",
        T_SOMM_RECOMMENDED_STRAINS => "Recommended strains",
        T_SOMM_NO_RECOMMENDATIONS => "No recommendations for these options. Try changing mood or experience.",
        T_SOMM_MATCH => "match",
        T_SOMM_MOOD_RELAX => "😌 Relax",
        T_SOMM_MOOD_ENERGY => "⚡ Energy",
        T_SOMM_MOOD_CREATIVE => "🎨 Creative",
        T_SOMM_MOOD_SLEEP => "😴 Sleep",
        T_SOMM_MOOD_STRONG => "💪 Strong",
        T_SOMM_MOOD_TASTE => "👅 Taste",
        T_SOMM_TIME_DAY => "☀️ Day",
        T_SOMM_TIME_EVENING => "🌙 Evening",
        T_SOMM_TIME_ANY => "🔄 Any",
        T_SOMM_EXP_BEGINNER => "🌱 Beginner",
        T_SOMM_EXP_MEDIUM => "🌿 Intermediate",
        T_SOMM_EXP_EXPERT => "🔥 Expert",
        // Game strings
        T_GAME_TITLE => "Woody Games",
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
        T_GAME_SHOP_TITLE => "WOODY SHOP",
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
        T_GAME_LOG_MOVED_TO_TABLE => "Woody moved to table {0}",
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
        T_SUCCESS_REWARDS_GARDEN => "🌱 Garden seed for strain orders",
        T_SUCCESS_REWARDS_BONUS => "⭐ Bonus points: {0}",
        T_SUCCESS_CASHBACK_EARNED => "💸 +{0} ฿ cashback earned",
        T_SUCCESS_SHARE_REFERRAL => "👥 Invite a friend",
        T_SUCCESS_REORDER => "🔄 Reorder",
        // Orders screen
        T_ORDERS_HISTORY => "Your order history",
        T_ORDERS_NO_ORDERS => "No orders yet",
        T_ORDERS_BROWSE_SETS => "Browse Sets 🎁",
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
        // Profile screen
        T_PROFILE_MEMBERSHIP => "Your membership status",
        T_PROFILE_QR_CODE => "Your QR Code",
        T_PROFILE_COPY_LINK => "Copy Link",
        T_PROFILE_SHARE => "Share",
        T_PROFILE_FRIENDS_INVITED => "👥 {0} friends invited",
        T_PROFILE_REFERRAL_LINK => "🔗 Referral Link",
        T_PROFILE_COPY => "Copy",
        T_PROFILE_INVITED => "👥 {0} invited",
        T_PROFILE_EARN_PER_REF => "Earn ฿100 per referral",
        T_PROFILE_QUICK_ACTIONS => "Quick Actions",
        T_PROFILE_MY_ORDERS => "My Orders",
        T_PROFILE_MY_GARDEN => "My Garden",
        T_PROFILE_QUESTS => "Quests",
        T_PROFILE_REFERRAL_PROGRAM => "Referral Program",
        T_PROFILE_TIER_BENEFITS => "💎 Tier Benefits",
        T_PROFILE_TIER_STARTER => "Starter",
        T_PROFILE_TIER_BRONZE => "Bronze Bud",
        T_PROFILE_TIER_SILVER => "Silver Bud",
        T_PROFILE_TIER_GOLD => "Gold Bud",
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
        T_PROFILE_BONUS_REFERRAL => "Referral bonus",
        T_PROFILE_BONUS_GARDEN => "Garden reward",
        T_PROFILE_BONUS_CASHBACK => "Order cashback",
        T_PROFILE_BONUS_ADMIN => "Admin grant",
        T_PROFILE_BONUS_OTHER => "Bonus",
        // Modal
        T_MODAL_CLOSE => "Close",
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
        T_MENU_SORT_THC => "🔥 THC",
        T_MENU_NO_RESULTS => "No {0} strains found",
        T_MENU_SOTD_HERO => "🔥 STRAIN OF THE DAY",
        T_MENU_NEW_ARRIVALS => "🆕 NEW ARRIVALS",
        T_MENU_PRICE_REQUEST => "Price on request",
        T_MENU_SOLD_OUT => "Sold Out",
        T_MENU_THC => "THC {0}%",
        T_MENU_CBD => "CBD {0}%",
        T_MENU_SOTD_BADGE => "⭐ SOTD",
        T_MENU_NEW_BADGE => "🆕 NEW",
        T_MENU_BEST_BADGE => "⭐ BEST",
        T_MENU_SALE_BADGE => "🔥 SALE",
        T_MENU_SET_LABEL => "📦 SET",
        T_STRAIN_BADGE_SOTD => "🌟 Strain of Day",
        T_STRAIN_BADGE_NEW => "🆕 New Arrival",
        T_STRAIN_BADGE_BEST => "⭐ Best Seller",
        T_STRAIN_BADGE_SALE => "🔥 Sale",
        T_SET_BADGE => "📦 SET",
        T_MENU_OFF => "{0}% OFF",
        T_MENU_WEIGHT => "⚖️ {0}",
        T_MENU_FLAVOR_PREFIX => "🍃 {0}",
        T_MENU_PER_GRAM => "/g",
        // Home screen
        T_HOME_CATEGORIES => "Categories",
        T_HOME_SETS_PACKS => "📦 Packs",
        T_HOME_SOTD => "⭐ Strain of the Day",
        T_HOME_NO_SOTD => "No strain of the day yet",
        T_HOME_ADVENTURES => "🎯 Adventures",
        T_HOME_DAILY_QUEST => "Daily Quest",
        T_HOME_TREASURE_HUNT => "Treasure Hunt",
        T_HOME_AR_HUNT => "AR Hunt",
        T_HOME_LOCATION_QUEST => "Location Quest",
        T_HOME_SOMMELIER => "Sommelier",
        T_HOME_GAME => "Game",
        T_HOME_SHARE => "Share",
        T_HOME_WATCH_VIDEO => "Watch video",
        T_HOME_GARDEN_TITLE => "🌱 My Garden",
        T_HOME_GARDEN_WATER => "💧 Water",
        T_HOME_GARDEN_HARVEST => "🏆 Harvest",
        T_HOME_GARDEN_GROWING => "growing",
        T_HOME_GARDEN_EMPTY => "Plant a seed from your order to earn a discount",
        T_HOME_GARDEN_CTA => "Open Garden",
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
        T_NAV_GARDEN,
        T_NAV_ACCESSORIES,
        T_NAV_TEA,
        T_NAV_QUEST,
        T_NAV_EVENTS,
        T_NAV_CART,
        T_NAV_PROFILE,
        // Garden
        T_GARDEN_TITLE,
        T_GARDEN_SUBTITLE,
        T_GARDEN_EMPTY_LABEL,
        T_GARDEN_EMPTY_CTA,
        T_GARDEN_LOADING,
        T_GARDEN_CHANGE_PRODUCT,
        T_GARDEN_RESET_PROGRESS,
        T_GARDEN_RESET_CONFIRM_TITLE,
        T_GARDEN_RESET_CONFIRM_BODY,
        T_GARDEN_CANCEL,
        T_GARDEN_CONFIRM_RESET,
        T_GARDEN_DIAGNOSTICS_COPY,
        T_GARDEN_DIAGNOSTICS_COPIED,
        T_GARDEN_CHOOSE_PRODUCT,
        T_GARDEN_CHOOSE_PRODUCT_HINT,
        T_GARDEN_PLANT_ALT,
        T_GARDEN_PRODUCT_ALT,
        T_GARDEN_DISCOUNT_BADGE,
        T_GARDEN_READY,
        T_GARDEN_COOLDOWN,
        T_GARDEN_CHOOSER_TITLE,
        T_GARDEN_CHOOSER_EMPTY,
        T_GARDEN_CHOOSER_LOADING,
        T_GARDEN_CHOOSER_ERROR,
        T_GARDEN_ERROR_HARVEST,
        T_GARDEN_ERROR_RESET,
        T_GARDEN_ERROR_COOLDOWN,
        T_GARDEN_ERROR_PRODUCT_UNAVAILABLE,
        T_GARDEN_CAT_STRAIN,
        T_GARDEN_CAT_ACCESSORY,
        T_GARDEN_CAT_TEA,
        T_GARDEN_CAT_SET,
        T_GARDEN_CAT_ACCESSORY_SET,
        T_GARDEN_CAT_TEA_SET,
        T_GARDEN_CAT_OTHER,
        T_CLOSE,
        // Telegram buttons
        T_BTN_CART,
        T_BTN_CHECKOUT,
        T_BTN_PAY,
        T_BTN_WATER,
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
        T_SOMM_TITLE,
        T_SOMM_DESC,
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
        T_SOMM_MOOD,
        T_SOMM_TIME,
        T_SOMM_EXP,
        T_SOMM_RESULT,
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
        T_SOMM_GET_RECOMMENDATIONS,
        T_SOMM_RECOMMENDED_FOR_YOU,
        T_SOMM_RESTART,
        T_SOMM_RECOMMENDED_SETS,
        T_SOMM_RECOMMENDED_STRAINS,
        T_SOMM_NO_RECOMMENDATIONS,
        T_SOMM_MATCH,
        T_SOMM_MOOD_RELAX,
        T_SOMM_MOOD_ENERGY,
        T_SOMM_MOOD_CREATIVE,
        T_SOMM_MOOD_SLEEP,
        T_SOMM_MOOD_STRONG,
        T_SOMM_MOOD_TASTE,
        T_SOMM_TIME_DAY,
        T_SOMM_TIME_EVENING,
        T_SOMM_TIME_ANY,
        T_SOMM_EXP_BEGINNER,
        T_SOMM_EXP_MEDIUM,
        T_SOMM_EXP_EXPERT,
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
        assert_eq!(t(Lang::Russian, T_TITLE), "Woody Island Quest");
        assert_eq!(t(Lang::English, T_TITLE), "Woody Island Quest");
    }

    #[test]
    fn test_translation_different() {
        assert_eq!(t(Lang::Russian, T_SUBTITLE), "Пройди 5 точек на Пангане");
        assert_eq!(
            t(Lang::English, T_SUBTITLE),
            "Complete 5 locations on Koh Phangan"
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
            Some(&"Пройди 5 точек на Пангане".to_string())
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
