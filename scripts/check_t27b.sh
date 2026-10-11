#!/usr/bin/env bash
# Run the TurboBaby contracts on t27b, t27's native backend ($T27B), and hold
# two floors: every spec the host executes passes on its own, and the number of
# whole files that pass never drops below the measured floor.
#
# t27b's JIT runs on arm64 only, so this runs on an arm64 runner.
set -euo pipefail
: "${T27B:?set T27B to the pinned t27b binary}"
FLOOR="${T27B_PASS_FLOOR:-42}"  # measured 2026-10-11, epic #77 / #78

for rs in src/spec/*.rs; do
  id="$(basename "$rs" .rs)"
  [ "$id" = "mod" ] && continue
  out="$("$T27B" test "specs/turbobaby/${id}.t27" --quiet 2>&1)" || true
  echo "$out"
  if ! echo "$out" | grep -qE ' 0 failed, '; then
    echo "check_t27b: FAIL - specs/turbobaby/${id}.t27 does not pass on t27b"
    exit 1
  fi
  if ! echo "$out" | grep -qE ' 0 broken, '; then
    echo "check_t27b: FAIL - specs/turbobaby/${id}.t27 breaks an invariant on t27b"
    exit 1
  fi
done

summary="$("$T27B" corpus specs/turbobaby --jobs 4 2>&1)"
echo "$summary" | head -12
passed="$(echo "$summary" | sed -nE 's/^ *supported, all tests pass *: *([0-9]+).*/\1/p')"
mismatch="$(echo "$summary" | sed -nE 's/^ *JIT\/interpreter mismatch *: *([0-9]+).*/\1/p')"
if [ -z "$passed" ]; then
  echo "check_t27b: FAIL - could not read the pass count from t27b corpus"
  exit 1
fi
if [ "$passed" -lt "$FLOOR" ]; then
  echo "check_t27b: FAIL - $passed whole files pass on t27b, floor is $FLOOR"
  exit 1
fi
if [ "${mismatch:-1}" != "0" ]; then
  echo "check_t27b: FAIL - JIT/interpreter mismatch is ${mismatch:-unreadable}, must be 0"
  exit 1
fi
echo "check_t27b: OK - $passed files pass (floor $FLOOR), 0 mismatch"
