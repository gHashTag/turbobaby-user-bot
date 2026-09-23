#!/usr/bin/env bash
#
# Post-deploy smoke for TurboBaby's production service.
#
# Read-only by construction: every request below is a plain GET with no request
# body, so it is safe to run against the live shop at any moment and as often as
# needed. tests/postdeploy_smoke_wiring.rs keeps it that way: it fails if a write
# verb or a body-sending curl flag ever appears in this file.
#
# Usage:
#     scripts/postdeploy-smoke.sh [BASE_URL]
#
# BASE_URL is the first argument, else $E2E_BASE_URL, else TurboBaby's production
# origin -- the default e2e/playwright.config.ts uses and the canonical
# WEB_APP_URL in src/config.rs. Never point it at woody-weed-bot: that is another
# shop's live service in the same Railway project (AGENTS.md, loop/LOOP_STATE.md).
#
# Optional environment:
#   SMOKE_DIST_REF   git ref whose committed dist/index.html the served "/" must
#                    equal byte for byte (default HEAD). After a rollback, set it
#                    to the commit you rolled back to (docs/ROLLBACK.md).
#   SMOKE_OUT        directory the fetched bodies are written to, so a mismatch
#                    can be diffed afterwards. Files are overwritten on every run
#                    and never removed (default ${TMPDIR:-/tmp}/turbobaby-postdeploy-smoke).
#   SMOKE_TIMEOUT    per-request timeout in seconds (default 30).
#
# Checks. Each prints one PASS or FAIL line; NOTE lines are information only.
#   1. /health              200 and "db":"ok"       src/api/mod.rs health_handler
#   2. /api/bikes           200 and {"bikes":[..]}  src/api/bikes.rs list_bikes
#                           The count is printed, not judged: it is compared, as
#                           a NOTE, with totals.families_offered in
#                           data/fleet_seed.json, which is a dated measurement of
#                           the fleet and not a contract on the live database.
#   3. /api/delivery/zones  200 and {"zones":[..]}  src/api/orders.rs list_delivery_zones
#   4. an unmatched /api    404 with a JSON body    src/api/mod.rs api_not_found
#      path                 (not the SPA's HTML, which it answered before that
#                           fallback existed)
#   5. /                    byte-equal to dist/index.html at SMOKE_DIST_REF
#                           (src/main.rs spa_handler serves the dist/index.html
#                           it loaded at boot). Prints both sha256 digests, both
#                           bundle hashes and /version.txt; on a mismatch, names
#                           the commit whose dist/index.html IS being served.
#
# Exit: 0 every check passed, 1 at least one check failed, 2 could not run.

set -uo pipefail

BASE="${1:-${E2E_BASE_URL:-https://turbobaby-bot-production.up.railway.app}}"
BASE="${BASE%/}"
REPO="$(cd "$(dirname "$0")/.." && pwd)"
DIST_REF="${SMOKE_DIST_REF:-HEAD}"
OUT_DIR="${SMOKE_OUT:-${TMPDIR:-/tmp}/turbobaby-postdeploy-smoke}"
TIMEOUT="${SMOKE_TIMEOUT:-30}"
# A path no route will ever claim; only the /api fallback can answer it.
UNMATCHED_PATH="/api/postdeploy-smoke-unmatched-route"

case "$BASE" in
  http://* | https://*) ;;
  *)
    echo "usage: $0 [BASE_URL]   ('$BASE' is not an http(s) URL)" >&2
    exit 2
    ;;
esac

PY=""
for candidate in python3 python; do
  if "$candidate" -c 'import json, hashlib, re' >/dev/null 2>&1; then
    PY="$candidate"
    break
  fi
done
if [ -z "$PY" ]; then
  echo "cannot run: no working python3/python (used to read JSON)" >&2
  exit 2
fi
for tool in curl cmp git; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "cannot run: $tool not found" >&2
    exit 2
  fi
done
if ! mkdir -p "$OUT_DIR"; then
  echo "cannot run: cannot create $OUT_DIR" >&2
  exit 2
fi

PASSED=0
FAILED=0
pass() { PASSED=$((PASSED + 1)); printf 'PASS  %-21s %s\n' "$1" "$2"; }
fail() { FAILED=$((FAILED + 1)); printf 'FAIL  %-21s %s\n' "$1" "$2"; }
note() { printf 'NOTE  %-21s %s\n' "$1" "$2"; }
detail() { printf '      %s\n' "$1"; }

