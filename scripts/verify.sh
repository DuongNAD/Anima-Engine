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
GUARD="$HD_CRATE/$HD_TEST/"
reject() { printf 'TU CHOI: %s\n' "$*" >&2; exit 1; }

# --- Luật 1: chỉ được ghi trong <crate>/<tests>/ ---
# Artifact do chính công cụ sinh ra — không phải agent sửa.
ART_RE='(^|/)(Cargo\.lock|target/|mutants\.out/|mutants\.verify/|logs/|baseline-.*\.txt|dataflow-truth\..*)'
# Bộ khung tự nó chưa commit -> lỗi của người cài, không phải của agent.
KIT_RE='^(justfile|AGENTS\.md|FINDINGS\.md|\.hardening\.env|scripts/|\.agents/skills|\.claude/skills|.*/\.cargo/mutants\.toml)'

CHANGED=$( { git diff --name-only HEAD; git ls-files --others --exclude-standard; } \
           | sort -u | grep -v '^$' | grep -vE "$ART_RE" || true)
BAD=$(echo "$CHANGED" | grep -v "^${GUARD}" | grep -v '^FINDINGS.md$' | grep -v '^$' || true)
NONKIT=$(echo "$BAD" | grep -vE "$KIT_RE" | grep -v '^$' || true)
if [ -n "$BAD" ] && [ -z "$NONKIT" ]; then
  reject "bo khung hardening chua duoc commit. Chay:
  git add justfile AGENTS.md FINDINGS.md .hardening.env scripts .agents/skills && git commit -m 'chore: cai hardening kit'"
fi
[ -n "$NONKIT" ] && reject "sua file ngoai ${GUARD} va FINDINGS.md:
$NONKIT"

# --- Diff PHẢI gồm cả file untracked. Không có dòng này thì agent chỉ cần tạo
#     file MỚI là lách sạch luật 2..6. Đây là lỗ thủng lớn nhất của cổng. ---
git add --intent-to-add -- "$GUARD" >/dev/null 2>&1 || true
DIFF=$(git diff HEAD -- "$GUARD" || true)
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
  ( cd "$HD_CRATE" && cargo mutants --file "$FILE" --re "$RE" \
      --baseline skip --output mutants.verify ) \
    || reject "mutant van song sau khi them test"
fi

echo "QUA CONG"
