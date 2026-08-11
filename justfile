set shell := ["bash", "-uc"]
set dotenv-filename := ".hardening.env"
set dotenv-load := true

# Giao diện lệnh duy nhất. Agent không được gọi cargo trực tiếp.
default:
    @just --list --unsorted

# Chốt số đo gốc. LÀM TRƯỚC MỌI THỨ.
baseline:
    ./scripts/baseline.sh | tee "baseline-$(date +%F).txt"

test:
    cd "$HD_CRATE" && cargo test --release

# CỔNG DUY NHẤT. Không thấy "QUA CONG" nghĩa là chưa xong.
verify mutant="":
    ./scripts/verify.sh "{{mutant}}"

clippy:
    cd "$HD_CRATE" && cargo clippy --all-targets -- -W clippy::pedantic

cov:
    cd "$HD_CRATE" && cargo llvm-cov --summary-only

audit:
    cd "$HD_CRATE" && cargo audit

# ---------- determinism ----------
# Cổng ra Phase 0. Phải n/n pass mới được sang mutation.
flaky n="20":
    ./scripts/flaky_check.sh {{n}}

# Liệt kê mọi nguồn phi tất định còn lại trong src.
seed-audit:
    @grep -rnE 'thread_rng|rand::random|SystemTime::now|Instant::now' "$HD_CRATE/$HD_SRC" --include='*.rs' || echo "sach"

# Chạy riêng golden test tất định.
determinism:
    cd "$HD_CRATE" && cargo test --release determinism -- --nocapture

# ---------- mutation ----------
# Quét toàn bộ. shard 1..5 để chia sweep theo NGÀY.
# KHÔNG chạy 5 shard cùng lúc trên một máy — chúng tranh CPU của nhau.
#
# KHÔNG truyền --timeout ở đây. Nó ĐÈ mọi thứ trong .cargo/mutants.toml, kể cả
# minimum_test_timeout. Đặt số cứng thấp hơn thời gian chạy suite là giết luôn
# cả baseline, và cargo-mutants bỏ nguyên lượt quét với thông báo
# "cargo test failed in an unmutated tree".
#
# jobs mặc định 4: bằng số P-core của máy Apple Silicon phổ thông. Đặt cao hơn
# thì phần dôi rơi xuống E-core, chậm hơn nhiều mà vẫn tranh bộ nhớ với P-core —
# tổng thời gian tệ đi chứ không tốt lên.
hunt shard="1" jobs="4":
    -cd "$HD_CRATE" && cargo mutants --iterate --shard {{shard}}/5 -j {{jobs}}

# Quét hẹp một file. Dùng cái này nhiều hơn `hunt`.
hunt-file file jobs="4":
    -cd "$HD_CRATE" && cargo mutants --iterate --file {{file}} -j {{jobs}}

# Danh sách task cho agent. Mỗi dòng = MỘT task cho MỘT agent.
list-missed:
    @cat "$HD_CRATE/mutants.out/missed.txt" 2>/dev/null || echo "chua co missed.txt — chay 'just hunt' truoc"

mutation-score:
    @cd "$HD_CRATE" && c=$(grep -c . mutants.out/caught.txt 2>/dev/null || echo 0); \
     m=$(grep -c . mutants.out/missed.txt 2>/dev/null || echo 0); \
     t=$((c+m)); [ "$t" -eq 0 ] && echo "chua co du lieu" || \
     echo "caught=$c missed=$m score=$((100*c/t))%"
