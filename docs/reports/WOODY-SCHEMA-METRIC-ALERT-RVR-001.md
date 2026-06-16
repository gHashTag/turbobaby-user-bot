# 🌊 WAVE LOOP REPORT — Schema-Drift Metric + Cause-Based Alert

**Document ID:** `WOODY-SCHEMA-METRIC-ALERT-RVR-001`
**Wave:** #20 — реализован «Alert на schema-warning» (Вариант A из Wave #19)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `8f587bf` — `feat(metrics): expose schema_missing_columns gauge + Prometheus alert`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — gauge + alert-rule добавлены, 8 metrics-тестов зелёные, backend+WASM компилируются.**

Wave #19 логировал недостающие prod-колонки при старте — но строчку в логах надо смотреть. Этот Wave закрывает observability-петлю: счётчик публикуется как Prometheus-gauge `schema_missing_columns` (ставится при старте, даже в 0) + alert-rule `WoodyWeedBotSchemaDrift` (`schema_missing_columns > 0`). Теперь prod-БД, отставшая по миграциям, поднимает тикет по **тому же Prometheus/Grafana-пути, что и 5xx-алерты** — но **на деплое, до того как пользователь словит 500**.

Это редкий оправданный **cause-based** алерт (по Google SRE): причина «определённая и неминуемая» (нет колонки → каталог 500-ит), severity=`ticket` (не page) — как для leading-индикаторов.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-49 | Schema-warning только в логах; нет алерта | 🟠 MED | ✅ Prometheus gauge + alert-rule (severity=ticket) |
| W-48 | `CRITICAL_COLUMNS` рукоподдерживаемый (drift с SELECT'ами) | 🟡 LOW | 📋 Wave+1 (авто-вывод) |
| W-47/W-42 | Admin icon a11y + alt-quality | 🟡 LOW | 📋 Wave+1 |
| 🚨 | #18/#19/#20-фиксы на ветке — нужен деплой в main | 🔴 | ⚠️ ops (см. §6) |

---

## 3. НАУЧНАЯ БАЗА

**Тема: symptom- vs cause-based alerting (Google SRE) + white-box monitoring.**

- Google SRE: «**alert on symptoms, not causes**; when it comes to causes, only worry about **very definite, very imminent** causes.» 5xx-алерт (Four Golden Signals → errors) — **symptom/black-box**: реагирует, когда пользователю уже плохо.
- Мой `schema_missing_columns > 0` — **cause/white-box, leading-indicator**: попадает в узкую «definite + imminent» категорию (отсутствие колонки гарантированно ломает каталог) и даёт lead time (срабатывает на деплое). Это white-box: «to page on an imminent problem before users are affected, you need white-box, cause-based monitoring.»
- **Severity**: SRE рекомендует **ticket, не page** для cause/leading-индикаторов (как saturation) → выбрал `severity: ticket`. Избегаю alert-fatigue.

Итог: два дополняющих слоя — symptom (5xx, page) + cause (schema drift, ticket, до удара по юзеру). Дополняет CI-слой [[schema-drift-defense]] и startup-лог Wave #19.

Источники:
- [Google SRE Book — Monitoring Distributed Systems (symptoms vs causes, golden signals)](https://sre.google/sre-book/monitoring-distributed-systems/)
- [The Four Golden Signals of SRE](https://www.sherlocks.ai/blog/four-golden-signals-of-sre)
- [SRE Metrics & Golden Signals (Splunk)](https://www.splunk.com/en_us/blog/learn/sre-metrics-four-golden-signals-of-monitoring.html)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Разведка alert-инфры: 5xx идёт через Prometheus/Grafana (`axum_http_requests_total`), не in-app webhook → идиоматично добавить gauge + alert-rule.
2. ✅ `src/metrics.rs`: gauge-хелпер `schema_missing_columns(n)` (`gauge!`).
3. ✅ `main.rs` startup: `crate::metrics::schema_missing_columns(missing_cols.len())` (всегда, даже 0).
4. ✅ `docs/prometheus-alerts.yaml`: group `woody_weed_bot_schema` → alert `WoodyWeedBotSchemaDrift` (`>0`, for 2m, severity=ticket).
5. ✅ Metrics call-site defense-тест подтверждает проводку; backend+WASM компилируются; fmt.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/metrics.rs`**: `pub fn schema_missing_columns(n: u64) { gauge!("schema_missing_columns").set(n as f64); }`.
- **`src/main.rs`**: после `missing_critical_columns().await` — публикация gauge (даже при 0, чтобы серия существовала и «recovered» был наблюдаем).
- **`docs/prometheus-alerts.yaml`**: alert `WoodyWeedBotSchemaDrift` с summary/description, указывающими на startup-лог и применение миграций.

Проверка: `metrics` defense-тест (call-site) зелёный (8 passed); backend + `cargo check --target wasm32` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

> ⚠️ Ops: `ce0648e` (#18 degrade) + `f36da9c` (#19 self-check) + `8f587bf` (#20 gauge/alert) на `fix/profile-routing` — cherry-pick в `main` + деплой. Grafana подтянет `schema_missing_columns`; если >0 на prod — тикет назовёт недостающие колонки.

### Вариант A — 🔁 Авто-вывод `CRITICAL_COLUMNS` из SELECT'ов (W-48)
Список рукоподдерживаемый. Переиспользовать парсер `schema_drift_tests` (он уже извлекает {table→columns} из SELECT'ов) → defense-тест, что `CRITICAL_COLUMNS` ⊆ реально-SELECT'имых колонок set-таблиц. Единый источник истины, убирает shotgun-surgery.

### Вариант B — ♿ Admin icon a11y + alt-quality (W-47/W-42)
Долейбелить admin icon-кнопки (✏️/👁️/🚫/✕) + эвристика filename-as-alt в img-alt тесте.

### Вариант C — 🧪 Расширить cause-based мониторинг
Применить тот же «definite+imminent cause → ticket» подход к другим точным причинам (напр. отсутствие критичных ENV, недостижимость AI-провайдера на старте). Осторожно — только determinate causes, чтобы не плодить alert-fatigue.

---

## 7. SKILL SAVED

Память: обновлён `api-sets-500-resilience.md` (добавлен metric+alert observability-слой). Принцип: symptom-алерты (5xx) — основа; cause-алерты — только для definite+imminent причин, severity=ticket, дают lead time на деплое.

**Anchor:** `phi^2 + phi^-2 = 3`
