---
title: ADR-0009 — Nâng Burn lên 0.22.1, dùng burn-flex và cô lập stream WGPU liên luồng (QĐ-C, QĐ-R1c-a, QĐ-R1c-b)
status: proposed
owner: architecture
last_reviewed: 2026-11-18
decision_date: 2026-11-18
supersedes: none
superseded_by: none
---

# ADR-0009 — Nâng Burn lên 0.22.1, dùng `burn-flex` và cô lập stream WGPU liên luồng (`QĐ-C`, `QĐ-R1c-a`, `QĐ-R1c-b`)

> **Trạng thái: đề xuất (`proposed`), chờ phê duyệt của chủ dự án sau khi tháo các điểm chặn R5 ngoài phạm vi uỷ quyền thẻ R1c.**
> Phần mã nguồn, test, di trú snapshot và ADR này được thực hiện trên nhánh `win/r1c-burn-upgrade` (từ `origin/feat/ecological-selection`). Toàn bộ phép đo bên dưới lấy trực tiếp trên Windows 11 x64 + RTX 5060 Ti cùng một cấu hình vừa kiểm chứng.

## Bối cảnh

1. **Chặn phát hành pháp lý từ `hexf-parse` 0.2.1.** Theo [sổ quyết định](../planning/COMPLETION_ROADMAP_2026-10-31.md) và `licensing/UNRESOLVED.md`, crate `hexf-parse` 0.2.1 khai báo `CC0-1.0` trên crates.io nhưng không từng phát hành tập tin văn bản giấy phép tại commit phát hành `4225763d744183d720f575ae96d04161b4d08ea0`, nên chặn cổng `check:licenses-complete`. Nó chỉ đi vào cây phụ thuộc qua `ml-wgpu -> burn-wgpu 0.13.2 -> wgpu 0.19.4 -> naga 0.19.2 -> hexf-parse 0.2.1`.
2. **Chuyển lối QĐ-C = C.** Ngày 2026-11-17, chủ dự án quyết định giữ tuỳ chọn GPU trong bản build (`QĐ-C = C`) bằng cách nâng `burn` và `burn-wgpu` từ `0.13.2` lên dòng `0.22` (`0.22.1`), qua `cubecl-wgpu 0.11.4 -> wgpu 30.0.2 -> naga 30.0.2` (bộ đọc số thực thập lục phân nội tuyến với ghi chú `0BSD`, không kéo `hexf-parse`).
3. **Khai tử `burn-ndarray` và mất tương thích nhị phân `BinBytesRecorder`.** Như đối chiếu ở [tài liệu nghiên cứu Burn 0.22](../ai/planning/2026-11-18-research-burn-and-determinism.md), `burn-ndarray` bị khai tử từ Burn 0.21 và bị xoá ở nhánh 0.23, đồng thời Burn 0.22 bỏ `BinBytesRecorder` và tham số kiểu backend ở mã người dùng. Ngày 2026-11-18 chủ dự án quyết định thêm `QĐ-R1c-a` và `QĐ-R1c-b`.

## Quyết định (thẻ R1c)

### 1. Nâng phụ thuộc Burn `0.22.1` và `burn-flex` (`QĐ-C`, `QĐ-R1c-b`)

- Nâng `burn = { version = "0.22.1", features = ["flex", "simd", "rayon"] }` và `ml-wgpu = ["burn/wgpu"]` trong `src-tauri/Cargo.toml`, loại bỏ `burn-ndarray`, `hexf-parse`, `d3d12`, `gix-fs`, `gix-features`, `libsqlite3-sys` và `burn 0.13` khỏi `src-tauri/Cargo.lock`.
- Không bật `cpu` (`CubeCL JIT`), không bật BLAS, và không dùng `Device::default()` vốn ưu tiên GPU nếu tính năng WGPU được biên dịch. Toàn bộ đường CPU chỉ dùng `Device::flex()`.
- Giữ nguyên bố cục trọng số phẳng `w[out * fan_in + in]` trong `ActorCriticModel::from_flat_weights` / `to_flat_weights` ([ADR-0003](ADR-0003-evolved-per-agent-brains.md), gate parity `EB-S02`) và một hàm mục tiêu A2C duy nhất (`simulation_loop::a2c_loss`, hệ số actor `+td`).
- Gỡ bỏ ba mục miễn trừ advisory trong `src-tauri/.cargo/audit.toml` mà crate của chúng đã rời `Cargo.lock` (`RUSTSEC-2024-0350`, `RUSTSEC-2025-0021`, `RUSTSEC-2026-0459`), giữ lại 8 mục của `zip 0.6.6` / `zstd-safe` / `zstd-sys` (`RUSTSEC-2026-0367`, `0368`, `0369`, `0403`, `0410`, `0429`, `0545`, `0548`) do còn nằm trong cây lockfile ở các feature Burn chưa bật.

