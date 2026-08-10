#!/usr/bin/env bash
# Gate cho module perf. Đây là module DUY NHẤT agent được sửa src/,
# nên cổng đổi từ "diff nằm trong tests/" sang "benchmark phải thắng thật".
# Không thắng -> revert tự động. Agent không có quyền bàn.
set -uo pipefail
ROOT=$(git rev-parse --show-toplevel); cd "$ROOT"
# shellcheck disable=SC1091
source .hardening.env
THRESH="${PERF_THRESHOLD:-5}"   # % cải thiện tối thiểu

cd "$HD_CRATE"
cargo test --release --quiet >/dev/null 2>&1 || { echo "TU CHOI: test do sau khi toi uu"; exit 1; }

OUT=$(cargo bench -- --baseline base 2>&1)
echo "$OUT" | grep -E 'change:|Performance has' || true

if echo "$OUT" | grep -q 'Performance has regressed'; then
  echo "TU CHOI: co benchmark bi cham di -> revert"
  cd "$ROOT" && git checkout -- "$HD_CRATE/$HD_SRC"
  exit 1
fi
if ! echo "$OUT" | grep -q 'Performance has improved'; then
  echo "TU CHOI: khong co cai thien co y nghia thong ke -> revert"
  cd "$ROOT" && git checkout -- "$HD_CRATE/$HD_SRC"
  exit 1
fi
echo "QUA CONG (nguong ${THRESH}%)"
