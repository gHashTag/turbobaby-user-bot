# 🌊 WAVE LOOP REPORT — build_info metric (deploy correlation)

**Document ID:** `WOODY-BUILD-INFO-RVR-001`
**Wave:** #27 — реализован «build_info метрика» (Вариант C из Wave #26)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `63c43bc` — `feat(metrics): build_info info-metric (which commit is prod running)`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — build_info info-метрика добавлена, call-site defense-тест зелёный, backend+WASM компилируются.**

Добавлена классическая Prometheus info-метрика `build_info{version,build} 1`, выставляется на старте из `CARGO_PKG_VERSION` (semver) + существующего `BUILD_VERSION` (short-sha + build-ts из build.rs). Значение всегда 1; идентичность деплоя — в labels.

Прямая польза для рекуррентного /api/sets-кейса: ops/PromQL могут **подтвердить, какой коммит реально крутится на prod** (задеплоен ли фикс), и join'ить error-rate по версии, чтобы увидеть, какой релиз дал регресс. `BUILD_VERSION` раньше был только в admin-UI — теперь наблюдаем в метриках.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-57 | Нельзя из метрик/дашборда понять, какой коммит крутится на prod (корреляция деплой↔инцидент) | 🟠 MED | ✅ `build_info{version,build}` info-метрика |
| W-58 | Нет deploy-аннотаций на Grafana (вертикальные маркеры релизов) | 🟡 LOW | 📋 Wave+1 (query-annotation на смену version) |
| W-55 / W-50 | trust-but-verify / derive CRITICAL_COLUMNS | 🟡/🟢 | 📋 Wave+1 |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: build_info info-metric, deploy↔incident корреляция, DORA Change Failure Rate.**

- **Info-pattern**: `build_info` value=1, метаданные в labels. «Use a separate build_info metric rather than tagging every metric with version.» Сам Prometheus экспортит `prometheus_build_info{branch,version,revision}`.
- **Cardinality-урок (GitLab/Gitaly)**: НЕ клади build-time в `version`-label (он варьируется между серверами одной версии → ложные серии). Моё разбиение **корректно**: `version` = чистый semver, `build` = волатильный sha+ts (именно то место для волатильного id по гайдлайну `build_info{version, commit, build_number}`).
- **Regression-detection join**: `rate(http[5m]) * on(instance) group_left(version) build_info` → error-rate по версии → регресс конкретного релиза виден сразу. Новый экспериментальный `info()` (Prometheus, 2025) упрощает и чинит «churn problem» таких join'ов.
- **DORA**: атрибуция инцидента к версии (через build_info) делает **Change Failure Rate** измеримым. «did something ship right before this regression?» — первый вопрос триажа.

Завершает startup-observability слой (#19/#20/#25/#26) deployment-context'ом.

Источники:
- [Exposing the software version to Prometheus (Robust Perception)](https://www.robustperception.io/exposing-the-software-version-to-prometheus/)
- [Introducing the Experimental info() Function (Prometheus, 2025)](https://prometheus.io/blog/2025/12/16/introducing-info-function/)
- [Gitaly: version label should not include build time (GitLab)](https://gitlab.com/gitlab-org/gitaly/issues/388)
- [DORA Metrics — Change Failure Rate (Octopus)](https://octopus.com/devops/metrics/dora-metrics/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Разведка: `build.rs` уже эмитит `BUILD_VERSION` (sha+ts) через `env!`; используется только в admin-UI, не в метриках.
2. ✅ `metrics.rs`: `build_info(version, build)` — `gauge!("build_info","version"=>..,"build"=>..).set(1.0)`.
3. ✅ Startup: `build_info(env!("CARGO_PKG_VERSION"), env!("BUILD_VERSION"))`.
4. ✅ Проверил label-разбиение против Gitaly-урока: semver в `version`, волатильное в `build` → корректно.
5. ✅ Call-site defense-тест зелёный; backend+WASM; fmt.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/metrics.rs`**: `pub fn build_info(version: &str, build: &str)` — info-метрика value=1, labels `version`/`build`.
- **`src/main.rs`**: вызов на старте после capability-gauges.

Проверка: `metric_wiring_tests` зелёный; backend + `cargo check --target wasm32` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 📍 Grafana deploy-аннотации + regression-join панель (W-58)
Query-based annotation на смену `version` в `build_info` → вертикальные маркеры релизов на всех панелях («что задеплоили в 14:22 перед спайком в 14:23»). Плюс панель error-rate-by-version (`* on(instance) group_left(version) build_info`). Завершает deploy↔incident корреляцию визуально. ⚠️ ручной edit `grafana-dashboard.json` (валидировать осторожно).

### Вариант B — 🔌 trust-but-verify dry-run (W-55)
Проверять валидность AI-ключа/доступность S3 на старте (timeout + best-effort).

### Вариант C — 🏗️ Derive CRITICAL_COLUMNS (build.rs, W-50)
Истинный DRY — генерировать список из SELECT'ов на сборке.

---

## 7. SKILL SAVED

Память: обновлён `startup-capability-report.md` (добавлен build_info info-метрика). Принцип: deploy-identity как Prometheus info-метрика (value=1, semver в `version`, волатильный sha+ts в `build` — не наоборот!); join'ить метрики по version для regression-detection; питает DORA Change Failure Rate.

**Anchor:** `phi^2 + phi^-2 = 3`