# fetch NAME PATH -- one GET of BASE+PATH, no request body. The response body
# lands in $OUT_DIR/NAME. Sets CODE (000 when no HTTP answer arrived), CTYPE and,
# on a transport failure, ERR.
fetch() {
  local name="$1" path="$2" meta
  CODE="000"
  CTYPE=""
  ERR=""
  if meta="$(curl -sS --max-time "$TIMEOUT" -o "$OUT_DIR/$name" \
    -w '%{http_code} %{content_type}' "$BASE$path" 2>"$OUT_DIR/$name.stderr")"; then
    CODE="${meta%% *}"
    CTYPE="${meta#* }"
    return 0
  fi
  ERR="$(head -c 300 "$OUT_DIR/$name.stderr" | tr '\n' ' ')"
  return 1
}

# probe MODE FILE [ARG] -- reads one fetched body and prints a one-line verdict.
probe() {
  "$PY" - "$@" <<'PYEOF'
import hashlib, json, re, sys

mode, path = sys.argv[1], sys.argv[2]
arg = sys.argv[3] if len(sys.argv) > 3 else ""

def bundle_hash(text):
    # The same reading as src/main.rs extract_trunk_bundle_hash: the first
    # token ending in ".js" whose part after the last '-' is 16+ hex digits.
    for token in re.split(r"[^A-Za-z0-9/_.\-]", text):
        if not token.endswith(".js"):
            continue
        stem = token[:-3]
        if "-" not in stem:
            continue
        tail = stem.rsplit("-", 1)[1]
        if len(tail) >= 16 and all(c in "0123456789abcdefABCDEF" for c in tail):
            return tail
    return "none"

if mode == "html":
    with open(path, "rb") as fh:
        raw = fh.read()
    print("%s %d %s" % (hashlib.sha256(raw).hexdigest(), len(raw),
                        bundle_hash(raw.decode("utf-8", "replace"))))
    sys.exit(0)

try:
    with open(path, encoding="utf-8") as fh:
        doc = json.load(fh)
except Exception as exc:
    print("NOTJSON " + type(exc).__name__)
    sys.exit(0)

if mode == "health":
    db = doc.get("db") if isinstance(doc, dict) else None
    print("OK" if db == "ok" else "BAD db=%r" % (db,))
elif mode == "list":
    items = doc.get(arg) if isinstance(doc, dict) else None
    if not isinstance(items, list):
        print("NOLIST")
    else:
        with_image = sum(
            1 for item in items
            if isinstance(item, dict)
            and isinstance(item.get("image_url"), str)
            and item["image_url"].strip()
        )
        print("LIST %d %d" % (len(items), with_image))
elif mode == "notfound":
    error = doc.get("error") if isinstance(doc, dict) else None
    print("OK" if error == "not_found" else "BAD error=%r" % (error,))
elif mode == "seed":
    totals = doc.get("totals", {}) if isinstance(doc, dict) else {}
    offered = totals.get("families_offered") if isinstance(totals, dict) else None
    measured = doc.get("measured_at", "undated") if isinstance(doc, dict) else "undated"
    print("%s %s" % (offered if isinstance(offered, int) else "none", measured))
else:
    print("BADMODE")
PYEOF
}

printf 'postdeploy smoke  %s  (GET only)  %s UTC\n' "$BASE" "$(date -u '+%Y-%m-%d %H:%M:%S')"
printf 'bodies in         %s\n' "$OUT_DIR"

# 1. /health -- the readiness probe Railway itself uses (railway.toml).
if fetch health.json /health; then
  if [ "$CODE" = "200" ]; then
    verdict="$(probe health "$OUT_DIR/health.json")"
    if [ "$verdict" = "OK" ]; then
      pass "/health" "200, db ok"
    else
      fail "/health" "200, but $verdict"
    fi
  else
    fail "/health" "HTTP $CODE (503 is the handler's answer when SELECT 1 fails)"
  fi
else
  fail "/health" "no HTTP answer: $ERR"
fi

# 2. /api/bikes -- the catalog. The count is reported, never judged.
if fetch bikes.json /api/bikes; then
  if [ "$CODE" = "200" ]; then
    verdict="$(probe list "$OUT_DIR/bikes.json" bikes)"
    case "$verdict" in
      LIST\ *)
        read -r _ count with_image <<<"$verdict"
        pass "/api/bikes" "200, JSON list of $count families ($with_image with an image_url)"
        read -r seed_offered seed_date <<<"$(probe seed "$REPO/data/fleet_seed.json" 2>/dev/null || echo "none unread")"
        if [ "$seed_offered" = "none" ]; then
          note "/api/bikes" "data/fleet_seed.json could not be read; nothing to compare the count with"
        elif [ "$seed_offered" = "$count" ]; then
          note "/api/bikes" "equals totals.families_offered=$seed_offered in data/fleet_seed.json (measured $seed_date)"
        else
          note "/api/bikes" "differs from totals.families_offered=$seed_offered in data/fleet_seed.json (measured $seed_date); the seed is a dated measurement, check which one moved"
        fi
        ;;
      *)
        fail "/api/bikes" "200, but no \"bikes\" list: $verdict (content-type ${CTYPE:-none})"
        ;;
    esac
  else
    fail "/api/bikes" "HTTP $CODE"
  fi
