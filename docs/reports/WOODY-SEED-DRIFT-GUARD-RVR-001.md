# 🌊 WAVE LOOP REPORT — SQL↔Rust Seed-Key Drift Guard

**Document ID:** `WOODY-SEED-DRIFT-GUARD-RVR-001`
**Wave:** #17 — реализован «SQL↔Rust seed-logic drift guard» (Вариант B из Wave #16, item W-31)
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `0023791` — `test(garden): cross-language drift guard — seed keys vs SQL backfill 037`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — cross-language guard добавлен, тест зелёный, проверен negative-тестом.**

Закрыл давно отмеченный W-31 (Wave #12). Логика «какой item заказа становится семенем» живёт в **двух** местах на **двух языках**: live-путь `first_seedable_item` (Rust, `force_seed`) и batch-backfill (SQL, migration 037). Обе кодируют одну precedence — `strain > set > accessory > tea` — но **ничто их не связывает**: компилятор/типы не видят SQL, а серия 026→036→037 — ровно история этого рассинхрона.

Добавлен **cross-language consistency-тест**: извлекает упорядоченные `*_id` ключи из тела `first_seedable_item` (Rust, quote-delimited) и из `->>'..'` миграции 037 (SQL), требует равенства `[strain_id, set_id, accessory_id, tea_id]`. Скрытая логическая связь стала машинно-проверяемой.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-31 | Seed-precedence продублирована SQL(037)↔Rust(first_seedable_item) без связи | 🟠 MED | ✅ Cross-language drift-тест |
| W-43 | Миграции 026/036 тоже кодируют seed-логику (не покрыты guard'ом) | 🟢 INFO | 📋 (исторические, выполняются один раз; 037 — universal) |
| W-42 | Icon-only кнопки без aria-label; alt-качество | 🟡 LOW | 📋 Wave+1 |
| W-39 | Padding-sized тач-таргеты | 🟡 LOW | 📋 Wave+1 |

---

## 3. НАУЧНАЯ БАЗА

**Тема: logical (change) coupling и shotgun surgery через границу языка.**

- **Logical / change coupling** (Gall, Hajek, Jazayeri, ICSM 1998): артефакты, которые **со-изменяются** в истории, образуют связь, «which cannot be found by scanning code or documentation» — скрытую зависимость. SQL-backfill и Rust-функция логически связаны (должны меняться вместе), но **структурно невидимы** (разные языки/файлы, нет compiler-link). Change coupling **коррелирует с дефектами**.
- **Shotgun surgery / duplicated logic across layers** (Fowler): одно смысловое изменение требует правок в разных местах; «the more locations a single concept occupies, the more places for it to fall out of sync». Даже без copy-paste «implementations are guaranteed to be very similar and just as prone to bug-fixing drift».

Идеальная абстракция (один источник истины) тут невозможна — SQL batch и Rust live физически разные runtime'ы. Поэтому вместо устранения дублирования делаю **скрытую логическую связь явной и машинно-проверяемой**: drift-тест превращает невидимое co-change в падающий тест. Тот же «guard the intentional contract» класс (#1, #2, #8, #15, #16), но впервые **через границу языка**.

Источники:
- [Gall et al., Detecting logical coupling from release history (ICSM 1998) — обзор «Understanding Evolutionary Coupling»](https://par.nsf.gov/servlets/purl/10199009)
- [Strong change coupling correlates with defects (Apache Aries, Springer)](https://link.springer.com/chapter/10.1007/978-3-319-17837-0_1)
- [Shotgun Surgery (Wikipedia)](https://en.wikipedia.org/wiki/Shotgun_surgery)
- [Shotgun Surgery — duplicated logic across layers (NDepend)](https://blog.ndepend.com/shotgun-surgery/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Прочитал миграции 036/037; подтвердил, что 037 COALESCE precedence = `strain>set>accessory>tea` = Rust `first_seedable_item` KEYS.
2. ✅ Написал `ordered_id_keys(hay, quote)` — извлекает упорядоченные dedup `*_id` ключи (quote `"` для Rust, `'` для SQL).
3. ✅ Тест `seed_keys_match_sql_backfill_037`: scope к телу fn (исключая `strain_id` в force_seed INSERT) + 037; assert оба == `[strain_id,set_id,accessory_id,tea_id]`.
4. ✅ Negative-тест: reorder KEYS → FAIL «key set/order changed»; restore → PASS.
5. ✅ WASM 0 warnings; fmt; commit.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/api/garden.rs`** (`mod tests`): `ordered_id_keys` + `seed_keys_match_sql_backfill_037`. Читает `src/api/garden.rs` (scope `fn first_seedable_item`..`async fn force_seed`) и `migrations/037_garden_universal_backfill.sql` через `CARGO_MANIFEST_DIR`; сравнивает упорядоченные `*_id` ключи. Host-тест (defensive-tests hook).

Проверка: PASS; reorder → FAIL (доказано); `cargo check --target wasm32` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🔤 Alt-quality + icon-button aria-label (W-42)
Эвристики «плохого alt» (filename-as-alt `.jpg/.png`, `alt="image"`) + кнопки-иконки (emoji/символ + `cursor:pointer`, без текста) должны иметь `aria-label`/`title` (WCAG 4.1.2 Name/Role/Value). Продолжает a11y-линию (#15/#16); затрагивает контролы, добавленные в #11-#15.

### Вариант B — 📏 Padding-sized touch targets (W-39)
Расширить touch-target тест на кнопки без явного width/height: оценка эффективной высоты из `padding` + `font-size`; flag < 44px.

### Вариант C — 🧬 Generalize cross-language drift detection
Поискать другие SQL↔Rust (или client↔server) пары с продублированной логикой (напр. статус-наборы `('completed','confirmed','ready')` в SQL vs Rust-фильтрах) и закрыть их drift-тестами того же класса.

---

## 7. SKILL SAVED

Память: обновлён `garden-seed-pure-core.md` (W-31 закрыт; зафиксирован cross-language guard и принцип «делай скрытую логическую связь машинно-проверяемой, когда единый источник истины невозможен»).

**Anchor:** `phi^2 + phi^-2 = 3`
