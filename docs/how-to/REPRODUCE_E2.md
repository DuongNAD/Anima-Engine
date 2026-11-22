---
title: Cách tái lập thí nghiệm E2 (evolved-brain vs shared BrainModel)
status: active
owner: maintainers
last_reviewed: 2026-11-22
review_cycle: per-release
---

# Cách tái lập thí nghiệm E2 (`evolved-brain` vs `shared BrainModel`)

Hướng dẫn này mô tả từng bước tái lập thí nghiệm **E2** đã tiền đăng ký trên thế giới sống headless (`LiveExperimentAdapter`), kiểm chứng lại các cổng `E2-G1..G6`, và đối chiếu độc lập mọi checksum qua `npm run check:e2`.

> **Tài liệu tiền đăng ký & hợp đồng chuẩn:** [`e2-preregistration.json`](../../src-tauri/tests/fixtures/experiments_e2/e2-preregistration.json), [`RESULT.md`](../../artifacts/experiments/e2-evolved-brain-default/RESULT.md), [`2026-07-27-experiment-e2-evolved-brain-default.md`](../ai/testing/2026-07-27-experiment-e2-evolved-brain-default.md), và [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) *(proposed)* (`ER07`, `ER08`, `ER11`).
>
> **Ranh giới suy luận:** mọi kết luận từ runner này chỉ mang trạng thái **`headless adapter verified`** trên một thiết kế thế giới (`256 × 256`), không mở app desktop và không chạy vòng thay thế tiến hóa qua nhiều thế hệ.

---

## Bước 1 — Chạy test tiền đăng ký, seam và manifest đóng băng (`E2-G1..G4`, `E2-G6`)

Trước khi build runner `--release`, chạy ba bộ test Rust để khóa giao diện factor (`live.evolved_brains`), cô lập luồng `SimRng`, đồng nhất `WorldIdentity`, lệnh cấm `cargo run`, và khớp từng byte với các manifest E2 đã công bố:

<!-- doc-test -->
```bash
cargo test --manifest-path src-tauri/Cargo.toml --test e2_seam_tests
cargo test --manifest-path src-tauri/Cargo.toml --test prereg_e2_manifest_tests
cargo test --manifest-path src-tauri/Cargo.toml --test e2_frozen_manifest_tests
```

Kết quả quan sát được trên `macOS arm64` (chạy ngày `2026-11-22` tại commit `03b9d3d` bằng 3 lệnh `cargo test` ở trên; tổng số test toàn dự án có thẩm quyền xem tại [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền)):

```text
running 9 tests (e2_seam_tests) ... ok. 9 passed; 0 failed; 0 ignored
running 9 tests (prereg_e2_manifest_tests) ... ok. 9 passed; 0 failed; 0 ignored
running 2 tests (e2_frozen_manifest_tests) ... ok. 2 passed; 0 failed; 0 ignored
```

Tóm tắt 6 cổng E2 được kiểm tra trong bộ test và runner:

| Cổng | Ý nghĩa kiểm chứng |
|---|---|
| `E2-G1` | `live.evolved_brains = 1.0` dựng `10/10` founder mang `AgentBrain`; control dựng `0/10` |
| `E2-G2` | Thế giới không khai báo `live.evolved_brains` giữ nguyên hành vi cũ |
| `E2-G3` | `SimRng` seed, vị trí stream và 8 lần rút kế tiếp trùng nhau giữa hai arm sau genesis |
| `E2-G4` | `FactorDiff` chỉ cho phép khác đúng đường dẫn `initial_conditions` (`live.evolved_brains`) |
| `E2-G5` | `--replay` chạy lại một `(seed, arm)` cho ra đúng `final_checksum` |
| `E2-G6` | `WorldIdentity` ghi trong `provenance.json` trùng khớp giữa hai arm |

---

## Bước 2 — Build binary `--release --features desktop` (CẤM `cargo run`)

Hợp đồng tiền đăng ký E2, `e2_seam_tests`, và `scripts/verify_e2_artifacts.mjs` **cấm dùng `cargo run`**. Phải biên dịch binary trước rồi gọi trực tiếp file nhị phân để `provenance.json` ghi đúng `"profile": "release"` và `binary_sha256`:

<!-- doc-test -->
```bash
cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example run_e2_brain_experiment
```

