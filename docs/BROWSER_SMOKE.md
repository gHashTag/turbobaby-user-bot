# Browser smoke test

Runtime check that the WASM bundle in `dist/` actually mounts. It catches a
broken WASM build, missing asset, module-load exception, and regressions in the
WebKit network-retry wrapper. It does **not** test API calls because no backend
is started.

The check is automated over the Chrome DevTools Protocol and fails unless the
Rust-rendered `#turbobaby-app-mounted` sentinel exists.

## Prerequisites

```sh
# Build the release bundle and its compressed sidecars:
./scripts/build-frontend.sh

# `dist/` should now contain index.html, one hashed
# turbobaby-bot-*.{js,wasm} pair, and CSS.
```

The runner auto-detects Google Chrome, BrowserOS, Chromium, or Microsoft Edge.
Set `CHROME_BIN` only when the browser executable lives somewhere else.

## Running

```sh
./scripts/predeploy-smoke.sh --no-build
```

Expected output: `SMOKE OK`, the TurboBaby mount selector present, no uncaught
JavaScript errors, and a two-call/three-byte pass from the synthetic retry
probe. API 404s are expected on the static server and are reported as notes.

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

If the app fails to mount or the console has errors:

1. **WASM module fails to load** — check `dist/turbobaby-bot-*.wasm`
   exists and is the latest build. Re-run `./scripts/build-frontend.sh`.
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
