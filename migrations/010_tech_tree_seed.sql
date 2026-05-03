-- Migration 010: Tech Tree Nodes + Achievements Seed
-- Seeds tech_nodes and achievements from tech-tree-data.ts

CREATE TABLE IF NOT EXISTS tech_nodes (
    id               VARCHAR(50)  PRIMARY KEY,
    name             VARCHAR(100) NOT NULL,
    description      TEXT         NOT NULL DEFAULT '',
    category         VARCHAR(50)  NOT NULL DEFAULT '',
    icon             VARCHAR(10)  NOT NULL DEFAULT '',
    status           VARCHAR(20)  NOT NULL DEFAULT 'locked',
    xp_required      INTEGER      NOT NULL DEFAULT 0,
    xp_reward        INTEGER      NOT NULL DEFAULT 0,
    dependencies     TEXT[]       NOT NULL DEFAULT '{}',
    unlocks          TEXT[]       NOT NULL DEFAULT '{}',
    features         TEXT[]       NOT NULL DEFAULT '{}',
    estimated_hours  INTEGER      NOT NULL DEFAULT 0,
    priority         INTEGER      NOT NULL DEFAULT 3
);

CREATE TABLE IF NOT EXISTS achievements (
    id          VARCHAR(50)  PRIMARY KEY,
    name        VARCHAR(100) NOT NULL,
    description TEXT         NOT NULL DEFAULT '',
    icon        VARCHAR(10)  NOT NULL DEFAULT '',
    xp_reward   INTEGER      NOT NULL DEFAULT 0,
    requirement TEXT         NOT NULL DEFAULT '',
    category    VARCHAR(50)  NOT NULL DEFAULT 'general'
);

-- ═══════════════════════════════════════════════════════════════
-- TIER 0: CORE FOUNDATION
-- ═══════════════════════════════════════════════════════════════
INSERT INTO tech_nodes (id, name, description, category, icon, status, xp_required, xp_reward, dependencies, unlocks, features, estimated_hours, priority)
VALUES
    (
        'core-menu',
        'Базовое меню',
        'Каталог сортов с фильтрацией',
        'core',
        '📋',
        'completed',
        0,
        100,
        ARRAY[]::TEXT[],
        ARRAY['core-cart', 'wasm-calculator'],
        ARRAY['strain_catalog', 'category_filter', 'price_display'],
        8,
        1
    ),
    (
        'core-cart',
        'Корзина и заказы',
        'Система корзины и оформления заказов',
        'core',
        '🛒',
        'completed',
        100,
        150,
        ARRAY['core-menu'],
        ARRAY['core-orders', 'commerce-loyalty'],
        ARRAY['cart_management', 'checkout_flow', 'order_history'],
        12,
        1
    ),
    (
        'core-sommelier',
        'Сомелье',
        'Подбор сортов по предпочтениям',
        'core',
        '🍷',
        'completed',
        100,
        200,
        ARRAY['core-menu'],
        ARRAY['ai-sommelier', 'wasm-recommendation'],
        ARRAY['mood_selection', 'time_preference', 'experience_level'],
        6,
        1
    ),
    (
        'core-admin',
        'Админ-панель',
        'Управление товарами и заказами',
        'core',
        '⚙️',
        'completed',
        200,
        250,
        ARRAY['core-cart'],
        ARRAY['analytics-dashboard', 'commerce-inventory'],
        ARRAY['product_crud', 'order_management', 'admin_auth'],
        10,
        1
    ),
    (
        'core-orders',
        'История заказов',
        'Просмотр и отслеживание заказов',
        'core',
        '📦',
        'completed',
        150,
        100,
        ARRAY['core-cart'],
        ARRAY['commerce-tracking', 'analytics-user'],
        ARRAY['order_list', 'order_status', 'order_details'],
        4,
        1
    )
ON CONFLICT (id) DO UPDATE SET
    name            = EXCLUDED.name,
    description     = EXCLUDED.description,
    category        = EXCLUDED.category,
    icon            = EXCLUDED.icon,
    status          = EXCLUDED.status,
    xp_required     = EXCLUDED.xp_required,
    xp_reward       = EXCLUDED.xp_reward,
    dependencies    = EXCLUDED.dependencies,
    unlocks         = EXCLUDED.unlocks,
    features        = EXCLUDED.features,
    estimated_hours = EXCLUDED.estimated_hours,
    priority        = EXCLUDED.priority;

