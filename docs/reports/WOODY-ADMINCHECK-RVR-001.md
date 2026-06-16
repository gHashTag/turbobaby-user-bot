# 🌊 WAVE LOOP REPORT — /admin/check Must Not Echo Unverified Identity

**Document ID:** `WOODY-ADMINCHECK-RVR-001`
**Wave:** #36 — fresh-area scan (Вариант B из Wave #35) → дефект в `src/api/admin.rs`
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `d23a2e9` — `fix(admin): /admin/check token path must not echo unverified telegram_id`
**Agent:** Claude Opus 4.8 (Wave loop) + Explore-subagent

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — API-контракт identity исправлен; 19 admin/auth-тестов + 69 defensive зелёные.**

Fresh-area scan по крупнейшим нетронутым модулям (`orders.rs` 2022, `db/orders.rs` 1507, `quest.rs` 1080, `admin.rs`, `auth.rs`) дал один дефект: `check_admin_access` (`/admin/check`, `admin.rs:438-445`) на password-token-пути возвращал **клиентский** `query.telegram_id` как «аутентифицированную личность», тогда как initData-ветка (и сам `check_admin`) используют **проверенный** `user.id` / sentinel `0`.

**Честная оценка серьёзности — LOW (не privilege escalation).** Реальные authz-гейты клиентский id не используют: `check_admin` на token-пути возвращает `0` (sentinel), `check_owner` валидирует только через initData (`user.id == expected`). Эндпоинт требует валидный токен (= знание пароля = легитимный админ). Это **латентный contract/defense-in-depth баг**: непроверенный ввод преподносился как identity, и будущий потребитель, доверившись полю `telegram_id`, получил бы баг. Фронт сейчас читает только `is_admin` → поведение не меняется.

Фикс: на token-пути возвращаем `telegram_id: 0` (как `check_admin`), а не эхо. Принцип OWASP A01: identity берётся из того, чему сервер криптографически доверяет, не из данных клиента.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-69 | `/admin/check` token-путь эхал непроверенный `query.telegram_id` как identity | 🟡 LOW (contract/defense-in-depth, OWASP A01) | ✅ возвращаем sentinel `0` (как `check_admin`) |
| — | `orders.rs`, `db/orders.rs`, `quest.rs`, `auth.rs` (primitives) — authz/идемпотентность/bonus-guards на месте, clean | ✅ | — |
| W-70 | Нет handler-level теста на `check_admin_access` (нет harness для State+HeaderMap) | 🟢 INFO | 📋 см. [[integration-test-harness-blocker]] |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: OWASP A01:2025 Broken Access Control + API2 Broken Authentication — identity из проверенного источника.**

- **Ядро**: «identity and entitlements must come from something the server **cryptographically trusts — never from data the client controls**». Token-путь возвращал client-controlled id → нарушение принципа (хоть и не эксплуатируемое здесь).
- **Confused deputy / IDOR**: сервер действует по client-supplied идентификатору без подтверждения. Наш `check_admin`/`check_owner` это НЕ делают (используют `0`/initData) — поэтому только сам `/admin/check`-ответ был неаккуратен.
- **Authn vs Authz**: «authentication answers who are you; authorization what you can do». `/admin/check` отвечает на authn-вопрос — и должен сообщать только проверенную личность (или её отсутствие = `0`).
- **Token verification at every boundary**: каждый сервис сам проверяет токен, не доверяет upstream. Репо так и делает — `check_admin` верифицирует на каждом запросе; фикс выравнивает identity-отчёт `/admin/check` с этой дисциплиной.

Источники:
- [OWASP Top 10:2025 A01 Broken Access Control](https://owasp.org/Top10/2025/A01_2025-Broken_Access_Control/)
- [OWASP API2:2023 Broken Authentication](https://owasp.org/API-Security/editions/2023/en/0xa2-broken-authentication/)
- [OWASP Authorization Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html)
- [OWASP API1:2023 Broken Object Level Authorization](https://owasp.org/API-Security/editions/2023/en/0xa1-broken-object-level-authorization/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Fresh-area scan (Explore) по `orders/db.orders/quest/admin/auth` → дефект: `/admin/check` эхает client id.
2. ✅ Оценил достижимость/импакт: `check_admin`→`0`, `check_owner`→initData; фронт читает только `is_admin` → LOW, latent contract bug (не escalation). Честно понизил severity относительно отчёта агента.
3. ✅ Фикс: token-путь возвращает `telegram_id: 0` (sentinel, как `check_admin`) + поясняющий комментарий.
4. ✅ `query.telegram_id` всё ещё используется (валидация + debug-лог) → нет unused-warning.
5. ✅ Build backend; 19 admin/auth-тестов PASS; defensive 69 PASS; WASM 0 warnings; fmt.
6. ✅ Research (OWASP A01) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/api/admin.rs`** (`check_admin_access`, token-ветка): `json!({ "is_admin": true, "telegram_id": query.telegram_id })` → `json!({ "is_admin": true, "telegram_id": 0 })`; лог `token authenticated (no specific telegram_id)`; комментарий о sentinel-согласовании с `check_admin` и о том, что фронт читает только `is_admin`.

Проверка: backend build OK; `cargo test … admin` → 19 PASS; defensive-tests → 69 PASS; WASM 0 warnings; fmt clean. Поведение фронта не меняется (только `is_admin` потребляется, `admin_screen.rs:505`).

Честно: handler-level тест на `check_admin_access` не добавлен — нет harness для конструирования `State<AppState>`+`HeaderMap` (см. [[integration-test-harness-blocker]]); фикс защищён ревью + согласованием с покрытыми тестами `verify_admin_token`.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🆕 Продолжить fresh-area scan
Следующий нетронутый модуль (`trios/garden.rs`, `trios/store.rs`, `trios/quest.rs`, `db/strains.rs`, `s3.rs`) на реальный дефект.

### Вариант B — 🧪 Handler-test harness (W-70) — крупная задача
Решить [[integration-test-harness-blocker]] (lib.rs WASM-only → tests/ не видят backend), чтобы покрыть authz-handler'ы (`check_admin_access` и др.) end-to-end. Структурная переработка, оценить scope.

### Вариант C — 🚀 Deploy-долг (#18-#20) / DB CHECK self-ref (W-68) — нужен sign-off
Ops/migration-действия, требуют подтверждения пользователя.

---

## 7. SKILL SAVED

Память: новый `admin-identity-from-verified-source.md` + строка в `MEMORY.md`. Принцип: identity в ответе/решении бери из криптографически проверенного источника (initData/верифицированный токен), НЕ из клиентского параметра — даже на «безобидном» authn-ответе; согласуй identity-семантику между параллельными auth-ветками (sentinel `0` для shared-password). OWASP A01.

**Anchor:** `phi^2 + phi^-2 = 3`
