---
title: Vì sao các cổng kiểm tra tồn tại
status: active
owner: architecture
last_reviewed: 2026-11-22
review_cycle: quarterly
---

# Vì sao các cổng kiểm tra tồn tại

Tài liệu này giải thích **mỗi cổng kiểm tra (gate)** trong Anima Engine tồn tại để chặn **kiểu hỏng thực tế nào** — đặc biệt là các lỗi *"exit 0 nhưng sai"* khiến bộ kiểm thử vẫn báo xanh trong khi hệ thống đã gãy. Toàn bộ số đo, số lượng test hiện hành và lịch sử bằng chứng được giữ tại nguồn chuẩn duy nhất:

- Vì sao bộ gate đáng tin: [`STATE_OF_THE_PROJECT.md` §2.1](../planning/STATE_OF_THE_PROJECT.md#21-vì-sao-bộ-gate-này-đáng-tin) và bảng số đo tại [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).
- Sáu lỗi quy trình lịch sử đã sinh ra kiến trúc gate hiện tại: [`COMPLETION_ROADMAP_2026-10-16.md` §1.2](../planning/COMPLETION_ROADMAP_2026-10-16.md#12-sáu-lỗi-quy-trình-đã-dẫn-tới-đây--mỗi-lỗi-thành-một-gate-ở-ht04).
- Sổ ghi các lỗ hổng đã phát hiện (gồm cả các điểm còn mở như `F-054`): [`FINDINGS.md`](../../FINDINGS.md).
- Các bảng đo khoa học hiện có cho bài báo (`determinism_matrix.md`, `scaling.md`, `eu_residual.md`): [`docs/paper/data/README.md`](../paper/data/README.md).

---

## 1. Nguyên lý chung: chống "exit 0 mà sai"

Rủi ro lớn nhất trong lịch sử dự án không phải là crash ồn ào làm CI đỏ ngay lập tức, mà là **tín hiệu xanh giả**: target biên dịch ra 0 test vẫn exit `0`; mock ở frontend nuốt sai tên field của Rust; agent vô tình sửa nhầm code nguồn hay hạ assertion để qua test; hoặc chính script cổng kiểm tra hỏng regex nên không còn bắt lỗi nữa.

Để chống tình trạng đó, kiến trúc gate trong `.github/workflows/ci.yml`, `.github/workflows/hardening.yml`, `justfile` và thư mục `scripts/` lặp lại bốn mẫu thiết kế xuyên suốt:

1. **Tự kiểm đối chứng âm (self-test negative controls):** năm script cổng (`check_docs_metadata.mjs`, `check_ipc_reference.mjs`, `check_root_hygiene.mjs`, `check_npm_audit.mjs`, `check_e2e_specs.mjs`) tự động chạy bộ fixture cài lỗi *trước khi* quét repo thật và thoát mã `2` nếu chính bộ phân tích của cổng không còn bắt được lỗi; trong khi đó `compare_checksums.mjs --self-test` (thoát mã `2`) và `verify_e3_artifacts.mjs --self-test` (thoát mã `1`) cung cấp chế độ `--self-test` chạy độc lập theo cờ gọi.
2. **Danh sách cho phép hai chiều kèm lý do bắt buộc:** `test_target_policy.mjs`, `root_allowlist.json`, `npm_audit_policy.mjs`, `e2e_spec_policy.mjs` và `NO_FRONT_MATTER` trong `check_docs_metadata.mjs` vừa từ chối mục lạ chưa khai báo, vừa từ chối mục miễn trừ đã ôi hoặc không còn tồn tại để danh sách ngoại lệ không mục rữa theo thời gian.
3. **Oracle xuyên ranh giới / xuyên tiến trình:** không để một subsystem tự xác nhận cho chính mình (`cargo tree` kiểm đồ thị thật thay vì chỉ tin build; `node:crypto` và `git cat-file` đối chiếu hash do Rust sinh; `verify_dataflow.py` đối chiếu `dataflow.json` với `cargo-modules`; `check_flora_footprint.mjs` chạy trên `three` thật trong Node thay vì Vitest mock).
4. **Ratchet một chiều & chống lách luật:** nâng hoặc khóa chặt ngưỡng (`eslint_ratchet.mjs` giữ nền `0`, `check_bundle_budget.mjs` khóa trần dung lượng và tách route), kết hợp các luật chống gian lận trong `scripts/verify.sh` (Luật 1–8 kèm Luật 4b).

---

## 2. Nhóm 1 — Cổng biên dịch, tách feature và chuỗi cung ứng Rust

- **Ghim toolchain (`rust-toolchain.toml` trong `ci.yml`, `hardening.yml`, `determinism-matrix.yml`):** khi trình biên dịch `rustc` ở ngoài nâng phiên bản (như đợt `rustc 1.98` thêm lint clippy mới hoặc `1.100.0` lệch toolchain ghim), mã không hề chạm tới cũng có thể đỏ dây chuyền và làm trôi cache build Windows. Ghim đúng một phiên bản trong `rust-toolchain.toml` kèm khóa cache theo hash của file này giúp lỗi build luôn phản ánh thay đổi trong repo.
- **Kiểm dạng chuẩn & tư vấn bảo mật (`cargo fmt --check`, `cargo audit`, `cargo deny check`):** `cargo audit` (đọc `src-tauri/.cargo/audit.toml`) và `cargo deny check` (đọc `src-tauri/deny.toml`) giữ kỷ luật giấy phép (`MIT OR Apache-2.0` cùng các crate ngoại lệ đã duyệt) và chặn lỗ hổng bắc cầu lọt vào build như lỗ hổng từng đi qua feature `train` của `burn`. Việc giữ cả hai cổng là có chủ ý: như [`FINDINGS.md` (`F-032`/`F-033`)](../../FINDINGS.md#f-033--miễn-trừ-tokio-vừa-viết-vào-denytoml-có-bản-vá-trong-tầm-với-1533) chỉ ra, `cargo audit` đọc thẳng lockfile còn `cargo deny` dựng đồ thị phụ thuộc mục tiêu nên phạm vi quét không trùng khít hoàn toàn.
- **Cô lập ranh giới crate & feature bằng `cargo tree` (`.github/workflows/ci.yml`):** bên cạnh `cargo clippy --all-targets --features desktop -- -D warnings`, `cargo clippy -p anima-worldgen --all-targets --features reader,generate,bake -- -D warnings` và `cargo clippy --all-targets --no-default-features -- -D warnings`, CI chạy ba kiểm tra `cargo tree` tách biệt khẳng định **0 lần xuất hiện**:
  - `cargo tree --no-default-features` trên toàn workspace cấm 10 họ crate nặng tùy chọn (`neo4rs`, `tokio-tungstenite`, `burn-wgpu`, `naga`, `zstd-sys`, `wgpu`, `ktx2`, `meshopt`, `bc7enc`, `cubecl-wgpu`);
  - `cargo tree -p anima-engine` (cấu hình mặc định của `anima-engine`) cấm cả 10 họ crate trên lẫn `anima-worldgen`;
  - `cargo tree -p anima-worldgen --features reader,generate,bake` cấm `bevy_ecs`, `tauri` và `wgpu`.
  Kiểm riêng bước compile là chưa đủ: chỉ cần bật nhầm feature hoặc nối phụ thuộc không-tùy-chọn trong `Cargo.toml` (chứ không phải một dòng `use` đơn lẻ, vốn tự gãy biên dịch dưới `--no-default-features`), cả đồ thị phụ thuộc nặng đã lọt vào bản mặc định trong khi `cargo build` và `cargo clippy` vẫn báo xanh.

---

## 3. Nhóm 2 — Cổng thực thi test và chống target rỗng / bỏ qua im lặng

- **`scripts/check_test_targets.mjs` + `scripts/test_target_policy.mjs`:** khi chạy `cargo test --features desktop` và `cargo test -p anima-worldgen --features "reader,generate,bake"`, có bốn kiểu chết im lặng mà `cargo test` vẫn có thể exit `0` hoặc bỏ sót:
  1. Một binary test bị chết giữa dòng `Running ...` và `running N tests` (ví dụ lỗi nạp DLL trên Windows).
  2. File test có `#![cfg(feature = "...")]` nhưng thiếu cờ nên biên dịch thành binary `running 0 tests` và exit `0` — lịch sử từng có nhiều file test bị bỏ qua im lặng theo cách này.
  3. Test bị gắn `#[ignore]` để né lỗi.
  4. Tên `required-features` trong `Cargo.toml` bị gõ sai hoặc mục miễn trừ bị bỏ quên.
  `test_target_policy.mjs` khóa hai chiều các danh sách `EMPTY_TARGETS`, `IGNORED_TESTS`, `FEATURE_GATED_TARGETS` và `PROFILES` (`default`, `desktop`, `worldgen-cpu`) với lý do tối thiểu 40 ký tự cho mỗi ngoại lệ. Lịch sử từng có sự cố chính `check_test_targets.mjs` bị commit kèm 3 khối merge conflict chưa giải quyết khiến cổng chết ngay khi chạy ([`FINDINGS.md` (`F-017`)](../../FINDINGS.md)).
- **`src-tauri/tests/feature_gated_targets_tests.rs`:** giữ cho khai báo `#![cfg(feature = "...")]` trong Rust, `required-features` trong `Cargo.toml` và `scripts/test_target_policy.mjs` khớp nhau hai chiều ngay từ trong suite `cargo test`.

---

## 4. Nhóm 3 — Cổng tất định, thứ tự lịch trình và hardening cục bộ

- **Chống trôi tính tất định và thứ tự mơ hồ của ECS (`hardening.yml` & Rust gates):**
  - `src-tauri/tests/determinism_tests.rs` giữ hồi quy golden `world_hash` qua 200 tick đồng thời có test chống vô nghĩa `world_hash_bat_duoc_thay_doi_that` (chứng minh hash thực sự đổi khi trạng thái đổi) và kiểm tính tất định của `build_tick_schedule`.
  - `src-tauri/tests/determinism_gate_tests.rs` gọi `child_entry_point` trong **tiến trình con tách biệt** để bắt các rò rỉ chỉ lộ giữa hai tiến trình (như `RandomState` của `HashMap`, địa chỉ allocator hay `Uuid::new_v4()`).
  - `src-tauri/tests/sim_determinism_tests.rs` kiểm `SimRng`, `derived_rng`, chạy `no_process_random_source_in_backend_sources` cấm `thread_rng` trong nguồn backend, và `live_simulation_does_not_bypass_the_deterministic_id_gate` cấm `Uuid::new_v4` lọt qua cổng định danh `next_entity_id`.
  - `src-tauri/tests/schedule_ambiguity_tests.rs` và `src-tauri/tests/schedule_total_order_tests.rs` (kèm `f041_child_entry_point` kiểm invariance qua nhiều tiến trình) bịt lỗ hổng mô tả ở [`FINDINGS.md` (`F-041`)](../../FINDINGS.md): chỉ cần một cặp hệ thống không có cạnh thứ tự tường minh và `auto_insert_apply_deferred = false` không được giữ nghiêm, hai lần chạy cùng seed có thể ra hai checksum khác nhau.
  - `.github/workflows/hardening.yml` chạy `cargo test --release`, lặp 5 lần với `RAYON_NUM_THREADS=1` (cục bộ qua `just flaky` / `scripts/flaky_check.sh`) và quét cấm `rand::thread_rng()`.
- **Phân biệt workflow đo với gate chặn:** `.github/workflows/determinism-matrix.yml` chạy `cross_platform_checksum_probe` cùng `scripts/paper/compare_checksums.mjs --self-test` trên Windows, Linux và macOS để **đo đạc** bằng chứng cho [`docs/paper/data/README.md`](../paper/data/README.md), không đóng vai trò cổng chặn mặc định của PR.
- **Bộ công cụ chống gian lận cục bộ (`justfile`, `scripts/verify.sh`, `scripts/perf_gate.sh`):** `just verify` (`scripts/verify.sh`) thực thi các luật chống gian lận cho agent sửa test (Luật 1–8 kèm Luật 4b, tóm thành 6 nhóm kiểm tra):
  1. Khóa vùng ghi vào `<crate>/tests/` và `FINDINGS.md`, dùng `git add --intent-to-add` để không bỏ sót file test mới chưa track (bịt lỗ hổng từng ghi nhận ở [`FINDINGS.md` (`F-024`)](../../FINDINGS.md#f-024--cổng-verify-tự-hủy-im-lặng-khi-diff-không-chứa-file-test-nào)).
  2. Cấm `#[ignore]` / `#[should_panic]`.
  3. Cấm xóa assertion hiện có.
  4. Cấm nguồn phi tất định (`thread_rng`, `rand::random`, `SystemTime::now`, `Instant::now`) và cấm `unsafe` / `transmute` / `from_raw_parts` để đọc lén trường private.
  5. Cấm assertion rỗng kiểu chỉ kiểm `is_ok()`, đồng thời bắt buộc thêm assertion khi làm thẻ diệt mutant.
  6. Chạy ổn định 3 lần ở `--release` và xác minh `cargo mutants` thực sự diệt được đúng mutant mục tiêu (từ chối cả `"Found 0 mutants"` lẫn timeout để tránh pass giả).
  Ngoài ra `just perf-gate` (`scripts/perf_gate.sh`) tự động `git checkout` hoàn tác thay đổi hiệu năng nếu test hỏng hoặc Criterion không cho thấy cải thiện có ý nghĩa thống kê.

---

## 5. Nhóm 4 — Cổng ranh giới IPC, kiến trúc module và Frontend

- **Parity Rust ↔ TypeScript (`ts-rs`, `ipc_registration_tests.rs`, `check_ipc_arg_case.mjs`):**
  - `cargo test --lib export_bindings` + `git diff --exit-code` trên `src/types/generated` bảo đảm mọi thay đổi ở struct IPC phía Rust (`SimulationTickPayload`, `SimulationStatus`, ...) buộc phải đi kèm file `.ts` tái sinh — chặn sự cố lệch dạng map vs array như `head_directions` trước đây.
  - `src-tauri/tests/ipc_registration_tests.rs` chứng minh mọi `#[tauri::command]` đều được đăng ký trong `tauri::generate_handler!`, lỗi mà cả compiler lẫn mock UI đều không thấy cho tới khi user bấm nút gặp `Unknown command`.
  - `scripts/check_ipc_arg_case.mjs` (`npm run check:ipc`) đối chiếu khóa object truyền vào `invoke(...)` phía frontend với tên tham số Rust vì `tauri-macros` mặc định dùng `camelCase`: truyền nhầm `snake_case` khiến tham số `Option<T>` âm thầm biến thành `None` trong khi Vitest mock vẫn xanh.
- **Đồ thị phụ thuộc module (`scripts/verify_dataflow.py`, `dataflow.json`):** đối chiếu các cạnh dữ liệu khai báo trong `dataflow.json` với đầu ra thật của `cargo modules` (sau khi lọc cạnh `"owns"`) và báo đỏ chu trình phụ thuộc hai chiều (xem [`FINDINGS.md` (`F-026`)](../../FINDINGS.md): phát hiện đang `OPEN` ghi nhận lỗi gọi sai cờ `--count` trong `justfile`, vẫn tồn tại ngay trong dòng tổng kết của recipe `dataflow-truth`).
- **Chất lượng & an toàn Frontend (`ci.yml`):**
  - `scripts/check_npm_audit.mjs` + `scripts/npm_audit_policy.mjs`: tách `DEV_ONLY_EXEMPTIONS` có lý do rõ ràng và luôn chạy lại `npm audit --omit=dev` (thay thế lệnh `npm audit --audit-level=high` thuần sau sự cố advisory `picomatch` chỉ nằm ở dev dependencies).
  - `scripts/eslint_ratchet.mjs`: giữ `DEFAULT_BASELINE = 0` để warning ESLint không thể phình ngược lại.
  - `npm run typecheck:tests`: chạy `tsc -p tests/tsconfig.json --noEmit` vì `npm run build` chỉ kiểm type cho `src/` còn Vitest transpile mà không kiểm type — nhờ cổng này đã phát hiện hàng loạt lỗi lệch kiểu trong test tích tụ theo thời gian.
  - `scripts/check_csp_compat.mjs`: chặn `tauri.conf.json` thiếu chính sách CSP nghiêm ngặt (`default-src 'self'`, cấm `'unsafe-inline'`/`'unsafe-eval'` ở `script-src`) và quét `dist/*.html` để chặn thẻ `<script>` inline hoặc tải CDN ngoài như sự cố `public/ecosystem.html` từng gặp.
  - `scripts/check_bundle_budget.mjs`: giữ trần kích thước từng chunk (`CHUNK_BUDGET_KIB`), tổng bundle (`TOTAL_BUDGET_KIB`) và kiểm tra route không kéo nhầm chunk 3D nặng (`THREE_D_CHUNK`) vào trang chính `index.html`. *Lưu ý trung thực:* hiện tượng [`FINDINGS.md` (`F-054`)](../../FINDINGS.md#f-054--checkbundle-nhận-ra-chunk-3d-chỉ-bằng-tên-file-nên-không-thấy-threejs-bị-import-thẳng-vào-trang-chính) vẫn đang ở trạng thái **OPEN** trên nhánh thân cây vì heuristic theo tên file không bắt được trường hợp `import * as THREE` trực tiếp bị rolldown gộp vào chunk khác (chỉ bị chặn gián tiếp bởi trần dung lượng).
  - `scripts/check_flora_footprint.mjs`: chạy trên `three` thật trong Node để đối chiếu `FLORA_FOOTPRINT_UNIT_RADIUS` với hình học thật, vì Vitest đang alias `three` sang mock.
  - `scripts/check_e2e_specs.mjs` + `scripts/e2e_spec_policy.mjs` (`SCHEDULED_SPECS`, `EXCLUDED_SPECS`) + `tests/e2e/tauri-mock.ts` + `tests/e2e/real_backend.spec.ts` + `tests/frontend/mapManifestEvidence.test.ts`: chặn lỗi thu hẹp lệnh `test:e2e` về một thư mục con khiến các spec quan trọng như kiểm hợp đồng IPC bị bỏ rơi trong im lặng, chặn bám nhầm Vite server của tiến trình khác trên cùng máy, và xác minh checksum PNG bằng chứng bản đồ trong `map_manifest.json`.

---

## 6. Nhóm 5 — Cổng bằng chứng thí nghiệm và tiền đăng ký

Khi chạy thí nghiệm tiến hóa, lỗi nguy hiểm nhất là **"kết quả đẹp từ dữ liệu giả hoặc manifest bị sửa ngầm"** (như từng ghi nhận sự cố dùng ảnh 1×1, seed cố định `42`, hay sửa fixture không đối chứng ở [`COMPLETION_ROADMAP_2026-10-16.md` §1.2](../planning/COMPLETION_ROADMAP_2026-10-16.md#12-sáu-lỗi-quy-trình-đã-dẫn-tới-đây--mỗi-lỗi-thành-một-gate-ở-ht04)). Nhóm cổng này dựng xác minh độc lập xuyên ngôn ngữ:

- **`scripts/verify_e2_artifacts.mjs` (`npm run check:e2`):** tự tính lại toàn bộ `checksums.sha256` bằng `node:crypto` (độc lập với mã SHA-256 trong Rust), đọc manifest gốc trực tiếp từ object git (`git cat-file blob HEAD:...`) để miễn nhiễm với khác biệt CRLF trên Windows, kiểm tra `seed_order` loại trừ `smoke_seed`, đối chiếu `effects.json` với `paired-report.json` và `per-seed-deltas.csv`, kiểm khóa `execution-lock.json`, từ chối `cargo run` trong `reproduction_command`, và cưỡng chế các invariant `E2-G1`, `E2-G3`, `E2-G4`, `E2-G6`.
- **`scripts/verify_e3_artifacts.mjs` (`npm run check:e3`) cùng `scripts/verify_esel1_artifacts.mjs` và `scripts/verify_sel2p_artifacts.mjs`:** thực hiện vai trò đối chiếu độc lập tương tự bằng JS cho các bộ bằng chứng E3 và chọn lọc sinh thái.
- **Cổng Rust phía backend (`e2_seam_tests.rs`, `prereg_e2_manifest_tests.rs`, `e2_frozen_manifest_tests.rs`):** thực thi kiểm tra `E2-G1`/`E2-G3`/`E2-G6` và bất biến `the_preregistered_reproduction_never_launches_the_app`, đồng thời nhân bản kiểm tra byte-identity của manifest vào thẳng `cargo test` (`e2_frozen_manifest_tests.rs`) sau sự cố một commit phía Rust làm gãy `check:e2` mà bộ test Rust vẫn báo xanh.

---

## 7. Nhóm 6 — Cổng tuân thủ giấy phép, vệ sinh repo và tài liệu

- **Giấy phép & chuỗi đóng gói thật (`check_bundle_closure.mjs`, `gen_third_party_licenses.mjs`, `gen_sbom.mjs`, `check_sbom_schema.mjs`, `gen_notice.mjs`, `verify_upstream_licenses.mjs`):** `npm ls` không nhìn thấy các gói thực sự bị bundler nhúng vào `dist/` (`vite`/`rolldown`) nhưng lại đếm cả những gói chỉ dùng lúc build; vì vậy `check_bundle_closure.mjs` đối chiếu trực tiếp `licensing/bundle-closure.json` do Vite/rolldown xuất từ module graph thật với bản trong git. `check_sbom_schema.mjs` kiểm chứng `sbom.cdx.json` offline bằng bộ schema CycloneDX ghim SHA-256 tại `schemas/cyclonedx/PROVENANCE.json` và đối chiếu với chỉ mục giấy phép; các artifact này được chạy trên Windows CI vì `cargo tree --target` chịu ảnh hưởng từ proc-macro của máy host ([`FINDINGS.md` (`F-025`)](../../FINDINGS.md#f-025--cargo-tree---target-x-không-độc-lập-host-proc-macro-được-resolve-theo-host-nên-artifact-licence-không-sinh-được-từ-máy-khác)). `gen_notice.mjs --check` chặn bản in `NOTICE` trôi về câu chữ "proprietary" cũ sau khi đổi giấy phép.
- **Vệ sinh văn bản & thư mục gốc (`check_text_hygiene.mjs`, `check_root_hygiene.mjs` + `root_allowlist.json`):** `check_text_hygiene.mjs` chặn ký tự điều khiển trần (như byte NUL từng vô tình bị dùng làm dấu phân cách trong `gen_sbom.mjs` và lặp lại trong `check_sbom_schema.mjs`, biến file `.mjs` thành diff `Bin` khiến công cụ review mù hoàn toàn). `check_root_hygiene.mjs` dùng danh sách cho phép hai chiều `root_allowlist.json` để chặn hàng loạt script tạm rơi vào thư mục gốc.
- **Cổng chống tài liệu ôi (`check_docs_links.mjs`, `check_docs_metadata.mjs`, `check_ipc_reference.mjs`):**
  - `check_docs_links.mjs` kiểm mọi đường dẫn tương đối và các `#anchor` cùng trang bằng bộ tạo slug hỗ trợ đầy đủ tiếng Việt có dấu (`\p{L}\p{N}`).
  - `check_docs_metadata.mjs` cưỡng chế YAML front matter, phân biệt trạng thái `completed` chỉ dùng trong `docs/ai/`, kiểm chỉ mục `README.md` mỗi thư mục và tính nhất quán của khối chỉ mục sinh.
  - `check_ipc_reference.mjs` kiểm tính khớp hai chiều giữa `docs/reference/IPC_CONTRACT.md` với danh sách `generate_handler!` và `.emit*(` trong mã Rust thật.

---

## 8. Giới hạn hiện tại và bằng chứng đang hoàn thiện

- **Những lỗ hổng cổng đã biết còn mở:** danh sách đầy đủ các phát hiện — gồm cả các lỗ hổng chưa vá hoàn toàn như `F-054` (heuristic tên chunk của `check_bundle_budget.mjs`) — được theo dõi minh bạch tại [`FINDINGS.md`](../../FINDINGS.md).
- **Bảng số đo và tài liệu bài báo kế tiếp:** các bảng đo hiện có trên nhánh thân cây nằm ở [`docs/paper/data/README.md`](../paper/data/README.md) (`determinism_matrix.md`, `scaling.md`, `eu_residual.md`). Danh mục sự cố lịch sử hoàn chỉnh (`P1`) và bảng đối chứng âm tổng hợp chứng minh từng cổng đỏ trên lỗi cài sẵn (`M2`) là các hạng mục **đang nằm trong kế hoạch** tại [`2026-11-05-swarm-master-plan.md`](../ai/planning/2026-11-05-swarm-master-plan.md), chưa có file riêng trên nhánh thân cây hiện tại.
