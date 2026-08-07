use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Locale {
    pub code: String,
    pub flag: String,
    pub name: String,
    pub welcome: String,
    pub start_description: String,
    pub open_menu: String,
    pub menu: String,
    pub view_sets: String,
    pub sommelier: String,
    pub garden: String,
    pub accessories: String,
    pub quest: String,
    pub profile: String,
    pub my_orders: String,
    pub joke: String,
    pub fact: String,
    pub help: String,
    pub choose_lang: String,
    pub lang_changed: String,
    pub joke_thinking: String,
    pub joke_prompt: String,
    pub joke_fail_fallback: String,
    pub more_joke: String,
    pub fact_thinking: String,
    pub fact_prompt: String,
    pub fact_fail_fallback: String,
    pub interesting_fact: String,
    pub strain_of_day: String,
    pub prev_strain: String,
    pub next_strain: String,
    pub add_to_cart: String,
    pub order_new_header: String,
    pub order_items: String,
    pub order_items_empty: String,
    pub order_grams: String,
    pub order_pickup: String,
    pub order_source: String,
    pub order_anonymous: String,
    pub order_phone_not_set: String,
    pub order_confirm_btn: String,
    pub order_reject_btn: String,
    pub order_complete_btn: String,
    pub order_confirmed: String,
    pub order_rejected: String,
    pub order_status_confirmed: String,
    pub order_status_preparing: String,
    pub order_status_ready: String,
    pub order_status_out_for_delivery: String,
    pub order_status_completed: String,
    pub order_status_rejected: String,
    pub order_status_cancelled: String,
    pub order_open_app: String,
    pub cashback_earned: String,
    pub tier_upgrade: String,
    pub cashback_now: String,
    pub referral_bonus: String,
    pub sets_for_beginners: String,
    pub sets_description: String,
    pub sommelier_description: String,
    pub start_sommelier: String,
    pub help_commands: String,
    pub opening_sets: String,
    pub starting_sommelier: String,
    pub welcome_feature1: String,
    pub welcome_feature2: String,
    pub welcome_feature3: String,
    pub welcome_feature4: String,
    pub welcome_feature5: String,
    pub welcome_feature6: String,
    pub welcome_feature7: String,
    pub lang_instruction: String,
    // Referral system
    pub referral_title: String,
    pub referral_your_code: String,
    pub referral_invited_count: String,
    pub referral_confirmed: String,
    pub referral_pending: String,
    pub referral_bonus_earned: String,
    pub referral_share_button: String,
    pub referral_share_hint: String,
    pub referral_leaderboard: String,
    pub garden_water_reminder: String,
    pub garden_harvest_ready: String,
    pub garden_open_app: String,
    pub cart_abandonment_reminder: String,
    pub cart_open: String,
}

pub(crate) fn get_locale(lang: &str) -> Locale {
    match lang {
        "ru" => ru(),
        _ => en(),
    }
}

pub(crate) fn map_telegram_lang(lang: Option<&str>) -> String {
    match lang {
        Some("ru") | Some("be") | Some("uk") => "ru".into(),
        _ => "en".into(),
    }
}

pub(crate) fn supported_langs() -> Vec<&'static str> {
    vec!["ru", "en"]
}

pub(crate) fn detect_language(text: &str) -> &'static str {
    if text
        .chars()
        .any(|c| ('\u{0400}'..='\u{04FF}').contains(&c) || ('\u{0500}'..='\u{052F}').contains(&c))
    {
        return "ru";
    }
    if text.chars().any(|c| ('\u{0E00}'..='\u{0E7F}').contains(&c)) {
        return "th";
    }
    if text.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c)) {
        return "zh";
    }
    if text.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c)) {
        return "ar";
    }
    "en"
}

