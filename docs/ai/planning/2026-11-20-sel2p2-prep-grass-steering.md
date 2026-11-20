---
phase: planning
feature: ecological-selection
kind: thẻ
title: Thẻ SEL2P2-PREP — Khôi phục opt-in live.grass_steering và đồng bộ missed_births_capped cho runner SEL2-P
description: Cổng hoá tuỳ chọn live.grass_steering với thứ tự ưu tiên khát -> Food cục bộ -> cỏ (LAND1/S10), bổ sung missed_births_capped vào drift_diag_json của runner SEL2-P, và giữ nhánh di sản trùng khớp từng bit.
status: active
owner: anima-mac
last_reviewed: 2026-11-20
---

# Thẻ SEL2P2-PREP — `live.grass_steering` opt-in & trường `missed_births_capped` cho runner SEL2-P

## 1. Bối cảnh và thiết kế cổng hoá (`origin/dell/s11-grass-steering`: `226d4b2`, `95044c4b`)

- Chẩn đoán `verdicts/DIAG-SEL2.md` chỉ ra trong cấu hình SEL2-P thiếu thức ăn, gặm mù (`steering_drive_system` chỉ tìm `Food` và `Lake`) làm thú ăn cỏ vắt kiệt ô cỏ dưới chân trong khi 95–99% bản đồ vẫn đầy cỏ chỉ cách chưa tới một ô.
- Thay vì merge nhánh S11 cũ (`226d4b2` / `95044c4b`, đã đóng tại `verdicts/S11_95044c4b.md`), **SEL2P2-PREP** port sạch khoá `live.grass_steering` lên trunk hiện tại đã có `LAND1`, giữ nguyên nhị phân mặc định cho nhánh tắt/vắng (`absent == 0.0 == pre-SEL2P2`):
  1. **Tài nguyên chính sách riêng ([`GrassSteeringPolicy`](../../../src-tauri/src/core/resources.rs)):** đặt `enabled: bool` tách khỏi `LocomotionPolicy` để không làm đổi cấu trúc literal ở các bài kiểm thử khác; mặc định `false`.
  2. **Đăng ký khoá & kiểm tra tiền điều kiện ([`live_experiment.rs`](../../../src-tauri/src/core/live_experiment.rs)):** `LIVE_KEY_GRASS_STEERING = "live.grass_steering"`, nâng `LIVE_KEYS` lên `28` khoá, nhận chặt `{0.0, 1.0}` (từ chối `OutOfRange` / `NotFinite`), từ chối `InvalidLaw` khi `grass_steering = 1.0` mà `live.locomotion != 1.0`, và chèn `GrassSteeringPolicy` trong `apply_live_genesis_and_policies`.
  3. **Thứ tự ưu tiên rõ ràng trong `steering_drive_system` ([`locomotion.rs`](../../../src-tauri/src/core/locomotion.rs)):** không cấp phát heap, không sửa hành vi thú săn mồi hay nhánh di sản:
     - **Ưu tiên 1 — Khát (> 50% thiếu nước):** `thirst_steering && hydration < 0.50 * hydration_target && !lake_cache.is_empty()` -> hướng tới `Lake` có nước gần nhất.
     - **Ưu tiên 2 — `Food` cục bộ:** nếu có `Food` trong bán kính cục bộ `(GRASS_SCAN_RADIUS + 0.5) * max(cell_w, cell_h)`, hướng tới `Food` gần nhất trước khi xét cỏ.
     - **Ưu tiên 3 — Cỏ `ResourceField` (S10 `PlantReservePolicy` + LAND1 `TerrainNavigation`):** nếu ô hiện tại đi được (`is_pos_walkable` khi `TerrainNavigation::enabled`) và còn cỏ khả dụng `>= GRASS_STAY_FRACTION * r_max` (`0.25`, `r_max > 0`), đứng yên gặm (`Vec3::ZERO`); ngược lại quét cửa sổ cố định `(2 * GRASS_SCAN_RADIUS + 1)^2` (`7x7`, `GRASS_SCAN_RADIUS = 3`) đã cắt biên lưới, bỏ qua ô không đi được khi bật LAND1, chọn ô đi được có lượng cỏ khả dụng `r - floor` cao nhất vượt hẳn ô hiện tại (hoà chọn `(row, col)` nhỏ nhất), và rơi về `Food` toàn cục gần nhất nếu không có ô nào hơn.

## 2. Đồng bộ runner SEL2-P, phân hoạch E3 và danh sách file uỷ quyền

- `src-tauri/examples/run_sel2p_experiment.rs`: bổ sung `"missed_births_capped"` vào `drift_diag_json` chỉ khi `d.missed_births_capped > 0` (giữ artifact hiện có và nhánh `0` trùng từng byte).
- 5 tệp kiểm thử chốt `LIVE_KEYS.len()` (`27 -> 28`): `e2_seam_tests.rs`, `eco_card_b_tests.rs`, `lake_layout_tests.rs`, `land_mask_tests.rs`, `strict_prey_cap_tests.rs`.
- E3: thêm `"live.grass_steering": 0` vào `default_keys_left_absent` (`e3-preregistration.json`) và nối 1 dòng tu chỉnh `2026-11-20` vào kế hoạch E3.

## 3. Kiểm chứng, số đo tham khảo và 5 đột biến (`MG1`–`MG5`)

- **`src-tauri/tests/grass_steering_tests.rs` (7/7):** bao phủ đăng ký/validation/preflight refusal, `absent == 0.0` qua 1.000 tick (kèm LAND1 500 tick), thứ tự ưu tiên + S10 + LAND1 + thú săn mồi bất biến, dịch chuyển khỏi ô cạn trong 240 tick (trái ngược đối chứng tắt), cấp phát heap bằng `0` khi ấm kèm LAND1, đối chiếu checksum + bảo toàn EU kín (`< 1e-6`), và so sánh mật độ cao 100 khởi sinh trong 500 tick (không chốt cứng hằng số):
  - Thuần gặm cỏ (`trees = 0, food_cap = 0`): lượng hấp thu/con tăng từ `1.988665` lên `8.122672` (+308%), năng lượng cuối `95.75 -> 98.71`, tỷ lệ mắc kẹt trên ô cạn giảm `1.0000 -> 0.5700`.
  - Có cây trái (`trees = 8, food_cap = 50`, khởi sinh đói `0.4 * target`): tỷ lệ mắc kẹt ô cạn giảm `0.8200 -> 0.7600`; tổng `total_energy_gained` ngắn hạn 500 tick gồm cả trái cây `Food` 20 EU ở xa (`15.53` vs `16.78`) là ghi nhận thực nghiệm cho pha thăm dò SEL2-P2.
- **Tương đương di sản:** `verify_sel2p_artifacts.mjs` và `--self-test` đạt; E-SEL1 `--smoke` bản release (trunk `eedf577` vs PR) trùng khớp từng byte trên `smoke-series.csv` (`5998 B`) và `summary.md` (`405 B`), trùng toàn bộ `final_checksum` (`4182711911`, `331712764`, `2153699540`).
- **5 đột biến cục bộ `MG1`–`MG5` (`locomotion.rs`):** đều bị `grass_steering_tests` bắt với `exit = 101` (`MG1` tắt khoá -> 3 test fail; `MG2` bỏ `!steering_to_water` -> `:316`; `MG3` bỏ ưu tiên Food cục bộ -> `:326`; `MG4` bỏ `is_walkable` LAND1 -> `:534`; `MG5` bật cưỡng bức trên di sản -> 2 test fail) và hoàn nguyên sạch sẽ.
