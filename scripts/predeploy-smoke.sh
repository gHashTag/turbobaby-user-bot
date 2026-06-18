#!/usr/bin/env bash
#
# MANDATORY pre-deploy frontend smoke test.
#
# A green `cargo`/`trunk` build does NOT prove the frontend runs: wasm-bindgen
# glue + JS snippets are linked at MODULE LOAD, after every Rust check passes.
# On 2026-06-18 two deploys shipped a bundle that threw
# "Importing binding name 'get_select_data' is not found" and failed to mount —
# every compile check was green. This gate catches that class of error by
# actually loading the built dist/ in headless Chrome and failing on any console
# error or an unmounted app.
#
# Run BEFORE committing dist/ and deploying any `src/ui/` change:
#     ./scripts/predeploy-smoke.sh
# Exit code 0 = safe to ship. Non-zero = DO NOT deploy.
#
set -euo pipefail
cd "$(dirname "$0")/.."

# --no-build: skip the version check + trunk build and smoke-test the dist/ that
# is ALREADY on disk (i.e. what is about to be pushed). Used by the pre-push hook
# to validate the committed bundle fast; the full run (default) rebuilds first.
NO_BUILD=0
[ "${1:-}" = "--no-build" ] && NO_BUILD=1

PORT="${SMOKE_PORT:-$((8000 + RANDOM % 1500))}"
MARKER="${SMOKE_MARKER:-WOODY}"

bold() { printf '\033[1m%s\033[0m\n' "$1"; }

if [ "${NO_BUILD}" != "1" ]; then
  # ── 1. wasm-bindgen CLI must match the crate (root cause of the 2026-06-18 bug)
  bold "▶ 1/4  wasm-bindgen version match"
  crate_ver="$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock \
    | grep -m1 'version' | sed -E 's/.*"([0-9.]+)".*/\1/')"
  cli_ver="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')"
  if [ -z "${cli_ver}" ]; then
    echo "  ✖ wasm-bindgen CLI not found in PATH"; exit 1
  fi
  if [ "${crate_ver}" != "${cli_ver}" ]; then
    echo "  ✖ MISMATCH: crate=${crate_ver}  CLI=${cli_ver}"
    echo "    fix:  cargo install wasm-bindgen-cli --version ${crate_ver} --locked"
    exit 1
  fi
  echo "  ✓ crate == CLI == ${cli_ver}"

  # ── 2. build the release bundle
  bold "▶ 2/4  trunk build --release"
  if ! trunk build --release >/tmp/wwb-smoke-trunk.log 2>&1; then
    echo "  ✖ trunk build failed:"; tail -25 /tmp/wwb-smoke-trunk.log; exit 1
  fi
  echo "  ✓ built dist/"
else
  bold "▶ smoke (--no-build): validating the dist/ already on disk"
fi

# ── 3. serve dist/ statically (the wasm load error is independent of the API)
bold "▶ 3/4  serve dist/ on :${PORT}"
( cd dist && exec python3 -m http.server "${PORT}" ) >/tmp/wwb-smoke-serve.log 2>&1 &
SERVE_PID=$!
cleanup() { kill "${SERVE_PID}" 2>/dev/null || true; }
trap cleanup EXIT
sleep 1.5
if ! curl -fs -o /dev/null "http://127.0.0.1:${PORT}/"; then
  echo "  ✖ static server did not come up"; exit 1
fi
echo "  ✓ serving"

# ── 4. load it in headless Chrome; fail on any console error / unmounted app
bold "▶ 4/4  headless browser smoke check"
if python3 scripts/cdp_smoke.py "http://127.0.0.1:${PORT}/" "${MARKER}"; then
  bold "✅ SMOKE PASS — frontend mounts, console clean. Safe to commit dist/ + deploy."
  exit 0
else
  bold "❌ SMOKE FAIL — DO NOT deploy. Fix the above before committing dist/."
  exit 1
fi
