# AGENTS.md — Woody Weed Bot

Контекст для AI-агентов, работающих с этим проектом. Сохраняет "боль", чтобы не повторять одни и те же ошибки.

## Стек
- **Backend**: Axum + tokio-postgres + SQLx migrations
- **Frontend**: Dioxus 0.6 WASM, собирается Trunk
- **Deploy**: Railway (авто-деплой по push в main), Dockerfile multi-stage
- **Auth**: Telegram initData HMAC ИЛИ `X-Admin-Token` password fallback
- **Storage**: `/data/uploads/` (Railway volume), отдаётся как `/uploads/`

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

### 10. Railway CLI недоступен — deploy только через GitHub

**Симптом:** `railway up` таймаутится на `backboard.railway.com`.

**Причина:** Railway CLI заблокирован в текущей сети.

**Решение:** Push в main → Railway автодеплой. Никакой CLI не нужен.

**Урок:** Не трать время на CLI. Git push = deploy.

---

## Чеклист перед деплоем

- [ ] `cargo check --features backend` проходит
- [ ] `cargo check --target wasm32-unknown-unknown` проходит
- [ ] `trunk build --release` проходит
- [ ] Все HTTP-запросы в `admin_screen.rs` имеют `.header("X-Admin-Token", admin_token())`
- [ ] CSP содержит `'unsafe-eval'`
- [ ] `check_admin_access` не возвращает `false` раньше fallback
- [ ] `on_saved` вызывается только внутри `if success`
- [ ] `use_telegram_id_or_admin` используется вместо `use_telegram_id` в админке
- [ ] После push: сообщить пользователю про полный рестарт Mini App
