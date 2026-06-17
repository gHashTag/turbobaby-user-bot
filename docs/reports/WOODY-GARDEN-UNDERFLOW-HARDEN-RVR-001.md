# 🌊 WAVE LOOP REPORT — Garden time_to_harvest underflow hardening + subagent-finding triage

**Document ID:** `WOODY-GARDEN-UNDERFLOW-HARDEN-RVR-001`
**Wave:** #64 — fresh-area scan: game/garden progress math (Вариант C из Wave #63)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `e5f5af4` — `fix(garden): saturating_sub in time_to_harvest (no usize underflow on overgrown count)`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — субагент-находка «underflow-паника» проверена как FALSE POSITIVE (guard защищает); всё равно захардил операцию (saturating_sub) + регресс-тест, т.к. защита была хрупкой (decoupled guard).**

Explore-субагент пометил `src/trios/garden.rs:379` (`(TOTAL_WATER_STAGES - 1 - water_count)`) как usize-underflow → панику. **Проверил по достижимости: это НЕ живой баг.** Ветка `else` достижима только когда `is_ready_to_harvest == false`, а `is_ready_to_harvest = water_count >= TOTAL_WATER_STAGES - 1` (=13). Значит в этой ветке всегда `water_count < 13` → `13 - water_count >= 1`, underflow невозможен **сегодня**.

**Честно: «фикса бага» в смысле устранения живой паники не было** — паника недостижима. Но защита держится на *неявном* инварианте (distant guard), decoupled от самой арифметики: `validate_water_count` допускает значения до 100, и любой будущий рефактор guard'а молча открыл бы underflow (в release — wrap к огромному числу, не паника). Поэтому перенёс защиту **внутрь операции**: `(TOTAL_WATER_STAGES - 1).saturating_sub(water_count)` — нулевое изменение поведения для валидных входов, совпадает с saturating-идиомой на соседних строках cooldown. Регресс-тест пинит, что `water_count = 50` не паникует и даёт `is_ready=true / time_to_harvest=0 / stage=Final`.

Главный навык Wave: **проверяй находки субагента/анализатора против guard'а и достижимости перед фиксом** — не фабрикуй ложный фикс; но если guard делает код «safe-but-fragile», хардь саму операцию.

---

## 2. WEAK-SPOT MATRIX

| # | Находка | Файл:строка | Проверка / триаж | Действие |
|---|---|---|---|---|
| — | «usize underflow → panic» в `time_to_harvest` | `garden.rs:379` | FALSE POSITIVE: `is_ready_to_harvest` guard (≥13) делает ветку недостижимой при `water_count≥13` → `13-water_count≥1` | защитил операцию `saturating_sub` + тест (хрупкий decoupled guard) |
| — | `validate_water_count` допускает до 100 | `garden.rs` | расширяет «теоретический» диапазон входа → guard единственная защита | покрыто тем же `saturating_sub` |
| 🚨 | PR ветки (104+ ahead) + deploy #18-20, /api/sets 500 | — | e2e-validated, ждёт sign-off | ⚠️ ops/review |
| 🚀 | W-97: AWS SDK rustls 0.21→0.23 | — | нужен S3 integration-тест | 📋 sign-off / dedicated |

---

## 3. НАУЧНАЯ БАЗА

**Тема: триаж false-positive находок статического анализа по достижимости; defensive saturating-арифметика для unsigned underflow в Rust; хардинг хрупкого инварианта, защищённого distant guard'ом.**

- **Static-analysis FP от over-approximation → триаж по достижимости.** Анализатор (и LLM-субагент) не исполняет код, предполагает worst-case data-flow. Перед фиксом подтверди: достижим ли уязвимый путь, всегда ли вход санитизируется guard'ом до сайта. Caveat: отсутствие доказанного достижимого пути ≠ доказательство безопасности — поэтому хардим, а не просто dismiss'им. «Verify at least once before marking false; document the guard for future reviewers.»
- **Rust НЕ свободен от арифметических ошибок.** В `--release` дефолт для overflow меняется с panic на wrap → unsigned underflow `0 - 1` молча становится огромным числом (классика финтех/blockchain-инцидентов). `saturating_sub` клампит к нулю (правильная доменная семантика «не ниже нуля»); `checked_sub` → `None` когда хочешь явную ошибку. clippy ловит хрупкую арифметику.
- **«Currently guarded» ≠ безопасно: guard decoupled от операции.** corrode.dev: «vector indexing decoupled from the length check → panics if you refactor and forget the is_empty() check»; «violating implicit invariants not enforced by the compiler are the root cause». Перенос защиты **в саму операцию** (`saturating_sub`/`checked_sub`) делает её неотделимой будущими правками.
- **Counterbalance — не over-hardening.** Делай это там, где операция реально хрупкая (distant guard + широкий допустимый вход), а не рефлекторно везде — лишние guard-clause'ы топят читаемость. Здесь: один сайт, идиома уже на соседних строках, нулевое изменение поведения.

