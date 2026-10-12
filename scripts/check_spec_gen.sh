#!/usr/bin/env bash
# Every rule the host executes from a spec lives in src/spec/<id>.rs as the
# untouched output of `t27c gen-rust specs/turbobaby/<id>.t27`. This gate
# regenerates each one with the pinned compiler ($T27C) and fails on any
# difference, so the host can never run a rule the spec no longer states.
#
# A t27c that dies on the spec must not masquerade as drift: 2026-10-13, a
# spec comment holding a literal ';' inside its prose made the pinned t27c
# exit 1 with empty stdout, and diffing that nothing against the module
# read as "drifted". The compiler's own words are the failure here.
#
# Discovery floor: finding no generated module is a failure, not a pass.
set -euo pipefail
: "${T27C:?set T27C to the pinned t27c binary}"

checked=0
failed=0
for rs in src/spec/*.rs; do
  id="$(basename "$rs" .rs)"
  [ "$id" = "mod" ] && continue
  spec="specs/turbobaby/${id}.t27"
  checked=$((checked + 1))
  if [ ! -f "$spec" ]; then
    echo "check_spec_gen: FAIL - $rs has no spec at $spec"
    failed=$((failed + 1))
    continue
  fi
  if ! "$T27C" gen-rust "$spec" > /tmp/spec-gen-out.txt 2> /tmp/spec-gen-err.txt; then
    echo "check_spec_gen: FAIL - t27c gen-rust rejected $spec:"
    cat /tmp/spec-gen-err.txt
    failed=$((failed + 1))
    continue
  fi
  if [ -s /tmp/spec-gen-err.txt ]; then
    echo "check_spec_gen: FAIL - t27c gen-rust printed to stderr on $spec (a warning is drift here):"
    cat /tmp/spec-gen-err.txt
    failed=$((failed + 1))
    continue
  fi
  if ! diff -u /tmp/spec-gen-out.txt "$rs" > /tmp/spec-gen-diff.txt; then
    echo "check_spec_gen: FAIL - $rs differs from t27c gen-rust $spec:"
    head -40 /tmp/spec-gen-diff.txt
    failed=$((failed + 1))
  else
    echo "check_spec_gen: ok - $rs == t27c gen-rust $spec"
  fi
done

if [ "$checked" -eq 0 ]; then
  echo "check_spec_gen: FAIL - no generated modules found under src/spec/"
  exit 1
fi
if [ "$failed" -ne 0 ]; then
  echo "check_spec_gen: $failed of $checked generated module(s) drifted; regenerate with:"
  echo "  t27c gen-rust specs/turbobaby/<id>.t27 > src/spec/<id>.rs"
  exit 1
fi
echo "check_spec_gen: OK - $checked generated module(s) match their specs"
