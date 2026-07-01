#!/usr/bin/env bash
#
# Build the WASM frontend for deploy: trunk build + snippet cache-bust.
#
# Why the cache-bust: trunk content-hashes the main JS + wasm (safe to cache
# forever) but emits the wasm-bindgen JS snippets under STABLE names
# (snippets/dioxus-web-<crate-hash>/inline0.js, inline1.js, ...). When those
# files' CONTENT changes between deploys (e.g. a wasm-bindgen version bump),
# returning clients — especially the Telegram in-app WebView, which ignores
# no-store — keep serving the OLD snippet against the NEW main JS and crash with
# "Importing binding name 'get_select_data' is not found" (2026-06-18 incident,
# recurring for cached users).
#
# Fix: append ?v=<main-bundle-hash> to every ./snippets/*.js import. The hash
# changes every build, so the snippet URL changes every deploy → the WebView
# cannot reuse a stale copy. The static server / ServeDir ignores the query and
# serves the current file.
#
set -euo pipefail
cd "$(dirname "$0")/.."

echo "▶ trunk build --release"
trunk build --release

main_js="$(ls dist/woody-weed-bot-*.js | grep -v '_bg' | head -1)"
hash="$(basename "$main_js" | sed -E 's/^woody-weed-bot-([a-f0-9]+)\.js$/\1/')"
if [ -z "${hash}" ]; then
  echo "✖ could not derive bundle hash from ${main_js}"; exit 1
fi

echo "▶ cache-busting snippet imports with ?v=${hash}"
# Rewrite in every emitted JS (main bundle + snippets that import siblings).
# Only touch import specifiers that don't already carry a query.
busted=0
while IFS= read -r js; do
  if grep -qE "from '[^']*snippets/[^']*\.js'" "$js"; then
    sed -i '' -E "s#(from '[^']*snippets/[^']*\.js)'#\1?v=${hash}'#g" "$js"
    busted=$((busted + 1))
  fi
done < <(find dist -name '*.js' -type f)
echo "  ✓ busted snippet imports in ${busted} file(s)"

# Cycle #170: write a live build-version token for the WebView cache-bust loader.
# The loader in index.html fetches /version.txt and redirects to ?v=<hash> when
# the embedded JS hash no longer matches the server's current build.
echo "${hash}" > "dist/version.txt"
echo "  ✓ wrote dist/version.txt (${hash})"

# Pre-compress static assets so the server can serve brotli/gzip sidecars without
# runtime CPU cost. Skip files already smaller than the compressed form would be.
echo "▶ pre-compressing static assets"
(
  cd dist
  compressed=0
  find . -type f -not -name "*.br" -not -name "*.gz" -print0 | while IFS= read -r -d '' f; do
    base="${f#./}"
    if command -v brotli >/dev/null 2>&1 && [ ! -f "${base}.br" ]; then
      brotli -q 11 -o "${base}.br" "$base" 2>/dev/null || true
    fi
    if [ ! -f "${base}.gz" ]; then
      if gzip -k -9 "$base" 2>/dev/null; then :; else
        gzip -9 -c "$base" > "${base}.gz"
      fi
    fi
  done
)
echo "  ✓ pre-compressed dist/ sidecars ready"

echo "✅ frontend built. Next: ./scripts/predeploy-smoke.sh --no-build  (then commit dist/ + deploy)"
