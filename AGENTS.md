# AGENTS.md — TurboBaby

Контекст для AI-агентов, работающих с этим проектом. Сохраняет "боль", чтобы не повторять одни и те же ошибки.

## Стек
- **Backend**: Axum + tokio-postgres + SQLx migrations
- **Frontend**: Dioxus 0.6 WASM, собирается Trunk
- **Deploy**: Railway, сервис `turbobaby-bot`, вручную через `railway up` из чистой LF-выгрузки коммита, не из рабочего дерева (см. §10–13 и `docs/ROLLBACK.md` §1, §3B)
- **Auth**: Telegram initData HMAC ИЛИ `X-Admin-Token` password fallback
- **Storage**: MinIO-бакет `media` (bucket-production-0ae7), отдаётся как `/media/`

---

## 🔥 Проблемы и решения (Lessons Learned)

### 1. CSP блокирует WASM — `EvalError: unsafe-eval`

**Симптом:** Чёрный/пустой экран, в консоли `Refused to evaluate a string as JavaScript`.

**Причина:** Dioxus 0.6 генерирует JS, который использует `new Function()` для динамического eval. CSP директива `'wasm-unsafe-eval'` разрешает eval **только внутри WASM**, но не `new Function()` в JS-шиме.

**Решение:** В `main.rs` добавить `'unsafe-eval'` в `script-src`:
```rust
"script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval';"
```

**Урок:** `'wasm-unsafe-eval' ≠ 'unsafe-eval'`. Для Dioxus нужны оба.

---

### 2. Admin auth fallback — initData есть, но user не в admin_ids

**Симптом:** Пользователь в Telegram WebApp, но видит "Доступ закрыт", хотя знает пароль.

**Причина:** `check_admin_access` сначала проверял initData, видел что user не в `admin_ids`, и **сразу возвращал** `is_admin: false`, не давая fallback на пароль.

**Решение:** Если initData валиден, но user не в `admin_ids`, **не возвращать** `false`, а "проваливаться" (fall through) к проверке `X-Admin-Token`:
```rust
if is_admin {
    return Ok(Json(json!({ "is_admin": true, "telegram_id": user.id })));
}
// FALL THROUGH to password check instead of returning false
```

**Урок:** Fallback должен быть единым — initData ИЛИ пароль. Нельзя возвращать отказ до проверки второго метода.

---

### 3. `X-Admin-Token` не передавался в PUT/POST запросах

**Симптом:** Сохранение в админке молча не работает, 401 Unauthorized.

**Причина:** После ввода пароля `admin_token()` читал `wwb_admin_token` из localStorage, но 31 запрос (PUT/POST/DELETE) в `admin_screen.rs` не добавляли `.header("X-Admin-Token", admin_token())`.

**Решение:** Добавить `X-Admin-Token` header ко **всем** защищённым запросам:
```rust
.header("X-Telegram-Init-Data", init_data.read().clone())
.header("X-Admin-Token", admin_token())
.header("X-Admin-Telegram-Id", telegram_id.to_string())
```

**Урок:** Если есть fallback auth, он должен быть везде. Проверяй ВСЕ HTTP-запросы в файле.

---

### 4. `on_saved` вызывался до ответа сервера

**Симптом:** Карточка редактирования закрывалась, но изменения не сохранялись.

**Причина:** `on_saved.call(())` вызывался сразу после `send().await`, не дожидаясь `status().is_success()`.

**Решение:** Переместить `on_saved` внутрь `if success`:
```rust
let success = match res { Ok(r) => r.status().is_success(), Err(_) => false };
if success {
    on_saved.call(());
    status.set("✅ Сохранено".into());
} else {
    status.set("❌ Не сохранено".into());
}
```

**Урок:** Никогда не закрывай UI до подтверждения от сервера. Оптимистичный UI — только для отображения, не для навигации.

---

### 5. Dioxus 0.6 Signal reactivity — child не может обновить parent Signal

**Симптом:** Поле ввода в компоненте-ребёнке не обновляет значение в родителе.

**Причина:** В Dioxus 0.6 `Signal`, переданный как prop в `#[component]`, нельзя надёжно обновлять из `spawn(async move)` внутри child.

