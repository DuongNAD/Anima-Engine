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

### 1. Nâng phụ thuộc Burn `0.22.1` và `burn-flex` với tập feature thu hẹp (`QĐ-C`, `QĐ-R1c-b`)

- Cấu hình `burn = { version = "0.22.1", default-features = false, features = ["std", "optim", "flex", "simd", "rayon"] }` và `ml-wgpu = ["burn/wgpu"]` trong `src-tauri/Cargo.toml`.
  - Bỏ feature `default` tắt đồng thời `persistence` (`cubecl-environment/persistence -> turso*`), `fusion` (`burn-fusion`, `burn-cubecl-fusion`), `autotune`, `burn-train` và `burn-dataset`. Nhờ vậy `pack1`, `genawaiter`, `genawaiter-macro`, `cfg_block`, `turso*`, `burn-train`, `burn-dataset`, `burn-ndarray`, `hexf-parse`, `d3d12`, `gix-*`, `libsqlite3-sys` và `burn 0.13` đều rời hoàn toàn cây phụ thuộc và/hoặc `Cargo.lock`.
  - Chi phí đánh đổi: WGPU không lưu cache autotune xuống đĩa qua Turso SQLite và không bật kernel fusion; với tính năng GPU mặc định tắt (`OFF`) và chỉ là chế độ đo tuỳ chọn, WGPU vẫn vận hành đầy đủ trên RTX 5060 Ti với mức cấp phát heap tương đương bản đầy đủ.
- Không bật `cpu` (`CubeCL JIT`), không bật BLAS, và không dùng `Device::default()` vốn ưu tiên GPU nếu tính năng WGPU được biên dịch. Toàn bộ đường CPU chỉ dùng `Device::flex()`.
- Giữ nguyên bố cục trọng số phẳng `w[out * fan_in + in]` trong `ActorCriticModel::from_flat_weights` / `to_flat_weights` ([ADR-0003](ADR-0003-evolved-per-agent-brains.md), gate parity `EB-S02`) và một hàm mục tiêu A2C duy nhất (`core::training::a2c_loss`, hệ số actor `+td`).
- Gỡ bỏ ba mục miễn trừ advisory trong `src-tauri/.cargo/audit.toml` mà crate của chúng đã rời `Cargo.lock` (`RUSTSEC-2024-0350`, `RUSTSEC-2025-0021`, `RUSTSEC-2026-0459`), giữ lại 8 mục của `zip 0.6.6` / `zstd-safe` / `zstd-sys` (`RUSTSEC-2026-0367`, `0368`, `0369`, `0403`, `0410`, `0429`, `0545`, `0548`) do vẫn tồn tại trong `Cargo.lock` qua các cây nhánh `burn-candle`/`burn-tch` chưa bật (vắng mặt khỏi mọi đồ thị biên dịch).

### 2. Nâng schema snapshot 7 lên 8 và migration save cũ (`QĐ-R1c-a`)

- Tăng `SCHEMA_VERSION` trong `src-tauri/src/core/snapshot.rs` từ `7` lên `8`. Định dạng mới lưu learner dùng chung bằng burnpack `ModuleRecord` / `OptimizerRecord` (`burn::store`) và round-trip chính xác.
- Snapshot schema 7 (ghi bằng `BinBytesRecorder` của Burn 0.13.2) vẫn khôi phục nguyên vẹn toàn bộ thế giới, nhưng hai bản ghi learner (`training_model_record`, `optimizer_record`) bị từ chối với lỗi định danh `LearnerRecordError::Schema7Burn013Unsupported` ghi log cảnh báo và learner dùng chung khởi tạo lại từ đầu.
- Kèm fixture thực sinh trước khi nâng cấp từ Burn `0.13.2` tại `src-tauri/tests/fixtures/snapshots/schema7_burn013_shared_learner.json` và bộ test `schema7_learner_migration_tests`.

### 3. Cô lập stream WGPU giữa luồng learner và luồng inference

- **Phát hiện thực nghiệm trong R1c:** Burn 0.22.1 thay bằng mô hình `MultiStream` theo từng `ThreadId`/`StreamId` dùng chung `FusionServer` (với hàng đợi `ChannelDeviceHandle::submit` dùng `SEND_NO_FLUSH`, `tag_shared_view`, và `foreign_drop`). Khi truyền trực tiếp `ModelUpdate::Wgpu(train_model.valid())` chứa `FusionTensor` giữa luồng training (`S_A`) và luồng inference (`S_B`) ở tần suất cao (~800–1.600 lần đổi model trong 35 giây):
  - Trước vá và ngay cả sau khi gọi `device.sync()` trước `try_send`, luồng `DSU-100-0` đều panic tại `burn-ir-0.22.1/src/handle.rs:333:32` (`Should have handle for tensor TensorId { ... }`), làm sập luồng inference tại `materialize_params`.
