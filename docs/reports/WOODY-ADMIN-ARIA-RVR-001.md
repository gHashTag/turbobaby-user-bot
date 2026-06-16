# 🌊 WAVE LOOP REPORT — Admin icon-button accessible names

**Document ID:** `WOODY-ADMIN-ARIA-RVR-001`
**Wave:** #23 — реализован «Admin icon-button aria-label» (Вариант A из Wave #22, item W-47)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `8a3ac6f` — `fix(a11y): aria-label on admin icon-only buttons (WCAG 4.1.2)`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — admin icon-кнопки получили accessible names, WASM без warning'ов, fmt чист.**

Завершено покрытие icon-only кнопок accessible-именами (Wave #18 закрыл customer-facing). Admin-контролы озвучивались скринридером просто как «button» (классический провал WCAG 4.1.2 Name/Role/Value). Добавлены `aria-label`:
- **`ItemRow`** (общий компонент для **каждой** admin-строки каталога): ▶️ видео, ✏️ редактировать, 👁️/🚫 видимость, 🌟 сорт-дня, 🗑 удалить — один правок-кластер покрывает все строки.
- Модальные закрытия (✕/✖) + инлайн-сброс ошибки (✕).
- ✏️ редактирования мест/охот.

Кнопки с текстом («❌ Отмена») уже имеют имя — не трогал.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-47 | Admin icon-only кнопки без accessible name (скринридер: «button») | 🟡 LOW | ✅ aria-label на ItemRow (×5) + закрытия + edit мест/охот |
| W-51 | Нет структурной защиты: новый icon-button легко забыть лейбл | 🟡 LOW | 📋 Wave+1 (IconButton-компонент или fitness-guard на glyph-set) |
| W-50 | `CRITICAL_COLUMNS` guard, не derive | 🟢 INFO | 📋 Wave+1 (build.rs) |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: WCAG 4.1.2 (Name, Role, Value) — accessible name для icon-only кнопок.**

- 4.1.2 (Level A): у интерактивного элемента должны быть программные **name/role/value**. Без имени скринридер озвучивает только тип: «button» → пользователь гадает.
- **Visible label ≠ accessible name**: «4.1.2 cares about the accessible name — which might be invisible (like aria-label on an icon button).» Для icon-only — имя из `aria-label`/`aria-labelledby`, не из плейсхолдера.
- **Эмодзи + aria-label**: `aria-label` **переопределяет** текстовое содержимое (эмодзи) для accessible name → скринридер озвучивает «Удалить, кнопка», а не «корзина emoji». Native `<button>` уже даёт правильный role.
- **Design-system**: «require a label or aria-label prop for buttons» — структурная профилактика (→ W-51).
- **Граница автоматизации**: тулзы ловят *отсутствие* имени, но не его осмысленность — нужен ручной screen-reader тест (как и с alt, Wave #16/#22).

Источники:
- [WCAG 4.1.2 Name, Role, Value (Silktide)](https://silktide.com/accessibility-guide/the-wcag-standard/4-1/compatible/wcag-4-1-2-name-role-value/)
- [WCAG 4.1.2 — Name, Role, Value 2025 Guide (TestParty)](https://testparty.ai/blog/wcag-4-1-2-name-role-value-2025-guide)
- [Accessible names for buttons (GetWCAG)](https://getwcag.com/en/accessibility-guide/input-button-name)
- [WCAG Fix It: 4.1.2 (AccessibilityChecker)](https://accessibilitychecker.com/news/wcag-fix-it-4-1-2-name-role-value/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Скан admin icon-only кнопок без aria-label (отсев текст-несущих типа «❌ Отмена»).
2. ✅ `ItemRow` (shared): 5 кнопок (▶️/✏️/toggle/🌟/🗑) — высокий рычаг (каждая admin-строка).
3. ✅ Модальные ✕/✖ + error-dismiss ✕ + edit мест/охот (✏️).
4. ✅ Re-scan → не осталось unlabeled icon-кнопок; `cargo check --target wasm32` → 0 warnings; fmt.
5. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/ui/screens/admin_screen.rs`**: `"aria-label": "…"` на icon-only `button {}`: ItemRow (Смотреть видео / Редактировать / Переключить видимость / Сорт дня / Удалить), order/edit modal close (Закрыть ×2), error-dismiss (Закрыть ×2), place/hunt edit (Редактировать ×2). Эмодзи остаётся как текст; `aria-label` переопределяет accessible name.

Проверка: re-scan чист; backend (через UI в wasm) компилируется; `cargo check --target wasm32` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🧩 IconButton-компонент или fitness-guard (W-51, структурная профилактика)
Либо общий `IconButton { icon, aria_label, on_click }` (требует `aria_label` проп — design-system enforcement, нельзя забыть), либо source-scanning fitness-тест: кнопка, чей label ∈ {известный glyph-set ▶️✕−+🗑✏️🌟🛒👁️🚫}, обязана иметь `aria-label`. Превращает ручную дисциплину в автоматический guard (в Wave #18 счёл fragile — но glyph-set делает его надёжным).

### Вариант B — 🏗️ Derive CRITICAL_COLUMNS (build.rs, W-50)
Истинный DRY — генерировать список из SELECT'ов на сборке.

### Вариант C — 🧪 Cause-based мониторинг определённых причин
«definite+imminent → ticket gauge» для отсутствия критичного ENV / недостижимости БД/AI на старте.

---

## 7. SKILL SAVED

Память: обновлён `img-alt-guard.md` (W-47 закрыт — admin icon-кнопки залейблены). Принцип: icon-only кнопке нужен accessible name через `aria-label` (переопределяет эмодзи-текст); общий компонент/ItemRow — высокий рычаг; структурная защита (required prop / glyph-guard) — следующий шаг.

**Anchor:** `phi^2 + phi^-2 = 3`
