# 🌊 WAVE LOOP REPORT — Cap AI Prompt + Persona at the Paid-API Boundary (W-89)

**Document ID:** `WOODY-AI-INPUT-CAP-RVR-001`
**Wave:** #54 — input-validation/DoS fresh-area scan (Вариант C из Wave #53)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `cce136f` — `fix(ai): cap prompt + persona length at the paid-API boundary`
**Agent:** Claude Opus 4.8 (Wave loop) + Explore-subagent

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — prompt+persona капаются на границе AI-вызова; 2 host-теста + 69 defensive зелёные; backend+WASM.**

Скан input-validation нашёл: `ask_grok` встраивал **user-controlled** `persona` (Telegram first_name, через `handlers.rs`) в system-prompt **без length-cap** — `sanitize_user_text` фильтрует только injection-маркеры, не длину. `prompt` капался лишь одним вызывающим (handlers.rs:1500); остальные call-sites + persona — без лимита. System-prompt уходит в Grok/GLM на **каждый** вызов → token-cost / denial-of-wallet.

**Честная серьёзность — LOW** (Telegram ограничивает first_name ~64 симв., так что «100KB name» агента преувеличен), но это реальный boundary-validation gap (асимметрия: prompt капается, persona нет). Фикс: капаю **оба на границе `ask_grok`** (defense-in-depth, не доверяя 5 call-sites): prompt≤2000, persona≤100, через char-safe `util::truncate_string`. Извлёк тестируемый `build_system_prompt`; 2 unit-теста.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-89 | `ask_grok`: persona (Telegram name) → system-prompt → paid API без length-cap; prompt капался только caller'ом | 🟡 LOW (cost/DoW, defense-in-depth) | ✅ cap prompt≤2000 + persona≤100 на границе |
| — | `sanitize_user_text` фильтрует маркеры, не длину | ✅ by design | length-cap теперь рядом, на границе |
| — | personas в commands/callbacks хардкод ("Joker"/"Professor"/"FactMaster") | ✅ | защищены тем же boundary-cap'ом |
| 🚨 | PR ветки (104 ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: OWASP LLM10 Unbounded Consumption / denial-of-wallet; pre-flight input cap; малый system-prompt.**

- **OWASP LLM10**: «Unbounded Consumption … denial-of-wallet attacks **exploit the lack of input limits and cost controls**». Точно: uncapped persona в каждом запросе.
- **Size-limit все входы LLM**: «implement strict input validation with **size limits on all inputs to the LLM**». Капаю prompt+persona на границе.
- **System-prompt мал**: «a 2000-token system prompt sent with every call costs 400k input tokens / session; keep system prompts small». Persona живёт в system-prompt → cap критичен.
- **Pre-flight gating**: «cap input cost **before** the paid call (a pre-flight check, not a wrapper)». Cap до `call_grok`/`call_glm`.
- **Rate-limit недостаточно**: «standard rate limiters count requests, not cost». Поэтому нужен именно size-cap.

Источники:
- [OWASP LLM10:2025 Unbounded Consumption](https://genai.owasp.org/llmrisk/llm102025-unbounded-consumption/)
- [Denial of Wallet (ToxSec)](https://www.toxsec.com/p/denial-of-wallet)
- [LLM10 Unbounded Consumption (StackHawk)](https://www.stackhawk.com/blog/owasp-llm10-unbounded-consumption/)
- [The $12 LLM call nobody saw coming — pre-flight cost cap (DEV)](https://dev.to/mukundakatta/the-12-llm-call-nobody-saw-coming-llm-cost-cap-4cb5)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Скан input-validation → `ask_grok` persona uncapped (prompt капался только caller'ом).
2. ✅ Скорректировал серьёзность агента (Telegram first_name ~64 → LOW, не High).
3. ✅ Cap на границе `ask_grok`: `truncate_string(prompt, 2000)` + `truncate_string(sanitize(persona), 100)`.
4. ✅ Извлёк pure `build_system_prompt(persona, lang)` (капает persona) — тестируемо без API-ключа.
5. ✅ 2 unit-теста (huge persona → bounded system-prompt; normal persona unchanged); backend+WASM; 69 defensive PASS; fmt.
6. ✅ Research (OWASP LLM10/denial-of-wallet) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/ai.rs`**: консты `MAX_AI_PROMPT_CHARS=2000`, `MAX_AI_PERSONA_CHARS=100`; `fn build_system_prompt(persona, lang)` капает persona через `truncate_string(sanitize_user_text(...), 100)`; `ask_grok` капает `clean_prompt` до 2000 и зовёт `build_system_prompt`. Тесты `test_build_system_prompt_caps_oversized_persona`, `..._normal_persona_unchanged`.

Проверка: 2 ai-теста PASS; backend+WASM compile; defensive 69 PASS; fmt clean. Cap применяется ко всем 5 call-sites `ask_grok` (boundary-level).

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
104 коммита ahead. Ops/review — твоё «go».

### Вариант B — 🛡️ Rate-limit на AI/bot-DM путь (если отсутствует)
Проверить, стоит ли bot-DM AI-ответ (`handlers.rs`) за per-user rate-limit (OWASP LLM10: rate-limit + quotas поверх size-cap). Если нет — добавить.

### Вариант C — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

---

## 7. SKILL SAVED

Память: обновлён [[ai-injection-filter]] (добавлен length-cap на границе). Принцип: капай ВСЕ входы LLM по размеру на границе paid-API-вызова (pre-flight), не доверяя вызывающим; user-controlled значения в system-prompt особенно опасны (отправляются каждый раз → denial-of-wallet); rate-limit считает запросы, не стоимость — нужен size-cap.

**Anchor:** `phi^2 + phi^-2 = 3`
