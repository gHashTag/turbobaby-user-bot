# 🌊 WAVE LOOP REPORT — Grafana deploy markers + build/capability panels

**Document ID:** `WOODY-GRAFANA-DEPLOY-RVR-001`
**Wave:** #28 — реализован «Grafana deploy-аннотации + панели» (Вариант A из Wave #27)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `f1a3801` — `feat(monitoring): Grafana deploy markers + build/capability panels`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — дашборд расширен (валидный JSON, 8→11 панелей + Deploys-аннотация); config-only, без app-кода.**

Капстоун observability-арки (#25 лог → #26 capability-gauge → #27 build_info): сделал метрики **видимыми на дашборде**.
- **Deploys-аннотация** (query на `build_info`) → вертикальные маркеры «Deploy {version}» на всех панелях → «что задеплоили прямо перед спайком?» с одного взгляда.
- **Deployed Build** — какой version/commit крутит каждая реплика (подтвердить, что фикс доехал).
- **Capabilities (1=enabled)** — матрица AI/S3.
- **5xx rate by deploy version** — `rate(5xx) * on(instance) group_left(version) build_info` → регресс конкретного релиза виден.

Изменён только `monitoring/grafana-dashboard.json` (config-артефакт), через `jq`, JSON провалидирован.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-58 | Метрики deploy/capability (#26/#27) не визуализированы; нет deploy-маркеров | 🟡 LOW | ✅ 3 панели + Deploys-аннотация |
| W-59 | `grafana-dashboard.json` не проверяется в CI (может стать невалидным/ссылаться на несуществующие метрики) | 🟢 INFO | 📋 Wave+1 (dashboards-as-code lint) |
| W-55 / W-50 | trust-but-verify / derive CRITICAL_COLUMNS | 🟡/🟢 | 📋 Wave+1 |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops (build_info теперь подтвердит доставку) |

---

## 3. НАУЧНАЯ БАЗА

**Тема: дизайн observability-дашбордов (RED) + deploy-аннотации для корреляции.**

- **RED method** (Rate/Errors/Duration) для сервисов — «proxy for user experience»; дашборд уже имел rate/5xx/latency. Мои добавки: deployment-context (build_info, capabilities) + **per-version error breakdown** (RED-by-version) — регресс релиза диверджит визуально.
- **Избегай «wall of panels nobody reads during an outage»** — добавил **3 фокусных** панели, не свалку; ключевые KPI остаются первыми.
- **Deploy-аннотации** — устоявшаяся практика: «mark deploys on graphs so you can tell whether a metric change coincided with a release». Закрывает первый вопрос триажа.
- **Alert on symptoms** (5xx), cause-метрики (#20 schema) как ticket — слои уже разнесены.

⚠️ Самокритика по гайдлайну: build_info/capabilities — low-traffic state-панели (можно ужать в один stat-row); оставил отдельными для ясности, но это граница «не плодить панели».

Источники:
- [Grafana dashboard best practices (RED, hierarchy, naming)](https://grafana.com/docs/grafana/latest/visualizations/dashboards/build-dashboards/best-practices/)
- [What is observability — RED vs USE, symptom alerting (Grafana Labs)](https://grafana.com/blog/2022/07/01/what-is-observability-best-practices-key-metrics-methodologies-and-more/)
- [Getting started with Grafana — design your first dashboard](https://grafana.com/blog/2024/07/03/getting-started-with-grafana-best-practices-to-design-your-first-dashboard/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Разбор схемы `grafana-dashboard.json` (datasource `${prometheus}`, panel-shape, max id/y).
2. ✅ Клонировал shape timeseries-панели; 3 новые панели (build_info / capability_enabled / 5xx-by-version) на y=32, ids 9-11.
3. ✅ Deploys-аннотация (Prometheus query на `build_info`, titleFormat «Deploy {version}»).
4. ✅ Вставка через `jq` + валидация JSON (`jq -e`); 11 панелей, 2 аннотации.
5. ✅ Commit (config-only, без cargo).
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`monitoring/grafana-dashboard.json`**: `.panels += [3 timeseries]` (Deployed Build / Capabilities / 5xx by version), `.annotations.list += [Deploys]`. Все используют datasource `${prometheus}`, refId/legendFormat по образцу существующих панелей. Изменения внесены `jq`, JSON валиден.

Проверка: `jq -e '.panels|length'` → 11; аннотации `["Annotations & Alerts","Deploys"]`; нет app-кода → cargo не требуется.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🧪 Dashboards-as-code lint (W-59)
Host-тест: `grafana-dashboard.json` — валидный JSON И каждый panel/annotation `expr` ссылается на метрику, которая существует (декларирована в `src/metrics.rs` или известный axum-prometheus набор). Ловит «панель ссылается на удалённую/опечатанную метрику». В духе остальных defense-тестов; защищает дашборд от тихого протухания.

### Вариант B — 🔌 trust-but-verify dry-run (W-55)
Проверять валидность AI-ключа/S3 на старте (timeout + best-effort).

### Вариант C — 🏗️ Derive CRITICAL_COLUMNS (build.rs, W-50)
Истинный DRY — генерировать список из SELECT'ов на сборке.

---

## 7. SKILL SAVED

Память: обновлён `startup-capability-report.md` (дашборд-панели + Deploys-аннотация). Принцип: метрики бесполезны без визуализации/корреляции — RED-панели + deploy-аннотации (build_info version-change) отвечают «что изменилось, когда метрика поехала»; держать панели фокусными.

**Anchor:** `phi^2 + phi^-2 = 3`
