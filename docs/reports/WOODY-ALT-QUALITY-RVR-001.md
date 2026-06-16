# 🌊 WAVE LOOP REPORT — Image alt-text Quality Heuristic

**Document ID:** `WOODY-ALT-QUALITY-RVR-001`
**Wave:** #22 — реализована «alt-quality эвристика» (часть Варианта B из Wave #21, item W-42)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `82d05e5` — `test(a11y): img alt-quality heuristic (reject filename/placeholder alt)`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — quality-эвристика добавлена, 2 img-alt теста зелёные (проверено negative-тестом), backend+WASM компилируются.**

Wave #16 охранял **наличие** `alt`, но честно отмечал, что **качество** не судит — классический провал `alt="image1.jpg"` проходит «has alt», но для скринридера бесполезен (WCAG 1.1.1 требует *эквивалент*, не любой текст). Этот Wave добавляет базовый quality-слой: `img_alt_is_meaningful` сканирует литеральные `alt: "…"` под `src/ui` и фейлит на (1) generic-плейсхолдерах (`image`/`img`/`photo`/`picture`/`thumbnail`/…) и (2) filename-образных alt (оканчивается на `.jpg/.png/.webp/…`). Интерполированные `alt: "{name}"` и декоративные `alt: ""` пропускаются. Текущие alt чисты → guard охраняет будущее.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-42 | img-alt guard ловил presence, не quality (filename-as-alt проходил) | 🟡 LOW | ✅ `img_alt_is_meaningful` — эвристика на generic/filename alt |
| W-47 | Admin icon-кнопки (✏️ ×3, 🗑 ×3, ✕ ×2, 👁️/🚫) без aria-label | 🟡 LOW | 📋 Wave+1 (8 кнопок, staff-tool) |
| W-50 | `CRITICAL_COLUMNS` — guard, не derive | 🟢 INFO | 📋 Wave+1 (build.rs) |
| 🚨 | #18-#20 фиксы — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: спектр a11y-автоматизации — presence → эвристики качества → ручной аудит.**

Из исследования Wave #16 (WCAG 1.1.1, alt presence-vs-quality): автотулзы «**verify presence and check for common issues such as file paths used as alt text**, and warn if non-figure elements have alt text — **but these remain heuristics, not true quality assessments**». Это ровно три уровня:
1. **Presence** (Wave #16, `every_img_has_alt`) — есть ли alt.
2. **Heuristic quality** (этот Wave) — не filename, не generic-плейсхолдер. Ловит «obvious bad», как делают axe/WAVE/scribely.
3. **Semantic quality** — эквивалентен ли alt смыслу; decorative-vs-informative — **только человек** (честно вне охвата).

`alt="image1.jpg"` passes (1) но fails (2) — моя эвристика закрывает этот зазор. Тот же «guard the intentional contract» класс, что и остальные defense-тесты.

⚠️ *Поиск свежих источников в этом Wave недоступен (API 529 Overloaded)* — опираюсь на установленную базу Wave #16.

Источники (Wave #16, on-point):
- [Myth: Alternate text and automation (A11Y Project)](https://www.a11yproject.com/posts/alternate-text-and-automation/)
- [Good Alt Text, Bad Alt Text (WCAG.com)](https://www.wcag.com/blog/good-alt-text-bad-alt-text-making-your-content-perceivable/)
- [WCAG 1.1.1 Alt Text Guide (AltText.ai)](https://alttext.ai/blog/wcag-alt-text-guide)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Аудит текущих `alt:` значений — все осмысленные (нет filename/placeholder) → эвристика пройдёт, охранит будущее.
2. ✅ `img_alt_is_meaningful` в `img_alt_tests`: GENERIC + IMG_EXT списки; парсит литеральные `alt: "…"` (skip `{…}` и `""`).
3. ✅ Обновлён doc-комментарий модуля (presence + basic quality).
4. ✅ Negative-тест: `alt="preview.png"` → flagged «looks like a filename»; restore → PASS.
5. ✅ backend + wasm компилируются; fmt.
6. ✅ Отчёт + skill (поиск недоступен — Wave #16 база).

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/main.rs` `img_alt_tests`**: новый `img_alt_is_meaningful` — walk `src/ui/**/*.rs`, для каждой строки извлекает литеральное значение `alt: "…"` (по первой паре кавычек после `alt:`), пропускает интерполяцию (`{`) и пустое (decorative); флагует, если `lower ∈ GENERIC` или оканчивается на расширение картинки. Doc-комментарий модуля обновлён.

Проверка: 2 теста PASS; negative (`preview.png`) → flagged; backend + `cargo check --target wasm32` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — ♿ Admin icon-button aria-label (W-47)
Долейбелить 8 admin icon-кнопок (✏️ редактировать ×3, 🗑 удалить ×3, ✕ закрыть ×2, 👁️/🚫 toggle). Завершает icon-button a11y, начатый в Wave #18 (customer-facing уже сделаны). Механическое, можно + расширить touch-target/aria guard на admin.

### Вариант B — 🏗️ Derive CRITICAL_COLUMNS (build.rs, W-50)
Истинный DRY — генерировать список из SELECT'ов на сборке, убрать ручную поддержку. Выше риск build-скрипта.

### Вариант C — 🧪 Cause-based мониторинг определённых причин
«definite+imminent cause → ticket gauge» для отсутствия критичного ENV / недостижимости БД/AI на старте. Только determinate, без alert-fatigue.

---

## 7. SKILL SAVED

Память: обновлён `img-alt-guard.md` (добавлен quality-слой). Принцип: a11y-автоматизация — спектр (presence → heuristic-quality → semantic/human); эвристика ловит «obvious bad» (filename/placeholder), но не заменяет ручной аудит.

**Anchor:** `phi^2 + phi^-2 = 3`