### 2. Nâng schema snapshot 7 lên 8 và migration save cũ (`QĐ-R1c-a`)

- Tăng `SCHEMA_VERSION` trong `src-tauri/src/core/snapshot.rs` từ `7` lên `8`. Định dạng mới lưu learner dùng chung bằng burnpack `ModuleRecord` / `OptimizerRecord` (`burn::store`) và round-trip chính xác.
- Snapshot schema 7 (ghi bằng `BinBytesRecorder` của Burn 0.13.2) vẫn khôi phục nguyên vẹn toàn bộ thế giới, nhưng hai bản ghi learner (`training_model_record`, `optimizer_record`) bị từ chối với lỗi định danh `LearnerRecordError::Schema7Burn013Unsupported` ghi log cảnh báo và learner dùng chung khởi tạo lại từ đầu.
- Kèm fixture thực sinh trước khi nâng cấp từ Burn `0.13.2` tại `src-tauri/tests/fixtures/snapshots/schema7_burn013_shared_learner.json` và bộ test `schema7_learner_migration_tests`.

### 3. Cô lập stream WGPU giữa luồng learner và luồng inference

- **Phát hiện thực nghiệm trong R1c:** Burn 0.22.1 thay bằng mô hình `MultiStream` theo từng `ThreadId`/`StreamId` dùng chung `FusionServer` (với hàng đợi `ChannelDeviceHandle::submit` dùng `SEND_NO_FLUSH`, `tag_shared_view`, và `foreign_drop`). Khi truyền trực tiếp `ModelUpdate::Wgpu(train_model.valid())` chứa `FusionTensor` giữa luồng training (`S_A`) và luồng inference (`S_B`) ở tần suất cao (~800–1.600 lần đổi model trong 35 giây):
  - Trước vá và ngay cả sau khi gọi `device.sync()` trước `try_send`, luồng `DSU-100-0` đều panic tại `burn-ir-0.22.1/src/handle.rs:333:32` (`Should have handle for tensor TensorId { ... }`), làm sập luồng inference tại `materialize_params`.
- **Giải pháp được chọn:** Trong `spawn_wgpu_learner` (`src-tauri/src/core/training.rs`), đọc trọng số phẳng (`to_flat_weights`) và đóng gói tạm trên `Device::flex()` ngay tại luồng learner; bên nhận `BrainModel::replace_wgpu_model` (`src-tauri/src/ai/model.rs`) dựng lại `ActorCriticModel::from_flat_weights` trên `device` của chính luồng inference, chạy `materialize()` rồi `drop(old)` ngay tại chỗ (`return None`). Không một `FusionTensor` WGPU nào vượt ranh giới luồng.
- Bổ sung test hồi quy liên luồng `test_wgpu_learner_publishes_synced_models_across_threads` (≥ 32 lần cập nhật liên luồng) vào `src-tauri/tests/phase5_burn_wgpu_fallback.rs`.

### 4. Giữ `gpu_backend_requested` tắt mặc định và hoãn quyết định `LIVE_MODEL_VERSION`

- Cờ `gpu_backend_requested` (`src-tauri/src/core/resources.rs`) **giữ nguyên mặc định `false` (`OFF`)**. Mặc dù phép đo RTX 5060 Ti cho thấy đường WGPU ổn định sau khi cô lập stream (0 panic trong 180s, đi ngang sau `t = 75s`), đường CPU `burn-flex` là đường tất định tham chiếu hiện hành; mọi thay đổi mặc định GPU thuộc quyết định riêng của chủ dự án.
- Chuyển đổi từ `burn-ndarray 0.13.2` sang `burn-flex 0.22.1` giữ nguyên mọi checksum tại `tick = 0` và toàn bộ thế giới dùng não tiến hoá (`evolved = true`), nhưng **làm đổi quỹ đạo sau `tick > 0` của mọi thế giới dùng shared Burn `BrainModel`** (`evolved = false`). Theo [ADR-0008](ADR-0008-world-past-and-time.md), việc tăng hay giữ `LIVE_MODEL_VERSION` (`"live-bevy-world/1"` tại `src-tauri/src/core/live_experiment.rs:67`) được báo cáo để chủ dự án quyết định.

