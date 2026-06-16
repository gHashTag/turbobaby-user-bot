# 🌊 WAVE LOOP REPORT — Dashboards-as-Code Lint

**Document ID:** `WOODY-DASHBOARD-LINT-RVR-001`
**Wave:** #29 — реализован «Dashboards-as-code lint» (Вариант A из Wave #28, item W-59)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `25fdaf8` — `test(monitoring): dashboards-as-code lint — dashboard exprs reference real metrics`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — dashboard-lint добавлен (поймал бы переименование метрики), тест зелёный, проверен negative-тестом.**

`monitoring/grafana-dashboard.json` ссылается на метрики строками PromQL. Переименуй/удали метрику в `src/metrics.rs` (или опечатайся) — панель молча покажет «No data» до инцидента. Добавлен host-тест `dashboard_exprs_reference_real_metrics`: парсит дашборд (заодно проверяя, что это **валидный JSON**), извлекает имя метрики из каждого panel/annotation `expr` (пропуская `{…}`-матчеры, `[…]`-длительности, label-листы `by/on/group_left`, PromQL-функции) и проверяет, что каждое — реальное: объявлено в `src/metrics.rs`, `axum_*`-builtin, или `wwb:`-recording-rule.

Это и есть «CI check that validates the dashboard before merge», плюс **семантическая** проверка ссылок (сильнее, чем JSON-синтаксис).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-59 | Дашборд ссылается на метрики строками; переименование в коде → тихий «No data» | 🟡 LOW | ✅ `dashboard_exprs_reference_real_metrics` |
| W-60 | Дашборд хранит top-level `id` (best practice: убирать — Grafana назначает при импорте) | 🟢 INFO | 📋 (мелочь) |
| W-55 / W-50 | trust-but-verify / derive CRITICAL_COLUMNS | 🟡/🟢 | 📋 Wave+1 |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: dashboards-as-code — CI-валидация против rot/drift.**

- UI-дашборды хрупки («live only in a DB, can be accidentally modified/deleted»). Хранение в Git + CI решает: version control, peer review, reproducibility.
- **Ключевая практика**: «run a **CI check that validates dashboard JSON** on pull requests **before merging**» — валидируй JSON-синтаксис, `schemaVersion`, **datasource UIDs**. Мой тест делает это (+ серде-парс на валидность) и идёт дальше — проверяет, что метрики в `expr` реально известны коду (семантика, не только синтаксис).
- **Best practices**: хранить без `id` (Grafana назначает при импорте; у нас `id` есть → W-60), GitOps-reconciliation, типизированные SDK/Jsonnet для compile-time валидации.
- Дашборд уже «as code» (в Git этого репо); добавлен недостающий **validation-слой**.

Тот же «guard the intentional contract» класс, что schema-drift (#1), build_info/metrics (#27), icon-button (#24).

Источники:
- [Automate dashboard provisioning with CI/CD (Grafana docs)](https://grafana.com/docs/grafana/latest/as-code/observability-as-code/foundation-sdk/dashboard-automation/)
- [Observability as code (Grafana docs)](https://grafana.com/docs/grafana/latest/as-code/observability-as-code/)
- [Manage Grafana Dashboards as Code with Flux CD (OneUptime)](https://oneuptime.com/blog/post/2026-03-13-manage-grafana-dashboards-code-flux-cd/view)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Снял все `expr` дашборда (panels[].targets[].expr + annotations[].expr) и метрики из `src/metrics.rs` — подтвердил: всё либо declared, либо `axum_*`.
2. ✅ `metric_names(expr)` — мини-PromQL-сканер (skip `{}`/`[]`/modifier-label-листы/функции).
3. ✅ `declared_metrics()` (парс `counter!`/`gauge!` имён) + known = declared ∪ `axum_*` ∪ `wwb:`.
4. ✅ Тест: serde_json-парс (валидность) + каждое имя метрики ∈ known.
5. ✅ Negative-тест: `build_info`→`build_infoz` → FAIL; restore → PASS; fmt.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/main.rs` `grafana_dashboard_tests`**: `metric_names` (PromQL-сканер: пропускает label-матчеры/длительности/grouping-модификаторы/функции), `declared_metrics` (парс metrics.rs), `dashboard_exprs_reference_real_metrics` (serde_json-парс + проверка known). Host-тест (defensive-tests hook).

Проверка: PASS (валидный JSON + все метрики known); negative (`build_infoz`) → FAIL с именем; fmt clean. Без app-кода/UI → cargo wasm не требуется (но bin компилируется).

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🧹 Расширить dashboard-lint (datasource UID / strip id, W-60)
Добавить в тест: каждый panel/annotation `datasource.uid == "${prometheus}"` (одна объявленная datasource); опц. флагнуть top-level `id` (best practice — убрать). Усиливает as-code гигиену.

### Вариант B — 🔌 trust-but-verify dry-run (W-55)
Валидность AI-ключа/S3 на старте (timeout + best-effort).

### Вариант C — 🏗️ Derive CRITICAL_COLUMNS (build.rs, W-50)
Истинный DRY — генерировать список из SELECT'ов на сборке.

---

## 7. SKILL SAVED

Память: обновлён `startup-capability-report.md` (добавлен dashboard-as-code lint). Принцип: дашборд в Git без CI-валидации тихо гниёт; тест проверяет валидный JSON + что каждая метрика в `expr` реально известна коду (declared/axum_/wwb:) — семантика поверх синтаксиса.

**Anchor:** `phi^2 + phi^-2 = 3`
