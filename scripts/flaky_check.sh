#!/usr/bin/env bash
# Cổng ra Phase 0. Phải n/n pass mới được sang mutation.
# Thả cargo-mutants vào suite flaky = trộn "missed" với "nhiễu" = sinh task rác.
set -uo pipefail
ROOT=$(git rev-parse --show-toplevel); cd "$ROOT"
# shellcheck disable=SC1091
source .hardening.env
N="${1:-20}"
mkdir -p logs
cd "$HD_CRATE"

fail=0
for i in $(seq 1 "$N"); do
  log="$ROOT/logs/flaky-$i.log"
  if RAYON_NUM_THREADS=1 cargo test --release --quiet >"$log" 2>&1; then
    printf '  %2d/%s ok\n' "$i" "$N"
  else
    fail=$((fail+1))
    printf '  %2d/%s FLAKY -> logs/flaky-%d.log\n' "$i" "$N" "$i"
    grep -E '^(test .* FAILED|thread .* panicked)' "$log" | sort -u | head -5 | sed 's/^/      /'
  fi
done

echo "ket qua: $((N-fail))/$N pass"
[ "$fail" -eq 0 ] || exit 1
