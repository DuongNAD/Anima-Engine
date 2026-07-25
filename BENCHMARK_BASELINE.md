# BENCHMARK BASELINE — M0.4

Tài liệu này mô tả *reproducible benchmark scaffold* của Anima-Engine: cách capture
seed + config + hardware + timings một cách **rẻ và trung thực**, và cách thay các số
proxy bằng số thật trên phần cứng mục tiêu.

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

```bash
# 1) Sinh / cập nhật report (chạy từ repo root):
node scripts/bench_baseline.mjs

# 2) Validate report bằng test S04:
npx vitest run src/__tests__/benchmarkReport.test.ts
```

Các biến môi trường tuỳ chọn (đều có default, không bắt buộc):

| Env | Ý nghĩa | Default |
|---|---|---|
| `ANIMA_BENCH_SEED` | seed ghi vào report | `1337` |
| `ANIMA_BENCH_TIMESTAMP` | ghi chú thời điểm capture (KHÔNG dùng `Date.now`) | `"set on capture"` |

## Phương pháp (methodology)

1. **Reproducibility envelope.** Report luôn ghi lại `seed`, `config`
   (`gridDim=128`, `tickHz=60`, `ticksPerEpoch=1000` — trùng hằng số trong
   [`src-tauri/src/core/sim_rules.rs`](src-tauri/src/core/sim_rules.rs)) và `hardware`
   (`platform`/`release`/`arch`/`cpuModel`/`cpuCount`/`totalMemMB` lấy từ `os.*`).
   Đây là phần bắt buộc để một số đo có thể tái lập.
2. **Timings = PROXY, không phải engine.** Vì không được chạy backend, script đo một
   vòng lặp số học kiểu **fBm** trên lưới 128² (nhiều octave), thuần CPU, không cấp
   phát heap. Nó **không phải** terrain generator thật và **không** chạy trên máy đích;
   nó chỉ chứng minh harness hoạt động và cho `timings` một block non-empty, tái lập
   được. Mỗi entry ghi rõ điều này trong `note`.
3. **Ổn định để diff.** `timestampNote` không đọc đồng hồ máy, nên file diff sạch giữa
   các lần chạy; chỉ các số `ms` đổi theo tải máy. `proxyChecksum` giữ cho vòng lặp
   proxy không bị JIT loại bỏ — **không** phải một metric hiệu năng.

## Thay proxy bằng số thật (trên phần cứng mục tiêu)

Trên **Dell Vostro 3530 (i7-1355U, Iris Xe iGPU + dGPU)**, thay `timings` proxy bằng
một trong hai nguồn đo thật, rồi ghi lại vào bảng dưới:

- **Terrain-gen timing:** chạy `cargo test --release` cho test sinh terrain (đo phần
  worldgen/`terrain.rs`), lấy thời gian thật thay cho `terrain_fbm_proxy`.
- **In-app tick capture:** khi có harness đo tick an toàn (không crash), capture thời
  gian một tick vật lý ở 60 Hz theo ngân sách plan §10.2.

## Kết quả — TEMPLATE (điền trên máy đích)

> Bảng dưới **chưa** được điền số máy đích. Đây là template; giá trị hiện tại trong
> [`benchmark_report.json`](benchmark_report.json) là PROXY trên máy capture.

| Hạng mục | Mục tiêu (plan §10.2) | Đo được trên Vostro 3530 | Ghi chú |
|---|---|---|---|
| Physics tick | 60 Hz cho active radius | _chưa đo_ | in-app tick capture |
| Brain/sensor | 10–20 Hz, batched | _chưa đo_ | |
| Ecology local | 1 Hz | _chưa đo_ | |
| Plant/decomposition | 0.1–0.2 Hz | _chưa đo_ | |
| UI telemetry | 1–5 Hz | _chưa đo_ | |
| Hot-loop allocation | 0 | _chưa đo_ | test đã assert `allocs == 0` |
| Terrain gen (128²) | — | _chưa đo_ | `cargo test --release` |
| Full-brain agents MVP | 1.000 | _chưa đo_ | rồi đo tăng dần |

## Trạng thái khoá số

Các con số hiệu năng **CHƯA được khoá**. Theo
[`WORLD_SIMULATION_PLAN.md`](WORLD_SIMULATION_PLAN.md) §10.2 — *"Không khóa các con số
hiệu năng trước khi chạy M0.4 trên phần cứng mục tiêu"* — baseline chỉ trở thành chính
thức sau khi bảng trên được điền bằng số đo thật trên Dell Vostro 3530. Cho tới lúc đó,
mọi số trong report là **proxy** và chỉ dùng để kiểm tra harness.
