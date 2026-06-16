# 🌊 WAVE LOOP REPORT — Discount Math Invariant Lock-In

**Document ID:** `WOODY-DISCOUNT-INVARIANT-RVR-001`
**Wave:** #37 — fresh-area scan (Вариант A из Wave #36) → модули чистые → invariant lock-in в `src/trios/store.rs`
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `571f0c1` — `test(store): lock in calculate_cart_total discount invariants`
**Agent:** Claude Opus 4.8 (Wave loop) + Explore-subagent

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — финансовый инвариант скидки зафиксирован тестом; 3 cart-total + 69 defensive зелёные.**

Fresh-area scan по нетронутым модулям (`trios/garden.rs`, `trios/store.rs`, `trios/quest.rs`, `trios/core.rs`, `trios/i18n.rs`, `s3.rs`, `db/strains.rs`, `locales.rs`) **не нашёл практического дефекта** — код чистый. Единственный кандидат (overflow в `calculate_cart_total:311`) непрактичен: нужен total ~9.2e16 THB при cap 1M/item, а `saturating_*` исключает панику. Скидка уже двойне защищена: `discount: Option<u32>` (не бывает отрицательной) + `.min(100)` + `saturating_sub` (пол 0). **Честный вывод: дефекта нет.**

Поэтому самое ценное реальное действие — **зафиксировать инвариант тестом** (property/characterization-стиль). Существующие тесты пинали только 2 точки (None, 20%). Добавил тест на *форму* кривой скидки: `0 ≤ total ≤ subtotal`, ровно `0` при 100%, монотонно неубывающая скидка по 0..=100. Теперь будущая правка формулы (неверный оператор, off-by-one, переход на non-saturating арифметику, неверное округление) — ловится.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| — | `calculate_cart_total` discount-математика без guard'а *формы* (2 точечных теста) | 🟢 INFO (latent regression risk) | ✅ invariant-тест: bounds + 100%→0 + монотонность |
| W-71 | `store.rs:311` overflow при total ~9.2e16 (теоретич.) | 🟢 INFO | непрактичен (cap 1M/item, saturating) — не трогаю |
| — | `garden.rs`, `quest.rs`, `core.rs`, `i18n.rs`, `s3.rs`, `db/strains.rs`, `locales.rs` — clean | ✅ | — |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: property-based / invariant / metamorphic / characterization-тесты для финансовой математики.**

- **Invariant — это property для всех входов**: «a property is an invariant — these hold for all valid inputs; if you find an input that violates them, you've found a bug». Мой тест проверяет инвариант `0 ≤ total ≤ subtotal` по всему диапазону 0..=100, а не 2 примера.
- **Monotonicity** — признанная финансовая/регуляторная property («ordered relationships between inputs and outputs»). Скидка ↑ ⇒ total ↓ (неубывание) — ровно это.
- **Characterization/golden tests** (Feathers): «document/lock existing behavior before refactoring … safety net so you don't inadvertently alter intended behavior». Модули чистые → фиксируем поведение, чтобы рефактор его не сломал.
- **Financial invariants структурны**: «bid-ask spreads non-negative; timestamps monotonic … structural properties; when violated, something is broken in your pipeline». Total в `[0, subtotal]` — такая же структурная гарантия.

Источники:
- [Property-Based Testing meets Financial Data (Susan Potter)](https://www.susanpotter.net/quant/property-based-testing-statistical-validation/)
- [Metamorphic testing (Wikipedia)](https://en.wikipedia.org/wiki/Metamorphic_testing)
- [Characterization test (Wikipedia)](https://en.wikipedia.org/wiki/Characterization_test)
- [Choosing properties for property-based testing (F# for Fun and Profit)](https://fsharpforfunandprofit.com/posts/property-based-testing-2/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Fresh-area scan (Explore) по 8 нетронутым модулям → кандидаты только теоретические (overflow), код чистый.
2. ✅ Сам проверил `calculate_cart_total`: `u32` + `.min(100)` + `saturating_sub` → инвариант держится; overflow непрактичен. Дефекта нет.
3. ✅ Решение: lock-in инварианта (property/characterization) вместо «натянутого» фикса.
4. ✅ Тест `test_calculate_cart_total_discount_is_clamped_to_100_percent`: 100%→0, over-100→0, bounds `[0,subtotal]` + монотонность по 0..=100.
5. ✅ Точный комментарий (не переоценивать, что ловит тест: bound двойне защищён `u32`+saturating, clamp — defense-in-depth).
6. ✅ 3 cart-total теста PASS; defensive 69 PASS; backend+WASM; fmt.
7. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/trios/store.rs`** (`#[cfg(test)] mod tests`): новый `test_calculate_cart_total_discount_is_clamped_to_100_percent` — asserts: `Some(100)`→0; `Some(101..=u32::MAX)`→0; для `d` в 0..=100 → `total ∈ [0, subtotal]` и монотонно неубывает. Поясняющий doc-комментарий о двойной защите bound'а.

Проверка: `cargo test … calculate_cart_total` → 3 PASS; defensive-tests → 69 PASS; WASM 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🆕 Продолжить fresh-area scan
Остатки нетронутого: `api/observability.rs`, `api/mod.rs`, `bot/handlers.rs` (мелкие), либо глубокий property-аудит `trios/garden.rs` time-математики (наибольшая game-логика).

### Вариант B — 🧪 Handler-test harness (W-70)
Решить [[integration-test-harness-blocker]] (lib.rs WASM-only), чтобы покрыть authz-handler'ы end-to-end. Крупная структурная задача.

### Вариант C — 🚀/🛡️ Накопленный долг — нужен sign-off
Deploy #18-#20 в `main`, либо DB CHECK self-ref (W-68). Ops/migration.

---

## 7. SKILL SAVED

Память: новый `invariant-lock-in-when-clean.md` + строка в `MEMORY.md`. Принцип: когда fresh-area scan находит чистый код — не натягивай маргинальный фикс; зафиксируй *инвариант* (bounds/монотонность/граничные точки) property/characterization-тестом, чтобы будущий рефактор не сломал его тихо. Честно отчитывайся «дефекта нет», и не переоценивай, что именно ловит тест.

**Anchor:** `phi^2 + phi^-2 = 3`
