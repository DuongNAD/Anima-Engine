---
phase: planning
feature: ecological-selection
kind: thẻ
title: Thẻ FOUNDER-LAYOUT — Bố cục khởi sinh tuỳ chọn live.founder_layout, kiểm tra skip-zero SEL2-P và fallback verifier E3
description: Bổ sung khoá live.founder_layout (0/vắng = hàng ngang di sản, 1 = lưới 2-D đều trên đất đi được, 2 = rải ngẫu nhiên từ nhánh RNG riêng LIVE_GENESIS_FOUNDERS), bổ sung assert bỏ qua missed_births_capped khi bằng 0 và fallback sha256_as_committed cho verify_e3_artifacts.mjs khi shallow-clone thiếu commit gốc.
status: active
owner: anima-mac
last_reviewed: 2026-11-20
---

# Thẻ FOUNDER-LAYOUT — `live.founder_layout` & hoàn thiện ghi chú hậu kiểm SEL2P2-PREP

## 1. Bối cảnh và thiết kế bố cục khởi sinh (`live.founder_layout`)

- Chẩn đoán `DIAG-SEL2` chỉ ra toàn bộ khởi sinh (`live.founders`) trước đây bị xếp trên một đường thẳng ngang `z_fraction = 0.35` (`z = -30`), gây vắt kiệt cỏ cục bộ ngay những tick đầu.
- **FOUNDER-LAYOUT** bổ sung khoá tuỳ chọn `live.founder_layout` với `3` giá trị hợp lệ `{0.0, 1.0, 2.0}` (vắng/mặc định `0`), giữ nguyên nhánh di sản từng bit và tách luồng ngẫu nhiên khỏi `SimRng` / `LIVE_GENESIS_BRAINS`:
  1. **`0.0` hoặc vắng (legacy):** giữ nguyên `snap_if_masked(lattice_point(i, config.founders.max(1), bounds, 0.35))` và không rút bất kỳ số ngẫu nhiên nào.
  2. **`1.0` (`founder_grid_positions`):** đặt các khởi sinh thành lưới 2-D đều bên trong `MapBounds` (không rút RNG); khi bật `live.land_mask = 1.0`, lọc các nút lưới đi được qua `terrain_navigation::is_pos_walkable`, tìm độ phân giải thưa nhất đủ `n` điểm đi được sao cho khoảng cách hình xuyến cặp bất kỳ đạt `>= FOUNDER_MIN_SPACING` (`2.0` đơn vị thế giới), rồi lấy mẫu phân tầng đều; trên đảo cực nhỏ tự động dùng tâm ô đất `walkable_cell_centers` với hạ ngưỡng khoảng cách có kiểm soát.
  3. **`2.0` (`founder_scatter_positions`):** dùng bộ sinh riêng [`sim_stream::LIVE_GENESIS_FOUNDERS = 5`](../../../src-tauri/src/core/resources.rs) qua `derived_rng(seed, LIVE_GENESIS_FOUNDERS)` nên không bao giờ dịch vị trí `SimRng` hay `brain_rng`. Mỗi điểm lấy mẫu ngẫu nhiên trong `MapBounds`, kiểm tra `is_pos_walkable` (khi `land_mask = 1.0`) và khoảng cách hình xuyến `>= FOUNDER_MIN_SPACING` (`2.0`) với dự phòng xáo trộn Fisher–Yates các ô đất đi được nếu mật độ cao / đảo nhỏ.

## 2. Hai điểm bổ sung theo ghi chú `SEL2P2-PREP` & phạm vi uỷ quyền

- **Ghi chú 3(a) (`src-tauri/tests/prereg_sel2p_manifest_tests.rs`):** bổ sung assertion kiểm tra `smoke/smoke-report.json["drift_diagnostics"]` và mọi phần tử `effects.json["drift_diagnostics_by_seed"][i]["diagnostics"]` không chứa khoá `"missed_births_capped"` khi bằng `0` trong `--pipeline-check` (chặn đột biến `MR1`).
- **Ghi chú 3(b) (`scripts/verify_e3_artifacts.mjs`):** trước đây `resolveManifestGitRef` rơi về `'HEAD'` khi `build.git_commit` không có trong clone cục bộ và so sánh 4 bản sao `manifests/*` với `'HEAD'`, khiến shallow clone (`--depth 1`) thất bại trên `#181` sau các tu chỉnh kế hoạch E3. Nay khi `recordedCommit` không sẵn có cục bộ, bộ kiểm chứng đọc cấu hình từ `manifests/e3-preregistration.json` trong gói artifact và đối chiếu SHA-256 chuẩn hoá LF của cả 4 tệp với `provenance.json["manifests"]["sha256_as_committed"]` (kèm ghi chú tường minh và các ca âm/dương trong `--self-test`).
- **Chốt khoá & E3:** nâng `LIVE_KEYS` từ `28 -> 29` trong `live_experiment.rs` và 6 tệp kiểm thử chốt số khoá (`e2_seam_tests.rs`, `eco_card_b_tests.rs`, `lake_layout_tests.rs`, `land_mask_tests.rs`, `strict_prey_cap_tests.rs`, `grass_steering_tests.rs`); thêm `"live.founder_layout": 0` vào `default_keys_left_absent` (`e3-preregistration.json`) và 1 dòng tu chỉnh `2026-11-20` cuối kế hoạch E3.

