---
phase: planning
feature: real-world-and-bodies
title: Thẻ LAND1 — phân biệt đất và nước, hệ thống terrain_navigation và live.land_mask (prototype)
description: Bổ sung chính sách TerrainNavigation (DeepOcean/Ocean không đi được, River đi được), sinh thực thể trên đất liền, khoá opt-in live.land_mask giữ nguyên bit-identical khi tắt và bật mặc định ở desktop app
status: completed
kind: thẻ
owner: anima-dell
last_reviewed: 2026-11-19
---

# Thẻ LAND1 — Phân biệt đất/nước, `terrain_navigation_system` và khoá `live.land_mask`

> **Prototype cho tính năng đi trên đất liền của W1/W2 (`prototype, to be replaced` khi có định dạng World Artifact mới theo ADR-0010).**
> **Tác động quỹ đạo:** Desktop app (`SimulationEngine::start`) bật `TerrainNavigation { enabled: true }` theo mặc định nên **quỹ đạo của desktop app có chủ đích thay đổi** (sinh vật, cây, hồ và thức ăn bám đất liền thay vì đi/mọc giữa biển). Ngược lại, các **thực nghiệm headless (`live.*`) giữ nguyên bit-identical** với nhánh gốc khi `live.land_mask` vắng mặt hoặc bằng `0.0`.