fn ru() -> Locale {
    Locale {
        code: "ru".into(),
        flag: "🇷🇺".into(),
        name: "Русский".into(),
        welcome: "Добро пожаловать в Woody Weed! 🪵".into(),
        start_description: "Лучший каннабис-магазин на Ко Пангане".into(),
        open_menu: "Открыть меню".into(),
        menu: "🌿 Меню".into(),
        view_sets: "Наборы".into(),
        sommelier: "Сомелье".into(),
        garden: "Сад".into(),
        accessories: "Аксессуары".into(),
        quest: "Квест".into(),
        profile: "Профиль".into(),
        my_orders: "Мои заказы".into(),
        joke: "Анекдот".into(),
        fact: "Факт".into(),
        help: "Помощь".into(),
        choose_lang: "🌐 Выберите язык:".into(),
        lang_changed: "Язык изменён!".into(),
        joke_thinking: "😜 Придумываю анекдот...".into(),
        joke_prompt: "Расскажи короткий смешной анекдот про каннабис.".into(),
        joke_fail_fallback: "😅 Не смог придумать анекдот, попробуй ещё раз!".into(),
        more_joke: "Ещё анекдот".into(),
        fact_thinking: "🧠 Ищу интересный факт...".into(),
        fact_prompt: "Расскажи один интересный научный факт о каннабисе.".into(),
        fact_fail_fallback: "😅 Не смог найти факт, попробуй ещё раз!".into(),
        interesting_fact: "Ещё факт".into(),
        strain_of_day: "Сорт дня".into(),
        prev_strain: "◀️".into(),
        next_strain: "▶️".into(),
        add_to_cart: "🛒 В корзину".into(),
        order_new_header: "Новый заказ".into(),
        order_items: "Позиции".into(),
        order_items_empty: "(пусто)".into(),
        order_grams: "г".into(),
        order_pickup: "Самовывоз:".into(),
        order_source: "Источник:".into(),
        order_anonymous: "Аноним".into(),
        order_phone_not_set: "не указан".into(),
        order_confirm_btn: "Подтвердить".into(),
        order_reject_btn: "Отклонить".into(),
        order_complete_btn: "Выполнен".into(),
        order_confirmed: "Заказ подтверждён".into(),
        order_rejected: "Заказ отклонён".into(),
        order_status_confirmed: "✅ Ваш заказ подтверждён".into(),
        order_status_preparing: "🔥 Ваш заказ готовится".into(),
        order_status_ready: "📦 Ваш заказ готов к выдаче".into(),
        order_status_out_for_delivery: "🚗 Ваш заказ в пути".into(),
        order_status_completed: "📦 Ваш заказ выполнен".into(),
        order_status_rejected: "❌ Ваш заказ отклонён. Возврат обработан.".into(),
        order_status_cancelled: "❌ Заказ отменён. Возврат обработан.".into(),
        order_open_app: "Открыть приложение".into(),
        cashback_earned: "Кэшбэк начислен".into(),
        tier_upgrade: "Поздравляем! Новый уровень:".into(),
        cashback_now: "Ваш кэшбэк теперь".into(),
        referral_bonus: "Ваш друг сделал первую покупку!".into(),
        sets_for_beginners: "Наборы для начинающих".into(),
        sets_description: "Готовые наборы для комфортного старта".into(),
        sommelier_description: "AI-подбор сортов по вашим предпочтениям".into(),
        start_sommelier: "Начать подбор".into(),
        help_commands: "/start — начало\n/menu — меню\n/joke — анекдот\n/fact — факт\n/lang — язык"
            .into(),
        opening_sets: "Открываю наборы...".into(),
        starting_sommelier: "Запускаю сомелье...".into(),
        welcome_feature1: "Широкий выбор сортов".into(),
        welcome_feature2: "AI-сомелье для подбора".into(),
        welcome_feature3: "Сад и выращивание".into(),
        welcome_feature4: "Квест по острову".into(),
        welcome_feature5: "Аксессуары".into(),
        welcome_feature6: "Программа лояльности".into(),
        welcome_feature7: "Анекдоты и факты".into(),
        lang_instruction: "Отвечай на русском языке.".into(),
        referral_title: "Реферальная программа".into(),
        referral_your_code: "Ваш реф-код".into(),
        referral_invited_count: "Приглашено".into(),
        referral_confirmed: "Подтверждено".into(),
        referral_pending: "Ожидает".into(),
        referral_bonus_earned: "Заработано бонусов".into(),
        referral_share_button: "Поделиться".into(),
        referral_share_hint: "Поделитесь ссылкой и получите бонус за каждого нового друга!".into(),
        referral_leaderboard: "Таблица лидеров".into(),
        garden_water_reminder: "🌱 Вашему растению нужна вода! Зайдите в сад, чтобы полить.".into(),
        garden_harvest_ready: "🏆 Растение готово к сбору урожая! Откройте сад, чтобы получить награду.".into(),
        garden_open_app: "Открыть сад".into(),
        cart_abandonment_reminder: "🛒 Вы не завершили оформление заказа. Товары ждут вас в корзине — вернитесь и заберите их одним касанием.".into(),
        cart_open: "Открыть корзину".into(),
    }
}