**Lưu ý đường dẫn thực thi theo hệ điều hành:**
- **macOS / Linux:** `./src-tauri/target/release/examples/run_e2_brain_experiment` (như ghi trong `artifacts/experiments/e2-evolved-brain-default/execution-lock.json`).
- **Windows PowerShell:** `.\src-tauri\target\release\examples\run_e2_brain_experiment.exe` (như ghi trong `e2-preregistration.json`).

---

## Bước 3 — Chạy `--smoke` hiệu chuẩn (seed `999983`) vào thư mục tạm

Luôn xuất vào một thư mục gitignored dưới `src-tauri/target/` (không ghi đè [`artifacts/experiments/e2-evolved-brain-default/`](../../artifacts/experiments/e2-evolved-brain-default)). Chế độ `--smoke` chạy cặp manifest smoke (seed `999983` — bị loại cơ học khỏi danh sách seed thí nghiệm) và xuất vào `<out>/smoke/`:

<!-- doc-test -->
```bash
./src-tauri/target/release/examples/run_e2_brain_experiment --smoke \
  --manifest-dir src-tauri/tests/fixtures/experiments_e2 \
  --out src-tauri/target/doc2-e2-repro
```

> **Trên Windows PowerShell:**
>
> ```powershell
> .\src-tauri\target\release\examples\run_e2_brain_experiment.exe --smoke `
>   --manifest-dir src-tauri\tests\fixtures\experiments_e2 `
>   --out src-tauri\target\doc2-e2-repro
> ```

Kết quả quan sát được trên `macOS arm64` (chạy ngày `2026-11-22` tại commit `03b9d3d` với lệnh `--smoke` ở trên):

```text
E2 Smoke: 1 seeds x 2 arms x 18000 ticks, sample period 600
  integrity: control brains 0 / treatment brains 10 of 10 founders; ecology stream identical: true; world identity identical: true
  finished in 4.4s: 1 complete pairs of 1
  artifacts written to src-tauri/target/doc2-e2-repro/smoke
```

> **Thang thời lượng (`--duration-rung`):** runner hỗ trợ `--duration-rung <ticks>` chỉ nhận một trong ba nấc `[18000, 12000, 6000]` đã đăng ký trong `e2-preregistration.json` (mặc định `18000`). Nấc phải được khóa **một lần** từ kết quả `--smoke` trước khi chạy `--ensemble` (giới hạn ngân sách 90 phút).

---

## Bước 4 — Chạy `--ensemble` (12 seed) và `--replay` (`E2-G5`)

Sau khi hiệu chuẩn xong, chạy đủ 12 seed (`700001..711001`) bằng `--ensemble`, rồi kiểm chứng tính tái lập từng arm bằng `--replay` (yêu cầu đủ cả `--replay-seed` nằm trong manifest thí nghiệm và `--replay-arm control|treatment`):

```bash
./src-tauri/target/release/examples/run_e2_brain_experiment --ensemble \
  --manifest-dir src-tauri/tests/fixtures/experiments_e2 \
  --out src-tauri/target/doc2-e2-repro

./src-tauri/target/release/examples/run_e2_brain_experiment --replay \
  --manifest-dir src-tauri/tests/fixtures/experiments_e2 \
  --out src-tauri/target/doc2-e2-repro \
  --replay-seed 700001 \
  --replay-arm treatment

./src-tauri/target/release/examples/run_e2_brain_experiment --replay \
  --manifest-dir src-tauri/tests/fixtures/experiments_e2 \
  --out src-tauri/target/doc2-e2-repro \
  --replay-seed 700001 \
  --replay-arm control
```

Kết quả quan sát được trên `macOS arm64` (chạy ngày `2026-11-22` tại commit `03b9d3d` với các lệnh `--ensemble` và `--replay` ở trên; `52.3s` cho toàn bộ 12 cặp seed):

```text
E2 Ensemble: 12 seeds x 2 arms x 18000 ticks, sample period 600
  integrity: control brains 0 / treatment brains 10 of 10 founders; ecology stream identical: true; world identity identical: true
  finished in 52.3s: 12 complete pairs of 12
  artifacts written to src-tauri/target/doc2-e2-repro

replay treatment seed 700001: checksum 1081145212 (completed in 2.4s) -> src-tauri/target/doc2-e2-repro/replay/replay-treatment-seed-700001-t18000.json
replay control seed 700001: checksum 3017397262 (completed in 2.1s) -> src-tauri/target/doc2-e2-repro/replay/replay-control-seed-700001-t18000.json
```

