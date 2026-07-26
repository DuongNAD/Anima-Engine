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

**Đây là số đo, không phải proxy.** Trung vị Criterion, build `--release`, mỗi mục 100 mẫu.

### Phần cứng

| | |
|---|---|
| CPU | Intel Core i5-14600KF · 14 nhân / 20 luồng · 3,5 GHz base |
| RAM | 47,8 GB |
| OS | Windows 11 Pro 10.0.26200 |
| Toolchain | rustc 1.95.0 · cargo 1.95.0 |
| Lệnh | `cargo bench --bench tick_systems -- --warm-up-time 1 --measurement-time 3` |

> ⚠️ **Đây KHÔNG phải phần cứng mục tiêu đã khai.** [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md)
> đặt mục tiêu là *Dell Vostro 3530 (i7-1355U, Iris Xe iGPU + dGPU)* — một máy laptop khác hẳn. Vì
> vậy bảng này **chưa** đóng được mệnh đề "số thật trên phần cứng mục tiêu" của
> `STATE_OF_THE_PROJECT.md` §3.2. Hoặc khai báo phần cứng mục tiêu đã lỗi thời và cần cập nhật,
> hoặc bảng này phải được chạy lại trên máy kia. Không tự chọn giúp — đó là quyết định của người
> duy trì.

### Trường thế giới, 256×256 (kích thước thật, `MapSettings::default()`)

| Hàm | Trung vị | Ghi chú |
|---|---:|---|
| `ResourceField::step_regrowth` | **72,0 µs** | Không gate, một pass |
| `ResourceField::step_regrowth_gated` | **240,7 µs** | Hai pass, có ngân sách detritus |
| `ResourceField::step_regrowth_gated_strided` | **57,0 µs** | `REGROWTH_STRIDE = 4` — **nhanh hơn 4,22×** bản không stride |
| `DynamicFields::step_water` | **297,6 µs** | **Đắt nhất trong mọi system chạy mỗi tick** |
| `DynamicFields::step_soil` | 47,5 µs | |
| `DynamicFields::step_erosion` | 20,1 µs | Công thức cục bộ, không vận chuyển trầm tích |

### Theo số agent

| Hàm | 100 | 1.000 | 10.000 | Biên/agent |
|---|---:|---:|---:|---:|
| `integrate_physics_system` | 494 ns | 4,91 µs | 50,5 µs | ~5,0 ns |
| `rebuild_spatial_grid_system` | 13,7 µs | 90,8 µs | 609,0 µs | ~60 ns + ~13 µs cố định |

### Ngoài đường tick

| Hàm | Trung vị | Ghi chú |
|---|---:|---|
| `a2c_loss` (batch 32, kiến trúc 15→64→64→{4,1}) | **309,7 µs** | Mỗi bước optimiser, trên **thread learner** — không nằm trong ngân sách khung hình |
| `WorldArtifact::to_bytes` (256²) | 1,62 ms | ~1,05 MiB · ~656 MiB/s |
| `WorldArtifact::from_bytes` (256²) | 1,25 ms | ~852 MiB/s |
| `WorldArtifact::checksum` (256²) | 1,39 ms | ~766 MiB/s |

---

## Ba điều những số này nói

### 1. Ngân sách khung hình — cận dưới, không phải khung hình

Cộng các system chạy mỗi tick, ở 1.000 agent:

| Thành phần | µs |
|---|---:|
| `step_regrowth_gated_strided` | 57,0 |
| `step_water` | 297,6 |
| `step_soil` | 47,5 |
| `step_erosion` | 20,1 |
| `integrate_physics_system` | 4,9 |
| `rebuild_spatial_grid_system` | 90,8 |
| **Tổng** | **≈ 518 µs** |

≈ **3,1 %** của khung hình 16,67 ms. Ở 10.000 agent: ≈ 1,08 ms ≈ **6,5 %**.

**Đây là cận dưới và phải đọc đúng như vậy.** Nó chưa gồm suy luận của não agent, lập lịch ECS,
change detection, thread emit, va chạm, CPG, trao đổi chất, và mọi thứ không có trong bảng. Một
tuyên bố "60 FPS" **không** được rút ra từ con số này.

Ngoại suy tới trần quần thể EB-S12 (~46.500 agent), tuyến tính theo biên/agent đo được:
physics ≈ 233 µs, spatial ≈ 2,80 ms, trường ≈ 422 µs → **≈ 3,46 ms ≈ 21 %** khung hình. Đây là
**ngoại suy, không phải phép đo** — lưới băm có số ô cố định nên mật độ agent mỗi ô tăng theo N, và
hành vi ngoài dải đã đo không được bảo đảm.

### 2. Chi phí không nằm ở chỗ người ta hay đoán

`integrate_physics_system` **rẻ**: 5 ns/agent, tuyến tính sạch qua ba bậc. Ở 10.000 agent nó tốn
50 µs — 0,3 % khung hình.

`rebuild_spatial_grid_system` tốn **gấp 12 lần** ở cùng số agent (609 µs so với 50 µs). Và hình
dạng chi phí của nó khác: ở 100 agent là 137 ns/agent, ở 10.000 là 61 ns/agent — tức phần lớn chi
phí ở quy mô nhỏ là **quét toàn bộ ô lưới đã cấp phát sẵn**, không phải xử lý agent. Đây là nơi
đáng tối ưu trước, không phải solver vật lý.

`step_water` một mình đắt hơn cả nhóm trường còn lại cộng lại, và đắt hơn physics ở 10.000 agent.

### 3. Con số 4,2 ms trong `ecology.rs` không tái lập được ở đây

Doc comment của `ResourceField::REGROWTH_STRIDE` ghi rằng đường regrowth trước khi stride tốn bốn
pass mỗi tick, **"đo được ~4,2 ms/tick — một phần tư ngân sách khung hình 60 FPS"**.

Đo lại trên máy này, build release: `step_regrowth_gated` = **0,241 ms**. Cộng hai pass
`total_biomass()` mà bản cũ cần (mỗi pass cỡ `step_regrowth`, ~72 µs) ra **≈ 0,34 ms** — thấp hơn
con số ghi trong doc khoảng **12 lần**.

Hai điều cần tách bạch, vì trộn vào nhau sẽ dẫn tới kết luận sai:

- **Việc stride là đúng và có lợi thật.** Đo được **4,22×**. Không có gì phải rút lại.
- **Con số headline biện minh cho nó thì không tái lập.** Không thể kết luận doc sai từ đây: bản đo
  cũ có thể ở build debug, trên máy khác, hoặc gồm cả công việc khác trong cùng tick. Đây là một
  **finding cần đối chứng**, không phải một lỗi đã xác định — theo quy tắc 6 của
  [chính sách tài liệu](../governance/DOCUMENTATION_POLICY.md), mở finding chứ không tự coi bên nào
  đúng.

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