fn en() -> Locale {
    Locale {
        code: "en".into(),
        flag: "🇬🇧".into(),
        name: "English".into(),
        welcome: "Welcome to Woody Weed! 🪵".into(),
        start_description: "Best cannabis shop on Koh Phangan".into(),
        open_menu: "Open menu".into(),
        menu: "🌿 Menu".into(),
        view_sets: "Sets".into(),
        sommelier: "Sommelier".into(),
        garden: "Garden".into(),
        accessories: "Accessories".into(),
        quest: "Quest".into(),
        profile: "Profile".into(),
        my_orders: "My orders".into(),
        joke: "Joke".into(),
        fact: "Fact".into(),
        help: "Help".into(),
        choose_lang: "🌐 Choose language:".into(),
        lang_changed: "Language changed!".into(),
        joke_thinking: "😜 Thinking of a joke...".into(),
        joke_prompt: "Tell a short funny cannabis joke.".into(),
        joke_fail_fallback: "😅 Couldn't think of a joke, try again!".into(),
        more_joke: "More jokes".into(),
        fact_thinking: "🧠 Looking for an interesting fact...".into(),
        fact_prompt: "Tell one interesting scientific fact about cannabis.".into(),
        fact_fail_fallback: "😅 Couldn't find a fact, try again!".into(),
        interesting_fact: "More facts".into(),
        strain_of_day: "Strain of the Day".into(),
        prev_strain: "◀️".into(),
        next_strain: "▶️".into(),
        add_to_cart: "🛒 Add to cart".into(),
        order_new_header: "New Order".into(),
        order_items: "Items".into(),
        order_items_empty: "(empty)".into(),
        order_grams: "g".into(),
        order_pickup: "Pickup:".into(),
        order_source: "Source:".into(),
        order_anonymous: "Anonymous".into(),
        order_phone_not_set: "not set".into(),
        order_confirm_btn: "Confirm".into(),
        order_reject_btn: "Reject".into(),
        order_complete_btn: "Complete".into(),
        order_confirmed: "Order confirmed".into(),
        order_rejected: "Order rejected".into(),
        order_status_confirmed: "✅ Your order has been confirmed".into(),
        order_status_preparing: "🔥 Your order is being prepared".into(),
        order_status_ready: "📦 Your order is ready for pickup".into(),
        order_status_out_for_delivery: "🚗 Your order is on the way".into(),
        order_status_completed: "📦 Your order has been completed".into(),
        order_status_rejected: "❌ Your order was rejected. Refund processed.".into(),
        order_status_cancelled: "❌ Order cancelled. Refund processed.".into(),
        order_open_app: "Open app".into(),
        cashback_earned: "Cashback earned".into(),
        tier_upgrade: "Congratulations! New tier:".into(),
        cashback_now: "Your cashback is now".into(),
        referral_bonus: "Your friend made a purchase!".into(),
        sets_for_beginners: "Sets for beginners".into(),
        sets_description: "Ready-made sets for a comfortable start".into(),
        sommelier_description: "AI strain recommendations based on your preferences".into(),
        start_sommelier: "Start selection".into(),
        help_commands: "/start — start\n/menu — menu\n/joke — joke\n/fact — fact\n/lang — language"
            .into(),
        opening_sets: "Opening sets...".into(),
        starting_sommelier: "Starting sommelier...".into(),
        welcome_feature1: "Wide strain selection".into(),
        welcome_feature2: "AI sommelier".into(),
        welcome_feature3: "Garden & growing".into(),
        welcome_feature4: "Island quest".into(),
        welcome_feature5: "Accessories".into(),
        welcome_feature6: "Loyalty program".into(),
        welcome_feature7: "Jokes & facts".into(),
        lang_instruction: "Reply in English.".into(),
        referral_title: "Referral Program".into(),
        referral_your_code: "Your ref code".into(),
        referral_invited_count: "Invited".into(),
        referral_confirmed: "Confirmed".into(),
        referral_pending: "Pending".into(),
        referral_bonus_earned: "Bonus earned".into(),
        referral_share_button: "Share".into(),
        referral_share_hint: "Share your link and earn a bonus for every new friend!".into(),
        referral_leaderboard: "Leaderboard".into(),
        garden_water_reminder: "🌱 Your plant is thirsty! Visit the garden to water it.".into(),
        garden_harvest_ready: "🏆 Your plant is ready to harvest! Open the garden to claim the reward.".into(),
        garden_open_app: "Open garden".into(),
        cart_abandonment_reminder: "🛒 You didn't finish your order. Your items are still in the cart — come back and grab them with one tap.".into(),
        cart_open: "Open cart".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{detect_language, get_locale, map_telegram_lang, supported_langs};

    #[test]
    fn test_get_locale_ru() {
        let l = get_locale("ru");
        assert_eq!(l.code, "ru");
        assert_eq!(l.flag, "🇷🇺");
    }

    #[test]
    fn test_get_locale_en_fallback() {
        let l = get_locale("en");
        assert_eq!(l.code, "en");
        assert_eq!(l.flag, "🇬🇧");
    }

    #[test]
    fn test_get_locale_unknown_fallback() {
        let l = get_locale("fr");
        assert_eq!(l.code, "en");
    }

    #[test]
    fn test_map_telegram_lang_ru() {
        assert_eq!(map_telegram_lang(Some("ru")), "ru");
        assert_eq!(map_telegram_lang(Some("be")), "ru");
        assert_eq!(map_telegram_lang(Some("uk")), "ru");
    }

    #[test]
    fn test_map_telegram_lang_en_fallback() {
        assert_eq!(map_telegram_lang(Some("en")), "en");
        assert_eq!(map_telegram_lang(Some("de")), "en");
        assert_eq!(map_telegram_lang(None), "en");
    }

    #[test]
    fn test_detect_language_ru() {
        assert_eq!(detect_language("Привет"), "ru");
    }

    #[test]
    fn test_detect_language_th() {
        assert_eq!(detect_language("สวัสดี"), "th");
    }

    #[test]
    fn test_detect_language_zh() {
        assert_eq!(detect_language("你好"), "zh");
    }

    #[test]
    fn test_detect_language_ar() {
        assert_eq!(detect_language("مرحبا"), "ar");
    }

    #[test]
    fn test_detect_language_en_fallback() {
        assert_eq!(detect_language("Hello"), "en");
    }

    #[test]
    fn test_detect_language_cyrillic_supplement() {
        assert_eq!(detect_language("\u{0501}"), "ru");
    }

    #[test]
    fn test_supported_langs() {
        assert_eq!(supported_langs(), vec!["ru", "en"]);
    }
}
