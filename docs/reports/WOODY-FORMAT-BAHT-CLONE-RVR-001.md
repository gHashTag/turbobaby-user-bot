# 🌊 WAVE LOOP REPORT — Single format_baht: 3 Clones + i32 Narrowing Killed

**Document ID:** `WOODY-FORMAT-BAHT-CLONE-RVR-001`
**Wave:** #51 — UI fresh-area scan (Вариант C из Wave #50) → clone-drift + numeric narrowing
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `c797d5e` — `refactor(ui): single format_baht in trios; kill 3 format_price clones + i32 narrowing`
**Agent:** Claude Opus 4.8 (Wave loop) + Explore-subagent (UI scan)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — 3 клона `format_price` устранены, i32→i64 narrowing исправлен; 2 host-теста + 69 defensive зелёные; WASM компилируется.**

UI-скан (Dioxus) нашёл: `format_price(f64) -> String` — **байт-в-байт клон в 3 экранах** (menu/cart/home), плюс разбросанные инлайн `as i32` price-касты. Все форматируют через `as i32`, что **сужает** i64-домен цены/тотала (`ProductPrice.price`, `calculate_cart_total` — оба i64) и сатурирует тотал >2.1B в `i32::MAX`.

**Честная коррекция агента**: он заявил «UB при f64→i32» — это **неверно** (с Rust 1.45 float→int касты **сатурирующие**, не UB). Реальный дефект — не UB, а numeric narrowing (i64→i32) + clone-drift. Извлёк один pure `trios::pricing::format_baht` (компилится для wasm UI + backend, как остальной `trios::pricing`) на `as i64`; 3 экрана делегируют, accessories зовёт напрямую. 2 host-юнит-теста (clamps + no-i32-saturation).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-84 | `format_price` — 3 идентичных клона (menu/cart/home) → clone-drift | 🟡 LOW (maintainability) | ✅ один `trios::pricing::format_baht` (SSOT) |
| W-85 | Price display через `as i32` сужает i64-домен → сатурация тотала >2.1B | 🟢 LOW (correctness) | ✅ `as i64` (cap ~9.2e18, недостижим) |
| — | Агент: «f64→i32 = UB» | ❌ неверно | Rust 1.45+ сатурирует; скорректировано в отчёте |
| W-86 | Инлайн `as i32` в orders/checkout/profile rsx | 🟢 INFO | 📋 follow-up (rsx-инлайн, осторожный диф) |
| 🚨 | PR ветки (104 ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: DRY / single source of truth / Rule of Three; Rust saturating casts; i32 vs i64 для денег.**

- **DRY / SSOT**: «every piece of knowledge must have a single, unambiguous, authoritative representation»; «copies drift apart over time → bugs». 3 клона = ровно этот риск.
- **Rule of Three** (Fowler/Roberts): «two instances don't require refactoring, but when used **three** times, extract». У меня было ровно 3 → извлечение оправдано (не преждевременно).
- **Rust 1.45 saturating casts**: «out-of-range float→int `as` is a **saturating** conversion (was UB before 1.45)». Подтверждает: не UB, а сатурация → агент ошибся.
- **i32 vs i64 для денег**: «narrowing i64→i32 discards high bits / saturates; use **64-bit** integers for money». Ровно мой `as i32`→`as i64`.

Источники:
- [DRY / single source of truth (Principles Wiki)](http://principles-wiki.net/principles:don_t_repeat_yourself)
- [Rule of three (Wikipedia)](https://en.wikipedia.org/wiki/Rule_of_three_(computer_programming))
- [Rust 1.45 saturating float→int casts (PR #71269)](https://github.com/rust-lang/rust/pull/71269)
- [Storing currency as integers / width matters (Modern Treasury)](https://www.moderntreasury.com/journal/floats-dont-work-for-storing-cents)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ UI-скан → `format_price` клон ×3 + инлайн `as i32`.
2. ✅ Проверил claim агента: f64→i32 не UB (Rust 1.45 сатурирует) → реальный дефект = narrowing + clone.
3. ✅ Подтвердил, что `trios::pricing` компилится для wasm+backend и UI уже его зовёт.
4. ✅ `pub fn format_baht(f64) -> String` (через `sanitize_money`, `as i64`) + 2 теста.
5. ✅ menu/cart/home → делегируют; accessories → зовёт напрямую.
6. ✅ 2 host-теста PASS; WASM 0 warnings; backend; 69 defensive PASS; fmt.
7. ✅ Research (DRY/Rule-of-Three/saturating-casts/money-width) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/trios/pricing.rs`**: `pub fn format_baht(amount: f64) -> String { format!("฿{}", sanitize_money(amount) as i64) }` + тесты `test_format_baht_basic_and_clamps`, `test_format_baht_large_value_does_not_saturate_like_i32`.
- **`src/ui/screens/{menu,cart,home}_screen.rs`**: `format_price` теперь делегирует к `trios::pricing::format_baht`.
- **`src/ui/screens/accessories_screen.rs`**: `price_str = crate::trios::pricing::format_baht(a_price)`.

Проверка: `cargo test … format_baht` → 2 PASS; WASM check → 0 warnings; backend; defensive → 69 PASS; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
104 коммита ahead. Ops/review — твоё «go».

### Вариант B — 🧹 Доезд `as i32` в rsx (W-86)
orders/checkout/profile инлайн `{x as i32}` → `format_baht` (осторожный rsx-диф; orders/profile total-display).

### Вариант C — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

---

## 7. SKILL SAVED

Память: новый `format-baht-clone-eliminated.md` + строка в `MEMORY.md`. Принципы: извлекай SSOT на 3-й идентичной копии (Rule of Three); деньги форматируй/храни в i64, не i32 (narrowing сатурирует/режет high bits); Rust float→int `as` — **сатурирующий** (с 1.45), не UB (не путать). Pure-формат живёт в `trios` (компилится wasm+backend → host-тестируемо).

**Anchor:** `phi^2 + phi^-2 = 3`