-- ═══════════════════════════════════════════════════════════════
-- TIER 1: WASM MODULES
-- ═══════════════════════════════════════════════════════════════
INSERT INTO tech_nodes (id, name, description, category, icon, status, xp_required, xp_reward, dependencies, unlocks, features, estimated_hours, priority)
VALUES
    (
        'wasm-core',
        'WASM Core',
        'Базовый модуль WebAssembly для высокопроизводительных вычислений',
        'wasm',
        '⚡',
        'available',
        300,
        500,
        ARRAY['core-menu'],
        ARRAY['wasm-calculator', 'wasm-particles', 'wasm-recommendation'],
        ARRAY['wasm_runtime', 'js_bindings', 'memory_management'],
        16,
        2
    ),
    (
        'wasm-calculator',
        'THC/CBD Калькулятор',
        'WASM-калькулятор дозировки и эффектов',
        'wasm',
        '🧮',
        'locked',
        500,
        300,
        ARRAY['wasm-core'],
        ARRAY['experience-dosage'],
        ARRAY['dose_calculation', 'effect_prediction', 'tolerance_adjustment'],
        8,
        2
    ),
    (
        'wasm-particles',
        'Particle Effects',
        'WASM-система частиц для визуальных эффектов',
        'wasm',
        '✨',
        'locked',
        500,
        250,
        ARRAY['wasm-core'],
        ARRAY['experience-animations'],
        ARRAY['smoke_effects', 'glow_particles', 'confetti'],
        12,
        3
    ),
    (
        'wasm-recommendation',
        'Recommendation Engine',
        'WASM-движок рекомендаций на основе ML',
        'wasm',
        '🎯',
        'locked',
        800,
        600,
        ARRAY['wasm-core', 'core-sommelier'],
        ARRAY['ai-sommelier'],
        ARRAY['collaborative_filtering', 'content_based', 'hybrid_model'],
        20,
        2
    ),
    (
        'wasm-charts',
        'WASM Charts',
        'Высокопроизводительные графики на Canvas',
        'wasm',
        '📊',
        'locked',
        600,
        350,
        ARRAY['wasm-core'],
        ARRAY['analytics-dashboard'],
        ARRAY['line_charts', 'bar_charts', 'pie_charts', 'realtime_updates'],
        14,
        3
    ),
    (
        'wasm-image',
        'Image Processing',
        'WASM-обработка изображений сортов',
        'wasm',
        '🖼️',
        'locked',
        700,
        400,
        ARRAY['wasm-core'],
        ARRAY['future-ar'],
        ARRAY['image_filters', 'compression', 'color_analysis'],
        16,
        4
    )
ON CONFLICT (id) DO UPDATE SET
    name            = EXCLUDED.name,
    description     = EXCLUDED.description,
    category        = EXCLUDED.category,
    icon            = EXCLUDED.icon,
    status          = EXCLUDED.status,
    xp_required     = EXCLUDED.xp_required,
    xp_reward       = EXCLUDED.xp_reward,
    dependencies    = EXCLUDED.dependencies,
    unlocks         = EXCLUDED.unlocks,
    features        = EXCLUDED.features,
    estimated_hours = EXCLUDED.estimated_hours,
    priority        = EXCLUDED.priority;

