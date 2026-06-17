# 🌊 WAVE LOOP REPORT — E2E use_bonus Overdraw Guard (balance preserved)

**Document ID:** `WOODY-USE-BONUS-OVERDRAW-E2E-RVR-001`
**Wave:** #50 — финансовая e2e-трилогия завершена (Вариант B из Wave #49)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `37006e4` — `test(use_bonus): overdraw on funded account rejected, balance unchanged, no debit row`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — overdraw-guard `use_bonus` покрыт e2e на funded-счёте; 2 use_bonus e2e + 69 defensive зелёные. Веха #50.**

Завершил финансовую e2e-трилогию (use_reward #48 credit-once, add_bonus #49 ledger-count, use_bonus #50 overdraw). Существующие тесты покрывали replay+deduction-ledger и пустой-баланс-reject; пробел — **overdraw на пополненном счёте**. Новый `use_bonus_overdraw_rejected_balance_unchanged`: баланс 50, списать 100 → **400**, баланс **остаётся ровно 50** (нет частичного списания, нет клампа `GREATEST(0,…)` в 0), и **нет** debit-ledger-строки.

Это проверяет all-or-nothing инвариант атомарного `UPDATE … WHERE bonus_balance >= $1` (succeeds-or-does-nothing; `rows_affected==0` → 400) на счёте, где баг частичного списания реально двигал бы деньги (пустой-баланс-тест этого не ловит). Попутно зафиксировал follow-up **W-83**: нет DB-backstop `CHECK (bonus_balance >= 0)` — инвариант держится только на WHERE-guard приложения.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-72.x | use_bonus overdraw на funded-счёте без e2e (только empty-balance) | 🟢 INFO | ✅ e2e: 400 + balance==50 unchanged + 0 debit-rows |
| W-83 | Нет DB-backstop `CHECK (bonus_balance >= 0)` (инвариант только на app WHERE-guard) | 🟢 INFO | 📋 нужна migration+deploy → sign-off (как [[referral-self-ref-guard]] W-68) |
| — | Финансовая e2e-трилогия (credit-once / ledger / overdraw) завершена | ✅ | — |
| 🚨 | PR ветки `fix/profile-routing` (96 ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: атомарный conditional-update против overdraw; all-or-nothing (нет частичного списания); CHECK-backstop.**

- **Single atomic conditional update**: «`UPDATE … SET balance = balance - amount WHERE id=X AND balance >= amount`, succeeds-or-does-nothing; affected-row-count определяет успех». Ровно use_bonus; мой тест: `rows_affected==0` → 400.
- **Нет частичного списания**: «a single statement is inherently atomic — the deduction happens completely or not at all». Мой ассерт: balance остался ровно 50.
- **Тест инварианта**: «row-count assertion — assert zero rows affected when funds insufficient, treat as rejected; assert no side effects». Ровно мой (balance unchanged + 0 ledger-rows).
- **Backstop (W-83)**: «layer a DB `CHECK (balance >= 0)` so even buggy code can't violate the invariant; the whole tx aborts». В репо отсутствует → follow-up.

Источники:
- [How to prevent negative stock — conditional WHERE + @@ROWCOUNT (Microsoft Q&A)](https://learn.microsoft.com/en-us/answers/questions/1077286/how-to-prevent-negative-stock-in-database)
- [ACID — CHECK(balance>=0), tx aborts on violation (MotherDuck)](https://motherduck.com/learn/acid-transactions-sql/)
- [ACID & idempotent UPDATE…RETURNING, no negative balance (Pachot/DEV)](https://dev.to/franckpachot/acid-and-idempotent-update-returning-in-mongodb-with-findoneandupdate-955)
- [Atomicity in PostgreSQL — single statement all-or-nothing (DEV)](https://dev.to/kfir-g/understanding-atomicity-in-postgresql-a-deep-dive-into-the-a-in-acid-209a)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Прочитал `integration_use_bonus.rs` + негативный тест → пробел: overdraw на funded-счёте.
2. ✅ Тест: seed 50 → use 100 → 400.
3. ✅ Balance unchanged (owner GET → 50.0).
4. ✅ Нет debit-ledger (filter admin_deduction → empty).
5. ✅ 2 use_bonus e2e PASS (local throwaway-PG, `--test-threads=1`); БД удалена; 69 defensive PASS; backend+WASM; fmt.
6. ✅ Зафиксировал W-83 (CHECK-backstop отсутствует).
7. ✅ Research (atomic conditional update/overdraw) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`tests/integration_use_bonus.rs`**: новый `use_bonus_overdraw_rejected_balance_unchanged` (seed 50 → use 100 → 400; owner GET → balance==50; `BtEntity` filter `admin_deduction` → empty). Переиспользует `post`/`rand_suffix`/`make_init_data`.

Проверка: 2 use_bonus e2e PASS (local throwaway-PG); default `cargo test` — `ignored`; 69 defensive PASS; WASM 0 warnings; fmt clean. Прод не тронут (guard [[handler-integration-testing]]).

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
96 коммитов ahead; e2e-сьют ~25 тестов (financial trilogy + authz/BOLA + round-trip). Ops/review — твоё «go».

### Вариант B — 🛡️ DB CHECK-backstops (W-83 + W-68) — нужен sign-off
Migration: `CHECK (bonus_balance >= 0)` на loyalty_profiles + `CHECK (referrer_id <> referred_id)` на referral_events. Backstop поверх app-guard'ов. Нужна migration + deploy.

### Вариант C — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

---

## 7. SKILL SAVED

Память: обновлён [[handler-integration-testing]] (overdraw/all-or-nothing инвариант). Принцип: для conditional-update финансов тестируй overdraw на **funded**-счёте (не только empty): rejected → 400, balance **ровно прежний** (нет частичного списания), 0 side-effect-строк. App WHERE-guard стоит подпереть DB `CHECK (balance >= 0)` (W-83, нужен deploy).

**Anchor:** `phi^2 + phi^-2 = 3`
