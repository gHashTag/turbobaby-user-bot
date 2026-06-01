# Error-UX audit across UI endpoints (cycle #73)

Cycles #65 → #72 made `POST /api/orders` errors customer-friendly and
localised. Every other screen with an API call still renders raw error
strings. This is an inventory, not a fix — the recommendation is to
generalise `trios::checkout_errors::friendly_order_error` into a shared
`trios::api_errors` helper and migrate consumers.

## What's good

### `checkout_screen.rs` (cycles #65 / #69 / #70 / #71 / #72) — gold standard

* Per-status copy via `trios::checkout_errors::friendly_order_error`.
* Localised through `trios::i18n` (RU + EN, nearest-neighbor for long-tail
  Telegram codes via `normalize_lang_code`).
* Banner uses `ErrorBanner` component.

## What needs work

### `menu_screen.rs:317` (GET `/api/strains`)

```rust
Some(Err(e)) => {
    let err_msg = e.clone();
    // …
    p { …, "{err_msg}" }    // raw, no status discrimination, RU-only
}
```

**Fix shape:** the helper takes a `Result<_, String>` where `e` is whatever
`map_err(|e| format!("Network error: {}", e))` produced — no HTTP status
at hand. To get parity with checkout, the fetch path would need to
surface `(u16, body)` like `post_json_authed_idempotent_full` does, then
route through a shared `friendly_response_error` helper.

### `ar_hunt_screen.rs:74`, `location_quest_screen.rs:75`

Both follow the same shape — `err_msg` signal driven from `Err(e)` arm
of a fetch, rendered via `ErrorBanner`. RU-only, no status awareness.

### `tech_tree_screen.rs:233-236`

Best of the non-checkout lot — has *some* status discrimination:

```rust
Ok((401, _)) | Ok((403, _)) => err_msg.set("Только для админа".into()),
Ok((404, _)) => err_msg.set("Узел не найден".into()),
…
Err(e) => err_msg.set(format!("Ошибка: {}", e)),
```

Already uses the `(u16, body)` pattern. Missing:
* 422 / 429 specific copy.
* i18n — strings hardcoded RU.

### `admin_screen.rs:410-412`

```rust
format!("Ошибка загрузки: HTTP {}", status)
format!("Ошибка загрузки: HTTP {} — {}", status, trimmed)
```

Admin-only screen — leaks raw status by design (admins want the wire-
level detail). Localisation less critical here. Status quo acceptable.

## Recommended next step

1. Promote `friendly_order_error` to `trios::api_errors::friendly_response_error(lang, status)`.
2. Migrate menu/ar/location_quest/tech_tree to consume it where they
   already have a status, surface status where they don't (will require
   `fetch_text_full` returning `(u16, body)` for the bare GET paths).
3. Add 401 key (`T_API_ERR_401` → "Войдите в Telegram WebApp заново" /
   "Sign in to Telegram WebApp again"). 403/422/429/5xx keys reuse the
   existing T_CHECKOUT_ERR_* keys (rename to T_API_ERR_*).

Scope estimate: one cycle (~80 lines + 6 keys + tests).

Out of scope for cycle #73 — this audit is the deliverable.
