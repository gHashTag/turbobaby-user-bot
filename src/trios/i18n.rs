//! i18n engine for Trios ecosystem

use std::collections::HashMap;
use crate::trios::core::Lang;

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
pub const T_NAV_CART: Key = "nav.cart";
pub const T_NAV_PROFILE: Key = "nav.profile";

/// Garden translations
pub const T_GARDEN_TITLE: Key = "garden.title";
pub const T_GARDEN_SUBTITLE: Key = "garden.subtitle";

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
        T_NAV_TEA => "Чай",
        T_NAV_QUEST => "Квест",
        T_NAV_CART => "Корзина",
        T_NAV_PROFILE => "Профиль",
        // Garden
        T_GARDEN_TITLE => "Мой сад",
        T_GARDEN_SUBTITLE => "Выращивайте и собирайте урожай",
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
        T_NAV_TEA => "Tea",
        T_NAV_QUEST => "Quest",
        T_NAV_CART => "Cart",
        T_NAV_PROFILE => "Profile",
        // Garden
        T_GARDEN_TITLE => "My Garden",
        T_GARDEN_SUBTITLE => "Grow and harvest your plants",
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
        T_NAV_CART,
        T_NAV_PROFILE,
        // Garden
        T_GARDEN_TITLE,
        T_GARDEN_SUBTITLE,
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
}