- **Giải pháp được chọn:** Trong `spawn_wgpu_learner` (`src-tauri/src/core/training.rs`), đọc trọng số phẳng (`to_flat_weights`) và đóng gói tạm trên `Device::flex()` ngay tại luồng learner; bên nhận `BrainModel::replace_wgpu_model` (`src-tauri/src/ai/model.rs`) dựng lại `ActorCriticModel::from_flat_weights` trên `device` của chính luồng inference, chạy `materialize()` rồi `drop(old)` ngay tại chỗ (`return None`). Không một `FusionTensor` WGPU nào vượt ranh giới luồng.
- Bổ sung test hồi quy liên luồng `test_wgpu_learner_publishes_synced_models_across_threads` (≥ 32 lần cập nhật liên luồng) vào `src-tauri/tests/phase5_burn_wgpu_fallback.rs`.

### 4. Giữ `gpu_backend_requested` tắt mặc định và tăng `LIVE_MODEL_VERSION` lên `"live-bevy-world/2"`

- Cờ `gpu_backend_requested` (`src-tauri/src/core/resources.rs`) **giữ nguyên mặc định `false` (`OFF`)**. Mặc dù phép đo RTX 5060 Ti cho thấy đường WGPU ổn định sau khi cô lập stream (0 panic trong 180s, đi ngang sau `t = 75s`), đường CPU `burn-flex` là đường tất định tham chiếu hiện hành; mọi thay đổi mặc định GPU thuộc quyết định riêng của chủ dự án.
- Chuyển đổi từ `burn-ndarray 0.13.2` sang `burn-flex 0.22.1` giữ nguyên mọi checksum tại `tick = 0` và toàn bộ thế giới dùng não tiến hoá (`evolved = true`), nhưng **làm đổi quỹ đạo sau `tick > 0` của mọi thế giới dùng shared Burn `BrainModel`** (`evolved = false`). Theo [ADR-0008](ADR-0008-world-past-and-time.md) mục 5, thay đổi quỹ đạo ở thế giới mặc định mở ra một kỷ nguyên engine mới, nên `LIVE_MODEL_VERSION` tại `src-tauri/src/core/live_experiment.rs` được tăng từ `"live-bevy-world/1"` lên `"live-bevy-world/2"`.

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
| `GPU_WGPU` (chưa cô lập, cấu hình ban đầu) | 20s / 180s | `35->0` | `544->0` | `+1.225` | `+0.408` | `+0.159` | `-7.957` | Panic `DSU-100-0` ở `~35s` (`burn-ir/src/handle.rs:333`) |
| `GPU_WGPU` (chỉ thêm `device.sync()`) | 20s / 180s | `29->0` | `799->0` | `+0.590` | `+0.197` | `+0.099` | `-8.940` | Panic `DSU-100-0` ở `~35s` (`burn-ir/src/handle.rs:333`) |
| `GPU_WGPU` (cô lập stream, trước thu hẹp feature) | 20s / 180s | `36->84` | `834->27831` | `+0.970` | `+0.323` | `+0.187` | `-8.909` | **0 panic**, ổn định ở `25.07–25.17 MB` từ `t=75s..180s` |
| `GPU_WGPU` (cô lập stream + **feature thu hẹp**) | 20s / 180s | `33->83` | `807->27538` | `+1.187` | `+0.396` | `+0.195` | `-8.502` | **0 panic**, `live_mb` ổn định `21.34–21.61 MB` |
| `CPU_FLEX_CONTROL` (feature thu hẹp) | 10s / 60s | `20->83` | `334->7592` | `+0.188` | `+0.188` | `+0.197` | `-13.346` | Đối chứng CPU sạch |

Trong phép chạy `GPU_WGPU` cuối với feature thu hẹp, tốc độ hồi quy OLS `+0.195 MB/phút` khớp sát đối chứng CPU `+0.197 MB/phút`, chứng minh không có rò bộ nhớ không chặn dù không bật `persistence`/`fusion`/`autotune`; rò bộ nhớ của Burn 0.13 (`burn#2042`) không còn tái hiện.

## Đánh giá giấy phép mới & điểm chặn cuối (`OPEN_SOURCE_POLICY.md`)

1. **`src-tauri/deny.toml` (đã xanh):** Cây phụ thuộc Burn 0.22 / CubeCL thu hẹp giữ lại ba ngoại lệ từng crate trong `src-tauri/deny.toml` kèm đường phụ thuộc (không nới `allow` toàn cục):
   - `buildid v1.1.0` (`MPL-2.0`, qua `ml-wgpu -> burn/wgpu -> cubecl 0.11.4 -> cubecl-runtime 0.11.4`, nguồn công khai tại `https://github.com/jmesmon/buildid`).
   - `colored v3.1.1` (`MPL-2.0`, qua `burn v0.22.1 -> burn-core 0.22.1 -> burn-tensor 0.22.1`, nguồn công khai tại `https://github.com/mackwic/colored`).
   - `xxhash-rust v0.8.21` (`BSL-1.0`, qua `ml-wgpu -> burn/wgpu -> burn-std 0.22.1 -> cubecl-common 0.11.4`).
   - Cả hai crate `MPL-2.0` đều được phân phối nguyên vẹn không sửa đổi và liệt kê trong `NOTICE` / `licensing/THIRD_PARTY_LICENSES.txt` kèm URL nguồn công khai (`crates.io` + repository gốc). `cargo deny check` thoát `0` (`advisories ok, bans ok, licenses ok, sources ok`).
