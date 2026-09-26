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
    // `sommelier`, `sommelier_description`, `start_sommelier` and
    // `starting_sommelier` were removed with the `/sommelier` command they
    // served. The English one read "AI strain recommendations based on your
    // preferences" and the bot sent it verbatim. #2.
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
    // strain_of_day, prev_strain, next_strain: removed 2026-09-25 with the
    // retired carousel page they labelled (owner: nothing cannabis-related
    // anywhere); its buttons answer with the rental menu (see callbacks.rs).
    pub add_to_cart: String,
    pub order_new_header: String,
    pub order_items: String,
    pub order_items_empty: String,
    // order_grams ("г" / "g", the old shop's unit of sale): removed 2026-09-25.
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
    pub help_commands: String,
    pub opening_sets: String,
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
    // Four fields stood here: `garden` (a menu label nothing rendered) and
    // `garden_water_reminder` / `garden_harvest_ready` / `garden_reward_expiry`
    // — three Telegram messages that `build_message` has no arm for and that
    // nothing enqueues, so no customer could receive one even before D5. Their
    // only readers were the assertions in `bot/notify.rs` that checked they
    // were non-empty, which is how a dead string passes for a live one.
    /// Label on the button every referral notification carries.
    pub referrals_open_app: String,
    pub cart_abandonment_reminder: String,
    pub cart_abandonment_reminder_v2: String,
    pub cart_second_nudge: String,
    pub cart_open: String,
    // Loop #21: referrer-facing lifecycle messages (sent by notification worker)
    pub referral_friend_joined: String,
    // `garden_friend_watered_legacy` stood here, the garden message for queued
    // `friend_watered` rows. Removed 2026-09-26 (owner: nothing cannabis-related
    // anywhere): such rows are now held unsent (src/notification_queue.rs,
    // `DeliverableKind`), and these comment lines keep every cited line put.
    pub referral_friend_ordered: String,
    pub referral_invite_progress_hint: String,
    pub referral_milestone_bonus: String,
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
        welcome: "Добро пожаловать в TurboBaby! 🏍️".into(),
        start_description: "Прокат мотоциклов и скутеров на Пхукете".into(),
        open_menu: "Открыть каталог".into(),
        menu: "🏍 Меню".into(),
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
        joke_prompt: "Расскажи короткий смешной анекдот про мотоциклы или дорожные путешествия.".into(),
        joke_fail_fallback: "😅 Не смог придумать анекдот, попробуй ещё раз!".into(),
        more_joke: "Ещё анекдот".into(),
        fact_thinking: "🧠 Ищу интересный факт...".into(),
        fact_prompt: "Расскажи один интересный факт о Пхукете или Таиланде.".into(),
        fact_fail_fallback: "😅 Не смог найти факт, попробуй ещё раз!".into(),
        interesting_fact: "Ещё факт".into(),
        // strain_of_day, prev_strain and next_strain left on 2026-09-25 with
        // the retired carousel page (the struct says why); these comment lines
        // keep every src/locales.rs line a contract cites where it was.
        add_to_cart: "🛒 В корзину".into(),
        order_new_header: "Новый заказ".into(),
        order_items: "Позиции".into(),
        order_items_empty: "(пусто)".into(),
        // order_grams left the same day (see the struct).
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
        help_commands: "/start — начало\n/menu — меню\n/joke — анекдот\n/fact — факт\n/lang — язык"
            .into(),
        opening_sets: "Открываю наборы...".into(),
        welcome_feature1: "Каталог: от NMAX 155 до X-ADV 750".into(),
        welcome_feature2: "Тарифы на день и на месяц".into(),
        welcome_feature3: "Доставка по всему Пхукету".into(),
        welcome_feature4: "Прозрачные залоги и условия".into(),
        welcome_feature5: "Бронирование прямо в приложении".into(),
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
        referrals_open_app: "Открыть приглашения".into(),
        cart_abandonment_reminder: "🛒 Вы не завершили оформление заказа. Товары ждут вас в корзине — вернитесь и заберите их одним касанием.".into(),
        cart_abandonment_reminder_v2: "👀 В корзине что-то классное осталось. Вернитесь — оформление займёт меньше минуты.".into(),
        cart_second_nudge: "⏰ Ваши товары всё ещё ждут. Оформите сегодня — добавим немного бонусных баллов к заказу.".into(),
        cart_open: "Открыть корзину".into(),
        // Loop #21
        referral_friend_joined: "🎉 {name} присоединился по вашей ссылке!".into(),
        // garden_friend_watered_legacy left on 2026-09-26 (see the struct).
        referral_friend_ordered: "🎉 {name} сделал первый заказ! Вам начислено {bonus} бонусных баллов.".into(),
        referral_invite_progress_hint: "Приглашайте больше друзей — получайте бонусы за их активность.".into(),
        referral_milestone_bonus: "🏆 Поздравляем! Вы достигли рубежа {milestone} друзей и получили {bonus} бонусных баллов от @{bot}.".into(),
    }
}

