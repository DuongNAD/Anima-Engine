# BENCHMARK BASELINE — M0.4

> **Cập nhật 2026-07-26 — hai thay đổi lớn.**
>
> 1. **Phần cứng mục tiêu nay là Intel Core i5-14600KF** (desktop, 14 nhân / 20 luồng, 47,8 GB,
>    Windows 11 Pro 10.0.26200). Khai báo cũ — *Dell Vostro 3530 (i7-1355U, Iris Xe iGPU + dGPU)* —
>    **không còn hiệu lực** và đã được thay ở mọi chỗ trong tài liệu này. Máy capture và máy mục
>    tiêu giờ là một, nên phân biệt "trên máy capture" / "trên máy đích" mà bản cũ dựa vào đã không
>    còn nghĩa.
> 2. **`timings` không còn hoàn toàn là proxy.** OSS-010 đã thêm Criterion và một bộ benchmark
>    headless đo **từng system** (`step_water`, `integrate_physics_system`, `a2c_loss`,
>    `WorldArtifact::to_bytes`, …) mà không boot Tauri và không mở GPU device — nên nó không vi phạm
>    cảnh báo vận hành bên dưới. `scripts/bench_baseline.mjs` nay đọc thẳng kết quả đó vào
>    [`benchmark_report.json`](benchmark_report.json) dưới tiền tố `criterion/`. Bảng số và cách
>    diễn giải: [`docs/how-to/BENCHMARKING.md`](docs/how-to/BENCHMARKING.md).
>
> **Điều này vẫn chưa khoá được các con số.** Bộ Criterion đo **cận dưới của một tick**, không đo
> khung hình: nó chưa gồm suy luận não, lập lịch ECS, thread emit, va chạm và trao đổi chất. Các
> hàng "Physics tick" / "Brain/sensor" dưới đây vẫn đòi một in-app tick capture, và ràng buộc "không
> chạy full backend" vẫn còn hiệu lực. Xem [§ Trạng thái khoá số](#trạng-thái-khoá-số).
>
> **Cập nhật 2026-07-27 — dụng cụ đã có, số thì chưa.** In-app tick capture đã ship
> ([`src-tauri/src/core/tick_capture.rs`](src-tauri/src/core/tick_capture.rs) + bốn lệnh IPC, có
> test ở `tick_capture_tests.rs`, xanh trong lần chạy 2026-07-27 ghi ở
> [`STATE_OF_THE_PROJECT.md` §1](docs/planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền)).
>
> 🔧 **"Instrumentation đã ship" KHÔNG phải "đã có một phép đo phần cứng".** Tính đến 2026-07-27
> **chưa có lần chạy app desktop nào**, nên **không tồn tại** một số đo tick nào từ app đang chạy —
> mọi hàng "Physics tick" / "Brain/sensor" / "Full-brain agents" dưới đây vẫn trống hoặc là proxy.
> Thủ tục ba bước để lấy số nằm ở [`docs/how-to/BENCHMARKING.md`](docs/how-to/BENCHMARKING.md); nó
> cần một con người mở app, việc mà CLAUDE.md cấm tự động hoá trên máy này.

Tài liệu này mô tả *reproducible benchmark scaffold* của Anima-Engine: cách capture
seed + config + hardware + timings một cách **rẻ và trung thực**, và cách thay các số
proxy còn lại bằng số thật.

> **Cảnh báo vận hành.** KHÔNG chạy full backend để đo (`npm run tauri dev` /
> `cargo run`) — việc này đã **crash máy dev**. Ngay cả việc khởi tạo một GPU device
> cũng nằm ngoài phạm vi. Scaffold này chạy hoàn toàn trên CPU, không mở app, không
> mở device, và kết thúc trong dưới một giây.

## Thành phần

