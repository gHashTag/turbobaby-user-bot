# 🌊 WAVE LOOP REPORT — Sanitize garden reward config at the harvest (financial) path

**Document ID:** `WOODY-GARDEN-REWARD-CONFIG-SANITIZE-RVR-001`
**Wave:** #65 — fresh-area scan: garden reward / loyalty bonus math (Вариант C)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `f649603` — `fix(garden): sanitize reward config at harvest so corrupt values can't drain bonus_balance`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — закрыта реальная асимметрия defense-in-depth: harvest-путь (создаёт финансовую награду) теперь санитизирует config из БД, как и display-путь; clamp + ГРОМКИЙ лог; subagent-находки про overdraw/negative проверены — все остальные уже guarded (FP).**

Просканировал две свежих области: (1) garden reward/streak-математику и (2) loyalty bonus-арифметику (`add_bonus`/`use_bonus`/balance). **Subagent выдал 8 находок — 7 оказались FP или by-design** (overdraw защищён `WHERE bonus_balance >= amount`; client `bonus_used` валидируется `.max(0)`+`is_finite`+`<= subtotal`; streak-системы вообще нет; `i32→f64` точен до 2^53). Это снова подтвердило правило: проверяй находки до фикса.

**Одна находка — реальная (low-prob, но финансовая):** в `harvest_plant` config (`reward_discount_percent/bonus_points/expiration_days`) читается из `garden_config` **без range-клампа**, и `bonus_points` потом КРЕДИТУЕТСЯ в `bonus_balance` через `use_reward` (`bonus_balance + $1`). Админ-API — `u32` (отрицательное через API недостижимо), **но** `get_config` (отображение) уже защитно клампит `.max(0)`, а вот **финансовый harvest-путь — нет**. Порченая/мигрированная-криво строка БД с `bonus_points = -200` молча **вычла бы** из баланса юзера; отрицательный discount превратился бы в наценку.

Закрыл: чистая `trios::garden::sanitize_reward_config` клампит в тот же домен, что валидатор апдейта (discount `0..=100`, bonus `0..=1_000_000`, days `1..=365`) и возвращает флаг `corrupted` → `harvest_plant` логирует кламп **громко** (fail loud, degrade safe — не блокирует легитимный harvest, не молчит). 4 хост-теста (вкл. `i32::MIN/MAX`).

---

## 2. WEAK-SPOT MATRIX

| # | Находка (subagent) | Файл:строка | Проверка / реальность | Действие |
|---|---|---|---|---|
| ✅ FIX | reward config не клампится в harvest (финансовый путь) | `api/garden.rs:384` | РЕАЛЬНО, но достижимо лишь через порчу БД (API=u32). Асимметрия с `get_config` (тот клампит) | `sanitize_reward_config` + loud log + 4 теста |
| FP | overdraw bonus_balance (client `bonus_used`) | `orders.rs:144`, `:831` | guarded: `.max(0)`+`is_finite`+`<=subtotal`, и `WHERE BonusBalance.gte(bonus_used)` | — |
| FP | `use_bonus` amount > баланса | `loyalty.rs:440` | guarded: `WHERE BonusBalance.gte(amount)` + `validate_use_bonus_amount` | — |
| FP | streak/daily-login TZ-баг | — | системы streak НЕТ в коде | — |
| FP | `i32→f64` потеря точности | `garden.rs:636` | i32::MAX ≪ 2^53 → точно | — |
| FP | balance display `.max(0.0)` прячет порчу | `loyalty.rs:103` | by-design; ledger `bonus_transactions` хранит полную историю | — |
| FP | expiration overflow | `api/garden.rs:393` | `saturating_add` + days клампится теперь к ≤365 | покрыто фиксом |
| 🚨 | PR ветки (104+) + deploy #18-20, /api/sets 500 | — | e2e-validated, ждёт sign-off | ⚠️ ops/review |
| 🚀 | W-97: AWS SDK rustls 0.21→0.23 | — | нужен S3-тест | 📋 sign-off |

---

## 3. НАУЧНАЯ БАЗА

**Тема: defense-in-depth валидация на КАЖДОЙ trust boundary (вкл. чтение из своей БД); server-side authority для денег; range-валидация перед расчётом; fail loud vs silent clamp.**

- **БД — это trust boundary.** OWASP: «data from all potentially untrusted sources should be subject to input validation… not only Internet-facing clients but also backend feeds» — персистентные данные могли быть записаны скомпрометированным путём / кривой миграцией / руками. Валидируй на ЧТЕНИИ, не только на записи.
- **Server-side authority для денег непреложно.** Любая ранняя валидация может быть обойдена/повреждена; авторитетная проверка живёт на сервере, **непосредственно перед расчётом** (pre-calculation filter «present / typed / in-range / consistent»).
- **Range-валидация — штатный контроль для денежных значений** (явные min/max до сохранения и до расчёта).
- **Clamp vs reject — осознанный выбор.** «Clamping silently masks corruption → для денег безопаснее fail loud / reject + log; clamp допустим для non-critical, если нарушение ЗАФИКСИРОВАНО.» Мой выбор — **clamp + `tracing::error!`** (degrade-safe: не блокировать легитимный harvest из-за should-never-happen админ-порчи; баланс защищён; порча наблюдаема в логах + ledger). Честно: строже было бы reject (500), но это наказывает юзера за админскую ошибку.
- **Input validation = defense-in-depth, не первичный контроль.** Это второй слой к валидатору апдейта.

