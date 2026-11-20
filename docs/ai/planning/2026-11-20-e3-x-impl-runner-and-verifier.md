---
phase: planning
feature: experiment-e3-productivity-gradient
title: Thẻ W13 (E3-X-impl) — Bộ chạy E3-X và Trình kiểm chứng Độc lập
description: Triển khai run_e3_experiment.rs, verify_e3_artifacts.mjs, bộ kiểm thử Rust/Vitest cho E3-X và ghi chính xác các lệnh chạy W14 trên Windows 1 (không chạy seed thí nghiệm ở W13)
status: active
owner: anima-case
last_reviewed: 2026-11-20
---

# Thẻ W13 (`E3-X-impl`) — Bộ chạy `run_e3_experiment` và Kiểm chứng Độc lập `verify_e3_artifacts.mjs`

- **Phạm vi W13:** xây dựng runner (`src-tauri/examples/run_e3_experiment.rs`), verifier (`scripts/verify_e3_artifacts.mjs`), tích hợp `package.json` (`check:e3`), cùng các bộ kiểm thử đơn vị và tích hợp (`src-tauri/tests/e3_runner_tests.rs`, `tests/frontend/verifyE3Artifacts.test.ts`). **Không** chạy bất kỳ hạt giống thí nghiệm nào (`730001..741001`) và **không** chạy lại `e3_smoke` ở W13.
- **Tài liệu gốc đóng băng:**
  - `src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json`
  - `docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md`

## 1. Thành phần triển khai ở W13

1. **`src-tauri/examples/run_e3_experiment.rs`** (`--ensemble`, `--replay`, `--self-test`):
   - Chỉ chạy ở `--release` (từ chối build debug cho `--ensemble` và `--replay`).
   - Từ chối trước khi chạy bất kỳ tick nào nếu `ANIMA_WORLD_ARTIFACT` đang được đặt, tệp `<temp_dir>/anima_world_artifact.anmw` tồn tại, thư mục đầu ra khác rỗng, hoặc thiếu hồ sơ `e3_smoke` `--release` hợp lệ khi `OS/arch` hoặc `git_commit` khác bản smoke gốc (`windows/x86_64` tại `9958f27`).
   - Sử dụng `ObservableRegistry::live_default()`, `FactorDiff`, `run_manifest_seed` (và `run_paired_ensemble_with_control` trong tự kiểm tra), kiểm tra `terrain_source` và `world_identity` ngay sau mỗi lượt chạy, dừng ngay lập tức và ghi trạng thái dừng vào `provenance.json` + `runs.csv` nếu hai lượt bất kỳ lệch `world_identity`.
   - Xuất bộ artifact tất định (`runs.csv`, `series.csv`, `final-observables.json`, `effects.json`, `provenance.json`, `summary.md`, `manifests/*`, `checksums.sha256`), ghi danh sách `git_dirty_paths` (không chỉ boolean), bản ghi kiểm tra trước khi chạy `ANIMA_WORLD_ARTIFACT`, tham chiếu smoke, toàn bộ phân tích tương phản, hiệu chỉnh Bessel `var * n / (n - 1)`, cửa sổ đuôi `[14400, 18000]`, loại trừ per-contrast (`extinction_rule`, `min_agent_count_rule`, `empty_half_rule`), phân rã cộng tính, và `decision_enum` đọc trực tiếp từ `e3-preregistration.json`.
   - Thời gian tường (`wall_clock_seconds`) chỉ ghi riêng vào `timing.json`, không đưa vào `checksums.sha256`.

2. **`scripts/verify_e3_artifacts.mjs` & `npm run check:e3`**:
   - Đọc mọi ngưỡng từ `e3-preregistration.json` (không hard-code ngưỡng), tự tính lại bằng `node:crypto` + Student-t95 của Rust (`statistics.rs`) toàn bộ tương phản chính/phụ, Bessel sensitivity, cửa sổ đuôi, S1–S6, I1, M1, loại trừ per-contrast, phân rã cộng tính, và nhãn `decision_enum`.
   - `--self-test` kiểm chứng các điều khiển âm: sửa giá trị (cả khi re-hash), xoá 1 lượt chạy, lệch `world_identity`, sửa ngưỡng trong bản sao `prereg`, và tuyệt chủng ở `control_uniform` chỉ loại tương phản chứa `U`.

3. **Kiểm thử**:
   - `src-tauri/tests/e3_runner_tests.rs`: dùng seed tách biệt hoàn toàn `612_001` / `612_002`, kiểm tra từ chối build debug, từ chối cờ sai / artifact env / thư mục không rỗng / thiếu smoke, dừng tức thì khi lệch `world_identity`, chạy ngắn `60` tick hoàn chỉnh và kiểm chứng đối chiếu replay.
   - `tests/frontend/verifyE3Artifacts.test.ts`: kiểm tra biên ranh giới quyết định tương xứng với `prereg_e3_manifest_tests.rs`, phân tích Bessel, S3 fallback `control_mean == 0.0`, loại trừ per-contrast 3 arm, và các điều khiển âm trên tệp.

## 2. Lệnh thực thi chính xác cho W14 (`E3-X-run` trên Windows 1 sau khi W13 được merge)

Chạy từ **bên trong thư mục checkout sạch** của Windows 1 tại commit trunk được chỉ định trong thông điệp `GO` (đảm bảo `ANIMA_WORLD_ARTIFACT` unset, `<temp_dir>/anima_world_artifact.anmw` không tồn tại, và chỉ một tiến trình `cargo` tại một thời điểm với `CARGO_BUILD_JOBS=6`):

```powershell
# (b) Chạy lại host smoke 1 lần ở --release vào thư mục riêng (dừng ngay nếu bất kỳ cổng WP-2 nào trượt):
$env:CARGO_BUILD_JOBS = "6"
cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example e3_smoke
src-tauri/target/release/examples/e3_smoke.exe `
  --manifest-dir src-tauri/tests/fixtures/experiments_e3 `
  --out artifacts/experiments/e3-productivity-gradient-body-mass/host-smoke

# (c) Build & chạy 36 lượt E3-X confirmation (--release, giữ nguyên tuần tự execution_order 12 seed):
cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example run_e3_experiment
src-tauri/target/release/examples/run_e3_experiment.exe --ensemble `
  --manifest-dir src-tauri/tests/fixtures/experiments_e3 `
  --smoke-dir artifacts/experiments/e3-productivity-gradient-body-mass/host-smoke `
  --out artifacts/experiments/e3-productivity-gradient-body-mass/confirmation

# (d) Kiểm chứng độc lập toàn bộ artifact, checksum, provenance và quyết định:
node scripts/verify_e3_artifacts.mjs artifacts/experiments/e3-productivity-gradient-body-mass/confirmation
```
