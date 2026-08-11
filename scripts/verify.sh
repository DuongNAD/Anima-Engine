#!/usr/bin/env bash
# CỔNG CHỐNG GIAN LẬN. Agent KHÔNG được tự chứng nhận — chỉ script này phán.
# Dùng: ./scripts/verify.sh ["<dòng mutant từ missed.txt>"]
# Không tham số -> chạy luật 1..7 (đủ cho task triage / dataflow).
set -uo pipefail
set -e
ROOT=$(git rev-parse --show-toplevel); cd "$ROOT"
# shellcheck disable=SC1091
source .hardening.env

MUTANT="${1:-}"
# Vung ghi: thu muc test cua crate chinh VA cua moi crate con trong workspace.
# Tach crate con (vi du anima-core) la viec nen lam — no mo khoa fuzz/miri va lam
# mutation nhanh hang tram lan — nhung neu cong chi cho ghi vao $HD_CRATE/$HD_TEST
# thi agent khong the them test cho crate con, tuc tach xong lai khong dung duoc.
GUARD_RE="^$HD_CRATE/([^/]+/)?$HD_TEST/"
GUARD="$HD_CRATE/$HD_TEST/"
reject() { printf 'TU CHOI: %s\n' "$*" >&2; exit 1; }

# --- Luật 1: chỉ được ghi trong <crate>/<tests>/ ---
# Artifact do chính công cụ sinh ra — không phải agent sửa.
ART_RE='(^|/)(Cargo\.lock|target/|mutants\.out/|mutants\.verify/|logs/|baseline-.*\.txt|dataflow-truth\..*)'
# Bộ khung tự nó chưa commit -> lỗi của người cài, không phải của agent.
KIT_RE='^(justfile|AGENTS\.md|FINDINGS\.md|\.hardening\.env|scripts/|\.agents/skills|\.claude/skills|.*/\.cargo/mutants\.toml)'

CHANGED=$( { git diff --name-only HEAD; git ls-files --others --exclude-standard; } \
           | sort -u | grep -v '^$' | grep -vE "$ART_RE" || true)
BAD=$(echo "$CHANGED" | grep -vE "$GUARD_RE" | grep -v '^FINDINGS.md$' | grep -v '^$' || true)
NONKIT=$(echo "$BAD" | grep -vE "$KIT_RE" | grep -v '^$' || true)
if [ -n "$BAD" ] && [ -z "$NONKIT" ]; then
  reject "bo khung hardening chua duoc commit. Chay:
  git add justfile AGENTS.md FINDINGS.md .hardening.env scripts .agents/skills && git commit -m 'chore: cai hardening kit'"
fi
[ -n "$NONKIT" ] && reject "sua file ngoai ${HD_CRATE}/**/${HD_TEST}/ va FINDINGS.md:
$NONKIT"

# --- Diff PHẢI gồm cả file untracked. Không có dòng này thì agent chỉ cần tạo
#     file MỚI là lách sạch luật 2..6. Đây là lỗ thủng lớn nhất của cổng. ---
GUARD_DIRS=$(echo "$CHANGED" | grep -E "$GUARD_RE" | sed -E "s|($HD_CRATE/([^/]+/)?$HD_TEST)/.*|\\1|" | sort -u)
for d in $GUARD_DIRS; do git add --intent-to-add -- "$d" >/dev/null 2>&1 || true; done
DIFF=$(git diff HEAD -- $GUARD_DIRS 2>/dev/null || true)
ADDED=$(echo "$DIFF"   | grep '^+' | grep -v '^+++' || true)
REMOVED=$(echo "$DIFF" | grep '^-' | grep -v '^---' || true)

# --- Luật 2: không được vô hiệu hoá test ---
echo "$ADDED" | grep -qE '#\[(ignore|should_panic)' && reject "them #[ignore] hoac #[should_panic]"

# --- Luật 3: không được xoá assertion đã có (kể cả assert nằm giữa dòng) ---
echo "$REMOVED" | grep -qE '(assert|assert_eq|assert_ne|debug_assert|panic)!' \
  && reject "xoa hoac sua dong co assertion san co"

# --- Luật 4: test phải tất định ---
echo "$ADDED" | grep -qE '(thread_rng|rand::random|SystemTime::now|Instant::now)' \
  && reject "test dung RNG/dong ho phi tat dinh"

