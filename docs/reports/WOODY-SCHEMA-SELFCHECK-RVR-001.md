# 🌊 WAVE LOOP REPORT — Startup Schema Self-Check (root-cause observability)

**Document ID:** `WOODY-SCHEMA-SELFCHECK-RVR-001`
**Wave:** #19 — реализован «Root-cause /api/sets: prod migration introspection» (Вариант A из Wave #18)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `f36da9c` — `feat(db): startup schema self-check names missing prod columns`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — startup self-check добавлен, 4 host-теста зелёные, backend+WASM компилируются.**

Прямое продолжение прод-инцидента (#18): `GET /api/sets` 500'ил дважды, и настоящая причина — **состояние prod-БД** (неприменённая миграция) — была **невидима**. `schema_drift_tests` доказывает, что код↔миграции согласованы, но **не видит, применила ли prod их реально**.

Добавлен **startup schema self-check**: после `run_migrations` приложение запрашивает живой `information_schema` и логирует **по имени** любую ожидаемую колонку (`CRITICAL_COLUMNS`: позднемиграционные `image_url`/`video_url`/`name_en`/`description_en` на set-таблицах), которой в БД нет. Следующий «prod отстал по миграциям» станет громким **named startup ERROR**, а не загадочным 500. Non-fatal (логирует, не валит старт — runtime уже защищён graceful degradation из #18).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-46 | Отставание prod-БД по миграциям невидимо до первого 500 | 🟠 MED | ✅ Startup self-check логирует missing колонки по имени |
| W-48 | `CRITICAL_COLUMNS` — рукоподдерживаемый список (может разойтись с SELECT'ами) | 🟡 LOW | 📋 Wave+1 (авто-вывод из SELECT'ов) |
| W-49 | Startup-warning только в логах; нет алерта (как 5xx) | 🟡 LOW | 📋 Wave+1 (Slack/alert) |
| 🚨 | /api/sets всё ещё нужно задеплоить #18-фикс + проверить prod | 🔴 | ⚠️ см. §6 — действие на стороне ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: schema drift — fail-fast at startup + observability (make invisible visible).**

- «Schema drift is dangerous because it's **invisible until runtime** … queries may fail, application logic may break.» Ровно наш кейс: prod-БД разошлась с ожиданиями кода.
- **Layered defence**: (1) shift-left в CI — мой `schema_drift_tests` (код↔миграции); (2) **fail-fast/verify at startup** — этот self-check (код↔живая БД); (3) observability/alerting — turn silent drift into visible signals.
- **Severity-выбор**: Atlas рекомендует начинать с `on_error = CONTINUE` (не FAIL) на существующих окружениях. Я выбрал **non-fatal log** (не abort): жёсткий abort уронил бы всё приложение, что хуже, чем деградировавший /api/sets (#18). Логирование = правильный уровень.

Связь: дополняет [[schema-drift-defense]] (CI-слой) и [[migration-wiring-defense]] рантайм-слоем.

Источники:
- [Detect and Prevent Database Schema Drift (Liquibase)](https://www.liquibase.com/blog/database-drift)
- [Schema Drift Detection (Atlas Docs) — on_error CONTINUE→FAIL rollout](https://atlasgo.io/monitoring/drift-detection)
- [Detecting Missing Migrations in EF Core — fail-fast at startup](https://medium.com/@adrianbailador/detecting-missing-migrations-in-ef-core-a-guide-for-net-developers-5de35ac335e8)
- [Schema Drift Detection in Spring Boot Migration Pipelines](https://medium.com/@AlexanderObregon/schema-drift-detection-in-spring-boot-migration-pipelines-8568b342f6ab)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Сформулировал инвариант: prod-БД должна иметь позднемиграционные колонки, которые SELECT'ит каталог.
2. ✅ Pure `missing_columns(expected, present) -> Vec<String>` + `CRITICAL_COLUMNS`.
3. ✅ `Database::missing_critical_columns()` — запрос `information_schema.columns` для set-таблиц; non-fatal (query-ошибка → пусто).
4. ✅ Wire в startup (`main.rs`) после `run_migrations`: пусто → обычный лог; иначе громкий `ERROR` со списком.
5. ✅ 4 host-теста на `missing_columns` (missing/all-present/table-absent/const-sanity); backend+WASM компилируются; fmt.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/db/mod.rs`**: `CRITICAL_COLUMNS` (set-таблицы × позднемиграционные колонки), pure `missing_columns`, async `Database::missing_critical_columns()` (information_schema → present-set → diff). `#[cfg(test)] mod schema_self_check_tests` (4 теста).
- **`src/main.rs`**: после `run_migrations` — `missing_critical_columns().await`; при непустом — `tracing::error!("🚨 SCHEMA SELF-CHECK: live DB is missing …: {missing}")`.

Проверка: `cargo test … schema_self_check` → 4 passed; backend + `cargo check --target wasm32` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

> ⚠️ Напоминание ops: #18-фикс (`ce0648e`, graceful degradation) и #19 self-check на ветке `fix/profile-routing` — нужно cherry-pick в `main` + деплой, чтобы дошло до prod. На следующем рестарте self-check назовёт недостающие колонки (если есть).

### Вариант A — 📡 Alert на schema-warning (закрыть observability-петлю, W-49)
Завести startup-warning в тот же канал, что и 5xx-алерты (Slack/monitoring), чтобы «prod отстал по миграциям» поднимал human-алерт, а не тонул в логах. Превращает наблюдаемость в proactive-сигнал.

### Вариант B — 🔁 Авто-вывод CRITICAL_COLUMNS из SELECT'ов (W-48)
Сейчас список рукоподдерживаемый (shotgun-surgery риск). Переиспользовать парсер `schema_drift_tests` (он уже извлекает {table→columns} из SELECT'ов) для генерации ожидаемого набора set-таблиц — один источник истины.

### Вариант C — ♿ Admin icon-button a11y + alt-quality (W-47/W-42)
Долейбелить admin icon-кнопки (✏️/👁️/🚫/✕) + эвристика filename-as-alt в img-alt тесте.

---

## 7. SKILL SAVED

Память: обновлён `api-sets-500-resilience.md` (добавлен startup self-check как root-cause observability-слой). Принцип: schema drift невидим до рантайма — слои CI (`schema_drift_tests`) + startup-verify (`missing_critical_columns`) + alert; severity non-fatal, т.к. runtime защищён degradation.

**Anchor:** `phi^2 + phi^-2 = 3`