**Решение:** Передавать `String` + `EventHandler<String>` вместо `Signal`:
```rust
#[component]
fn VideoUpload(video_url: String, on_change: EventHandler<String>) -> Element {
    // child вызывает on_change.call(new_url)
}
// parent хранит Signal и обновляет его в коллбеке
```

**Урок:** В Dioxus 0.6: родитель владеет Signal, ребёнок шлёт события. Не передавай `Signal` как prop для мутаций из async.

---

### 6. `use_telegram_id()` возвращает 0 вне Telegram WebApp

**Симптом:** В браузере (не WebApp) `tg: 0`, `ID: 0`, `check_admin` шлёт `telegram_id=0`.

**Причина:** `use_telegram_id()` читает `window.Telegram.WebApp.initDataUnsafe.user.id`. В обычном браузере Telegram WebApp не инициализирован — возвращает `None` → `unwrap_or(0)`.

**Решение:** Добавить fallback-хук `use_telegram_id_or_admin()`:
1. Сначала пытается `use_telegram_id()`
2. Если None — читает `wwb_admin_telegram_id` из `localStorage`
3. `AccessDeniedScreen` теперь спрашивает ID при логине по паролю
4. После успешного логина сохраняет ID в `localStorage`

**Урок:** Не предполагай, что Mini App всегда открыта в Telegram. Админка должна работать и в обычном браузере.

---

### 7. Кэширование WASM — обновления не применяются

**Симптом:** После деплоя новый код не работает, старые баги остаются.

**Причина:** Telegram Mini App (и браузер) агрессивно кэширует WASM/JS. Railway отдаёт `Cache-Control: no-store`, но это не помогает для assets с хэшем в имени, если HTML закэширован.

**Решение:** После каждого обновления WASM **полностью закрыть** Mini App (смахнуть из recent apps) и открыть заново. В браузере — Hard Refresh (Cmd+Shift+R).

**Урок:** Никогда не тестируй WASM-обновления через обычный F5. Только полный рестарт.

---

### 8. `js_escape` — компиляция падает на `s.replace('\', ...)`

**Симптом:** `cargo check` падает с ошибкой парсинга на строке `s.replace('\', ...)`.