| File | Vai trò |
|---|---|
| [`scripts/bench_baseline.mjs`](scripts/bench_baseline.mjs) | Script Node ESM (chỉ built-in `os`/`fs`/`process`) sinh ra report. |
| [`benchmark_report.json`](benchmark_report.json) | Report mẫu, tạo bằng cách chạy script một lần. |
| [`benchmark_report.schema.json`](benchmark_report.schema.json) | JSON-Schema (draft-07) mô tả shape bắt buộc của report. |
| [`src/__tests__/benchmarkReport.test.ts`](src/__tests__/benchmarkReport.test.ts) | Validator S04 (Vitest) — kiểm tra report có đủ field và loại report thiếu field. |

## Cách chạy

**Thứ tự quan trọng.** `target/` nằm trong `.gitignore`, nên một bản clone mới không có dữ liệu
Criterion; chạy script trước khi bench sẽ ghi đè số thật bằng proxy.

```bash
# 1) Đo thật, chạy từ src-tauri/ (PowerShell, không phải Git Bash):
cargo bench --bench tick_systems

# 2) Sinh / cập nhật report (chạy từ repo root) — đọc kết quả Criterion ở bước 1:
node scripts/bench_baseline.mjs

# 3) Validate report bằng test S04:
npx vitest run src/__tests__/benchmarkReport.test.ts
```

Nếu không tìm thấy dữ liệu Criterion **và** report hiện tại đang chứa số thật, script **từ chối
ghi** và thoát khác 0. Đây là chủ ý: một report proxy-only đè lên một report thật vẫn validate được,
vẫn trông như baseline, và vô giá trị — đúng loại hỏng im lặng mà bộ gate của dự án tồn tại để chặn.

Các biến môi trường tuỳ chọn (đều có default, không bắt buộc):

| Env | Ý nghĩa | Default |
|---|---|---|
| `ANIMA_BENCH_SEED` | seed ghi vào report | `1337` |
| `ANIMA_BENCH_TIMESTAMP` | ghi chú thời điểm capture (KHÔNG dùng `Date.now`) | `"set on capture"` |
| `ANIMA_BENCH_ALLOW_PROXY_ONLY` | `1` để cho phép ghi report proxy-only đè lên report thật | *(tắt)* |

## Phương pháp (methodology)

1. **Reproducibility envelope.** Report luôn ghi lại `seed`, `config`
   (`gridDim=128`, `tickHz=60`, `ticksPerEpoch=1000` — trùng hằng số trong
   [`src-tauri/src/core/sim_rules.rs`](src-tauri/src/core/sim_rules.rs)) và `hardware`
   (`platform`/`release`/`arch`/`cpuModel`/`cpuCount`/`totalMemMB` lấy từ `os.*`).
   Đây là phần bắt buộc để một số đo có thể tái lập.
   > **Finding đã đóng 2026-07-27.** Trước đây `gridDim=128` khớp `DEFAULT_GRID_DIM`, nhưng hằng số
   > đó **không được đọc ở đâu trong `src/`** còn thế giới thật chạy **256²**
   > (`MapSettings::default()`). `DEFAULT_GRID_DIM` nay là **256**, và
   > `s03_default_grid_dim_tracks_map_settings_default` buộc nó bằng `MapSettings::default().width`.
   > `COORDINATE_CONTRACT.md` §4 cùng `SIMULATION_RULES.md` §5 đã sửa theo (0.78125 thay vì 1.5625).
   > Bộ Criterion vốn đã dùng 256², nên **số đo không đổi** — chỉ nhãn `config.gridDim` mới đúng.
2. **Timings — hai loại, ghi nhãn khác nhau.**
   - `criterion/*` là **số đo thật**: trung vị Criterion, build release, một system mỗi entry, đọc
     từ `src-tauri/target/criterion/**/new/estimates.json`. Mỗi entry mang thêm `medianNs`,
     `meanNs`, `stdDevNs`.
   - `*_fbm_proxy` là **self-test của harness**: một vòng lặp số học kiểu fBm trên lưới 128²,
     thuần CPU, không cấp phát heap. Nó không phải terrain generator và chưa bao giờ là. Giữ lại vì
     nó neo `proxyChecksum` và chứng minh harness chạy.
3. **Đánh đổi về tính ổn định của diff.** Bản cũ tự hào file diff sạch giữa các lần chạy — nhưng
   điều đó chỉ có được vì không có gì thật được đo. Số thật dao động theo lần chạy, nên file nay
   đổi mỗi lần. Giá trị được làm tròn để giảm nhiễu. `timestampNote` vẫn không đọc đồng hồ máy.

