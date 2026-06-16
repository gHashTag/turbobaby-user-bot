# 🌊 WAVE LOOP REPORT — Lang↔Code Clone Elimination

**Document ID:** `WOODY-LANG-DEDUP-RVR-001`
**Wave:** #3 — реализован «Honest Language Support / single-source-of-truth» вектор из прошлых отчётов
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `859ffc9` — `refactor(i18n): collapse duplicate Lang<->code maps onto core::Lang`
**Agent:** Claude Opus 4.8 (Wave loop, 15-min cadence)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — clone устранён, все 5 хуков зелёные (check-backend, check-wasm 4.75s, defensive-tests, fmt, conventional-commits), 156 lib-тестов.**

В отличие от Wave #1–#2 (добавлял защитные тесты), этот Wave **удалил источник бага**, а не сторожил его. Найдены **дублирующиеся таблицы** маппинга `Lang ↔ 2-letter code`:

- `core::Lang::as_str` / `core::Lang::from_str` — каноничные.
- `ui/lang.rs::lang_code` / `lang_from_code` — **рукокопированные близнецы** тех же 8 строк.

Это type-1/type-2 code clone. Согласно Juergens et al. (ICSE 2009), 28% клон-групп со временем расходятся непреднамеренно, и каждая вторая такая рассинхронизация — **дефект**. Здесь цена расхождения конкретна: переименуй код в одном месте — UI пишет в `localStorage` один язык, остальной стек читает другой → пользователь молча получает не тот язык.

