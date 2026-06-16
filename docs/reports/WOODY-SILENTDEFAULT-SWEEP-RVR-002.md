# 🌊 WAVE LOOP REPORT — Silent-Default Sweep Completion (orders.rs COUNT reads)

**Document ID:** `WOODY-SILENTDEFAULT-SWEEP-RVR-002`
**Wave:** #40 — завершение sweep W-74 (Вариант A из Wave #39)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `f10c0ac` — `fix(orders): fail loud on corrupt COUNT reads (is_first + fraud auto-block)`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — sweep класса silent-default закрыт; 79 orders + 69 defensive зелёные; backend+WASM.**

Завершил начатый в #39 sweep (W-74). Две чувствительные `COUNT(*)` читки в `db/orders.rs` использовали `.unwrap_or(0)`:
1. **`complete_order_and_update_loyalty:145`** — `count_before` (число прошлых завершённых заказов). Тихий 0 при сбое чтения → существующий клиент помечается «первый заказ» → ошибочное first-order tier-повышение + referral-бонус.
2. **`auto_block_for_fraud:793`** — `count` фрод-событий. Тихий 0 → `should_auto_block_for_fraud(0)=false` → security-гейт **молча fail-open**: фродстер проходит порог.

Обе функции уже возвращают `DbErr` и используют `?`, так что замена на `try_get(...)?` тривиально пропагирует (fail loud; для #1 — внутри tx → авто-rollback). **`tech_tree.rs` переоценён честно: это read-only roadmap-display** (xp/priority не участвуют в economy-логике сервера) → не трогаю (падать всем списком из-за одного NULL хуже). Класс багов закрыт.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-74.1 | `complete_order…:145` count→is_first `.unwrap_or(0)`: existing → «первый заказ» → tier/referral | 🟠 MED (financial) | ✅ `try_get(...)?` (fail loud, в tx) |
| W-74.2 | `auto_block_for_fraud:793` count `.unwrap_or(0)`: fraud-gate fail-open | 🟠 MED (security) | ✅ `try_get(...)?` (fail loud) |
| W-74.3 | `tech_tree.rs` xp/priority `.unwrap_or(0)` | — | ✅ переоценён: display-only, не economy → корректно как есть |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

Класс W-72/W-74 (silent-default на чувствительных DB-чтениях): **закрыт** (garden water_count #38, garden rewards #39, orders COUNT #40).

---

## 3. НАУЧНАЯ БАЗА

**Тема: fail-open vs fail-closed (default-deny) security-гейт; idempotent first-order detection.**

- **Fail-open security check — OWASP анти-паттерн**: «all security mechanisms should **deny access until specifically granted**, not grant until denied»; «well-written authorization code treats **any unexpected result as a denial**». Фрод-гейт с `unwrap_or(0)` грантил «не блокировать» при ошибке = fail-open.
- **Attacker forces the weaker fallback**: «adversaries trigger edge cases to force a weaker fallback — if a control fails open, the attacker wins by making it uncertain». Поэтому read-ошибку нельзя интерпретировать как «0 событий».
- **Честный нюанс**: мой фикс делает **fail-loud** (ошибка всплывает/логируется/обрабатывается caller'ом), а не silent fail-open. Строгий fail-closed (блок при любой неопределённости) сильнее, но рискует блокировать честных при transient DB-ошибке — это продуктовое решение, не автономное.
- **Idempotent first-order / loyalty integrity**: «check before acting; unique identifiers; repeated/retried checks don't double-process». Корректный `is_first` критичен для tier/referral; тихий 0 ломает инвариант.

Источники:
- [Improper Error Handling — fail-open security check (OWASP)](https://owasp.org/www-community/Improper_Error_Handling)
- [Failed Open vs Fail Closed (AuthZed)](https://authzed.com/blog/fail-open)
- [Fail-Secure & Fail-Safe Strategies (ITU Online)](https://www.ituonline.com/comptia-securityx/comptia-securityx-4/mitigations-implementing-fail-secure-and-fail-safe-strategies-for-robust-security/)
- [Loyalty Fraud Detection & Prevention (DataDome)](https://datadome.co/learning-center/loyalty-fraud/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Из W-74: 3 оставшихся кандидата (orders count, fraud count, tech_tree).
2. ✅ Подтвердил: обе orders-функции возвращают `DbErr` + используют `?` → `try_get(...)?` корректно пропагирует.
3. ✅ `complete_order…:145` → fail loud (в tx → rollback на ошибке).
4. ✅ `auto_block_for_fraud:793` → fail loud (silent fail-open → ошибка наружу).
5. ✅ Проверил `tech_tree.get_tech_nodes`: read-only display, нет economy-логики → честно оставил (падать списком из-за NULL хуже).
6. ✅ 79 orders + 3 fraud + 69 defensive PASS; backend+WASM; fmt.
7. ✅ Research (fail-open/closed) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/db/orders.rs`**: `count_before: i64 = count_row.try_get("", "cnt")?;` (было `.unwrap_or(0)`); `count: i64 = row.try_get("", "n")?;` (было `.unwrap_or(0)`). Поясняющие комментарии о tier/referral и fail-open.

Проверка: `cargo test … orders/fraud` → 79+3 PASS; defensive-tests → 69 PASS; WASM 0 warnings; fmt clean. Honest scope: handler/DB-функции не покрыты unit-тестом напрямую (нет DB-harness, [[integration-test-harness-blocker]]); правка механическая (propagate vs default), pure `should_auto_block_for_fraud` уже покрыт.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🛡️ Фитнес-тест против класса silent-default
Source-scanning тест (как `schema_drift_tests`), запрещающий `try_get(<sensitive col>).unwrap_or(...)` на allow-list финансовых/security-колонок (amount/balance/bonus/count/...). Превращает закрытый класс в постоянную защиту от регрессии. В сигнатурном стиле проекта.

### Вариант B — 🆕 Fresh-area scan
Следующий нетронутый модуль на реальный дефект.

### Вариант C — 🚀/🛡️ Накопленный долг — нужен sign-off
Deploy #18-#20 в `main`, либо DB CHECK self-ref (W-68). Ops/migration.

---

## 7. SKILL SAVED

Память: обновлён `fail-loud-not-silent-clamp.md` (W-74 закрыт; добавлен fail-open/closed нюанс для security-гейтов). Принцип: security-гейт никогда не должен молча fail-open на read-ошибке (OWASP: unexpected result = denial); финансовый «первый раз» инвариант (tier/referral) — fail loud, не default-0. Различай fail-loud (ошибка наружу) и строгий fail-closed (блок при неопределённости — продуктовое решение).

**Anchor:** `phi^2 + phi^-2 = 3`