**Причина:** Rust `char` литерал `\` — это escape для обратного слэша. Нельзя написать `\` в одинарных кавычках как один символ.

**Решение:**
```rust
// НЕПРАВИЛЬНО: s.replace('\', "\\") — не компилируется
// ПРАВИЛЬНО:
s.replace('\\', "\\\\")
```

**Урок:** В Rust char `\` пишется как `'\\'`. Всегда компилируй после правок в string-escape функциях.

---

### 9. Сборка фризится — `.cargo-lock` deadlock

**Симптом:** `cargo build` или `trunk build` висит бесконечно.

**Причина:** Процесс cargo держит `.cargo-lock` в target, следующий запуск блокируется.

**Решение:**
```bash
pkill cargo; rm -f target/.cargo-lock; rm -rf target/tmp
```

**Урок:** Если сборка зависла >2 минуты — убивай процессы и чисти lock.

---

### 10. «Git push = deploy» — НЕПРАВДА для этого репо

**Симптом:** мерж в main прошёл, а на проде изменений нет.

**Причина:** у воркфлоу `.github/workflows/deploy.yml` нет ни `RAILWAY_TOKEN`,
ни переменной `RAILWAY_SERVICE` — он честно скипается («loud-skip», замерено
2026-09-13). Репозиторий к сервису Railway не подключён: ни один из мержей
этой ночи (#39–#41) не задеплоился сам.

**Решение:** деплой вручную, из чистой LF-выгрузки ровно того коммита, который
выкатываешь, — НЕ `cd <репо> && railway up`: из рабочего дерева в образ уедет
всё незакоммиченное (включая `src/`), из worktree — прошлый исходник (§13).
Полная процедура с разбором — `docs/ROLLBACK.md` §3B; команды те же, одним
прогоном (Git Bash):
```
DEP=<новый пустой каталог вне всех репозиториев и worktree>
SHA=<полный 40-символьный коммит>
(
  set -euo pipefail
  : "${DEP:?set DEP}" "${SHA:?set SHA}"
  git clone -c core.autocrlf=false --no-checkout https://github.com/gHashTag/turbobaby-user-bot.git "$DEP"
  cd "$DEP"
  git checkout --detach "$SHA"
  test "$(git rev-parse HEAD)" = "$SHA"
  test -z "$(git status --porcelain)"
  test -z "$(git ls-files --eol | grep 'w/crlf' || true)"
  cat dist/version.txt                          # хеш бандла, закоммиченный на $SHA
  bash scripts/predeploy-smoke.sh --no-build    # ненулевой выход, если не SMOKE PASS
  test -z "$(git status --porcelain)"
  test ! -e dist/assets                         # dist/assets в выгрузке нет
  railway link -p woody -e production -s turbobaby-bot
  railway status
  read -r -p 'Does railway status name woody / production / turbobaby-bot? Type yes: ' answer
  test "$answer" = yes
  test "$(git rev-parse HEAD)" = "$SHA"
  test -z "$(git status --porcelain)"
  railway up --service turbobaby-bot --environment production
)
```
Прогон закрывается на отказ: первая упавшая команда внутри скобок обрывает
всё, что после неё. Незаданный `DEP` или `SHA`, сорвавшийся clone, CRLF,
упавший smoke, любой ответ кроме `yes` (и отсутствие терминала для ответа) до
`up` не доходят. Построчно так нельзя: при пустом `DEP` `git -C "$DEP"` бьёт в
текущий каталог, `cd "$DEP"` падает и оставляет тебя на месте, `railway link`
перепривязывает твоё собственное дерево, `railway status` по-прежнему
показывает woody / production / turbobaby-bot — и `up` выгружает это дерево с
незакоммиченным. link + status доказывают СЕРВИС, а не каталог; каталог
доказывает проверка HEAD и чистоты прямо перед `up`, в том же прогоне.
Проверено 2026-09-25 на локальной подмене репозитория, `railway` и smoke —
заглушки: из 14 прогонов до `up` дошёл только тот, где все проверки прошли и
ответ был `yes` (`docs/ROLLBACK.md` §3B).

Никогда `-y` и никогда `--new`: по справке CLI 5.62.1 (`railway up --help`,
2026-09-25) `up` без входа сам входит и создаёт НОВЫЙ проект с сервисом и
деплоит в него; в каталоге без линка `-y` делает то же (`--new` «implied … for
`-y` when nothing is linked»), а `--new` создаёт новый проект «even if one is
already linked». Под агентской обвязкой одного отказа от `-y` мало: 5.62.1 сам
пропускает подтверждение входа («Agent harness detected … (skipping confirm)»
в строках бинаря рядом с этим подтверждением; в том же бинаре список имён
переменных с `CLAUDECODE`, а шелл Claude Code задаёт `CLAUDECODE` и
`CLAUDE_CODE_ENTRYPOINT`; прочитано по строкам бинаря 2026-09-25, не запуском).
Поэтому вход — отдельным шагом `railway login`, а `up` — только после
`railway status` в каталоге выгрузки, назвавшего woody / production /
turbobaby-bot. Линк хранится ПО КАТАЛОГУ: link и status — в каталоге
выгрузки, прямо перед `up`. Потом ждать SUCCESS и пройти «Чеклист после
деплоя».

**Урок:** push в main — это только код. Прод обновляется ровно одним способом:
`railway up`. Если когда-нибудь захочется автоматики — положить `RAILWAY_TOKEN`
и `RAILWAY_SERVICE` в секреты GitHub, тогда deploy.yml оживет как есть.

---

### 12. Railway vars: `--service` на `--set` НЕ работает — только link→set

**Симптом:** `railway variables --set X=1 --service woody-weed-bot` — переменная
оказалась на ДРУГОМ сервисе (залинкованном), с него и задеплоилась.

**Причина:** CLI применяет `--set` к линку из локального конфига, флаг сервиса
на запись не влияет (замерено 2026-09-13: так лечился турбобот чужим токеном
50 минут и отравилась чужая база).

**Решение (детерминированный порядок):**
```
railway link -p woody -e production -s <нужный сервис>
railway variables --set 'KEY=value'        # БЕЗ --service
railway variables --kv | grep -E 'KEY|DATABASE_URL'   # проверить
```

**Урок:** любая запись в Railway начинается с link и заканчивается проверкой.
Смена var сама триггерит автодеплой последнего загруженного исходника —
сначала грузи правильный код, потом меняй vars.

---

### 13. `railway up` из git-worktree молча деплоит ПРОШЛЫЙ исходник

**Симптом:** up из worktree-каталога «прошёл успешно», но на сервисе живёт
старый код (проверяется эндпоинтом, которого в исходнике нет).

**Решение:** деплой из чистого каталога: `git clone -c core.autocrlf=false` на
точный SHA одним прогоном из §10 (`docs/ROLLBACK.md` §3B). Выгрузка
`git -c core.autocrlf=false archive <sha> | tar -x -C <пустой каталог>` тоже
чистая, но без `.git`: ни проверку HEAD и чистоты перед `up`, ни проверку 5
post-deploy smoke на ней не сделать — поэтому clone.
Флаг обязателен: на Windows `core.autocrlf=true` стоит системно, `.gitattributes`
нет, и без флага выгрузка получает CRLF — отдаваемая страница уже не равна
коммиту (`docs/ROLLBACK.md` §1).

**Урок:** после КАЖДОГО up проверять, что доехало именно то, что грузили
(специфичный эндпоинт/версия/число миграций), а не только статус SUCCESS.

---

### 11. Пустой/чёрный экран в Mini App — редирект `?v=4` ломал первый рендер

**Симптом:** Белый/пустой экран в Telegram (чаще macOS Telegram / iOS WKWebView). Диагностика: `Blank screen detected · no [WASM] steps logged · Steps reached: none`, при этом `Telegram.WebApp` и `initData` присутствуют. Не выполнился даже Step 1 в `lib.rs::run()`.

**Причина:** SPA-handler в `main.rs` на первый запрос без `v=4` отдавал `303 Redirect` на `<path>?v=4` (хак кэш-бастинга). Внутри WebView редирект теряет URL-фрагмент `#tgWebAppData=…` и срывает начальный кадр — итоговый документ загружается так, что WASM вообще не стартует.