2. **Sửa `ENOBUFS` ở `scripts/lib/licensing.mjs:195` & hoàn tất `gen:compliance` / `check:compliance`:**
   - Gỡ `--no-dedupe` khỏi `cargoInventory` sau khi kiểm chứng trực tiếp tập `name@version` với `--no-dedupe` và bản khử trùng lặp trùng khớp tuyệt đối (`diff = 0`) trên cả commit gốc `071e43ce` (`112.147` vs `1.109` dòng, `424` crate) và cấu hình thu hẹp (`21.695.977` dòng stream trực tiếp vs `1.629` dòng, `526` crate).
   - Loại bỏ được `cfg_block@0.1.1`, `pack1@1.1.0`, `genawaiter@0.99.1`, `genawaiter-macro@0.99.1` và `turso*` nhờ bỏ `persistence`; `licensing/upstream/` lưu 35 file từ 24 commit / 21 repo cho 51 thành phần (`npm run verify:upstream-licenses`: `35/35` khớp từng byte).
   - `npm run gen:compliance` (`563` thành phần: `524` Rust + `21` npm phân phối + `18` cài thêm, `296` văn bản giấy phép) và `npm run check:compliance` (gồm `check:sbom-schema` CycloneDX 1.5 và `check:text-hygiene`) đều thoát `0`, cùng `44/44` `upstreamLicenses.test.ts`, `47/47` `thirdPartyLicenses.test.ts`, `9/9` `noticeInventory.test.ts`.
3. **Điểm chặn không thể gỡ bằng feature khi `ml-wgpu` bật (`check:licenses-complete` thoát `1`, còn đúng 1 mục trong `licensing/UNRESOLVED.md`):**
   - **`utf8-chars@3.0.7` (`MIT OR Apache-2.0`, duy nhất trong `licensing/UNRESOLVED.md`):** đi vào qua `pliron v0.19.0` feature `"std"` (`<- cubecl-core/std & cubecl-wgpu/std <- cubecl/std <- burn-backend/std & burn-cubecl/std & burn-wgpu/std <- burn/std`). Thử nghiệm thực tế bỏ `"std"` ở `burn` (`default-features = false, features = ["optim", "flex", "simd", "rayon"]`) làm `cargo check --features desktop` thất bại (`exit 101`, `error[E0432]: unresolved import 'spin::LazyLock'` trong `pliron-0.19.0/src/std_deps.rs:47`), và `cubecl-runtime/src/logging/server.rs:90` cần `cubecl-environment/std` cho `spawn_detached`. Repo `https://github.com/A1-Triard/utf8-chars` (5 file ở cả commit `9e35858ccf` và `HEAD`) không có file `LICENSE*`/`COPYING*`/`NOTICE*` hay tuyên bố giấy phép trong `README`/`src`.
   - **Lưu ý bổ sung về `buildid@1.1.0` (`MPL-2.0`):** phụ thuộc **vô điều kiện** (không `optional`) trong `Cargo.toml` của cả `cubecl-runtime v0.11.4` và `cubecl-server v0.11.4` (`src/compiler.rs:21`), không có bất kỳ feature flag nào loại bỏ được kể cả khi thử bỏ `std`. Repo `https://github.com/jmesmon/buildid` chưa từng phát hành file toàn văn `MPL-2.0` nên được đóng trong `licensing/upstream/` dưới dạng `README.md` (`licence-statement`) thay cho toàn văn MPL-2.0.

## Hệ quả & bước tiếp theo

- **Đã thu hẹp khoảng cách từ 4 crate xuống còn đúng 1 crate (`utf8-chars 3.0.7`):** Điều kiện 1, 2, 3, 4, 5, 7 và toàn bộ `gen:compliance` / `check:compliance` đều xanh trên cấu hình thu hẹp.
- **Quyết định `QĐ-C` còn lại của chủ dự án:** Do không có tập feature Cargo nào gỡ được `utf8-chars@3.0.7` (`pliron/std`) và `buildid@1.1.0` (`cubecl-runtime`/`cubecl-server`) mà vẫn giữ `ml-wgpu` (`burn/wgpu`) biên dịch và chạy Điều kiện 2/5, chủ dự án cần quyết định giữ hay bỏ `ml-wgpu` (hoặc cấp ngoại lệ pháp lý cho `utf8-chars 3.0.7`). Sau quyết định cuối cùng, cập nhật `CLAUDE.md`, `STATE_OF_THE_PROJECT.md` ở thẻ nối tiếp và chuyển ADR này sang `accepted`.
