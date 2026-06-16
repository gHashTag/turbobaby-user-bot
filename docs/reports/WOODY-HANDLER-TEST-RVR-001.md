# 🌊 WAVE LOOP REPORT — Handler Integration Tests + Harness Prod-DSN Guard

**Document ID:** `WOODY-HANDLER-TEST-RVR-001`
**Wave:** #43 — Вариант B (handler-test harness, W-70)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commits:** `ce8ac01` (admin/check e2e test), `b3eaeb8` (harness prod-DSN guard)
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — harness был уже разблокирован (#162); добавлен e2e регресс-тест W-69 + защита harness от прод-DSN. Всё зелёное.**

Взялся за «handler-test harness» (W-70) — и обнаружил, что блокер **давно решён** (cycle #162): `tests/common/mod.rs` + `tower::oneshot` harness существует, dev-deps (`tower`, `http-body-util`) на месте, есть 7 integration-сьютов. Мой прежний тезис «нет harness» был устаревшим (исправил индекс памяти).

Сделал два реальных вклада:
1. **E2E регресс-тест для W-69** (Wave #36 fix): `GET /api/admin/check` с валидным `X-Admin-Token` возвращает `telegram_id: 0` (sentinel), НЕ клиентский `?telegram_id=999`. Прежде фикс был проверен только рассуждением; теперь — end-to-end через реальный Router. Прогнал зелёным против локальной throwaway-базы (3 passed).
2. **Защита harness от прод-DSN**: окружение содержало **живой Railway prod `DATABASE_URL`**. `cargo test -- --ignored` мигрировал бы и писал в **прод**. Добавил pure `is_safe_test_dsn()` (allow localhost/127.0.0.1/::1 или `*test*`-имя БД) с assert **до** `Database::connect`, + hermetic unit-тест (без БД), фиксирующий что Railway/prod DSN отклоняется.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-70 | Считалось, что нет handler-harness (устаревшая память) | — | ✅ исправлено: harness есть (#162); индекс памяти обновлён |
| W-78 | Harness читал `DATABASE_URL` напрямую → запуск против прод-БД (footgun; env имел live Railway DSN) | 🟠 MED (data-integrity footgun) | ✅ `is_safe_test_dsn` guard до connect + hermetic тест |
| W-69 | Фикс `/admin/check` без runtime-теста | 🟢 INFO | ✅ e2e регресс-тест (3 случая) |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: axum Router integration-тесты через tower oneshot; регресс-тест для security-фикса; ephemeral/throwaway DB, не прод.**

- **`Router` реализует `tower::Service` → тестируй через `oneshot`, минуя сеть**: «faster and more deterministic than network-based testing». Harness ровно так и делает.
- **Регресс-тест для security-фикса**: «assert the fix holds — before the fix this returned X». Мой `telegram_id == 0` — именно это (вернул бы 999 до фикса W-69).
- **Никогда не гоняй деструктивные тесты против прод**: «use a separate env var so a production DSN can never be picked up by accident; add an assertion guard that aborts if the DSN looks like production; ephemeral DB per run». Ровно мой `is_safe_test_dsn`-guard (env реально содержал прод-DSN).
- **type-inference**: `oneshot` доступен на `Router<()>` (после `with_state`) — harness возвращает уже-stateful `api::router(state)`.

Источники:
- [axum Testing Strategies (DeepWiki)](https://deepwiki.com/tokio-rs/axum/12.2-testing-strategies)
- [axum official testing example](https://github.com/tokio-rs/axum/blob/main/examples/testing/src/main.rs)
- [The Ultimate Guide to Axum — testing (Shuttle)](https://www.shuttle.dev/blog/2023/12/06/using-axum-rust)
- [Testing Router with State (axum discussion #1658)](https://github.com/tokio-rs/axum/discussions/1658)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Исследовал блокер → оказался RESOLVED (#162); harness + dev-deps есть.
2. ✅ Нашёл footgun: env `DATABASE_URL` = живой Railway **прод**. Решил гонять только против локальной throwaway-БД.
3. ✅ Проверил harness локально (smoke зелёный против `woody_wave_test`).
4. ✅ Написал e2e регресс-тест W-69 (`/api/admin/check`: no-auth→401, bad-token→401, token→200+`telegram_id:0`); зелёный локально (`--test-threads=1`).
5. ✅ Добавил `is_safe_test_dsn` guard (до connect) + hermetic unit-тест (Railway/prod DSN отклоняется); guard пропускает локальную БД.
6. ✅ Удалил локальную throwaway-БД; обновил индекс памяти.
7. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`tests/integration_admin_check.rs`** (новый): 3 `#[ignore]` e2e теста (oneshot) + 1 hermetic (`harness_rejects_production_dsn`). Использует `woody_weed_bot::api::auth::generate_admin_token` для валидного токена.
- **`tests/common/mod.rs`**: `pub fn is_safe_test_dsn(url) -> bool` (localhost/127.0.0.1/::1 или `*test*`-db); `assert!` в `make_app_with_db` до `Database::connect`.

Проверка: hermetic-тест PASS (без БД); 3 e2e PASS против локальной `woody_wave_test` (`--test-threads=1`); default `cargo test` — e2e корректно `ignored`; backend+WASM; 69 defensive PASS; fmt clean. **Прод-БД не тронута** (guard срабатывает до connect; проверено только локально/hermetic).

Зам.: `--test-threads=1` обязателен — каждый тест строит свой `AppState` + `run_migrations()`; конкурентный DDL на одной БД ⇒ deadlock.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🧪 Расширить e2e-покрытие fail-loud/authz фиксов
Теперь harness доступен и безопасен → написать e2e для self-referral (W-67), use_reward fail-loud (W-72.1), quest fake-id (W-77). Требует local throwaway-PG (CI).

### Вариант B — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс дефектов.

### Вариант C — 🚀/🛡️ Накопленный долг — нужен sign-off
Deploy #18-#20 в `main`; либо PR ветки `fix/profile-routing` (#35-#43, ~18 коммитов) в `main`. Ops/review.

---

## 7. SKILL SAVED

Память: новый `handler-integration-testing.md` + обновлён индекс `integration-test-harness-blocker` (RESOLVED). Принципы: harness через `tower::oneshot` (минуя сеть); регресс-тест security-фикса утверждает именно исправленное поведение; **никогда** деструктивные тесты против прод — guard по DSN до connect (env часто содержит прод-`DATABASE_URL`); authz/sentinel-пути тестируемы без данных в БД; `--test-threads=1` из-за конкурентных миграций.

**Anchor:** `phi^2 + phi^-2 = 3`
