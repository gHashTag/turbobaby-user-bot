# 🌊 WAVE LOOP REPORT — Icon-Button Accessible-Name Guard

**Document ID:** `WOODY-ICON-BUTTON-GUARD-RVR-001`
**Wave:** #24 — реализован «fitness-guard на icon-buttons» (Вариант A из Wave #23, item W-51)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `753627a` — `test(a11y): fitness guard — icon-only buttons must have an accessible name`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — regression-guard добавлен, поймал 2 реальных пропуска, тест зелёный, WASM+fmt чисты.**

Waves #18/#23 руками добавили `aria-label` ко всем icon-only кнопкам. Этот Wave превращает ручную дисциплину в **автоматический guard** (W-51): `icon_only_buttons_have_accessible_name` извлекает каждый `button { … }` под `src/ui` (string-/closure-aware brace matching) и фейлит, если блок с child-глифом из allowlist (▶️ ✕ ✖ 🗑 ✏️ 🌟 👁️ 🚫) **не** имеет `aria-label`/`aria-labelledby`/`title` (WCAG 4.1.2).

Guard **сразу поймал 2 неразмеченные кнопки удаления** мест/охот (🗑) — теперь «Удалить». Это Rust/rsx-аналог индустриального ESLint-правила `control-has-associated-label`.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-51 | Нет защиты от регресса: новый icon-button легко без label | 🟠 MED | ✅ `icon_only_buttons_have_accessible_name` (glyph-allowlist) |
| W-52 | 2 admin 🗑-кнопки (места/охоты) были без aria-label | 🟡 LOW | ✅ Пойманы guard'ом → «Удалить» |
| W-53 | Static-lint не видит rendered a11y-tree (нужен axe/manual) | 🟢 INFO | 📋 (честная граница) |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: static-analysis enforcement accessible names (shift-left a11y) + его границы.**

- **Индустриальный аналог**: `eslint-plugin-jsx-a11y/control-has-associated-label` — статически требует, чтобы у control (вкл. icon-button) был label через text content / `aria-label` / `aria-labelledby`. Запускается в CI как required-check, блокирует PR. У нас JSX-ESLint нет (Rust/rsx) → написал эквивалент на Rust как defense-тест в lefthook.
- **Design-system enforcement**: у правила есть `controlComponents`/`labelAttributes` — регистрируешь свой `IconButton` и обязательный label-prop. Это следующий уровень (→ W-51 продолжение: общий компонент).
- **Граница (честно)**: «static linting catches missing labels in source code, but it can't verify the rendered accessibility tree… always test with assistive technology.» Мой guard — defense-in-depth слой (source), не замена axe/ручного screen-reader теста.

Тот же «guard the intentional contract» класс, что img-alt (#16), touch-target (#15), bilingual-wiring (#8).

Источники:
- [eslint-plugin-jsx-a11y — control-has-associated-label (rule docs)](https://github.com/jsx-eslint/eslint-plugin-jsx-a11y/blob/main/docs/rules/control-has-associated-label.md)
- [eslint-plugin-jsx-a11y (npm — static eval, complement with axe + AT)](https://www.npmjs.com/package/eslint-plugin-jsx-a11y)
- [eslint-plugin-fluentui-jsx-a11y — image buttons need accessible labelling](https://github.com/microsoft/eslint-plugin-fluentui-jsx-a11y)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Проверил: icon-глифы встречаются только как button-children (не в span/div) → exact-quoted-glyph match надёжен.
2. ✅ `button_blocks(src)` — string-/closure-aware brace matching (пропускает `{` в `"…"`, считает скобки closures).
3. ✅ `icon_only_buttons_have_accessible_name`: блок с glyph-child без `aria-label`/`labelledby`/`title` → offender; `icon_buttons_seen > 0` sanity.
4. ✅ Guard поймал 2× 🗑 (места/охоты) → залейблил «Удалить».
5. ✅ Re-run → PASS; backend+WASM компилируются; fmt.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/main.rs` `icon_button_aria_tests`**: `ICON_GLYPHS` allowlist; `button_blocks` (brace/string-aware extractor); тест флагует glyph-button без accessible name.
- **`src/ui/screens/admin_screen.rs`**: `"aria-label": "Удалить"` на 2 delete-кнопки мест/охот (пойманы guard'ом).

Проверка: тест PASS (`icon_buttons_seen > 0`); negative (убрать label) → FAIL; `cargo check --target wasm32` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🧩 Shared `IconButton` component (design-system enforcement)
Общий `IconButton { icon, aria_label, on_click }` с **обязательным** `aria_label`-пропом — нельзя забыть by construction (как `controlComponents` в ESLint-правиле). Переводит guard из «ловит после» в «невозможно нарушить». Рефактор существующих icon-кнопок на него.

### Вариант B — 🏗️ Derive CRITICAL_COLUMNS (build.rs, W-50)
Истинный DRY — генерировать из SELECT'ов на сборке.

### Вариант C — 🧪 Cause-based мониторинг определённых причин
«definite+imminent → ticket gauge» для отсутствия критичного ENV / недостижимости БД/AI на старте.

---

## 7. SKILL SAVED

Память: обновлён `img-alt-guard.md` (W-51 закрыт guard'ом; зафиксированы `button_blocks` + glyph-allowlist). Принцип: ручную a11y-дисциплину → автоматический source-guard (аналог `control-has-associated-label`), но это defense-in-depth, не замена axe/ручного теста.

**Anchor:** `phi^2 + phi^-2 = 3`
