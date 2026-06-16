# 🌊 WAVE LOOP REPORT — i18n Translation-Completeness Defense

**Document ID:** `WOODY-I18N-COMPLETENESS-RVR-001`
**Wave:** #2 (продолжение migration-wiring Wave) — реализован «Generalize the Wiring Invariant» из прошлого отчёта
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `de347ad` — `test(i18n): fitness test — every T_* key is translated in ru and en`
**Agent:** Claude Opus 4.8 (Wave loop, 15-min cadence)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — invariant shipped, lefthook чист, тест зелёный (lib + bin).**

Этот Wave реализует **Вариант C** из прошлого отчёта (`WOODY-MIGRATION-WIRING-RVR-001`): обобщение «ручной реестр → fitness-тест» на i18n-слой.

Находка: `src/trios/i18n.rs` определяет **99** `pub const T_*: Key` и сопоставляет их в `get_ru_translation` / `get_en_translation` через `match key { … _ => key }`. Сейчас все 99 переведены в обоих языках — **но это поддерживается руками без защиты**. Любой новый `T_*` без arm'а в match'е **молча проваливается в `_ => key`** и рендерит сырой dotted-ключ ("nav.home") прямо в UI. Компилятор молчит: catch-all «использует» const. Это в точности класс багов, описанный в индустрии как *silent i18n fallback*.

Реализована fitness-функция, парсящая исходник и проверяющая `has_translation()` для каждого ключа в ru и en.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-06 | 99 `T_*` ключей переведены вручную, нет защиты от пропуска arm → сырой ключ в UI | 🟠 MED | ✅ Закрыт fitness-тестом `every_translation_key_is_translated_in_ru_and_en` |
| W-07 | 6 языков (`Thai/Chinese/Hebrew/German/French/Spanish`) маппятся на English fallback — `t()` никогда не вернёт локализацию, хотя язык «поддерживается» | 🟡 LOW | 📋 Wave+1 (см. §6, Вариант A) |
| W-08 | Нет плюрализации (`_one/_few/_many/_other`); русский требует 3 формы — числа в UI могут звучать неестественно | 🟡 LOW | 📋 Wave+1 |
| W-09 | Инверсная проверка (arm в match без объявленного `T_*` const) не покрыта (сейчас пусто, но дрейфует) | 🟢 INFO | 📋 Опционально |