fn en() -> Locale {
    Locale {
        code: "en".into(),
        flag: "🇬🇧".into(),
        name: "English".into(),
        welcome: "Welcome to TurboBaby! 🏍️".into(),
        start_description: "Motorbike & scooter rental in Phuket".into(),
        open_menu: "Open catalog".into(),
        menu: "🏍 Menu".into(),
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
        joke_prompt: "Tell a short funny joke about motorbikes or road trips.".into(),
        joke_fail_fallback: "😅 Couldn't think of a joke, try again!".into(),
        more_joke: "More jokes".into(),
        fact_thinking: "🧠 Looking for an interesting fact...".into(),
        fact_prompt: "Tell one interesting fact about Phuket or Thailand.".into(),
        fact_fail_fallback: "😅 Couldn't find a fact, try again!".into(),
        interesting_fact: "More facts".into(),
        // strain_of_day, prev_strain and next_strain left on 2026-09-25 with
        // the retired carousel page (the struct says why); these comment lines
        // keep every src/locales.rs line a contract cites where it was.
        add_to_cart: "🛒 Add to cart".into(),
        order_new_header: "New Order".into(),
        order_items: "Items".into(),
        order_items_empty: "(empty)".into(),
        // order_grams left the same day (see the struct).
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
        help_commands: "/start — start\n/menu — menu\n/joke — joke\n/fact — fact\n/lang — language"
            .into(),
        opening_sets: "Opening sets...".into(),
        welcome_feature1: "Catalog: from NMAX 155 to X-ADV 750".into(),
        welcome_feature2: "Daily and monthly rates".into(),
        welcome_feature3: "Delivery across Phuket".into(),
        welcome_feature4: "Clear deposits and terms".into(),
        welcome_feature5: "Book right in the app".into(),
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
        referrals_open_app: "Open invites".into(),
        cart_abandonment_reminder: "🛒 You didn't finish your order. Your items are still in the cart — come back and grab them with one tap.".into(),
        cart_abandonment_reminder_v2: "👀 Something nice is still waiting in your cart. Come back — checkout takes under a minute.".into(),
        cart_second_nudge: "⏰ Your items are still waiting. Complete your order today and we'll add a few bonus points.".into(),
        cart_open: "Open cart".into(),
        // Loop #21
        referral_friend_joined: "🎉 {name} joined via your link!".into(),
        // garden_friend_watered_legacy left on 2026-09-26 (see the struct).
        referral_friend_ordered: "🎉 {name} placed their first order! You earned {bonus} bonus points.".into(),
        referral_invite_progress_hint: "Invite more friends — earn bonuses for their activity.".into(),
        referral_milestone_bonus: "🏆 Congrats! You hit the {milestone} friends milestone and received {bonus} bonus points from @{bot}.".into(),
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