-- ═══════════════════════════════════════════════════════════════
-- TIER 2: AI & MACHINE LEARNING
-- ═══════════════════════════════════════════════════════════════
INSERT INTO tech_nodes (id, name, description, category, icon, status, xp_required, xp_reward, dependencies, unlocks, features, estimated_hours, priority)
VALUES
    (
        'ai-sommelier',
        'AI Сомелье',
        'Умный подбор сортов с ML-моделью',
        'ai',
        '🤖',
        'locked',
        1000,
        800,
        ARRAY['wasm-recommendation', 'core-sommelier'],
        ARRAY['ai-chat', 'ai-prediction'],
        ARRAY['ml_recommendations', 'taste_profile', 'mood_analysis'],
        24,
        2
    ),
    (
        'ai-chat',
        'AI Чат-консультант',
        'Умный чат-бот с пониманием контекста',
        'ai',
        '💬',
        'locked',
        1200,
        700,
        ARRAY['ai-sommelier'],
        ARRAY['future-voice'],
        ARRAY['context_understanding', 'strain_qa', 'order_assistance'],
        20,
        3
    ),
    (
        'ai-prediction',
        'Предиктивная аналитика',
        'Прогнозирование спроса и поведения',
        'ai',
        '🔮',
        'locked',
        1500,
        900,
        ARRAY['ai-sommelier', 'analytics-dashboard'],
        ARRAY['commerce-dynamic-pricing'],
        ARRAY['demand_forecast', 'churn_prediction', 'inventory_optimization'],
        30,
        3
    )
ON CONFLICT (id) DO UPDATE SET
    name            = EXCLUDED.name,
    description     = EXCLUDED.description,
    category        = EXCLUDED.category,
    icon            = EXCLUDED.icon,
    status          = EXCLUDED.status,
    xp_required     = EXCLUDED.xp_required,
    xp_reward       = EXCLUDED.xp_reward,
    dependencies    = EXCLUDED.dependencies,
    unlocks         = EXCLUDED.unlocks,
    features        = EXCLUDED.features,
    estimated_hours = EXCLUDED.estimated_hours,
    priority        = EXCLUDED.priority;

-- ═══════════════════════════════════════════════════════════════
-- TIER 2: COMMERCE & LOYALTY
-- ═══════════════════════════════════════════════════════════════
INSERT INTO tech_nodes (id, name, description, category, icon, status, xp_required, xp_reward, dependencies, unlocks, features, estimated_hours, priority)
VALUES
    (
        'commerce-loyalty',
        'Программа лояльности',
        'XP, уровни и награды для клиентов',
        'commerce',
        '🏆',
        'available',
        400,
        400,
        ARRAY['core-cart'],
        ARRAY['commerce-referral', 'social-achievements'],
        ARRAY['xp_system', 'levels', 'rewards', 'discounts'],
        12,
        2
    ),
    (
        'commerce-referral',
        'Реферальная система',
        'Приглашай друзей - получай бонусы',
        'commerce',
        '🤝',
        'locked',
        600,
        350,
        ARRAY['commerce-loyalty'],
        ARRAY['social-sharing'],
        ARRAY['referral_codes', 'bonus_tracking', 'viral_mechanics'],
        8,
        2
    ),
    (
        'commerce-tracking',
        'Отслеживание доставки',
        'Реалтайм статус доставки',
        'commerce',
        '🛵',
        'locked',
        500,
        300,
        ARRAY['core-orders'],
        ARRAY['experience-notifications'],
        ARRAY['live_tracking', 'eta_calculation', 'driver_info'],
        10,
        2
    ),
    (
        'commerce-inventory',
        'Управление запасами',
        'Автоматизация инвентаря',
        'commerce',
        '📦',
        'locked',
        700,
        450,
        ARRAY['core-admin'],
        ARRAY['commerce-supplier'],
        ARRAY['stock_alerts', 'auto_reorder', 'batch_tracking'],
        14,
        3
    ),
    (
        'commerce-supplier',
        'Интеграция с поставщиками',
        'API для автозаказа у поставщиков',
        'commerce',
        '🏭',
        'locked',
        900,
        500,
        ARRAY['commerce-inventory'],
        ARRAY[]::TEXT[],
        ARRAY['supplier_api', 'auto_ordering', 'price_comparison'],
        20,
        4
    ),
    (
        'commerce-dynamic-pricing',
        'Динамическое ценообразование',
        'Умные цены на основе спроса',
        'commerce',
        '💹',
        'locked',
        1200,
        600,
        ARRAY['ai-prediction'],
        ARRAY[]::TEXT[],
        ARRAY['demand_pricing', 'time_discounts', 'personalized_offers'],
        16,
        4
    )
