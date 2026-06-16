# 🌊 WAVE LOOP REPORT — Self-Referral Guard at the Data Layer

**Document ID:** `WOODY-SELFREF-RVR-001`
**Wave:** #35 — fresh-area scan (Вариант A из Wave #34) → реальный дефект в `src/db/referrals.rs`
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `4ab0f3e` — `fix(referrals): reject self-referral at the data layer (defense-in-depth)`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — defense-in-depth дыра самореферала закрыта; 4 unit-теста + 69 defensive зелёные.**

Fresh-area scan (веерный поиск по нетронутым модулям) нашёл **реальный** financial-fraud weak spot: `record_referral` (`src/db/referrals.rs`) принимал любые `referrer_id`/`referred_id` без проверки, что они различны. Инвариант «нельзя реферить самого себя» жил **только в вызывающем** (бот `/start ref_…`, `commands.rs:120` — `if referrer_id != user_id`). HTTP API (`api/referrals.rs`) сейчас только на чтение, так что **активной эксплуатации нет**, но любой будущий вызывающий (админ-тул, новый эндпоинт) мог создать self-referral pending-событие, которое `confirm_referral` позже **оплатил бы бонусом на собственный баланс пользователя**.

Перенёс инвариант в саму функцию: pure-предикат `is_self_referral(a, b)` + ранний `bail!` в `record_referral` (зеркалит существующие `bail!`-guard'ы в `confirm_referral`). Теперь дыру нельзя открыть ни одним вызывающим.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-67 | `record_referral` не проверял `referrer_id != referred_id` (инвариант только в caller) | 🟠 MED (financial, defense-in-depth) | ✅ `is_self_referral` + `bail!` в data-слое + 2 теста |
| W-68 | Нет DB-level `CHECK (referrer_id <> referred_id)` (сильнейший слой) | 🟢 INFO | 📋 нужна migration + prod-deploy → sign-off |
| — | `rate_limit.rs`, `loyalty.rs`, `cart.rs`, `happy_hour.rs`, `pricing.rs`, `tech_tree.rs`, `strains.rs` — clean | ✅ | — |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: referral fraud (self-referral) + enforce invariants at the data/domain layer (DDD).**

- **Self-referral — признанный fraud-вектор**: «Your referral platform should have built-in logic to **prevent an advocate from referring themselves**» — это foundational-проверка, поверх которой идут IP/device-fingerprint/риск-скоринг/pending-hold/payout-delay.
- **Инвариант — на data/domain-слое, не у вызывающего**: «the class makes no promises, so **every caller has to enforce the rules itself**» → drift; «**new endpoints can't accidentally bypass a rule that lives on the entity**». Ровно мой случай: caller-only guard → data-layer guard.
- **Уже сделано в репо (best practice)**: referral идёт через `pending` (record) → `confirmed` при первой покупке (confirm) — это «reward real actions, not signups» + «pending-reward hold / payout delay». Мой fix добавляет недостающий foundational-инвариант.

Источники:
- [Referral Fraud Prevention (GrowSurf)](https://growsurf.com/glossary/referral-fraud/)
- [Combat referral abuse & fraud (Voucherify)](https://www.voucherify.io/blog/blowing-the-whistle-how-to-combat-referral-abuse-and-fraud)
- [Invariants & why the domain model is the best place to enforce them (Jovanović)](https://www.milanjovanovic.tech/blog/what-invariants-are-and-why-a-domain-model-is-the-best-place-to-enforce-them)
- [Domain model layer validations (Microsoft .NET DDD)](https://learn.microsoft.com/en-us/dotnet/architecture/microservices/microservice-ddd-cqrs-patterns/domain-model-layer-validations)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Fresh-area scan (Explore-агент) по нетронутым модулям → highest-confidence дефект: `record_referral` без self-ref guard.
2. ✅ Подтвердил достижимость: единственный caller — `commands.rs:122` (guard есть); HTTP — read-only → defense-in-depth, не live-exploit.
3. ✅ Pure `is_self_referral(referrer_id, referred_id)` (functional-core, тестируемо без БД).
4. ✅ Ранний `bail!` в `record_referral` перед любым DB-вызовом; зеркалит `confirm_referral`.
5. ✅ 2 unit-теста (same-id → true incl. 0/-1; distinct → false); referrals-сьют 11 PASS, defensive 69 PASS; fmt.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/db/referrals.rs`**: новый `pub(crate) fn is_self_referral(i64, i64) -> bool`; в начале `record_referral` — `if is_self_referral(referrer_id, referred_id) { anyhow::bail!(...) }` (до всех DB-операций). Тесты `test_is_self_referral_detects_same_id` + `..._allows_distinct_ids`.

Проверка: `cargo test … referrals` → 11 PASS; defensive-tests → 69 PASS; WASM check 0 warnings; fmt clean. Поведение существующего бот-пути не меняется (он и так не вызывает с равными id).

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🛡️ DB-level CHECK constraint (W-68) — нужен sign-off
Migration `ALTER TABLE referral_events ADD CONSTRAINT chk_no_self_ref CHECK (referrer_id <> referred_id)` — сильнейший (последний) слой инварианта. Требует новой миграции + prod-deploy → решение пользователя (учитывая текущий deploy-долг #18-20).

### Вариант B — 🆕 Продолжить fresh-area scan
Следующий нетронутый модуль (`orders.rs`/`quest.rs`/`db/orders.rs` ещё не аудированы веером) на реальный дефект.

### Вариант C — 🚀 Deploy-долг (#18-#20) — нужен «go»
Cherry-pick прод-фиксов `/api/sets` 500 в `main` + деплой.

---

## 7. SKILL SAVED

Память: новый `referral-self-ref-guard.md` + строка в `MEMORY.md`. Принцип: инвариант домена живи на data-слое, не у вызывающего — «new endpoints can't accidentally bypass a rule that lives on the entity»; self-referral — foundational referral-fraud guard; репо уже делает pending→confirmed (payout-delay best practice). DB CHECK — следующий слой.

**Anchor:** `phi^2 + phi^-2 = 3`
