# 🌊 WAVE LOOP REPORT — Dashboard Portability Guard

**Document ID:** `WOODY-DASHBOARD-PORTABILITY-RVR-001`
**Wave:** #30 — реализован «Расширить dashboard-lint» (Вариант A из Wave #29, item W-60)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `d70f5e0` — `test(monitoring): dashboard portability guard (datasource uid + null id)`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — portability-guard добавлен, тест зелёный (проверен negative-тестом).**

Расширил dashboard-lint (#29) проверкой портируемости: каждый `datasource.uid` в `grafana-dashboard.json` должен быть шаблонным `${prometheus}` (из `__inputs`) или встроенным `-- Grafana --` (для builtIn-аннотаций) — **никогда** hardcoded instance-specific uid (классический баг «вставил панель из другого дашборда», который ломается при импорте). Плюс проверка, что top-level `id` остаётся `null` (Grafana назначает свой при импорте → нет id-коллизий).

Дашборд уже соответствует → это **regression-guard** (честно: thread наблюдаемости/дашбордов в зоне убывающей отдачи).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-60 | Дашборд мог получить hardcoded datasource uid (ломает импорт на другой инстанс) | 🟢 INFO | ✅ `dashboard_is_portable` (uid ∈ {${prometheus}, -- Grafana --}, id=null) |
| W-55 | trust-but-verify (валидность ключей на старте) | 🟡 LOW | 📋 Wave+1 |
| W-50 | Derive CRITICAL_COLUMNS (build.rs) | 🟢 INFO | 📋 Wave+1 |
| — | **Диминишинг в observability/dashboard-ветке** | 🟢 META | 📋 след. Wave — свежая область / деплой (см. §6) |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: portable Grafana dashboards (templated datasource, no hardcoded uid).**

- Портируемость = **datasource template variable** вместо хардкода: «avoid hard-coding specific values… reference the datasource variable in all panels». Export добавляет `__inputs` + меняет datasource на `${DS_*}`. У нас — `${prometheus}` (портируемый паттерн).
- «**Avoid hardcoded UIDs** in panels; rely on the variable so the datasource picker controls the whole dashboard.» Мой guard это и проверяет.
- `-- Grafana --` — легитимное исключение (встроенная datasource builtIn-аннотаций).
- GitOps: «let the system generate UIDs rather than hardcoding»; top-level `id`=null → Grafana назначает.

Тот же «guard the intentional contract» класс. Самокритика: guard на уже-compliant single-dashboard — низкая маржинальная ценность; следующий Wave стоит увести в свежую область.

Источники:
- [Grafana datasource template variable export issue #6189](https://github.com/grafana/grafana/issues/6189)
- [Grafana Dashboard Templating for Reusable Components (Reintech)](https://reintech.io/blog/grafana-dashboard-templating-reusable-components)
- [grafana-operator — dashboards (auto-generated uid)](https://grafana.github.io/grafana-operator/docs/examples/dashboard/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Проверил: top-level `id` уже `null` (compliant); datasource uids = {`${prometheus}`, `-- Grafana --`}.
2. ✅ `collect_datasource_uids` (рекурсивный обход serde_json::Value) + `dashboard_is_portable` тест.
3. ✅ Assert: id=null; каждый uid ∈ allowed set.
4. ✅ Negative-тест: hardcoded uid → FAIL; restore → PASS; fmt.
5. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/main.rs` `grafana_dashboard_tests`**: `collect_datasource_uids` (рекурсивно собирает `.datasource.uid`), `dashboard_is_portable` (id=null + uid ∈ {`${prometheus}`,`-- Grafana --`}).

Проверка: PASS; negative (`abc123-hardcoded`) → FAIL; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

> 🧭 Honesty: observability/dashboard-ветка (#19-#30) глубоко в diminishing returns. Рекомендую увести следующий Wave в свежую область или к доставке.

### Вариант A — 🆕 Fresh-area scan (рекомендуется)
Исследовать нетронутый модуль на реальный weak spot: `src/bot` (callbacks/commands — недавно были баги профиля), error-UX (`docs/ERROR_UX_AUDIT.md`), или security-path (rate-limit/fraud). Цель — найти настоящий дефект/пробел, а не ещё один guard в насыщенной зоне.

### Вариант B — 🚀 Закрыть deploy-долг
Подготовить чистый PR прод-фиксов #18-#20 (`ce0648e`/`f36da9c`/`8f587bf`) против `main` (cherry-pick на hotfix-ветку) — наибольшая реальная ценность сейчас. Требует подтверждения пользователя (push/PR — outward-facing).

### Вариант C — 🔌 trust-but-verify dry-run (W-55)
Валидность AI-ключа/S3 на старте (timeout + best-effort) — ловит present-but-invalid.

---

## 7. SKILL SAVED

Память: обновлён `startup-capability-report.md` (portability-guard). Принцип: дашборд портируем — только templated `${...}` datasource, без hardcoded uid; top-level id=null. ⚠️ Зафиксировано: observability-ветка насыщена, следующий Wave — свежая область.

**Anchor:** `phi^2 + phi^-2 = 3`
