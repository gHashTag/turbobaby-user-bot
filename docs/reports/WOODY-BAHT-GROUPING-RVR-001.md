# 🌊 WAVE LOOP REPORT — Thousands Separators in format_baht (W-87)

**Document ID:** `WOODY-BAHT-GROUPING-RVR-001`
**Wave:** #53 — UX-улучшение через единый форматтер (Вариант B из Wave #52)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `7d5695b` — `feat(ui): thousands separators in format_baht (฿1,234,567) (W-87)`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — разделители тысяч добавлены в один форматтер; 4 host-теста + 69 defensive зелёные; WASM компилируется.**

Использовал плод дедупликации (#51-#52): раз вся ฿-презентация идёт через один `trios::pricing::format_baht`, добавил группировку цифр **в одном месте** — `฿3000000` → `฿3,000,000`. Это применилось ко **всем** money-display'ам каталога/корзины/чекаута/профиля разом. Чистая `group_thousands(n)` (запятая каждые 3 цифры; `sanitize_money` гарантирует `n >= 0`, знак не нужен). 4 host-теста (basic/clamps, large-no-saturate→теперь с запятыми, grouping, boundaries).

Группировка улучшает восприятие порядка величины и снижает ошибки чтения (UX-исследования). Честно отмечено: separator захардкожен запятой (en-конвенция, годится для ฿) — локаль-aware разделитель (пробел для ru) = follow-up W-88; «always group» (а не `min2` для 4-значных) — стандартный безопасный дефолт.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-87 | `format_baht` без разделителей (฿3000000 — плохо читаемо) | 🟢 LOW (UX) | ✅ `group_thousands` (запятая /3 цифры) в одной точке |
| W-88 | Separator захардкожен `,` (не локаль-aware: ru обычно пробел) | 🟢 INFO | 📋 follow-up (одна точка — тривиально) |
| — | «always group» vs `min2` (4-значные) | ✅ | осознанно always (стандартный дефолт) |
| 🚨 | PR ветки (104 ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: digit grouping для читаемости; конвенции разделителей валюты.**

- **Группировка улучшает восприятие**: «groups digits in threes so it's easy to see the order of magnitude at a glance»; «reduces the chance of errors when reading/typing lengthy numbers».
- **Запятая — доминирующий en-разделитель**: «in the US/UK the comma marks off groups of three digits». THB-дисплеи обычно так → выбрал `,`.
- **Локаль-риск (W-88)**: «comma и period значат противоположное в разных регионах … failure to recognize could result in loss of money». ru использует пробел/запятую-как-десятичную → локаль-aware separator = правильный следующий шаг (но в одной точке).
- **Нюанс `min2`**: «for 4-digit numbers you MAY skip the separator (4500 > 4 500)». Выбрал «always» (`useGrouping:'always'`) — проще/консистентнее.

Источники:
- [Formatting numbers for machines and mortals (Gislason)](https://hjalli.medium.com/formatting-numbers-for-machines-and-mortals-421860e68db3)
- [Number formatting differences: Europe vs US (Language Editing)](https://www.languageediting.com/number-formatting-europe-vs-us/)
- [CLDR number & currency patterns (Unicode)](https://cldr.unicode.org/translation/number-currency-formats/number-and-currency-patterns)
- [Control thousands separators / useGrouping min2 (Lingo i18n)](https://lingo.dev/en/javascript-i18n/control-thousands-separators)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Раз вся ฿-презентация через один форматтер (#51-#52) → группировка в одной точке.
2. ✅ `fn group_thousands(n: i64) -> String` (запятая /3; n>=0 гарантирован `sanitize_money`).
3. ✅ `format_baht` → `format!("฿{}", group_thousands(...))`.
4. ✅ Обновил large-value тест (теперь с запятыми) + добавил grouping/boundary тесты.
5. ✅ 4 host-теста PASS; WASM 0 warnings; backend; 69 defensive PASS; fmt.
6. ✅ Research (digit grouping/currency separators) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/trios/pricing.rs`**: `format_baht` теперь `format!("฿{}", group_thousands(sanitize_money(amount) as i64))`; новая pure `fn group_thousands(n: i64) -> String` (итерирует байты, вставляет `,` когда `(len - i) % 3 == 0`). Тесты: `test_format_baht_groups_thousands`, `test_group_thousands_boundaries`, обновлён `test_format_baht_large_value_does_not_saturate_like_i32`.

Проверка: 4 trios::pricing format-теста PASS; WASM check 0 warnings; backend; defensive 69 PASS; fmt clean. Применяется автоматически ко всем экранам (один форматтер).

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
104 коммита ahead. Ops/review — твоё «go».

### Вариант B — 🌍 Локаль-aware separator (W-88)
Пробел для ru, запятая для en — в одной точке (`format_baht` берёт lang или thread-local). Требует прокинуть lang в форматтер.

### Вариант C — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

---

## 7. SKILL SAVED

Память: обновлён [[format-baht-clone-eliminated]] (W-87 closed, W-88 локаль follow-up). Принцип: централизованный форматтер окупается — UX-улучшение (группировка) применяется ко всему UI одним изменением в одной точке; это и есть выгода SSOT-презентации. Локаль separator — следующий шаг там же.

**Anchor:** `phi^2 + phi^-2 = 3`