ON CONFLICT (id) DO UPDATE SET
    name            = EXCLUDED.name,
    description     = EXCLUDED.description,
    category        = EXCLUDED.category,
    icon            = EXCLUDED.icon,
    status          = EXCLUDED.status,
    xp_required     = EXCLUDED.xp_required,
    xp_reward       = EXCLUDED.xp_reward,
    dependencies    = EXCLUDED.dependencies,
    unlocks         = EXCLUDED.unlocks,
    features        = EXCLUDED.features,
    estimated_hours = EXCLUDED.estimated_hours,
    priority        = EXCLUDED.priority;

-- ═══════════════════════════════════════════════════════════════
-- TIER 2: ANALYTICS
-- ═══════════════════════════════════════════════════════════════
INSERT INTO tech_nodes (id, name, description, category, icon, status, xp_required, xp_reward, dependencies, unlocks, features, estimated_hours, priority)
VALUES
    (
        'analytics-dashboard',
        'Аналитика продаж',
        'Дашборд с метриками и графиками',
        'analytics',
        '📈',
        'locked',
        600,
        450,
        ARRAY['core-admin', 'wasm-charts'],
        ARRAY['analytics-user', 'ai-prediction'],
        ARRAY['sales_metrics', 'revenue_charts', 'top_products'],
        16,
        2
    ),
    (
        'analytics-user',
        'Аналитика пользователей',
        'Поведение и сегментация клиентов',
        'analytics',
        '👥',
        'locked',
        700,
        400,
        ARRAY['analytics-dashboard', 'core-orders'],
        ARRAY['analytics-ab'],
        ARRAY['user_segments', 'cohort_analysis', 'ltv_calculation'],
        12,
        3
    ),
    (
        'analytics-ab',
        'A/B Тестирование',
        'Эксперименты и оптимизация',
        'analytics',
        '🧪',
        'locked',
        800,
        350,
        ARRAY['analytics-user'],
        ARRAY['analytics-flags'],
        ARRAY['experiment_setup', 'statistical_analysis', 'auto_winner'],
        14,
        4
    ),
    (
        'analytics-flags',
        'Feature Flags',
        'Управление фичами без деплоя',
        'analytics',
        '🚩',
        'locked',
        500,
        250,
        ARRAY['analytics-ab'],
        ARRAY[]::TEXT[],
        ARRAY['feature_toggles', 'gradual_rollout', 'user_targeting'],
        8,
        4
    )
ON CONFLICT (id) DO UPDATE SET
    name            = EXCLUDED.name,
    description     = EXCLUDED.description,
    category        = EXCLUDED.category,
    icon            = EXCLUDED.icon,
    status          = EXCLUDED.status,
    xp_required     = EXCLUDED.xp_required,
    xp_reward       = EXCLUDED.xp_reward,
    dependencies    = EXCLUDED.dependencies,
    unlocks         = EXCLUDED.unlocks,
    features        = EXCLUDED.features,
    estimated_hours = EXCLUDED.estimated_hours,
    priority        = EXCLUDED.priority;

-- ═══════════════════════════════════════════════════════════════
-- TIER 2: SOCIAL
-- ═══════════════════════════════════════════════════════════════
INSERT INTO tech_nodes (id, name, description, category, icon, status, xp_required, xp_reward, dependencies, unlocks, features, estimated_hours, priority)
VALUES
    (
        'social-achievements',
        'Достижения',
        'Система ачивок и бейджей',
        'social',
        '🎖️',
        'locked',
        500,
        300,
        ARRAY['commerce-loyalty'],
        ARRAY['social-leaderboard'],
        ARRAY['achievement_system', 'badges', 'progress_tracking'],
        10,
        3
    ),
    (
        'social-leaderboard',
        'Лидерборд',
        'Рейтинг топ-покупателей',
        'social',
        '🏅',
        'locked',
        600,
        250,
        ARRAY['social-achievements'],
        ARRAY['social-sharing'],
        ARRAY['weekly_top', 'monthly_top', 'all_time_top'],
        6,
        4
    ),
    (
        'social-sharing',
        'Социальный шеринг',
        'Делись заказами и достижениями',
        'social',
        '📤',
        'locked',
        400,
        200,
        ARRAY['social-leaderboard', 'commerce-referral'],
        ARRAY['social-reviews'],
        ARRAY['share_order', 'share_achievement', 'invite_friends'],
        6,
        4
    ),
    (
        'social-reviews',
        'Отзывы и рейтинги',
        'Система отзывов на сорта',
        'social',
        '⭐',
        'locked',
        500,
        350,
        ARRAY['social-sharing'],
        ARRAY[]::TEXT[],
        ARRAY['strain_reviews', 'photo_reviews', 'helpful_votes'],
        12,
        3
    )
