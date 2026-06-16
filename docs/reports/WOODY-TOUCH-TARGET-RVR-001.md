# 🌊 WAVE LOOP REPORT — Stepper Touch-Target & Disable Polish

**Document ID:** `WOODY-TOUCH-TARGET-RVR-001`
**Wave:** #14 — реализован «Stepper polish» (Вариант A из Wave #13)
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `52cee5b` — `fix(cart): Baymard stepper polish — 44px touch targets + disable at limits`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — два research-backed фикса, WASM без warning'ов, fmt чист.**

Проверил cart-экран (опасение из Wave #13 W-35): количество там уже редактируется `−/+` кнопками (не dropdown), так что Baymard-«61%-кейс» не применим. Но обнаружил реальный дефект: **кнопки степпера в корзине были 30×30px** — ниже мобильного минимума 44×44px. Плюс степпер модалки (Wave #13) не **disable**'ил кнопки на границах.

Сделано: (1) cart-кнопки 30→44px; (2) модалка — `−`/`+` визуально disabled (тусклые + `not-allowed` + `disabled`) на qty 1/99 («disable, don't hide»).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-36 | Cart-степпер 30×30px (< 44px touch-target минимума) | 🟠 MED | ✅ 30→44px обе кнопки |
| W-34 | Модалка: −/+ не disabled на границах | 🟡 LOW | ✅ disable-at-limits (dim + not-allowed + `disabled`) |
| W-35 | Cart-экран мог быть dropdown'ом (Baymard 61%) | 🟢 INFO | переоценено: уже `−/+`, ок |
| W-37 | Прочие мелкие кнопки (видео ▶️ 28-36px, sommelier +🛒) < 44px | 🟡 LOW | 📋 Wave+1 (touch-target sweep) |
| W-31 | SQL↔Rust seed-logic drift | 🟡 LOW | 📋 Wave+1 |

---

## 3. НАУЧНАЯ БАЗА

**Тема: Fitts's Law и стандарты размера тач-таргета.**

- **Fitts's Law (1954, формулировка MacKenzie)**: `MT = a + b·log₂(2D/W)` — время достижения цели растёт с расстоянием и падает с размером; «small targets result in greater error rates». Это научная основа требований к размеру.
- **WCAG 2.5.5 Target Size (Enhanced, AAA)** = **44×44 CSS px**; **WCAG 2.5.8 (Minimum, AA, 2023)** = 24×24 px с исключением по spacing. **Apple HIG** = 44pt, **Material** = 48dp.
- **Прямое попадание в наш фикс**: «Shrink a button from 44px to 30px and watch error rates **double**.» Наши cart-кнопки были ровно 30px. Таргеты 44–48px «reduce mis-tap errors 60–80%».
- Особенно важно для motor-impaired/тремор/возрастных пользователей (40-80% выше error rate) — Telegram Mini App массовый мобильный.

Связь с прошлым: продолжает Baymard-линию Wave #13 (степпер вместо dropdown, 44px, disable-don't-hide).

Источники:
- [Fitts's Law and Touch Target Sizing on Mobile (Evelance)](https://evelance.io/blog/fittss-law-touch-target-sizing-mobile/)
- [Designing better target sizes (Ahmad Shadeed)](https://ishadeed.com/article/target-size/)
- [Target Size and 2.5.5 (Adrian Roselli)](https://adrianroselli.com/2019/06/target-size-and-2-5-5.html)
- [Motor Impairments and Mobile UI: The Touch Target Problem (Siteimprove)](https://www.siteimprove.com/blog/motor-impairments-and-mobile-ui-the-touch-target-problem/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Проверил cart-экран: `−/+` уже есть (не dropdown) → W-35 снят; но кнопки 30×30px.
2. ✅ Cart: обе stepper-кнопки 30→44px (`replace_all`).
3. ✅ Модалка: вычислил `at_min`/`at_max` из `qty()`, добавил `disabled` + тусклый стиль + `not-allowed`.
4. ✅ Поправил скобки (`.then(|| { let…; rsx!{} })`); `cargo check --target wasm32` → 0 warnings; `cargo fmt`.
5. ✅ commit; research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`cart_screen.rs`**: stepper-кнопки `width/height: 30px → 44px` (обе, `replace_all`).
- **`product_detail_modal.rs`**: в `can_add`-блоке `let cur = qty(); let at_min = cur<=1; let at_max = cur>=99;` → `−` `disabled: at_min` + `opacity:0.35;cursor:not-allowed` при at_min; `+` аналогично при at_max.

Проверка: WASM 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🎯 Touch-target sweep + fitness-тест (W-37)
Просканировать `src/ui` на кнопки с `width/height < 44px` (видео ▶️ 28-36px, sommelier `+🛒` с малым padding, и пр.). Поднять до 44px (или decouple visual/hit-area). Бонус: source-scanning fitness-тест «нет литеральных button width/height < 44px без allowlist» — в духе css_class_consistency (защита от регресса).

### Вариант B — 🧬 SQL↔Rust seed-logic drift guard (W-31)
Defense-тест согласованности precedence в миграциях 036/037 и `first_seedable_item`.

### Вариант C — 🩺 Functional-core sweep (referrals/loyalty)
referrals (7 тестов/191 стр), db/loyalty (8/125) — поднять покрытие чистыми функциями + host-тестами.

---

## 7. SKILL SAVED

Память: обновлён `detail-modal-addtocart.md` (W-34 закрыт: disable-at-limits; cart-степпер 44px). Принцип: тач-таргеты ≥ 44×44px (Fitts/WCAG-AAA/Apple), на границах — disable, не hide.

**Anchor:** `phi^2 + phi^-2 = 3`