**Решение:** SPA-handler теперь отдаёт `index.html` НАПРЯМУЮ (без редиректа) с `Cache-Control: no-store`. Кэш-бастинг уже обеспечен хешами в именах ассетов (Trunk) + `no-store` на HTML.

**Урок:** НИКОГДА не делай HTTP-редирект на начальной загрузке Mini App — редирект теряет `#tgWebAppData`. Кэш бьётся хешами файлов и `no-store`, а не редиректом.

---

## Чеклист перед деплоем

- [ ] `cargo check --features backend` проходит
- [ ] `cargo check --target wasm32-unknown-unknown` проходит
- [ ] `dist/` собран `scripts/build-frontend.sh` и закоммичен. Голый `trunk build --release` для `dist/` не годится: он не делает постобработку `build-frontend.sh` (`?v=` у импортов snippets, `await window.__telegramReady`, обёртка `__loadWasmWithRetry`, запись `dist/version.txt`, сжатые `.br`/`.gz`)
- [ ] Выгрузка по §10: чистая, LF, точный SHA, `dist/assets` нет
- [ ] На выгрузке `scripts/predeploy-smoke.sh --no-build` → SMOKE PASS, после него `git -C <выгрузка> status --porcelain` пуст. Без `--no-build` скрипт пересобирает `dist/` голым `trunk build --release` и проверяет НЕ закоммиченный бандл (а выгрузка перестаёт быть коммитом). Не смог запуститься (нет модуля Python `websocket-client`, `scripts/cdp_smoke.py`) — это FAIL, не пропуск
- [ ] Деплой впервые накатывает 087 (на проде `4a5aa72` или старше): до деплоя снять строки `delivery_zones` и прогон `scripts/postdeploy-smoke.sh` с `SMOKE_DIST_REF=<живой sha>` в `SMOKE_OUT` вне выгрузки — без этого аварийный ремонт не из чего собрать. Откат после 087 — только вперёд либо `4a5aa72` + SQL-ремонт владельца (`docs/ROLLBACK.md` §4 «Rolling back after 087»); окно 500 на зонах у старого контейнера во время деплоя ожидаемо (`docs/ROLLBACK.md` §2)
- [ ] Все HTTP-запросы в `admin_screen.rs` имеют `.header("X-Admin-Token", admin_token())`
- [ ] CSP содержит `'unsafe-eval'`
- [ ] `check_admin_access` не возвращает `false` раньше fallback