Прочее: прод-`unwrap()` ≈ 0 (подтверждено в Wave #1), 155 lib + 629 backend тестов зелёные.

---

## 3. НАУЧНАЯ / ИНДУСТРИАЛЬНАЯ БАЗА

Тема: **silent i18n fallback** и **build-time key-parity checking**.

> «Most i18n libraries fail silently… If a locale file is missing a key, the library falls back to the raw key string. No error is thrown. The page renders. It just renders wrong… Each scenario produces the same symptom: raw dotted keys visible to real users.» — *How to find missing i18n keys (DEV Community)*

Рекомендуемый индустрией подход — **многослойная защита**: build-time lint на **структурный паритет ключей** + runtime-проверка (Playwright). Наш fitness-тест — это именно build-time слой (бежит в lefthook `defensive-tests`), останавливающий merge до прода. Тулинг этого класса: `i18n-check` (lingualdev), `i18next-cli status --ci` (exit≠0 при пропуске), `i18n-validate` (`--min-coverage`, JUnit). Мы воспроизвели их суть нативно на Rust, без внешних зависимостей.

Связь с прошлым Wave: тот же класс **architectural fitness function** (Ford/Parsons/Kua), что `migration_wiring_tests`, `entity_schema_consistency_tests`, `ui_module_wiring_tests`.

Будущее (упомянуто в источниках): **pseudolocalization** ("Save"→"Šàvē") для выявления layout-breakage и хардкод-строк; **plural completeness** для ru/Arabic; **RTL** для Hebrew (уже в списке языков — W-07).

Источники:
- [How to find missing i18n keys without losing your mind (DEV Community)](https://dev.to/dev_harry/how-to-find-missing-i18n-keys-without-losing-your-mind-or-your-job-1jff)
- [Missing Translations in i18next: Fallbacks, Detection & Fixes (Locize)](https://www.locize.com/blog/missing-translations/)
- [i18n-check — Validate i18n translation files (lingualdev)](https://github.com/lingualdev/i18n-check)
- [i18n Testing: Catching Missing Translations Automatically (Autonoma)](https://getautonoma.com/blog/i18n-testing-catching-missing-translations)
- [i18n Testing — A Practical Guide for QA Engineers (A. Antonov)](https://medium.com/@AntonAntonov88/i18n-testing-a-practical-guide-for-qa-engineers-a92f7f4fc8b2)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Survey + выбор: реализовать Вариант C прошлого Wave на i18n-слое.
2. ✅ Разведка: нашёл `t()` → `get_ru/en_translation` с `_ => key` fallthrough; 99 const vs 86… (артефакт regex) → перепроверка с цифрами: 99/99/99, все переведены, но без защиты.
3. ✅ Root-cause: ручной реестр `match`, молчаливый fallback на сырой ключ.
4. ✅ Реализация: тест `every_translation_key_is_translated_in_ru_and_en` (парсит исходник, `has_translation` ru+en).
5. ✅ Доказательство: probe-const `T_WAVE_PROBE` без arm → тест FAIL (ru+en), восстановлено.
6. ✅ Зелёный прогон + commit через lefthook.
7. ✅ Research + отчёт + skill-memory.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

`src/trios/i18n.rs`, новый тест в `mod tests`:

- **`every_translation_key_is_translated_in_ru_and_en`** — читает собственный исходник, извлекает строковое значение каждого `pub const T_*: Key = "…";`, и для каждого проверяет `has_translation(Lang::Russian, k)` и `has_translation(Lang::English, k)`. Парсит исходник (а не второй список, который сам бы дрейфовал). Лимит `keys.len() >= 90` — sanity на случай поломки парсера. `Key = &'static str`, поэтому значение `Box::leak`-ается (one-shot, только в тест-процессе).

**Проверка-доказательство:** добавлен `pub const T_WAVE_PROBE: Key = "wave.probe.unguarded";` без arm → тест упал:
```
2 translation key(s) have no arm and fall through to `_ => key` ...
  wave.probe.unguarded (ru)
  wave.probe.unguarded (en)
```
Probe удалён, дерево чистое.

Запуск: `cargo test --lib every_translation_key` → `1 passed`.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🌐 Honest Language Support (закрыть W-07)
Сейчас `Thai/Chinese/Hebrew/German/French/Spanish` молча отдают English. Либо (1) добавить реальные таблицы переводов, либо (2) честно сократить `supported_languages()` до `[ru, en]`, чтобы UI не обещал языки, которых нет. Бонус: fitness-тест расширяется на все «заявленные» языки. Эффект: пользователь не выбирает «中文» и не получает английский.

### Вариант B — 🧹 Repo Hygiene (перенесён из Wave #1, W-03)
Вынести `dist/.stage/`, `*.mp4`, аплоады, `*_bg.wasm` из git-трекинга (221 трекаемый бинарник топит диффы). Подтвердить, что Railway/Trunk регенерируют `dist/` при деплое. Низкий риск, высокий сигнал-шум выигрыш.

### Вариант C — 🛡️ Inverse + Pseudoloc (закрыть W-09, развить §3)
(1) Инверсный fitness-тест: arm в `get_*_translation` без объявленного `T_*` const = мёртвый перевод. (2) Pseudolocalization-режим (`Lang::Pseudo` → "Šàvē") как dev-инструмент для отлова хардкод-строк и layout-breakage. Развивает индустриальный «layered approach» из §3.

---

## 7. SKILL SAVED

Память: `i18n-completeness-defense.md` + строка в `MEMORY.md`. Паттерн «ручной `match`-реестр → парсинг исходника → fitness-тест» закреплён как переиспользуемый для будущих Wave (третий применённый случай после migrations и schema-drift).

**Anchor:** `phi^2 + phi^-2 = 3`
