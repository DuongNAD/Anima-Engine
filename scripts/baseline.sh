#!/usr/bin/env bash
# Chốt baseline. Không có baseline thì không phân biệt được "agent làm việc
# hữu ích" với "agent tiêu 8 tiếng quota".
set -uo pipefail
ROOT=$(git rev-parse --show-toplevel); cd "$ROOT"
# shellcheck disable=SC1091
source .hardening.env
cd "$HD_CRATE"

n() { awk -F: '{s+=$2} END{print s+0}'; }

echo "# baseline $(date -u +%FT%TZ)  commit=$(git rev-parse --short HEAD)  modules=$HD_MODULES"
echo -n "src_loc: ";        find "$HD_SRC" -name '*.rs' | xargs cat 2>/dev/null | wc -l | tr -d ' '
echo -n "test_loc: ";       find "$HD_TEST" -name '*.rs' 2>/dev/null | xargs cat 2>/dev/null | wc -l | tr -d ' '
echo -n "test_count: ";     grep -rhoE '#\[(tokio::)?test\]' "$HD_SRC" "$HD_TEST" 2>/dev/null | wc -l | tr -d ' '
echo -n "ignored_tests: ";  grep -rhoE '#\[ignore\]' "$HD_SRC" "$HD_TEST" 2>/dev/null | wc -l | tr -d ' '
echo -n "unsafe: ";         grep -rc 'unsafe' "$HD_SRC" --include='*.rs' 2>/dev/null | n
# Đếm LỜI GỌI thật, không đếm dòng chú thích nhắc tới tên hàm.
echo -n "thread_rng: ";     grep -rn 'rand::thread_rng()' "$HD_SRC" --include='*.rs' 2>/dev/null | grep -vc ':[[:space:]]*//'
echo -n "wallclock: ";      grep -rcE '(SystemTime|Instant)::now' "$HD_SRC" --include='*.rs' 2>/dev/null | n
echo -n "hashmap_iter: ";   grep -rc 'HashMap\|HashSet' "$HD_SRC" --include='*.rs' 2>/dev/null | n

case " $HD_MODULES " in *" mutation "*)
  echo -n "mutants_total: "; cargo mutants --list 2>/dev/null | wc -l | tr -d ' ';;
esac
echo -n "clippy_pedantic: "; cargo clippy --all-targets -- -W clippy::pedantic 2>&1 | grep -c '^warning'
echo -n "coverage_lines: ";  cargo llvm-cov --summary-only 2>/dev/null | awk '/TOTAL/{print $10; f=1} END{if(!f)print "n/a"}'
echo -n "findings_bug: ";    grep -c "^- \*\*Loại:\*\* BUG$" "$ROOT/FINDINGS.md" 2>/dev/null || echo 0