## Чеклист после деплоя (на turbobaby-bot)

- [ ] `railway deployment list` — последний SUCCESS
- [ ] `SMOKE_DIST_REF=<выкаченный sha> SMOKE_OUT=<новый каталог вне выгрузки> scripts/postdeploy-smoke.sh` → `0 failed` (только GET). Запускать из клона, где есть этот коммит: проверка 5 сравнивает `/` с `dist/index.html` коммита по git-истории. `SMOKE_OUT` вне выгрузки, чтобы она осталась чистой, и новый на каждый прогон — файлы перезаписываются
- [ ] `curl …/health` — 200
- [ ] `curl …/api/bikes` — JSON, фото 13/13
- [ ] `railway logs -s turbobaby-bot` — 0 `InvalidToken`, нет `SCHEMA SELF-CHECK`
- [ ] НИКАКИХ команд в сторону `woody-weed-bot` — чужой сервис (см. топологию в loop/LOOP_STATE.md)
- [ ] `on_saved` вызывается только внутри `if success`
- [ ] `use_telegram_id_or_admin` используется вместо `use_telegram_id` в админке
- [ ] После SUCCESS (не после push — push не деплоит, §10): сообщить пользователю про полный рестарт Mini App
- [ ] SPA-маршруты отдают `index.html` напрямую (без `303`-редиректа на `?v=4`)

## Own language first

When this project publishes something about itself, it publishes in **this
project's own language and format** -- not translated into somebody else's.

Owner's rule, 2026-09-20: stop writing in other people's languages, we have our
own.

This bites on any file whose only reason to exist is that an outside tool
expects that shape: `llms.txt`, `agents.json`, `ai.txt`, `.well-known/*.json`,
A2A agent cards, `ai-plugin` manifests, OpenAPI stubs, JSON-LD blocks, a README
that restates a spec. The reflex is to write four of them in four foreign
formats, and the reflex is wrong: a project whose claim is "here is a language
worth writing" and which then describes itself in three of other people's
formats has published three documents that are not true of it.

**The move:** find the address the outside world already fetches, then serve our
own language at it. `/llms.txt` at t27.ai **is** a t27 module -- `llms.txt`
requires nothing but text, and every prose line of a `.t27` file is a `;`
comment, so it stays readable to anything that cannot compile it.

**Three qualifications, so the rule stays honest:**

- A format a resolver genuinely parses -- a sitemap, `package.json`, a lockfile
  -- is machinery, not a description. **Generate** it from our own source; never
  hand-write it into a second home for the truth.
- Code against someone else's API uses their types. Prose for a human who has
  never heard of the project uses that human's language.
- If a format demands a claim we cannot back, **publish nothing**. An A2A card
  with no A2A server behind it is a false claim, and a missing file is more
  honest than a lying one.

The test: *is this file the project speaking about itself?* If yes, it speaks
our language. If it is plumbing, it speaks the plumbing's.

**Worked example, compiler-checked rather than asserted:** in `gHashTag/trinity`,
`apps/website/public/t27/files/specs/catalog/onboarding.t27` generates
`/llms.txt` and `/agents.t27` byte-identically, gated in CI as
`check:onboarding`. The generator evaluates the spec's own `test` blocks --
`typecheck.ok` stays true for `assert 1 > 2`, so a compiler saying "this parses"
is not a compiler saying "this is true" -- and re-compiles the rendered document
before writing it.

**The full rule lives in exactly one place: the `own-language-first` skill**
(`~/.claude/skills/own-language-first/SKILL.md`). It carries the consent gate for
documents addressed to other people's agents, the six negative controls, and the
`;`-alone-on-a-line trap that silently discards a `module` declaration. This
section is a pointer, not a copy -- the recorded defect in this codebase family
is the hand-copied rule that only two of its three homes knew about.
