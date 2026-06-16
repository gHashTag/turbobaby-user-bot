# 🌊 WAVE LOOP REPORT — Image alt-text + WCAG 1.1.1 Guard

**Document ID:** `WOODY-IMG-ALT-RVR-001`
**Wave:** #16 — реализован «alt-text a11y guard» (Вариант A из Wave #15)
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `7abff25` — `fix(a11y): add alt text to all images + img-alt fitness test`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — все 31 `<img>` имеют alt, добавлен regression-guard, WASM без warning'ов.**

Закрыл WCAG 1.1.1 (Non-text Content, Level A — **первый** критерий WCAG). Из 31 `<img>` под `src/ui` **6 не имели `alt`** → скринридеры озвучивали бы имя файла (`DSC_0234.jpg`) или ничего. Добавлен осмысленный alt: растение сада («Растение»), loyalty-tier бутоны (`tier.label()`), картинка набора (`set_name`), admin-thumbnail (`name`), admin-превью («Превью»).

Добавлен **source-scanning fitness-тест** `every_img_has_alt` (main.rs, host): любой `img {` без `alt:` фейлит сборку. Проверен negative-тестом. Честно: тест ловит **наличие**, не **качество** alt.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-40 | 6/31 `<img>` без `alt` (WCAG 1.1.1, Level A) | 🟠 MED | ✅ alt добавлен всем |
| W-41 | Нет защиты от регресса alt | 🟠 MED | ✅ fitness-тест `every_img_has_alt` |
| W-42 | Тест не судит качество alt (filename-as-alt, decorative misclass) | 🟡 LOW | 📋 Wave+1 (эвристики + ручной аудит) |
| W-39 | Padding-sized кнопки <44px тач-таргет тест не ловит | 🟡 LOW | 📋 Wave+1 |
| W-31 | SQL↔Rust seed-logic drift | 🟡 LOW | 📋 Wave+1 |

---

## 3. НАУЧНАЯ БАЗА

**Тема: WCAG 1.1.1, alt-эквивалентность и граница автоматизации.**

- **WCAG 1.1.1 (Level A)** — «all non-text content … must have a text alternative that serves the **equivalent purpose**». Это первый и фундаментальный критерий (P в POUR).
- **Decorative ≠ missing**: декоративным нужен `alt=""` (присутствует, но пуст) — скринридер пропустит; **без** alt он читает имя файла («major distraction»). Информативным — осмысленное описание; функциональным (кнопка-иконка) — описание функции.
- **Автоматизация ловит наличие, не качество**: «Automated testing catches presence, not quality. Manual review ensures alt text actually communicates.» `alt="image1.jpg"` проходит «has alt», но бесполезен. → нужен 2-слойный подход (авто presence + ручной quality).

Мой guard — авто-слой (presence) на каждый коммит; качество/decorative-классификацию честно оставляю ручному аудиту. Тот же «guard the intentional contract» класс, что touch-target (#15), bilingual-wiring (#8), migration-wiring (#1).

Источники:
- [WCAG 1.1.1 Alt Text Guide (AltText.ai)](https://alttext.ai/blog/wcag-alt-text-guide)
- [Decorative Images — W3C WAI Tutorial](https://www.w3.org/WAI/tutorials/images/decorative/)
- [Myth: Alternate text and automation (A11Y Project)](https://www.a11yproject.com/posts/alternate-text-and-automation/)
- [Good Alt Text, Bad Alt Text (WCAG.com)](https://www.wcag.com/blog/good-alt-text-bad-alt-text-making-your-content-perceivable/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Скан: 31 `img {` под src/ui; 6 без `alt` (garden, profile×2, admin×2, sets).
2. ✅ Добавил осмысленный alt каждому (имя/tier.label()/set_name/name/«Превью»/«Растение»).
3. ✅ Fitness-тест `img_alt_tests::every_img_has_alt` (8-строчное окно после `img {`, требует `alt:`).
4. ✅ Negative-тест (убрал alt → FAIL); restore → PASS; WASM 0 warnings; fmt.
5. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **Fix**: `garden.rs` (`alt: "Растение"`), `profile_screen.rs` ×2 (`alt: "{tier.label()}"`), `admin_screen.rs` (`alt: "Превью"` + `alt: "{name}"`), `sets_screen.rs` (`alt: "{set_name}"`).
- **Guard** (`src/main.rs::img_alt_tests`): walk `src/ui/**/*.rs`; для каждого `img {` берёт 8-строчное окно, требует `alt:`; иначе violation с `file:line`. Runs host (defensive-tests hook).

Проверка: `every_img_has_alt` PASS (31 проверено); negative-тест ловит missing alt; `cargo check --target wasm32` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🔤 Alt-quality heuristics + icon-button labels (W-42)
Добавить эвристики «плохого alt»: flag `alt` оканчивающийся на `.jpg/.png/.webp` (filename-as-alt) или равный `"image"/"img"`. Плюс: кнопки-иконки (только emoji/символ, `cursor:pointer`) должны иметь `aria-label`/`title`. Углубляет non-text-content покрытие (presence→базовое качество).

### Вариант B — 🧬 SQL↔Rust seed-logic drift guard (W-31)
Defense-тест согласованности precedence в миграциях 036/037 и `first_seedable_item`.

### Вариант C — 📏 Padding-sized touch targets (W-39)
Расширить touch-target тест на кнопки без явного width/height: оценка высоты из `padding` + `font-size`; flag < 44px эффективной высоты.

---

## 7. SKILL SAVED

Память: новый `img-alt-guard.md` + строка в `MEMORY.md`. Принцип: каждый `<img>` имеет alt (descriptive или `""` для decorative); guard ловит presence, качество — ручной аудит (2-слойная модель WCAG-тестирования).

**Anchor:** `phi^2 + phi^-2 = 3`
