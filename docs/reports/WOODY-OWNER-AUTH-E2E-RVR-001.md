# 🌊 WAVE LOOP REPORT — E2E check_owner Gate with Real Forged initData (BOLA)

**Document ID:** `WOODY-OWNER-AUTH-E2E-RVR-001`
**Wave:** #47 — e2e-покрытие auth-слоя (Вариант B из Wave #46)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `2724db1` — `test(auth): e2e check_owner gate with real forged Telegram initData`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — `check_owner` (owner-gate всех user-data эндпоинтов) покрыт e2e реальной подписанной initData; reusable `make_init_data` хелпер; 3 e2e + 69 defensive зелёные.**

Реализовал **horizontal-BOLA негатив-тест** для `check_owner` — самый часто пропускаемый класс. Добавил reusable хелпер `common::make_init_data(user_id, bot_token)`, который **forge'ит ВАЛИДНУЮ подписанную Telegram initData** (точная репликация HMAC из `validate_init_data`: secret=HMAC("WebAppData", token); hash=HMAC(secret, sorted decoded `k=v`)). Это тестирует **реальный auth-пайплайн** (не bypass) и разблокирует все owner-gated e2e.

Тесты (через живой Router, `GET /api/loyalty/:id`): (a) initData-user == path-id → **200** + профиль; (b) initData-user != path-id (атакующий лезет к чужим данным) → **403** (horizontal BOLA); (c) без initData → **401**. Профиль засеян через admin `add_bonus` (all-API). Прохождение accept-теста доказывает, что HMAC-подпись совпала с `validate_init_data` байт-в-байт.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-81 | `check_owner` (owner-gate) без runtime e2e (только статика/рассуждение) | 🟢 INFO | ✅ e2e: accept 200 / BOLA 403 / no-auth 401 |
| — | Нет хелпера для подписанной initData → owner-gated e2e невозможны | — | ✅ reusable `make_init_data` (real HMAC) |
| — | Auth/owner-логика сама по себе корректна (подтверждено e2e) | ✅ | — |
| 🚨 | Прод-фиксы #18-#20 + PR ветки (96 коммитов ahead) | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: тестируй auth-middleware подписанным токеном (не bypass); IDOR/BOLA негатив; матрица, не happy-path-only.**

- **Тестируй реальный пайплайн настоящим подписанным токеном**: «issue genuine signed tokens rather than mocking auth away … test the whole thing». Ровно `make_init_data` (real HMAC, не bypass).
- **Критический негатив — BOLA/IDOR**: «authenticate as User A, access User B's resource, assert 403/404. A 200 for another user's object indicates BOLA». Мой mismatch-тест = **horizontal BOLA**.
- **Матрица, не happy-path-only**: happy 200 / no-token 401 / bad-token 401 / horizontal-IDOR 403. Покрыл 3 из 4 (accept/no-token/IDOR).
- **Deny-by-default / fail-secure**: «authentication passing tells you nothing about whether object-level access control works». `check_owner` валидирует object-level (user.id == expected) — подтверждено e2e.

Источники:
- [Integration testing JWT authenticated APIs — inject signing key, sign own tokens (Frodehus)](https://www.frodehus.dev/integration-testing-jwt-authenticated-apis/)
- [Testing permission-protected API endpoints (Joao Grassi)](https://blog.joaograssi.com/posts/2021/asp-net-core-testing-permission-protected-api-endpoints/)
- [OWASP WSTG — Testing for IDOR](https://owasp.org/www-project-web-security-testing-guide/latest/4-Web_Application_Security_Testing/05-Authorization_Testing/04-Testing_for_Insecure_Direct_Object_References)
- [Horizontal IDOR/BOLA (Invicti)](https://www.invicti.com/web-application-vulnerabilities/horizontal-idor-bola-broken-object-level-authorization)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Цель: e2e owner-gated фикса. Блокер: `check_owner` требует подписанную initData (не admin-token).
2. ✅ Прочитал `validate_init_data` HMAC-алгоритм; подтвердил, что hmac/sha2/hex/urlencoding — прямые deps (доступны тест-крейту).
3. ✅ `make_init_data(user_id, bot_token)` в `common/mod.rs` (реплика secret/hash; user url-encoded).
4. ✅ `integration_owner_auth.rs`: accept (200+profile) / mismatch (403, BOLA) / missing (401); seed через admin add_bonus.
5. ✅ 3 e2e PASS против локальной throwaway-PG (`--test-threads=1`); БД удалена; 69 defensive PASS; backend+WASM; fmt.
6. ✅ Research (auth-testing/BOLA) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`tests/common/mod.rs`**: `pub fn make_init_data(user_id: i64, bot_token: &str) -> String` — secret=HMAC-SHA256("WebAppData", bot_token); hash=HMAC-SHA256(secret, `auth_date={now}\nuser={json}`); возвращает `user=<urlenc>&auth_date=<now>&hash=<hex>`.
- **`tests/integration_owner_auth.rs`** (`#[ignore]`): `add_bonus_as_admin` хелпер (seed); 3 теста (accept/mismatch/missing) на `GET /api/loyalty/:id`.

Проверка: 3 e2e PASS (local throwaway-PG); accept-тест доказывает HMAC-совпадение; default `cargo test` — `ignored`; 69 defensive PASS; WASM 0 warnings; fmt clean. Прод не тронут (guard [[handler-integration-testing]]).

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
96 коммитов ahead, e2e-валидированы. Ops/review — твоё «go».

### Вариант B — 🧪 e2e use_reward happy-path (W-72.1)
Теперь `make_init_data` готов → owner-gated `POST /api/garden/rewards/:id/use`: seed reward, redeem, assert bonus credited + idempotent replay.

### Вариант C — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

---

## 7. SKILL SAVED

Память: обновлён [[handler-integration-testing]] (`make_init_data` + BOLA-матрица). Принципы: тестируй auth-middleware **настоящим подписанным** токеном (не bypass); всегда добавляй BOLA/IDOR негатив (User A → ресурс User B → 403), не только happy-path; матрица accept/no-token/bad-token/IDOR.

**Anchor:** `phi^2 + phi^-2 = 3`
