# 🌊 WAVE LOOP REPORT — Modal Keyboard a11y: Escape + Focus-into-Dialog

**Document ID:** `WOODY-MODAL-KEYBOARD-RVR-001`
**Wave:** #59 — modal focus/keyboard a11y (Вариант B из Wave #58, W-93)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `bd75296` — `fix(a11y): modal Escape-to-close + focus-into-dialog (WCAG 2.1.2/2.4.3)`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — Escape-to-close + focus-into-dialog у 2 customer-модалок; WASM компилируется; 69 defensive зелёные.**

Продолжил a11y-модалок (W-93 из #58). Для customer-facing `ProductDetailModal` + `VideoModal` (берут `EventHandler<()>`) добавил на dialog-контейнер: `tabindex="-1"` + `onmounted`→`set_focus(true)` (фокус **входит** в диалог при открытии — WCAG 2.4.3) и `onkeydown` Escape→`on_close` (клавиатурный выход — WCAG 2.1.2, паритет с ✕). Строится на role/aria-modal/aria-label из #58.

Честно: **focus-trap (Tab-цикл внутри) и focus-return-to-trigger не сделаны** — нужен store предыдущего фокуса + tabindex-менеджмент (остаток W-93). Но 2.1.2 keyboard-exit обеспечен (Escape + focusable ✕), а focus-into-dialog (2.4.3) — новый. Generic `Modal` (admin, `EventHandler<MouseEvent>`) пропущен — Escape потребует prod-type рефактор всех admin-callers (отдельно).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | WCAG | Действие |
|---|---|---|---|
| W-93a | модалки без Escape-to-close | 2.1.2 (A) | ✅ Escape→on_close (ProductDetail, Video) |
| W-93b | фокус не входит в диалог при открытии | 2.4.3 (A) | ✅ tabindex=-1 + onmounted set_focus |
| W-93c | нет focus-return-to-trigger + Tab-trap | 2.4.3 | 📋 остаток (store prev focus + tab mgmt) |
| W-93d | generic `Modal` (admin) — Escape требует prop-type рефактор | 2.1.2 | 📋 admin-only, отдельно |
| 🚨 | PR ветки (104+ ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: keyboard-accessible modal lifecycle (2.1.2 No Keyboard Trap / 2.4.3 Focus Order).**

- **2.1.2 — нужен клавиатурный выход**: «modals that can only be dismissed by a mouse click trap keyboard users … provide Escape handler + focusable close button». Мой Escape + ✕.
- **On open — focus в диалог**: «set initial focus to an element inside the modal». Мой `onmounted` set_focus.
- **Focus-trap уместен ПРИ наличии выхода**: «a focus trap inside a modal is best practice … the violation only occurs when there's no keyboard way out». → Tab-trap (остаток) не нарушает 2.1.2, т.к. выход есть.
- **Escape + return focus to trigger**: «Escape closes AND returns focus to the trigger so keyboard users don't lose context». Return-focus — остаток W-93c.
- **Click-outside ≠ keyboard exit**: «clicking outside requires a mouse». Поэтому Escape важен.

Источники:
- [WCAG 2.1.2 No Keyboard Trap — exit mechanism required (GetWCAG)](https://getwcag.com/en/accessibility-guide/keyboard-trap)
- [Build accessible modals with focus traps (UXPin)](https://www.uxpin.com/studio/blog/how-to-build-accessible-modals-with-focus-traps/)
- [WCAG 2.1.2 guide (TestParty)](https://testparty.ai/blog/wcag-2-1-2-no-keyboard-trap-2025-guide)
- [WCAG 2.1.2 — move focus freely (dock.codes)](https://wcag.dock.codes/documentation/wcag212/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Проверил Dioxus 0.6 + `mounted`-фича; идиома `e.key()` (woody_catch).
2. ✅ Заскоупил на 2 модалки с `EventHandler<()>` (Escape чистый); modal.rs (MouseEvent) — отдельно.
3. ✅ `tabindex=-1` + `onmounted`→`spawn(set_focus(true))` (focus into dialog).
4. ✅ `onkeydown` Escape→`on_close.call(())` (keyboard exit).
5. ✅ WASM compile (Key::Escape/KeyboardData/MountedData/spawn/set_focus); 69 defensive PASS; fmt.
6. ✅ Зафиксировал остаток (W-93c focus-return+tab-trap, W-93d admin Modal).
7. ✅ Research (modal keyboard lifecycle) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/ui/components/product_detail_modal.rs`** + **`video_modal.rs`** (dialog-контейнер): `tabindex: "-1"`, `onmounted: |e: Event<MountedData>| spawn(async move { let _ = e.set_focus(true).await; })`, `onkeydown: |e: Event<KeyboardData>| if e.key() == Key::Escape { on_close.call(()) }`. `on_close` — `EventHandler<()>` (Copy → переиспользуется в overlay/✕/Escape).

Проверка: `cargo check --target wasm32` → 0 warnings; backend; defensive 69 PASS; fmt clean. Honest: WASM focus/keyboard поведение не runtime-тестируемо здесь (нет browser-harness) — проверено компиляцией; изменения аддитивны (click-to-close не тронут).

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
104+ коммита ahead. Ops/review — твоё «go».

### Вариант B — ♿ Завершить W-93c/d
Focus-return-to-trigger + Tab-trap; и `Modal` (admin) — рефактор `EventHandler<MouseEvent>`→`()` для Escape.

### Вариант C — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

---

## 7. SKILL SAVED

Память: обновлён [[a11y-dialog-lang-labels]] (W-93a/b сделаны). Принцип: модалка обязана иметь клавиатурный выход (Escape + focusable ✕ — 2.1.2) и переносить фокус внутрь при открытии (`onmounted`+`set_focus`, 2.4.3); focus-trap уместен при наличии выхода; click-outside ≠ keyboard exit. Dioxus: `onkeydown` срабатывает только при фокусе внутри → автофокус обязателен для Escape.

**Anchor:** `phi^2 + phi^-2 = 3`