## Bằng chứng đo được (Windows 11 x64, RTX 5060 Ti)

### Điều kiện 1 & 2 — Cây phụ thuộc và cổng Rust

- `cargo tree --locked -e all --features desktop --target all | grep -c " hexf-parse v"`: `0`.
- `cargo tree --locked --no-default-features -e normal` kiểm tra `neo4rs|tokio-tungstenite|burn-wgpu|naga`: `0`.
- `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --features desktop -- -D warnings`, `cargo clippy --locked --all-targets --no-default-features -- -D warnings`, `cargo bench --locked --bench tick_systems --no-run`, `cargo test --locked --lib export_bindings` (50/50 pass, `git diff --exit-code -- src/types/generated` thoát 0): đều đạt.
- `cargo test --features desktop -- --nocapture` + `node scripts/check_test_targets.mjs out.txt --profile desktop`: `targets=151, passed=1483, failed=0, ignored=2 (allow-listed 2), empty: 3 (allow-listed 3), feature-gated running under "desktop": 8`, `POLICY_EXIT=0`. Bao gồm 6 bộ test trọng tâm: `a2c_loss_direction_tests` (7/7), `brain_parity_tests` (8/8), `burn_neural_net_tests` (6/6), `gpu_brain_batch_parity_tests` (3/3), `phase5_burn_wgpu_fallback` (2/2, WGPU thật), `schema7_learner_migration_tests` (3/3).
- `cargo audit`: thoát `0` (`warning: 2 allowed warnings found`: `async-executor 1.14.0`, `chacha20 0.10.1`).

### Điều kiện 4 — So sánh checksum CPU trước (`Burn 0.13.2 NdArray`) và sau (`Burn 0.22.1 flex`)

Cùng seed `[1, 2, 20261118]` ở các mốc `tick = 0, 60, 120, 180`:

- `tick = 0` ở cả 3 cấu hình (`2020381237`, `2856665478`, `2900601128`) và toàn bộ **12/12** hàng của `evolved_lifetime_learning` (`evolved=true, lifetime=true`): **trùng khớp hoàn toàn trước và sau nâng cấp**.
- Toàn bộ **18/18** hàng `tick > 0` của `default_world` (`evolved=false, lifetime=false`) và `shared_lifetime_flag_on` (`evolved=false, lifetime=true`): **khác nhau giữa `burn-ndarray 0.13.2` và `burn-flex 0.22.1`** (ví dụ seed 1 tại `t=60/120/180`: `1963114950 -> 85451031`, `3733739000 -> 3084206812`, `694568034 -> 1783811784`).

### Điều kiện 5 — Bộ đếm cấp phát heap sống (`alloc - free`) trên bản `--release`

| Chế độ | Warmup / Đo | Agent | Published | `delta_mb` | Tốc độ điểm cuối (MB/phút) | Hồi quy OLS (MB/phút) | Sau `stop()` | Kết quả |
|---|---|---|---|---|---|---|---|---|
| `GPU_WGPU` (chưa cô lập, chưa `sync`) | 20s / 180s | `35->0` | `544->0` | `+1.225` | `+0.408` | `+0.159` | `-7.957` | Panic `DSU-100-0` ở `~35s` (`burn-ir/src/handle.rs:333`) |
| `GPU_WGPU` (chỉ thêm `device.sync()`) | 20s / 180s | `29->0` | `799->0` | `+0.590` | `+0.197` | `+0.099` | `-8.940` | Panic `DSU-100-0` ở `~35s` (`burn-ir/src/handle.rs:333`) |
| `GPU_WGPU` (sau khi cô lập stream) | 20s / 180s | `36->84` | `834->27831` | `+0.970` | `+0.323` | `+0.187` | `-8.909` | **0 panic**, ổn định ở `25.07–25.17 MB` từ `t=75s..180s` |
| `CPU_FLEX_CONTROL` | 10s / 60s | `20->84` | `335->7773` | `+0.165` | `+0.165` | `+0.182` | `-13.358` | Đối chứng CPU sạch |

