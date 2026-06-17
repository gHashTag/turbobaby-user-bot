# 🌊 WAVE LOOP REPORT — Auth-Gated Strains Variant: private/no-store + own ETag key

**Document ID:** `WOODY-CACHE-AUTH-RVR-001`
**Wave:** #62 — CORS/cache fresh-area scan (Вариант B из Wave #61)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `eb992d5` — `fix(strains): admin include_hidden variant is private/no-store with its own ETag key`
**Agent:** Claude Opus 4.8 (Wave loop) + Explore-subagent

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — auth-gated strains-вариант больше не shared-кэшируемый + свой ETag-ключ; 5 cache + 69 defensive зелёные; backend+WASM.**

Скан CORS + cache. **CORS — LOW**: `allow_origin(Any)` БЕЗ `allow_credentials` + header-token auth (`X-Admin-Token`/`X-Telegram-Init-Data`, не cookies) → нет ambient credentials для кражи; CORS почти неактуален. **Cache — агент заявил in-memory cross-user утечку тела; это НЕВЕРНО**: `ETagCache` хранит только хэш (`HashMap<String,String>`), тело всегда из текущего fetch'а; 304 только когда `current==stored==client-etag`.

Но **реальный (low-med) дефект**: `GET /api/strains?include_hidden=1` (admin-only: hidden rows + real flags) делил public ETag-ключ `"strains"` и отдавался с `Cache-Control: public, max-age=60`. Auth через **custom** `X-Admin-Token` (CDN не распознаёт как auth → не применяет дефолтную защиту «не кэшировать Authorization-ответы»), так что `public` был единственным контролем → shared-кэш/CDN мог закэшировать admin-payload и отдать его не-админу с тем же URL. Фикс: admin-вариант → ключ `"strains_admin"` + `Cache-Control: private, no-store`; public-вариант без изменений; `invalidate_strains` чистит оба ключа.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-96 | admin `include_hidden`-вариант: `public` cache-control + общий ETag-ключ → shared-кэш мог отдать admin-данные | 🟡 LOW-MED (CDN cross-serve, defense-in-depth) | ✅ `private, no-store` + ключ `strains_admin` |
| — | CORS `allow_origin(Any)` без credentials + header-token auth | 🟢 LOW | осознанно ок (нет ambient creds) |
| — | in-memory `ETagCache` cross-user body leak (заявлено агентом) | ❌ неверно | кэш хранит хэш, не тело |
| 🚨 | PR ветки (104+ ahead) + deploy #18-20 | 🔴 | ⚠️ ops/review |

---

## 3. НАУЧНАЯ БАЗА

**Тема: Cache-Control private/no-store для auth-ответов; cache key/Vary; не копируй static-заголовки на personalized.**

- **private vs public**: «private = single user, shared cache MUST NOT store; public = любой кэш». Personalized/auth-данные → `private`/`no-store`.
- **Authorization-default + override**: «shared caches don't store Authorization-responses BY DEFAULT — UNLESS `public`/`s-maxage` present». Старый `public` переопределял защиту. Нюанс: у нас custom `X-Admin-Token` → CDN не знает что это auth → `public` тем более опасен.
- **«Не копируй static-asset cache-заголовки на authenticated endpoint»**: «the single most common cause of cross-user CDN leaks». Ровно `public, max-age=60` на admin-варианте.
- **Тот же класс — CVE-2025-69202** (axios-cache-interceptor): разные auth-токены → неверные кэш-данные между сессиями. Фикс — honor Vary/per-auth key.
- Альтернатива: `Vary: Authorization` + auth в cache key; но `private, no-store` проще и безопаснее для редкого admin-эндпоинта.

Источники:
- [Cache-Control (MDN)](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Cache-Control)
- [Effective HTTP Caching: public/private/no-store (Factotum)](https://software-factotum.medium.com/effective-http-caching-part-iii-public-private-and-no-store-b64f0452325)
- [Cache Keys & Vary Headers — CDN correctness (Webstack Builders)](https://www.webstackbuilders.com/articles/cdn-edge-caching-cache-keys-vary-headers)
- [CVE-2025-69202 — cross-session cache leak (NVD)](https://nvd.nist.gov/vuln/detail/cve-2025-69202)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Скан CORS — LOW (header-token auth, нет cookies); честно зафиксировал.
2. ✅ Проверил `ETagCache` (cache.rs): хранит хэш, не тело → агентская «утечка тела» неверна.
3. ✅ Реальный дефект: admin-вариант `public` + общий ключ; custom-header auth → `public` единственный контроль.
4. ✅ Fix: `(cache_key, cache_control)` по `include_hidden`: admin→`strains_admin`/`private, no-store`, public→`strains`/`public, max-age=60`.
5. ✅ `invalidate_strains` чистит оба ключа.
6. ✅ 5 cache + 69 defensive PASS; backend+WASM; fmt.
7. ✅ Research (cache-control auth) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/api/strains.rs`** (`get_strains`): `let (cache_key, cache_control) = if include_hidden { ("strains_admin", "private, no-store") } else { ("strains", "public, max-age=60") };` → `has_changed(cache_key, …)` + `cache-control` header из `cache_control`.
- **`src/api/cache.rs`** (`invalidate_strains`): удаляет `"strains"` И `"strains_admin"`.

Проверка: 5 cache-тестов PASS; backend+WASM compile; defensive 69 PASS; fmt clean.

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

Память: новый `cache-control-auth-responses.md` + строка в `MEMORY.md`. Принцип: auth-gated/personalized ответы — `private`/`no-store`, НЕ `public` (shared-кэш отдаст их другому); не копируй static-asset cache-заголовки на authenticated endpoint; cache key должен включать все измерения (auth/variant) или Vary; custom auth-header (X-Admin-Token) не даёт дефолтной CDN-защиты — тем более нужен `private`. И: проверяй, кэширует ли структура **тело** или только хэш, прежде чем называть «cross-user leak».

**Anchor:** `phi^2 + phi^-2 = 3`
