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
pub const T_NAV_EVENTS: Key = "nav.events";
pub const T_NAV_CART: Key = "nav.cart";
pub const T_NAV_PROFILE: Key = "nav.profile";

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
pub const T_EVENTS_LOADING: Key = "events.loading";
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
pub const T_TEA_TITLE: Key = "tea.title";
pub const T_TEA_DESC: Key = "tea.description";
pub const T_SOMM_TITLE: Key = "somm.title";
pub const T_SOMM_DESC: Key = "somm.description";
pub const T_CART_TITLE: Key = "cart.title";
pub const T_CART_EMPTY: Key = "cart.empty";
pub const T_CART_EMPTY_DESC: Key = "cart.empty_desc";
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
pub const T_ADD_TO_CART: Key = "btn.add_to_cart";
pub const T_SHARE: Key = "btn.share";
pub const T_SHARE_MESSAGE: Key = "share.message";
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

// Generic API error messages (cycle #74). Used by
// `trios::api_errors::friendly_response_error` for any non-checkout API
// call. T_API_ERR_401 covers the "Telegram session expired" scenario;
// T_API_ERR_UNKNOWN replaces the hardcoded RU `Ошибка сервера: HTTP {n}`
// fallback so non-RU users don't see Cyrillic on a random 418.
pub const T_API_ERR_401: Key = "api.err.401";
pub const T_API_ERR_UNKNOWN: Key = "api.err.unknown";

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
        T_NAV_EVENTS => "События",
        T_NAV_CART => "Корзина",
        T_NAV_PROFILE => "Профиль",
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
        T_EVENTS_LOADING => "Загружаем события...",
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
        T_TEA_TITLE => "🥤 Напитки",
        T_TEA_DESC => "Чай, кофе и другие напитки",
        T_SOMM_TITLE => "🍷 Сомелье",
        T_SOMM_DESC => "Подберём сорт под настроение",
        T_CART_TITLE => "🛒 Корзина",
        T_CART_EMPTY => "Корзина пуста",
        T_CART_EMPTY_DESC => "Добавьте товары из каталога",
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
        T_BACK => "← Назад",
        T_TOTAL => "Итого:",
        T_ADD_TO_CART => "В корзину",
        T_SHARE => "Поделиться",
        T_SHARE_MESSAGE => "Посмотри {0} в Woody Weed 👇",
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
        T_CHECKOUT_ERR_403 => "Аккаунт ограничен или возраст не подтверждён. Проверьте Профиль или свяжитесь с поддержкой.",
        T_CHECKOUT_ERR_404 => "Один из товаров больше не доступен. Обновите меню и попробуйте снова.",
        T_CHECKOUT_ERR_409 => "Этот заказ уже создан. Откройте «Мои заказы» — он там.",
        T_CHECKOUT_ERR_422 => "Цены или товары изменились с момента добавления в корзину. Обновите меню и оформите заказ заново.",
        T_CHECKOUT_ERR_429 => "Слишком быстро. Подождите минуту и попробуйте снова.",
        T_CHECKOUT_ERR_5XX => "Сервер сейчас недоступен. Попробуйте через минуту.",
        // Generic API error messages (cycle #74)
        T_API_ERR_401 => "Войдите в Telegram WebApp заново.",
        T_API_ERR_UNKNOWN => "Что-то пошло не так. Попробуйте позже.",
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
        T_NAV_EVENTS => "Events",
        T_NAV_CART => "Cart",
        T_NAV_PROFILE => "Profile",
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
        T_EVENTS_LOADING => "Loading events...",
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
        T_TEA_TITLE => "🥤 Drinks",
        T_TEA_DESC => "Tea, coffee & more",
        T_SOMM_TITLE => "🍷 Sommelier",
        T_SOMM_DESC => "Find your perfect strain",
        T_CART_TITLE => "🛒 Cart",
        T_CART_EMPTY => "Your cart is empty",
        T_CART_EMPTY_DESC => "Browse the menu to add items",
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
        T_BACK => "← Back",
        T_TOTAL => "Total:",
        T_ADD_TO_CART => "Add to Cart",
        T_SHARE => "Share",
        T_SHARE_MESSAGE => "Check out {0} in Woody Weed 👇",
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
        T_CHECKOUT_ERR_403 => "Account restricted or age not verified. Check Profile or contact support.",
        T_CHECKOUT_ERR_404 => "One of the items is no longer available. Refresh the menu and try again.",
        T_CHECKOUT_ERR_409 => "This order has already been placed. Open «My Orders» — it's there.",
        T_CHECKOUT_ERR_422 => "Prices or items changed since you added to cart. Refresh the menu and place the order again.",
        T_CHECKOUT_ERR_429 => "Too fast. Wait a minute and try again.",
        T_CHECKOUT_ERR_5XX => "Server is currently unavailable. Try again in a minute.",
        // Generic API error messages (cycle #74)
        T_API_ERR_401 => "Sign in to Telegram WebApp again.",
        T_API_ERR_UNKNOWN => "Something went wrong. Please try again later.",
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
        T_EVENTS_LOADING,
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