Источники:
- [What's a False Positive & How to Triage It (Astra/SAST+DAST)](https://www.getastra.com/blog/dast/false-positive-triage/)
- [Reachability analysis (Snyk User Docs)](https://docs.snyk.io/manage-risk/prioritize-issues-for-fixing/reachability-analysis)
- [Patterns for Defensive Programming in Rust (corrode.dev)](https://corrode.dev/blog/defensive-programming/)
- [Understanding arithmetic overflow/underflows in Rust (Sec3)](https://sec3.dev/blog/understanding-arithmetic-overflow-underflows-in-rust-and-solana-smart-contracts)
- [Rust: detect unsigned integer underflow (frehberg)](https://frehberg.com/2022/01/rust-detect-unsigned-integer-underflow/)
- [Defensive Programming — Friend or Foe? (Memfault Interrupt)](https://interrupt.memfault.com/blog/defensive-and-offensive-programming)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Fresh-area scan game/garden math (Explore-субагент) → flagged `garden.rs:379` underflow.
2. ✅ **Триаж достижимости**: прочитал `is_ready_to_harvest` guard (≥13) + `time_to_harvest = if is_ready {0} else {...}` → ветка недостижима при water_count≥13 → FALSE POSITIVE (живой паники нет).
3. ✅ Оценил хрупкость: `validate_water_count` ≤100, guard decoupled от арифметики → хардинг оправдан (не over-hardening: 1 сайт, идиома рядом).
4. ✅ `saturating_sub` в строке 379 + комментарий (почему: defensive vs corrupt/overgrown count).
5. ✅ Регресс-тест `test_calculate_progress_overgrown_water_count_no_underflow` (water_count=50 → no panic, is_ready, t2h=0, stage=Final).
6. ✅ `cargo test calculate_progress` → 5 passed; `cargo check --target wasm32` clean; pre-commit defensive-tests 69 passed.
7. ✅ Research + отчёт + skill. Честно зафиксировал: FP, не живой фикс; хардинг хрупкого инварианта.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

**`src/trios/garden.rs`** (`calculate_progress`, строка ~379):
```rust
// saturating_sub: the is_ready_to_harvest guard above already keeps us out
// of this branch when water_count >= TOTAL_WATER_STAGES - 1, but compute
// defensively so an over-grown / corrupt count (validate_water_count allows
// up to 100) can never underflow this usize subtraction...
((TOTAL_WATER_STAGES - 1).saturating_sub(water_count) as i64)
    .saturating_mul(WATER_COOLDOWN_MS)
```
+ тест `test_calculate_progress_overgrown_water_count_no_underflow`.

Проверка: `cargo test calculate_progress` → 5 passed; WASM compile clean; lefthook (block-main/check-backend/check-wasm/defensive-tests 69/fmt) + conventional-commits → все зелёные. Поведение для валидных входов (0..13) не изменилось.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
104+ коммита ahead, e2e-validated. Прод-фиксы (#18-20, /api/sets 500) ждут деплоя. Ops/review — твоё «go».

### Вариант B — 🔐 W-97: AWS SDK на rustls 0.23 + S3 integration-тест (нужен sign-off/тест-окружение)
Переключить aws-config/aws-sdk-s3 TLS-feature на rustls-0.23, удалить 3 webpki-0.101 ignore'а, прогнать S3-хендшейк (MinIO/реальный). Требует S3 тест-окружения.

### Вариант C — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс. Кандидаты: garden reward/streak-математика (соседняя с этим Wave), quest-локационная логика, или admin-панель счётчики.

---

## 7. SKILL SAVED

Память: новый `triage-subagent-finding-then-harden.md` + строка в `MEMORY.md`. Принцип: проверяй находку анализатора/субагента против guard'а + достижимости ПЕРЕД фиксом (не фабрикуй ложный фикс, не dismiss молча); если guard делает код safe-but-fragile (decoupled от операции, широкий допустимый вход) — перенеси защиту ВНУТРЬ операции (`saturating_sub`/`checked_sub`), нулевое изменение для валидных входов + регресс-тест; не over-hardening (только хрупкие сайты). Связь: [[fail-loud-not-silent-clamp]], [[invariant-lock-in-when-clean]].

**Anchor:** `phi^2 + phi^-2 = 3`
