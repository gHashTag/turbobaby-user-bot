# 🌊 WAVE LOOP REPORT — SQL-Injection Audit (clean) + from_string Static Guard

**Document ID:** `WOODY-SQLI-GUARD-RVR-001`
**Wave:** #61 — SQL-construction fresh-area scan (Вариант B из Wave #60)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `296f182` — `test(security): fitness fn — Statement::from_string must be static SQL (no SQLi)`
**Agent:** Claude Opus 4.8 (Wave loop) + Explore-subagent

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — SQL-слой подтверждён чистым (нет SQLi) + зафиксирован static-guard'ом; 70 defensive зелёные.**

Скан SQL-construction (свежий высокий класс). **Результат: SQL-слой чист.** Все 33 `Statement::from_string(..)` используют статичный SQL; каждое user-controlled значение идёт через `from_sql_and_values($1, …)` bind-параметры; LIMIT/OFFSET клампятся (`orders` 1..=500, `referrals` 1..=50, хардкод ≤5000); ORDER BY — статичный/enum (нет user column-names); нет LIKE-инъекций. Дефекта нет.

Поэтому (правило scoped-fitness-function) зафиксировал ключевой инвариант: `sql_injection_guard_tests` — ни один `from_string(` в production-коде не содержит `format!`. Логика: `from_string` **не принимает bind-параметры**, поэтому любое `format!`-интерполированное значение = unparameterized injection vector (должно идти через `from_sql_and_values`). Это проектный аналог SQLi-линтеров (SafeQL / Go SQL-analyzers).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-95 | SQLi-инвариант (`from_string` = static) не защищён от регрессии | 🟢 INFO | ✅ `sql_injection_guard_tests` (from_string без format!) |
| — | 33 from_string статичны; user-values через `$1`; LIMIT clamp; ORDER BY enum/static — clean | ✅ | подтверждено аудитом |
| 🚨 | PR ветки (104+ ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: SQLi prevention — bind params, не конкатенация; identifiers → allowlist; static-lint raw SQL.**

- **Bind params, не конкатенация**: «parameterized queries … values can never become SQL syntax; manual escaping is fragile». Репо: `from_sql_and_values($1,…)` везде для user-values.
- **Identifiers нельзя параметризовать → allowlist**: «bound params work for values, not table/column names or ASC/DESC … map to a fixed set». Репо: ORDER BY статичный/enum (нет dynamic user column).
- **Audit DB-кода**: «audit the code that touches the database, looking for concatenated SQL / raw query fragments». Ровно мой скан.
- **Static-lint raw SQL**: «tools that surface unsafe queries / SQL injections (SafeQL, Go analyzers)». Мой `from_string`-guard — такой проектный линтер.
- **DB-ошибки sensitive**: «don't write raw SQL with sensitive values to logs/errors» — связано с error-leak фиксом ([[upload-hardening]]).

Источники:
- [SQL Injection Prevention Cheat Sheet (OWASP)](https://cheatsheetseries.owasp.org/cheatsheets/SQL_Injection_Prevention_Cheat_Sheet.html)
- [Crafting Parameterized Queries (ITU Online)](https://www.ituonline.com/blogs/crafting-parameterized-queries-to-prevent-sql-injection-attacks/)
- [SQL static analysis tools / linters (analysis-tools.dev)](https://www.analysis-tools.dev/tag/sql)
- [Beginner's Guide to SQL Injection & prevention (OpenReplay)](https://blog.openreplay.com/beginners-guide-sql-injection-prevent/)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Скан: 33 from_string статичны; user-values через bind `$1`; LIMIT clamp; ORDER BY enum/static; нет LIKE-инъекций. Чисто.
2. ✅ Греп подтвердил: 0 `from_string` содержат `format!` (одно-/многострочно).
3. ✅ Lock-in: `sql_injection_guard_tests::from_string_sql_is_always_static_no_format_interpolation` — рекурсивно сканит `src/` (без ui, без test-модулей), на каждый `from_string(` проверяет окно до `;` на `format!`.
4. ✅ Тест PASS; backend+WASM; defensive 70 PASS; fmt.
5. ✅ Research (OWASP SQLi / static-lint) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/main.rs`**: `#[cfg(test)] mod sql_injection_guard_tests` — `rs_files()` рекурсия по `src/`; для каждого non-ui файла берёт prod-часть (до `#[cfg(test)]`), на каждый `from_string(` проверяет аргумент-окно (до `;`) на `format!`; offenders → fail с инструкцией использовать `from_sql_and_values`.

Проверка: тест PASS (33 from_string, 0 offenders); backend+WASM compile; defensive 70 PASS; fmt clean.

Honest scope: guard ловит `from_string`+`format!` (главный injection vector в SeaORM, т.к. from_string без bind). Он НЕ валидирует корректность `from_sql_and_values` (безопасны по конструкции) и не нужен для ORDER BY (dynamic user-column отсутствует).

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
104+ коммита ahead. Ops/review — твоё «go».

### Вариант B — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

### Вариант C — ♿/🛡️ Завершить остатки
W-93c/d (modal focus-trap/return), W-68/W-83 (DB CHECK-backstops — sign-off).

---

## 7. SKILL SAVED

Память: новый `sql-injection-guard.md` + строка в `MEMORY.md`. Принцип: user-values только через bind `$1` (`from_sql_and_values`); `from_string` = строго static SQL (нет bind → format! = инъекция); identifiers (ORDER BY/column) — allowlist/enum, не user-string; когда SQL-слой чист — зафиксируй проектным static-линтером (from_string без format!), чтобы регрессия не открыла дыру.

**Anchor:** `phi^2 + phi^-2 = 3`
