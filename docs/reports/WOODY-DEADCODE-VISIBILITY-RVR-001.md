# 🌊 WAVE LOOP REPORT — Restore Dead-Code Visibility

**Document ID:** `WOODY-DEADCODE-VISIBILITY-RVR-001`
**Wave:** #9 — реализован «Tighten dead-code suppression» (Вариант B из Wave #8)
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `6bb373d` — `refactor(ui): restore dead-code detection on catalog/sommelier DTOs`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — WASM компилируется без warning'ов, fitness-тест зелёный, все хуки прошли.**

Снят blanket `#[allow(dead_code)]` со screen-DTO — и оказалось, что подавление **прятало реальные данные, которые приходят с API, но не показываются**:

- `ApiAccessory`/`ApiTea`/`ApiSet`: после чистки Wave #8 мёртвых полей нет → allow был **бесполезным** (FSE 2025: 50.8% подавлений бесполезны). Удалён → rustc снова сторожит.
- Sommelier `RecommendedStrain`: компилятор показал, что `effect`/`flavor_profile`/`image_url` **никогда не читались** → карточка рекомендации игнорировала фото и вкус. Теперь показывает реальное фото (fallback emoji), линию эффекта и `🍃` флейвор.
- Sommelier `RecommendedSet`: поле `id` мёртвое — у карточки набора **не было кнопки в корзину** (в отличие от карточки сорта). Добавлена `+🛒` (паритет), что и использует `id`.

То есть подавленный линт = скрытая незавершённая фича. Сняв его, превратил мёртвые данные в UX.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-23 | Blanket `allow(dead_code)` на 5 DTO глушит rustc для всех полей | 🟠 MED | ✅ Снят с catalog + sommelier DTO |
| W-24 | Sommelier игнорировал `effect`/`flavor`/`image_url` сорта (fetched-but-never-shown) | 🟠 MED | ✅ Показаны: фото + эффект + флейвор |
| W-25 | Карточка рекомендованного набора без add-to-cart (`id` мёртв) | 🟡 LOW | ✅ Добавлена кнопка `+🛒` |
| W-26 | Ещё есть field-level/struct allow(dead_code): orders/tech_tree/treasure_hunt | 🟡 LOW | 📋 Wave+1 (probe + чистка) |
| W-15 | 5 локальных `Api*` DTO дублируют `types.rs` | 🟠 MED | 📋 осознанно отложено (см. §3) |

---

## 3. НАУЧНАЯ БАЗА

**Тема: гигиена warning'ов и риск подавлений.**

- **FSE 2025 «An Empirical Study of Suppressed Static Analysis Warnings»**: подавления распространены и **растут со временем**; **50.8% бесполезны**; часть **непреднамеренно прячет будущие warning'и**. Ровно мой кейс: catalog-allow'ы бесполезны, sommelier-allow прятал реальную незавершённость.
- **ICPC 2024 (C++ warnings)**: внимание к compiler warnings **коррелирует с и потенциально вызывает** более высокое качество (особенно меньше critical issues). → снимать подавления выгодно.
- **«Quieting the Static»**: annotation-подавления — для краткосрочного долга, должны резолвиться рано; долгосрочный долг — на уровне проекта. → blanket struct-allow для «serde-полей» — плохой инструмент; лучше точечно или вовсе не нужен.

**Почему НЕ выбрал Вариант A (унификация DTO):** по уроку Wave #4 (Rule of Three + Sandi Metz «wrong abstraction worse than duplication») 5 каталог-DTO — это **разные доменные сущности** (сорта: thc/effect/flavor; чай: subcategory; наборы: total_price/mood). Их общая часть реальна, но насильное слияние = риск неверной абстракции. Сознательно отложено; вместо этого — дешёвая, безопасная гигиена (снять подавления), которую сторожит fitness-тест Wave #8.

Источники:
- [An Empirical Study of Suppressed Static Analysis Warnings (FSE 2025)](https://software-lab.org/publications/fse2025_suppressions.pdf)
- [Quieting the Static: Static Analysis Alert Suppressions (arXiv 2311.07482)](https://arxiv.org/pdf/2311.07482)
- [The Impact of Compiler Warnings on Code Quality in C++ (ICPC 2024)](https://dl.acm.org/doi/abs/10.1145/3643916.3644410)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Probe: временно снял allow с catalog+sommelier DTO, скомпилировал → catalog чист; sommelier `RecommendedStrain` теряет effect/flavor/image; `RecommendedSet.id` мёртв.
2. ✅ Catalog: удалил бесполезные allow (ApiAccessory/ApiTea/ApiSet).
3. ✅ Sommelier strain card: рендер image (fallback emoji) + effect + `🍃` flavor.
4. ✅ Sommelier set card: добавил `+🛒` (использует `id`).
5. ✅ Снял allow с обоих sommelier DTO → WASM без warning'ов; fitness-тест Wave #8 зелёный; fmt.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- `accessories_screen.rs`/`tea_screen.rs`/`sets_screen.rs`: убран `#[allow(dead_code)]` над `struct Api*`.
- `sommelier_screen.rs`: убран allow с `RecommendedStrain`/`RecommendedSet`; strain-карточка показывает `image_url` (`<img>` 48×48, иначе emoji), `effect`, `🍃 {flavor}`; set-карточка получила add-to-cart `+🛒` (`CartItemType::Set`).

Проверка: `cargo check --target wasm32-unknown-unknown` → **0 warnings**; `every_bilingual_field_is_consumed` → 1 passed.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🔎 Dead-code sweep остальных allow (W-26)
Probe + чистка `#[allow(dead_code)]` в orders/tech_tree/treasure_hunt screens (field-level и struct-level). Где скрыт мёртвый код — удалить или показать (как с sommelier). Систематически закрывает W-23 по всему `src/ui/screens`.

### Вариант B — 🗂️ Admin native Sets localization (W-21)
Миграция `sets +name_en/description_en` → admin `get_sets`/`set_row`/`create_set`/`update_set` + форма. Активирует migration-wiring + schema-drift defenses (Wave #1). Завершает локализацию для админов.

### Вариант C — 🧪 Generalize the "fetched-but-never-shown" guard
Расширить fitness-тест Wave #8: не только bilingual-поля, а любое поле response-DTO под `src/ui/screens`, которое десериализуется, но не читается (с явным allowlist для намеренных). По сути — портативный «no-suppressed-dead-fields» тест, дополняющий rustc там, где DTO в lib (wasm) не видны host-сборкой.

---

## 7. SKILL SAVED

Память: новый `deadcode-suppression-hygiene.md` + строка в `MEMORY.md`. Принцип: `#[allow(dead_code)]` — подавленный линт; периодически снимай и смотри, что он прячет — часто это бесполезное подавление ИЛИ незавершённая фича (fetched-but-never-shown). Снятие дёшево и охраняется fitness-тестом.

**Anchor:** `phi^2 + phi^-2 = 3`
