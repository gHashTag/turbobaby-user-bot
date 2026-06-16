# 🌊 WAVE LOOP REPORT — Migration-Wiring Defense

**Document ID:** `WOODY-MIGRATION-WIRING-RVR-001`
**Wave:** Слабые места → научная база → декомпозиция → реализация
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `a0d731f` — `test(db): defense test for migration wiring + sequential numbering`
**Agent:** Claude Opus 4.8 (Wave loop, 15-min cadence)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — invariant shipped, all hooks pass (check-backend, check-wasm, 63 defensive-tests, fmt, conventional-commits).**

Исследование слабых мест показало, что кодовая база **дисциплинированна**: 155 lib-тестов + 629 backend-тестов зелёные, `unwrap()` в проде почти отсутствуют (93 из 93 — в `#[cfg(test)]`, regex, HMAC `new_from_slice` которые не паникуют). Реальная уязвимость — **не код, а wiring**: `MIGRATION_SQL` собирается вручную через `concat!(include_str!(...))`, и файл миграции можно положить в `migrations/`, забыв вписать в список. Это уже случалось дважды (коммит `114de7d` «wire 036 and 037 into MIGRATION_SQL») и проявляется как runtime-500 на свежей БД спустя время после merge.

Реализованы две **architectural fitness functions** (Ford/Parsons/Kua), закрывающие класс багов на этапе `cargo test`, а не в проде.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие в этом Wave |
|---|---|---|---|
| W-01 | `MIGRATION_SQL` — ручной `concat!`, файл миграции легко не подключить (баг уже бил 2×) | 🔴 HIGH | ✅ Закрыт fitness-тестом |
| W-02 | Нет проверки последовательности номеров миграций (gap/дубликат → недетерминированный порядок) | 🟠 MED | ✅ Закрыт fitness-тестом |
| W-03 | Репозиторий трекает 221 файл артефактов: `dist/.stage/*`, `*.mp4` (видео), `*_bg.wasm`, аплоады из `/data` | 🟠 MED | 📋 В план Wave+1 (см. §6) |
| W-04 | Последние ~15 коммитов — реактивное тушение пожаров garden-seed (036, 037, force-seed endpoint); нет регресс-теста инварианта «у пользователя с завершённым заказом есть активное растение» | 🟠 MED | 📋 В план Wave+1 |
| W-05 | Backend-тесты требуют `--features backend` и не запускаются обычным `cargo test --lib` (WASM-only lib) — легко пропустить локально | 🟡 LOW | 📋 Документировано здесь |

---

## 3. НАУЧНАЯ БАЗА

