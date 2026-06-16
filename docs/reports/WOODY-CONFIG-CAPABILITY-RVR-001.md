# 🌊 WAVE LOOP REPORT — Startup Capability Report (config visibility)

**Document ID:** `WOODY-CONFIG-CAPABILITY-RVR-001`
**Wave:** #25 — свежее исследование (config/startup), не пункт меню
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `9e64c41` — `feat(config): startup report of capabilities disabled by missing config`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — startup capability-report добавлен, 4 host-теста зелёные, backend+WASM компилируются.**

Вместо авто-продолжения меню — свежий скан **config/startup** (нетронутая область). Находка: `grok_api_key`/`glm_api_key` дефолтятся в пустую строку, S3-креды опциональны. Значит отсутствие `GROK_API_KEY` **молча** убивает AI-сомелье, а неполный S3 — загрузки медиа. AI-клиент предупреждает только **по факту неудачного запроса** — ops узнаёт о проблеме от пользователя, не на старте.

Добавлен **startup capability-report**: `Config::disabled_capabilities()` логирует на старте каждую отключённую из-за конфига возможность (`⚠️ capability disabled: AI sommelier (GROK_API_KEY + GLM_API_KEY both unset)`). Логирую **имена возможностей и какой ENV пуст**, не значения секретов. Это не fault (env может намеренно быть без AI/S3) → warn, не alert.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-54 | Отключённые из-за конфига возможности (AI/S3) невидимы на старте | 🟠 MED | ✅ `disabled_capabilities()` + startup warn |
| W-55 | Presence-check, не «trust but verify» (валидность ключа/доступность S3 не проверяется) | 🟡 LOW | 📋 Wave+1 (dry-run, осторожно) |
| W-50 | `CRITICAL_COLUMNS` guard, не derive | 🟢 INFO | 📋 Wave+1 (build.rs) |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: twelve-factor config — validate/surface на старте, а не молча в рантайме.**

- «Config problems should surface **loudly at startup** rather than silently at runtime… frustrating to learn which values are required through runtime errors.»
- **Required → fail non-zero** (twelve-factor + «patch-level» предложение). Репо это уже делает: `collect_required_env(["BOT_TOKEN","DATABASE_URL"])` → exit. **Optional → log visibly** (этот Wave) — правильная разница в severity: AI/S3 не обязательны, поэтому warn, не exit.
- «**Log the resolved config (non-sensitive parts)** so operators can see what's enabled/disabled.» Ровно мой capability-report.
- **Secret-masking tension**: «you can't see what's wrong if you can't see what's set.» Обхожу: логирую *имена* возможностей и *какой ENV пуст*, без значений секретов — польза без утечки.
- Глубже — «**trust but verify**»: dry-run проверка валидности ключа / доступности S3 (→ W-55, осторожно: сетевые вызовы на старте).

Слой: required-fail-fast (есть) + optional-capability-visibility (этот Wave) + schema self-check (#19) + cause-gauge (#20) = связная startup-observability.

Источники:
- [Twelve-Factor: How Do You Validate Your Configuration? (marmelab)](https://marmelab.com/blog/2018/12/05/twelve-factor-applications-how-do-you-validate-your-configuration.html)
- [12.1-Factor Apps: Config (DEV) — fail non-zero on missing required config](https://dev.to/ahawkins/12-1-factor-apps-config-3c78)
- [Twelve-Factor App methodology (Wikipedia) — Config & Disposability](https://en.wikipedia.org/wiki/Twelve-Factor_App_methodology)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Скан config.rs: `grok/glm_api_key` → `unwrap_or_default()` (пустые при отсутствии); S3 — Option. Подтвердил: AI-клиент warn'ит только per-request (ai.rs `is_empty()` skip).
2. ✅ `Config::ai_enabled()` + `disabled_capabilities()` (через pure `disabled_capabilities_from(ai, s3)`); переиспользует `s3_enabled()`.
3. ✅ 4 host-теста на pure-функцию (all/ai/s3/both).
4. ✅ Startup (main.rs): warn по каждой отключённой возможности (после env-info, до connect).
5. ✅ backend+WASM компилируются; fmt.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/config.rs`**: `ai_enabled(&self)` (хоть один ключ непуст); `disabled_capabilities(&self)` → `disabled_capabilities_from(ai_enabled, s3_enabled)` (pure, testable); список — AI sommelier / S3 uploads. 4 теста.
- **`src/main.rs`**: после стартовых info-логов — `for cap in config.disabled_capabilities() { tracing::warn!("⚠️ capability disabled: {cap}") }`.

Проверка: 4 теста PASS; backend + `cargo check --target wasm32` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🔌 «Trust but verify» dry-run (W-55, осторожно)
На старте проверять не только наличие, но и **валидность**: лёгкий ping AI-провайдера / `HEAD` к S3-бакету (с таймаутом, non-fatal, опционально под флагом). Ловит «ключ есть, но протух/неверный». Риск: сетевые вызовы замедляют/флапают старт — строго timeout + best-effort.

### Вариант B — 📊 Capability gauge (дашборд)
Экспонировать состояние возможностей как Prometheus-gauge (`capability_enabled{name=...}`), чтобы Grafana показывала, какие фичи живы по окружениям (как `schema_missing_columns` из #20). Info-метрика, без алерта.

### Вариант C — 🏗️ Derive CRITICAL_COLUMNS (build.rs, W-50)
Истинный DRY — генерировать список из SELECT'ов на сборке.

---

## 7. SKILL SAVED

Память: новый `startup-capability-report.md` + строка в `MEMORY.md`. Принцип: required-config → fail-fast (exit non-zero); optional-capability → log visibly на старте (имена + какой ENV пуст, без секретов); «surface loudly at startup, not silently at runtime».

**Anchor:** `phi^2 + phi^-2 = 3`
