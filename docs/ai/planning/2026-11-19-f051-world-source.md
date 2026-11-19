---
phase: planning
feature: experiment-e-sel1-food-scarcity
kind: thẻ
title: Thẻ F051 (WORLD-SRC) — cô lập nguồn địa hình headless và ghi WorldIdentity vào provenance
description: Tách TerrainSource::Generated cho mọi mô phỏng headless khỏi ANIMA_WORLD_ARTIFACT và file tạm của app desktop đồng thời ghi terrain_source và world_identity vào RunProvenance cùng provenance.json của các runner.
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-19
review_cycle: weekly
---

# Thẻ F051 (`WORLD-SRC`) — Cô lập nguồn địa hình headless và ghi nhận `WorldIdentity`

## 1. Lỗi gốc (`PR167-trial-mode_c2e3ba69.md`)

- Trước thẻ này, `build_live_world` (`src-tauri/src/core/live_experiment.rs`) gọi `crate::core::ecs::init_world()`, hàm vốn đọc `ANIMA_WORLD_ARTIFACT` hoặc `<temp_dir>/anima_world_artifact.anmw` (artifact mà app desktop ghi mỗi lần chạy) và chỉ sinh địa hình từ `MapSettings::default()` khi không có file nào trên đĩa.
- Hệ quả: cùng một binary `run_esel1_experiment --trial` (hoặc `--smoke`, `--ensemble`, `run_sel2p_experiment`, `anima-lab`) chạy trên máy từng mở app desktop sẽ âm thầm chạy trên địa hình khác máy sạch (`1380775836` so với `1853767331` ở control seed `100001`, `1.200` tick), trong khi `provenance.json` giống nhau từng byte và không ghi nguồn địa hình lẫn `WorldIdentity`.

## 2. Thiết kế và phạm vi thay đổi

1. **Tách nguồn địa hình trong [`ecs.rs`](../../../src-tauri/src/core/ecs.rs):**
   - Thêm `pub enum TerrainSource { Generated, AppArtifactOrGenerated }` và `pub fn init_world_with(terrain_source: TerrainSource) -> World`.
   - `TerrainSource::Generated` sinh địa hình một lần trong tiến trình (`OnceLock`) từ `MapSettings::default()` (`TerrainMap::generate`), hoàn toàn không đọc biến môi trường hay file/cache tạm.
   - `pub fn init_world() -> World` giữ nguyên hành vi cho app desktop (`TerrainSource::AppArtifactOrGenerated`).
2. **Áp dụng cho thế giới live headless ([`live_experiment.rs`](../../../src-tauri/src/core/live_experiment.rs)):**
   - `build_live_world` gọi `init_world_with(TerrainSource::Generated)`.
   - `LiveExperimentAdapter` cung cấp `world_identity()` và cài đặt `ExperimentModel::terrain_source` (`Some("generated")`) + `ExperimentModel::world_identity`.
   - `live_run_provenance` / `run_live_drift_pair` ghi `terrain_source` và `world_identity` quan sát được vào `DriftPairReport` / `DriftEnsembleReport`.
3. **Truyền vào `RunProvenance` dùng chung ([`experiment_runner.rs`](../../../src-tauri/src/core/experiment_runner.rs), [`fork.rs`](../../../src-tauri/src/core/experiment_runner/fork.rs), [`branch_tree.rs`](../../../src-tauri/src/core/branch_tree.rs)):**
   - Thêm hai trường optional (tương thích ngược `serde(default, skip_serializing_if = "Option::is_none")`): `terrain_source: Option<String>` và `world_identity: Option<WorldIdentity>`.
   - `run_manifest_seed`, `checkpoint_fork` (cả `prefix`, `control`, `treatment` mang nguồn/danh tính thế giới cha) và `run_branch_tree` đều điền đủ hai trường khi model có địa hình.
4. **Ghi vào `provenance.json` của các runner headless ([`run_esel1_experiment.rs`](../../../src-tauri/examples/run_esel1_experiment.rs), [`run_sel2p_experiment.rs`](../../../src-tauri/examples/run_sel2p_experiment.rs)):**
   - Kiểm tra đồng nhất giữa mọi run (`uniform_world_provenance`) và xuất `"terrain_source"` + `"world_identity"` trong `--smoke`, `--trial`, `--ensemble` của `run_esel1_experiment` cũng như giai đoạn smoke/pipeline-check/confirm của `run_sel2p_experiment`.

