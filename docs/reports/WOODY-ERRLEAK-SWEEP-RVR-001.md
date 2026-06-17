# 🌊 WAVE LOOP REPORT — Error-Leakage Sweep + Regression Pin (CWE-209)

**Document ID:** `WOODY-ERRLEAK-SWEEP-RVR-001`
**Wave:** #57 — sweep класса error-leakage (Вариант B из Wave #56)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `91e7d04` — `test(upload): regression-pin W-91 — err() must not interpolate raw errors`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — класс error-leakage подтверждён single-file + зафиксирован регресс-тестом; backend+WASM; 69 defensive + 12 upload зелёные.**

Sweep класса «raw error → client response» (после фикса #56). **Результат: класс уже чист и локализован в одном файле.** `err(status, msg: impl Into<String>)` — **единственный** client-facing String-message error-sink во всём API; остальные handler'ы возвращают голый `StatusCode` (тело не утекает), и ни одно JSON-error-тело не интерполирует ошибку. После Wave #56 ни один `err()` не содержит raw `e`.

Поэтому (правило «runtime-bug → static lint») добавил **regression-pin**: source-scan `upload_responses_never_interpolate_raw_errors` — production-часть upload.rs не должна содержать non-tracing `format!`, интерполирующий error-binding (`, e)` / `{e}` / `{:?}", e`). Это проектный аналог .NET **CA3004** / CodeQL stack-trace-exposure (CWE-209) для нашей кодовой базы.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-91 (class) | Raw error → client response: sweep → класс single-file (`err()` в upload.rs), уже чист (#56) | ✅ verified clean | ✅ regression-pin тест |
| — | Прочие api-handler'ы возвращают голый StatusCode / sanitized JSON — не утекают | ✅ | подтверждено |
| 🚨 | PR ветки (104+ ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: static-analysis против exception-in-response (CWE-209); lint новый класс; water-leak adoption.**

- **CA3004 (.NET)**: «find an exception message, stack trace, or string representation being output to an **HTTP response**» — ровно цель моего source-scan (Rust-аналог).
- **CodeQL stack-trace-exposure**: BAD = `sendError(exception)` к юзеру; GOOD = логировать только server-side. Мой инвариант: tracing-лог ok, response — generic.
- **«Weak points»**: «when some parts handle errors securely while others don't, attackers find the weak points — library/DB/external errors need the same scrutiny». → поэтому подтвердил, что `err()` — единственный sink.
- **«Runtime-bug → static lint»**: «every time you find a runtime bug, ask whether a static rule could prevent it». Ровно scoped-fitness-function ([[scoped-fitness-function-ratchet]]). CWE-209.

Источники:
- [CA3004 — exception info to HTTP response (Microsoft)](https://learn.microsoft.com/en-us/dotnet/fundamentals/code-analysis/quality-rules/ca3004)
- [CodeQL — information exposure through a stack trace](https://codeql.github.com/codeql-query-help/java/java-stack-trace-exposure/)
- [OWASP C10 — Handle all Errors and Exceptions](https://owasp-top-10-proactive-controls-2018.readthedocs.io/en/latest/c10-handle-errors-exceptions.html)
- [Info disclosure in error messages (PortSwigger)](https://portswigger.net/web-security/information-disclosure/exploiting/lab-infoleak-in-error-messages)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Греп `format!(...{e/err}...)` в `src/api/` вне tracing → только `{k}={v}`/тест-фикстуры/валидационные msg (safe), не raw error.
2. ✅ Греп JSON-error-тел с интерполяцией → нет.
3. ✅ Подтвердил: `err()` (String-msg sink) только в upload.rs (7 использований); прочие → голый StatusCode.
4. ✅ Вывод: класс single-file, уже чист (#56).
5. ✅ Regression-pin: source-scan production-части upload.rs на non-tracing `format!` с error-binding.
6. ✅ Тест PASS; backend+WASM; defensive 69 PASS; fmt.
7. ✅ Research (CA3004/CodeQL/CWE-209) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/api/upload.rs`** (`#[cfg(test)]`): `upload_responses_never_interpolate_raw_errors` — читает `src/api/upload.rs`, берёт prod-часть (до `#[cfg(test)]`), фильтрует non-`tracing::` строки с `format!` + (`, e)` | `{e}` | `{:?}", e`) → fail с перечнем. Текущий код passes; tracing-логи и валидационные msg (`{size}`/`{ext}`/`{MAX}`) не флагаются.

Проверка: тест PASS; backend+WASM compile; defensive 69 PASS; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
104+ коммита ahead. Ops/review — твоё «go».

### Вариант B — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

### Вариант C — 🛡️/🚀 Накопленный долг — нужен sign-off
DB CHECK-backstops (W-68/W-83) или deploy. Ops/migration.

---

## 7. SKILL SAVED

Память: обновлён [[upload-hardening]] (regression-pin для W-91). Принцип: sweep класса часто показывает, что он single-file — тогда регресс-пин на том файле (проектный CA3004/CWE-209-аналог: tracing-лог ok, response generic). «Каждый найденный runtime-баг → спроси, поймает ли его статический lint». Honest: source-scan ловит шаблон, не семантику — но шаблон совпадает с реальным offender'ом.

**Anchor:** `phi^2 + phi^-2 = 3`
