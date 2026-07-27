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

Số dưới đây là **thật**, đo trên phần cứng mục tiêu (i5-14600KF). Bảng đầy đủ và các cảnh báo diễn
giải: [`docs/how-to/BENCHMARKING.md`](docs/how-to/BENCHMARKING.md).

⚠️ **Bảng này giờ trộn hai nguồn, và chúng không so trực tiếp được với nhau.** Hàng ghi
*(p50, in-app)* đến từ một lần chạy app desktop **bản debug**; mọi hàng còn lại là trung vị
Criterion **bản release**. Một tick debug chậm hơn release nhiều lần, nên đừng đặt hai con số cạnh
nhau rồi kết luận gì về tỷ lệ giữa chúng. Mỗi hàng đều ghi rõ nguồn.

| Hạng mục | Mục tiêu (plan §10.2) | Đo được | Ghi chú |
|---|---|---|---|
| Physics tick | 60 Hz cho active radius | **338,8 µs** (p50, in-app) | Đo trong app 2026-07-27 — **debug**, 10 agent, `exact: false`; xem [§ Đo trong app](#đo-trong-app--2026-07-27-bản-debug). `integrate_physics_system` riêng nó là 4,9 µs @1.000 agent (release, Criterion) |
| Brain/sensor | 10–20 Hz, batched | **119,8 µs** (p50, in-app) | Cùng lần chạy, `exact: false`. Là **đường legacy**: não per-agent tắt mặc định (§3.1), nên đây **không** phải chi phí suy luận não |
| Ecology local | 1 Hz | `step_regrowth_gated_strided` **55,0 µs** @256² | Đo được, nhưng ở mức system chứ không phải nhịp |
| Plant/decomposition | 0.1–0.2 Hz | `step_soil` **47,2 µs**, `step_erosion` **20,1 µs** @256² | |
| UI telemetry | 1–5 Hz | **113,3 µs** (p50, in-app) | Cùng lần chạy, `exact: true` — nằm trọn giữa hai checkpoint có tên, nên là chi phí thật của pha chứ không phải phần dư |
| Hot-loop allocation | 0 | **đạt** | Các test zero-alloc assert `allocs == 0` |
| Terrain gen (256²) | — | **chưa đo** | `cargo test --release`; chưa vào bộ bench |
| Full-brain agents MVP | 1.000 | **chưa đo — nay chạy được** | Lần chạy in-app 2026-07-27 có **10 agent** nên **không** đóng được hàng này, và cho tới cùng ngày hôm đó nó **không thể** đóng: số founder là hằng số `10` trong `simulation_loop.rs`, bản release không có đường lấy dữ liệu ra. Cả hai đã gỡ (`ANIMA_FOUNDING_POPULATION`, `ANIMA_TICK_CAPTURE_OUT`) — thủ tục ở [BENCHMARKING.md § Checklist lần chạy release](docs/how-to/BENCHMARKING.md#checklist-lần-chạy-release--1000-agent). `full_tick` ở 10 agent là **1,642 ms** (p50, debug); đừng nhân `mean_ns_per_agent` lên 1.000 — xem cảnh báo ở [§ Đo trong app](#đo-trong-app--2026-07-27-bản-debug). Tổng các system mỗi tick ở 1.000 agent ≈ **493 µs ≈ 3,0 %** khung hình vẫn là **cận dưới** Criterion, chưa gồm não |

### Đo trong app — 2026-07-27, bản debug

**Lần đầu tiên có số từ app desktop đang chạy.** Nguồn: `core/tick_capture.rs`, xuất qua
`export_tick_capture` ra `%APPDATA%\com.anima.engine\captures\tick-capture-2026-07-27.json` (file
nằm ở thư mục app-data, **không** trong repo). Mọi hàng dưới đây là 📏 **đo**, theo quy ước phân loại
ở [`STATE_OF_THE_PROJECT.md` §1.1](docs/planning/STATE_OF_THE_PROJECT.md#11-phân-loại-mọi-con-số-trong-tài-liệu).

Bối cảnh của lần chạy, và mọi con số dưới đây chỉ có nghĩa kèm nó:

| | |
|---|---|
| Profile | **debug** (`npm run tauri:dev`) |
| Executor | `multi-threaded` |
| Thế giới | **256 × 256**, `dimensions_measured: true` — đọc từ thế giới đang chạy, không phải hằng số |
| Số agent | **10** (suy ra từ `mean_ns / mean_ns_per_agent`, trùng 10,00 trên cả bảy pha) |
| Não per-agent | **tắt** — `ANIMA_EVOLVED_BRAINS` không đặt, nên `sensor_brain` là đường legacy |
| Mẫu | 1800, sau 300 tick warm-up · `ticks_observed` 2101 · tick 301 → 2101 |
| Phần cứng ghi được | `x86_64` / `windows` / `available_parallelism: 20`; `cpu_model`, `clock_speed`, `installed_ram`, `gpu` **không đo được** |

| Pha | `exact` | p50 | p95 | p99 | % của `full_tick` (p50) |
|---|---|---|---|---|---|
| `full_tick` | ✅ | **1642,2 µs** | 2344,5 µs | 2611,3 µs | 100% |
| `schedule` | ✅ | 1504,6 µs | 2156,0 µs | 2348,2 µs | 91,6% |
| `ecology_resources` | ❌ | **961,4 µs** | 1443,6 µs | 1584,2 µs | 58,5% |
| `physics_movement` | ❌ | 338,8 µs | 550,9 µs | 645,1 µs | 20,6% |
| `sensor_brain` | ❌ | 119,8 µs | 218,0 µs | 306,9 µs | 7,3% |
| `telemetry_publish` | ✅ | 113,3 µs | 248,1 µs | 317,4 µs | 6,9% |
| `schedule_tail` | ❌ | 32,6 µs | 68,4 µs | 106,3 µs | 2,0% |

Ở 60 Hz ngân sách một khung là 16 667 µs, nên `full_tick` p50 dùng **9,9%** và p99 dùng **15,7%** —
ở bản debug, với 10 agent.

Bốn điều phải đọc kèm, nếu không con số sẽ bị hiểu sai:

- **`exact: false` không phải chi phí của một system.** Nó là "phần việc executor làm giữa hai
  checkpoint có tên". Một profiler đo được nhiều hơn thế; capture này thì không, và nói thẳng ra
  điều đó thay vì gán nhãn đẹp cho một con số nó không sở hữu.
- **Đừng ngoại suy theo số agent.** `mean_ns_per_agent` chỉ là `mean_ns / 10`. Pha đắt nhất,
  `ecology_resources`, chạy trên lưới 256² và gần như **không** phụ thuộc số agent — nhân nó lên
  1.000 cho ra 169 ms/tick, một con số không mô tả bất cứ thứ gì. Muốn đóng hàng
  `Full-brain agents MVP` thì phải chạy lại **với 1.000 agent thật**.
- **`dropped_out_of_order: 1`** trên 2101 tick. Executor đa luồng đã xáo trộn thứ tự checkpoint đúng
  một lần và mẫu đó bị loại thay vì được ghi sai. Đây là dữ liệu về chính executor, không phải lỗi
  của lần chạy.
- **`plant_soil_weather` nằm trong `unavailable` kèm lý do**, không phải báo 0: `core::dynamic_fields`
  không có trong lịch trình sống của build này — không resource nào chèn `DynamicFields` và không
  system nào bước nó, nên không có việc thật để đo. Criterion đo trực tiếp các hàm đó.

## Trạng thái khoá số

Các con số hiệu năng **vẫn CHƯA được khoá**, nhưng lý do đã đổi.

Điều kiện trong [`WORLD_SIMULATION_PLAN.md`](WORLD_SIMULATION_PLAN.md) §10.2 — *"Không khóa các con
số hiệu năng trước khi chạy M0.4 trên phần cứng mục tiêu"* — **đã được thoả** cho phần đo được: máy
mục tiêu nay là i5-14600KF và bộ Criterion chạy trên chính nó.

Cái còn thiếu không phải phần cứng mà là **phạm vi**: những gì đo được là cận dưới của một tick, và
ba hàng đắt nhất trong bảng trên (`Physics tick`, `Brain/sensor`, `Full-brain agents MVP`) đòi một
nhịp thật của app đang chạy. Chừng nào chưa có, đừng trích một con số nào ở đây như thể nó là ngân
sách khung hình.

**Ngày 2026-07-27 app desktop đã được chạy một lần, và điều đó đóng hai trong ba hàng — không phải
cả ba.** `Physics tick` và `Brain/sensor` nay có số in-app; `UI telemetry` cũng vậy. Xem
[§ Đo trong app](#đo-trong-app--2026-07-27-bản-debug) để biết lần chạy đó là gì.

`Full-brain agents MVP` **vẫn mở**, và lý do đáng ghi ra: lần chạy có **10 agent**, còn hàng đó được
định nghĩa ở **1.000**. Một capture hợp lệ, đúng lịch trình sống, với `p50` thật — nhưng của một
khối lượng công việc khác. Đóng hàng đó bằng cách nhân lên là biến một phép đo thành một phép đoán
trông giống phép đo, và `ecology_resources` (58,5% của tick, chạy trên lưới 256²) là bằng chứng
ngay tại chỗ rằng phép nhân đó sai.

Cũng phải giữ nguyên: mọi số in-app là **bản debug**. Chúng chứng minh lịch trình sống chạy được và
cho biết tiền đi đâu giữa các pha; chúng **không** phải ngân sách khung hình của sản phẩm. Một lần
chạy release còn thiếu, và nó là việc tiếp theo của hàng này.