**Architectural Fitness Functions** — Neal Ford, Rebecca Parsons, Patrick Kua, *Building Evolutionary Architectures* (O'Reilly). Тезис: unit-тесты проверяют **поведенческую** корректность, fitness-функции — **структурную** корректность (инварианты архитектуры). Их встраивают в CI рядом с тестами/линтерами/type-checker'ами, чтобы ловить **architectural drift** (медленную эрозию замысла) до того, как он превратится в дорогой технический долг.

Ключевые выводы, применённые здесь:
- **Триггерные проверки ловят ошибки в момент их появления** — наш тест падает при `cargo test`, как только появляется неподключённая миграция (а не в проде).
- **Инвариант, а не намерение** — нельзя полагаться на code review и добрые намерения; правило кодифицируется и фейлит билд.
- **Без премахи** — fitness-функцию пишем под реальный, уже случившийся класс багов (W-01 бил дважды), а не гипотетический.

Тот же класс защит уже живёт в проекте: `entity_schema_consistency_tests`, `schema_drift_tests`, `ui_module_wiring_tests`. Этот Wave добавляет четвёртую — migration-wiring.

Источники:
- [Building Evolutionary Architectures — fitness function-driven development (Thoughtworks)](https://www.thoughtworks.com/en-us/insights/articles/fitness-function-driven-development)
- [Fitness Functions: Unit Tests for Your Architecture (R. Ramirez)](https://xpromx.me/articles/fitness-functions-unit-tests-for-your-architecture/)
- [Continuous Integration with Architectural Invariants (SCG, Univ. Bern)](https://scg.unibe.ch/archive/masters/Truf15a.pdf)
- [Stop Architecture Drift: Operationalizing ADRs with Automated Fitness Functions](https://platformtoolsmith.com/blog/operationalizing-adrs-fitness-functions/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Survey: git log, src-дерево, тестовый прогон → база здоровья.
2. ✅ Weak-spot scan: `unwrap`/`panic`/`expect` по файлам, отсев test-модулей.
3. ✅ Root-cause: `MIGRATION_SQL` = ручной `concat!`; подтверждён баг-класс из `114de7d`.
4. ✅ Реализация: `mod migration_wiring_tests` в `src/db/mod.rs` — 2 теста.
5. ✅ Доказательство: throwaway `038_*.sql` → тест FAIL с понятным сообщением → удалён.
6. ✅ Зелёный прогон + commit через lefthook (fmt, defensive-tests, conventional-commits).
7. ✅ Отчёт + skill-memory.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

`src/db/mod.rs`, новый `#[cfg(test)] mod migration_wiring_tests`:

- **`every_migration_file_is_wired_into_migration_sql`** — каждый `migrations/*.sql` на диске должен встречаться как `include_str!("../../migrations/<name>")` в исходнике `mod.rs`. Иначе — список с именами неподключённых файлов и подсказкой.
- **`migration_numbers_are_sequential_and_unique`** — числовые префиксы образуют беспропускный, бездублирующий ряд от 001. Ловит и дубль номера (два дева от одной точки), и пропуск (удалённый/переименованный файл).

**Проверка-доказательство:** создан `038_wave_guard_probe.sql` (не подключён) → тест упал:
```
1 migration file(s) on disk are NOT wired into MIGRATION_SQL ...
  038_wave_guard_probe.sql
```
Probe удалён, дерево чистое (миграции по 037).

Запуск: `cargo test --bin woody-weed-bot-server --features backend migration_wiring` → `2 passed`.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🧹 Repo Hygiene (закрыть W-03)
Вынести `dist/.stage/`, `assets/*.mp4`, аплоады и `*_bg.wasm` из git-трекинга в `.gitignore`, проверить что Dockerfile/Trunk регенерируют `dist/` при деплое. Эффект: диффы перестают тонуть в бинарниках (221 файл → единицы), репозиторий худеет. Риск: низкий, но нужно подтвердить, что Railway-деплой не зависит от закоммиченного `dist/`.

### Вариант B — 🌱 Garden-seed Regression Guard (закрыть W-04)
Добавить fitness/регресс-тест инварианта «пользователь с завершённым (или confirmed/ready) заказом имеет активное растение», чтобы прекратить серию hotfix-миграций (026/036/037 + force-seed). Эффект: garden-seed logic фиксируется тестом, новые регрессии ловятся до прода. Нужен лёгкий harness над seed-функциями (без живой БД — на чистых функциях, как остальные defensive-tests).

### Вариант C — 🛡️ Generalize the Wiring Invariant
Обобщить migration-wiring проверку в один «manifest-consistency» модуль, охватывающий все ручные реестры проекта: `pub mod X;` под `src/ui/`, список миграций, i18n-ключи `T_*`, маршруты API. Эффект: единая защита от drift всех «руками поддерживаемых списков». Это масштабирование §3-подхода на весь проект.

---

## 7. SKILL SAVED

Память: `migration-wiring-defense.md` + строка в `MEMORY.md`. Зафиксирован паттерн «ручной реестр → fitness-тест» как переиспользуемый приём для будущих Wave-лупов.

**Anchor:** `phi^2 + phi^-2 = 3`
