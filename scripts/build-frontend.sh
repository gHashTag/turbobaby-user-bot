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

# In-place `sed -E` that works with both BSD sed (macOS: `-i ''`) and GNU sed
# (Linux, Git Bash on Windows: `-i` with no argument). The BSD-only spelling this
# script used before 2026-09-24 made GNU sed read '' as the script and the
# expression as a file name, so the cache-bust step failed off macOS.
sed_in_place() {
  local expr="$1" file="$2"
  sed -E "$expr" "$file" > "$file.sedtmp" && mv "$file.sedtmp" "$file"
}

# clap-backed Trunk versions expect a boolean value, while many CI/agent
# environments use the conventional NO_COLOR=1 spelling.
if [ "${NO_COLOR:-}" = "1" ]; then
  export NO_COLOR=true
fi

echo "▶ trunk build --release"
trunk build --release

main_js=""
hash=""
while IFS= read -r candidate; do
  candidate_name="$(basename "$candidate")"
  if [[ "$candidate_name" =~ ^.+-([a-f0-9]{16,})\.js$ ]]; then
    if [ -n "$main_js" ]; then
      echo "✖ multiple top-level Trunk bundles found: ${main_js}, ${candidate}" >&2
      exit 1
    fi
    main_js="$candidate"
    hash="${BASH_REMATCH[1]}"
  fi
done < <(find dist -maxdepth 1 -type f -name '*.js' -print)

if [ -z "$main_js" ] || [ -z "$hash" ]; then
  echo "✖ could not find one <target>-<hex hash>.js bundle in dist/" >&2
  exit 1
fi
if [ ! -f "${main_js%.js}_bg.wasm" ]; then
  echo "✖ matching WASM bundle is missing for ${main_js}" >&2
  exit 1
fi

echo "▶ cache-busting snippet imports with ?v=${hash}"
# Rewrite in every emitted JS (main bundle + snippets that import siblings).
# Only touch import specifiers that don't already carry a query.
busted=0
while IFS= read -r js; do
  if grep -qE "from '[^']*snippets/[^']*\.js'" "$js"; then
    sed_in_place "s#(from '[^']*snippets/[^']*\.js)'#\1?v=${hash}'#g" "$js"
    busted=$((busted + 1))
  fi
done < <(find dist -name '*.js' -type f)
echo "  ✓ busted snippet imports in ${busted} file(s)"

# Main JS/WASM names are already content-hashed. Add the same hash as a query
# too, so even a WebView with a broken filename cache sees a new full URL on
# every build. This makes long-lived immutable caching safe.
echo "▶ cache-busting main bundle URLs with ?v=${hash}"
sed_in_place "s#(/[^/\"'[:space:]?]+-${hash}(_bg)?\.(js|wasm))(['\"])#\1?v=${hash}\4#g" dist/index.html
echo "  ✓ main JS/WASM URLs versioned"

# Trunk owns the generated module script. Make it wait for the async Telegram
# SDK promise declared in index.html: the loader can paint immediately, while
# the Rust app still starts with initData available. `onerror` resolves the
# promise so ordinary browsers are not held hostage by a blocked Telegram CDN.
echo "▶ preserving Telegram SDK-before-WASM ordering"
perl -0pi -e 's/const wasm = await init/await window.__telegramReady;\nconst wasm = await init/' dist/index.html
if ! grep -q 'await window.__telegramReady;' dist/index.html; then
  echo "✖ could not inject Telegram readiness wait into dist/index.html"; exit 1
fi
echo "  ✓ WASM waits asynchronously for Telegram SDK"

# Trunk emits a top-level `await init(...)`. Safari turns any interrupted WASM
# transfer into an unhandled `TypeError: Load failed`, so route init through the
# bounded retry helper defined in index.html.
echo "▶ wrapping WASM fetch with WebKit network retries"
perl -0pi -e 's#const wasm = await init\(\{ module_or_path: (.*?) \}\);#const wasm = await window.__loadWasmWithRetry(init, $1);#s' dist/index.html
if ! grep -q 'const wasm = await window.__loadWasmWithRetry' dist/index.html; then
  echo "✖ could not wrap WASM init in dist/index.html"; exit 1
fi
echo "  ✓ WASM fetch retries enabled"

# Cycle #170: write a live build-version token for the WebView cache-bust loader.
# The loader in index.html fetches /version.txt and redirects to ?v=<hash> when
# the embedded JS hash no longer matches the server's current build.
echo "${hash}" > "dist/version.txt"
echo "  ✓ wrote dist/version.txt (${hash})"

# Pre-compress static assets so the server can serve brotli/gzip sidecars without
# runtime CPU cost.
#
# Only what compression actually helps. The comment above this block used to
# claim it skipped files that would not shrink; it did not, and once the shop
# had videos uploaded the build stalled for hours on `brotli -q 11` over them.
# Measured on this repo's own dist/:
#
#   main.css        27954 ->  5496   (5.1x)
#   the wasm bundle  4.7M ->  733K   (6.4x)
#   video-...936412157.mp4  3213125 -> 3213137   (12 bytes LARGER)
#   video-...918953840.mp4  2377800 -> 2377809   (9 bytes larger)
#
# That is not a tuning question. MP4, JPEG, PNG, WebP and WOFF2 are already
# entropy-coded, so a second pass has nothing left to find — and a sidecar
# bigger than its original means the server sends MORE bytes to a client that
# asked for compression.
SKIP_COMPRESSION_RE='\.(mp4|webm|mov|m4v|avi|jpe?g|png|gif|webp|avif|ico|woff2?|mp3|ogg|opus|zip|gz|br|pdf)$'

echo "▶ pre-compressing static assets"
(
  cd dist
  find . -type f -not -name "*.br" -not -name "*.gz" -print0 | while IFS= read -r -d '' f; do
    base="${f#./}"
    if printf '%s' "$base" | grep -qiE "$SKIP_COMPRESSION_RE"; then
      continue
    fi

    if command -v brotli >/dev/null 2>&1 && [ ! -f "${base}.br" ]; then
      brotli -q 11 -o "${base}.br" "$base" 2>/dev/null || true
      # Belt and braces: the extension list is a prediction, this is a
      # measurement. A sidecar that did not shrink is deleted rather than
      # shipped, so no future file type can quietly cost bytes.
      if [ -f "${base}.br" ] && [ ! "${base}.br" -ot "$base" ] &&
         [ "$(wc -c <"${base}.br")" -ge "$(wc -c <"$base")" ]; then
        rm -f "${base}.br"
      fi
    fi
    if [ ! -f "${base}.gz" ]; then
      if gzip -k -9 "$base" 2>/dev/null; then :; else
        gzip -9 -c "$base" > "${base}.gz"
      fi
      if [ -f "${base}.gz" ] && [ "$(wc -c <"${base}.gz")" -ge "$(wc -c <"$base")" ]; then
        rm -f "${base}.gz"
      fi
    fi
  done
)
echo "  ✓ pre-compressed dist/ sidecars ready (media skipped — see the note above)"

echo "✅ frontend built. Next: ./scripts/predeploy-smoke.sh --no-build  (then commit dist/ + deploy)"