Источники:
- [Input Validation Cheat Sheet (OWASP)](https://cheatsheetseries.owasp.org/cheatsheets/Input_Validation_Cheat_Sheet.html)
- [Defence in Depth, Part 4: Validate Everything (DZone)](https://dzone.com/articles/defence-in-depth-part-4-validate-everything-parame)
- [How to Validate Financial Data Before Modeling (FMP)](https://site.financialmodelingprep.com/insights/enterprise/-how-to-validate-financial-data-before-it-enters-a-financial-model)
- [Server-side Validation (Klarna Open Banking)](https://docs.openbanking.klarna.com/xs2a/xs2a-form/server-side-validation.html)
- [The Essential Guide to Input Validation (Aptori)](https://www.aptori.com/blog/the-essential-guide-to-input-validation-for-secure-software)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Fresh-area scan (Explore-субагент): garden reward + loyalty bonus math → 8 находок.
2. ✅ **Триаж по достижимости** (read реального кода): overdraw/`use_bonus`/`i32→f64`/streak → FP/by-design. Одна реальная: harvest config без клампа (асимметрия с `get_config`).
3. ✅ Проверил тип `ConfigUpdateRequest` = `Option<u32>` → негатив через API недостижим (subagent переоценил severity) — но DB-порча достижима, путь финансовый.
4. ✅ Чистая `sanitize_reward_config` (домен валидатора) + флаг `corrupted`; в `harvest_plant` — clamp + `tracing::error!`.
5. ✅ 4 хост-теста (valid passthrough, negative bonus → 0, over-range clamp, `i32::MIN/MAX` без паники).
6. ✅ `cargo test sanitize` 3✅ (+1 в общем), `cargo check --features backend` clean; lefthook (defensive 69 + conventional) зелёные.
7. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

**`src/trios/garden.rs`** (функциональное ядро):
```rust
pub struct SanitizedRewardConfig { pub discount_percent: i32, pub bonus_points: i32,
    pub expiration_days: i32, pub corrupted: bool }

pub fn sanitize_reward_config(discount_percent: i32, bonus_points: i32, expiration_days: i32)
    -> SanitizedRewardConfig {
    let d = discount_percent.clamp(0, 100);
    let b = bonus_points.clamp(0, 1_000_000);
    let days = expiration_days.clamp(1, 365);
    SanitizedRewardConfig { discount_percent: d, bonus_points: b, expiration_days: days,
        corrupted: d != discount_percent || b != bonus_points || days != expiration_days }
}
```
+ 4 теста.

**`src/api/garden.rs`** (`harvest_plant`): читает raw config → `garden::sanitize_reward_config(...)` → `if sane.corrupted { tracing::error!(...) }` → использует sane-значения в `INSERT garden_rewards`.

Проверка: backend+WASM compile clean; defensive-tests 69 passed; fmt clean; conventional-commits passed. Поведение для валидного config (как у всех прод-строк) не изменилось.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR `fix/profile-routing` → `main` + деплой (нужен sign-off)
~106 коммитов ahead, e2e-validated. Прод-фиксы (#18-20, /api/sets 500) ждут. Ops/review — твоё «go».

### Вариант B — 🔐 W-97: AWS SDK на rustls 0.23 + S3 integration-тест (нужен sign-off/окружение)
Переключить TLS-feature, удалить webpki-0.101 ignore'ы, прогнать S3-хендшейк.

### Вариант C — 🆕 Fresh-area scan
Кандидаты: order-completion → garden auto-seed side-effect (cycle #168, форвард-запись `garden_plants` из order items JSONB) — критичный путь, ещё не скан; quest-локационная логика; admin-метрики/счётчики `/engage`.

---

## 7. SKILL SAVED

Память: новый `db-read-trust-boundary-financial.md` + строка в `MEMORY.md`. Принцип: данные из СВОЕЙ БД на пути к деньгам — это trust boundary; валидируй на ЧТЕНИИ, не только на записи; держи симметрию между display- и financial-путями (display клампил, harvest — нет → дыра); clamp + ГРОМКИЙ лог для degrade-safe (не блокировать легитимное действие из-за should-never-happen порчи), reject строже но наказывает юзера за админ-ошибку; вынеси в чистое ядро + хост-тесты. Связь: [[fail-loud-not-silent-clamp]], [[triage-subagent-finding-then-harden]], [[server-price-authority]].

**Anchor:** `phi^2 + phi^-2 = 3`
