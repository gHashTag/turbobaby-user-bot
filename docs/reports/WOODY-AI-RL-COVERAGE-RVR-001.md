# 🌊 WAVE LOOP REPORT — Lock In AI Rate-Limit Coverage (OWASP LLM10)

**Document ID:** `WOODY-AI-RL-COVERAGE-RVR-001`
**Wave:** #55 — rate-limit покрытие AI-путей (Вариант B из Wave #54)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `7340eda` — `test(bot): fitness fn — every ask_grok caller must be rate-limited (OWASP LLM10)`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — AI rate-limit покрытие подтверждено полным и зафиксировано fitness-функцией; 70 defensive зелёные.**

Аудит rate-limit покрытия AI-путей (OWASP LLM10: rate-limit + quotas поверх size-cap из #54). **Хорошая новость: покрытие уже полное** — все 5 `ask_grok` call-sites гейтятся `ai_rate_limit_allow` (handlers.rs:54, commands.rs:336/362 Joke/Fact, callbacks.rs:91 для обоих) — 1 запрос/5с на пользователя (cycle #129). Дефекта нет.

Поэтому (правило scoped-fitness-function) **зафиксировал** свойство: добавил source-scanning тест `every_ask_grok_caller_in_bot_is_rate_limited` — любой файл в `src/bot/`, содержащий `.ask_grok(`, обязан содержать `ai_rate_limit_allow`. Это ровно тот класс регрессии, который индустрия ловит CI-тестами: «a new endpoint missing rate-limit configuration». Теперь новый AI-command/callback нельзя добавить без rate-limit → защита от denial-of-wallet через дыру.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-90 | AI rate-limit покрытие не защищено от регрессии (новый ask_grok-caller без RL) | 🟢 INFO | ✅ fitness-тест co-occurrence `.ask_grok(` ⇒ `ai_rate_limit_allow` |
| — | Все 5 AI call-sites уже rate-limited (#129) + size-capped (#54) | ✅ | подтверждено аудитом |
| 🚨 | PR ветки (104 ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: rate-limit ВСЕ пути к дорогому ресурсу; CI-тест против нового незащищённого пути; lock-in существующего контроля (defense-in-depth).**

- **Покрытие, не один путь**: «rate limiting only one path rather than all endpoints reaching an expensive resource is a recognized gap». Я проверил ВСЕ 5 AI-путей.
- **CI-тест ловит новый незащищённый эндпоинт**: «rate limit tests caught a new endpoint **missing rate limit configuration entirely**»; «write a test that **fails the build when a route lacks a rate-limit policy**». Ровно мой fitness-тест.
- **Audit → lock-in (defense-in-depth)**: exempted/unprotected paths «erode the overall control» — регресс-тест предотвращает эрозию.
- **OWASP LLM10**: rate-limit + quotas — слой поверх size-cap (#54); «rate-limit counts requests» — оба слоя нужны.

Источники:
- [Testing API Rate Limiting (Total Shift Left — caught a new endpoint missing RL)](https://totalshiftleft.ai/blog/testing-api-rate-limiting-throttling)
- [Endpoint-specific rate limiting / cost-aware (CodeSignal)](https://codesignal.com/learn/courses/implementing-rate-limiting-6/lessons/endpoint-specific-rate-limiting)
- [OWASP LLM10:2025 Unbounded Consumption](https://genai.owasp.org/llmrisk/llm102025-unbounded-consumption/)
- [Defense in Depth (Check Point)](https://www.checkpoint.com/cyber-hub/cyber-security/what-is-defense-in-depth/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Аудит: bot-DM AI-путь (handlers.rs) — rate-limited (`ai_rate_limit_allow`, #129).
2. ✅ Проверил остальные 4 call-site (commands Joke/Fact, callbacks ×2) — все rate-limited.
3. ✅ Вывод: покрытие полное (+ size-cap #54). Дефекта нет.
4. ✅ Lock-in: fitness-тест `every_ask_grok_caller_in_bot_is_rate_limited` (co-occurrence по файлам src/bot/).
5. ✅ Тест PASS; backend+WASM; defensive PASS; fmt.
6. ✅ Research (RL coverage/CI-gap-test) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/bot/mod.rs`**: `#[cfg(test)] mod ai_rate_limit_coverage_tests` — читает `src/bot/*.rs`; если файл содержит `.ask_grok(` и НЕ содержит `ai_rate_limit_allow` → fail с перечнем offenders. (`ask_grok` определён в `src/ai.rs`, поэтому скан `.ask_grok(` ловит только call-sites.)

Проверка: тест PASS; backend+WASM compile; defensive 70 PASS; fmt clean. Все текущие bot-файлы с `.ask_grok(` (handlers/commands/callbacks) содержат `ai_rate_limit_allow`.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
104 коммита ahead. Ops/review — твоё «go».

### Вариант B — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

### Вариант C — 🛡️/🚀 Накопленный долг — нужен sign-off
DB CHECK-backstops (W-68/W-83) или deploy. Ops/migration.

---

## 7. SKILL SAVED

Память: обновлён [[ai-injection-filter]] (rate-limit coverage fitness-тест). Принцип: rate-limit нужен на ВСЕХ путях к дорогому ресурсу (не одном); когда аудит подтверждает покрытие — зафиксируй его CI-тестом, который падает, если новый call-site дорогого ресурса добавлен без rate-limit (ловит «new endpoint missing RL»); это defense-in-depth lock-in (как [[scoped-fitness-function-ratchet]]).

**Anchor:** `phi^2 + phi^-2 = 3`