else
  fail "/api/bikes" "no HTTP answer: $ERR"
fi

# 3. /api/delivery/zones -- the public zone list checkout reads.
if fetch zones.json /api/delivery/zones; then
  if [ "$CODE" = "200" ]; then
    verdict="$(probe list "$OUT_DIR/zones.json" zones)"
    case "$verdict" in
      LIST\ *)
        read -r _ count _ <<<"$verdict"
        pass "/api/delivery/zones" "200, JSON list of $count zones"
        ;;
      *)
        fail "/api/delivery/zones" "200, but no \"zones\" list: $verdict (content-type ${CTYPE:-none})"
        ;;
    esac
  else
    fail "/api/delivery/zones" "HTTP $CODE"
  fi
else
  fail "/api/delivery/zones" "no HTTP answer: $ERR"
fi

# 4. An unmatched /api path must be a JSON 404, never the SPA's index.html.
if fetch unmatched.json "$UNMATCHED_PATH"; then
  verdict="$(probe notfound "$OUT_DIR/unmatched.json")"
  if [ "$CODE" = "404" ] && [ "$verdict" = "OK" ] && [[ "$CTYPE" == application/json* ]]; then
    pass "unmatched /api path" "404, $CTYPE, error=not_found"
  else
    fail "unmatched /api path" "HTTP $CODE, content-type ${CTYPE:-none}, body: $verdict (want 404 JSON error=not_found)"
  fi
else
  fail "unmatched /api path" "no HTTP answer: $ERR"
fi

# 5. The served "/" against the committed dist/index.html.
ref_sha="$(git -C "$REPO" rev-parse -q --verify "$DIST_REF^{commit}" 2>/dev/null || true)"
if ! fetch index.served.html /; then
  fail "/" "no HTTP answer: $ERR"
elif [ "$CODE" != "200" ]; then
  fail "/" "HTTP $CODE"
elif [ -z "$ref_sha" ]; then
  fail "/" "200, but $DIST_REF is not a commit in $REPO, so there is nothing to compare with"
elif ! git -C "$REPO" show "$ref_sha:dist/index.html" >"$OUT_DIR/index.committed.html" 2>/dev/null; then
  fail "/" "200, but $DIST_REF (${ref_sha:0:7}) has no dist/index.html"
else
  read -r served_sha served_size served_bundle <<<"$(probe html "$OUT_DIR/index.served.html")"
  read -r committed_sha committed_size committed_bundle <<<"$(probe html "$OUT_DIR/index.committed.html")"
  if cmp -s "$OUT_DIR/index.served.html" "$OUT_DIR/index.committed.html"; then
    pass "/" "200, byte-equal to dist/index.html at $DIST_REF (${ref_sha:0:7})"
  else
    fail "/" "200, but differs from dist/index.html at $DIST_REF (${ref_sha:0:7})"
  fi
  detail "served     sha256 $served_sha  $served_size bytes  bundle $served_bundle"
  detail "committed  sha256 $committed_sha  $committed_size bytes  bundle $committed_bundle"
  if fetch version.txt /version.txt && [ "$CODE" = "200" ]; then
    detail "/version.txt says bundle $(head -c 80 "$OUT_DIR/version.txt" | tr -d '\r\n')"
  else
    detail "/version.txt: HTTP $CODE ${ERR}"
  fi
  if [ "$served_sha" != "$committed_sha" ]; then
    served_blob="$(git -C "$REPO" hash-object --no-filters "$OUT_DIR/index.served.html")"
    found=""
    while read -r commit; do
      if [ "$(git -C "$REPO" rev-parse -q --verify "$commit:dist/index.html" 2>/dev/null)" = "$served_blob" ]; then
        found="$commit"
        break
      fi
    done < <(git -C "$REPO" log --all --format=%H -n 400 -- dist/index.html)
    if [ -n "$found" ]; then
      detail "the served index.html is dist/index.html as committed in $(git -C "$REPO" log -1 --format='%h %cs %s' "$found")"
    else
      detail "the served index.html matches no dist/index.html in the last 400 commits that touched it"
    fi
  fi
fi

printf 'SUMMARY  %d passed, %d failed\n' "$PASSED" "$FAILED"
[ "$FAILED" -eq 0 ]
