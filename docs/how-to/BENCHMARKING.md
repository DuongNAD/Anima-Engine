---
title: Đo hiệu năng bằng Criterion
status: active
owner: maintainers
last_reviewed: 2026-07-26
review_cycle: per-release
---

# Đo hiệu năng từng system bằng Criterion

Tài liệu này là **cách làm**. Số đo cam kết nằm ở [§ Baseline](#baseline-2026-07-26); cách
diễn giải một report và metadata bắt buộc vẫn ở
[`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md).

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

## Cái vẫn còn thiếu

Phần cứng mục tiêu nay đã khớp, nhưng §3.2 của
[`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) vẫn **chưa đóng**, và lý do không
phải phần cứng:

- **Đây là cận dưới của tick, không phải khung hình.** Các hàng "Physics tick 60 Hz",
  "Brain/sensor 10–20 Hz" trong [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md) hỏi một
  **nhịp thực tế của app đang chạy**, mà bộ này theo thiết kế không chạy app. Cần một in-app tick
  capture, và ràng buộc "không chạy full backend" vẫn còn hiệu lực.
- **Chưa có số cho phần đắt nhất còn lại:** suy luận não per-agent. Nó đang tắt mặc định (§3.1), nên
  chưa có gì để đo trên đường mặc định.
- `config.gridDim` trong [`benchmark_report.json`](../../benchmark_report.json) vẫn ghi **128**
  (`DEFAULT_GRID_DIM` trong `sim_rules.rs`), trong khi thế giới thật chạy **256²**
  (`MapSettings::default()`) và hằng số 128 kia **không được đọc ở đâu trong `src/`**. Bộ bench này
  dùng 256². Đây là một finding riêng đang mở, không sửa ở đây vì nó chạm vào
  `COORDINATE_CONTRACT.md`.

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
