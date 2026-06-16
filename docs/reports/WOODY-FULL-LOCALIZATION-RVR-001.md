# 🌊 WAVE LOOP REPORT — Full Catalog Field Localization

**Document ID:** `WOODY-FULL-LOCALIZATION-RVR-001`
**Wave:** #6 — реализован «Full field localization» (Вариант A из Wave #5)
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `170a4e7` — `feat(catalog): localize name, effect, flavor, category for non-Russian users`
**Agent:** Claude Opus 4.8 (Wave loop, 15-min cadence)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — фича готова, все хуки зелёные (check-wasm, defensive-tests, fmt, conventional-commits).**

Wave #5 сделал двуязычными только **описания**. Этот Wave расширил `lang::localized()` на остальные видимые поля каталога: **название, эффект, флейвор (сорта), категорию (аксессуары), подкатегорию (чай)**. Теперь весь каталог переключается на английский для не-русских пользователей, а не только описание.

**Ключевое архитектурное решение:** локализованное **название** также используется в `CartItem`, чтобы корзина совпадала с тем, что пользователь видел/нажал. При этом `id` остаётся каноническим ключом — серверная price-authority проверяет заказ по `id`, а `name` в заказе — лишь презентационный снимок. Это классическое разделение **surrogate key (id) vs display label (name)**.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-17 | name/effect/flavor/category показывались по-русски EN-пользователям (всё `_en` уже в API) | 🟠 MED | ✅ Закрыт расширением `localized()` |
| W-18 | Риск: локализованное имя в корзине → имя заказа зависит от языка | 🟢 INFO | ✅ Безопасно: `id` каноничен (server price-authority по id), `name` — снимок |
| W-16 | Sets всё ещё ru-only (нет `_en` колонок/эмита) | 🟡 LOW | 📋 Wave+1 (нужна миграция) |
| W-15 | 4 локальных `Api*` DTO дублируют `types.rs` (теперь с 3-4 `_en` полями каждый) | 🟠 MED↑ | 📋 Wave+1 — дублирование выросло, дозрело до Rule of Three |

---

## 3. НАУЧНАЯ БАЗА

**Тема: surrogate key vs display label / разделение presentation и domain.**

Решение «локализуем `name` для показа и корзины, но `id` остаётся ключом» опирается на два устоявшихся принципа:

- **Surrogate key** (БД): стабильный идентификатор не выводится из прикладных данных, в отличие от natural/business key. «Never overload a user-facing display name with the job of being a stable identifier.» У нас `id` — surrogate; `name` — изменяемый, локализуемый бизнес-атрибут.
- **i18n presentation/domain separation**: человекочитаемый текст — забота слоя представления, варьируется по языку и может меняться без поломки ссылок. Идентификатор в коде/хранилище неизменен.

Практический вывод для заказов: `name` в `CartItem`/заказе — презентационный снимок (что клиент видел), `id` — то, по чему сервер считает цену и выполняет fulfillment. Локализация `name` не ломает целостность, потому что идентичность несёт `id`, а не строка.

Связь с прошлыми Wave: #5 дал механику fallback (`lang → en → ru`); этот Wave применил её ко всем полям и формализовал границу «что локализуемо (display) vs что каноническое (id)».

Источники:
- [Surrogate key — Wikipedia](https://en.wikipedia.org/wiki/Surrogate_key)
- [The Art of the Key: i18n Key Naming (Locize)](https://www.locize.com/blog/guide-to-i18n-key-naming/)
- [Localization best practices for developers (Mozilla L10n)](https://mozilla-l10n.github.io/documentation/localization/dev_best_practices.html)
- [Localization, i18n — SAP CAP (capire)](https://cap.cloud.sap/docs/guides/uis/i18n)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Подтверждено: GET-ответы эмитят `name_en`/`effect_en`/`flavor_profile_en` (strains), `name_en`/`category_en` (accessories), `name_en`/`subcategory_en` (tea). Проверены `accessory_row`/`tea_product_row`/`Strain`.
2. ✅ Проверено: `name` в заказе косметичен (server price-authority по `id`) → локализация имени в корзине безопасна.
3. ✅ Добавлены `_en` поля в 3 локальных DTO (serde-default).
4. ✅ Применён `localized()`: name (display+modal+cart), effect, flavor, category/subcategory (display-текст; ru-ключ сохранён для emoji/style).
5. ✅ `cargo check --target wasm32` (поймал и пофикшен borrow-after-move в accessories: имя в модалке считается инлайн от `a`, а не из перемещённого в cart-замыкание `a_name`); `cargo fmt`; commit.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **Menu** (`ApiStrain` +`name_en`/`effect_en`/`flavor_profile_en`): `name_disp`/`effect_str`/`flavor_str` через `localized()`; `name_disp` → заголовок, alt, `ProductDetailModal.name`, `CartItem.name`.
- **Accessories** (`+name_en`/`category_en`): `a_name` локализован → заголовок/корзина; `cat_disp` → бейдж (а `cat` остаётся ru для `category_emoji`/`category_badge_style`); имя в модалке считается инлайн от `a` (избежать borrow-after-move в cart-замыкании).
- **Tea** (`+name_en`/`subcategory_en`): `t_name` локализован → заголовок/корзина; `sub_disp` → подкатегория; screen-level модалка локализует `tea.name`/`tea.subcategory`.

Не трогали: бейдж типа сорта (`category` = Sativa/Indica/Hybrid, латиница), наборы. Проверка: `cargo check --target wasm32-unknown-unknown` → clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🧱 Unify Api* DTOs (W-15, дозрело)
4 локальных `Api*` теперь несут по 3-4 `_en` поля каждый — дублирование выросло. Свести к единым DTO в `src/ui/api/types.rs` (+ перенести туда `localized`-обёртки/геттеры вида `fn display_name(&self) -> String`). Чистая Rule-of-Three итерация; убирает риск рассинхрона `_en`-полей между экранами.

### Вариант B — 🗂️ Sets bilingual + schema (W-16)
Миграция `sets` (+`name_en`/`description_en`), расширить `get_sets` SQL + `set_row`, фронт через `localized()`. Закрывает последний каталог. Активирует schema-drift-defense (Wave #1) как страховку.

### Вариант C — 🧪 i18n display fitness test
Тест-«fitness function», проверяющий что каждое отображаемое поле каталога, у которого в API есть `_en`-пара, действительно проходит через `localized()` (а не хардкодит `.name`/`.description`). Защита от регресса локализации — в духе Wave #1–#2 (защита намеренного контракта).

---

## 7. SKILL SAVED

Память: обновлён `bilingual-content-fallback.md` (поля name/effect/flavor/category/subcategory теперь подключены; остались Sets + DTO-унификация). Закреплён принцип **surrogate id vs localized display label** для решений «что локализовать, что оставить каноническим».

**Anchor:** `phi^2 + phi^-2 = 3`
