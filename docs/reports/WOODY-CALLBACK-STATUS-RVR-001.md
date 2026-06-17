# 🌊 WAVE LOOP REPORT — Bot Callbacks: Fail Loud on Corrupt Order Status

**Document ID:** `WOODY-CALLBACK-STATUS-RVR-001`
**Wave:** #44 — fresh-area scan (Вариант B из Wave #43) → asymmetric-default дефект в `src/bot/callbacks.rs`
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `d2749c6` — `fix(callbacks): fail loud on corrupt order status in confirm/reject flows`
**Agent:** Claude Opus 4.8 (Wave loop) + Explore-subagent

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — asymmetric-default refund-риск устранён; 15 callbacks + 69 defensive зелёные; backend+WASM.**

Fresh-area scan нашёл реальный дефект в bot-слое: confirm/reject-обработчики заказа (`bot/callbacks.rs`) читали статус заказа через `.unwrap_or_default()`. Ключ — **асимметрия направления дефолта**: `can_confirm_order("")` → `false` (fail-safe: не подтвердит), но `should_refund_bonus("")` → **`true`** (fail-unsafe: на пустом/неизвестном статусе refund **срабатывает**). При сбое чтения статуса reject-путь начислил бы бонусный refund И флипнул заказ в `rejected` на **неизвестном** состоянии — риск двойного refund на уже отклонённом заказе.

Фикс: обе читки статуса теперь **fail loud**. Reject — на сбое чтения `refund_ok=false` → ни refund, ни флип (tx откатывается). Confirm — лог вместо тихого «не подтверждаемо» (направление и так безопасно). + hazard-guard тест, фиксирующий что `should_refund_bonus("")==true` (документирует, почему чтение обязано быть fail-loud).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-79 | reject: `status.unwrap_or_default()` + `should_refund_bonus("")==true` → refund/флип на неизвестном статусе (двойной refund) | 🟠 MED (financial) | ✅ fail loud → `refund_ok=false` на сбое чтения (abort) |
| W-79b | confirm: `status.unwrap_or_default()` тихо трактует сбой как «не подтверждаемо» | 🟢 LOW | ✅ лог на сбой (направление safe) |
| — | Прочее (notify, cache, mod, web, macros) — clean (parameterized SQL, html_escape, bounds) | ✅ | — |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: fail-safe defaults; асимметрия направления дефолта; неизвестное состояние трактуй консервативно; double-refund.**

- **Saltzer & Schroeder, fail-safe defaults**: «mistake in a mechanism that **gives** permission tends to fail by **refusing** (safe, quickly detected); mistake that **excludes** tends to fail by **allowing** (unnoticed)». Ровно: `can_confirm("")→false` (safe) vs `should_refund("")→true` (unsafe). Опасное направление — refund.
- **arc42 fail-safe defaults**: «treat all unrecognized inputs/conditions as **denied/invalid** by default; on anomaly transition to a safe state» — явно называет **financial transaction processing**. Reject теперь идёт в safe-state (do nothing) на неизвестном статусе.
- **Corrupt-read ≠ legit value**: «a null from the DB due to a connectivity blip is different from an erroneous null in a request» — обрабатывай отдельно (fail loud), не схлопывай в дефолт.
- **Double-refund prevention**: «treat the operation as already-done; reject ambiguous in-flight; terminal states authoritative». Пустой статус — НЕ терминальный → нельзя считать «надо refund».

Источники:
- [Security Principles of Saltzer and Schroeder — fail-safe defaults (Shostack)](https://shostack.org/blog/the-security-principles-of-saltzer-and-schroeder)
- [Fail-Safe Defaults pattern (arc42 Quality Model)](https://quality.arc42.org/approaches/fail-safe-defaults)
- [Avoiding double payments in a distributed payments system (Airbnb Eng)](https://medium.com/airbnb-engineering/avoiding-double-payments-in-a-distributed-payments-system-2981f6b070bb)
- [Prevent Double Refund Chargebacks (Chargebacks911)](https://chargebacks911.com/double-refund-chargebacks/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Fresh-area scan (Explore) по notify/cache/mod/web/macros/callbacks → дефект: status `.unwrap_or_default()` в callbacks.
2. ✅ Подтвердил асимметрию: `can_confirm("")→false` (safe), `should_refund("")→true` (unsafe → опасное направление).
3. ✅ Reject: status через `match`; на `Err` → лог + `refund_ok=false` (ни refund, ни флип; tx rollback); gate `refund_ok && should_refund_bonus(...)`.
4. ✅ Confirm: status через `match`; на `Err` → лог + `""` (can_confirm("")→false, safe).
5. ✅ Hazard-тест `should_refund_bonus_is_dangerously_true_for_unknown_status`.
6. ✅ 15 callbacks + 69 defensive PASS; backend+WASM; fmt.
7. ✅ Research (fail-safe defaults) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/bot/callbacks.rs`**:
  - reject-флоу: `let current_status = match r.try_get::<String>("","status") { Ok(s)=>s, Err(e)=>{ error!; refund_ok=false; String::new() } };` затем `if refund_ok && should_refund_bonus(&current_status) { … }`.
  - confirm-флоу: `let status = match row.try_get::<String>("","status") { Ok(s)=>s, Err(e)=>{ error!; String::new() } };`.
  - тест `should_refund_bonus_is_dangerously_true_for_unknown_status`.

Проверка: backend build OK; `cargo test … callbacks` → 15 PASS; defensive → 69 PASS; WASM 0 warnings; fmt clean. Honest scope: callback-handler полностью требует teloxide-runtime + DB, runtime-тест не добавлен; pure-предикат покрыт hazard-тестом.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🆕 Fresh-area scan
Следующий нетронутый модуль / новый класс дефектов.

### Вариант B — 🧪 e2e-покрытие fail-loud/authz фиксов (harness готов)
Написать integration-тесты (local throwaway-PG) для self-referral (W-67), use_reward (W-72.1), quest fake-id (W-77).

### Вариант C — 🚀/🛡️ Накопленный долг — нужен sign-off
**PR ветки `fix/profile-routing` (#35-#44, ~22 коммита) в `main`** + deploy #18-#20. Ops/review.

---

## 7. SKILL SAVED

Память: обновлён [[fail-loud-not-silent-clamp]] (asymmetric-default-direction урок). Принцип: при дефолте на enum/status, выбирай направление по **fail-safe** (Saltzer-Schroeder): ошибка должна проваливаться в *отказ/no-op*, не в *действие* (особенно финансовое: refund/grant). Если предикат возвращает «надо действовать» на неизвестном значении (`should_refund("")==true`), чтение этого значения ОБЯЗАНО быть fail-loud. Corrupt-read ≠ легитимное значение.

**Anchor:** `phi^2 + phi^-2 = 3`
