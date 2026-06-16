# 🌊 WAVE LOOP REPORT — VideoModal Clone Sweep

**Document ID:** `WOODY-VIDEOMODAL-DEDUP-RVR-001`
**Wave:** #4 — реализован «Clone Sweep» (Вариант C из Wave #3)
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `993bcff` — `refactor(ui): extract shared VideoModal, de-dupe 4 copies of the video popup`
**Agent:** Claude Opus 4.8 (Wave loop, 15-min cadence)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — clone устранён, все хуки зелёные (check-wasm, defensive-tests, fmt, conventional-commits).**

Во время прошлой задачи (добавление `ProductDetailModal`) я заметил, что **видео-попап** (`▶️` → оверлей + `<video controls>` + «Закрыть») скопирован руками в **4 места**: `menu_screen`, `accessories_screen`, `sets_screen`, `admin_screen`. Более того, копии **уже разошлись**: admin использовал backdrop `rgba(0,0,0,0.8)` и `<source>`-child, остальные — `0.85` и `src`-атрибут. Это ровно тот класс отложенного дефекта, что описан в Wave #3 (Juergens 2009).

Извлёк в один компонент `VideoModal { url, on_close }`, заменил все 4 копии. Бонус-фикс: в модальный `<video>` добавлен `playsinline`/`webkit-playsinline` (как у инлайн-видео в карточках) — иначе в Telegram/iOS WebView плеер форсит fullscreen.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-12 | Видео-попап скопирован в 4 экрана, копии уже разошлись (0.8 vs 0.85, `<source>` vs `src`) | 🟠 MED | ✅ Устранён извлечением `VideoModal` |
| W-13 | Модальный `<video>` без `playsinline` → fullscreen-форс в iOS/Telegram | 🟡 LOW | ✅ Пофикшено в общем компоненте |
| W-14 | `ProductDetailModal` показывает ru-описание даже для EN-пользователей (`description_en` игнорируется; локальные структуры не десериализуют `_en`) | 🟠 MED | 📋 Wave+1 (см. §6, Вариант A) |
| W-15 | `Accessory` в `types.rs` без поля `description` (экраны используют локальные `Api*`-дубли структур) | 🟡 LOW | 📋 Wave+1 |
| W-07/W-03 | Honest language support / repo hygiene (из прошлых Wave) | LOW/MED | 📋 ещё открыты |

---

## 3. НАУЧНАЯ БАЗА

Прошлый Wave обосновал дедупликацию через Juergens et al. (ICSE 2009). Этот Wave добавляет **критерий «когда извлекать»**, чтобы не свалиться в обратную крайность (преждевременную абстракцию):

- **Rule of Three** — Martin Fowler, *Refactoring* (приписывается Don Roberts): «две копии терпим, на третьей — извлекаем». Здесь копий было **4** → извлечение оправдано экономически.
- **Sandi Metz, «The Wrong Abstraction»**: неверная абстракция дороже дублирования. Защита от этого риска: видео-попап имеет **стабильную, ясно именуемую** форму (`VideoModal`), все 4 копии семантически идентичны (расхождения — случайные, не «эссенциальные»), → абстракция корректна, а не натянута.
- **AHA / YAGNI**: абстрагируем, когда паттерн понятен. 4 живых примера + наблюдаемый drift = паттерн понятен.

Контраст с прошлым Wave (`lang_code`): там было 2 копии каноничного кода — дедуп был тривиален (делегирование). Здесь — 4 копии UI-блока с дрейфом, классический кейс Rule of Three.

Источники:
- [Rule of three (computer programming) — Wikipedia](https://en.wikipedia.org/wiki/Rule_of_three_(computer_programming))
- [Clarifying the Rule of Three in Refactoring — jbrains](https://blog.jbrains.ca/permalink/clarifying-the-rule-of-three-in-refactoring/)
- [Don't make Clean Code harder to maintain, use the Rule of Three — understandlegacycode](https://understandlegacycode.com/blog/refactoring-rule-of-three/)
- Juergens et al., [«Do Code Clones Matter?», ICSE 2009](https://dl.acm.org/doi/10.1109/ICSE.2009.5070547) (база Wave #3)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Разведка: grep оверлей-блока → 4 копии (menu/accessories/sets/admin); подтверждён drift (0.8 vs 0.85, `<source>` vs `src`).
2. ✅ Создан `src/ui/components/video_modal.rs` (`VideoModal { url, on_close }`) + экспорт в `mod.rs`.
3. ✅ Заменены все 4 инлайн-копии вызовом компонента; импорты добавлены.
4. ✅ Бонус: `playsinline`/`webkit-playsinline` в модальном плеере.
5. ✅ `cargo check --target wasm32` зелёный; `cargo fmt`; grep подтвердил 0 инлайн-плееров в screens.
6. ✅ Commit через lefthook (все хуки) + research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/ui/components/video_modal.rs`** — `#[component] VideoModal`: фикс-оверлей `inset:0;z-index:1000`, клик по фону `on_close`, внутренний контейнер `stop_propagation`, `<video controls src playsinline webkit-playsinline>`, кнопка «Закрыть». Зарегистрирован в `components/mod.rs`.
- **Замена** в `menu_screen` / `accessories_screen` / `sets_screen` / `admin_screen`: инлайн-блок → `{show_video().then(|| rsx!{ VideoModal { url: …, on_close: move |_| show_video.set(false) } })}` (sets — bare `if show_video()`). `show_video`-сигнал и `▶️`-кнопка остаются на местах.

Проверка: `grep 'max-height:60vh' src/ui/screens` → **0** (все плееры в общем компоненте). `cargo check --target wasm32-unknown-unknown` → clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🌍 Bilingual descriptions (W-14, прямое продолжение)
`ProductDetailModal` (и карточки) показывают только ru `description`. Бэкенд уже возвращает `description_en` (migration 016). Добавить `description_en` (и `name_en` и т.п.) в локальные `Api*`-структуры экранов и выбирать по `current_lang()` с fallback на ru. Высокая ценность для EN/не-ru пользователей; ограниченный, чёткий объём.

### Вариант B — 🧱 Unify the Api* DTO duplication (W-15, развитие §3)
Каждый экран (`menu`/`accessories`/`tea`/`sets`) держит **свой локальный** `ApiStrain`/`ApiAccessory`/`ApiTea`/`ApiSet`, частично дублируя `types.rs`. Свести к единым DTO в `src/ui/api/types.rs` (где уже есть `Strain`/`TeaProduct`/`Set`). Это «следующая итерация Rule of Three» — 4 параллельных DTO-дубля каталога.

### Вариант C — 🧹 Repo Hygiene (W-03, нужно подтверждение)
Вынести `dist/.stage/`, `*.mp4`, аплоады из git (221 трекаемый бинарник). Требует подтверждения по деплою Railway/Trunk и согласования с уже идущей очисткой.

---

## 7. SKILL SAVED

Память: `videomodal-clone-sweep.md` + строка в `MEMORY.md`. Закреплён критерий принятия решения: **Rule of Three + «wrong abstraction worse than duplication»** — извлекать общий компонент, когда копий ≥3 И форма стабильна/ясно именуема; до этого дублирование терпимо.

**Anchor:** `phi^2 + phi^-2 = 3`
