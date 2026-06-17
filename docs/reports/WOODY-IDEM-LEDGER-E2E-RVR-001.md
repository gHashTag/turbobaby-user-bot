# 🌊 WAVE LOOP REPORT — Closing the Assertion Gap: Idempotent add_bonus Ledger Count

**Document ID:** `WOODY-IDEM-LEDGER-E2E-RVR-001`
**Wave:** #49 — e2e-покрытие idempotency-эффекта (Вариант B из Wave #48)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `980c789` — `test(idempotency): assert add_bonus credits once + writes exactly one ledger row`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — закрыт assertion-gap в idempotency-тесте; 2 idempotency e2e + 69 defensive зелёные.**

Нашёл **assertion-gap**: существующий `add_bonus_idempotent_replay_returns_same_tx_id` в doc-комментарии заявляет «two POSTs produce ONE bonus_transactions row», но проверяет только **response-контракт** (тот же `tx_id`, `idempotent_replay:true`) — **не** фактический эффект (баланс, число ledger-строк). Это ровно тот анти-паттерн, о котором говорит индустрия: «тест, проверяющий только статус, верифицирует *заявление* API, а не *эффект*».

Добавил `add_bonus_same_key_credits_once_and_writes_one_ledger_row`: дважды POST с одним idempotency-ключом, затем (1) `bonus_balance == 100` (credited once, через owner GET — application-effect) и (2) **ровно одна** `bonus_transactions` строка (прямой DB-COUNT — ловит дубли, которые balance-only-проверка пропустила бы). Закрывает разрыв между заявлением и проверкой.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-82 | idempotency-тест проверял только response, не эффект (balance/ledger-count); doc-claim не верифицирован | 🟡 LOW (test assertion-gap) | ✅ новый тест: balance==100 + ledger-count==1 |
| — | add_bonus idempotency (replay + atomic ledger) — корректен (подтверждён e2e) | ✅ | — |
| 🚨 | PR ветки `fix/profile-routing` (96 ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: проверяй side-effect (persistence/ledger-count), не только response-статус; audit-assertions.**

- **Status ≠ effect**: «a test asserting only the response status verifies the API's *claim*, not the *effect* — add a query confirming the row was actually persisted». Ровно мой пробел.
- **Assert no extra rows**: «assert that no further rows were found — catches duplicate writes». Мой `COUNT==1`.
- **Query DB directly для ledger**: «least ambiguity arises if the test reads the database directly» (vs read-back через тот же код). Balance — через API, ledger-count — прямой DB-query.
- **Audit-assertions (occurrence + completeness)**: row-count + content покрывает «транзакция записана» (completeness) и «соответствует запросу» (occurrence).

Источники:
- [Integration Test Patterns — verify the side-effect in DB (Pugh)](https://www.brandonpugh.com/better-way/development-guidelines/testing/integration-test-patterns.html)
- [How to Assert Database State — first row / no further rows / query directly (Enterprise Craftsmanship)](https://enterprisecraftsmanship.com/posts/how-to-assert-database-state/)
- [The audit of assertions — occurrence / completeness (ACCA)](https://www.accaglobal.com/gb/en/student/exam-support-resources/fundamentals-exams-study-resources/f8/technical-articles/assertions.html)
- [Django integration testing — round-trip persistence, not cache (Honeybadger)](https://www.honeybadger.io/blog/django-integration-testing/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Прочитал существующий idempotency-тест → assertion-gap (doc-claim «ONE row» не проверен).
2. ✅ Новый тест на `make_app_with_db` (нужен db-handle для ledger-COUNT).
3. ✅ Дважды POST с одним ключом → r2 `idempotent_replay:true`.
4. ✅ Effect-1: owner GET → `bonus_balance == 100.0` (credited once).
5. ✅ Effect-2: прямой `SELECT COUNT(*) FROM bonus_transactions WHERE telegram_id` → 1.
6. ✅ 2 idempotency e2e PASS (local throwaway-PG, `--test-threads=1`); БД удалена; 69 defensive PASS; backend+WASM; fmt.
7. ✅ Research (status-vs-effect/audit-assertions) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`tests/integration_idempotency.rs`**: новый `add_bonus_same_key_credits_once_and_writes_one_ledger_row` (переиспользует `post_add_bonus`/`rand_suffix`; `make_app_with_db` для db; owner GET для balance; raw `Statement` COUNT для ledger).

Проверка: 2 idempotency e2e PASS (local throwaway-PG); default `cargo test` — `ignored`; 69 defensive PASS; WASM 0 warnings; fmt clean. Прод не тронут (guard [[handler-integration-testing]]).

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
96 коммитов ahead, e2e-сьют существенно расширен (~24 integration-теста). Ops/review — твоё «go».

### Вариант B — 🧪 e2e use_bonus exactly-once / insufficient-funds
Дополнить: use_bonus двойной replay (один debit-ledger), и insufficient-funds → 400 без записи.

### Вариант C — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

---

## 7. SKILL SAVED

Память: обновлён [[handler-integration-testing]] (status-vs-effect / assertion-gap). Принцип: не доверяй только response-коду/флагу — проверяй фактический side-effect (баланс через API + ровно N ledger-строк прямым DB-query); «assert no extra rows» ловит дубли; если doc/комментарий теста что-то заявляет — добавь ассерт, иначе это claim без проверки.

**Anchor:** `phi^2 + phi^-2 = 3`
