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
#
# --copy-target true là thay đổi có tác động lớn nhất, đo được:
#   không có nó -> moi cay tam build lai TOAN BO dependency tu so 0
#                  (tauri + wgpu + burn), lap lai MOI luot quet, MOI worker
#   có nó       -> build 5-87 giay
# Trên APFS thì bản chép dùng reflink (copy-on-write) nên 13 GB target gần như
# không tốn byte đĩa nào và gần như tức thì.
#
# CARGO_BUILD_JOBS là thứ dễ bỏ sót nhất: `-j N` của cargo-mutants chạy N tiến
# trình cargo, NHƯNG mỗi cargo lại tự build với TOÀN BỘ số core. Không siết lại
# thì song song thật = N × ncpu (đo được: 4 × 10 = load 27 trên máy 10 core), và
# mỗi lượt test chậm gấp 3 lần chạy đơn — 263s so với 89s. Giảm song song lại
# NHANH HƠN. Công thức: jobs × CARGO_BUILD_JOBS ≈ ncpu.
hunt shard="1" jobs="4":
    -bj=$(( $(sysctl -n hw.ncpu 2>/dev/null || nproc) / {{jobs}} )); [ "$bj" -lt 1 ] && bj=1; \
     cd "$HD_CRATE" && CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$bj}" \
     cargo mutants --iterate --copy-target true --shard {{shard}}/5 -j {{jobs}}

# Quét hẹp một file. Dùng cái này nhiều hơn `hunt`.
#
# tests="a,b" thu hẹp tập test binary được chạy. Sau khi vá copy-target thì phase
# TEST là nút thắt còn lại (575s/mutant so với 89s chạy đơn), và phần lớn test
# binary không thể nào giết được mutant trong file đang xét.
# ĐÁNH ĐỔI PHẢI BIẾT: thu hẹp làm cổng LỎNG hơn — mutant bị test đã loại giết
# thì sẽ báo "missed" giả, đẻ task rác. Dùng cho `hunt` (đi tìm) thì được;
# `verify` (phán quyết) luôn chạy toàn suite, không bao giờ thu hẹp.
# Xem test nào liên quan:  grep -rl "<tên module>" $HD_CRATE/$HD_TEST
hunt-file file jobs="4" tests="":
    -bj=$(( $(sysctl -n hw.ncpu 2>/dev/null || nproc) / {{jobs}} )); [ "$bj" -lt 1 ] && bj=1; \
     ta=""; for t in $(echo "{{tests}}" | tr ',' ' '); do ta="$ta --test $t"; done; \
     cd "$HD_CRATE" && CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$bj}" \
     cargo mutants --iterate --copy-target true --file {{file}} -j {{jobs}} \
       ${ta:+--} $ta

# Danh sách task cho agent. Mỗi dòng = MỘT task cho MỘT agent.
list-missed:
    @cat "$HD_CRATE/mutants.out/missed.txt" 2>/dev/null || echo "chua co missed.txt — chay 'just hunt' truoc"

# In prompt dán thẳng cho agent, mỗi mutant một cái.
tasks n="5":
    @./scripts/tasks.sh {{n}}

mutation-score:
    @cd "$HD_CRATE" && c=$(grep -c . mutants.out/caught.txt 2>/dev/null || echo 0); \
     m=$(grep -c . mutants.out/missed.txt 2>/dev/null || echo 0); \
     t=$((c+m)); [ "$t" -eq 0 ] && echo "chua co du lieu" || \
     echo "caught=$c missed=$m score=$((100*c/t))%"
