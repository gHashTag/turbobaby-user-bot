-- Migration 009: Loyalty Tiers Seed
-- Creates loyalty_tiers table with all 6 tier configs from commerce-data.ts

CREATE TABLE IF NOT EXISTS loyalty_tiers (
    tier              VARCHAR(20)  PRIMARY KEY,
    name              VARCHAR(50)  NOT NULL,
    min_points        INTEGER      NOT NULL DEFAULT 0,
    discount_percent  INTEGER      NOT NULL DEFAULT 0,
    points_multiplier REAL         NOT NULL DEFAULT 1.0,
    perks             TEXT[]       NOT NULL DEFAULT '{}',
    icon              VARCHAR(10)  NOT NULL DEFAULT '',
    color             VARCHAR(20)  NOT NULL DEFAULT ''
);

INSERT INTO loyalty_tiers (tier, name, min_points, discount_percent, points_multiplier, perks, icon, color)
VALUES
    (
        'bronze',
        'Бронза',
        0,
        0,
        1.0,
        ARRAY['Накопление баллов'],
        '🥉',
        '#cd7f32'
    ),
    (
        'silver',
        'Серебро',
        500,
        5,
        1.2,
        ARRAY['Скидка 5%', 'Ранний доступ к новинкам'],
        '🥈',
        '#c0c0c0'
    ),
    (
        'gold',
        'Золото',
        2000,
        10,
        1.5,
        ARRAY['Скидка 10%', 'Бесплатная доставка', 'Приоритетная поддержка'],
        '🥇',
        '#ffd700'
    ),
    (
        'platinum',
        'Платина',
        5000,
        15,
        2.0,
        ARRAY['Скидка 15%', 'Эксклюзивные сорта', 'Персональный менеджер'],
        '💎',
        '#e5e4e2'
    ),
    (
        'diamond',
        'Бриллиант',
        15000,
        20,
        2.5,
        ARRAY['Скидка 20%', 'VIP мероприятия', 'Подарки на день рождения'],
        '💠',
        '#b9f2ff'
    ),
    (
        'woody',
        'Woody Elite',
        50000,
        25,
        3.0,
        ARRAY['Скидка 25%', 'Именной сорт', 'Личные встречи с Вуди', 'Всё включено'],
        '🪵',
        '#39ff14'
    )
ON CONFLICT (tier) DO UPDATE SET
    name              = EXCLUDED.name,
    min_points        = EXCLUDED.min_points,
    discount_percent  = EXCLUDED.discount_percent,
    points_multiplier = EXCLUDED.points_multiplier,
    perks             = EXCLUDED.perks,
    icon              = EXCLUDED.icon,
    color             = EXCLUDED.color;