Đối chiếu `final_checksum` trong `replay/replay-*-seed-700001-t18000.json` khớp từng chữ số với các hàng `700001` trong `runs.csv` của **cùng lượt chạy này** (`3017397262` cho `control`, `1081145212` cho `treatment`).

> **Vì sao `final_checksum` có thể khác với bộ artifact đã commit:** theo [`DETERMINISM_CONTRACT.md` §1](../reference/DETERMINISM_CONTRACT.md#1-lời-hứa-và-giới-hạn-của-nó), tính tái lập bit-identical áp dụng cho **cùng manifest + cùng build trên cùng nền tảng**. Hiện nay `artifacts/experiments/e2-evolved-brain-default/` đang giữ bản chạy lại ngày **2026-11-16 trên macOS arm64** tại commit `900ba2d` (22 observable trước S9/S2), còn bộ artifact gốc ngày **2026-07-27 trên Windows x86_64** đã được lưu trữ tại [`superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md`](../../artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md). Vì vậy, `final_checksum` từ một lượt chạy cục bộ có thể khác với các file đã commit do: (a) khác biệt phép tính dấu phẩy động liên kiến trúc (`macOS arm64` so với `Windows x86_64` trong `superseded/`), và (b) các thay đổi ở engine/observable kể từ commit `900ba2d` (chẳng hạn tại `03b9d3d`, `700001,control` là `3017397262` so với `2046314586` ở `900ba2d`). Luôn đối chiếu `--replay` với `runs.csv` sinh ra từ **cùng một lần build và chạy**.

---

## Bước 5 — Kiểm chứng độc lập bằng `npm run check:e2`

Chạy [`scripts/verify_e2_artifacts.mjs`](../../scripts/verify_e2_artifacts.mjs) bằng `node:crypto` cho cả thư mục vừa tạo lẫn bộ artifact đã commit trong repository:

<!-- doc-test -->
```bash
npm run check:e2 -- src-tauri/target/doc2-e2-repro
npm run check:e2
```

Kết quả quan sát được trên `macOS arm64` sau lượt `--smoke` (chạy ngày `2026-11-22` tại commit `03b9d3d`, khi chỉ chạy Bước 3 trước Bước 5):

```text
note  analysis: no ensemble has been run yet (no paired-report.json at the artifact root)
note  smoke: recomputed 27 per-seed deltas
note  analysis: no execution-lock.json — the duration rung was not locked from a smoke calibration

OK — every E2 artifact in .../src-tauri/target/doc2-e2-repro verifies against node:crypto and the preregistration.
```

Và sau khi chạy đủ Bước 4 (`--ensemble` 12 seed × 27 observable = `324` delta + `27` smoke delta):

```text
note  analysis: recomputed 324 per-seed deltas
note  smoke: recomputed 27 per-seed deltas
note  analysis: no execution-lock.json — the duration rung was not locked from a smoke calibration

OK — every E2 artifact in .../src-tauri/target/doc2-e2-repro verifies against node:crypto and the preregistration.

> node scripts/verify_e2_artifacts.mjs
note  analysis: recomputed 264 per-seed deltas
note  smoke: recomputed 22 per-seed deltas

OK — every E2 artifact in .../artifacts/experiments/e2-evolved-brain-default verifies against node:crypto and the preregistration.
```

Cấu trúc file sinh ra trong `--out`:
- `manifests/` (3 JSON khớp byte với `HEAD:src-tauri/tests/fixtures/experiments_e2/*`), `paired-report.json`, `effects.json`, `per-seed-deltas.csv`, `runs.csv`, `provenance.json`, `summary.md`, `checksums.sha256`.
- `smoke/` (chứa cấu trúc tương tự riêng cho seed hiệu chuẩn `999983`) và `replay/` (`replay-<arm>-seed-<seed>-t<rung>.json`).
- Thư mục commit `artifacts/experiments/e2-evolved-brain-default/` còn giữ thêm `execution-lock.json`, `RESULT.md`, và `superseded/`.

---

## Bước tiếp theo

- Chạy `anima-lab` cho các thí nghiệm khác: [RUN_EXPERIMENTS.md](RUN_EXPERIMENTS.md).
- Cách viết cổng kiểm tra mới như `verify_e2_artifacts.mjs`: [ADD_A_GATE.md](ADD_A_GATE.md).