## Phần còn lại vẫn là proxy

Ba nguồn số vẫn chưa có, và không cái nào bị chặn bởi phần cứng:

- **In-app tick capture** — nhịp thật của app đang chạy. Ràng buộc "không chạy full backend" vẫn
  còn hiệu lực, nên đây vẫn cần một harness đo tick an toàn.
- **Suy luận não per-agent** — phần đắt nhất còn lại, và nó đang **tắt mặc định** (xem
  `STATE_OF_THE_PROJECT.md` §3.1), nên chưa có gì để đo trên đường mặc định.
- **Terrain-gen timing** — `cargo test --release` cho test sinh terrain; chưa được đưa vào bộ bench.

## Kết quả

Số dưới đây là **thật**, đo trên phần cứng mục tiêu (i5-14600KF), trung vị Criterion, build release.
Bảng đầy đủ và các cảnh báo diễn giải: [`docs/how-to/BENCHMARKING.md`](docs/how-to/BENCHMARKING.md).

| Hạng mục | Mục tiêu (plan §10.2) | Đo được | Ghi chú |
|---|---|---|---|
| Physics tick | 60 Hz cho active radius | **chưa đo** | Cần in-app tick capture; `integrate_physics_system` riêng nó là 4,9 µs @1.000 agent |
| Brain/sensor | 10–20 Hz, batched | **chưa đo** | Não per-agent đang tắt mặc định (§3.1) |
| Ecology local | 1 Hz | `step_regrowth_gated_strided` **55,0 µs** @256² | Đo được, nhưng ở mức system chứ không phải nhịp |
| Plant/decomposition | 0.1–0.2 Hz | `step_soil` **47,2 µs**, `step_erosion` **20,1 µs** @256² | |
| UI telemetry | 1–5 Hz | **chưa đo** | Thread emit không nằm trong bộ bench |
| Hot-loop allocation | 0 | **đạt** | Các test zero-alloc assert `allocs == 0` |
| Terrain gen (256²) | — | **chưa đo** | `cargo test --release`; chưa vào bộ bench |
| Full-brain agents MVP | 1.000 | **chưa đo** | Tổng các system chạy mỗi tick ở 1.000 agent ≈ **493 µs ≈ 3,0 %** khung hình — **cận dưới**, chưa gồm não |

## Trạng thái khoá số

Các con số hiệu năng **vẫn CHƯA được khoá**, nhưng lý do đã đổi.

Điều kiện trong [`WORLD_SIMULATION_PLAN.md`](WORLD_SIMULATION_PLAN.md) §10.2 — *"Không khóa các con
số hiệu năng trước khi chạy M0.4 trên phần cứng mục tiêu"* — **đã được thoả** cho phần đo được: máy
mục tiêu nay là i5-14600KF và bộ Criterion chạy trên chính nó.

Cái còn thiếu không phải phần cứng mà là **phạm vi**: những gì đo được là cận dưới của một tick, và
ba hàng đắt nhất trong bảng trên (`Physics tick`, `Brain/sensor`, `Full-brain agents MVP`) đòi một
nhịp thật của app đang chạy. Chừng nào chưa có, đừng trích một con số nào ở đây như thể nó là ngân
sách khung hình.

**Tính đến 2026-07-27, ba hàng đó vẫn `chưa đo`, và dụng cụ đo đã có.** Đó là hai câu khác nhau và
phải giữ chúng khác nhau: `core/tick_capture.rs` tồn tại, có test, và đo đúng lịch trình sống — nhưng
**chưa ai chạy app desktop**, nên không có mẫu nào. Khi có, số đọc từ `p50_ns` của file export và
điền vào bảng trên **kèm lệnh và ngày**, theo quy ước phân loại ở
[`STATE_OF_THE_PROJECT.md` §1.1](docs/planning/STATE_OF_THE_PROJECT.md#11-phân-loại-mọi-con-số-trong-tài-liệu).
