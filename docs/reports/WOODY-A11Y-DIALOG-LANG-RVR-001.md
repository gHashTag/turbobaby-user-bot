# 🌊 WAVE LOOP REPORT — WCAG: Dialog Semantics, Page Lang, Input Labels

**Document ID:** `WOODY-A11Y-DIALOG-LANG-RVR-001`
**Wave:** #58 — accessibility fresh-area scan (Вариант B из Wave #57)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `f129504` — `fix(a11y): dialog roles, document lang, admin login input labels (WCAG)`
**Agent:** Claude Opus 4.8 (Wave loop) + Explore-subagent

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — 3 WCAG-пробела закрыты + html-lang guard; WASM компилируется; 69 defensive зелёные.**

Accessibility-скан UI (за пределами существующих guard'ов alt/touch-target/icon-aria) нашёл 3 реальных пробела:
1. **WCAG 3.1.1** — `index.html <html>` без `lang` → screen-reader не знает язык произношения. Поставил `lang="ru"` (ru-first контент) + guard-тест `html_lang_tests`.
2. **WCAG 4.1.2** — все 3 modal-компонента (`Modal`/`DeleteConfirmModal`, `ProductDetailModal`, `VideoModal`) без dialog-семантики → SR не объявляет «диалог». Добавил `role="dialog"` + `aria-modal="true"` + `aria-label` (title / имя товара / «Видео») — accessible name обязателен (иначе SR слышит лишь «dialog»).
3. **WCAG 1.3.1/4.1.2** — admin-login inputs только с placeholder → SR не называет поле (placeholder исчезает при вводе, не заменяет label). Добавил `aria-label` (Telegram ID / Пароль админа).

Честно: focus-trap/Escape/focus-return для модалок (2.1.2/2.4.3) **не сделаны** — крупнее (keyboard-wiring в Dioxus) → follow-up **W-93**.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | WCAG | Действие |
|---|---|---|---|
| W-92a | `<html>` без `lang` | 3.1.1 (A) | ✅ `lang="ru"` + `html_lang_tests` guard |
| W-92b | 3 modal'а без `role=dialog`/`aria-modal`/name | 4.1.2 (A) | ✅ role+aria-modal+aria-label на всех |
| W-92c | admin-login inputs placeholder-only | 1.3.1/4.1.2 (A) | ✅ `aria-label` ×2 |
| W-93 | модалки без focus-trap/Escape/focus-return | 2.1.2/2.4.3 | 📋 follow-up (keyboard-wiring) |
| — | color-only status, checkout inputs — clean (badges с текстом, видимые label) | ✅ | — |
| 🚨 | PR ветки (104+ ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: WCAG dialog accessible name; lang of page (3.1.1); placeholder ≠ label.**

- **Dialog нужен accessible name**: «every dialog must have a name; announcing 'dialog' alone is not enough … without a name users only hear 'dialog' with no context». → `role=dialog` + `aria-modal=true` + `aria-label`/`aria-labelledby`. (Focus-trap/inertness — отдельно, W-93.)
- **3.1.1 lang**: «use the HTML `lang` on `<html>`, BCP47 two-letter (`ru`) … without it screen readers guess and mispronounce». Pitfall — underscore (`ru_RU`); я использовал дефис-free `ru` (валидно).
- **Placeholder ≠ label**: «placeholder disappears on typing, not a replacement for labels; `aria-label` satisfies 1.3.1/4.1.2». Ровно мой фикс admin-inputs.

Источники:
- [Using ARIA role=dialog to implement a modal (W3C WAI)](https://www.w3.org/WAI/GL/wiki/Using_ARIA_role%3Ddialog_to_implement_a_modal_dialog_box)
- [ARIA dialog must have accessible name (GetWCAG 4.1.2)](https://getwcag.com/en/accessibility-guide/aria-dialog-name)
- [WCAG 3.1.1 Language of Page (BOIA)](https://www.boia.org/blog/wcag-success-criteria-3.1.1-and-3.1.2-language-of-page-and-parts)
- [Accessible Forms: Problem with Placeholders (Deque)](https://www.deque.com/blog/accessible-forms-the-problem-with-placeholders/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ a11y-скан → 3 пробела (lang / dialog-semantics / admin-input labels); color-only + checkout inputs clean.
2. ✅ `index.html` → `<html lang="ru">` + `html_lang_tests` guard.
3. ✅ `modal.rs`/`product_detail_modal.rs`/`video_modal.rs` → role=dialog + aria-modal + aria-label (title/name/«Видео»).
4. ✅ admin-login 2 inputs → aria-label.
5. ✅ html-lang тест PASS; WASM compile (все rsx-aria); 69 defensive PASS; fmt.
6. ✅ Research (WCAG dialog/lang/labels) + отчёт + skill. Зафиксировал W-93 (focus-trap).

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`index.html`**: `<html>` → `<html lang="ru">`.
- **`src/ui/components/modal.rs`**: inner dialog div + `role:"dialog"`, `"aria-modal":"true"`, `"aria-label":"{title|Диалог}"`.
- **`src/ui/components/product_detail_modal.rs`**: + `role/aria-modal`, `"aria-label":"{name}"`.
- **`src/ui/components/video_modal.rs`**: + `role/aria-modal`, `"aria-label":"Видео"`.
- **`src/ui/screens/admin_screen.rs`**: 2 login inputs + `"aria-label"`.
- **`src/main.rs`**: `html_lang_tests::index_html_declares_a_document_language`.

Проверка: `cargo test … index_html_declares` → PASS; WASM 0 warnings; backend; defensive 69 PASS; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
104+ коммита ahead. Ops/review — твоё «go».

### Вариант B — ♿ Modal focus-trap/Escape/focus-return (W-93)
WCAG 2.1.2/2.4.3: onkeydown Escape→close, фокус в модалку при открытии, возврат при закрытии. Keyboard-wiring в Dioxus (умеренный объём).

### Вариант C — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

---

## 7. SKILL SAVED

Память: новый `a11y-dialog-lang-labels.md` + строка в `MEMORY.md`. Принцип: modal = `role=dialog`+`aria-modal=true`+**accessible name** (иначе SR слышит лишь «dialog»); `<html lang>` обязателен (BCP47); placeholder НЕ заменяет label → `aria-label`. Focus-trap/Escape — отдельный слой (W-93).

**Anchor:** `phi^2 + phi^-2 = 3`
