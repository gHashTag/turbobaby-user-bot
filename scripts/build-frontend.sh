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

echo "✅ frontend built. Next: ./scripts/predeploy-smoke.sh --no-build  (then commit dist/ + deploy)"
