# 🌊 WAVE LOOP REPORT — Garden Water-Count: Fail Loud, Don't Silently Clamp

**Document ID:** `WOODY-GARDEN-FAILLOUD-RVR-001`
**Wave:** #38 — deep-audit `trios/garden.rs` (Вариант A из Wave #37) → реальный дефект на API-границе
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `624442b` — `fix(garden): fail loud on corrupt water_count instead of silent .max(0) clamp`
**Agent:** Claude Opus 4.8 (Wave loop) + Explore-subagent

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — silent-clamp дефект устранён; 6 новых unit-тестов + 69 defensive зелёные; backend+WASM.**

Глубокий аудит game-логики огорода. Time-математика (`calculate_progress`) и state-переходы (double-harvest через `WHERE harvested_at IS NULL AND reward_claimed=false`, double-water через cooldown `WHERE`) **подтверждены безопасными** (всё на `saturating_*`, `.min(100)` на прогрессе). Но на API-границе `water_plant` (`api/garden.rs`) нашёлся реальный дефект: `water_count: i32` читался из БД, валидировалась только **верхняя** граница (`>=13`), затем `.max(0)` **тихо клампил** отрицательный/повреждённый счётчик в 0 → растение сбрасывалось в Seed без ошибки; счётчик >13 маскировался в Final через `unwrap_or`.

Это нарушает философию «fail loud on data drift» (которой следует pure-ядро `Plant::water` через `validate_water_count`). Извлёк pure-функцию `next_water_step(water_count) -> {Advance|AtFinalStage|Corrupt}` в garden-ядро и на `Corrupt` теперь **fail loud (500)** вместо тихой коррекции. Severity LOW (нужен corrupt DB-state, не user-эксплойт), но это data-integrity gap + дублирование валидации, теперь устранённое.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-72 | `water_plant`: `.max(0)` тихо клампил отрицательный `water_count` → сброс в Seed; >13 → Final через `unwrap_or` | 🟡 LOW (data-integrity, не user-exploit) | ✅ pure `next_water_step` + fail-loud на `Corrupt` |
| — | `calculate_progress` time-math (`saturating_*`), double-harvest/water guards, progress `.min(100)` — clean | ✅ | — |
| W-73 | API-граница `water_plant` дублировала логику `Plant::water` сырым SQL | 🟢 INFO | ✅ часть теперь общая (pure `next_water_step`); полное слияние — позже |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: fail fast / fail loud vs тихая коррекция данных; trust-boundary валидация.**

- **«Don't silently correct invalid data — make problems visible immediately»** — ровно про `.max(0)`. Тихий клампинг прячет баг и продлевает feedback-loop.
- **Раньше — лучше**: «checks should be done as early as possible … crashes at a later stage hide potential for corruption beyond that point». Валидируем `water_count` сразу при чтении из БД.
- **Barricade / trust boundary**: «validate all input data — type, length, range of values» на границе (данные «извне» = из БД); внутри — по контракту (pure-ядро доверяет валидным значениям). API-handler — это и есть барьер.
- **Defensive programming ≠ swallowing errors**: «it's the trade-off robustness vs correctness; choose: return an error and stop (fast fail) …». `.max(0)` был paranoid-masking; правильный defensive-выбор — fail-fast на границе.

Источники:
- [Fail Fast, Fail Loud: Defensive Programming (Kittikawin)](https://medium.com/@kittikawin_ball/fail-fast-fail-loud-defensive-programming-in-c-89f88c969dcf)
- [Fail Fast principle (Enterprise Craftsmanship)](https://enterprisecraftsmanship.com/posts/fail-fast-principle/)
- [Defensive Programming: Being Just-Enough Paranoid (SW Reflections)](http://swreflections.blogspot.com/2012/03/defensive-programming-being-just-enough.html)
- [Fail fast or fail safe? (Marcel Pintó)](https://medium.com/@marxallski/fail-fast-or-fail-safe-3b9ff8f68c26)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Deep-audit garden.rs + callers: time-math safe (saturating), state-transitions safe; дефект — silent `.max(0)` на API-границе.
2. ✅ Подтвердил, что pure-ядро `Plant::water` валидирует через `validate_water_count`, а API-путь — нет (сырой SQL, дублирование).
3. ✅ Pure `next_water_step(i32) -> WaterStep {Advance{new_count,stage,completed}|AtFinalStage|Corrupt}` в garden-ядре + const `FINAL_WATER_COUNT`.
4. ✅ Handler `water_plant` переключён на `next_water_step`; `Corrupt`→500 (fail loud), `AtFinalStage`→friendly JSON; убраны `.max(0)` и `unwrap_or(Final)`-маскировки.
5. ✅ 6 unit-тестов (advance/last-advance-completes/final-noop/negative→Corrupt/above-final→Corrupt/all-valid→real-stage); 69 defensive PASS; backend+WASM; fmt.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/trios/garden.rs`**: `pub const FINAL_WATER_COUNT: i32 = TOTAL_WATER_STAGES as i32 - 1;`; `pub enum WaterStep`; `pub fn next_water_step(i32) -> WaterStep` (range-check → `Corrupt`; `== FINAL` → `AtFinalStage`; иначе `Advance` с валидной стадией). 6 тестов.
- **`src/api/garden.rs`** (`water_plant`): заменил upper-only-check + `.max(0)` + `unwrap_or(Final)` на `match garden::next_water_step(water_count)`; `Corrupt` → `tracing::error!` + `500`.

Проверка: `cargo test … next_water_step` → 6 PASS; defensive-tests → 69 PASS; WASM check (garden — shared core) → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🆕 Продолжить fresh-area scan
Остатки: `api/observability.rs`, `api/mod.rs`, `bot/handlers.rs`, или глубокий аудит другого крупного API-handler'а на silent-default/clamp анти-паттерны (тот же класс, что W-72).

### Вариант B — 🔭 Sweep silent-default anti-pattern (W-72 как класс)
Поискать по кодовой базе другие `.max(0)`/`unwrap_or(<sensitive default>)`/`.unwrap_or_default()` на финансовых/game-state значениях из БД, которые маскируют corruption. Маленький фитнес-тест/lint мог бы их ловить.

### Вариант C — 🚀/🛡️ Накопленный долг — нужен sign-off
Deploy #18-#20 в `main`, либо DB CHECK self-ref (W-68). Ops/migration.

---

## 7. SKILL SAVED

Память: новый `fail-loud-not-silent-clamp.md` + строка в `MEMORY.md`. Принцип: на trust-boundary (чтение из БД/клиента) валидируй диапазон и **fail loud** на нарушении — не клампь/не подставляй тихо «безопасный» дефолт (`.max(0)`, `unwrap_or(Final)`): это прячет corruption и продлевает feedback-loop. Дублирование валидации между pure-ядром и сырым-SQL API убирай извлечением общей pure-функции (functional-core).

**Anchor:** `phi^2 + phi^-2 = 3`
