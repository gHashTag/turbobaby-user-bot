# 🌊 WAVE LOOP REPORT — Quantity Stepper in Detail Modal

**Document ID:** `WOODY-QTY-STEPPER-RVR-001`
**Wave:** #13 — реализован «Quantity selector» (Вариант C из Wave #12)
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `8671440` — `feat(catalog): quantity stepper in the product detail modal`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — степпер добавлен во всех 4 каталогах, WASM без warning'ов, fitness-тест зелёный.**

Перед выбором подтвердил, что денежный путь и бизнес-логика **уже хорошо покрыты** (api/orders.rs — чистые `check_full_subtotal`/`validate_create_order`/`is_valid_idempotency_key` + 47 тестов; trios quest/store/garden/pricing — 17-21 теста каждый). «Functional-core sweep» не нашёл бы низковисящих плодов. Поэтому выбрал **user-facing** завершение арки детальной модалки (Wave #3 → #11 add-to-cart → #13 количество).

Добавлен `− [n] +` степпер (1..=99) над кнопкой «В корзину»; выбранное количество кладётся в корзину одним нажатием вместо повторных одиночных добавлений. `on_add_to_cart` теперь `EventHandler<u32>`; `Cart::add_item` уже суммирует количество по id, так что компонуется с существующими позициями.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-33 | Нельзя выбрать количество в детальной модалке (только +1 за нажатие) | 🟠 MED | ✅ `− [n] +` степпер 1..=99 (4 каталога) |
| — | Денежный путь / бизнес-логика | — | ✅ Проверено: уже покрыто (47 + десятки тестов) |
| W-34 | Степпер: −/+ не **disabled** на границах (Baymard «disable, don't hide»); нет press-and-hold | 🟡 LOW | 📋 Wave+1 (полировка) |
| W-35 | Cart-экран: как редактируется количество? (если dropdown/поле — это 61%-кейс Baymard) | 🟡 LOW | 📋 Wave+1 (проверить/привести к степперу) |
| W-31 | SQL↔Rust seed-logic drift | 🟡 LOW | 📋 Wave+1 |

---

## 3. НАУЧНАЯ БАЗА

**Тема: дизайн quantity-степпера и его связь с конверсией/AOV.**

Опираюсь на **Baymard Institute** (эталон e-commerce UX research):
- **Кнопки > dropdown/текстовое поле**: «tweaking the quantity field to use buttons … generally resolves the friction», при этом **61% сайтов** используют менее удачные dropdown/поле. → степпер `−/+` — research-backed выбор.
- **Mobile touch targets ≥ 44×44px** с достаточным расстоянием (Telegram Mini App — мобильный). Мои кнопки — ровно 44×44.
- **Steppers хороши для малых диапазонов (1–10)** и «avoid invalid entries» — клампинг 1..=99 исключает 0/отрицательные.
- **AOV**: quantity-selection «lifts average order value … reduces cart abandonment».
- ⚠️ Честно: я **не** делаю disable −/+ на границах (Baymard «disable, don't hide») — сейчас клампинг без визуального disabled; press-and-hold тоже нет. → W-34, полировка.

Связь с прошлым: завершает interaction-cost линию (Wave #11) — теперь «N за раз» вместо N открытий/нажатий.

Источники:
- [Use Buttons (or Buttons + Field) for Cart Quantity — 61% Don't (Baymard)](https://baymard.com/blog/auto-update-users-quantity-changes)
- [Stepper UI Design: best practices & variants (Setproduct)](https://www.setproduct.com/blog/stepper-ui-design)
- [Mastering Quantity Selection in E-commerce UX](https://www.numberanalytics.com/blog/ultimate-guide-quantity-selection-ux-ecommerce)
- [Stepper UI Design (Mobbin glossary)](https://mobbin.com/glossary/stepper)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Разведка покрытия: orders/quest/store/garden/pricing — уже тестированы → sweep низкоценен; выбрал UX-фичу C.
2. ✅ Проверил `CartItem.quantity: u32` + `add_item` суммирует по id → степпер компонуется.
3. ✅ Модалка: `qty` сигнал (1..=99), `− [n] +` степпер; `on_add_to_cart: EventHandler<u32>`.
4. ✅ 4 call-site: `move |q: u32| { … quantity: q … }`.
5. ✅ `cargo check --target wasm32` → 0 warnings; `cargo fmt`; bilingual fitness-тест зелёный; commit.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`product_detail_modal.rs`**: `let mut qty = use_signal(|| 1u32);`; внутри `can_add` — ряд `−` (44×44) / `{qty}` / `+` (44×44), клампинг `saturating_sub(1).max(1)` и `(qty+1).min(99)`; кнопка «В корзину» вызывает `on_add_to_cart.call(qty())` + `on_close`. Тип пропса — `EventHandler<u32>`.
- **menu/accessories/sets/tea**: `on_add_to_cart: move |q: u32| { cart.write().add_item(CartItem { quantity: q, … }) }`.

Проверка: WASM 0 warnings; `every_bilingual_field_is_consumed` зелёный; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🎚️ Stepper polish + cart-screen (W-34/W-35)
Disable −/+ на границах (Baymard «disable, don't hide»), press-and-hold для серий, и проверить cart-экран: если количество там редактируется dropdown'ом/полем — привести к тому же степперу (выход из 61%-кейса). Консистентный quantity-UX по всему флоу.

### Вариант B — 🧬 SQL↔Rust seed-logic drift guard (W-31)
Миграции 036/037 повторяют precedence `first_seedable_item` в SQL → defense-тест согласованности (те же catalog id-ключи/приоритет в SQL и Rust).

### Вариант C — 🩺 Functional-core sweep редких хендлеров
Точечно поискать оставшиеся «толстые» места с инлайн-решениями (напр. referrals — 7 тестов на 191 строку, db/loyalty — 8 на 125) и поднять покрытие чистыми функциями + host-тестами.

---

## 7. SKILL SAVED

Память: обновлён `detail-modal-addtocart.md` (модалка теперь с количеством; `on_add_to_cart: EventHandler<u32>`; Baymard-нюанс про disable-на-границах как долг). Зафиксировано: денежный путь и бизнес-модули уже хорошо покрыты тестами.

**Anchor:** `phi^2 + phi^-2 = 3`