Trong chặng ổn định `[75s..180s]` của phép chạy `GPU_WGPU` cuối, heap sống biến thiên `-0.042 MB` (`25.129 -> 25.087 MB`) dù `published` tăng thêm hàng chục nghìn lần; rò bộ nhớ không chặn của Burn 0.13 (`burn#2042`) không còn tái hiện.

## Đánh giá giấy phép mới & các điểm chặn còn mở (`OPEN_SOURCE_POLICY.md`)

1. **`cargo deny check` thất bại (mã thoát `4`, `licenses FAILED`):** Cây phụ thuộc vô điều kiện của Burn 0.22 / CubeCL kéo thêm ba crate mang giấy phép chưa có trong danh sách cho phép của `src-tauri/deny.toml`:
   - `buildid v1.1.0` (`MPL-2.0`, qua `cubecl-runtime v0.11.4 -> cubecl v0.11.4`).
   - `colored v3.1.1` (`MPL-2.0`, qua `burn-tensor v0.22.1 -> burn-core v0.22.1`).
   - `xxhash-rust v0.8.21` (`BSL-1.0`, qua `cubecl-common v0.11.4 -> burn-std v0.22.1`).
   - Lưu ý bổ sung: `naga 30.0.2` có một phần bộ đọc hex-float nội tuyến mang chú thích `0BSD` (dễ dãi, tương thích [chính sách nguồn mở](../governance/OPEN_SOURCE_POLICY.md)).
   - Do uỷ quyền R1c chỉ cho phép **xoá** mục trong `src-tauri/deny.toml`, chưa có quyền thêm `MPL-2.0` (copyleft yếu cấp tập tin) và `BSL-1.0` hoặc xem xét ngoại lệ.
2. **`npm run gen:compliance` & `npm run check:licenses-complete` thất bại (`ENOBUFS` + thiếu kho upstream):**
   - `scripts/lib/licensing.mjs:193` chạy `cargo tree --features desktop -e normal --prefix none --no-dedupe` (không chống lặp) với `maxBuffer = 256 MiB`, tràn bộ đệm thành `ENOBUFS` vì DAG mới của `burn 0.22.1` + `cubecl 0.11.4` + `cubek 0.3.1` nở rộng hơn 256 MiB văn bản cây chưa khử trùng lặp.
   - Đồng thời, trên cây `Cargo.lock` mới có 11 mục trong `licensing/upstream/sources.json` (gồm `pkg:cargo/hexf-parse@0.2.1` và 10 crate cũ của `wgpu 0.19` / `naga 0.19` / `d3d12`) đã rời cây phụ thuộc, và các crate mới thuộc họ `burn 0.22.1` / `cubecl 0.11.4` / `cubek 0.3.1` / `buildid 1.1.0` / `spirv 0.4.0+sdk-1.4.341.0` không đính sẵn file `LICENSE*` trong gói `.crate` (cần đồng bộ kho `licensing/upstream/`). Cả `scripts/lib/licensing.mjs` lẫn `licensing/upstream/` đều nằm ngoài phạm vi uỷ quyền thẻ R1c.

## Hệ quả & bước tiếp theo

- **Tích cực:** Thoát hoàn toàn khỏi `hexf-parse` và `burn-ndarray`; mọi test Parity, A2C, WGPU fallback, di trú Schema 7→8, và 180s WGPU heap probe đều xanh sau cô lập stream.
- **Chặn cần chủ dự án phê duyệt R5:**
  1. Phê duyệt bổ sung `MPL-2.0` (`buildid`, `colored`) và `BSL-1.0` (`xxhash-rust`) vào `src-tauri/deny.toml` theo [chính sách nguồn mở](../governance/OPEN_SOURCE_POLICY.md).
  2. Cấp uỷ quyền cho thẻ nối tiếp (hoặc mở rộng R1c) để sửa lỗi `ENOBUFS` trong `scripts/lib/licensing.mjs`, cập nhật `licensing/upstream/sources.json` + kho giấy phép `licensing/upstream/github.com/**` cho các crate Burn 0.22 / CubeCL 0.11.4 / Cubek 0.3.1, và chạy lại `gen:compliance` + `check:licenses-complete` trên Windows.
  3. Sau khi hoàn tất bước (1) và (2), quyết định có tăng `LIVE_MODEL_VERSION` hay không, cập nhật `CLAUDE.md` và `STATE_OF_THE_PROJECT.md`, và chuyển ADR này sang `accepted`.
