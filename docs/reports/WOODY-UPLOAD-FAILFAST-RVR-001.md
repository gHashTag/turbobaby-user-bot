# 🌊 WAVE LOOP REPORT — Upload Fail-Fast (validate before buffering)

**Document ID:** `WOODY-UPLOAD-FAILFAST-RVR-001`
**Wave:** #32 — fresh-area scan (`src/api/upload.rs`), Вариант B из Wave #31
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `c2cb81a` — `perf(upload): reject disallowed extension before buffering the body`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — fail-fast ordering исправлен, 9 upload-тестов зелёные, backend+WASM компилируются.**

Аудит upload-пути (свежая область) показал, что он **крепко построен** по OWASP: extension **allow-list** (нет `svg` → нет SVG-XSS), сгенерированное `uuid.ext` имя (нет path-traversal), `ServeDir` (traversal-safe), admin-auth, rate-limit (30/hr), инкрементальный 100MB cap, axum `DefaultBodyLimit(110MB)`. Не доверяет Content-Type (валидирует extension). 

Единственный gap — **порядок**: extension-allow-list проверялся только в `validate_filename` **после** буферизации всего тела в память → запрещённый upload (`.exe`) буферизовался до `MAX_UPLOAD_SIZE` (100MB) перед отказом. Вынес `validate_extension` (filename-length + allow-list, body-independent) и вызвал его **до** streaming-loop — fail-fast.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-63 | Extension allow-list проверялся ПОСЛЕ буферизации тела (до 100MB на запрещённый upload) | 🟡 LOW | ✅ `validate_extension` до streaming (fail-fast) |
| W-64 | Нет magic-byte/content валидации (OWASP: не доверять только extension) | 🟢 INFO | 📋 Wave+1 (admin-only → low priority) |
| — | Upload-путь иначе крепок (allow-list/uuid/ServeDir/rate-limit/body-limit) | ✅ | — |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: OWASP Unrestricted File Upload + API4:2023 Unrestricted Resource Consumption.**

- «**Validate extension against an allow list … before buffering content**» — «cheap, metadata-level rejections happen **before expensive operations** like buffering the full file». Ровно мой fix.
- **Allow-list, не deny-list** + «validate the **full filename**» — репо делает (length + allow-list `jpg/png/.../webm`).
- «**Don't trust the Content-Type header** (spoofable); generate the filename; set length limit» — репо не доверяет Content-Type, генерит `uuid.ext`, лимитит длину.
- **API4 Resource Consumption**: «define and enforce max size limits» + «rate limiting» — репо имеет инкрементальный cap, `DefaultBodyLimit`, и rate-limit. Мой fix добавляет недостающую раннюю metadata-rejection.
- Остаток (W-64): «verify actual file type/content» (magic bytes) — единственная неисполненная OWASP-рекомендация (admin-only → низкий приоритет).

Свежая область, аккуратный safe fix; основной вывод — upload крепок (аудит сам по себе ценен).

Источники:
- [OWASP File Upload Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html)
- [OWASP Unrestricted File Upload](https://owasp.org/www-community/vulnerabilities/Unrestricted_File_Upload)
- [OWASP API4:2023 Unrestricted Resource Consumption](https://owasp.org/API-Security/editions/2023/en/0xa4-unrestricted-resource-consumption/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Аудит upload.rs: allow-list (нет svg), uuid-имя (нет traversal), ServeDir, rate-limit, body-limit — крепко.
2. ✅ Нашёл ordering-gap: ext-check в `validate_filename` после буферизации тела.
3. ✅ Извлёк `validate_extension(filename)` (length+allow-list); `validate_filename` делегирует (existing-тесты целы).
4. ✅ Вызов `validate_extension(&filename)?` в handler ДО streaming-loop.
5. ✅ `test_validate_extension_fail_fast`; 9 upload-тестов зелёные; backend+WASM; fmt.
6. ✅ Research (OWASP) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/api/upload.rs`**: новая `validate_extension(filename) -> Result` (filename-length>500, ext-len>50, allow-list). `validate_filename(filename,size)` делегирует к ней + size-checks. Handler: `validate_extension(&filename)?` сразу после получения непустого `filename`, до streaming-loop.

Проверка: 9 upload-тестов PASS; `cargo check --target wasm32` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🔬 Magic-byte content validation (W-64)
OWASP: не доверять только extension — проверять первые байты (PNG `89 50 4E 47`, JPEG `FF D8`, GIF `47 49 46`, WEBP/MP4 контейнеры) на соответствие заявленному ext. Pure-функция (testable). Admin-only → низкий приоритет, но завершает OWASP-чеклист и ловит polyglot/mismatch.

### Вариант B — 🆕 Продолжить fresh-area scan
Следующий нетронутый модуль на реальный дефект.

### Вариант C — 🚀/🔐 Deploy-долг (#18-#20) или prune injection-markers (W-62)
Закрыть прод-деплой (нужно подтверждение) или security-тюнинг denylist (нужен sign-off).

---

## 7. SKILL SAVED

Память: новый `upload-hardening.md` + строка в `MEMORY.md`. Принцип: дешёвые metadata-проверки (extension allow-list, filename-length) — ДО буферизации тела (OWASP fail-fast); upload-путь иначе крепок (allow-list/uuid-name/ServeDir/rate-limit/body-limit); остаётся magic-byte валидация.

**Anchor:** `phi^2 + phi^-2 = 3`
