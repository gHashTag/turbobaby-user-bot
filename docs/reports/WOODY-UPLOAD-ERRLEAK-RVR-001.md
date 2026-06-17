# 🌊 WAVE LOOP REPORT — Generic Upload Error Responses (no internals leak)

**Document ID:** `WOODY-UPLOAD-ERRLEAK-RVR-001`
**Wave:** #56 — sensitive-data-leakage fresh-area scan (Вариант B из Wave #55)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `d923fdf` — `fix(upload): return generic error messages, don't leak internals to client`
**Agent:** Claude Opus 4.8 (Wave loop) + Explore-subagent

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — внутренние ошибки больше не утекают в response-тело; 11 upload + 69 defensive зелёные.**

Скан утечки чувствительных данных. **Хорошая новость**: секреты/PII в логах **чисты** (auth.rs логирует `.len()`/`.is_some()`, не значения токенов; нет bot_token/api-key/phone в логах). Один реальный дефект: `/api/upload` возвращал **внутренние детали ошибок** в теле HTTP-ответа — raw AWS SDK error (s3), multipart parse error, chunk read error, local filesystem write error (путь/permissions) — через `format!("...: {}", e)`. Каждый уже логировался server-side (`tracing::error!`), так что клиентская копия лишь утекала infra-внутренности (information disclosure).

**Честная серьёзность — LOW**: эндпоинт **admin-only** (`check_admin`), disclosure доверенному админу (агент завысил до «highest/A01»). Но это реальная непоследовательность — timeout-ветка уже отдаёт generic `&str`. Сделал generic все 4 client-сообщения; server-side логи сохранили детали.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-91 | `/api/upload` отдавал raw error (s3/multipart/IO/fs-path) в response-тело | 🟡 LOW (info-disclosure, admin-only) | ✅ generic client-msg ×4; детали только в server-log |
| — | Секреты/PII в логах (auth/config/ai/s3) — чисто (логируют .len()/.is_some()) | ✅ | подтверждено |
| — | Прочие handler'ы маппят error→StatusCode + server-log (не утекают) | ✅ | — |
| 🚨 | PR ветки (104 ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: OWASP improper error handling / information disclosure; generic-to-client, detailed-to-logs.**

- **Не отдавай raw exception клиенту**: «Never include raw exception messages or stack traces in API responses. Provide **generic messages to clients and log full details server-side**». Ровно мой фикс.
- **Почему опасно**: «a stack trace might show a malformed SQL, the DB type, the container version … enables targeting known vulns» — reconnaissance. (Тут — AWS endpoint/SDK detail, fs-path.)
- **Correlation-ID паттерн**: «client receives a generic message + a unique error_id; details in server log» — возможное усиление (server-log уже есть; error_id — follow-up).
- **Microsoft CA3004**: «Don't output exception information to HTTP responses; provide a generic error message».

Источники:
- [Improper Error Handling (OWASP)](https://owasp.org/www-community/Improper_Error_Handling)
- [Error Handling Cheat Sheet (OWASP)](https://cheatsheetseries.owasp.org/cheatsheets/Error_Handling_Cheat_Sheet.html)
- [CA3004: information disclosure (Microsoft)](https://learn.microsoft.com/en-us/dotnet/fundamentals/code-analysis/quality-rules/ca3004)
- [Sensitive Data in Error Messages (InstaTunnel)](https://medium.com/@instatunnel/sensitive-data-in-error-messages-when-your-stack-traces-give-away-the-database-schema-68c93b2d1b83)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Скан logs+responses: секреты/PII в логах чисты; дефект — upload отдаёт raw error в тело.
2. ✅ Скорректировал серьёзность (admin-only → LOW, не «highest»).
3. ✅ Подтвердил: каждый сайт уже логирует детали server-side (`tracing::error!`).
4. ✅ Generic ×4: multipart→"invalid multipart request", chunk→"upload read error", s3→"s3 upload failed", local write→"file write failed".
5. ✅ `e` остаётся в server-log (нет unused-warning); 11 upload-тестов PASS; WASM; 69 defensive PASS; fmt.
6. ✅ Research (OWASP error handling) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/api/upload.rs`**: 4 `err(status, format!("...: {}", e))` → `err(status, "<generic>")`; `tracing::error!(... e)` выше сохранён в каждом. Сообщения совпадают по стилю с уже-generic timeout-веткой.

Проверка: `cargo test … upload` → 11 PASS; WASM 0 warnings; backend; defensive 69 PASS; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
104 коммита ahead. Ops/review — твоё «go».

### Вариант B — 🔭 Sweep error-leakage по всем handler'ам
Греп `format!("...: {}", e)` / `format!("{e}")` в `err(...)`/response-телах по `src/api/*` — фитнес-тест против раскрытия внутренних ошибок клиенту (как класс).

### Вариант C — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

---

## 7. SKILL SAVED

Память: обновлён [[upload-hardening]] (error-genericization). Принцип: HTTP-ответ клиенту — **generic** сообщение; детали (raw exception, SDK/IO/fs) — только в server-log (OWASP improper error handling). Даже на admin-only эндпоинте — консистентность + defense-in-depth. Correlation-id — возможное усиление.

**Anchor:** `phi^2 + phi^-2 = 3`
