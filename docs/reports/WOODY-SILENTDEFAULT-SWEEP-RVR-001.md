# 🌊 WAVE LOOP REPORT — Silent-Default DB-Read Sweep (Garden Rewards)

**Document ID:** `WOODY-SILENTDEFAULT-SWEEP-RVR-001`
**Wave:** #39 — sweep класса silent-default (Вариант B из Wave #38) → фикс reward-кластера
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `1cca822` — `fix(garden): fail loud on corrupt reward fields; unify reward-active boundary`
**Agent:** Claude Opus 4.8 (Wave loop) + Explore-subagent (triage)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — опасный reward-кластер исправлен; boundary унифицирован; 43 garden + 69 defensive зелёные; backend+WASM.**

Системный sweep класса W-72 («тихий дефолт на чувствительном DB-значении маскирует corruption»): 169 `try_get().unwrap_or()` сайтов, триаж агентом отделил косметику от опасных. Самый опасный — **финансовая мутация** `use_reward` (`api/garden.rs`): `bonus_points`/`discount_percent` читались с `.unwrap_or(0)` → при сбое чтения пользователь **тратит** награду (`is_used=true`) и получает **ноль** бонусов/скидки, без downstream-guard'а. Плюс найдена off-by-one несогласованность границы: список наград считает активность по `expires_at > now`, а `use_reward` — по `expires_at < now` (расхождение ровно при `== now`).

Фиксы: (1) `use_reward` — **fail loud** (propagate `DbErr`→500) на всех 4 чтениях reward-строки (tx ещё не начат → ничего не тратится), как эталон в `loyalty.rs`; (2) извлёк pure `reward_is_active(is_used, expires_at, now)` — единый источник истины для gate, оба пути теперь согласованы по границе. Остальные кандидаты (orders count, fraud count, tech_tree) задокументированы как W-74 follow-up (ограничил scope одним когерентным кластером).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-72.1 | `use_reward`: `bonus_points`/`discount_percent` `.unwrap_or(0)` → reward потрачен, начислено 0 | 🟠 MED (financial, value loss) | ✅ fail loud (propagate DbErr→500) на 4 reward-чтениях |
| W-72.2 | reward-active boundary off-by-one: список `> now` vs use_reward `< now` (расходятся при `==now`) | 🟡 LOW (correctness) | ✅ pure `reward_is_active` — единый источник истины |
| W-74 | Остаток sweep: `orders.rs:145` count→is_first, `orders.rs:793` fraud-count→auto-block, `tech_tree.rs` xp/priority `.unwrap_or(0)` | 🟢 INFO | 📋 follow-up (вне scope этого Wave) |
| — | `garden_config` дефолты (10%/100/7d) — намеренные product-fallback'и | ✅ | не трогаю |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: fail-loud на read-ошибке (не default-zero) в финансах; off-by-one на timestamp; single source of truth.**

- **Default-zero на сбое чтения — silent-failure анти-паттерн**: «instead of halting or flagging the transaction, its default exception handler may **assign a zero value** … silently introduces inaccurate data that skews reporting». Ровно `bonus_points.unwrap_or(0)` → начислили 0.
- **«Silent data loss in its pure form»**: повреждённое чтение, возвращающее 0 без ошибки и без следа в логах — самый опасный класс. Финансовая мутация обязана **fail loud**.
- **Off-by-one на timestamp**: «non-strict (≤) where strict (<) should be used … inclusive vs exclusive end dates … transaction at an exact boundary double-counted or dropped». Унификация `reward_is_active` (`> now`) убирает расхождение при `== now`.
- **Single source of truth**: один предикат gate вместо двух разошедшихся копий (dedup, [[lang-code-clone-eliminated]] класс).

Источники:
- [Silent Failure Modes in Financial Technology (Abyrint)](https://www.abyrint.com/perspectives/silent-failures-tech-wrong-unnoticed/)
- [Off-by-one error (Wikipedia)](https://en.wikipedia.org/wiki/Off-by-one_error)
- [Off-by-One Error (Baeldung CS)](https://www.baeldung.com/cs/off-by-one-error)
- [Single source of truth (Wikipedia)](https://en.wikipedia.org/wiki/Single_source_of_truth)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Sweep: 169 `try_get().unwrap_or()` сайтов; триаж агентом (sensitive vs cosmetic; эталон — `loyalty.rs`).
2. ✅ Выбрал highest-severity когерентный кластер: garden rewards (финансовая мутация + парный read-путь).
3. ✅ Pure `reward_is_active(is_used, expires_at, now)` в ядре + boundary-тест (==now → false).
4. ✅ `get_user_rewards.is_active` и `use_reward` expiry-check → через `reward_is_active` (единая граница).
5. ✅ `use_reward`: 4 чтения reward-строки → `map_err(...→500)` (fail loud; tx не начат).
6. ✅ 43 garden-теста + 69 defensive PASS; backend+WASM; fmt. Остаток → W-74.
7. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/trios/garden.rs`**: `pub fn reward_is_active(is_used: bool, expires_at: i64, now: i64) -> bool` + `test_reward_is_active_boundary_and_states`.
- **`src/api/garden.rs`**:
  - `get_user_rewards`: `is_active: garden::reward_is_active(is_used, expires_at, now)`.
  - `use_reward`: локальная `read_err(field)`; `is_used`/`expires_at`/`discount_percent`/`bonus_points` через `.map_err(read_err(...))?`; expiry-check через `!reward_is_active(...)` (граница `<= now`, согласована со списком).

Проверка: `cargo test … garden` → 43 PASS; defensive-tests → 69 PASS; WASM check (garden — shared core) → 0 warnings; fmt clean. Honest scope: handler-level тест не добавлен (нет harness, [[integration-test-harness-blocker]]); pure `reward_is_active` покрыт.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🧹 Завершить sweep W-74
`orders.rs:145` (count→is_first: ошибочный «первый заказ» → tier/referral-бонус), `orders.rs:793` (fraud-count→auto-block: тихий 0 = fraud-gate отключён), `tech_tree.rs` xp/priority. Тот же fail-loud принцип; завершает класс.

### Вариант B — 🆕 Fresh-area scan
Следующий нетронутый модуль.

### Вариант C — 🚀/🛡️ Накопленный долг — нужен sign-off
Deploy #18-#20 в `main`, либо DB CHECK self-ref (W-68). Ops/migration.

---

## 7. SKILL SAVED

Память: обновлён `fail-loud-not-silent-clamp.md` (добавлен financial-mutation reward-кейс + boundary-unification + W-74 остаток). Принцип расширен: на финансовой **мутации** default-zero при сбое чтения = silent value loss → propagate; off-by-one `< vs <=` на timestamp лечится единым pure-предикатом (single source of truth).

**Anchor:** `phi^2 + phi^-2 = 3`
