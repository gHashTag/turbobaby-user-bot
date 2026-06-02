# Browser smoke test

Visual sanity check that the WASM bundle in `dist/` actually mounts
and renders the home screen. Catches: broken WASM build, missing
asset, panic on init, font/CSS load failure. Does **not** test API
calls (no backend started).

This is the cycle #114 deliverable — first browser-side smoke after
~13 cycles of backend-only work. Procedure is short enough to run
locally; promote to an automated CI step (Playwright / Puppeteer) if
visual drift becomes a recurring concern.

## Prerequisites

```sh
# Build the WASM bundle (release for accurate timing):
cargo build --target wasm32-unknown-unknown --release

# If you've been running `cargo build` without trunk, `dist/` may
# already be the last successful release output — `ls dist/` should
# show index.html + woody-weed-bot-*.{js,wasm} + css.
```

## Running

```sh
cd dist
python3 -m http.server 18080
# In another tab / browser:
open http://localhost:18080/
```

Expected output (golden path, cycle #114 baseline):

  * Logo + "WOODY WEEDPECKER" title + Russian subtitle render
  * 6 category cards with emojis (Меню, Наборы, Sommelier, Аксессуары,
    Чай, Сад)
  * "Strain of the Day" panel shows `Loading...` (no backend)
  * "ADVENTURES" section with two adventure cards
  * Bottom nav bar: 9 items (Home / Menu / Sets / Gear / Tea / Garden
    / Quest / Cart / Profile)
  * Console: zero errors, zero warnings

## Known limitations

* **`python -m http.server` is not SPA-aware.** Direct navigation to
  `http://localhost:18080/cart` returns a 404 because there's no
  `cart` file on disk — the real backend's `spa_handler` (`src/main.rs`)
  serves `index.html` for unknown routes. To exercise routes locally,
  either: (a) navigate via in-app links from the home page, or
  (b) use a SPA-aware static server e.g.
  `npx serve dist -s` or `python -c "import http.server; ..."` with
  fallback logic.
* **Telegram WebApp APIs are unavailable** when loaded outside the
  Telegram client. The app degrades gracefully — `getUserId()` returns
  `None`, `initData` is empty — but anything calling those (lang
  picker, auth-bearing fetches) will skip its branch. Don't expect
  `?lang=` query overrides or admin auth to work in this mode.

## When this test catches something

If the home screen fails to render and console has errors:

1. **WASM module fails to load** — check `dist/woody-weed-bot-*.wasm`
   exists and is the latest build. Re-run `cargo build --target
   wasm32-unknown-unknown --release`.
2. **Panic on init** — open DevTools console; the `panic_hook` in
   `src/lib.rs:14-32` injects a red overlay with the panic message
   into the page. Stack trace lives there too.
3. **CSS missing** — check `dist/*.css` exists. If not, the trunk
   build path may have changed; check `index.html` for the actual
   stylesheet filenames.

## When to re-run

Recommended:
* After any change to `src/lib.rs` (panic hook, lang init, app launch).
* After bumping Dioxus or `wasm-bindgen` major versions.
* Before cutting a release.

Not necessary for backend-only changes (`src/api/*.rs`, `src/db/*.rs`).
