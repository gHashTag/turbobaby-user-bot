# 🌊 WAVE LOOP REPORT — Security-Header Fitness Function

**Document ID:** `WOODY-SECHEADERS-RVR-001`
**Wave:** #34 — Вариант A из Wave #33 (W-65: аудит security-заголовков на отдаче)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `003d67c` — `test(security): fitness function guarding HTTP security-header layers`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — заголовки уже хорошо настроены; добавлен регрессионный guard. 67 defensive-тестов + 3 новых зелёные.**

Аудит W-65 показал приятную новость: security-заголовки **уже выставляются** в `run_server` (`src/main.rs`) — `X-Content-Type-Options: nosniff`, `Content-Security-Policy` (с `frame-ancestors https://*.telegram.org`), `Referrer-Policy: strict-origin-when-cross-origin`. Применяются и на scoped-роутерах (SPA/static), и глобально на `app` через `if_not_present` (без двойного применения). `nosniff` дополняет magic-byte-проверку из Wave #33 на стороне отдачи.

**Реальный пробел был не в заголовках, а в их незащищённости**: рефактор мог молча выкинуть CSP/nosniff/referrer из роутеров — и ни один тест бы не поймал. Это сигнатурный класс багов проекта (config drift), и индустрия прямо предписывает регрессионный тест («if there is a bug on prod, add a regression test so it never happens again»; «validate values, not just presence»). Добавил `security_headers_tests` — source-scanning fitness function, **0 изменений поведения**.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-65 | Security-заголовки не защищены тестом (рефактор мог их выкинуть) | 🟡 LOW | ✅ `security_headers_tests` (3 guard'а: значения / global+scoped применение / нет ломающего XFO) |
| — | Заголовки сами по себе корректны (nosniff/CSP+frame-ancestors/referrer) | ✅ | — |
| W-66 | HSTS (`Strict-Transport-Security`) не выставляется | 🟢 INFO | 📋 нужен sign-off (hard-to-reverse: preload постоянный) |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: clickjacking (frame-ancestors vs X-Frame-Options), HSTS, security-headers regression testing.**

- **`frame-ancestors` вытесняет `X-Frame-Options`**: при наличии CSP `frame-ancestors` современные браузеры **игнорируют** XFO. XFO умеет только `DENY`/`SAMEORIGIN` (значение `ALLOW-FROM` мёртвое) → XFO здесь **сломал бы** Telegram-iframe, а `frame-ancestors https://*.telegram.org` точечно разрешает нужный origin. ⇒ текущий выбор репо **верный**; guard защищает от «полезного» добавления XFO.
- **HSTS** рекомендуется (`max-age=31536000; includeSubDomains; preload`), но это **hard-to-reverse, долгосрочное обязательство** (preload вкомпилирован в браузеры, удаление — месяцы). Best practice — staged rollout (короткий max-age → год → preload) после проверки всех сабдоменов на HTTPS. ⇒ outward-facing, требует sign-off, **не делаю автономно** (W-66).
- **Security-headers regression testing**: «automated testing catches config drift when headers get removed during deployments»; «validate header values, not just presence». ⇒ ровно то, что добавлено.

Источники:
- [CSP frame-ancestors vs X-Frame-Options (Invicti)](https://www.invicti.com/blog/web-security/missing-x-frame-options-header)
- [Clickjacking Defense (OWASP Cheat Sheet)](https://cheatsheetseries.owasp.org/cheatsheets/Clickjacking_Defense_Cheat_Sheet.html)
- [Strict-Transport-Security (MDN)](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Strict-Transport-Security)
- [HSTS Cheat Sheet (OWASP)](https://cheatsheetseries.owasp.org/cheatsheets/HTTP_Strict_Transport_Security_Cheat_Sheet.html)
- [Automated Security Headers Testing (LoadForge)](https://loadforge.com/blog/automated-test-for-security-headers)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Аудит `run_server`: nosniff/CSP+frame-ancestors/referrer уже есть, применены на SPA/static + глобально (`if_not_present`).
2. ✅ Вывод: пробел — отсутствие guard'а (config-drift) + отсутствие HSTS (требует sign-off).
3. ✅ `security_headers_tests` — 3 теста: (a) значения заголовков; (b) применение на global+scoped (≥2 call-site на layer); (c) запрет XFO, ломающего Telegram.
4. ✅ Анти-самореференция: `prod_src()` обрезает скан по первому `#[cfg(test)]` (иначе assertion-строки раздули бы счётчики).
5. ✅ 3 теста + 67 defensive-тестов зелёные; backend+WASM; fmt.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/main.rs`** (новый `mod security_headers_tests`, `#[cfg(test)]`):
  - `prod_src()` — читает `src/main.rs`, обрезает по первому `#[cfg(test)]` (скан только prod-кода).
  - `security_header_layers_are_defined_with_correct_values` — `nosniff` == «nosniff», CSP присутствует + `frame-ancestors`, `REFERRER_POLICY`.
  - `security_headers_applied_on_global_and_scoped_routers` — `csp_layer()`/`referrer_layer()`/`nosniff_layer()` имеют ≥2 call-site (global + scoped).
  - `no_x_frame_options_that_would_break_telegram_embed` — prod-код НЕ содержит `X_FRAME_OPTIONS`/`x-frame-options`.

Проверка: `cargo test … security_headers_tests` → 3 PASS; defensive-tests → 67 PASS; WASM check 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🆕 Fresh-area scan
Следующий нетронутый модуль на реальный дефект (upload/bot/headers уже признаны крепкими + защищены).

### Вариант B — 🔐 HSTS (W-66) — нужен твой sign-off
Добавить `Strict-Transport-Security` (staged: короткий `max-age` → год → `preload`). Hard-to-reverse (preload постоянный) → outward-facing решение, делаю только с подтверждением и предпочтительно без `preload` на старте.

### Вариант C — 🚀 Deploy-долг (#18-#20) — нужен твой «go»
Cherry-pick прод-фиксов `/api/sets` 500 в `main` + деплой. Ops-действие.

---

## 7. SKILL SAVED

Память: новый `security-headers-guard.md` + строка в `MEMORY.md`. Принцип: корректные security-заголовки **недостаточны без fitness-function** — config drift тихо их выкидывает; тестируй значения, не только присутствие; на Telegram-mini-app clickjacking-защита = CSP `frame-ancestors` (НЕ X-Frame-Options, который сломал бы iframe). HSTS — hard-to-reverse, только с sign-off.

**Anchor:** `phi^2 + phi^-2 = 3`