- **Nhánh:** `dell/land1-terrain-navigation`
- **Commit code đã chạy toàn bộ test & gate:** `bd9f3c26ab14196996c6f17ac894a390cabd729f` (`CODE_SHA`, trước khi gộp #152/#157/#158) và `9e34b7bdefa789ad486e64d8858e295dbb8b0a42` (`MERGED_SHA` sau khi gộp `origin/feat/ecological-selection` `9967715cef632f7fa62d1f99ca6a2f6fe06af285` với 0 thay đổi dưới `src-tauri/src/`).

## 1. Thiết kế & phạm vi thay đổi (`AUTHORIZATION`)

1. **Chính sách và thành phần tại chỗ (`resources.rs`, `components.rs`, `evolution/genotype.rs`):**
   - `TerrainNavigation { pub enabled: bool }` (`Default` = `false`; vắng resource == tắt).
   - `LastWalkablePosition { pub pos: Vec3, pub revert_delta: Vec3, pub reverted_this_tick: bool }` gắn ngay khi `decode_genotype` tạo thực thể gốc; mọi cập nhật trong tick ghi đè tại chỗ, không thêm/xoá component, không cấp phát heap.
   - Do `LastWalkablePosition.pos` sau `wrap_coordinates_system` luôn trùng với `Position.0` đã quấn vòng và `revert_delta`/`reverted_this_tick` là vùng nháp nội bộ bị ghi đè ở mỗi lượt `terrain_navigation_system`, quá trình khôi phục snapshot tái tạo lại `LastWalkablePosition` qua `decode_genotype` mà không cần sửa schema `snapshot.rs` hay `simulation_state.rs`.
2. **Mô-đun điều hướng địa hình & lịch chạy (`core/terrain_navigation.rs`, `core/mod.rs`, `core/simulation_schedule.rs`):**
   - `is_biome_walkable`: `DeepOcean` (`0`) và `Ocean` (`1`) là không đi được; `River` (`3`) và toàn bộ biome đất liền (`2`, `4..=10`) đi được.
   - `terrain_navigation_system` được đăng ký ngay sau `integrate_physics_system` (trước `wrap_coordinates_system` và các hệ thống sinh thái trong `build_tick_schedule`).
   - Khi bật và root bước vào ô không đi được (theo `wrap_pos`), hệ thống đưa root về `LastWalkablePosition.pos` (hoặc `snap_to_nearest_walkable` nếu vị trí cũ cũng không đi được), đặt `Velocity` và `RigidBody.velocity` của root bằng `Vec3::ZERO`, và dịch đồng bộ mọi segment con mang `ParentAgent(root)` theo đúng `revert_delta` cùng vận tốc bằng `0` để khớp nối không bị xé ngang bờ biển — hoàn toàn với `0` cấp phát heap.
3. **Sinh thực thể trên đất liền không vẽ thêm RNG khi chính sách tắt:**
   - `world_systems.rs` (`spawn_food_system`): khi bật, dùng `sample_walkable_pos(..., MAX_WALKABLE_RESAMPLE_TRIES = 64)` từ `SimRng` và bỏ qua nếu cả 64 lần đều rơi vào biển.
   - `environmental_systems.rs` (`seed_dropping_system`): sau khi rút `(angle, dist)`, kiểm tra `is_pos_walkable`; nếu rơi vào biển khi bật thì bỏ qua lần rải hạt này mà **không rút thêm RNG**.
   - `agent_systems.rs` (`execute_reproduction_birth` & `apply_staggered_evolution_system`): khi bật và ô sinh bị lệch rơi vào biển, quay về vị trí đi được của bố/mẹ (`LastWalkablePosition` hoặc `snap_to_nearest_walkable`) mà **không rút thêm RNG**; `LastWalkablePosition` ban đầu nhận đúng toạ độ đi được đó.
4. **Khoá thực nghiệm & genesis desktop app (`live_experiment.rs`, `simulation_loop.rs`):**
   - `LIVE_KEY_LAND_MASK = "live.land_mask"` (`0.0`/`1.0`, mặc định `false`, `LIVE_KEYS` `25 -> 26`, cập nhật số đếm neo trong `e2_seam_tests.rs`, `eco_card_b_tests.rs`, `lake_layout_tests.rs`). Khi `1.0`, `genesis` đưa vị trí lattice của founder, cây và hồ về ô đi được gần nhất qua `snap_to_nearest_walkable`; khi `0.0` hoặc vắng mặt, hành vi và luồng RNG giữ nguyên từng bit.
   - `SimulationEngine::start`: bật `TerrainNavigation { enabled: true }` (prototype W1/W2 navigability); founder lấy tối đa `64` lần thử `sample_walkable_pos` kèm khoảng cách `>= 4.0` trên dòng `WORLD_INIT`, hồ chọn từ các ô đi được giáp nước (`is_walkable_cell_next_to_water`), cây lọc qua `is_biome_walkable`, các nhánh dự phòng đều neo bằng `snap_to_nearest_walkable`.

## 2. Kết quả kiểm chứng

- **Toàn bộ suite `--features desktop` (`CODE_SHA = bd9f3c26ab14196996c6f17ac894a390cabd729f`):**
  - `cmd /c "cargo test --features desktop > out.txt 2>&1"`: **`CARGO_TEST_EXIT=0`** (`3122.6s`, **159 target**, **1542 passed; 0 failed; 2 ignored** là 2 test phụ trợ allow-listed `child_entry_point` và `diagnose_which_system_moves_the_closed_total`).
  - `node scripts/check_test_targets.mjs src-tauri/out.txt --profile desktop`: **`CHECK_TARGETS_EXIT=0`** (`test targets: 159, empty: 3 (allow-listed 3), ignored: 2 (allow-listed 2), feature-gated running under "desktop": 8`).
- **Kiểm chứng trọng tâm & sau khi gộp trunk (`MERGED_SHA = 9e34b7bd`):**
  - `cargo fmt --check`: `exit 0`.
  - `cargo clippy --all-targets --features desktop -- -D warnings`: `exit 0` (`1m 03s` ở `bd9f3c26`; `56.66s` ở `9e34b7bd`).
  - `cargo clippy --all-targets --no-default-features -- -D warnings`: `exit 0` (`44.68s` ở `bd9f3c26`).
  - `python scripts/verify_dataflow.py dataflow.json dataflow-truth.dot 1`: `QUA CONG — 16 edge khop hoan toan voi cargo-modules` (`exit 0`).
  - `terrain_navigation` (lib unit tests): `3 passed; 0 failed`.
  - `land_mask_tests`: `11 passed; 0 failed` (`29.32s`):
    - `absent_and_zero_match_at_every_sampled_tick`: thế giới `default` (`ticks 100..=1000`) và `active` (`ticks 100..=1000`) giữa vắng mặt và `0.0` khớp nhau từng bit (`10/10` mốc cho mỗi thế giới; đồng thời `lake_layout_tests::absent_and_zero_match_every_sampled_tick_with_lakes_and_locomotion_on` đối chứng bit-identical 16/16 dòng `S12_TRAJ` với commit gốc `733e27da` trước merge).
    - `same_checksum_in_three_processes_with_land_mask_enabled`: `LAND1_CHECKSUMS 0x16c187b3 0x16c187b3 0x16c187b3` (3 tiến trình độc lập).
    - `closed_eu_unchanged_with_land_mask_enabled`: `LAND1_EU start=155587.24986362457 worst_relative_drift=0.00000000000000486350644116622` (`< 1e-6`).
    - `terrain_navigation_system_allocates_nothing_once_warm`: `0` heap allocation trên cả đường đi bình thường lẫn đường đẩy lùi bờ biển; `land_mask_keeps_agents_founders_trees_lakes_and_food_on_land_over_2000_ticks`, `reproduction_with_land_mask_keeps_all_roots_on_land_and_records_births` (`births > 0`), `coastal_birth_and_staggered_evolution_use_walkable_positions_without_extra_rng_draws`, `restore_reproduces_last_walkable_position_and_matches_continuing_run`, `desktop_app_genesis_places_lakes_on_land_next_to_water` (`MockRuntime`) đều đạt.
  - `prereg_sel2p_manifest_tests`: `8 passed; 0 failed` (`0.10s`, hậu merge `#152`).
  - Các suite hợp đồng liên quan: `schedule_ambiguity_tests` `6/6`, `schedule_total_order_tests` `4/4`, `emit_zero_alloc_tests` `1/1`, `locomotion_steering_zero_alloc_tests` `2/2`, `e2_seam_tests` `9/9`, `eco_card_b_tests` `12/12`, `lake_layout_tests` `6/6`, `persistence_tests` `2/2`, `f047_restore_divergence_tests` `5/5`.
