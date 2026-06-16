# 🌊 WAVE LOOP REPORT — Bilingual Catalog Descriptions

**Document ID:** `WOODY-BILINGUAL-DESC-RVR-001`
**Wave:** #5 — реализован «Bilingual descriptions» (Вариант A из Wave #4)
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `b0b21af` — `feat(catalog): show English descriptions to non-Russian users`
**Agent:** Claude Opus 4.8 (Wave loop, 15-min cadence)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — фича в проде-готовности, все хуки зелёные (check-wasm, defensive-tests, fmt, conventional-commits).**

Карточки и `ProductDetailModal` всегда показывали **русское** `description`, даже англоязычным пользователям — хотя API уже возвращает `description_en` (migration 016). Это прямой недочёт фичи из Wave (детальная модалка). Добавлен общий хелпер `lang::localized(ru, en)`, повторяющий правило fallback из `t()` (любой не-русский UI-язык предпочитает английский текст, если он есть, иначе — русский), и применён к описаниям в каталогах **Меню (сорта), Аксессуары, Чай** (карточки + модалка).

**Наборы исключены осознанно:** `/api/sets` не отдаёт `description_en`, и таблица `sets` не входила в migration 016 — двуязычность для них требует миграции (отложено, чтобы не словить schema-drift 500).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-14 | Описания только на русском для EN-пользователей (API отдаёт `description_en`, фронт игнорировал) | 🟠 MED | ✅ Закрыт `lang::localized` для menu/accessories/tea |
| W-16 | Наборы (`/api/sets`) не отдают `description_en`; таблица `sets` без bilingual-колонок | 🟡 LOW | 📋 Wave+1 (нужна миграция) |
| W-17 | `name_en`, `effect_en`, `flavor_profile_en` есть в БД/API, но не показываются (только описание двуязычно) | 🟠 MED | 📋 Wave+1 (расширить `localized` на эти поля) |
| W-15 | 4 локальных `Api*`-DTO дублируют `types.rs` | 🟡 LOW | 📋 Wave+1 |

---

## 3. НАУЧНАЯ / ИНДУСТРИАЛЬНАЯ БАЗА

**Тема: locale fallback chains и graceful degradation отсутствующих переводов.**

Индустриальный консенсус — деградировать от частного к общему: **specific → general → default**, чтобы пользователь всегда видел осмысленный текст:

> «Without fallback, applications would … show untranslated text … By implementing proper fallback chains … you ensure users always see meaningful content regardless of translation completeness.»

Ключевые принципы, применённые здесь:
- **Fallback применяется на уровне lookup отдельного ключа**, а не только при загрузке бандла — наш `localized()` решает per-field (`description`): отсутствующий/пустой `description_en` → русский primary.
- **Цепочка `lang → en → ru`**: проект уже коллапсирует все не-ru языки в English в `t()`; `localized()` зеркалит это (английский — «general» rung, русский — «universal default»). Консистентность data-driven копий со статичными UI-строками.
- **Не конфликтить язык с регионом** и не падать на «сыром» теге — у нас простая 2-уровневая модель (ru/en), без BCP47-региональных вариантов, поэтому достаточно бинарного выбора.

Связь с прошлыми Wave: #2 защитил **статические** ключи (`every_translation_key_…`), а этот Wave распространил ту же fallback-философию на **динамический** контент каталога.

Источники:
- [How to handle locale fallback when preferred locale is unavailable (lingo.dev)](https://lingo.dev/en/javascript-i18n/handle-locale-fallback)
- [Flutter Localization Fallback Strategies](https://flutterlocalisation.com/blog/flutter-localization-fallback-strategies)
- [Understanding BCP 47 Locale Codes](https://corner.buka.sh/understanding-bcp-47-locale-codes-the-modern-standard-for-language-and-region-tags/)
- [KDE D9793 — fall back to base language when locale name fails](https://phabricator.kde.org/D9793)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Подтверждено: API эмитит `description_en` для strains/accessories/tea; `/api/sets` — нет (и колонки в `sets` нет) → sets вне охвата.
2. ✅ Хелпер `lang::localized(ru, en)` (зеркалит fallback `t()`).
3. ✅ `description_en: Option<String>` (serde-default) добавлен в локальные `ApiStrain`/`ApiAccessory`/`ApiTea`.
4. ✅ Применён `localized()` на местах рендера описания: карточки + `ProductDetailModal` (включая screen-level модалку чая).
5. ✅ `cargo check --target wasm32` зелёный, `cargo fmt`, commit через lefthook.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/ui/lang.rs::localized(ru: &str, en: Option<&str>) -> String`** — `en` при `current_lang() != Russian && !en.trim().is_empty()`, иначе `ru`.
- **DTO**: `#[serde(default)] description_en: Option<String>` в `menu_screen::ApiStrain`, `accessories_screen::ApiAccessory`, `tea_screen::ApiTea` (бэк-совместимо: старые ответы без поля продолжают парситься).
- **Рендер**: `desc_str`/`desc` теперь `localized(...)`; передаётся в карточку и `ProductDetailModal`.

Проверка: `cargo check --target wasm32-unknown-unknown` → clean (UI — `#[cfg(target_arch="wasm32")]`, host-check не компилирует экраны).

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🈳 Full field localization (W-17, прямое продолжение)
Расширить `localized()` на `name_en`, `effect_en`, `flavor_profile_en`, `subcategory_en`, `category_en` (всё уже в API). Тогда весь каталог, а не только описание, переключается на английский. Чёткий объём, та же механика.

### Вариант B — 🧱 Unify Api* DTOs (W-15)
Свести 4 локальных `Api*` к единым DTO в `src/ui/api/types.rs`. Следующая итерация Rule of Three; заодно `description_en`/`*_en` объявляются один раз.

### Вариант C — 🗂️ Sets bilingual + schema (W-16)
Миграция: добавить `name_en`/`description_en` в `sets` (если нет), расширить `get_sets` SQL + `set_row`, и `localized()` на фронте. Закрывает последний каталог. Требует миграции → активирует schema-drift-defense из Wave #1 как страховку.

---

## 7. SKILL SAVED

Память: `bilingual-content-fallback.md` + строка в `MEMORY.md`. Закреплён хелпер `lang::localized` и принцип «fallback-цепочка `lang → en → ru` на уровне per-field lookup», плюс карта того, какие `_en` поля уже отдаёт API и где они ещё не подключены.

**Anchor:** `phi^2 + phi^-2 = 3`
