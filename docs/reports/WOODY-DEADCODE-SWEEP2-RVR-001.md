# 🌊 WAVE LOOP REPORT — Dead-Code Sweep (orders / tech_tree / treasure_hunt)

**Document ID:** `WOODY-DEADCODE-SWEEP2-RVR-001`
**Wave:** #10 — реализован «Dead-code sweep остальных allow» (Вариант A из Wave #9)
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `0ca3bc3` — `refactor(ui): clear dead-code suppressions on orders/tech_tree/treasure_hunt`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — WASM без warning'ов; все `#[allow(dead_code)]` в этих 3 экранах устранены; хуки прошли.**

Завершил зачистку подавлений мёртвого кода по оставшимся screen-DTO. Probe выявил два **разных** класса (и это — важный нюанс):

- **`orders::ApiOrderItem`** — `strain_id`/`accessory_id`/`tea_id`/`set_id` десериализуются serde, но список заказов рендерит только **имя** (`item_name`). Структура **не** derive'ит `PartialEq`, поэтому rustc справедливо считает их мёртвыми. → удалены (serde игнорирует немоделированные ключи).
- **`tech_tree::TechNode`, `treasure_hunt::Hunt`** — поля помечены `#[allow(dead_code)]`, но структуры derive'ят `PartialEq`, чья сгенерированная реализация **читает все поля**. rustc это видит → поля не мертвы → подавления **избыточны**. → allow'ы сняты.

Итог: либо удалил мёртвое (orders), либо вернул компилятору видимость (tech_tree/treasure_hunt). 0 warning'ов.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-26 | `#[allow(dead_code)]` в orders/tech_tree/treasure_hunt | 🟡 LOW | ✅ Все сняты |
| W-27 | `ApiOrderItem` тащил 4 product-id, которые нигде не читаются | 🟡 LOW | ✅ Удалены (serde игнорит ключи) |
| W-15 | 5 локальных `Api*` DTO дублируют `types.rs` | 🟠 MED | 📋 осознанно отложено (wrong-abstraction риск) |
| W-21 | Admin `sets`-таблица без `_en` | 🟡 LOW | 📋 Wave+1 (нужна миграция) |
| W-28 | `#[allow(dead_code)]` вне screens (api/db/components?) не аудирован | 🟢 INFO | 📋 Wave+1 |

---

## 3. НАУЧНАЯ БАЗА

**Тема: пределы статического dead-code анализа вокруг сериализации и генерируемого кода.**

Исследования подтверждают, что dead-code анализ **труден именно там, где поля используются только сериализацией/рефлексией/сгенерированным кодом** — нет прямых ссылок в исходнике:
> «false positives … when fields/types are used only by serialization, DI containers, or generated code with no direct source references … this process cannot be automated as it requires deep knowledge about the code base.»

Наш случай — точный пример различия:
- **serde `Deserialize`** заполняет поле, но rustc считает «только записывается, не читается» → помечает мёртвым (корректно: десериализованное-но-нечитаемое поле — балласт). Поэтому `ApiOrderItem._id` справедливо пойманы.
- **`#[derive(PartialEq)]`** генерирует `eq`, который **читает** все поля; rustc видит этот сгенерированный код → поля живы. Поэтому `#[allow(dead_code)]` на них были избыточны (FSE 2025: 50.8% подавлений бесполезны — Wave #9).

Урок: прежде чем верить/ставить `#[allow(dead_code)]`, проверь, не оживляет ли поле derive (PartialEq/Hash/Serialize-для-вывода). rustc точен по derive, но трактует serde-deserialize как write-only.

Источники:
- [An Empirical Study of False Negatives/Positives of Static Code Analyzers (arXiv 2408.13855)](https://arxiv.org/pdf/2408.13855)
- [Detect and Remove Dead Code (NDepend) — reflection/framework false positives](https://www.ndepend.com/docs/detect-and-remove-dead-code)
- [Finding unreachable functions with deadcode (Go team) — conservative reflection handling](https://go.dev/blog/deadcode)
- [Suppressed Static Analysis Warnings (FSE 2025)](https://software-lab.org/publications/fse2025_suppressions.pdf) (Wave #9 база)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Probe: снял все allow в 3 файлах → только `ApiOrderItem._id` warning.
2. ✅ Расследование расхождения: grep показал tech_tree-поля «1 вхождение» (мёртвы?), но per-file probe — 0 warning → derive(PartialEq) их читает.
3. ✅ orders: удалил 4 мёртвых `_id` поля (serde игнорит ключи; нет PartialEq).
4. ✅ tech_tree/treasure_hunt: снял избыточные per-field allow (живы через PartialEq).
5. ✅ `cargo check --target wasm32` → **0 warnings**; `cargo fmt`; commit.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- `orders_screen.rs`: `ApiOrderItem` теперь `{ strain_name, accessory_name, tea_name, set_name, quantity }` (4 `_id` удалены). Если позже понадобится «повторить заказ»/ссылка на товар — вернуть с реальным использованием (YAGNI).
- `tech_tree_screen.rs`: сняты 4 per-field allow с `TechNode` (живы через `PartialEq`).
- `treasure_hunt_screen.rs`: сняты 2 per-field allow с `Hunt` (живы через `PartialEq`).

Проверка: `cargo check --target wasm32-unknown-unknown` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🗂️ Admin native Sets localization (W-21)
Миграция `sets +name_en/description_en` → admin `get_sets`/`set_row`/`create_set`/`update_set` + форма. Активирует migration-wiring + schema-drift defenses (Wave #1). Завершает локализацию для админов.

### Вариант B — 🔎 Audit allow(dead_code) вне screens (W-28)
Probe `#[allow(dead_code)]` в `src/api`, `src/db`, `src/ui/components` — где избыточно (derive/тесты) убрать, где прячет мёртвое — удалить/использовать. Расширяет гигиену подавлений за пределы экранов.

### Вариант C — 🧪 Generalize fetched-but-never-shown guard
Расширить fitness-тест Wave #8: любое response-DTO поле под `src/ui/screens`, десериализуемое serde но не читаемое (с allowlist для derive-only). Портативный «no serde-only-dead-fields» тест, дополняющий rustc там, где UI в wasm-lib не виден host-сборкой.

---

## 7. SKILL SAVED

Память: обновлён `deadcode-suppression-hygiene.md` (sweep завершён по 3 экранам; зафиксирован нюанс serde-deserialize vs derive(PartialEq) для dead-code анализа). Принцип: проверяй, не оживляет ли поле derive, прежде чем ставить/верить `#[allow(dead_code)]`.

**Anchor:** `phi^2 + phi^-2 = 3`
