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
verify mutant="" sub="":
    ./scripts/verify.sh "{{mutant}}" "{{sub}}"

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
# jobs mặc định 2, KHÔNG phải số core. Lý do: quét mutation là việc nền, còn máy
# thì người ta đang ngồi dùng. Đo được load average 114 trên máy 10 core với
# jobs=4 — trên MacBook Air (không quạt) thì giao diện giật hẳn, kể cả khi macOS
# chưa ghi nhận cảnh báo nhiệt nào. Sắp rời máy thì tự nâng: `just hunt-file <f> 4`.
#
# `nice -n 10`: việc nền không có lý do gì tranh CPU với thứ anh đang gõ.
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
hunt shard="1" jobs="2":
    -bj=$(( $(sysctl -n hw.ncpu 2>/dev/null || nproc) / {{jobs}} )); [ "$bj" -lt 1 ] && bj=1; \
     cd "$HD_CRATE" && CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$bj}" \
     nice -n 10 cargo mutants --iterate --copy-target true --shard {{shard}}/5 -j {{jobs}}

# Quét hẹp một file. Dùng cái này nhiều hơn `hunt`.
#
# tests="a,b" thu hẹp tập test binary được chạy. Sau khi vá copy-target thì phase
# TEST là nút thắt còn lại (575s/mutant so với 89s chạy đơn), và phần lớn test
# binary không thể nào giết được mutant trong file đang xét.
# ĐÁNH ĐỔI PHẢI BIẾT: thu hẹp làm cổng LỎNG hơn — mutant bị test đã loại giết
# thì sẽ báo "missed" giả, đẻ task rác. Dùng cho `hunt` (đi tìm) thì được;
# `verify` (phán quyết) luôn chạy toàn suite, không bao giờ thu hẹp.
# sub="anima-core" để quét crate con. Tách crate thuần ra khỏi vỏ ứng dụng làm
# mutation nhanh hàng trăm lần — đo được trên Anima-Engine: 24 mutant/55 phút ở
# crate chính so với 135 mutant/2 phút ở crate con. Quét crate con TRƯỚC.
#
# `file` nhận cả glob, NHỚ đặt trong nháy khi gọi:
#     just hunt-file 'src/evolution/*.rs'
# Không nháy thì shell bung glob ra nhiều đường dẫn, cargo-mutants chỉ nhận một.
# Xem test nào liên quan:  grep -rl "<tên module>" $HD_CRATE/$HD_TEST
hunt-file file jobs="2" tests="" sub="":
    -bj=$(( $(sysctl -n hw.ncpu 2>/dev/null || nproc) / {{jobs}} )); [ "$bj" -lt 1 ] && bj=1; \
     ta=""; for t in $(echo "{{tests}}" | tr ',' ' '); do ta="$ta --test $t"; done; \
     cd "$HD_CRATE/{{sub}}" && CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$bj}" \
     nice -n 10 cargo mutants --iterate --copy-target true --file "{{file}}" -j {{jobs}} \
       ${ta:+--} $ta

# Danh sách task cho agent. Mỗi dòng = MỘT task cho MỘT agent.
list-missed sub="":
    @cat "$HD_CRATE/{{sub}}/mutants.out/missed.txt" 2>/dev/null || echo "chua co missed.txt — chay 'just hunt' truoc"

# In prompt dán thẳng cho agent, mỗi mutant một cái.
tasks n="5":
    @./scripts/tasks.sh {{n}}

# Loại mutant KHÔNG giết được, kèm lý do bắt buộc. Đọc kỹ trước khi dùng:
# đây KHÔNG phải đường thoát cho mutant khó — chỉ dành cho equivalent mutant,
# tức thứ mà mọi giá trị thay thế đều cho hành vi y hệt.
skip re reason sub="":
    @./scripts/skip.sh "{{re}}" "{{reason}}" "{{sub}}"

mutation-score sub="":
    @cd "$HD_CRATE/{{sub}}" && c=$(grep -c . mutants.out/caught.txt 2>/dev/null || echo 0); \
     m=$(grep -c . mutants.out/missed.txt 2>/dev/null || echo 0); \
     t=$((c+m)); [ "$t" -eq 0 ] && echo "chua co du lieu" || \
     echo "caught=$c missed=$m score=$((100*c/t))%"

# ---------- fuzz + miri (chạy nền, 0 token) ----------
# Miri chỉ chạy được code Rust thuần, không FFI. Lọc hẹp, đừng chạy cả suite.
miri filter="":
    cd "$HD_CRATE" && MIRIFLAGS="-Zmiri-disable-isolation" cargo +nightly miri test --lib {{filter}}

fuzz target time="3600":
    cd "$HD_CRATE" && cargo +nightly fuzz run {{target}} -- -max_total_time={{time}}

fuzz-list:
    @ls "$HD_CRATE/fuzz/fuzz_targets" 2>/dev/null || echo "chua co fuzz target — chay 'cargo fuzz init' trong $HD_CRATE"

# Danh sách crash chưa triage.
crashes:
    @ls -1 "$HD_CRATE"/fuzz/artifacts/*/* 2>/dev/null || echo "khong co crash"

# Gắn vào Scheduled Task của Antigravity, 02:00 hằng đêm.
nightly time="14400":
    -just miri 2>&1 | tee "logs/miri-$(date +%F).log"
    -for t in $(ls "$HD_CRATE/fuzz/fuzz_targets" 2>/dev/null | sed 's/\.rs$//'); do \
        just fuzz "$t" {{time}} 2>&1 | tee "logs/fuzz-$t-$(date +%F).log"; done

# ---------- dataflow ----------
# Agent xuất dataflow.json, script đối chiếu với cargo-modules. Không có oracle
# thì "phân tích luồng dữ liệu" chỉ là văn xuôi đẹp — đây là oracle.
#
# Đo trên repo thật: đồ thị thô có 788 cạnh. Lọc bỏ "owns" (phân cấp module,
# không phải luồng dữ liệu) rồi gộp về mức module còn 11. 788 thì không agent
# nào liệt kê nổi; bắt liệt kê đủ là ra task bất khả thi. Việc lọc nằm trong
# scripts/verify_dataflow.py.
dataflow-truth:
    cd "$HD_CRATE" && cargo modules dependencies --lib --no-externs > ../dataflow-truth.dot
    @echo "canh 'uses' muc module: $(python3 scripts/verify_dataflow.py --count dataflow-truth.dot 2>/dev/null || echo '(chay dataflow-verify de xem)')"

# depth=1 -> so sánh ở mức "core", "ai". depth=2 -> "core::ecs".
dataflow-verify depth="1":
    python3 scripts/verify_dataflow.py dataflow.json dataflow-truth.dot {{depth}}

# ---------- perf ----------
# CẢNH BÁO: đây là module DUY NHẤT agent được sửa src/. Chạy trên nhánh riêng.
#
# `--benches` là bắt buộc, không phải tuỳ chọn. Thiếu nó thì `cargo bench --` đẩy
# cờ của criterion vào CẢ harness libtest mặc định (unittest trong src/), mà nó
# không hiểu `--save-baseline` và làm hỏng cả lượt đo:
#     error: Unrecognized option: 'save-baseline'
bench-baseline:
    cd "$HD_CRATE" && cargo bench --benches -- --save-baseline base

bench-compare:
    cd "$HD_CRATE" && cargo bench --benches -- --baseline base

# Gate: chỉ giữ tối ưu nếu vượt ngưỡng ý nghĩa thống kê, ngược lại revert.
perf-gate:
    ./scripts/perf_gate.sh
