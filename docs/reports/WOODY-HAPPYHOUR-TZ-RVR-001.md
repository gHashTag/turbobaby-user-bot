# 🌊 WAVE LOOP REPORT — Happy-Hour in Shop Timezone, not Server-Local (W-94)

**Document ID:** `WOODY-HAPPYHOUR-TZ-RVR-001`
**Wave:** #60 (milestone) — time/date fresh-area scan (Вариант C из Wave #59)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `7c8d51e` — `fix(happy-hour): compute hour in shop timezone (Asia/Bangkok), not server-local`
**Agent:** Claude Opus 4.8 (Wave loop) + Explore-subagent

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — happy-hour окно считается в TZ магазина; 13 happy_hour + 69 defensive зелёные; clippy чист.**

Time/date-скан. **Хорошая новость**: остальная time-математика **консистентна** (всё в millis через `timestamp_millis()` / UTC; auth_date freshness, idempotency, fraud-window, cooldowns — без unit-mismatch). Один реальный дефект: `get_happy_hour` считал текущий час через `chrono::Local::now().hour()` — **серверное** локальное время. Railway бежит в **UTC**, поэтому happy-hour 18:00–21:00 (shop-local, Koh Phangan/Asia-Bangkok UTC+7) читался как активный в **11:00–14:00 UTC** — клиент видел скидку в неверном окне (сдвиг на 7 часов) или не видел вовсе.

**Серьёзность — MEDIUM** (промо/pricing-feature, customer-facing, неверное окно скидки). Фикс: считать час в фиксированном offset магазина (UTC+7, без DST). Извлёк pure `hour_in_offset(utc, offset_secs)` + `shop_hour_now()`; 3 теста пинят Bangkok-сдвиг, переход через полночь, диапазон. Без `unwrap`/`expect` (clippy-deny) — `unwrap_or_else(|| Utc.fix())`.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-94 | `get_happy_hour`: `Local::now().hour()` (server-TZ=UTC) vs shop-local часы → окно сдвинуто на 7ч | 🟠 MED (promo/pricing correctness) | ✅ `hour_in_offset` UTC+7 + тесты |
| — | Прочая time-математика (millis/UTC, auth/idemp/fraud/cooldown) — консистентна | ✅ | подтверждено скан-агентом |
| 🚨 | PR ветки (104+ ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: store UTC / display local; `Local::now()` pitfall (cloud=UTC); business hours в явной TZ.**

- **`Local::now()` pitfall**: «cloud VMs default to UTC; `Local::now()` silently returns UTC, not your business timezone». Ровно баг.
- **Works-locally-fails-in-cloud**: «date calc looked correct in local dev but failed in CI/CD running UTC … assumed server TZ == business TZ».
- **Business hours → явная TZ**: «for business logic never rely on `now()` without an explicit timezone … anchor to a named zone». → FixedOffset(UTC+7).
- **Не выставляй server-TZ в local**: «DST → scheduled jobs go missing; PST-server+UTC-db → divergence». Bangkok без DST → FixedOffset точен (для DST-зоны нужен `chrono-tz`).
- **Не «сдвигай» хардкод-часы**: «naively shifting hours just relocates the bug». Якорь к offset, не правка start/end.

Источники:
- [JavaScript Timestamp/UTC bugs (OSTechNix)](https://ostechnix.com/javascript-timestamp-bug-utc-timezone-fix/)
- [Fix UTC timezone breaking CI/CD & date logic (Job/Medium)](https://medium.com/@tojosphine/how-to-fix-utc-time-zone-issues-breaking-your-ci-cd-jobs-and-date-logic-d96caa6a5d7c)
- [Worst server setup mistake — don't set server to local TZ (Yeller)](http://yellerapp.com/posts/2015-01-12-the-worst-server-setup-you-can-make.html)
- [Handling Local Timezones, UTC, DST (Safe/FME)](https://support.safe.com/hc/en-us/articles/25407590058765-Handling-Local-Timezones-UTC-Daylight-Savings-Time-and-Leap-Units)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Time-скан: остальное консистентно (millis/UTC); дефект — `Local::now()` в happy_hour.
2. ✅ Подтвердил: Railway/UTC → `Local::now()` == UTC → окно сдвинуто на shop offset.
3. ✅ `const SHOP_UTC_OFFSET_SECS = 7*3600`; pure `hour_in_offset(utc, secs)` (FixedOffset, `unwrap_or_else(Utc.fix())` — без unwrap/expect); `shop_hour_now()`.
4. ✅ `get_happy_hour` → `shop_hour_now()`.
5. ✅ 3 теста (Bangkok-сдвиг 11:30 UTC→18; полночь 20 UTC→3; диапазон); 13 happy_hour PASS; clippy чист; defensive 69 PASS; fmt.
6. ✅ Research (Local::now pitfall / business-hours-TZ) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/api/happy_hour.rs`**: `SHOP_UTC_OFFSET_SECS`; `fn hour_in_offset(utc: DateTime<Utc>, offset_secs: i32) -> i64` (`FixedOffset::east_opt(...).unwrap_or_else(|| Utc.fix())` + `.with_timezone().hour()`); `fn shop_hour_now()`; `get_happy_hour` использует `shop_hour_now()` вместо `Local::now().hour()`. Тесты `test_hour_in_offset_{bangkok_shift,wraps_past_midnight,in_range}`.

Проверка: `cargo test happy_hour` → 13 PASS; clippy чист (нет unwrap/expect/unused); defensive 69 PASS; backend+WASM; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
104+ коммита ahead. Ops/review — твоё «go».

### Вариант B — 🌍 chrono-tz для DST-устойчивости (если когда-нибудь другой регион)
Сейчас Bangkok без DST → FixedOffset точен; миграция на `chrono-tz` нужна только при DST-зоне. INFO.

### Вариант C — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

---

## 7. SKILL SAVED

Память: новый `shop-timezone-utc-offset.md` + строка в `MEMORY.md`. Принцип: бизнес-часы (happy hour, day-boundary) считай в ЯВНОЙ TZ магазина, не `Local::now()` (на cloud=UTC он тихо отдаёт UTC → «работает локально, ломается в проде»); храни UTC, конвертируй на границе; Bangkok без DST → FixedOffset(UTC+7) точен (DST-зона → chrono-tz). Не «сдвигай» хардкод-часы.

**Anchor:** `phi^2 + phi^-2 = 3`
