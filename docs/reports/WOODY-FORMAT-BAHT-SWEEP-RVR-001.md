# 🌊 WAVE LOOP REPORT — Complete the Baht-Formatting Sweep (W-86)

**Document ID:** `WOODY-FORMAT-BAHT-SWEEP-RVR-001`
**Wave:** #52 — завершение класса money-display (Вариант B из Wave #51)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `c7a2180` — `refactor(ui): route remaining baht displays through format_baht (W-86)`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — все ฿-money displays идут через единый `format_baht`; WASM компилируется; 69 defensive зелёные.**

Завершил начатый в #51 класс: повторный grep вскрыл **больше** money-сайтов с `as i32`-narrowing, чем казалось (не 4, а ~12). Сконвертировал ВСЕ оставшиеся ฿-money displays на единый `trios::pricing::format_baht` (i64, без сужения): orders total, checkout line+total, tea ×2, sommelier ×2, sets original+discounted, profile spent + remaining-to-unlock + tier-threshold. Не-деньги (quantity, %, THC, cashback, progress) намеренно оставлены. `B{bonus_balance}`-стат сохранил свой отдельный префикс `B` (не `฿`) — оставлен осознанно (смена префикса = UI-семантика).

Теперь вся презентация баата — в одном месте (presentation logic SSOT), что устраняет разброс ad-hoc форматов и открывает путь к будущей локали (разделители тысяч — W-87).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-86 | ~12 разрозненных ฿-money displays через `as i32` (narrowing + ad-hoc формат) | 🟢 LOW (correctness+consistency) | ✅ все → `format_baht` (i64) |
| — | Не-деньги касты (quantity/%/THC/cashback/progress) | ✅ | оставлены (корректны) |
| — | `B{bonus_balance}` (префикс `B`, не ฿) | ✅ | оставлен осознанно (смена = UI) |
| W-87 | `format_baht` без разделителей тысяч (฿3000000 vs ฿3,000,000) / локали | 🟢 INFO | 📋 теперь тривиально (один формат) |
| 🚨 | PR ветки (104 ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: централизация форматирования валюты в одну функцию; устранение ad-hoc форматов.**

- **Ad-hoc формат-строки → несогласованность**: «scattering ad-hoc format strings across your UI causes inconsistency». Ровно разброс `฿{x as i32}` по 8 экранам.
- **Одна функция, presentation logic в одном месте**: «centralize into one function … keeps all presentation logic in a single location». `format_baht` — это место.
- **Будущая локаль — в одном месте (W-87)**: «make the decision once (e.g. narrowSymbol/grouping) rather than each developer reinventing it». Разделители тысяч/локаль теперь добавляются в одной точке.
- **Driven by domain type**: деньги — i64; форматтер берёт f64 и сатурирует в i64 (см. [[format-baht-clone-eliminated]]).

Источники:
- [Using i18next to format currencies (Belzile/DEV)](https://dev.to/sbelzile/using-i18next-to-format-currencies-dates-and-much-more-25np)
- [Number, currency and unit formatting (W3C i18n)](https://www.w3.org/blog/International/2026/03/13/new-article-number-currency-and-unit-formatting)
- [react-i18n-currency-input — outsource to Intl API (Houdini)](https://github.com/houdiniproject/react-i18n-currency-input)
- [Oracle Fusion — centralized currencyPattern helpers](https://docs.oracle.com/cd/E28271_01/fusionapps.1111/e15524/ui_localize.htm)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Повторный grep `฿…as i32` → ~12 money-сайтов (8 экранов).
2. ✅ Классифицировал: money (฿) → конвертировать; не-money (%/THC/quantity/B-bonus) → оставить.
3. ✅ `format!("฿{}", x as i32)` кластер (tea×2/sommelier×2/sets×2/orders) → `format_baht`.
4. ✅ Inline rsx `฿{x as i32}` (checkout×2/profile spent/rem/threshold) → precompute `_str` + `format_baht`.
5. ✅ WASM compile (поймал бы unused/missing vars) + backend; 69 defensive PASS; fmt.
6. ✅ Verify: 0 оставшихся ฿-money `as i32` (только намеренный `B{bonus}`).
7. ✅ Research (centralized currency formatting) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/ui/screens/`**: orders_screen (total), checkout_screen (line+total), tea_screen (×2), sommelier_screen (×2), sets_screen (×2), profile_screen (spent + remaining + threshold) — все → `crate::trios::pricing::format_baht(...)`. Inline rsx-сайты переведены на precompute-`let _str` + `"{_str}"`.

Проверка: `cargo check --target wasm32` → 0 warnings; backend OK; defensive → 69 PASS; fmt clean. Grep подтверждает 0 ฿-money `as i32` (кроме намеренного `B{bonus_balance}`).

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
104 коммита ahead, e2e+UI-консистентность. Ops/review — твоё «go».

### Вариант B — 💱 Thousands-separators в format_baht (W-87)
Теперь тривиально (один формат): `฿3,000,000` вместо `฿3000000` + host-тест. Один файл, чисто.

### Вариант C — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

---

## 7. SKILL SAVED

Память: обновлён [[format-baht-clone-eliminated]] (W-86 закрыт; W-87 grouping — follow-up). Принцип: централизуй форматирование валюты в одну функцию (presentation logic SSOT) — разброс ad-hoc `฿{x as i32}` несогласован и тянет narrowing; единая точка открывает локаль/разделители одним изменением. Классифицируй касты: деньги (฿)→formatter; %/THC/quantity→оставь.

**Anchor:** `phi^2 + phi^-2 = 3`
