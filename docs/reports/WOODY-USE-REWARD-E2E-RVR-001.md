# 🌊 WAVE LOOP REPORT — E2E use_reward: Bonus Credited Exactly Once

**Document ID:** `WOODY-USE-REWARD-E2E-RVR-001`
**Wave:** #48 — e2e-покрытие финансовой мутации (Вариант B из Wave #47)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `79800e5` — `test(garden): e2e use_reward credits bonus exactly once + owner-gated`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — reward-redemption (финансовая мутация) покрыта e2e: «credit exactly once»; 2 e2e + 69 defensive зелёные.**

Закрепил Wave #39 работу (W-72.1 fail-loud reward-reads + унифицированный `reward_is_active`) **end-to-end** через живой Router, используя `make_init_data` (#47). Тест по канону exactly-once-финансов: засеял валидный reward, **дважды** дёрнул `POST /api/garden/rewards/:id/use`, и проверил, что бонус начислен **ровно один раз** (balance == bonus_points), а вторая попытка отклонена (атомарный `WHERE is_used = false` guard — нет двойного кредита). Плюс owner-enforcement: чужой пользователь (валидная initData другого id) → **403**.

`bonus_balance += bonus_points` — **delta-мутация** (самый рисковый класс для двойного начисления), так что тест бьёт ровно туда. Все начисления/чтения — через API; reward засеян raw SQL через db-handle harness'а.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-72.1 | use_reward (financial mutation) без runtime e2e | 🟢 INFO | ✅ e2e: credit-once + second-use-rejected + owner-403 |
| — | Атомарный `WHERE is_used=false` guard (no double credit) | ✅ | подтверждён e2e |
| — | reward_is_active boundary + fail-loud reads (Wave #39) | ✅ | косвенно прогнаны happy-path'ом |
| 🚨 | PR ветки `fix/profile-routing` (96 ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: exactly-once side-effect тесты; no-double-credit; atomic idempotent guard; delta vs state.**

- **«Fire the same mutation twice, assert single credit»**: «if a single operation to move money is called multiple times, the system should move money **at most once**». Ровно мой двойной use → credit once.
- **Delta — рисковый класс**: «if your credit operation is a delta (`balance += amount`), your test is correctly targeting the riskiest case». `bonus_balance += bonus_points` — delta.
- **Application-level effect, не delivery**: «verify the *application* effect (balance), not just that a request was processed once». Проверяю фактический баланс.
- **Idempotent/atomic guard**: «the DB logic treats a retry as a no-op». Атомарный `WHERE is_used=false` → второй use = no-op (success:false).

Источники:
- [Avoiding double payments in a distributed payments system (Airbnb Eng)](https://medium.com/airbnb-engineering/avoiding-double-payments-in-a-distributed-payments-system-2981f6b070bb)
- [Idempotency: Preventing Double Charges (DZone)](https://dzone.com/articles/art-of-idempotency-preventing-double-charges-and-duplicate)
- [Building Resilient GraphQL APIs Using Idempotency (Shopify)](https://shopify.engineering/building-resilient-graphql-apis-using-idempotency)
- [Exactly-Once Processing + Idempotent Writes (Threadsafe)](https://medium.com/threadsafe/exactly-once-processing-across-kafka-and-databases-using-kafka-transactions-idempotent-writes-09fe1f75bdab)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Прочитал `garden_rewards` схему (004_garden.sql) + use_reward handler.
2. ✅ Seed reward через db-handle raw `Statement` (id/plant/user/strain/discount/bonus/expires/created).
3. ✅ Happy-path: owner initData → use → 200 + discount/bonus; GET профиля → balance == bonus (credit once).
4. ✅ Second use → success:false (atomic guard, no double credit).
5. ✅ Owner-enforcement: attacker initData → 403.
6. ✅ 2 e2e PASS (local throwaway-PG, `--test-threads=1`); БД удалена; 69 defensive PASS; backend+WASM; fmt.
7. ✅ Research (exactly-once/no-double-credit) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`tests/integration_use_reward.rs`** (`#[ignore]`): `use_reward_credits_bonus_then_rejects_second_use` (seed → use → balance==250 → second use rejected) + `use_reward_rejects_non_owner` (attacker → 403). `now_millis` helper; seed через `db.orm.execute(Statement)`.

Проверка: 2 e2e PASS (local throwaway-PG); default `cargo test` — `ignored`; 69 defensive PASS; WASM 0 warnings; fmt clean. Прод не тронут (guard [[handler-integration-testing]]). Замечание: use_reward кредитует balance напрямую (без bonus_transactions ledger-строки в credit-направлении), поэтому ledger-count-ассерт не применим — проверяю баланс.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
96 коммитов ahead, e2e-валидированы (suite расширен до ~22+ integration-тестов). Ops/review — твоё «go».

### Вариант B — 🧪 e2e add_bonus exactly-once (idempotency-key)
Дополнить idempotency-suite: тот же ключ дважды → один tx_id + balance credited once (ledger-count-ассерт применим — add_bonus пишет bonus_transactions).

### Вариант C — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

---

## 7. SKILL SAVED

Память: обновлён [[handler-integration-testing]] (exactly-once / no-double-credit паттерн). Принцип: для финансовой мутации (особенно delta `balance += x`) пиши e2e «дёрни дважды → начислено ровно один раз»; проверяй фактический баланс (application-effect), не только статус; атомарный guard (`WHERE is_used=false`) подтверждается тем, что вторая попытка — no-op.

**Anchor:** `phi^2 + phi^-2 = 3`