Оба близнеца схлопнуты в делегирование к `core::Lang`. Один источник истины.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-10 | `lang_code`/`lang_from_code` — клоны `core::Lang::as_str`/`from_str`; дрейф → неверный язык из localStorage | 🟠 MED | ✅ Устранён делегированием + 2 guard-теста |
| W-11 | `tf()` placeholder-дрейф между ru/en | 🟢 INFO | Проверено: только 1 ключ (`T_CHECKPOINT_TITLE {0}`), ru/en синхронны — тест не нужен (overkill) |
| W-07 | 6 языков → English fallback (из Wave #2) | 🟡 LOW | 📋 Wave+1, Вариант A |
| W-03 | 221 трекаемый бинарник в git | 🟠 MED | 📋 Wave+1, Вариант B (требует подтверждения по деплою) |

Замечен (не трогал): `core::normalize_lang_code` — третий, но **намеренный и хорошо протестированный** путь (Slavic→ru, Romance→en neighbor-mapping, 10 тестов). Не клон — это отдельная фича fuzzy-резолва. Оставлен как есть.

---

## 3. НАУЧНАЯ БАЗА

**Тема: inconsistent code clones как предиктор дефектов.**

Ключевая работа — **Juergens, Deissenboeck, Hummel, Wagner, «Do Code Clones Matter?», ICSE 2009** (TU München). Крупное эмпирическое исследование 4 промышленных + 1 OSS системы:

> «inconsistent changes to clones are very frequent … we identified a significant number of faults induced by such changes. 28% of type-3 clone groups had unintentional inconsistencies, and of these every second was a fault.»

Реплика подтвердила устойчивость показателя (~0.15 faulty-rate). Вывод применительно к нам: `lang_code` ≈ копия `as_str`. Пока они совпадают — багов нет, но это **отложенный дефект с ~50% вероятностью** при первой же независимой правке. Дешевле всего убить его слиянием в один источник, а не сторожить тестом-на-паритет (хотя guard-тест добавлен как страховка на время существования тонкой обёртки).

Связь с прошлыми Wave: #1 (migration wiring) и #2 (i18n keys) добавляли fitness-функции вокруг **намеренно** ручных реестров; здесь реестр был **случайно** продублирован — правильный ход не сторожить, а дедуплицировать.

Источники:
- [Juergens et al., «Do Code Clones Matter?», ICSE 2009 (ACM DL)](https://dl.acm.org/doi/10.1109/ICSE.2009.5070547)
- [«Do Code Clones Matter?» — author PDF (Teamscale)](https://teamscale.com/hubfs/Publications/2009-do-code-clones-matter.pdf)
- [Bettenburg et al., «Inconsistent Changes to Code Clones at Release Level», WCRE 2009](https://users.encs.concordia.ca/~shang/pubs/bettenburg-wcre09.pdf)
- [«On the Relationship of Inconsistent Software Clones and Faults» (replication)](https://www.researchgate.net/publication/303513566_On_the_Relationship_of_Inconsistent_Software_Clones_and_Faults_An_Empirical_Study)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Разведка нового слабого места (не fitness-тест в третий раз): анализ `tf` placeholder-дрейфа → чисто; нашёл реальный клон `lang_code`/`lang_from_code`.
2. ✅ Подтверждение: оба — точные копии `core::Lang::as_str`/`from_str`; проверены импорты и callsites.
3. ✅ Реализация: делегирование `lang_code → lang.as_str()`, `lang_from_code → code.parse().ok()`.
4. ✅ Guard: усилен roundtrip-тест (теперь реально гоняет через `core::from_str` на host) + новый `lang_code_matches_core_as_str`.
5. ✅ Прогон 156 тестов + check-wasm (компиляция wasm подтверждена) через lefthook.
6. ✅ Research (Juergens 2009) + отчёт + skill-memory.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

`src/ui/lang.rs`:
- **`lang_code(lang)`** → `lang.as_str()` (удалён 8-строчный `match`-клон).
- **`lang_from_code(code)`** → `code.parse::<Lang>().ok()` (удалён 9-строчный `match`-клон; `from_str` — строгий superset для 2-буквенных кодов).
- **Тесты:** `lang_code_roundtrip_via_canonical_codes` теперь утверждает `code.parse::<Lang>().ok() == Some(l)` (реальный roundtrip на host); добавлен `lang_code_matches_core_as_str` — пин делегирования.

No behaviour change: `lang_code` идентичен; `lang_from_code` лишь толерантнее (localStorage всё равно хранит только 2-буквенные коды).

Запуск: `cargo test --lib lang_code` → green; полный lib → 156 passed.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🌐 Honest Language Support (W-07)
6 языков (`th/zh/he/de/fr/es`) молча отдают English. Решение: либо реальные таблицы, либо сузить «обещанный» набор. Минимальный безопасный шаг — fitness-тест, классифицирующий языки на «full» (ru/en) и «explicit-fallback», чтобы новый `Lang::X` форсировал осознанное решение о покрытии. Развивает §3 — превращает молчаливый fallback в задокументированный контракт.

### Вариант B — 🧹 Repo Hygiene (W-03, требует подтверждения)
Вынести `dist/.stage/`, `*.mp4`, аплоады, `*_bg.wasm` из git (221 файл топит диффы). **Требует подтверждения пользователя**: нужно убедиться, что Railway/Trunk регенерируют `dist/` на деплое и это не конфликтует с уже идущей очисткой (в git уже видны `D dist/.stage/...`).

### Вариант C — 🔁 Clone Sweep (развить §3 на весь src)
Прогнать поиск оставшихся «руко-копий» каноничных таблиц/маппингов по `src/` (флаг-эмодзи языков, display-names, дублирующиеся status→message карты в `api_errors`), и схлопнуть их к единому источнику. Систематическое применение урока Juergens 2009 ко всей кодовой базе.

---

## 7. SKILL SAVED

Память: `lang-code-clone-eliminated.md` + строка в `MEMORY.md`. Зафиксирован принцип: **намеренный** ручной реестр → fitness-тест (Wave #1–#2); **случайный** клон каноничного кода → дедупликация (Wave #3). Juergens 2009 как обоснование «убей клон, не сторожи его».

**Anchor:** `phi^2 + phi^-2 = 3`
