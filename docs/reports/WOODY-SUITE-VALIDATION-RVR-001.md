# 🌊 WAVE LOOP REPORT — End-to-End Suite Validation Surfaces a Rotted Test

**Document ID:** `WOODY-SUITE-VALIDATION-RVR-001`
**Wave:** #45 — fresh-area scan (Вариант A из Wave #44) → scan чист → e2e-валидация ветки нашла сломанный тест
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `2d3e21d` — `test(idempotency): fix broken negative test that panicked building the request`
**Agent:** Claude Opus 4.8 (Wave loop) + Explore-subagent

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — сломанный integration-тест починен; весь suite (17 тестов) зелёный e2e; ветка #35-#44 валидирована.**

Fresh-area scan на свежий класс (panic/race/leak/UI) дефекта **не нашёл** — backend крепко закалён (агент: только low-severity UI display-нит). Вместо натянутого фикса (по правилу [[invariant-lock-in-when-clean]]) сделал высокоценное безопасное: **прогнал весь накопленный `#[ignore]` integration-suite end-to-end** против локальной throwaway-PG (harness готов с #43), чтобы валидировать runtime-поведение ветки #35-#44 перед растущим PR-долгом.

Это сразу вскрыло **реальный сломанный тест**: `malformed_idempotency_key_rejected_400` включал `"with\nnewline"` в список «bad keys», но `\n` — невалидный символ HTTP-**заголовка**, так что `Request::builder().header(...).unwrap()` **паниковал** ещё до отправки. Тест проверяет *хендлер* (валидатор idem-ключа), а `\n` тестирует *HTTP-слой* — не тот boundary. Пред-существующая поломка (от 2026-06-07), невидимая пока suite реально не запускался (`#[ignore]` гниёт). Заменил на `"with.dot"` (валидный header-символ вне `[A-Za-z0-9_-]` → хендлер 400). Весь suite (17 тестов) теперь зелёный.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-80 | `malformed_idempotency_key_rejected_400` паниковал на построении запроса (`\n` = invalid header value → не тот test-boundary); rotted `#[ignore]`-тест | 🟡 LOW (broken test / false confidence) | ✅ `\n`→`"with.dot"`; suite зелёный e2e |
| — | Fresh-area scan (panic/race/leak/UI): дефекта нет — backend hardened | ✅ | честно: clean |
| — | Ветка #35-#44 валидирована e2e (17 integration-тестов) | ✅ deploy-readiness | — |
| 🚨 | Прод-фиксы #18-#20 — деплой; PR ветки в main | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: ignored-тесты гниют; тест не на том boundary; гоняй integration-suite перед merge.**

- **Ignored-тест, который не бегает — хуже отсутствия**: «gives false confidence … bit rot: eventually it fails — or worse, passes testing the wrong thing». Ровно `\n`-кейс.
- **`#[ignore]` пропускает запуск, НЕ компиляцию** → ignored-тесты живут как compile-check; «run the full integration suite before merge / nightly». Сделал именно это перед PR.
- **Тест на правильном boundary**: «unit-тест не пересекает порт; I/O с БД/сетью — integration». `\n` тестировал HTTP-header-слой внутри handler-теста — смешение слоёв.
- **Silent-skip даёт ложное «green»**: audit — «CI started failing on ~25% of PRs once skipped tests were un-skipped». Скрытая поломка ровно такого рода.

Источники:
- [Skip Rust tests with #[ignore] & run later (RustFAQ)](https://www.rustfaq.org/en/how-to-ignore-tests-and-run-them-conditionally/)
- [5% of codebases silently skipped tests (Code Review Doctor)](https://codereviewdoctor.medium.com/5-of-the-420-python-codebases-we-checked-silently-skipped-tests-so-we-fixed-them-1716bb8fcee7)
- [Unit Test Boundaries (Haacked)](https://haacked.com/archive/2008/07/22/unit-test-boundaries.aspx/)
- [False confidence from incomplete testing (Saini)](https://manishsaini74.medium.com/the-only-thing-worse-than-no-testing-is-false-confidence-from-incomplete-testing-345b0ffb4ef0)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Fresh-area scan (panic/race/leak/UI) → дефекта нет (backend hardened).
2. ✅ Решение: e2e-валидация ветки вместо натянутого фикса (deploy-readiness перед PR).
3. ✅ Создал локальную throwaway-PG; прогнал весь `--ignored` suite.
4. ✅ Нашёл panic в `malformed_idempotency_key_rejected_400` (`\n` invalid header value).
5. ✅ Диагноз: тест на не том boundary (HTTP-слой vs хендлер); `\n`→`"with.dot"`.
6. ✅ Перепрогон: 17 integration-тестов зелёные; удалил локальную БД; 69 defensive PASS; fmt.
7. ✅ Research (ignored-тесты/boundary) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`tests/integration_idempotency_negative.rs`**: в `malformed_idempotency_key_rejected_400` заменил `"with\nnewline"` → `"with.dot"` + комментарий, что control-chars отвергаются на HTTP-header-слое (другой boundary), а кейсы списка — валидные header-значения, доходящие до хендлера и получающие 400.

Проверка (локальная throwaway-PG `woody_wave_test`, `--test-threads=1`): полный suite — **17 passed** (admin_check 3, create_order 1, garden_seed 2, idempotency 1, idempotency_negative 5, marketing 3, smoke 1, use_bonus 1); defensive 69 PASS; backend+WASM; fmt clean. Локальная БД удалена; прод не тронут (guard [[handler-integration-testing]]).

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` в `main` (нужен sign-off)
Ветка валидирована e2e (17 integration + defensive). ~24 коммита (#18-20, #35-#45). Готова к review/merge/deploy. Ops-действие — твоё «go».

### Вариант B — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

### Вариант C — 🧪 Расширить e2e-покрытие
Integration-тесты для self-referral (W-67), use_reward fail-loud (W-72.1), quest fake-id (W-77).

---

## 7. SKILL SAVED

Память: новый `run-ignored-suite-before-merge.md` + строка в `MEMORY.md`. Принципы: когда scan чист — валидируй накопленную работу e2e (а не натягивай фикс); `#[ignore]`-тесты гниют (компилируются, но не бегут) → гоняй полный suite перед PR (local throwaway-PG); пиши тест на правильном boundary (control-char rejection — HTTP-слой, не handler-тест).

**Anchor:** `phi^2 + phi^-2 = 3`
