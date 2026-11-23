---
phase: planning
feature: ecological-selection
kind: thẻ
title: Thẻ FOUNDER-2SEG — Khai báo tuỳ chọn cá thể sáng lập 2 đốt (`live.founder_genotype`)
description: Thêm khoá manifest live.founder_genotype (0/vắng = sáng lập 3 đốt di sản trùng từng bit; 1 = sáng lập 2 đốt giảm đốt đuôi cho E3b D5 phương án b), nâng LIVE_KEYS lên 31 và ghi nhận ngưỡng toán tử 4 của mutate_genotype.
status: active
owner: anima-case
last_reviewed: 2026-11-23
---

# Thẻ FOUNDER-2SEG — Tuỳ chọn cá thể sáng lập 2 đốt (`live.founder_genotype`)

## 1. Bối cảnh và đặc tả khoá (`D5` phương án `(b)`)

Theo quyết định `E3b D5` (`MSG_20261123_140000`), nhánh E3b dùng cá thể sáng lập 2 đốt khai báo tường minh để không trộn xu hướng thích nghi cấu trúc `3.3 -> 2.5` vào tương phản `T - S`:

- **Tên khoá:** `LIVE_KEY_FOUNDER_GENOTYPE = "live.founder_genotype"` (`LIVE_KEYS.len()`: `30 -> 31`), cấu hình [`LiveWorldConfig::founder_genotype`](../../../src-tauri/src/core/live_experiment.rs) (`#[serde(default)]`, mặc định `0`).
- **Giá trị hợp lệ (`from_initial_conditions`):**
  - **`0.0` hoặc vắng mặt (mặc định):** giữ nguyên cá thể sáng lập 3 đốt di sản (node `[0, 1, 2]`, khối lượng `[1.5, 1.0, 0.8]`, cạnh `0 -> 1` và `1 -> 2`; tổng khối lượng `3.3`; SHA-256 `serde_json` chuẩn `9067cbf53fd6dbb561e99c1b1f690b2a1633e56522499309793b788354f5c808`), trùng từng bit và không tiêu thụ thêm RNG.
  - **`1.0`:** cá thể sáng lập 2 đốt cắt bỏ đốt đuôi (node `0` khối lượng `1.5`, node `1` khối lượng `1.0`, cạnh `0 -> 1` giữ nguyên `length = 1.0`, `radius = 0.2`, `joint_anchor = [1.0, 0.0, 0.0]`, `joint_axis = [0.0, 0.0, 1.0]`; tổng khối lượng `2.5`).
  - Mọi giá trị hữu hạn khác bị từ chối bằng `ExperimentError::OutOfRange { min: 0.0, max: 1.0 }`; giá trị không hữu hạn (`NaN`/`±Inf`) bị từ chối bằng `ExperimentError::NotFinite`.
- **SHA-256 chuẩn của `MorphologyGenotype` 2 đốt (`serde_json::to_vec`):**
  - JSON chuẩn: `{"nodes":[{"id":0,"length":1.0,"radius":0.2,"mass":1.5},{"id":1,"length":1.0,"radius":0.2,"mass":1.0}],"edges":[{"source_node":0,"target_node":1,"joint_anchor":[1.0,0.0,0.0],"joint_axis":[0.0,0.0,1.0]}]}`
  - SHA-256: **`dfeb276205f818acf3f34f5d716cbf163598a03a23ffc6964ff85753e280c719`**.

## 2. Tuân thủ vòng đời phát triển & tường toán tử 4 (`CLAUDE.md` / `ADR-0001`)

- **Phát triển một lần lúc `genesis`:** [`live_experiment::genesis`](../../../src-tauri/src/core/live_experiment.rs) chỉ chọn `MorphologyGenotype` khi khởi sinh thế giới mới. Đường restore (`build_live_world` nhánh `Some(state)` -> `spawn_serialized_agent`) đọc nguyên `SerializedAgent.genotype` đã lưu và **không** quay lại `genesis` hay gieo lại hình thái.
- **Công bố giới hạn toán tử 4 (`mutate_genotype`):** trong [`src-tauri/src/evolution/mutation.rs`](../../../src-tauri/src/evolution/mutation.rs), toán tử xoá đốt (operator 4) yêu cầu `genotype.nodes.len() > 2`. Cá thể sáng lập 2 đốt nằm ngay trên rào này, nên đột biến cấu trúc chỉ có thể **thêm** đốt chứ không rút xuống 1 đốt.

