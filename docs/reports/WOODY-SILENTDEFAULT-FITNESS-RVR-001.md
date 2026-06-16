# 🌊 WAVE LOOP REPORT — Fitness Function Locking the Silent-Default Class

**Document ID:** `WOODY-SILENTDEFAULT-FITNESS-RVR-001`
**Wave:** #41 — фитнес-функция против класса silent-default (Вариант A из Wave #40)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `d1a35c1` — `test(garden,orders): fitness fn pinning sensitive mutations fail-loud; fix water_plant read-error swallow`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — класс W-72/W-74 зафиксирован фитнес-функцией; найден+починен остаточный баг; 43 garden + 69 defensive + новый тест зелёные.**

Закрыл класс silent-default постоянной защитой от регрессии. Сначала проверил наивный подход (codebase-wide бан `try_get(<sensitive col>).unwrap_or`): **34 сайта**, большинство легитимны (display-defaults, намеренные config-fallback'и) → шумно, низкий signal. Поэтому сделал **таргетную** фитнес-функцию: парсит тела 4 финансовых/security **мутаций** (`use_reward`, `water_plant`, `complete_order_and_update_loyalty`, `auto_block_for_fraud`) и падает, если sensitive-колонка читается с silent default.

**Построение guard'а само нашло остаточный баг** (классика): `water_plant` всё ещё читал `water_count`/`is_completed` через `.unwrap_or` — Wave #38 починил случай отрицательного *значения* (`next_water_step`), но **read-ошибка** (DbErr) до сих пор тихо давала 0 → сброс растения в Seed. Починил (fail loud, в tx). Также при первом прогоне тест поймал false-positive (`config->>'referral_bonus' AS bonus` — намеренный config-default) → уточнил `SENSITIVE_COLS` до точных колонок bug-сайтов (signal-to-noise).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-75 | `water_plant`: read-*ошибка* на `water_count`/`is_completed` → `.unwrap_or` → тихий сброс в Seed (остаток W-72) | 🟡 LOW (data-integrity) | ✅ fail loud (`map_err→500`, в tx) |
| W-76 | Класс silent-default не имел регрессионного guard'а | 🟢 INFO | ✅ `sensitive_read_fail_loud_tests` (таргетный source-scan) |
| — | Codebase-wide column-бан отвергнут: 34 сайта, ~30 легитимны → noisy | ✅ | осознанно НЕ сделан |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: architectural fitness functions; ratchet/baseline; scoped vs codebase-wide (signal-to-noise).**

- **Fitness function = unit-тест для структуры/свойства** (Ford/Parsons/Kua, *Building Evolutionary Architectures*): «protects an architectural property … prevents regression». Мой тест защищает свойство «sensitive-мутации читают fail-loud».
- **«Lock, not a goal»**: «clean up first, THEN add the fitness function to prevent regression — it protects work you already did». Ровно: зафиксировал sweep #38-#40 ПОСЛЕ починки.
- **«Не пиши fitness function для кодовой базы, которую хочешь, а не которая есть — сломает всё в день один»**: точное обоснование, почему таргет на 4 починенные функции, а не бан на 34 сайта.
- **Signal-to-noise / «every signal actionable»** (Notion ratchet): подавляй известный/легитимный «долг», алертит только на новую регрессию. → уточнение `SENSITIVE_COLS` (убрал грубые `bonus`/`count`/`total`, поймавшие config-default).

Источники:
- [Fitness Functions: Unit Tests for Your Architecture (Ramirez)](https://xpromx.me/articles/fitness-functions-unit-tests-for-your-architecture/)
- [Building Evolutionary Architectures — fitness functions intro (Yonder)](https://medium.com/yonder-techblog/architectural-fitness-functions-an-intro-to-building-evolutionary-architectures-dc529ac76351)
- [Imbue `ratchets` — progressive lint enforcement](https://github.com/imbue-ai/ratchets)
- [Notion: ratcheting ESLint, signal-to-noise](https://www.notion.com/blog/how-we-evolved-our-code-notions-ratcheting-system-using-custom-eslint-rules)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Перечислил sensitive-col silent-default сайты (34) → вывод: codebase-wide бан noisy.
2. ✅ Выбрал таргетный scope: 4 финансовых/security мутации (точечный, высокий signal).
3. ✅ При проектировании нашёл остаток W-75 (`water_plant` read-error swallow) → fail loud.
4. ✅ `sensitive_read_fail_loud_tests` (brace-parser тел функций × `SENSITIVE_COLS`).
5. ✅ Первый прогон → false-positive (`referral_bonus AS bonus` config-default) → уточнил `SENSITIVE_COLS` до точных колонок.
6. ✅ Тест зелёный; 43 garden + 69 defensive PASS; backend+WASM; fmt.
7. ✅ Research (fitness functions/ratchet) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/api/garden.rs`** (`water_plant`): `is_completed`/`water_count` → `.map_err(…→500)?` (было `.unwrap_or`), `last_watered_at` остаётся `.ok()` (намеренно optional).
- **`src/main.rs`**: `#[cfg(test)] mod sensitive_read_fail_loud_tests` — `SENSITIVE_FNS` (4) × `SENSITIVE_COLS` (точные); `fn_body` brace-matcher; per-`try_get` извлечение колонки (2-й строковый литерал) + проверка statement на `unwrap_or`/`max(0)`; падает с инструкцией (fail loud или вынеси колонку из списка с обоснованием).

Проверка: `cargo test sensitive_read_fail_loud` → PASS; garden 43 PASS; defensive 69 PASS; WASM 0 warnings; fmt clean. Честно: тест проверяет *исходник* (presence of pattern), не runtime-поведение; runtime fail-loud — через сам код.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🆕 Fresh-area scan
Следующий нетронутый модуль на реальный дефект (новый класс).

### Вариант B — 🧪 Handler-test harness (W-70)
Решить [[integration-test-harness-blocker]] (lib.rs WASM-only), чтобы покрыть authz/financial handler'ы end-to-end (включая fail-loud-пути, которые сейчас не runtime-тестируемы). Крупная структурная задача.

### Вариант C — 🚀/🛡️ Накопленный долг — нужен sign-off
Deploy #18-#20 в `main`, либо DB CHECK self-ref (W-68). Ops/migration.

---

## 7. SKILL SAVED

Память: новый `scoped-fitness-function-ratchet.md` + строка в `MEMORY.md`; обновлён [[fail-loud-not-silent-clamp]] (W-75/W-76). Принципы: (1) фиксируй починенный класс багов фитнес-функцией («lock, not goal»); (2) скоупь узко (починенные горячие функции), а не codebase-wide — иначе шум из легитимных дефолтов; (3) построение guard'а часто вскрывает остаточный баг — ожидай этого; (4) подгоняй allow-list под signal (убирай токены, ловящие config-defaults).

**Anchor:** `phi^2 + phi^-2 = 3`
