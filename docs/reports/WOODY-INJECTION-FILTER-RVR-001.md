# 🌊 WAVE LOOP REPORT — Prompt-Injection Filter False Positives

**Document ID:** `WOODY-INJECTION-FILTER-RVR-001`
**Wave:** #31 — свежее исследование (`src/ai.rs`), Вариант A (fresh-area scan) из Wave #30
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `8711d4d` — `fix(ai): word-boundary prompt-injection markers (stop false positives)`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — реальный UX-баг исправлен без ослабления безопасности; 12 sanitize-тестов зелёные.**

Послушал собственную рекомендацию (#30) — ушёл из насыщенной observability-ветки в свежую область. Скан `src/bot` показал, что он крепкий (well-tested helpers, long-polling — нет webhook-attack-surface). Нашёл реальный дефект в **`src/ai.rs`**: фильтр prompt-injection в AI-сомелье использовал **наивный substring-матч** → блокировал легитимный free-text как `[filtered]`:
- `"shack"` ⊃ `"hack"`, `"ecosystem:"` ⊃ `"system:"`, `"the contract as written"` ⊃ `"act as"`.

Легитимные запросы сомелье молча падали. Исправлено: **word-boundary матч** (`contains_marker` — маркер должен быть окружён не-alphanumeric). **Ни один маркер не удалён** → реальные инъекции (`ignore previous`, `system:`, `you are now`, `###`, `jailbreak`…) всё ещё ловятся → нет регресса безопасности; убрана только over-blocking embedded-in-word.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-61 | Injection-denylist substring-матч → false positives на легит-вводе (`shack`→`hack`) | 🟠 MED | ✅ word-boundary `contains_marker` + regression-тесты |
| W-62 | Generic single-word маркеры (`simulate`/`act as`/`hack`) всё ещё over-block standalone legit usage | 🟡 LOW | 📋 security-owner решение (см. §6 A) |
| — | `src/bot` проверен — крепкий (tested helpers, polling, нет webhook) | ✅ | — |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: OWASP LLM01 (Prompt Injection) — пределы input-denylist.**

- LLM01 — топ-риск; «instructions and data inseparable» → инъекция фундаментальна.
- **Denylist одновременно**: «over-blocks legitimate input (false positives) while under-blocking real attacks (obfuscation)». Мой фикс адресует **первую** половину (over-blocking embedded-in-word), не трогая вторую.
- «No single technique provides reliable protection… provide specific instructions about the model's role/limitations within the **system prompt**» — реальный guard. У нас он есть: `ask_grok` строит строгий system-persona. Denylist — **defense-in-depth**, не основной барьер.
- Сильные guardrails (на будущее): structural separation (StruQ), instruction hierarchy, guard-LLM, output filtering. Denylist остаётся слабым by design.

Свежая область (не observability): применил «исследуй слабое место → точечный fix» к реальному UX-багу в security-коде, аккуратно (без ослабления).

Источники:
- [OWASP LLM01:2025 Prompt Injection (GenAI Security Project)](https://genai.owasp.org/llmrisk/llm01-prompt-injection/)
- [LLM01 Prompt Injection vuln (OWASP GitHub)](https://github.com/OWASP/www-project-top-10-for-large-language-model-applications/blob/main/2_0_vulns/LLM01_PromptInjection.md)
- [A Critical Evaluation of Defenses against Prompt Injection (arXiv 2505.18333)](https://arxiv.org/pdf/2505.18333)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Fresh-area скан `src/bot`: helpers tested, long-polling (`delete_webhook`+Dispatcher) → нет webhook-уязвимости. Крепко.
2. ✅ Пивот в `src/ai.rs`: `sanitize_user_text` — substring-матч → false positives (shack/ecosystem:/contract as).
3. ✅ `contains_marker` (word-boundary) + замена loop; маркеры не тронуты.
4. ✅ Regression-тесты (false-positives проходят; standalone-маркеры всё ещё блокируются); существующие 12 sanitize/инъекция-тестов зелёные.
5. ✅ backend+WASM компилируются; fmt.
6. ✅ Research (OWASP LLM01) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/ai.rs`**: `fn contains_marker(haystack, marker) -> bool` (маркер flanked by non-alphanumeric / границы строки; UTF-8-safe byte-checks). `sanitize_user_text` использует его вместо `lower.contains(marker)`. Список `dangerous` без изменений.
- Тесты: `test_sanitize_word_boundary_no_false_positives` (shack/ecosystem:/contract as/overrides), `test_sanitize_still_blocks_standalone_markers` (ignore previous / system: / ###).

Проверка: 12 sanitize-тестов PASS; `cargo check --target wasm32` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🔐 Prune generic markers (security-owner решение, W-62)
`simulate`/`act as`/`roleplay as`/`hypothetically`/`hack`/`bypass`/`exploit`/`override` — generic English, over-block standalone легит-ввод («simulate a chill evening»), при этом слабая security-ценность (легко обойти). Предложить оставить только clearly-injection фразы; основной guard — system prompt. **Требует sign-off владельца** (security-тюнинг). Опц.: spotlighting/structural separation вместо denylist.

### Вариант B — 🆕 Продолжить fresh-area scan
Следующий нетронутый модуль (error-UX `docs/ERROR_UX_AUDIT.md`, или другой) на реальный дефект.

### Вариант C — 🚀 Закрыть deploy-долг (#18-#20)
Подготовить чистый PR прод-фиксов против `main` — наибольшая реальная ценность. Требует подтверждения (outward-facing).

---

## 7. SKILL SAVED

Память: новый `ai-injection-filter.md` + строка в `MEMORY.md`. Принцип: input-denylist = defense-in-depth (OWASP LLM01), реальный guard — system prompt; матчить маркеры по word-boundary (не substring), чтобы не блокировать легит-ввод; не удалять маркеры без security sign-off.

**Anchor:** `phi^2 + phi^-2 = 3`
