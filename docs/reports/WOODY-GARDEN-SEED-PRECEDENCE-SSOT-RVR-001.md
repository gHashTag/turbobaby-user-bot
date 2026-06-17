# 🌊 WAVE LOOP REPORT — One SSOT for order→seed precedence (dedup the 3rd clone)

**Document ID:** `WOODY-GARDEN-SEED-PRECEDENCE-SSOT-RVR-001`
**Wave:** #66 — fresh-area scan: order-completion → garden auto-seed side-effect (Вариант C)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `90af69b` — `refactor(garden): one SSOT for order→seed precedence (dedup 3rd clone)`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — критичный путь order→garden auto-seed просканирован; реальной уязвимости НЕ найдено, зато найден и устранён Rule-of-Three дрейф-риск: «какой item заказа становится семенем» жило в ТРЁХ копиях, две из которых — line-for-line логические клоны, дрейфующие независимо. Свёл к одному чистому ядру; drift-тест перепривязан.**

Сканировал side-effect авто-посева сада при завершении заказа (`db::orders::complete_order_and_update_loyalty`, cycle #168 — форвард-запись `garden_plants` из `order.items` JSONB). Параллельный self-service путь — `api::garden::force_seed`. Правило выбора семени (precedence `strain→set→accessory→tea`, пропуск пустых id, name-fallback `""`) существовало в **трёх** местах, обязанных оставаться байт-идентичными:

1. `api::garden::force_seed` — аварийный self-service seed
2. `db::orders::complete_order_and_update_loyalty` — side-effect завершения заказа
3. миграция 037 — SQL universal-backfill

Пути **1 и 2 были line-for-line клонами**, дрейфующими независимо — ровно история хотфиксов **026→036→037** (ранний код сеял только из `strain_id`, оставляя accessory/tea/set-only заказы без растения). Memory-нота [[garden-seed-pure-core]] уже вынесла копию #1 в чистую fn `first_seedable_item`, **но оставила её в `api/garden.rs`** и **не тронула клон #2** в `db/orders.rs`. Дыра в самом dedup'е.

**Никакого функционального бага в side-effect не найдено** (subagent-разведка не дала эксплуатируемой находки): INSERT под `WHERE NOT EXISTS (active plant)` сохраняет инвариант «одно активное растение» (cycle #94), `cid` берётся из `order.telegram_id` (не из клиента), идемпотентность завершения держится `if order.status == "completed" { return }`. Это **maintainability/correctness-defense** wave, не security.

**Действие:** перенёс чистую `first_seedable_item` в ядро `trios::garden` (no DB, host-testable, общее backend↔WASM) и вызвал её из **обоих** write-путей. Drift-тест `seed_keys_match_sql_backfill_037` теперь читает файл ядра и держит Rust-precedence запертым против SQL-backfill — новый тип каталога, подключённый в один путь но не в другие, **роняет сборку**. Поведение сохранено: оба клона были item-major с идентичной within-item precedence; все 7 характеризационных `seedable_*` тестов проходят без изменений.

---

## 2. WEAK-SPOT MATRIX

| # | Находка | Файл:строка | Проверка / реальность | Действие |
|---|---|---|---|---|
| ✅ FIX | 3-я копия seed-precedence (Rule of Three) — клон #2 в side-effect завершения заказа | `db/orders.rs:226-264` | line-for-line клон `first_seedable_item`; дрейф = история 026→036→037 | вынос в `trios::garden`, вызов из обоих путей |
| ✅ FIX | `first_seedable_item` оставлена в `api/garden.rs` (не в общем ядре) | `api/garden.rs:844` | прошлый dedup ([[garden-seed-pure-core]]) был неполным | перенос в `trios::garden` |
| ✅ HARDEN | drift-тест читал `api/garden.rs` | `api/garden.rs:1018` | после переноса fn искала бы в неверном файле | перепривязан к `src/trios/garden.rs` + новый разделитель `fn filter_active_rewards` |
| FP/clean | seed-INSERT мог нарушить «1 активное растение» | `db/orders.rs:270` | guarded: `WHERE NOT EXISTS (... is_completed=false)` | — |
| FP/clean | user_id из клиента? | `db/orders.rs:278` | `cid = order.telegram_id` (из БД-строки заказа), не из тела запроса | — |
| FP/clean | двойной seed при повторном complete | `db/orders.rs:~27` | идемпотентно: `if order.status == "completed" { return }` | — |
| 🚨 | PR ветки (106+) + deploy #18-20, /api/sets 500 | — | e2e-validated, ждёт sign-off | ⚠️ ops/review |
| 🚀 | W-97: AWS SDK rustls 0.21→0.23 | — | нужен S3-тест | 📋 sign-off |

---

## 3. НАУЧНАЯ БАЗА

**Тема: Rule of Three / DRY для бизнес-инвариантов; functional-core/imperative-shell; «неправильная абстракция хуже дублирования» — но три синхронных копии одного правила уже за порогом.**

- **Rule of Three (Fowler / Refactoring).** Дублирование терпимо до третьей копии; на третьей — извлекай. Здесь было ровно три копии правила seed-precedence (две Rust-клона + одна SQL), обязанные оставаться синхронными. Извлечение Rust-части в одно ядро + fitness-test против SQL — каноничный ход.
- **DRY = Single Source of Truth для ЗНАНИЯ, не просто кода.** Pragmatic Programmer: дублируется не текст, а *решение* «какой item становится семенем». Три независимо-редактируемых копии этого решения = три места, где забыть синхронизацию (что и случилось в 026→036→037).
- **Functional core, imperative shell.** Чистое правило (precedence, без БД) живёт в `trios::garden` и юнит-тестируется на JSON-входах; обе imperative-оболочки (axum-хендлер, sea-orm tx) лишь вызывают его. Тестируемость + единственность.
- **«Wrong abstraction worse than duplication» (Sandi Metz) — но это НЕ тот случай.** Предупреждение касается *преждевременного* объединения *случайно похожего* кода. Здесь код не случайно похож — это одно бизнес-правило, намеренно скопированное, с явной историей дрейфа. Объединение оправдано фактами, не вкусом.
- **Fitness function вместо устной договорённости.** Drift-тест `seed_keys_match_sql_backfill_037` кодирует «Rust precedence == SQL 037 keys» как исполняемый инвариант; SQL-часть нельзя влить в Rust-ядро (это миграция), поэтому связь между ними держит тест, а не комментарий.

Источники:
- [Rule of Three / Duplicated Code (Refactoring, Fowler)](https://refactoring.com/catalog/extractFunction.html)
- [Don't Repeat Yourself — Pragmatic Programmer (c2 wiki)](https://wiki.c2.com/?DontRepeatYourself)
- [The Wrong Abstraction (Sandi Metz)](https://sandimetz.com/blog/2016/1/20/the-wrong-abstraction)
- [Functional Core, Imperative Shell (Destroy All Software, G. Bernhardt)](https://www.destroyallsoftware.com/screencasts/catalog/functional-core-imperative-shell)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Fresh-area scan (Explore-субагент): order-completion → garden auto-seed side-effect.
2. ✅ Триаж достижимости (read реального кода): seed-INSERT под `WHERE NOT EXISTS`; `cid` из БД; идемпотентность завершения → эксплуатируемого бага нет.
3. ✅ Найдена 3-я копия seed-precedence — клон #2 в `db/orders.rs:226-264` (line-for-line с `first_seedable_item`). Подтверждена поведенческая эквивалентность (оба item-major, та же within-item precedence, та же фильтрация пустых, тот же name-fallback).
4. ✅ Перенёс `first_seedable_item` в ядро `trios::garden` (pub, no-DB); вызов из `force_seed` (`garden::first_seedable_item`) и `complete_order_and_update_loyalty` (`crate::trios::garden::first_seedable_item`).
5. ✅ Перепривязал drift-тест `seed_keys_match_sql_backfill_037` к `src/trios/garden.rs` + разделитель `fn filter_active_rewards`.
6. ✅ Тесты: 9 seed-тестов (7 `seedable_*` + drift + 1 water) ✅; backend **693** ✅; WASM compile ✅; defensive **69** ✅; fmt + conventional-commits ✅.
7. ✅ Research + отчёт + skill.

> **Заметка по окружению:** в начале витка диск тома `/System/Volumes/Data` был заполнен на 100% (ENOSPC — не писались даже tmp-выводы хендлера). Освободил, удалив `target/` (gitignored, регенерируемый build-кэш) → 69 GiB свободно. После этого работа продолжилась штатно.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

**`src/trios/garden.rs`** (функциональное ядро — теперь SSOT):
```rust
/// Single source of truth for "which order item becomes the seed". BOTH the
/// order-completion side-effect (db::orders::complete_order_and_update_loyalty)
/// and api::garden::force_seed call this — precedence identical across the two
/// write paths and the SQL backfill (locked by seed_keys_match_sql_backfill_037).
pub fn first_seedable_item(items: &serde_json::Value) -> Option<(String, String)> {
    const KEYS: [(&str, &str); 4] = [
        ("strain_id", "strain_name"), ("set_id", "set_name"),
        ("accessory_id", "accessory_name"), ("tea_id", "tea_name"),
    ];
    for it in items.as_array()? {
        for (id_key, name_key) in KEYS {
            if let Some(id) = it.get(id_key).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                let name = it.get(name_key).and_then(|v| v.as_str()).unwrap_or("").to_string();
                return Some((id.to_string(), name));
            }
        }
    }
    None
}
```

**`src/db/orders.rs`** (`complete_order_and_update_loyalty`): inline-блок `strain_seed` (39 строк клона) → один вызов `crate::trios::garden::first_seedable_item(&order.items)`.

**`src/api/garden.rs`** (`force_seed`): call site → `garden::first_seedable_item(&items)`; тест-импорт → `use crate::trios::garden::first_seedable_item;`; drift-тест читает `src/trios/garden.rs`.

Diffstat: `+59 / −80` (две копии убраны, одно ядро добавлено). Поведение не изменилось.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR `fix/profile-routing` → `main` + деплой (нужен sign-off)
~107 коммитов ahead, e2e-validated. Прод-фиксы (#18-20, /api/sets 500) ждут деплоя. Ops/review — твоё «go». ⚠️ Не делаю автономно.

### Вариант B — 🔐 W-97: AWS SDK на rustls 0.23 + S3 integration-тест (нужен sign-off/окружение)
Переключить TLS-feature, удалить webpki-0.101 ignore'ы, прогнать S3-хендшейк.

### Вариант C — 🆕 Fresh-area scan (автономно)
Кандидаты: referral-bonus credit chain (`confirm_referral` — кредитует `bonus_balance` + ledger + `referral_events.status='paid'`, вне completion-tx — money-путь, ещё не скан этого витка); quest-локационная логика; admin-метрики/счётчики `/engage`.

⚠️ Долг по деплою стоит — всё в ветке, ничего не задеплоено; A требует твоего «go». Без выбора продолжу автономно с C (referral-bonus chain) на следующем витке.

---

## 7. SKILL SAVED

Память: новый `garden-seed-ssot-dedup.md` + строка в `MEMORY.md`. Принцип: при dedup доводи до конца — найди ВСЕ копии правила (grep по логике, не по имени fn), вынеси в общее ядро, не оставляй в модуле одного из потребителей; на третьей синхронной копии бизнес-правила извлекай (Rule of Three); связь Rust↔SQL, которую нельзя слить в один код, держи fitness-тестом, а не комментарием. Связь: [[garden-seed-pure-core]], [[videomodal-clone-sweep]], [[lang-code-clone-eliminated]], [[migration-wiring-defense]].

**Anchor:** `phi^2 + phi^-2 = 3`