## 3. Các tệp uỷ quyền đã sửa (`FOUNDER-2SEG`)

- [`src-tauri/src/core/live_experiment.rs`](../../../src-tauri/src/core/live_experiment.rs): hằng khoá `LIVE_KEY_FOUNDER_GENOTYPE`, `LIVE_KEYS` (`31` phần tử), trường `LiveWorldConfig::founder_genotype`, kiểm tra hợp lệ `{0.0, 1.0}`, và rẽ nhánh `MorphologyGenotype` trong `genesis`.
- [`src-tauri/tests/founder_genotype_tests.rs`](../../../src-tauri/tests/founder_genotype_tests.rs): 4 test kiểm chứng đăng ký/validation/serde, tương đẳng `absent == 0.0`, hình thái + số segment ECS + SHA-256 giá trị `1.0`, và khôi phục snapshot không re-roll.
- 7 tệp test cập nhật `LIVE_KEYS.len()` (`30 -> 31`): [`e2_seam_tests.rs`](../../../src-tauri/tests/e2_seam_tests.rs), [`eco_card_b_tests.rs`](../../../src-tauri/tests/eco_card_b_tests.rs), [`founder_layout_tests.rs`](../../../src-tauri/tests/founder_layout_tests.rs), [`grass_steering_tests.rs`](../../../src-tauri/tests/grass_steering_tests.rs), [`lake_layout_tests.rs`](../../../src-tauri/tests/lake_layout_tests.rs), [`land_mask_tests.rs`](../../../src-tauri/tests/land_mask_tests.rs), [`strict_prey_cap_tests.rs`](../../../src-tauri/tests/strict_prey_cap_tests.rs).
- [`src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json`](../../../src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json): khai báo `"live.founder_genotype": 0` trong `default_keys_left_absent`.
- [`docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md`](2026-08-26-experiment-e3-productivity-gradient-body-mass.md): nối 1 dòng tu chỉnh ngày `2026-11-23 (FOUNDER-2SEG)`.

## 4. Kết quả kiểm chứng

- **`founder_genotype_tests` + 8 test liên quan (khởi chạy cục bộ):** 9/9 target đạt (`e2_seam_tests` 9/9, `eco_card_b_tests` 12/12, `founder_genotype_tests` 4/4 trong `3.86s`, `founder_layout_tests` 8/8, `grass_steering_tests` 7/7, `lake_layout_tests` 6/6, `land_mask_tests` 14/14, `prereg_e3_manifest_tests` 12/12, `strict_prey_cap_tests` 5/5).
- **Toàn bộ suite desktop (`cargo test --features desktop --no-fail-fast`):** **`172` target, `1,640` passed, `0` failed, `2` allow-listed `#[ignore]`**.
- **Cổng danh sách target (`node scripts/check_test_targets.mjs ... --profile desktop`):** exit **`0`** — `test targets: 172, empty: 4 (allow-listed 4), ignored: 2 (allow-listed 2), feature-gated running under "desktop": 8`.
- **Định dạng & linter (`cargo fmt --check` & `cargo clippy --features desktop --lib --test founder_genotype_tests -- -D warnings`):** exit **`0`** (`0` cảnh báo).
- **Tài liệu & vệ sinh kho mã (`node scripts/check_*.mjs`):** `check_docs_links` (`2004` liên kết tương đối + `99` neo, `0` lỗi), `check_docs_metadata` (`201` tệp, `0` lỗi, `7` hàng chờ lập chỉ mục `docs/ai/README.md`), `check_root_hygiene` (`64` mục gốc hợp lệ), `check_text_hygiene` (`1218` tệp theo dõi, không byte điều khiển thô).