## 3. Kiểm chứng, số đo tham khảo và 6 đột biến (`ML1`–`ML5`, `MR1`)

- **`src-tauri/tests/founder_layout_tests.rs` (8/8):** kiểm tra đăng ký/validation/founders=0, tương đương từng bit `absent == 0.0` (có và không `land_mask`), cách ly hoàn toàn `SimRng`/`AgentBrain`/cây/hồ qua cả 3 chế độ `0/1/2`, kiểm tra lưới `1` (cả quần đảo & đảo nhỏ `6x6`) và rải ngẫu nhiên `2` (cả bản đồ `40x40` 80 khởi sinh & 80% đại dương), failure ồn ào khi `walkable_cells == 0` hoặc `walkable_cells < founders` (cộng biên `4 == 4`), định thức đa tiến trình + bảo toàn EU kín (`< 1e-6`), và chẩn đoán 500 tick SEL2-P (`100` khởi sinh, `trees=8`, `scale=0.63`, không chốt số; số đo minh hoạ trên máy Mac `arm64`, không phải chuẩn cross-arch):
  - `mode=0`: `agents=100`, `standing_crop=95784.74`, `checksum=0x6c217ba7`
  - `mode=1`: `agents=100`, `standing_crop=95768.09`, `checksum=0x2aa28ba8`
  - `mode=2`: `agents=100`, `standing_crop=95722.15`, `checksum=0x9999c80e`
- **6 đột biến cục bộ đều bị bắt (`exit = 101`) và hoàn nguyên sạch:**
  - `ML1` (bỏ qua `live.founder_layout`): 3 test trong `founder_layout_tests` fail (`:330`, `:399`, `:500`).
  - `ML2` (bỏ lọc `land_mask` trong mode `1`): fail tại `founder_layout_tests.rs:137`.
  - `ML3` (bỏ lọc `land_mask` trong mode `2`): fail tại `founder_layout_tests.rs:137`.
  - `ML4` (bỏ `FOUNDER_MIN_SPACING` trong mode `2`): fail tại `founder_layout_tests.rs:518`.
  - `ML5` (rút ngẫu nhiên mode `2` chạm `SimRng`): fail tại `founder_layout_tests.rs:322`.
  - `MR1` (luôn ghi `missed_births_capped` dù bằng 0): fail tại `prereg_sel2p_manifest_tests.rs:1031`.

## 4. Follow-up `r1`

- **`scripts/verify_e3_artifacts.mjs`:** khi `provenance.build.git_commit` không có cục bộ (shallow depth-1 clone), ngoài kiểm `provenance.manifests.sha256_as_committed`, đối chiếu chéo `manifests/e3-preregistration.json` với `manifests/e3-preregistration.json` trong working-tree/`HEAD` và chỉ chấp nhận khác biệt nếu là khóa mới thêm vào `default_keys_left_absent` mang giá trị `0`; đồng thời đối chiếu 3 manifest nhánh với bản trong working-tree/`HEAD`. Khi fallback được kích hoạt, dòng cuối trên stdout phân biệt rõ: `verify_e3_artifacts OK (fallback: commit <sha8> unavailable; preregistration matched HEAD up to added default-0 keys)`.
- **`tests/frontend/verifyE3Artifacts.test.ts` & `--self-test`:** kiểm chứng dòng fallback OK qua CLI và thêm negative control giả mạo nhất quán (`thresholds.dz_min`: `0.8 -> 0.5` kèm cập nhật `effects.json`, `sha256_as_committed` và `checksums.sha256`) đều bị từ chối ở chế độ shallow depth-1.
- **`src-tauri/src/core/live_experiment.rs` & `founder_layout_tests.rs`:** `live.founder_layout = 1 | 2` dưới `live.land_mask = 1` trả về `ExperimentError::InvalidPopulation` nêu rõ số founder và số ô đi bộ được khi `walkable_cells == 0` hoặc `walkable_cells < founders` (không còn đặt trên nước hay chồng lấn); tách `try_apply_live_genesis_and_policies` và bỏ nhánh fallback im lặng về `unmasked_grid_positions` khi có `TerrainMap`.
