# 🌊 WAVE LOOP REPORT — First Dependency Security Audit (cargo-audit / RustSec)

**Document ID:** `WOODY-DEP-AUDIT-RVR-001`
**Wave:** #63 — supply-chain fresh-area scan (Вариант B из Wave #62)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `4e5b046` — `chore(security): triage RustSec dep advisories via tracked audit.toml`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — первый dep-аудит: 5 RustSec-адвизори триажированы (reachability), `cargo audit` exit 0; реальный fix rustls-стека = W-97 (нужен S3-тест).**

Первый прогон `cargo audit` (554 транзитивных зависимости) нашёл **5 адвизори**, все **транзитивные** и **low practical risk** для этого приложения. Каждая триажирована по достижимости и зафиксирована в `.cargo/audit.toml` (осознанное, документированное принятие — не silent ignore; добавил `.gitignore`-исключение, чтобы security-решения были под версионным контролем и авто-читались `cargo audit`).

**Честно: безопасного code-фикса в этот Wave не было** — все 5 либо транзитивны без upstream-фикса, либо требуют рискованного TLS-bump'а AWS SDK, который нельзя слепо делать без S3 integration-теста (риск тихо сломать прод-S3). Это полноценный security-аудит + tracked finding (W-97), а не суррогат.

---

## 2. WEAK-SPOT MATRIX

| # | Адвизори | Crate (транзит) | Reachability / триаж | Действие |
|---|---|---|---|---|
| W-97 | RUSTSEC-2026-0104 (CRL panic) | rustls-webpki 0.101.7 ← AWS SDK `rustls` (rustls 0.21) | CRL не используется (grep=0) → недостижим | ignore + W-97 fix |
| W-97 | RUSTSEC-2026-0098/0099 (name-constraint cert) | rustls-webpki 0.101.7 | MITM с крафт-сертом на **доверенный, опциональный** S3-эндпоинт → low | ignore + W-97 fix |
| — | RUSTSEC-2026-0173 (proc-macro-error2 unmaintained) | sea-orm-macros/teloxide | build-time proc-macro, нет runtime | ignore (warning) |
| — | RUSTSEC-2023-0071 (rsa Marvin Attack, no fix) | rsa 0.9.10 ← sqlx-mysql (compile-time macro) | app **Postgres-only**, MySQL-RSA путь не исполняется | ignore |
| 🚀 W-97 | AWS SDK rustls 0.21→0.23 (real fix) | — | нужен корректный TLS-feature + **S3 integration-тест** | 📋 sign-off / dedicated |
| 🚨 | PR ветки (104+ ahead) + deploy #18-20 | — | — | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: cargo-audit/RustSec; триаж транзитивных уязвимостей по достижимости; upgrade-vs-accept; CI.**

- **cargo-audit сканит весь Cargo.lock** (direct + transitive) против RustSec DB; вывод даёт dependency-tree для триажа.
- **Default = upgrade, НО upgrades каскадят**: «a newer version that fixes a vuln might break an integration point — validate transitive bumps, test in CI». Ровно почему не делаю слепой AWS TLS-bump без S3-теста.
- **Accept legitimate ТОЛЬКО при недостижимости**: «accepting is legitimate only when the vulnerable code path isn't reachable in how you use the dependency». Мой триаж (CRL unused; MySQL-RSA не исполняется; S3 опционален/доверен).
- **Suppress with traceability**: «document each suppression with justification + owner + (ideally) expiry; don't grandfather forever». audit.toml с обоснованиями + W-97 + REVISIT-условиями.
- **CI**: «run `cargo audit --deny warnings`, commit Cargo.lock». Триаж делает audit зелёным → можно гейтить НОВЫЕ.

Источники:
- [cargo-audit (RustSec, README)](https://github.com/rustsec/rustsec/blob/main/cargo-audit/README.md)
- [RustSec Advisory Database](https://rustsec.org/)
- [Rust/Cargo supply-chain security: cargo-audit, cargo-deny (SystemsHardening)](https://www.systemshardening.com/articles/cicd/rust-cargo-supply-chain-security/)
- [Transitive dependency risks (Rafter)](https://rafter.so/blog/transitive-dependency-risks)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ `cargo audit` (инструмент установлен) → 5 адвизори (rustls-webpki ×3, proc-macro-error2, rsa).
2. ✅ Триаж достижимости: CRL grep=0; rustls 0.21 только через AWS SDK `rustls`-feature; app/sea-orm/teloxide на rustls 0.23; S3 опционален (`s3_enabled()`); rsa via sqlx-mysql compile-time, app Postgres-only.
3. ✅ Проверил: `cargo update` (dry-run) НЕ двигает rustls-цепочку → нужен major TLS-bump (не semver-fix).
4. ✅ `.cargo/audit.toml` с ignore + per-ID обоснованием + REVISIT/W-97; `.gitignore`-исключение для трекинга.
5. ✅ `cargo audit` → exit 0 (554 deps, 0 необработанных).
6. ✅ Research (cargo-audit/триаж) + отчёт + skill. Зафиксировал W-97 (AWS rustls-bump + S3-тест).

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`.cargo/audit.toml`** (новый, tracked): `[advisories] ignore = [...]` 5 ID с подробными обоснованиями (reachability) + REVISIT-условия + ссылка на W-97.
- **`.gitignore`**: исключение `!.cargo/audit.toml` (остальное в `.cargo/` остаётся ignored).

Проверка: `cargo audit` → exit 0; backend+WASM compile (lock не менялся); fmt clean. Lock НЕ тронут (без рискованного bump'а).

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🚀 PR ветки `fix/profile-routing` → `main` (нужен sign-off)
104+ коммита ahead. Ops/review — твоё «go».

### Вариант B — 🔐 W-97: AWS SDK на rustls 0.23 + S3 integration-тест (нужен sign-off/тест-окружение)
Переключить aws-config/aws-sdk-s3 TLS-feature на rustls-0.23-цепочку, удалить webpki-0.101 ignore'ы, прогнать S3-хендшейк (MinIO/реальный). Требует тест-окружения S3.

### Вариант C — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс.

---

## 7. SKILL SAVED

Память: новый `dependency-audit-cargo-audit.md` + строка в `MEMORY.md`. Принцип: гоняй `cargo audit` (установлен) периодически; триаж транзитивных по **достижимости** (не слепой bump — каскадит/ломает); accept только undeachable + документируй (audit.toml с обоснованием + REVISIT/owner); сделай audit зелёным, чтобы CI гейтил НОВЫЕ (`--deny warnings`). Текущий долг: W-97 (AWS rustls 0.21→0.23, нужен S3-тест).

**Anchor:** `phi^2 + phi^-2 = 3`
