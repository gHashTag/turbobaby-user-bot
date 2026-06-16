# 🌊 WAVE LOOP REPORT — CRITICAL_COLUMNS ↔ source-of-truth guard

**Document ID:** `WOODY-CRITICAL-COLS-SSOT-RVR-001`
**Wave:** #21 — реализован «Авто-вывод/guard CRITICAL_COLUMNS» (Вариант A из Wave #20, item W-48)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `2c3c5f5` — `test(db): guard CRITICAL_COLUMNS against migration/SELECT drift`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — drift-guard добавлен, тест зелёный (проверен negative-тестом), backend+WASM компилируются.**

`CRITICAL_COLUMNS` (список колонок для startup schema self-check, Wave #19) — **рукоподдерживаемый**, т.е. второе представление знания, которое уже выражено в (1) миграциях и (2) SELECT'ах каталога. Риск shotgun-surgery: список мог бы перечислять колонку, которой нет ни в одной миграции (фантомный алерт), или пропустить колонку, от которой зависит SELECT (self-check её не проверяет).

Поскольку полностью *вывести* список нельзя (рантайм-const, не может читать исходники на проде), применён **санкционированный fallback DRY**: consistency-тест, валидирующий рукоподдерживаемый список против источника истины. `critical_columns_match_migrations_and_selects` переиспользует парсеры `schema_drift_tests`: каждая `(table, col)` должна быть (1) объявлена в миграции и (2) реально SELECT'иться из этой таблицы в `catalog.rs`.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-48 | `CRITICAL_COLUMNS` рукоподдерживаемый, мог разойтись с миграциями/SELECT'ами | 🟡 LOW | ✅ Consistency-тест против обоих источников |
| W-50 | Список — рантайм-const; нельзя вывести из исходников на проде (только guard, не derive) | 🟢 INFO | 📋 Wave+1 (build.rs/macro — derive) |
| W-47/W-42 | Admin icon a11y + alt-quality | 🟡 LOW | 📋 Wave+1 |
| 🚨 | #18-#20 фиксы — нужен деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: DRY / Single Source of Truth — «derive, don't duplicate», и consistency-test как fallback.**

- **DRY** (Hunt & Thomas, *Pragmatic Programmer*): «Every piece of knowledge must have a single, unambiguous, authoritative representation.» При нескольких представлениях одно — definitive, остальные **генерируются автоматически**.
- **Опасность ручного списка**: «all of them have to be maintained separately while changing at the same time… danger that representations diverge — which is a fault.»
- **Санкционированный fallback (ровно мой кейс)**: «when you must keep a hand-maintained list, write a **consistency test that validates the duplicated list against the authoritative source** — automating the 'they must change at the same time' requirement.» → `critical_columns_match_migrations_and_selects`.
- **Caveat (SPOT, не over-apply)**: абстрагировать по shared meaning, не синтаксису. Здесь meaning общий (это реальные колонки каталога), так что guard оправдан.

Связь: тот же «source-of-truth consistency» класс, что SQL↔Rust seed-drift (#17) и schema-drift (#1/#19/#20).

Источники:
- [DRY (Principles Wiki) — «single source of truth, others generated automatically»](http://principles-wiki.net/principles:don_t_repeat_yourself)
- [DRY — The Pragmatic Programmer (OO Design)](https://www.oodesign.com/dry-dont-repeat-yourself)
- [Single Source of Truth (DEV)](https://dev.to/snowman647/single-source-of-truth-d29)
- [DRY & AI-generated code — derive don't duplicate (Faros)](https://www.faros.ai/blog/ai-generated-code-and-the-dry-principle)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Изучил `schema_drift_tests`: есть `parse_migration_schema()` ({table→cols из миграций}) и `find_select_sites_in(src)` (парсер SELECT'ов).
2. ✅ `CRITICAL_COLUMNS` → `pub(crate)`.
3. ✅ Тест `critical_columns_match_migrations_and_selects` (в `schema_drift_tests`): для каждой (table,col) — (1) в миграции, (2) в SELECT каталога.
4. ✅ Negative-тест: фантомная `sets.bogus_col` → 2 ошибки (нет в миграции + не SELECT'ится); restore → PASS.
5. ✅ backend + `cargo check --target wasm32` → 0 warnings; fmt.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/db/mod.rs`**: `CRITICAL_COLUMNS` → `pub(crate)`.
- **`src/main.rs` `schema_drift_tests`**: новый `critical_columns_match_migrations_and_selects` — `parse_migration_schema()` + `find_select_sites_in(catalog.rs)`; для каждой `(table,col)` из `crate::db::CRITICAL_COLUMNS` проверяет наличие в миграции и в SELECT'е этой таблицы; собирает ошибки.

Проверка: PASS (647 filtered); negative → 2 ошибки; backend + wasm компилируются; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🏗️ Derive CRITICAL_COLUMNS (build.rs/macro, W-50)
Истинный DRY: генерировать список из SELECT'ов на этапе сборки (build.rs парсит catalog.rs → пишет const), убрав ручную поддержку вовсе. Guard этого Wave станет избыточным (хорошо). Выше риск/сложность build-скрипта.

### Вариант B — ♿ Admin icon a11y + alt-quality (W-47/W-42)
Долейбелить admin icon-кнопки (✏️/👁️/🚫/✕) + эвристика filename-as-alt в img-alt тесте. Завершает a11y-покрытие.

### Вариант C — 🧪 Расширить cause-based мониторинг (определённые причины)
Применить «definite+imminent cause → ticket gauge» к другим точным причинам (отсутствие критичного ENV, недостижимость БД/AI на старте). Только determinate, чтобы не плодить alert-fatigue.

---

## 7. SKILL SAVED

Память: обновлён `api-sets-500-resilience.md` (W-48 закрыт consistency-тестом). Принцип: рукоподдерживаемый список = второе представление знания; если нельзя вывести — обязателен consistency-тест против источника истины (санкционированный DRY-fallback).

**Anchor:** `phi^2 + phi^-2 = 3`