ON CONFLICT (id) DO UPDATE SET
    name            = EXCLUDED.name,
    description     = EXCLUDED.description,
    category        = EXCLUDED.category,
    icon            = EXCLUDED.icon,
    status          = EXCLUDED.status,
    xp_required     = EXCLUDED.xp_required,
    xp_reward       = EXCLUDED.xp_reward,
    dependencies    = EXCLUDED.dependencies,
    unlocks         = EXCLUDED.unlocks,
    features        = EXCLUDED.features,
    estimated_hours = EXCLUDED.estimated_hours,
    priority        = EXCLUDED.priority;

-- ═══════════════════════════════════════════════════════════════
-- TIER 2: EXPERIENCE
-- ═══════════════════════════════════════════════════════════════
INSERT INTO tech_nodes (id, name, description, category, icon, status, xp_required, xp_reward, dependencies, unlocks, features, estimated_hours, priority)
VALUES
    (
        'experience-dosage',
        'Гид по дозировке',
        'Персональные рекомендации по дозе',
        'experience',
        '💊',
        'locked',
        600,
        350,
        ARRAY['wasm-calculator'],
        ARRAY['experience-journal'],
        ARRAY['dose_guide', 'tolerance_tracker', 'effect_log'],
        10,
        3
    ),
    (
        'experience-journal',
        'Дневник сессий',
        'Записывай и анализируй опыт',
        'experience',
        '📔',
        'locked',
        500,
        300,
        ARRAY['experience-dosage'],
        ARRAY[]::TEXT[],
        ARRAY['session_log', 'mood_tracking', 'strain_notes'],
        8,
        4
    ),
    (
        'experience-animations',
        'Анимации и эффекты',
        'Красивые визуальные эффекты',
        'experience',
        '🎨',
        'locked',
        400,
        200,
        ARRAY['wasm-particles'],
        ARRAY[]::TEXT[],
        ARRAY['page_transitions', 'micro_animations', 'particle_bg'],
        8,
        4
    ),
    (
        'experience-notifications',
        'Push-уведомления',
        'Уведомления о заказах и акциях',
        'experience',
        '🔔',
        'locked',
        400,
        250,
        ARRAY['commerce-tracking'],
        ARRAY[]::TEXT[],
        ARRAY['order_updates', 'promo_alerts', 'restock_notify'],
        6,
        3
    ),
    (
        'experience-pwa',
        'PWA Offline Mode',
        'Работа без интернета',
        'experience',
        '📱',
        'locked',
        600,
        400,
        ARRAY['core-menu'],
        ARRAY[]::TEXT[],
        ARRAY['offline_catalog', 'cached_orders', 'sync_queue'],
        12,
        3
    ),
    (
        'experience-i18n',
        'Мультиязычность',
        'Поддержка нескольких языков',
        'experience',
        '🌍',
        'locked',
        500,
        300,
        ARRAY['core-menu'],
        ARRAY[]::TEXT[],
        ARRAY['russian', 'english', 'thai', 'chinese'],
        10,
        3
    ),
    (
        'experience-a11y',
        'Доступность',
        'Поддержка людей с ограничениями',
        'experience',
        '♿',
        'locked',
        400,
        300,
        ARRAY['core-menu'],
        ARRAY[]::TEXT[],
        ARRAY['screen_reader', 'keyboard_nav', 'high_contrast'],
        8,
        4
    )
ON CONFLICT (id) DO UPDATE SET
    name            = EXCLUDED.name,
    description     = EXCLUDED.description,
    category        = EXCLUDED.category,
    icon            = EXCLUDED.icon,
    status          = EXCLUDED.status,
    xp_required     = EXCLUDED.xp_required,
    xp_reward       = EXCLUDED.xp_reward,
    dependencies    = EXCLUDED.dependencies,
    unlocks         = EXCLUDED.unlocks,
    features        = EXCLUDED.features,
    estimated_hours = EXCLUDED.estimated_hours,
    priority        = EXCLUDED.priority;

