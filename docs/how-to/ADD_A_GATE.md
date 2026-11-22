---
title: Cách thêm một cổng kiểm tra (gate) mới vào CI
status: active
owner: maintainers
last_reviewed: 2026-11-22
review_cycle: per-release
---

# Cách thêm một cổng kiểm tra (gate) mới vào CI

Hướng dẫn này mô tả quy trình chuẩn để xây dựng, gắn **đối chứng âm (negative control)** bắt buộc, và nối một cổng kiểm tra mới vào [`package.json`](../../package.json) cùng [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml).

> **Bối cảnh & lý do tồn tại của các cổng:** xem [`STATE_OF_THE_PROJECT.md` §2.1](../planning/STATE_OF_THE_PROJECT.md#21-vì-sao-bộ-gate-này-đáng-tin) và [`COMPLETION_ROADMAP_2026-10-16.md` §1.2](../planning/COMPLETION_ROADMAP_2026-10-16.md#12-sáu-lỗi-quy-trình-đã-dẫn-tới-đây--mỗi-lỗi-thành-một-gate-ở-ht04).

---

## Bước 1 — Chọn quy ước mã thoát, phạm vi quét và allow-list hai chiều

Mọi script gate đặt tại `scripts/` cần tuân thủ các quy ước sau:

- **Mã thoát (exit codes):**
  - `0`: kiểm tra đạt toàn bộ.
  - `1`: phát hiện vi phạm hợp đồng hoặc chính sách trên repository/artifact.
  - `2`: lỗi cấu hình, thiếu đầu vào bắt buộc, parser khớp `0` mục, hoặc **self-test / đối chứng âm thất bại**.
- **Quét cả file mới chưa `git add`:** đối với gate kiểm tài liệu hoặc văn bản nguồn (như `check_text_hygiene.mjs` và `check_docs_links.mjs`), liệt kê file bằng `git ls-files -z --cached --others --exclude-standard` để bắt lỗi trước khi commit.
- **Allow-list hai chiều (khi có danh sách miễn trừ):** như [`scripts/test_target_policy.mjs`](../../scripts/test_target_policy.mjs) và [`scripts/root_allowlist.json`](../../scripts/root_allowlist.json), phải fail cả khi có mục **ngoài danh sách** lẫn khi có mục **trong danh sách đã hết hiệu lực**, kèm lý do tối thiểu rõ ràng.
- **Kiểm độ phủ parser:** không để parser khớp 0 mục mà exit `0` (ví dụ `check_ipc_reference.mjs` exit `2` nếu không tìm thấy handler nào; `check_test_targets.mjs` đối chiếu chéo số `ignored` với tổng kết của `libtest`).

---

## Bước 2 — Chèn đối chứng âm (Negative Control) bắt buộc

Một gate không có đối chứng âm có thể âm thầm luôn trả `0` khi regex/đường dẫn thay đổi hoặc khi cấu hình hỏng. Thân cây hiện dùng hai mẫu chuẩn — chọn mẫu phù hợp với chi phí đầu vào của gate:

### Mẫu A — `selfTest()` nội tuyến chạy trước khi quét (cho parser/validator nhanh)

Áp dụng cho gate thuần bộ nhớ như [`scripts/check_docs_metadata.mjs`](../../scripts/check_docs_metadata.mjs), [`scripts/check_ipc_reference.mjs`](../../scripts/check_ipc_reference.mjs), [`scripts/check_root_hygiene.mjs`](../../scripts/check_root_hygiene.mjs), và [`scripts/check_npm_audit.mjs`](../../scripts/check_npm_audit.mjs):

- Viết hàm `selfTest()` chứa các ca `mustFail` (mẫu cố tình sai) và `mustPass` (mẫu hợp lệ).
- Chạy `selfTest()` **vô điều kiện trước khi đọc repo**; nếu bất kỳ ca `mustFail` nào lọt qua hoặc ca `mustPass` bị từ chối, in lỗi và thoát ngay bằng **`process.exit(2)`**.

### Mẫu B — Cờ `--self-test` và bộ test Vitest với fixture tạm trên đĩa (cho gate đọc artifact/bundle)

Áp dụng cho gate kiểm thư mục đầu ra như [`scripts/verify_e3_artifacts.mjs`](../../scripts/verify_e3_artifacts.mjs), [`scripts/verify_esel1_artifacts.mjs`](../../scripts/verify_esel1_artifacts.mjs), [`scripts/verify_sel2p_artifacts.mjs`](../../scripts/verify_sel2p_artifacts.mjs), và [`scripts/check_bundle_budget.mjs`](../../scripts/check_bundle_budget.mjs):

- Hỗ trợ `--self-test` tạo thư mục tạm bằng `fs.mkdtempSync`, cấy từng lỗi giả lập (sai SHA-256, thiếu file, sửa ngưỡng preregistration, rò route chunk, v.v.) và khẳng định hàm kiểm tra trả về thất bại.
- Guard entrypoint bằng `if (argv[1] && import.meta.url === pathToFileURL(argv[1]).href) main()` để vừa export hàm thuần cho Vitest (`tests/frontend/checkBundleBudget.test.ts`), vừa test thực thi `spawnSync` trên tiến trình con.

---

## Bước 3 — Tự chứng minh gate chuyển ĐỎ trên lỗi giả lập

Trước khi coi gate là xong, phải chứng minh gate **thật sự chuyển đỏ (`exit != 0`)** và báo đúng vị trí lỗi khi có lỗi thật. Ví dụ trực tiếp dưới đây sao chép thư mục `smoke` của E2 vào thư mục gitignored `src-tauri/target/doc2-gate-neg/`, sửa một giá trị trong `per-seed-deltas.csv` và kiểm chứng `verify_e2_artifacts.mjs` báo đỏ với mã thoát khác `0`:

<!-- doc-test -->
```bash
rm -rf src-tauri/target/doc2-gate-neg
mkdir -p src-tauri/target/doc2-gate-neg
cp -R artifacts/experiments/e2-evolved-brain-default/smoke src-tauri/target/doc2-gate-neg/smoke
node -e "const fs = require('fs'); const p = 'src-tauri/target/doc2-gate-neg/smoke/per-seed-deltas.csv'; const lines = fs.readFileSync(p, 'utf8').split('\n'); lines[1] = lines[1].replace(/[^,]+$/, '999999.0'); fs.writeFileSync(p, lines.join('\n'));"
! node scripts/verify_e2_artifacts.mjs src-tauri/target/doc2-gate-neg
```

Kết quả quan sát được trên `macOS arm64` (chạy ngày `2026-11-22` tại commit `03b9d3d`; script trả về exit code `1`, dấu `!` xác nhận gate bắt đúng lỗi):

```text
note  analysis: no ensemble has been run yet (no paired-report.json at the artifact root)
note  smoke: recomputed 22 per-seed deltas
note  analysis: no execution-lock.json — the duration rung was not locked from a smoke calibration

FAIL  smoke: per-seed-deltas.csv hashes to 1b3313e1..., but checksums.sha256 says a459a031...
FAIL  smoke: seed 999983 detritus: delta 999999.0 is not 1019.997442117276 - 1019.9974421173067 (= -3.069544618483633e-11)

2 failure(s).
```

---

## Bước 4 — Nối gate vào `package.json` và `.github/workflows/ci.yml`

1. Thêm script `"check:<ten>": "node scripts/check_<ten>.mjs"` vào mục `"scripts"` của [`package.json`](../../package.json).
2. Mở [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) và chọn đúng job:
   - **`rust`** (`windows-latest`, `working-directory: src-tauri`): khi gate cần `cargo`, `cargo tree`, output của `cargo test`, hoặc compliance trên Windows.
   - **`frontend`** (`ubuntu-latest`): cho các cổng Node, ESLint, TypeScript, Vitest, Playwright, tài liệu và artifact checksum.
3. Tạo step riêng cho gate kèm **`if: ${{ !cancelled() }}`** để nếu một cổng trước đó fail thì các cổng sau vẫn tiếp tục chạy và báo đủ lỗi trong cùng một lượt CI.
4. Nếu thêm unit test mới cho gate trong `tests/frontend/`, chạy cả `npm run typecheck:tests` và `npm run test:frontend` (lưu ý gói `tests/` cài dependency riêng qua `npm ci --prefix tests`).

---

## Bước 5 — Chạy xác minh các cổng hiện có và `--self-test`

Chạy các cổng hygiene, tài liệu và bộ `--self-test` sẵn có từ thư mục gốc repository:

<!-- doc-test -->
```bash
npm run check:docs
npm run check:text-hygiene
npm run check:root-hygiene
node scripts/verify_esel1_artifacts.mjs --self-test
node scripts/verify_e3_artifacts.mjs --self-test
node scripts/verify_sel2p_artifacts.mjs --self-test
```

Kết quả quan sát được (rút gọn trên `macOS arm64`, chạy ngày `2026-11-22` tại commit `03b9d3d`; tổng số đếm toàn repo có thẩm quyền xem tại [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền)):

```text
docs link check: … relative links + … same-page anchors in … files, 0 broken
…
docs metadata check: … files, 0 errors, …; self-test 21 controls ok
ipc reference check: … commands, … events, 0 differences
text hygiene: … tracked text file(s), no raw control bytes
root hygiene: … root entries, all listed in scripts/root_allowlist.json; self-test 4 controls ok
self-test ok    mean, sd, se of deltas 1,2,3
… (verify_esel1_artifacts: 8/8 ok)
verify_e3_artifacts --self-test PASSED (20/20 ok)
… (verify_sel2p_artifacts: 34/34 ok)
```

---

## Bước tiếp theo

- Quy trình cho coding agent & ratchet: [AGENT_WORKFLOW.md](AGENT_WORKFLOW.md).
- Chỉ mục giải thích hệ thống: [chỉ mục Explanation](../explanation/README.md).
