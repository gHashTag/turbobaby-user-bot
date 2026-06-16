# 🌊 WAVE LOOP REPORT — Quest Location: No Fabricated id=0

**Document ID:** `WOODY-QUEST-FAKEID-RVR-001`
**Wave:** #42 — fresh-area scan (Вариант A из Wave #41) → fake-id дефект в `src/api/quest.rs`
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `b8c2174` — `fix(quest): fail loud on location id reads instead of fabricating id=0`
**Agent:** Claude Opus 4.8 (Wave loop) + Explore-subagent

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — fabricated-id дефект устранён в 3 местах; 51 quest + 69 defensive зелёные; backend+WASM.**

Fresh-area scan нашёл реальный дефект класса «silent-default возвращает фальшивый идентификатор клиенту». `create_quest_location` (`api/quest.rs`) на CREATE возвращал `{"success": true, "id": try_get::<i32>("id").unwrap_or(0)}` — при сбое чтения `RETURNING id` клиент получал **сфабрикованный id=0** для только что созданной записи и затем ссылался на несуществующую location. Тот же fake-id паттерн в `scan_quest_qr` (success-ответ) и `get_quest_locations` (list).

`id` — NOT NULL PK, так что сбой `try_get` = реальный schema-drift → **fail loud** (propagate `DbErr`→500) корректнее, чем эмитить `id: 0`. Починил все 3. Намеренно **не** расширял `sensitive_read_fail_loud_tests` на `"id"` — это зафлагало бы множество легитимных display `unwrap_or_default()` на string-id (сохранил guard высокосигнальным). `util.rs::truncate_string` (вторая находка агента) перепроверен — **не баг** (обе ветви дают корректный char-результат; byte-fast-path лишь консервативен).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-77 | `create_quest_location:554` CREATE возвращает фальшивый `id=0` при сбое чтения RETURNING | 🟡 LOW (correctness, client acts on id) | ✅ fail loud (`map_err→500`) |
| W-77b | `scan_quest_qr:641` + `get_quest_locations:496` тот же fake-id паттерн (i32) | 🟡 LOW | ✅ fail loud (оба) |
| — | `util.rs::truncate_string` byte/char | ✅ | перепроверен: не баг (обе ветви корректны) |
| — | String-id display `unwrap_or_default` (catalog/tech_tree/quest list) | ✅ | display-path, не трогаю (high-signal guard) |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: CREATE-эндпоинт должен возвращать реальный id; 0-как-id анти-паттерн; RETURNING-целостность.**

- **«Never serialize 0 (or any sentinel) as the resource ID — if no ID exists, that's a server error, not a value to send to clients.»** Ровно мой фикс: сбой чтения id → 500, не `id:0`.
- **Response — это контракт**: «client cannot later fetch/update/delete the resource it just created» с фальшивым id.
- **0-как-id — magic-number анти-паттерн**: «NULL is not the same as zero»; `0` семантически перегружен (во многих БД 0 *триггерит* авто-генерацию) → никогда не «финальный id для клиента».
- **RETURNING даёт реальный ключ атомарно**: «compute and return value(s) based on each row actually inserted» — id уже на руках; `.unwrap_or(0)` поверх этого лишь прячет drift.

Источники:
- [The Best Way to Return Responses in REST APIs (Jindal)](https://medium.com/@vikkasjindal/the-best-way-to-return-responses-in-rest-apis-f248113e385e)
- [PostgreSQL RETURNING clause for generated keys](https://www.postgresql.org/message-id/45B6E568.2090208%40kensystem.com)
- [Antipatterns: Magic Numbers (Baeldung CS)](https://www.baeldung.com/cs/antipatterns-magic-numbers)
- [Magic number (programming) — Wikipedia](https://en.wikipedia.org/wiki/Magic_number_(programming))

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Fresh-area scan (Explore) → `create_quest_location` fake-id; перепроверил `truncate_string` (не баг).
2. ✅ Grep кластера: 3 i32 location-id silent-default (554 create, 641 scan, 496 list).
3. ✅ Подтвердил: id — NOT NULL PK → сбой = drift → fail loud (не display-косметика).
4. ✅ 554 + 641: `map_err(...→500)?`; 496: реструктурировал `.map` closure → `Result` + `collect::<Result<_,_>>()?`.
5. ✅ Решил НЕ добавлять `"id"` в fitness-функцию (зашумило бы legitimate display unwrap_or_default).
6. ✅ 51 quest + 69 defensive PASS; backend+WASM; fmt.
7. ✅ Research (REST create id / magic-number) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/api/quest.rs`**:
  - `create_quest_location`: `let id: i32 = row.try_get("", "id").map_err(…→500)?;` → `{"success": true, "id": id}`.
  - `scan_quest_qr`: `let id: i32 = r.try_get("", "id").map_err(…→500)?;` в success-ветке.
  - `get_quest_locations`: closure читает `id` через `map_err(…→500)?`, `.collect::<Result<Vec<Value>, StatusCode>>()?`.

Проверка: build OK; `cargo test … quest` → 51 PASS; defensive-tests → 69 PASS; WASM 0 warnings; fmt clean. Honest scope: handler-level runtime-тест отсутствует (нет harness, [[integration-test-harness-blocker]]); правки механические (propagate vs sentinel).

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🆕 Fresh-area scan
Следующий нетронутый модуль / класс дефектов.

### Вариант B — 🧪 Handler-test harness (W-70)
Решить [[integration-test-harness-blocker]] (lib.rs WASM-only) — покрыть authz/financial/fail-loud handler'ы end-to-end. Крупная структурная задача.

### Вариант C — 🚀/🛡️ Накопленный долг — нужен sign-off
Deploy #18-#20 в `main`, либо DB CHECK self-ref (W-68), либо PR ветки `fix/profile-routing` (#35-#42) в `main`. Ops/migration/review.

---

## 7. SKILL SAVED

Память: обновлён [[fail-loud-not-silent-clamp]] (добавлен fabricated-id кейс + REST «never return sentinel id»). Принцип: CREATE/action-эндпоинт никогда не возвращает sentinel-id (`0`) — это контракт, клиент по нему фетчит/обновляет; сбой чтения PK = server error (500), не `id:0`. И знай меру в фитнес-функциях: не добавляй шумную колонку (`id`) в guard.

**Anchor:** `phi^2 + phi^-2 = 3`