-- ═══════════════════════════════════════════════════════════════
-- TIER 3: FUTURE TECH
-- ═══════════════════════════════════════════════════════════════
INSERT INTO tech_nodes (id, name, description, category, icon, status, xp_required, xp_reward, dependencies, unlocks, features, estimated_hours, priority)
VALUES
    (
        'future-ar',
        'AR Превью',
        'Смотри сорта в дополненной реальности',
        'future',
        '👓',
        'locked',
        2000,
        1000,
        ARRAY['wasm-image'],
        ARRAY[]::TEXT[],
        ARRAY['ar_strain_view', '3d_models', 'size_comparison'],
        40,
        5
    ),
    (
        'future-voice',
        'Голосовой ассистент',
        'Заказывай голосом',
        'future',
        '🎤',
        'locked',
        1800,
        900,
        ARRAY['ai-chat'],
        ARRAY[]::TEXT[],
        ARRAY['voice_commands', 'voice_search', 'voice_checkout'],
        30,
        5
    ),
    (
        'future-blockchain',
        'Blockchain Loyalty',
        'NFT-бейджи и токены лояльности',
        'future',
        '🔗',
        'locked',
        2500,
        1200,
        ARRAY['commerce-loyalty'],
        ARRAY[]::TEXT[],
        ARRAY['nft_badges', 'loyalty_tokens', 'crypto_payments'],
        50,
        5
    )
ON CONFLICT (id) DO UPDATE SET
    name            = EXCLUDED.name,
    description     = EXCLUDED.description,
    category        = EXCLUDED.category,
    icon            = EXCLUDED.icon,
    status          = EXCLUDED.status,
    xp_required     = EXCLUDED.xp_required,
    xp_reward       = EXCLUDED.xp_reward,
    dependencies    = EXCLUDED.dependencies,
    unlocks         = EXCLUDED.unlocks,
    features        = EXCLUDED.features,
    estimated_hours = EXCLUDED.estimated_hours,
    priority        = EXCLUDED.priority;

-- ═══════════════════════════════════════════════════════════════
-- ACHIEVEMENTS SEED
-- ═══════════════════════════════════════════════════════════════
INSERT INTO achievements (id, name, description, icon, xp_reward, requirement, category)
VALUES
    ('first-order',   'Первый заказ',        'Сделай свой первый заказ',    '🎉', 50,  'orders >= 1',            'orders'),
    ('regular',       'Постоянный клиент',   'Сделай 5 заказов',            '🔄', 100, 'orders >= 5',            'orders'),
    ('vip',           'VIP',                 'Сделай 20 заказов',           '👑', 500, 'orders >= 20',           'orders'),
    ('collector',     'Коллекционер',        'Попробуй все сорта',          '🏆', 300, 'unique_strains >= 10',   'exploration'),
    ('sommelier',     'Сомелье',             'Используй сомелье 10 раз',    '🍷', 150, 'sommelier_uses >= 10',   'exploration'),
    ('big-spender',   'Щедрая душа',         'Потрать 10000 ฿',             '💰', 400, 'total_spent >= 10000',   'spending'),
    ('referrer',      'Амбассадор',          'Пригласи 5 друзей',           '🤝', 250, 'referrals >= 5',         'social'),
    ('night-owl',     'Ночная сова',         'Сделай заказ после полуночи', '🦉', 75,  'night_order',            'timing'),
    ('early-bird',    'Ранняя пташка',       'Сделай заказ до 8 утра',      '🐦', 75,  'early_order',            'timing'),
    ('set-lover',     'Любитель наборов',    'Закажи 5 разных наборов',     '🎁', 200, 'unique_sets >= 5',       'orders')
ON CONFLICT (id) DO UPDATE SET
    name        = EXCLUDED.name,
    description = EXCLUDED.description,
    icon        = EXCLUDED.icon,
    xp_reward   = EXCLUDED.xp_reward,
    requirement = EXCLUDED.requirement,
    category    = EXCLUDED.category;
