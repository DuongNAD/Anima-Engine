#!/usr/bin/env bash
# Loại một mutant KHÔNG THỂ giết được, kèm lý do bắt buộc.
#
# Đây là bước thứ ba mà kit từng thiếu. Trước đó chỉ có hai lựa chọn: giết mutant,
# hoặc để nó nằm trong missed.txt đẻ task vô hạn. Equivalent mutant — thứ mà mọi
# giá trị thay thế đều cho hành vi y hệt trong môi trường test — không thuộc cả
# hai. Không có đường này thì agent sẽ lặp đủ 3 lần rồi báo thất bại, mỗi lượt
# quét lại đẻ ra đúng task đó.
#
# Dùng: ./scripts/skip.sh "<regex khớp mutant>" "<lý do + điều kiện gỡ ra>"
set -uo pipefail
ROOT=$(git rev-parse --show-toplevel); cd "$ROOT"
# shellcheck disable=SC1091
source .hardening.env

RE="${1:-}"; LYDO="${2:-}"
[ -z "$RE" ] || [ -z "$LYDO" ] && {
  echo "dung: just skip \"<regex>\" \"<ly do + dieu kien go ra>\""
  echo
  echo "Ly do BAT BUOC va phai tra loi duoc: vi sao MOI gia tri thay the deu cho"
  echo "hanh vi y het? Neu chi la 'kho test' thi do KHONG phai equivalent mutant —"
  echo "do la task chua lam."
  exit 1
}
[ "${#LYDO}" -lt 30 ] && { echo "TU CHOI: ly do qua ngan (<30 ky tu). Viet ro vi sao khong quan sat duoc."; exit 1; }

CFG="$HD_CRATE/.cargo/mutants.toml"
[ -f "$CFG" ] || { echo "khong thay $CFG"; exit 1; }
grep -qF "\"$RE\"" "$CFG" && { echo "da co san trong exclude_re: $RE"; exit 0; }

python3 - "$CFG" "$RE" "$LYDO" <<'PY'
import sys, re, textwrap
cfg, pat, ly = sys.argv[1], sys.argv[2], sys.argv[3]
s = open(cfg).read()
block = "\n".join("    # " + l for l in textwrap.wrap(ly, 72))
entry = f'{block}\n    "{pat}",\n'
if "exclude_re = [" in s:
    s = s.replace("exclude_re = [\n", "exclude_re = [\n" + entry, 1)
else:
    s = s.rstrip() + "\n\nexclude_re = [\n" + entry + "]\n"
open(cfg, "w").write(s)
print(f"da them vao exclude_re: {pat}")
PY
echo
echo "NHO: ghi mot muc TEST-GAP vao FINDINGS.md kem dieu kien go dong nay ra."
