---
title: Đo hiệu năng bằng Criterion
status: active
owner: maintainers
last_reviewed: 2026-07-27
review_cycle: per-release
---

# Đo hiệu năng từng system bằng Criterion

Tài liệu này là **cách làm**. Số đo cam kết nằm ở [§ Baseline](#baseline-2026-07-26); cách
diễn giải một report và metadata bắt buộc vẫn ở
[`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md).

> **Bạn là chủ dự án và chỉ có năm phút?** Bỏ qua phần Criterion. Việc duy nhất máy không làm được
> là [§ Checklist một lần chạy](#checklist-một-lần-chạy-cho-chủ-dự-án-one-run-owner-checklist) —
> một lần mở app, thu cả ba thứ còn thiếu.

## Vì sao có bộ này

Tuyên bố "60 FPS real-time" của dự án **chưa từng được đo**:
[`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md) tự khai số của nó là proxy, vì chạy full
backend đã crash máy dev. Ràng buộc đó không mất đi — nên bộ bench này **không chạy backend**. Mỗi
benchmark lái đúng một system, hoặc một hàm thuần, trên dữ liệu nó tự dựng: không Tauri, không cửa
sổ, không GPU device, không thread mô phỏng.

Đó cũng là lý do Criterion hợp: một khung hình 60 FPS là **16,67 ms**, và ngân sách khung hình là
một **tổng theo system** — nên số theo từng system chính là thứ cấu thành tuyên bố kia.

## Chạy

```bash
cargo bench --bench tick_systems
```

Chạy nhanh hơn khi chỉ cần một con số thô (mặc định là 3 s warm-up + 5 s đo mỗi mục):

```bash
cargo bench --bench tick_systems -- --warm-up-time 1 --measurement-time 3
```

Chỉ một nhóm:

```bash
cargo bench --bench tick_systems -- tick/dynamic_fields
```

Chạy từ `src-tauri/`, **bằng PowerShell chứ không phải Git Bash** — cùng lý do với `cargo test`
(xem `STATE_OF_THE_PROJECT.md` §4).

## So sánh với lần trước

Criterion tự lưu kết quả vào `target/criterion/` và in `change: [...]` so với lần chạy trước trên
cùng máy. Đặt tên một mốc để so về sau:

```bash
cargo bench --bench tick_systems -- --save-baseline before
```

```bash
cargo bench --bench tick_systems -- --baseline before
```

`target/` nằm trong `.gitignore`, nên mốc đó là **cục bộ theo máy**. Mốc dùng chung cho cả dự án là
bảng ở dưới, và nó chỉ có nghĩa khi đi kèm khối phần cứng.

## Tắt / gỡ

`criterion` là `dev-dependency`, không vào binary. Gỡ = xoá mục `[dev-dependencies]`, khối
`[[bench]]` trong [`src-tauri/Cargo.toml`](../../src-tauri/Cargo.toml) và
`src-tauri/benches/`. Không có đường code nào của sản phẩm phụ thuộc vào nó.

Ràng buộc phải giữ: `cargo tree --no-default-features -e normal` **không được** thấy `criterion`.
Đó là cách gate tách feature G2 kiểm tra, và một `dev-dependency` vô hình với nó theo đúng thiết
kế. Đã xác minh 2026-07-26 — 0 kết quả.

---

## Baseline 2026-07-26

**Đây là số đo, không phải proxy.** Build `--release`, mỗi mục 100 mẫu.

> **Đọc số nào: trung vị.** Dòng `time: [a b c]` mà `cargo bench` in ra **không phải trung vị** —
> với lấy mẫu tuyến tính, `b` là **slope estimate**. Hai con số lệch nhau thật: `step_water` cho
> slope 297,6 µs nhưng trung vị 271,5 µs. Bảng dưới dùng **trung vị**, đọc từ
> `median.point_estimate` trong `target/criterion/**/new/estimates.json`, vì nó bền hơn với vài
> mẫu ngoại lai trên một máy desktop có tải nền. Cả trung vị lẫn trung bình đều được ghi vào
> [`benchmark_report.json`](../../benchmark_report.json).

### Phần cứng — đây là máy mục tiêu

| | |
|---|---|
| CPU | Intel Core i5-14600KF · 14 nhân / 20 luồng · 3,5 GHz base |
| RAM | 47,8 GB |
| OS | Windows 11 Pro 10.0.26200 |
| Toolchain | rustc 1.95.0 · cargo 1.95.0 |
| Lệnh | `cargo bench --bench tick_systems -- --warm-up-time 1 --measurement-time 3` |

Phần cứng mục tiêu của dự án nay **chính là máy này** (cập nhật 2026-07-26, thay cho khai báo
Dell Vostro 3530 cũ trong [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md)). Nghĩa là bảng
dưới **là** số đo trên phần cứng mục tiêu.

Điều đó vẫn **không** đủ để đóng `STATE_OF_THE_PROJECT.md` §3.2 — xem [§ Cái vẫn còn
thiếu](#cái-vẫn-còn-thiếu).

### Trường thế giới, 256×256 (kích thước thật, `MapSettings::default()`)

| Hàm | Trung vị | Trung bình | Ghi chú |
|---|---:|---:|---|
| `ResourceField::step_regrowth` | 70,6 µs | 72,2 µs | Không gate, một pass |
| `ResourceField::step_regrowth_gated` | 218,5 µs | 269,9 µs | Hai pass, có ngân sách detritus |
| `ResourceField::step_regrowth_gated_strided` | **55,0 µs** | 57,4 µs | `REGROWTH_STRIDE = 4` — **nhanh hơn 3,97×** bản không stride |
| `DynamicFields::step_water` | **271,5 µs** | 288,7 µs | **Đắt nhất trong mọi system chạy mỗi tick** |
| `DynamicFields::step_soil` | 47,2 µs | 47,8 µs | |
| `DynamicFields::step_erosion` | 20,1 µs | 20,6 µs | Công thức cục bộ, không vận chuyển trầm tích |

### Theo số agent (trung vị)

| Hàm | 100 | 1.000 | 10.000 | Biên/agent |
|---|---:|---:|---:|---:|
| `integrate_physics_system` | 493 ns | 4,9 µs | 49,2 µs | **4,92 ns** |
| `rebuild_spatial_grid_system` | 13,4 µs | 94,3 µs | **734,5 µs** | **72,8 ns** + ~13 µs cố định |

### Ngoài đường tick (trung vị)

| Hàm | Trung vị | Ghi chú |
|---|---:|---|
| `a2c_loss` (batch 32, kiến trúc 15→64→64→{4,1}) | **284,7 µs** | Mỗi bước optimiser, trên **thread learner** — không nằm trong ngân sách khung hình |
| `WorldArtifact::to_bytes` (256²) | 1,46 ms | Artifact ~1,05 MiB |
| `WorldArtifact::from_bytes` (256²) | 1,13 ms | |
| `WorldArtifact::checksum` (256²) | 1,37 ms | |

---

## Ba điều những số này nói

### 1. Ngân sách khung hình — cận dưới, không phải khung hình

Cộng các system chạy mỗi tick, ở 1.000 agent:

| Thành phần | µs |
|---|---:|
| `step_regrowth_gated_strided` | 55,0 |
| `step_water` | 271,5 |
| `step_soil` | 47,2 |
| `step_erosion` | 20,1 |
| `integrate_physics_system` | 4,9 |
| `rebuild_spatial_grid_system` | 94,3 |
| **Tổng** | **≈ 493 µs** |

≈ **3,0 %** của khung hình 16,67 ms. Ở 10.000 agent: ≈ 1,18 ms ≈ **7,1 %**.

**Đây là cận dưới và phải đọc đúng như vậy.** Nó chưa gồm suy luận của não agent, lập lịch ECS,
change detection, thread emit, va chạm, CPG, trao đổi chất, và mọi thứ không có trong bảng. Một
tuyên bố "60 FPS" **không** được rút ra từ con số này.

Ngoại suy tới trần quần thể EB-S12 (~46.500 agent), tuyến tính theo biên/agent đo được:
physics ≈ 229 µs, spatial ≈ 3,40 ms, trường ≈ 394 µs → **≈ 4,02 ms ≈ 24 %** khung hình. Đây là
**ngoại suy, không phải phép đo** — lưới băm có số ô cố định nên mật độ agent mỗi ô tăng theo N, và
hành vi ngoài dải đã đo không được bảo đảm.

### 2. Chi phí không nằm ở chỗ người ta hay đoán

`integrate_physics_system` **rẻ**: 4,92 ns/agent, tuyến tính sạch qua ba bậc. Ở 10.000 agent nó tốn
49 µs — 0,3 % khung hình.

`rebuild_spatial_grid_system` tốn **gấp gần 15 lần** ở cùng số agent (734 µs so với 49 µs). Và hình
dạng chi phí của nó khác: ở 100 agent là 134 ns/agent, ở 10.000 là 73 ns/agent — tức một phần đáng
kể chi phí ở quy mô nhỏ là **quét toàn bộ ô lưới đã cấp phát sẵn**, không phải xử lý agent. Đây là
nơi đáng tối ưu trước, không phải solver vật lý.

`step_water` một mình đắt hơn cả nhóm trường còn lại cộng lại, và đắt hơn physics ở 10.000 agent.

### 3. Con số 4,2 ms trong `ecology.rs` không tái lập được ở đây

Doc comment của `ResourceField::REGROWTH_STRIDE` ghi rằng đường regrowth trước khi stride tốn bốn
pass mỗi tick, **"đo được ~4,2 ms/tick — một phần tư ngân sách khung hình 60 FPS"**.

Đo lại trên máy này, build release: `step_regrowth_gated` = **0,219 ms**. Cộng hai pass
`total_biomass()` mà bản cũ cần (mỗi pass nhiều nhất cỡ `step_regrowth`, ~71 µs) ra **≈ 0,36 ms** —
thấp hơn con số ghi trong doc khoảng **12 lần**.

Hai điều cần tách bạch, vì trộn vào nhau sẽ dẫn tới kết luận sai:

- **Việc stride là đúng và có lợi thật.** Đo được **3,97×**. Không có gì phải rút lại.
- **Con số headline biện minh cho nó thì không tái lập.** Không thể kết luận doc sai từ đây: bản đo
  cũ có thể ở build debug, trên máy khác, hoặc gồm cả công việc khác trong cùng tick. Đây là một
  **finding cần đối chứng**, không phải một lỗi đã xác định — theo quy tắc 6 của
  [chính sách tài liệu](../governance/DOCUMENTATION_POLICY.md), mở finding chứ không tự coi bên nào
  đúng.

## In-app tick capture (2026-07-27)

Criterion đo **từng hàm**, trên fixture tự dựng, ngoài mọi engine đang chạy. Cái nó theo thiết kế
không đo được là **một tick của lịch trình sống**: lập lịch ECS, change detection, suy luận, khối
publish telemetry. [`src-tauri/src/core/tick_capture.rs`](../../src-tauri/src/core/tick_capture.rs)
đo đúng chỗ đó, **không** chạy app desktop.

Bật bằng biến môi trường khi khởi động engine, hoặc bằng bốn lệnh IPC
(`start_tick_capture` / `get_tick_capture_status` / `stop_tick_capture` / `export_tick_capture` —
xem [`PROJECT.md`](../../PROJECT.md) §Interface Contracts):

```bash
ANIMA_TICK_CAPTURE=warmup=300,capacity=1800,every=2
```

Ba pha là **chính xác**, vì vòng lặp mô phỏng kẹp chúng trực tiếp bằng một `Instant`:
`schedule` (quanh `schedule.run`), `telemetry_publish` (quanh khối trích xuất/publish), và
`full_tick` (tổng hai cái, **không** gồm giấc ngủ giữ nhịp khung hình).

Bốn pha còn lại (`sensor_brain`, `physics_movement`, `ecology_resources`, `schedule_tail`) là
**checkpoint-bounded**, và khác biệt đó nằm trong JSON xuất ra chứ không chỉ trong tài liệu này:
Bevy không có hook đo từng system, và tách lịch trình để đo sẽ **đổi thứ tự thực thi** — đúng thứ
một profiler không được phép làm. Mỗi checkpoint chỉ được bảo đảm chạy **sau** một system có tên;
một pha là "phần việc executor làm giữa hai checkpoint". `PhaseSummary.exact` nói hàng đó thuộc loại
nào, `CaptureExport.executor` nói executor nào đã tạo ra số.

Quy ước phân vị là **nearest rank** (`ceil(p/100·n) − 1`), nên mọi con số in ra là một mẫu thật, không
phải nội suy. `plant_soil_weather` xuất hiện trong `unavailable` kèm lý do, vì `core::dynamic_fields`
**không** nằm trong lịch trình sống — báo 0 cho nó sẽ là bịa.

**Đã chạy một lần trên app desktop đầy đủ — 2026-07-27.** Chủ dự án chạy checklist dưới đây và thu
được 1800 mẫu: `full_tick` p50 **1642,2 µs**, bản **debug**, executor multi-threaded, 256², **10
agent**. Số và toàn bộ bối cảnh ở
[`BENCHMARK_BASELINE.md` § Đo trong app](../../BENCHMARK_BASELINE.md#đo-trong-app--2026-07-27-bản-debug).

**Cái vẫn chưa có:** một lần chạy **release**, và một lần chạy ở **1.000 agent**. Lần chạy trên là
debug với 10 agent, nên nó đóng ba hàng của bảng baseline và **không** đóng hàng
`Full-brain agents MVP`. Bằng chứng headless vẫn nguyên giá trị của nó — cùng lịch trình
(`simulation_schedule::build_tick_schedule`, đúng hàm `SimulationEngine::start` gọi), qua
`tests/tick_capture_tests.rs`.

Thủ tục đo cần **một con người mở app**, và nó nằm ở
[§ Checklist một lần chạy](#checklist-một-lần-chạy-cho-chủ-dự-án-one-run-owner-checklist) bên dưới.
CLAUDE.md cấm agent chạy full backend trên máy dev này, nên không phiên tự động nào được thực hiện
bước đó — checklist tồn tại để chủ dự án chạy nó, không phải để agent chạy hộ.

## Checklist một lần chạy cho chủ dự án (one-run owner checklist)

> **MỘT lần khởi động. MỘT phiên. Khoảng 5 phút.** Phiên này thu ba thứ mà không gate nào trong
> repo lấy được vì cả ba đều cần một con người mở app:
>
> | # | Thu cái gì | Đóng phần nào |
> |---|---|---|
> | (a) | App có khởi động dưới CSP không, console có vi phạm không | [deployment §2.1](../ai/deployment/2026-07-27-feature-anima-completion.md) |
> | (b) | Một file export tick capture | các hàng `chưa đo` trong [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md) |
> | (c) | Một ảnh chụp màn hình | bằng chứng thị giác cho (a) |
>
> Đừng chạy ba lần cho ba việc — chúng nằm trong **cùng một cửa sổ app**.
>
> ⚠️ **Phiên này KHÔNG chứng minh sẵn sàng phát hành.** Nó chỉ điền bằng chứng còn thiếu. Xem
> [§ Phiên này đóng gì và không đóng gì](#phiên-này-đóng-gì-và-không-đóng-gì) trước khi trích bất kỳ
> con số nào.

### Bước 0 — Chuẩn bị, làm TRƯỚC và **không** tính trong 5 phút

Lần biên dịch đầu của backend (Bevy + Burn) mất hàng chục phút. Nếu chạy thẳng `npm run tauri:dev`
trên `target/` nguội thì "5 phút" là sai. Hâm nóng trước, từ repo root rồi `src-tauri/`:

```powershell
npm install
npm run build
```

```powershell
cargo build --features desktop
```

`cargo build` ở đây **là biên dịch, không phải chạy app** — nó không vi phạm cảnh báo vận hành ở
đầu [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md). Nó hâm nóng toàn bộ cây phụ thuộc; khi
`tauri dev` chạy, chỉ crate `anima-engine` biên dịch lại (`generate_context!` đổi chế độ giữa
production và dev), nên phần lớn thời gian đã trả xong.

Cần thêm: cổng **5173** trống (`strictPort`), vì `beforeDevCommand` tự chạy `npm run dev`.

> ⚠️ **Bẫy tốn nhiều thời gian nhất của lần chạy 2026-07-27, nay đã sửa ở tầng cấu hình — nhưng
> vẫn phải kiểm cổng.** Hôm đó `devUrl` là `http://localhost:5173`, và Node phân giải `localhost`
> theo thứ tự *verbatim* từ v17 — `::1` trước, trên Windows lẫn Linux. `[::1]:5173` đang bị dev
> server của **một project Vite khác** giữ (5173 là mặc định của Vite, nên mọi project đều muốn nó),
> còn server của Anima bind được `127.0.0.1:5173`. Cả hai cùng sống, và webview mở **ứng dụng của
> project kia bên trong cửa sổ Anima**: một trang trắng, không lỗi ở đâu cả — dev server báo `ready`,
> trang trả 200, và CSP cho phép `http://localhost:5173` vì cả hai server đều là địa chỉ đó.
>
> `devUrl` và `vite.config.ts` nay đều ghim `127.0.0.1`, nên không còn bước phân giải nào để một
> project khác chen vào. Cái **chưa** biến mất là `strictPort`: cổng 5173 bận thì `npm run dev` chết
> ngay từ `beforeDevCommand`. Kiểm trước, và đọc **cả cột địa chỉ**:
>
> ```powershell
> Get-NetTCPConnection -LocalPort 5173 -State Listen | Select-Object LocalAddress, OwningProcess
> ```
>
> Và ngay khi cửa sổ hiện, vẫn kiểm định danh trước khi tin bất cứ thứ gì nó vẽ ra:
> `window.__TAURI_INTERNALS__ !== undefined` phải là `true`, và tiêu đề phải là "Anima-Engine Control
> Center". Hai giây, và nó là thứ duy nhất phân biệt "app của bạn" với "một app nào đó ở đúng địa
> chỉ đó". Xem [deployment §2.4](../ai/deployment/2026-07-27-feature-anima-completion.md).

### Bước 1 — Đặt biến môi trường, rồi mở app (PowerShell, cùng một cửa sổ)

Biến này được đọc bởi **tiến trình Rust** ở `SimulationEngine::start`
([`core/simulation_loop.rs`](../../src-tauri/src/core/simulation_loop.rs)), nên phải đặt **trước
khi** app khởi động — đặt sau khi cửa sổ đã mở là vô tác dụng.

```powershell
$env:ANIMA_TICK_CAPTURE = "warmup=300,capacity=1800,max_samples=1800"
npm run tauri:dev
```

Vì sao đúng ba tham số đó: 300 tick warm-up ≈ **5 s** ở 60 Hz (bỏ tick lạnh), rồi 1800 mẫu liên
tiếp ≈ **30 s**. Khi đủ 1800 mẫu, capture tự chuyển sang trạng thái `Complete` và **ngừng tiêu thụ
tick** — một điểm dừng rõ ràng, không phải một con số bạn tự đoán là đã đủ. Tổng ≈ 2100 tick ≈ 35 s
đồng hồ. Cú pháp và mọi khoá khác: `CaptureConfig::parse` trong
[`core/tick_capture.rs`](../../src-tauri/src/core/tick_capture.rs).

- ✅ **Đúng:** cửa sổ "Anima Engine - Evolution Simulator" mở ra.
- ❌ **Sai:** terminal in `ANIMA_TICK_CAPTURE is not a usable configuration (...)` → capture **tắt**,
  app vẫn chạy bình thường và bạn sẽ thu được một file rỗng. Sửa chuỗi rồi mở lại.

### Bước 2 — Mở DevTools và **reload** để bắt vi phạm CSP lúc tải trang

`npm run tauri:dev` build ở chế độ debug, nên DevTools của webview luôn có: chuột phải trong cửa sổ
app → **Inspect**.

DevTools mở *sau* khi trang đã tải sẽ **không** có log của lần tải đó. Vì vậy, trong tab
**Console**, gõ:

```js
location.reload()
```

Reload chỉ tải lại webview; tiến trình Rust và luồng mô phỏng **không** khởi động lại, nên capture
không bị ảnh hưởng. Bây giờ Console đang mở trong suốt một lần tải trang đầy đủ.

- ✅ **Đúng:** giao diện "Anima-Engine Control Center" hiện ra, Console **không** có dòng nào chứa
  `Content Security Policy` / `Refused to …`.
- ❌ **Sai:** bất kỳ `Refused to load/connect/execute …` nào → chép nguyên văn, đó chính là phát
  hiện phiên này tồn tại để tìm.
- ❌ **Sai:** `window.__TAURI_INTERNALS__ === undefined` → bạn đang xem `localhost:5173` trong một
  trình duyệt thường, **không** phải webview Tauri. Mọi `invoke` sẽ reject; đây đúng là lỗi đã ghi
  ở [implementation §9](../ai/implementation/2026-07-27-feature-anima-completion.md).

### Bước 3 — Chạy mô phỏng (đây là lúc capture bắt đầu tiêu thụ tick)

Nút lớn bên trái, dưới header:

| Nhãn nút | Nghĩa | Làm gì |
|---|---|---|
| `Đang dựng thế giới chung…` (xám, disabled) | thế giới 2048² đang sinh (~7 s lần đầu) | **đợi** |
| `Bắt đầu mô phỏng` (xanh) | sẵn sàng | **bấm** |
| `Dừng mô phỏng` (đỏ) | đang chạy | không bấm |

Bấm rồi để yên **~40 giây**. Theo dõi ô **`Số Ticks:`** trong bảng trạng thái — nó phải tăng đều và
cần **tăng thêm ~2100** kể từ lúc bấm. (Đếm mức tăng, không đếm giá trị tuyệt đối: nếu app vừa nạp
một autosave thì bộ đếm chạy tiếp từ số đã lưu.) Điều kiện dừng thật nằm ở bước 4.

> Nếu app đã tự chạy ngay khi mở (có autosave ở `saves/autosave.json` thì
> [`lib.rs`](../../src-tauri/src/lib.rs) khởi động engine ngay lúc setup), capture đã chạy từ đầu và
> `Số Ticks` đã lớn. Không sao — sang bước 4 và đọc trạng thái.

### Bước 4 — Xác nhận capture đã xong, trong Console

Không có nút bấm nào cho tick capture: bốn lệnh là **IPC thuần**, chưa có UI gọi chúng (kiểm chứng:
không file nào trong `src/` nhắc tới `tick_capture`). Đường duy nhất là Console. `@tauri-apps/api`
định tuyến mọi `invoke` qua `window.__TAURI_INTERNALS__.invoke(cmd, args)` — cùng một mặt tiếp xúc
mà webview thật cài đặt, ghi ở [`tests/e2e/tauri-mock.ts`](../../tests/e2e/tauri-mock.ts).

```js
await window.__TAURI_INTERNALS__.invoke('get_tick_capture_status')
```

- ✅ **Đúng:** `status: "Complete"` và `accounting.samples_recorded: 1800`.
- 🟡 **Chấp nhận được:** `status: "Recording"` với `samples_recorded` vài trăm — đợi thêm rồi gọi
  lại. Bất kỳ số mẫu nào > 0 cũng xuất được, chỉ là ít mẫu hơn.
- ❌ **Sai:** `status: "Idle"` → **hai nguyên nhân, phân biệt bằng `accounting.ticks_observed`.** Khối
  đọc `ANIMA_TICK_CAPTURE` nằm **bên trong** luồng engine
  ([`core/simulation_loop.rs`](../../src-tauri/src/core/simulation_loop.rs), tại
  `CaptureConfig::from_env()`), nên nó chỉ chạy khi engine khởi động:
  - `ticks_observed: 0` → **engine chưa từng chạy**. Bạn chưa bấm `Bắt đầu mô phỏng`, không phải
    biến môi trường sai. Quay lại bước 3. *(Đo được 2026-07-27: đây là trường hợp thực tế xảy ra, và
    bản trước của dòng này chỉ ghi nguyên nhân kia nên người chạy tưởng mình gõ sai chuỗi.)*
  - `ticks_observed` > 0 → engine đã chạy nhưng **biến môi trường không tới được tiến trình**; xem
    bước 1. Không cần khởi động lại app — dùng đường IPC ở dưới.
- ❌ **Sai:** `workload.dimensions_measured: false` → chưa tick nào đi qua, cùng nghĩa với
  `ticks_observed: 0`.
- ❌ **Đáng ghi lại:** `dropped_out_of_order` > 0 → executor đa luồng đã xáo trộn các checkpoint.
  Không phải lỗi của bạn; **chép con số đó lại**, nó là dữ liệu về chính executor. *(Đo được
  2026-07-27: **1** trên 2101 tick.)*

#### Đường IPC — không cần biến môi trường, không cần khởi động lại

Biến môi trường ở bước 1 chỉ tiện khi bạn nhớ đặt nó **trước** khi app mở. Nếu quên, hoặc muốn đo
lại với tham số khác, `start_tick_capture` làm đúng việc đó **giữa lúc đang chạy** — sink được chèn
vô điều kiện chính vì lý do này, nên không phải dựng lại thế giới:

```js
await window.__TAURI_INTERNALS__.invoke('start_tick_capture', { config: { warmup_ticks: 300, capacity: 1800, max_samples: 1800, sample_every: 1, groups: 127 } })
```

Trả về `null` là thành công (`Result<(), String>`); một chuỗi là thông báo từ chối của
`CaptureConfig::validate`, không phải giá trị bị làm tròn cho hợp lệ.

Hai chỗ dễ sai trong object đó, cả hai đều **im lặng** nếu gõ sai:

- **Khoá là `snake_case`.** `CaptureConfig` không khai `#[serde(rename_all)]`, nên `warmupTicks`
  không phải tên nó — khác với `fileName` ở bước 5, vốn là **tham số lệnh** nên bị đổi sang camelCase.
  Cùng một lệnh gọi có thể có cả hai quy ước, và đó không phải nhầm lẫn.
- **`groups` là một số, không phải mảng.** `PhaseMask(pub u16)` là newtype nên qua JSON là số trần.
  `127` = đủ bảy pha (`TickPhase::ALL`, index 0–6). Mask rỗng bị `validate` từ chối; một mask hẹp
  **không** làm đổi con số của mask rộng, nó chỉ đổi cái được xuất ra.

Engine chưa chạy thì bật bằng chính console, `toggle_simulation` không nhận tham số nào và trả `true`
khi vừa khởi động:

```js
await window.__TAURI_INTERNALS__.invoke('toggle_simulation')
```

### Bước 5 — Xuất file (chú ý: tên khoá là `fileName`, KHÔNG phải `file_name`)

```js
await window.__TAURI_INTERNALS__.invoke('export_tick_capture', { fileName: 'tick-capture-2026-07-27' })
```

> ⚠️ **Bẫy im lặng, và nó là bẫy duy nhất nguy hiểm ở đây.** `#[tauri::command]` chuyển tên tham số
> sang **camelCase** (mặc định `ArgumentCase::Camel`, `tauri-macros/src/command/wrapper.rs`), và
> tham số là `Option<String>` — nên gõ `file_name` **không** báo lỗi: lệnh trả về đúng tài liệu
> JSON, in đẹp ra Console, và **không ghi file nào cả**. Console xanh, ổ đĩa trống. Phải là
> `fileName`.
>
> Tên file theo hợp đồng tên save trong [`commands/save_paths.rs`](../../src-tauri/src/commands/save_paths.rs):
> chỉ `[A-Za-z0-9._-]`, không dấu gạch chéo, không đường dẫn; đuôi `.json` được tự thêm.

File nằm ở thư mục app-data, **không** phải trong repo:

```powershell
Get-ChildItem "$env:APPDATA\com.anima.engine\captures"
```

- ✅ **Đúng:** thấy `tick-capture-2026-07-27.json`, kích thước vài KB.
- ❌ **Sai:** thư mục trống hoặc không tồn tại → gần như chắc chắn là đã gõ `file_name`. Gọi lại
  bước 5 với `fileName`.

Xem nhanh các con số quan trọng mà không cần mở editor:

```powershell
(Get-Content "$env:APPDATA\com.anima.engine\captures\tick-capture-2026-07-27.json" -Raw | ConvertFrom-Json).phases | Format-Table phase, exact, count, p50_ns, p95_ns
```

`p50_ns` là cột điền vào [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md), theo ánh xạ ở
[§ File này điền được hàng nào](#file-này-điền-được-hàng-nào).

### Bước 6 — Chụp một ảnh, **trước khi** đóng app

`Win`+`Shift`+`S`, chọn cả cửa sổ app **và** panel DevTools trong cùng một khung. Một ảnh phải thấy
được cả ba: tiêu đề cửa sổ, `Số Ticks` > 2100, và tab Console không có vi phạm CSP.

Lưu vào một đường dẫn bạn tự chọn, ví dụ `%USERPROFILE%\Desktop\anima-csp-boot-2026-07-27.png`.

> **Repo chưa có chỗ chuẩn cho ảnh này** và checklist này không tạo ra một chỗ mới. Đừng để nó vào
> [`map-views/`](../../map-views): tám PNG ở đó là artifact khác hẳn, bị `map_manifest.json` ghim
> bằng SHA-256 và `tests/frontend/mapManifestEvidence.test.ts` kiểm — thêm file lạ vào đó là làm
> nhiễu một gate đang xanh.

### Bước 7 — Đóng app, rồi gửi lại bốn thứ

Đóng cửa sổ bình thường (khi thoát, app tự ghi autosave — đó là hành vi bình thường, không phải lỗi).

Gửi lại cho agent:

1. **File** `%APPDATA%\com.anima.engine\captures\tick-capture-2026-07-27.json` — nguyên vẹn, đừng sửa tay.
2. **Ảnh** ở bước 6.
3. **Nội dung Console**: chép nguyên văn mọi dòng đỏ/vàng, hoặc câu "Console sạch, không có dòng nào
   chứa `Content Security Policy`".
4. **Kết quả `get_tick_capture_status`** ở bước 4 (chỉ cần `status` và khối `accounting`).

Với bốn thứ đó, agent điền được bảng trong `BENCHMARK_BASELINE.md` và cập nhật
[`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền)
kèm lệnh và ngày. Thiếu (1) thì không có số; thiếu (3) thì (a) chưa đóng.

### File này điền được hàng nào

| Hàng trong `BENCHMARK_BASELINE.md` | Đọc từ | `exact` |
|---|---|---|
| `Physics tick` | `phases[] .phase == "physics_movement"` → `p50_ns` | **false** — chặn bởi checkpoint |
| `Brain/sensor` | `phases[] .phase == "sensor_brain"` → `p50_ns` | **false** — chặn bởi checkpoint |
| `UI telemetry` | `phases[] .phase == "telemetry_publish"` → `p50_ns` | **true** |
| `Full-brain agents MVP` | `phases[] .phase == "full_tick"` → `p50_ns` + `mean_ns_per_agent`, kèm `workload` | **true** |

Ba điều phải ghi kèm, nếu không con số sẽ bị đọc sai:

- **`exact: false` nghĩa là "phần việc executor làm giữa hai checkpoint"**, không phải chi phí của
  một system. Chép cả cờ đó sang, đừng chỉ chép `p50_ns`.
- **`profile` trong file sẽ là `"debug"`**, vì `tauri dev` build debug. Bảng Criterion ở trên là
  **release**. Hai con số **không** so trực tiếp được với nhau; ghi rõ profile bên cạnh mỗi số.
- **Não per-agent đang tắt mặc định** (`ANIMA_EVOLVED_BRAINS` không đặt, §3.1). Nên `sensor_brain`
  đo **đường legacy**, không phải suy luận não. Điền hàng `Brain/sensor` như "đường mặc định, não
  tắt" chứ không phải "chi phí não".

### Phiên này đóng gì và không đóng gì

**Đóng:** app khởi động thật, IPC thật trả lời, và một tick của lịch trình sống có số đo đầu tiên
trên phần cứng mục tiêu.

**Không đóng — và đây là giới hạn phải giữ nguyên văn:**

- **CSP: đây là `devCsp`, không phải chính sách xuất xưởng.** `tauri dev` áp `app.security.devCsp`;
  `npm run check:csp` kiểm `app.security.csp`. Hai khối trong
  [`tauri.conf.json`](../../src-tauri/tauri.conf.json) khai cùng 13 directive, giống nhau ở **11**
  và khác đúng **hai**: `script-src` (dev cho phép thêm `'unsafe-inline'` và
  `http://localhost:5173`) và `connect-src` (dev thêm `ws://localhost:5173` +
  `http://localhost:5173`). Nên phiên này xác minh 11 directive **đúng như bản ship**, và để lại
  hai directive kia chỉ được xác minh ở dạng lỏng hơn.
  **Finding đang mở:** [deployment §2.1](../ai/deployment/2026-07-27-feature-anima-completion.md)
  mô tả "một `npm run tauri:dev` bởi con người" là đủ để đóng gate CSP. Theo đo đạc trên thì chưa
  đủ đúng như đã viết. Chưa sửa câu đó ở đây — theo quy tắc 6 của
  [chính sách tài liệu](../governance/DOCUMENTATION_POLICY.md), mở finding thay vì tự coi bên nào
  đúng.
- **Vì sao không dùng bản release để lấp hai directive đó:** bản release **không có DevTools**.
  `tauri` trong [`src-tauri/Cargo.toml`](../../src-tauri/Cargo.toml) khai `features = ["test"]`,
  không có `devtools` — mà DevTools là đường duy nhất để vừa đọc console vừa gọi
  `export_tick_capture` (chưa có UI). Một lần chạy release sẽ mất **cả** (a-console) lẫn (b). Đây là
  giới hạn thật của sản phẩm hôm nay, không phải một lựa chọn cho tiện.
- **Không phải "60 FPS đã được chứng minh".** Một capture là phân bố của tick, trên một workload,
  một profile, một máy.
- **Không phải "live Bevy world experiment-ready".** Tuyên bố duy nhất được phép vẫn là
  *headless adapter verified* — xem CLAUDE.md.
- **`plant_soil_weather` sẽ nằm trong `unavailable` kèm lý do.** Đó là đúng, không phải thiếu sót:
  `core::dynamic_fields` không nằm trong lịch trình sống.

## Cái vẫn còn thiếu

Phần cứng mục tiêu nay đã khớp, nhưng §3.2 của
[`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) vẫn **chưa đóng**, và lý do không
phải phần cứng:

- **Đây là cận dưới của tick, không phải khung hình.** Các hàng "Physics tick 60 Hz",
  "Brain/sensor 10–20 Hz" trong [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md) hỏi một
  **nhịp thực tế của app đang chạy**. Dụng cụ đo nay đã có (mục trên) và đã được kiểm bằng test;
  cái còn thiếu là **một lần chạy app thật** để lấy số — ràng buộc "không chạy full backend" trên
  máy này vẫn còn hiệu lực với agent. Thủ tục để một con người chạy nó một lần:
  [§ Checklist một lần chạy](#checklist-một-lần-chạy-cho-chủ-dự-án-one-run-owner-checklist).
- **Chưa có số cho phần đắt nhất còn lại:** suy luận não per-agent. Nó đang tắt mặc định (§3.1), nên
  chưa có gì để đo trên đường mặc định.
- `config.gridDim` trong [`benchmark_report.json`](../../benchmark_report.json) vẫn ghi **128**,
  trong khi thế giới thật chạy **256²** (`MapSettings::default()`) và `DEFAULT_GRID_DIM` trong
  `sim_rules.rs` **đã là 256** kể từ 2026-07-27 (test `s03_default_grid_dim_tracks_map_settings_default`
  ghim nó). Bộ bench này dùng 256², và tick capture đọc kích thước **từ `ResourceField` của thế giới
  đang chạy** rồi ghi vào `CaptureExport.workload` kèm cờ `dimensions_measured` — nên số 128 còn lại
  chỉ nằm trong file report cũ, không nằm trong đường đo nào.

## Cách thêm một benchmark

Sửa [`src-tauri/benches/tick_systems.rs`](../../src-tauri/benches/tick_systems.rs). Bốn quy tắc mà
file đó đang giữ, và một bench mới nên giữ tiếp:

1. **Dùng kích thước thật.** Trường được dựng ở 256² vì đó là kích thước engine chạy. Đo ở 32² sẽ
   cho một con số dễ chịu cho một workload không tồn tại.
2. **Dùng hằng số của engine, đừng chép.** Bench learner import `STATE_DIM`/`HIDDEN_DIM`/
   `ACTION_DIM`/`BATCH_SIZE` từ `core::training`. Chép giá trị vào bench nghĩa là lần đầu kiến trúc
   đổi, bench vẫn chạy, vẫn in ra một con số — cho một mạng engine không còn dùng.
3. **Dựng fixture ở trạng thái làm việc thật.** `from_biomes` khởi tạo mọi ô **ở** sức chứa, mà tăng
   trưởng logistic tại `r == r_max` bằng đúng 0 — một trường mới tinh sẽ đo nhánh thoát sớm chứ
   không đo regrowth. Fixture ở đây hạ xuống nửa sức chứa.
4. **Đừng hoist thứ engine không hoist.** Bench `a2c_loss` dựng lại tensor mỗi vòng, vì learner
   dựng chúng mới từ buffer transition ở mỗi bước.

Benchmark bị `cargo clippy --all-targets` biên dịch và lint trong CI ở **cả hai** cấu hình feature,
nên code bench phải sạch clippy và phải biên dịch được với `--no-default-features`.
