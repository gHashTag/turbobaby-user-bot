# 🌊 WAVE LOOP REPORT — Garden Seed-Item Regression Guard

**Document ID:** `WOODY-GARDEN-SEED-GUARD-RVR-001`
**Wave:** #12 — реализован «Garden-seed regression guard» (Вариант B из Wave #11)
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `2e3462c` — `refactor(garden): extract first_seedable_item + regression tests`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — pure-функция извлечена, 7 host-тестов зелёные, поведение сохранено.**

Закрыл самую «горячую» историческую зону хотфиксов. Серия правок garden-seed (миграции 026 → 036 «non_strain_orders» → 037 «universal_backfill», endpoint `force_seed`) крутилась вокруг **одного правила: какой item заказа становится посаженным семенем**. Эта логика жила инлайн в `force_seed` (DB-bound handler) и **не имела теста** — миграции 036/037 существовали именно потому, что заказы только с аксессуаром/чаем/набором (без strain) не сеяли растение.

Извлёк правило в **чистую функцию** `first_seedable_item(items_json) -> Option<(id, name)>` (приоритет strain > set > accessory > tea) и закрыл **7 host-юнит-тестами без БД**, покрывающими каждый тип каталога, не-strain кейсы (класс бага 036), пропуск пустых id, приоритет и краевые случаи. `force_seed` теперь вызывает функцию — поведение сохранено, намерение зафиксировано.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-04 | Garden seed-выбор item: историческая зона хотфиксов (026/036/037), 0 тестов, логика в handler | 🟠 MED | ✅ Извлечена в pure fn + 7 регресс-тестов |
| W-31 | SQL-бэкфилл (миграции 036/037) дублирует ту же precedence-логику в SQL → дрейф с Rust | 🟡 LOW | 📋 Wave+1 (note/guard) |
| W-32 | Набор статусов `('completed','confirmed','ready')` в SQL churn'ил, без предиката/теста | 🟡 LOW | 📋 Wave+1 |
| W-03 | 221 трекаемый бинарник в git | 🟠 MED | 📋 нужно подтверждение |

---

## 3. НАУЧНАЯ БАЗА

**Тема: Functional Core / Imperative Shell + Humble Object.**

- **Functional Core, Imperative Shell** (Gary Bernhardt, «Boundaries», 2012): решающую логику держат в чистом ядре без сайд-эффектов, а I/O (БД, сеть) — в тонкой императивной оболочке. «Making decisions is usually part of the core code. The shell is just the interface to components that cause side effects.» Ядро **тривиально тестируемо** — без мокинга и поднятия инфраструктуры.
- **Humble Object** (Gerard Meszaros, *xUnit Test Patterns*): когда поведение трудно тестировать из-за связки с тяжёлой зависимостью (БД/сеть), выноси всю значимую логику в легко-тестируемый компонент, оставляя «скромный» адаптер тонким.

Мой рефактор — точное применение: `force_seed` был handler'ом с БД-запросами + решающей логикой вперемешку. Вынес решение (`first_seedable_item`) в чистое ядро → быстрые юнит-тесты без БД; handler стал тоньше (humble shell). Рекомендованный сплит «много быстрых unit-тестов ядра + мало интеграционных тестов оболочки» — ровно то, что получилось (7 unit + существующий `tests/integration_garden_seed.rs` для записи в БД).

Источники:
- [Functional Core, Imperative Shell (Bernhardt, Destroy All Software)](https://www.destroyallsoftware.com/screencasts/catalog/functional-core-imperative-shell)
- [Functional Core, Imperative Shell — обзор паттерна](https://functional-architecture.org/functional_core_imperative_shell/)
- [Simplify & Succeed: Imperative Shell & Functional Core (R. Fritzsche)](https://ricofritzsche.me/simplify-succeed-replacing-layered-architectures-with-an-imperative-shell-and-functional-core/)
- Meszaros, *xUnit Test Patterns* — Humble Object (канон).

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Разведка: `force_seed` (garden.rs) — нашёл инлайн решающую логику выбора item + статус-фильтр; подтвердил историю 026/036/037.
2. ✅ Извлёк `first_seedable_item(&Value) -> Option<(String,String)>` (precedence strain>set>accessory>tea), эквивалентную исходной.
3. ✅ Заменил ~70 строк инлайна в `force_seed` на вызов функции (поведение сохранено).
4. ✅ Добавил 7 host-тестов в `mod tests` (strain/accessory/tea/set, precedence, skip-пустых-id, none, missing-name).
5. ✅ `cargo test … seedable` → 7 passed; `cargo fmt`; commit.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/api/garden.rs`**: новая `fn first_seedable_item(items: &serde_json::Value) -> Option<(String, String)>` — итерирует items, в каждом проверяет ключи `(strain_id,set_id,accessory_id,tea_id)` по precedence, возвращает первый непустой `(id, name)`. `force_seed` использует её через `let Some((strain_id, strain_name)) = first_seedable_item(&items) else { … no_catalog_items }`.
- 7 unit-тестов в существующем `#[cfg(test)] mod tests` (host-runnable: `cargo test --bin woody-weed-bot-server --features backend seedable`).

Запуск: 7 passed; bin компилируется (force_seed wired); fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🧬 SQL↔Rust seed-logic drift guard (W-31)
Миграции 036/037 повторяют precedence-выбор item в SQL. Либо defense-тест, что SQL-бэкфилл и `first_seedable_item` согласованы (парсинг SQL на наличие тех же ключей/приоритета), либо документировать единый контракт. Закрывает дрейф между batch-backfill (SQL) и live-seed (Rust).

### Вариант B — 🩺 Functional-core sweep других handler'ов
Применить тот же приём к другим «толстым» handler'ам с инлайн-решениями (checkout item-parsing, fraud-проверки, idempotency). Вынести pure-логику → host-тесты. Систематически повышает покрытие критичных путей без БД.

### Вариант C — 🔢 Quantity selector в модалке (из Wave #11)
−/+ выбор количества в `ProductDetailModal` перед «В корзину». Frontend-only, переиспользует `CartItem.quantity`.

---

## 7. SKILL SAVED

Память: новый `garden-seed-pure-core.md` + строка в `MEMORY.md`. Принцип: решающую логику (что/какой item) выноси из БД-handler'а в чистую функцию и закрывай host-тестами (Functional Core / Imperative Shell + Humble Object) — особенно в зонах с историей хотфиксов.

**Anchor:** `phi^2 + phi^-2 = 3`