# --- Luật 4b: cấm rèn trạng thái private bằng unsafe ---
# Agent that da lam: khai bao mot struct "guong" cung layout roi transmute de ghi
# thang vao field private. Struct khong co #[repr(C)] thi Rust KHONG dam bao thu
# tu field — do la UB, hom nay chay duoc la may. Va no khong test hanh vi that:
# no ren ra mot trang thai ma API cong khai khong bao gio tao duoc.
echo "$ADDED" | grep -qE 'unsafe |transmute|from_raw_parts' \
  && reject "test dung unsafe/transmute de ren trang thai private. Do khong phai test hanh vi. Neu API cong khai khong tao duoc trang thai can test thi day la mutant can refactor: dung just skip hoac bao lai."

# --- Luật 5: assertion phải quan sát được hành vi, không phải "khong panic" ---
N_ANY=$(echo "$ADDED"    | grep -cE 'assert' || true)
N_STRONG=$(echo "$ADDED" | grep -cE 'assert_eq!|assert_ne!|assert!\([^)]*(==|!=|<|>|\.contains|\.len\(\)|\.starts_with)' || true)
if [ "$N_ANY" -gt 0 ] && [ "$N_STRONG" -eq 0 ]; then
  reject "chi co assertion rong nghia (is_ok/is_some/true) — khong quan sat duoc hanh vi"
fi

# --- Luật 6: có mutant mà không thêm assertion nào thì không thể coi là xong ---
if [ -n "$MUTANT" ] && [ "$N_ANY" -eq 0 ]; then
  reject "khong them assertion nao — task chua duoc lam"
fi

# --- Luật 7: test phải ổn định (3 lần liên tiếp) ---
( cd "$HD_CRATE"
  for i in 1 2 3; do
    cargo test --release --quiet >/dev/null 2>&1 || { echo "run $i hong" >&2; exit 1; }
  done ) || reject "test khong on dinh qua 3 lan chay"

# --- Luật 8: mutant phải THẬT SỰ chết ---
if [ -n "$MUTANT" ]; then
  FILE=$(printf '%s' "$MUTANT" | cut -d: -f1)
  RE=$(printf '%s' "$MUTANT" | sed 's/[][\.^$*+?(){}|\\/]/\\&/g')
  # --output riêng: mặc định cargo-mutants ghi de mutants.out/ va xoa sach
  # missed.txt — tuc la moi lan verify se huy danh sach task cua ca doi.
  #
  # --timeout BAT BUOC o day, nguoc voi `just hunt`. Voi --baseline=skip thi
  # cargo-mutants khong co baseline de tu tinh timeout, no rot ve mac dinh 300s.
  # Suite nay mat 89s chay don va toi 263s khi tranh CPU -> 300s la qua sat,
  # mutant bi bao TIMEOUT roi cong ket luan nham la "mutant van song".
  TMO="${VERIFY_TIMEOUT:-900}"
  # `set +e` BAT BUOC quanh doan nay: voi `set -e`, phep gan OUT=$(...) that bai
  # se giet script NGAY, truoc khi doc duoc RC va truoc khi toi cac cau tu choi
  # co giai thich. Ma cargo-mutants tra ma khac 0 chinh la truong hop mutant CON
  # SONG — tuc duong quan trong nhat cua cong se im lang.
  set +e
  OUT=$( cd "$HD_CRATE" && cargo mutants --file "$FILE" --re "$RE" --timeout "$TMO" \
           --baseline skip --output mutants.verify 2>&1 )
  RC=$?
  set -e
  echo "$OUT" | tail -6

  # Cong PHAI kiem rang co dung mot mutant duoc test. Khong kiem thi:
  #   - chuoi mutant go sai  -> regex khong khop -> "Found 0 mutants" -> exit 0
  #   - mutant da bi exclude -> y het nhu tren
  # va cong bao QUA CONG trong khi chua test gi. Day la FALSE PASS, loai hong te
  # nhat: no khen mot cong viec chua lam.
  if echo "$OUT" | grep -q "Found 0 mutants"; then
    reject "KHONG tim thay mutant nao khop. Hai kha nang: (1) chuoi mutant go sai, dan lai NGUYEN dong tu just list-missed; (2) mutant da bi loai trong .cargo/mutants.toml. Cong khong the ket luan gi."
  fi
  if echo "$OUT" | grep -qi "timeout"; then
    reject "mutant bi TIMEOUT, KHONG phai con song. Day khong phai loi cua test. Tang VERIFY_TIMEOUT hoac bao lai."
  fi
  [ "$RC" -eq 0 ] || reject "mutant van song sau khi them test"
  echo "$OUT" | grep -qE "[0-9]+ caught" || reject "khong xac nhan duoc mutant da chet"
fi

echo "QUA CONG"
