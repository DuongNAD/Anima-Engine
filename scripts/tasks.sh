#!/usr/bin/env bash
# Biến missed.txt thành prompt dán được ngay, mỗi mutant một prompt.
# Không có bước này thì mỗi ngày phải copy tay từng dòng rồi ghép vào prompt —
# việc lặp lại 500 lần thì phải là lệnh, không phải thao tác.
set -uo pipefail
ROOT=$(git rev-parse --show-toplevel); cd "$ROOT"
# shellcheck disable=SC1091
source .hardening.env
N="${1:-5}"

MISSED="$HD_CRATE/mutants.out/missed.txt"
[ -s "$MISSED" ] || { echo "khong co mutant song. Chay 'just hunt-file <file>' truoc."; exit 0; }

i=0
while IFS= read -r m; do
  [ -z "$m" ] && continue
  i=$((i+1)); [ "$i" -gt "$N" ] && break
  cat <<PROMPT
════════════════════════ TASK $i ════════════════════════
Đọc AGENTS.md rồi dùng skill mutant-killer.

Mutant: $m

Chỉ được ghi trong $HD_CRATE/$HD_TEST/. Kết thúc bằng:
    just verify "$m"
Chưa thấy QUA CONG thì chưa xong. Tối đa 3 lần thử.
Không trả lời được "hành vi nào quan sát được sẽ sai" thì dừng và báo
cần refactor — đừng đoán.

PROMPT
done < "$MISSED"

TOTAL=$(grep -c . "$MISSED")
echo "════════════════════════════════════════════════════"
echo "in $i/$TOTAL task. Mỗi task MỘT agent, không gộp."
