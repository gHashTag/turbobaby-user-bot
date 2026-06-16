# 🌊 WAVE LOOP REPORT — Touch-Target Sweep + Fitness Guard

**Document ID:** `WOODY-TOUCH-TARGET-GUARD-RVR-001`
**Wave:** #15 — реализован «Touch-target sweep + fitness-тест» (Вариант A из Wave #14)
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `c644126` — `fix(ui): enlarge clickable controls to 44px + touch-target fitness test`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — все интерактивные контролы ≥44px, добавлен regression-guard, WASM без warning'ов.**

Системно закрыл sub-44px тач-таргеты (Wave #14 чинил только cart-степпер). Найдено 5 кликабельных контролов <44px (фильтр: `cursor:pointer` + явный `width/height` <44): видео-`▶️` (tea 28, sets 32, accessories 36), модальный `✕` (32) и admin-чекбокс (18). Первые 4 подняты до 44px; native-чекбокс оставлен (allowlist — попадает под WCAG 2.5.8 spacing exception).

Добавлен **source-scanning fitness-тест** `clickable_targets_meet_min_size` (main.rs, host): любой `style:`-литерал с `cursor:pointer` и явным `width/height` <44px фейлит сборку (с ALLOWLIST). Проверен negative-тестом. Это shift-left guard от регресса — в духе `css_class_consistency_tests`.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-37 | Кликабельные контролы <44px (видео ▶️, модальный ✕) | 🟠 MED | ✅ 4 контрола → 44px |
| W-38 | Нет защиты от регресса размера тач-таргетов | 🟠 MED | ✅ fitness-тест `clickable_targets_meet_min_size` |
| W-39 | Padding-sized кнопки (`padding:6px 10px` → ~28px высота) тест НЕ ловит | 🟡 LOW | 📋 Wave+1 (effective-size) |
| W-31 | SQL↔Rust seed-logic drift | 🟡 LOW | 📋 Wave+1 |

---

## 3. НАУЧНАЯ БАЗА

**Тема: shift-left автоматизированная проверка доступности (и её границы).**

- **Shift-left a11y**: ловить нарушения в разработке, а не в проде — «issues found in production cost 10-100x more to fix». Per-PR required-check «prevents regressions from being merged». Мой тест бежит в lefthook `defensive-tests` на каждый коммит.
- **Туч-таргет на границе автоматизации**: «measurable dimensions can sometimes be checked programmatically, but spacing exceptions and actual usability … fall into the manual-testing bucket». Я честно покрываю **только измеримую часть** (явный width/height), не spacing/padding/реальное взаимодействие.
- **Граница автоматизации**: «no single automated tool catches more than ~30-40% of WCAG … the rest require manual testing». Тест — baseline + regression guard, не замена ручного аудита.
- База размера (Wave #14): Fitts's Law, WCAG 2.5.5 (AAA, 44px), Apple HIG 44pt.

Связь с прошлым: тот же «guard the intentional contract» класс, что migration-wiring (#1), i18n-completeness (#2), bilingual-wiring (#8) — теперь для тач-таргетов.

Источники:
- [Building Accessibility Checks Into CI/CD Workflows (TestParty)](https://testparty.ai/blog/building-accessibility-checks-into-modern-ci-cd-workflows)
- [Accessibility Testing in CI/CD: Integration Guide (TestParty)](https://testparty.ai/blog/accessibility-testing-cicd)
- [Accessibility Testing in Continuous Integration (Perficient)](https://blogs.perficient.com/2025/07/25/the-intersection-of-agile-and-accessibility-accessibility-testing-in-continuous-integration/)
- [Axe Core: catches ~30-50% of WCAG — baseline & regression guard](https://www.blackcoffee.app/blog/axe-core-for-accessibility-testing-1753743810030)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Скан `src/ui`: `cursor:pointer` + width/height <44 → 5 контролов (отсеяв progress-bars/thumbnails/spinners/qty-span/checkbox).
2. ✅ Поднял 4 кнопки (tea/sets/accessories видео + modal ✕) до 44px (+ glyph font 16-18).
3. ✅ Чекбокс 18px → allowlist (native form control, 2.5.8 spacing exception).
4. ✅ Добавил `touch_target_tests::clickable_targets_meet_min_size` (парсит `style:`-литералы, exact-key width/height, ALLOWLIST).
5. ✅ Negative-тест (30px → FAIL с именем файла); restore → PASS; WASM 0 warnings; fmt.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **Fix**: `tea_screen.rs`/`sets_screen.rs`/`accessories_screen.rs` видео-кнопки → `width/height:44px`; `product_detail_modal.rs` ✕ → 44px.
- **Guard** (`src/main.rs::touch_target_tests`): walk `src/ui/**/*.rs`; для каждого `style: "..."` нормализует (strip whitespace); если есть `cursor:pointer` и `width|height` (exact key, игнор `min-`/`max-`) с px<44 — violation; `ALLOWLIST` (admin-чекбокс). Runs host (defensive-tests hook).

Проверка: `clickable_targets_meet_min_size` PASS; negative-тест ловит 30px; `cargo check --target wasm32` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 📏 Effective-size / alt-text a11y guards (W-39)
(1) Расширить тест на padding-sized кнопки (оценка высоты из `padding` + `font-size`), либо (2) новый guard: каждый `<img>` под `src/ui` имеет непустой `alt` (другой измеримый WCAG-критерий, baseline-автоматизируемый). Углубляет shift-left a11y.

### Вариант B — 🧬 SQL↔Rust seed-logic drift guard (W-31)
Defense-тест согласованности precedence в миграциях 036/037 и `first_seedable_item`.

### Вариант C — 🩺 Functional-core sweep (referrals/loyalty)
referrals (7 тестов/191 стр), db/loyalty (8/125) — поднять покрытие чистыми функциями + host-тестами.

---

## 7. SKILL SAVED

Память: новый `touch-target-guard.md` + строка в `MEMORY.md`. Принцип: тач-таргеты ≥44px + source-scanning guard на измеримую часть; честно — автоматизация покрывает ~30-50% WCAG, остальное — ручной аудит.

**Anchor:** `phi^2 + phi^-2 = 3`
