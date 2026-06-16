# 🚨🌊 WAVE LOOP REPORT — /api/sets 500 Hotfix + Icon-Button a11y

**Document ID:** `WOODY-SETS-500-RESILIENCE-RVR-001`
**Wave:** #18 — прерван prod-инцидентом (GET /api/sets 500); затем продолжен a11y (Вариант A из Wave #17)
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commits:** `ce0648e` (prod hotfix), `83879d3` (icon-button aria-label)
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟠→🟢 — инцидент смягчён (graceful degradation), a11y-фикс добавлен; требуется деплой на prod + проверка миграций.**

Во время a11y-волны пришёл **prod-алерт: `GET /api/sets` → HTTP 500** (повтор инцидента 2026-06-05). Приостановил волну, диагностировал, выкатил mitigation:

- **Диагностика:** `schema_drift_tests` (парсит миграции по таблицам) **проходит** → код↔миграции консистентны per-table; все SELECT-колонки объявлены. Значит причина — **состояние prod-БД** (неприменённая миграция / тип/данные), а не SQL. Доступа к prod-БД из лупа нет.
- **Mitigation (`ce0648e`):** публичный `get_sets` комбинирует `accessory_sets` + `tea_sets`; раньше ошибка любого из двух запросов роняла весь эндпоинт в 500 и обнуляла страницу /sets. Теперь каждый источник **деградирует в пустой список** при ошибке (с логом причины) → страница рендерит, что может, 200 вместо 500.

Затем завершил a11y-фикс (`83879d3`): `aria-label` на icon-only кнопки.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| 🚨 W-44 | `GET /api/sets` → 500: один битый set-источник ронял весь эндпоинт | 🔴 HIGH | ✅ Graceful degradation (mitigation); ⚠️ нужен деплой + проверка prod-миграций |
| W-45 | Icon-only кнопки (✕ ▶️ − + +🛒) без accessible name (WCAG 4.1.2) | 🟠 MED | ✅ aria-label на customer-facing кнопки |
| W-46 | `schema_drift_tests` зелён, но prod-БД может отставать по миграциям | 🟠 MED | 📋 нужен runtime-чек применённых миграций (см. §6) |
| W-47 | Admin icon-кнопки (✏️/👁️/🚫/✕) без aria-label | 🟡 LOW | 📋 deferred (staff-tool) |

---

## 3. НАУЧНАЯ БАЗА

**Тема: graceful degradation / fault isolation (design for partial failure).**

- «A single failing dependency should never take down your whole API. Design for partial failure… categorize every dependency as critical (fail) or non-critical (degrade).» Два set-источника теперь **non-critical**: при ошибке одного отдаём остальное (или пусто), не 500.
- **Bulkhead / fault isolation**: «isolate failures… preventing blast radius from spreading». Изоляция: сбой `accessory_sets` не валит `tea_sets` и наоборот.
- «Fast failure beats slow failure… continue operating with limited functionality instead of completely failing.» Лог сохраняет root-cause для ops.
- a11y: **WCAG 4.1.2 (Name, Role, Value)** — у кнопки должно быть accessible name; icon-only без `aria-label` скринридер озвучивает как «button» без смысла.

⚠️ Честно: mitigation **маскирует** симптом, не лечит root-cause. Настоящая причина (prod-БД) требует проверки применённых миграций на prod.

Источники:
- [API Gateway Resilience & Fault Tolerance (Zuplo)](https://zuplo.com/learning-center/api-gateway-resilience-fault-tolerance)
- [Graceful Degradation Patterns (DEV)](https://dev.to/young_gao/graceful-degradation-4b5p)
- [Bulkhead vs Circuit Breaker (Parser/Medium)](https://medium.com/@parserdigital/resilience-in-microservices-bulkhead-vs-circuit-breaker-54364c1f9d53)
- [Error handling in distributed systems (Temporal)](https://temporal.io/blog/error-handling-in-distributed-systems)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Инцидент: приостановил a11y, диагностировал (schema_drift зелён → prod-БД, не код).
2. ✅ Mitigation: `get_sets` — оба источника `unwrap_or_else(|e| { log; Vec::new() })`; backend компилируется; commit `ce0648e`.
3. ✅ Продолжил a11y: `aria-label` на ✕/−/+ (modal), −/+ (cart), ▶️ (3 каталога), +🛒 (sommelier ×2); WASM 0 warnings; commit `83879d3`.
4. ✅ (Решено НЕ ставить fragile icon-button fitness-guard — детекция icon-only лейблов ненадёжна; ручной аудит.)
5. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/api/catalog.rs` `get_sets`** (public): `accessory_sets`/`tea_sets` запросы → `.unwrap_or_else(|e| { tracing::error!(…degraded to empty…); Vec::new() })` вместо `.map_err(…500)?`.
- **a11y** (`"aria-label"`): `product_detail_modal.rs` (Закрыть/Убавить/Добавить количество), `cart_screen.rs` (Убавить/Добавить — `+` через add_item), `accessories/sets/tea` (Смотреть видео), `sommelier_screen.rs` (В корзину ×2).

Проверка: `cargo check --bin … --features backend` (hotfix) + `cargo check --target wasm32` (a11y) → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🩺 Root-cause /api/sets: prod migration introspection (W-46)
Добавить `/healthz`-расширение или admin-эндпоинт, который сверяет применённые на prod миграции (или `information_schema.columns` для `accessory_sets`/`tea_sets`) с ожидаемым набором — превращает «prod отстал по миграциям» из невидимого в наблюдаемое. Плюс: цель — найти и устранить настоящую причину 500.

### Вариант B — 🧱 Generalize graceful degradation
Применить тот же non-critical-source паттерн к другим «комбинирующим» эндпоинтам (если есть), и/или вынести хелпер `query_all_or_empty(...)` с единым логированием.

### Вариант C — ♿ Admin icon-button a11y + alt-quality (W-47/W-42)
Долейбелить admin icon-кнопки (✏️/👁️/🚫/✕) + эвристика «плохого alt» (filename-as-alt). Завершает a11y-покрытие.

---

## 7. SKILL SAVED

Память: новый `api-sets-500-resilience.md` (инцидент, диагностика, graceful-degradation, «проверь prod-миграции») + обновлён UX/a11y skill (icon-button aria-label). Связь: [[schema-drift-defense]], [[migration-wiring-defense]], [[detail-modal-addtocart]].

**Anchor:** `phi^2 + phi^-2 = 3`