## 3. Kiểm thử bổ sung

- [`src-tauri/tests/world_source_tests.rs`](../../../src-tauri/tests/world_source_tests.rs):
  - Một test duy nhất chứa mọi thao tác biến môi trường (với `EnvGuard` RAII khôi phục đầy đủ `ANIMA_WORLD_ARTIFACT`, `ANIMA_CACHE_DIR`, `TMPDIR`, `TEMP`, `TMP`):
    - So sánh `run_manifest_seed::<LiveExperimentAdapter>` (`180` tick) ở ba thiết lập trong cùng tiến trình: temp rỗng, `ANIMA_WORLD_ARTIFACT=artifacts/world_256.anmw`, và `TMPDIR`/`TEMP`/`TMP` chứa bản sao `anima_world_artifact.anmw` — `final_checksum`, `final_observables`, `terrain_source = "generated"`, và `world_identity` trùng khớp từng bit;
    - Đối chứng dương trên `ecs::init_world()` cho thấy đường dẫn app desktop vẫn nạp artifact khác hẳn địa hình sinh mặc định.
  - Test thuần kiểm tra `checkpoint_fork` và `run_branch_tree` ghi đầy đủ `terrain_source` và `world_identity` của thế giới cha.
- [`src-tauri/tests/esel1_trial_mode_tests.rs`](../../../src-tauri/tests/esel1_trial_mode_tests.rs):
  - Từ chối toàn bộ các seed xác nhận (`EXP_SEEDS` `[11, 12]` và đủ `12` seed `seeds.execution_order` trong `esel1-preregistration.json`), không chỉ seed đầu tiên.
  - Thêm `a_trial_output_files_contain_no_trait_mass_substring` xác nhận đủ `6` file đầu ra `--trial` không chứa chuỗi `trait_mass` và `provenance.json` ghi `"terrain_source": "generated"` cùng `world_identity` hợp lệ.

## 4. So sánh từng byte `run_esel1_experiment --smoke` giữa trunk và F051

Đo trên macOS (`macos/aarch64`), release profile `--features desktop`, chạy tuần tự với biến môi trường sạch (`ANIMA_WORLD_ARTIFACT` gỡ bỏ, `TMPDIR`/`TEMP`/`TMP` và `ANIMA_CACHE_DIR` trỏ vào thư mục rỗng mới, không có `anima_world_artifact.anmw`):

- Binary trunk (`4c6528f4`, sha256 `bd773c2119986a6f9e4cec5df374373c3c8014a0bc5b5fdaf72639e1eac2b772`):
  `control=4182711911`, `treatment=331712764`, `drift=2153699540` (`max 205.0 us/tick`).
- Binary F051 (`sha256` `7632c3a4c8051ee67be3a43cc69d60a56e33bed306a0502da276e19034a0aeb2`):
  `control=4182711911`, `treatment=331712764`, `drift=2153699540` (`max 206.5 us/tick`).

| File trong `smoke/` | Kết quả so sánh trunk vs F051 |
|---|---|
| `smoke-series.csv` | Khớp từng byte (`5.998` bytes) |
| `runs.csv` | Khớp từng byte sau khi bỏ cột thời gian `us_per_tick` |
| `smoke-report.json` | Khớp JSON sau khi bỏ `us_per_tick` và `max_us_per_tick` |
| `summary.md` | Khớp nguyên văn sau khi bỏ dòng `max us/tick` |
| `provenance.json` | Khớp JSON sau khi bỏ `timing`, `build` (`binary_sha256`/git) và hai trường mới cố ý thêm: `"terrain_source": "generated"`, `"world_identity"` (`seed=1337, generator_version=1, 256x256, checksum=1212187560` trên macOS) |

## 5. Tài liệu & trạng thái tích hợp

- Cập nhật [`docs/ai/planning/2026-11-19-esel1-runner-trial-mode.md`](2026-11-19-esel1-runner-trial-mode.md) và [`docs/paper/data/determinism_matrix.md`](../../paper/data/determinism_matrix.md) (PR Draft PENDING).
- Không sửa `docs/ai/README.md` (chủ/lead cập nhật mục lục qua `npm run gen:docs-index` khi hợp nhất).
