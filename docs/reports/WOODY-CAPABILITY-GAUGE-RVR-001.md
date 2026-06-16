# 🌊 WAVE LOOP REPORT — Capability State Gauge (dashboard observability)

**Document ID:** `WOODY-CAPABILITY-GAUGE-RVR-001`
**Wave:** #26 — реализован «Capability gauge» (Вариант B из Wave #25)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `9b4ead5` — `feat(metrics): capability_enabled gauge for dashboard visibility`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — labelled capability-gauge добавлен, 8 metrics-тестов зелёные, backend+WASM компилируются.**

Wave #25 логировал отключённые возможности на старте (одна строка в boot-логе). Этот Wave добавляет dashboard-аналог: labelled Prometheus-gauge `capability_enabled{capability="ai"|"s3"}` = 1/0, выставляется на старте из **того же** `Config::ai_enabled()`/`s3_enabled()` источника (gauge и warn-лог не разойдутся). Grafana теперь показывает матрицу «какие фичи живы» по окружениям/репликам во времени. Info-сигнал, не alert (возможность может быть намеренно выключена).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-56 | Состояние возможностей видно только в boot-логе одной реплики, не на дашборде/во времени | 🟡 LOW | ✅ `capability_enabled{capability}` 0/1 gauge |
| W-55 | Presence-check, не «trust but verify» | 🟡 LOW | 📋 Wave+1 (dry-run) |
| W-50 | `CRITICAL_COLUMNS` guard, не derive | 🟢 INFO | 📋 Wave+1 (build.rs) |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: Prometheus info/StateSet pattern для метаданных и feature-flag состояний.**

- Prometheus — числовая БД; текстовые метаданные/состояния выражают через **info-метрику** (gauge=1 + labels) и **StateSet** (OpenMetrics) для on/off-флагов: `feature_flags{feature="x"} 0|1`.
- Мой `capability_enabled{capability="ai"} 0|1` — **ровно StateSet-паттерн** (labelled gauge 0/1 на возможность). `metrics`/axum-prometheus не имеют нативного StateSet → labelled gauge = идиоматичный fallback.
- **Cardinality caution**: «avoid high-churn label values… each series consumes memory.» Мои labels — только `ai`/`s3` (2 серии) → низкая кардинальность, ок.
- Ценность: «bridging operational monitoring with deployment context… enrich dashboards via PromQL joins.» Связал gauge с тем же config-источником, что warn-лог (#25) — единый SSOT, без дрейфа.

Слой startup-observability: required-fail-fast → optional-warn-log (#25) → **capability-gauge (#26)** → schema self-check/gauge (#19/#20).

Источники:
- [How to Build Prometheus Info Metrics (OneUptime)](https://oneuptime.com/blog/post/2026-01-30-prometheus-info-metrics/view)
- [Prometheus client_java — Metric Types (StateSet)](http://prometheus.github.io/client_java/getting-started/metric-types/)
- [Prometheus Label Best Practices — cardinality (OneUptime)](https://oneuptime.com/blog/post/2026-01-30-prometheus-label-best-practices/view)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Выбран B (lowest-risk, переиспользует metrics-инфру #20; A=dry-run рискует сетью на старте, C=build.rs рискует сборкой).
2. ✅ `metrics.rs`: `capability_enabled(name, enabled)` — labelled `gauge!("capability_enabled","capability"=>name).set(0|1)`.
3. ✅ Startup (main.rs): `capability_enabled("ai", config.ai_enabled())`, `("s3", config.s3_enabled())` — тот же источник, что warn-лог #25.
4. ✅ Metrics call-site defense-тест подтверждает проводку (8 passed); backend+WASM; fmt.
5. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/metrics.rs`**: `pub fn capability_enabled(name: &str, enabled: bool)` — `gauge!("capability_enabled","capability"=>name.to_string()).set(if enabled {1.0} else {0.0})`.
- **`src/main.rs`**: после warn-лога #25 — выставление gauge для `ai`/`s3` из `config`.

Проверка: `metric_wiring_tests` call-site зелёный (8 passed); backend + `cargo check --target wasm32` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🔌 «Trust but verify» dry-run (W-55, осторожно)
Проверять на старте валидность, не только наличие: лёгкий ping AI / `HEAD` к S3 (timeout + best-effort, опц. под флагом). Ловит present-but-invalid ключ. Риск: сеть на старте.

### Вариант B — 🏗️ Derive CRITICAL_COLUMNS (build.rs, W-50)
Истинный DRY — генерировать список из SELECT'ов на сборке.

### Вариант C — 📈 Build-info метрика + Grafana-панель capability-matrix
Добавить `build_info{version,commit}` (классический info-pattern) для корреляции «какой деплой» ↔ метрики, и панель в `grafana-dashboard.json`, визуализирующую `capability_enabled`. Завершает observability deployment-context.

---

## 7. SKILL SAVED

Память: обновлён `startup-capability-report.md` (добавлен capability_enabled StateSet-gauge). Принцип: feature-flag/capability state в Prometheus — labelled gauge 0/1 (StateSet-паттерн), низкая кардинальность, из единого config-источника (без дрейфа с лог-сигналом).

**Anchor:** `phi^2 + phi^-2 = 3`
