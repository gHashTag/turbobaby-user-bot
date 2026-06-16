# 🌊 WAVE LOOP REPORT — Bilingual-Field Wiring Defense

**Document ID:** `WOODY-BILINGUAL-WIRING-RVR-001`
**Wave:** #8 — реализован «i18n display fitness test» (Вариант B из Wave #7), user-selected
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `68d97bd` — `test(ui): fitness test — every bilingual catalog field must be consumed`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — fitness-тест добавлен, поймал реальный dead-field, тест зелёный, все хуки прошли.**

Локализация каталога (Wave #5-7) — ручное соответствие: каждое `_en`-поле DTO нужно не забыть провести через `localized()`. Защиты не было, а компилятор слеп: screen-DTO помечены `#[allow(dead_code)]` (часть serde-полей намеренно не читается), что глушит предупреждение об **объявленном, но не используемом** поле. Значит `_en`/`_localized` поле можно получить из API и **никогда не показать** — английский текст молча теряется.

Добавлен source-scanning fitness-тест (`bilingual_field_wiring_tests` в `main.rs`, зеркалит `css_class_consistency_tests`): каждое `*_en`/`*_localized` поле в `src/ui/screens/*.rs` должно упоминаться минимум дважды (объявление + хотя бы одно использование). Тест **сразу поймал** `ApiSet.description_localized` — поле, которое API не эмитит и никто не читал, — оно удалено.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-20 | `ApiSet.description_localized` — мёртвое поле (fetched-but-never-shown), скрыто `#[allow(dead_code)]` | 🟡 LOW | ✅ Удалено + защищено тестом |
| W-22 | Нет защиты от регресса локализации: `_en`-поле легко добавить и забыть провести через `localized()` | 🟠 MED | ✅ Закрыт `every_bilingual_field_is_consumed` |
| W-23 | `#[allow(dead_code)]` на screen-DTO глушит rustc для ВСЕХ полей, не только bilingual | 🟡 LOW | 📋 Wave+1 (точечный allow или дроп) |
| W-15 | 5 локальных `Api*` DTO дублируют `types.rs` | 🟠 MED | 📋 Wave+1 |
| W-21 | Admin `sets`-таблица без `_en` (нативные наборы) | 🟡 LOW | 📋 Wave+1 (нужна миграция) |

---

## 3. НАУЧНАЯ БАЗА

**Тема: dead code как code smell + слепота подавленных линтов.**

Эмпирика подтверждает вред мёртвого кода:
- **Распространённость**: ~15.94% методов «мертвы» в 35 OSS Java-проектах (TSE'18 multi-study); в индустрии 25-30% (Eder et al., Boomsma et al.).
- **Вред для понимания**: контролируемые эксперименты Romano et al. (+3 репликации) — dead code достоверно **ухудшает понимаемость**, а **поддерживаемость — особенно на незнакомом коде**.
- **Скрытые риски**: dead code раздувает метрики (ложная уверенность по покрытию) и расширяет attack surface (устаревшие зависимости).
- **Отложенность**: «smells … cause problems during a later phase of the software's evolution» — разработчики недооценивают и не убирают.

Прямая связь с нашим кейсом: `#[allow(dead_code)]` — подавленный линт; «passing/suppressing linters is not a reliable proxy for the absence of deeper defects». Где компилятор замолчал, ставим **точечную fitness-функцию** на конкретный класс мёртвого кода («bilingual поле получено, но не показано»). Та же категория, что migration-wiring (Wave #1) и i18n-completeness (Wave #2): защита намеренного контракта от дрейфа.

Источники:
- [A Multi-Study Investigation Into Dead Code (TSE 2018, W&M)](https://www.cs.wm.edu/~denys/pubs/TSE'18-DeadCode.pdf)
- [How much does unused code matter for maintenance?](https://www.researchgate.net/publication/254041627_How_much_does_unused_code_matter_for_maintenance)
- [On the diffuseness and impact on maintainability of code smells (EMSE 2018)](https://link.springer.com/article/10.1007/s10664-017-9535-z)
- [The Hidden Cost of Unused and Dead Code (Azul)](https://www.azul.com/blog/the-hidden-cost-of-unused-and-dead-code/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Разведка: grep всех `*_en`/`*_localized` деклараций в `src/ui` → `description_localized` встречается 1 раз (мёртв); admin `_en`-поля используются (построение req).
2. ✅ Нашёл host-прецедент `css_class_consistency_tests` (walk `src/ui/**`), co-located новый тест в `main.rs`.
3. ✅ Реализован `every_bilingual_field_is_consumed` (парсер field-decl + word-boundary counter).
4. ✅ Доказательство: тест упал на `description_localized` → удалил поле → тест зелёный.
5. ✅ `cargo check --target wasm32` + `cargo fmt`, commit (defensive-tests hook гоняет тест на host).
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/main.rs::bilingual_field_wiring_tests::every_bilingual_field_is_consumed`** — walk `src/ui/screens/*.rs`; `bilingual_field()` парсит строки вида `ident_en:`/`ident_localized:` (split по первому `:`, левая часть — голый ident с суффиксом); `count_ident()` (word-boundary) требует ≥2 вхождений. Сообщение об ошибке указывает файл и поле.
- **`src/ui/screens/sets_screen.rs`** — удалён `#[serde(default)] description_localized: Option<HashMap<...>>` (мёртвый).

Проверка: тест 1 passed (632 filtered); `cargo check --target wasm32` clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🧱 Unify Api* DTOs (W-15)
Свести 5 локальных `Api*` к единым DTO в `types.rs` + методы `display_name()`/`display_description()`, инкапсулирующие `localized()`. Убирает дублирование и текущие повторяющиеся вызовы `localized()`; fitness-тест из этого Wave продолжит охранять результат.

### Вариант B — 🧹 Tighten dead-code suppression (W-23)
Убрать blanket `#[allow(dead_code)]` со screen-DTO; для реально-нужных-но-неиспользуемых serde-полей пометить точечно `#[allow(dead_code)]` на поле или `_`-обработать. Возвращает rustc-видимость мёртвого кода (наука §3 — dead code накапливается и вредит).

### Вариант C — 🗂️ Admin native Sets localization (W-21)
Миграция `sets +name_en/description_en` → admin `get_sets`/`set_row`/`create_set`/`update_set` + форма. Закрывает админскую локализацию нативных наборов; активирует migration-wiring + schema-drift defenses (Wave #1).

---

## 7. SKILL SAVED

Память: новый `bilingual-field-wiring-defense.md` + строка в `MEMORY.md`. Закреплён тест и принцип: где `#[allow(dead_code)]` глушит компилятор, ставь точечную fitness-функцию на конкретный класс мёртвого кода.

**Anchor:** `phi^2 + phi^-2 = 3`
