---
title: Kế hoạch tách Anima-Engine thành các crate con kiểm chứng độc lập
status: proposed
owner: maintainers
last_reviewed: 2026-10-31
review_cycle: weekly
---

# Tách thành crate con — kế hoạch

> **Mục tiêu:** mỗi phần của backend test được, lint được, build tăng dần được **một mình**, trong
> khi vẫn compile vào **cùng một binary** như hôm nay. Đây là tách theo **compile unit trong một
> workspace**, không phải tách repo: không semver, không tiến trình riêng, nên các invariant toàn cục
> (closed-EU bit-exact, tất định, zero-alloc tick, save/load trajectory-exact) vẫn khẳng định được
> trong **một** test trên **một** thế giới đang chạy. Mất điều đó là mất lý do dự án tồn tại.
>
> Kế hoạch này **không remake**. Lý do ở [§0](#0-vì-sao-không-viết-lại).
> Thứ tự việc chung của dự án: [`EXECUTION_PLAN_2026-10-31.md`](EXECUTION_PLAN_2026-10-31.md).

---

## 0. Vì sao không viết lại

| Đo được 2026-10-31 | Hệ quả |
|---|---|
| `cargo test --features desktop` → **1153 passed · 0 failed**, 107 target | Engine không phải phần hỏng |
| Hai gate tái lập xanh trên nhánh này | Phần khó nhất đang đúng |
| `main` đỏ CI 10 ngày · 1 nhánh treo 2 tháng · 1 khối WIP hạ cánh nguyên khối | Phần hỏng là **kỷ luật hợp nhất** |

Viết lại không sửa được cột thứ ba — cùng quy trình đó cho cùng kết quả trên codebase mới, muộn hơn
vài tháng và không còn bộ gate. Bộ gate là tài sản khó dựng lại nhất vì nó bắt các chế độ hỏng **im
lặng**: target compile thành binary rỗng rồi exit 0, feature rò qua `cargo tree`, `thread_rng()` quay
lại qua quét mã nguồn, binding lệch qua `git diff`, cấp phát trong tick path.

Và thứ *trông như* cần viết lại thì chỉ cần chuyển chỗ — xem [§1](#1-nguyên-nhân-thật-type-nằm-cạnh-system).

**Hai chỗ đáng làm lại thật**, và cả hai nhỏ, nằm ở tầng UI chứ không ở engine: Research Dashboard +
`telemetry_export` trên `main` (giá trị bịa, hai lệnh IPC chết, fallback mock — dạng "trông như chạy"
thì vá tệ hơn làm lại), và Map 2 (bằng chứng bịa, world artifact là bản sao của Map 1). Chúng là
task **B6** và **QĐ-2**, không thuộc kế hoạch này.

---

## 1. Nguyên nhân thật: type nằm cạnh system

F-005 ghi **ba** cặp phụ thuộc vòng (`ai ↔ core`, `ai ↔ physics`, `core ↔ physics`). Đo lại
2026-10-31 bằng cách đếm module đích của từng `use crate::<mod>::`: **cả sáu** cặp trong
{`ai`, `core`, `physics`, `evolution`} đều trỏ hai chiều. F-005 nói thiếu vì nó không tính
`evolution`.

| Cạnh | Số module đích | Cạnh ngược | Số module đích |
|---|---|---|---|
| `core → evolution` | 8 | `evolution → core` | 7 |
| `ai → core` | 5 | `core → ai` | 4 |
| `evolution → ai` | 5 *(ký hiệu)* | `ai → evolution` | 1 *(ký hiệu)* |
| `physics → core` | 2 | `core → physics` | 1 |
| `physics → ai` | 2 | — | — |
| `evolution → physics` | 1 | `physics → evolution` | 1 |

**Đọc nội dung từng cạnh mỏng thì ra một mẫu duy nhất — gần như mọi cạnh là một *type component*, không phải logic:**

```
physics → core::ecs::CognitiveState, core::ecs::InertiaComponent      (type)
physics → ai::hrrl::HomeostaticState, ai::hrrl::LastTransitionState   (type)
core    → physics::dynamics::RigidBody                                (type)
evolution → ai::hrrl::HomeostaticState, ai::cpg::CpgOscillator,
            ai::model::BrainModel                                     (type)
evolution → core::components, core::ecs, core::determinism            (type + SimRng)
```

Và chỗ các type đó đang ở, đo được:

| File | Dòng | `pub fn` | `pub struct`/`enum` |
|---|---|---|---|
| `core/components.rs` | 670 | **0** | **33** |
| `core/ecs.rs` | 117 | 1 | 0 |
| `core/determinism.rs` | 368 | 5 | 2 |
| `ai/hrrl.rs` | 206 | **0** | 6 |
| `ai/cpg.rs` | 60 | 1 | 2 |
| `physics/dynamics.rs` | 174 | 2 | 2 |

`core/components.rs` là **670 dòng toàn type, không một hàm nào**. Nó đã là một tầng type thuần, chỉ
đang nằm trong crate chứa system. Vòng phụ thuộc không phải lỗi thiết kế sâu — nó là **một tầng bị
thiếu**.

**Hai cạnh KHÔNG phải type, và chúng phải được xử lý khác nhau:**

- `physics → core::agent_systems::apply_inertia_to_oscillators` — một **system function**. Đây là
  cạnh sai thật: tầng physics gọi vào tầng system. Phải chuyển hoặc đảo chiều (task S2.2).
- `ai/model.rs → evolution::brain_genotype::{learn_step, LearnScratch, action_index, BrainGenotype}`
  — **cố ý, giữ nguyên**. `CLAUDE.md` nói có **một** objective A2C và `brain_genotype::learn_step`
  là bản chuẩn mà `ai/model.rs` phải khớp, có gate parity EB-S02 giữ. Nên `anima-ai` **phụ thuộc**
  `anima-evolution`; đừng "phá vòng" chỗ này.

`core/engine.rs` là 4 dòng `pub use super::*` — shim chết, xoá (task S0.3).

---

## 2. Cơ chế xác minh — đã có sẵn, chưa ai bật

Đây là lý do kế hoạch này kiểm chứng được chứ không phải tin nhau:

| Có sẵn | Trạng thái |
|---|---|
| `cargo-modules` **0.27.0** | ✅ đã cài |
| `just dataflow-truth` → `cargo modules dependencies --lib --no-externs` | ✅ có recipe |
| `just dataflow-verify [depth]` → `scripts/verify_dataflow.py` | ✅ có recipe, so ở mức module (depth 1) hoặc submodule (depth 2) |
| `dataflow.json` — **hợp đồng phụ thuộc đã khai** | ❌ **file không tồn tại** |
| `dataflow` trong `.github/workflows/ci.yml` | ❌ **0 lần xuất hiện** |

Toàn bộ máy móc để biến "không có vòng" thành một **gate** đã nằm trong repo; chỉ thiếu đúng file
khai báo và một bước CI. F-005 tự ghi điều này: *"Test khoá lại: chưa có — cần khai contract phụ
thuộc rồi cho CI chặn."*

Đồ thị thô 788 cạnh, lọc "owns" rồi gộp về mức module còn **11** cạnh `uses` — đủ nhỏ để khai bằng
tay và đọc được trong một PR.

### 2.1 Bộ ba kiểm cho **mỗi** bước tách

Một bước chỉ được gọi là xong khi cả ba xanh. Mỗi cái chặn một chế độ hỏng đã xảy ra thật:

| # | Kiểm | Chặn điều gì |
|---|---|---|
| **V1** | `just dataflow-truth && just dataflow-verify 1` exit 0 | Đồ thị thật lệch khỏi hợp đồng đã khai — tức một vòng mới, hoặc một cạnh không được phép |
| **V2** | Tổng số test **TĂNG** đúng bằng số test chuyển đi, và `node scripts/check_test_targets.mjs <capture> --profile desktop` exit 0 | Test rơi khỏi gate **im lặng**. `src-tauri/Cargo.toml` ghi lại vết thương này: lúc tạo `anima-domain`, **20 test của nó rơi khỏi mọi gate** cho tới khi thêm `default-members`, vì crate vẫn compile như dependency nên không gì trông sai. "Giữ nguyên số" là **đỏ**, không phải xanh |
| **V3** | Checksum tất định **trùng bit** trước/sau | Chuyển mã làm đổi thứ tự draw RNG ⇒ đổi **mọi** quỹ đạo. Tách là refactor thuần: một bit lệch nghĩa là không thuần |

> ⚠️ **V2 không kiểm được khi `scripts/check_test_targets.mjs` còn chết.** File đó đã commit kèm ba
> khối conflict marker (F-017) nên nó chết ngay khi parse. Tách crate trong lúc gate đếm target đang
> chết là **đúng tổ hợp** làm mất test mà không ai biết. Vì vậy **task A1 của
> [`EXECUTION_PLAN_2026-10-31.md`](EXECUTION_PLAN_2026-10-31.md) là tiền đề cứng của toàn bộ kế
> hoạch này**, không phải một việc dọn dẹp song song.

`default-members = [".", "crates/*"]` đã là glob, nên một crate mới dưới `crates/` được tự động đưa
vào. Đó là điều kiện cần, **không đủ**: V2 vẫn phải đo, vì glob không nói gì về `[[test]]` target.

---

## 3. Kiến trúc đích

Mũi tên là "phụ thuộc". Không cạnh nào đi lên.

```
S5  anima-engine          ~12k   simulation_loop, simulation_schedule, agent_systems,
    (crate gốc)                  world_systems, resources, terrain, ecology, mortality,
                                 environmental_systems, live_experiment (adapter),
                                 tick_capture, aggregate_population, simulation_lod, commands/
                 │
S4  anima-lab             ~16k   experiment_runner 4497, experiment 2783, reference_world 1628,
                 │               evolution_pathway 1484, exotic_energy 1839, observer 895,
                 │               scenario 849
S3  anima-persistence     ~4k    snapshot 1687, simulation_state 1397, world_artifact 780,
                 │               sha256 173
                 │
S2  anima-ai  →  anima-evolution        anima-physics
    ~2.1k        ~4.3k                  ~0.6k
                 │
S1  anima-ecs-types       ~1.6k   components 670 (33 type, 0 fn), determinism 368 (SimRng),
    ◀── TẦNG BỊ THIẾU            hrrl 206 (6 type), ecs 117, dynamics (RigidBody), cpg 60
                 │
S0  anima-domain          (đã có) laws, sim_clock, intervention, causal — thuần, không ECS
```

**Vì sao S1 là then chốt:** nó phá **cả sáu** vòng cùng lúc, vì mọi cạnh vòng (trừ hai ngoại lệ ở
§1) là một type sẽ nằm trong S1. Sau S1, `physics`, `ai`, `evolution` chỉ còn phụ thuộc xuống.

**Vì sao `anima-lab` ở dưới `anima-engine`:** trait `ExperimentModel` và `experiment_runner` thuộc
lab; `core/live_experiment.rs` (`LiveExperimentAdapter: ExperimentModel`) là **bản hiện thực** nên ở
lại engine. Inversion này **đã được thiết kế sẵn** — cả `ReferenceEvolutionWorld` lẫn
`LiveExperimentAdapter` đã implement cùng một trait qua cùng một runner. Tách lab chỉ là dọn nó về
chỗ nó đã được thiết kế để ở.

**Phần phải ở lại S5:** nơi invariant toàn cục được khẳng định — lịch tick, census, ledger, các
system. Đó là glue, không phải thư viện. Đừng tách tiếp.

---

## 4. Các bước

Mỗi bước: **một PR**, bộ ba V1/V2/V3, và không kèm thay đổi hành vi nào.

### S0 — Dựng gate trước khi di chuyển gì

| ID | Việc | Xong khi |
|---|---|---|
| **S0.1** | **A1 trước** — hồi sinh `scripts/check_test_targets.mjs` (port từ `origin/main` `286e45a`). Không có nó, V2 không đo được | `node scripts/check_test_targets.mjs <capture> --profile desktop` exit 0 |
| **S0.2** | **Khai `dataflow.json`** — hợp đồng phụ thuộc *hiện tại* (gồm cả sáu vòng, đúng như thực tế), rồi `just dataflow-verify 1` xanh. Khai **hiện trạng** trước, đừng khai mong muốn: một gate xanh ngay là gate đo được, một gate đỏ sẵn thì vô dụng | `just dataflow-truth && just dataflow-verify 1` exit 0; thêm bước này vào `ci.yml`; sửa một `use` để tạo cạnh lạ ⇒ gate **đỏ** và nêu tên cạnh |
| **S0.3** | Xoá `core/engine.rs` (4 dòng shim), chuyển các call-site sang module thật | `cargo test --features desktop` 1153 passed; V1 cho thấy **mất** cạnh qua `engine` |
| **S0.4** | Đóng **F-005**: cập nhật finding với số đo 6 cặp, trỏ vào gate mới | F-005 có "Test khoá lại" thật |

### S1 — `anima-ecs-types` (bước quan trọng nhất)

| ID | Việc | Xong khi |
|---|---|---|
| **S1.1** | Tạo crate `crates/anima-ecs-types`, phụ thuộc `bevy_ecs` + `anima-domain` + `glam`, **không** phụ thuộc gì khác trong repo | `cargo tree -p anima-ecs-types` không chứa `anima-engine` |
| **S1.2** | Chuyển `core/components.rs` (670, 0 fn) và `core/ecs.rs` (117) vào crate, `pub use` lại từ `core` để call-site không đổi | V1/V2/V3 |
| **S1.3** | Chuyển `core/determinism.rs` (`SimRng`, 368). **Rủi ro V3 cao nhất ở đây** — đo checksum trước khi chạm | V3 trùng bit, và test quét mã chặn `thread_rng()` vẫn chạy |
| **S1.4** | Chuyển type thuần của `ai/hrrl.rs` (6 struct, 0 fn) và `ai/cpg.rs`; giữ system/logic ở `ai` | V1 cho thấy `physics → ai` và `evolution → ai` **mất** cạnh type |
| **S1.5** | Chuyển `RigidBody` khỏi `physics/dynamics.rs` | V1 cho thấy `core → physics` **biến mất** |
| **S1.6** | **Cập nhật `dataflow.json` sang đồ thị KHÔNG VÒNG** và chứng minh | `just dataflow-verify 1` xanh trên hợp đồng mới; `cargo modules` không báo vòng nào trong {ai, core, physics, evolution} |

> Sau S1, `cargo test -p anima-ecs-types` chạy một mình trong vài giây. Đó là lần đầu một phần của
> engine kiểm chứng được độc lập.

### S2 — Ba crate lá

| ID | Việc | Xong khi |
|---|---|---|
| **S2.1** | `anima-physics` (588 dòng — nhỏ nhất, làm trước để học quy trình) | `cargo test -p anima-physics` xanh một mình; V1/V2/V3 |
| **S2.2** | Cạnh sai thật: `physics → core::agent_systems::apply_inertia_to_oscillators`. Chuyển hàm vào `anima-physics`, **hoặc** đảo để `core` gọi physics. Quyết định và ghi lý do | V1 không còn cạnh `physics → core`; V3 trùng bit |
| **S2.3** | `anima-evolution` (4.3k): genotype, brain_genotype, mutation, crossover, fitness, map_elites, lineage, newick, simplify | `cargo test -p anima-evolution` xanh một mình |
| **S2.4** | `anima-ai` (2.1k): model (Burn), hrrl/cpg system. **Giữ** `anima-ai → anima-evolution` (một objective A2C, gate EB-S02) | gate parity EB-S02 vẫn xanh; `cargo tree --no-default-features` vẫn **không** chứa `burn-wgpu`/`naga` |

### S3 — `anima-persistence`

| ID | Việc | Xong khi |
|---|---|---|
| **S3.1** | Chuyển `snapshot` 1687, `simulation_state` 1397, `world_artifact` 780, `sha256` 173 | V1/V2/V3 |
| **S3.2** | Giữ gate schema/migration chạy trong crate mới (7 file test feature-gated có `required-features` — **đừng** để rơi) | `check_test_targets.mjs --profile desktop` báo **7/7** target feature-gated vẫn chạy |

### S4 — `anima-lab` (cụm lớn nhất)

| ID | Việc | Xong khi |
|---|---|---|
| **S4.1** | Chuyển trait `ExperimentModel` + `experiment_runner` 4497 + `experiment` 2783 | `anima-engine` chỉ còn **implement** trait, không định nghĩa |
| **S4.2** | Chuyển `reference_world` 1628, `scenario` 849, `evolution_pathway` 1484, `exotic_energy` 1839, `observer` 895 | `cargo test -p anima-lab` xanh một mình |
| **S4.3** | `core/live_experiment.rs` **ở lại** `anima-engine` (nó là adapter). Xác nhận hai gate tái lập vẫn xanh | `live_experiment_tests` **17 passed** |
| **S4.4** | `anima-lab` **không** được phụ thuộc `anima-engine` | `cargo tree -p anima-lab` không chứa `anima-engine`; V1 |

### S5 — Thu hoạch

| ID | Việc | Xong khi |
|---|---|---|
| **S5.1** | Đưa `dataflow-verify`, và `cargo test -p <crate>` cho từng crate, thành bước CI riêng | CI báo crate nào đỏ, không chỉ "backend đỏ" |
| **S5.2** | Đo lại và ghi vào `STATE_OF_THE_PROJECT.md` **§1.g**: số test **từng crate**, thời gian build tăng dần trước/sau | §1.g có bảng per-crate |

---

## 5. Rủi ro, và cái nào thật

| Rủi ro | Thật? | Giảm bằng |
|---|---|---|
| Test rơi khỏi gate im lặng khi tạo crate | **Đã xảy ra một lần** (20 test của `anima-domain`) | V2, và S0.1 trước mọi thứ |
| Thứ tự draw RNG đổi ⇒ mọi quỹ đạo đổi | **Cao**, nhất là S1.3 (`SimRng`) | V3 từng bước; một bước = một PR nên bisect được |
| Vòng mới lọt vào sau khi phá xong | Trung bình | V1 trong CI (S0.2) — không có nó thì vòng quay lại trong một PR |
| Mất invariant toàn cục | **Không** — cùng workspace, cùng binary, test vẫn chạy trên cả thế giới | — |
| 7 target test feature-gated rơi khi chuyển crate | Trung bình | S3.2 kiểm đúng con số 7/7 |
| Bỏ dở giữa đường, cây mã nửa tách | Trung bình | Mỗi bước tự đứng được: `pub use` lại giữ call-site không đổi, nên dừng ở bất kỳ S nào vẫn là cây xanh |

**Không làm song song với HT-C (hợp nhất).** Tách crate đổi chỗ của gần như mọi file; merge một nhánh
68 commit vào giữa lúc đó là tự tạo xung đột trên toàn bộ cây. Thứ tự: **A1 → HT-C xong → kế hoạch
này**, hoặc **A1 → kế hoạch này xong → HT-C**, chọn một, không trộn.

---

## 6. Vì sao kế hoạch này kiểm chứng được

Ba câu, mỗi câu là một con số đo được hôm nay chứ không phải một lời hứa:

1. **Đồ thị phụ thuộc có gate**: 11 cạnh mức module, khai trong `dataflow.json`, CI chặn. Một vòng
   mới là CI đỏ kèm tên cạnh.
2. **Số test có gate**: `check_test_targets.mjs` cưỡng chế allow-list hai chiều, nên một target rỗng
   hay một test bị ignore không lọt. V2 đòi số test **tăng**, không phải "không giảm".
3. **Quỹ đạo có gate**: checksum tất định trùng bit. Tách là refactor thuần; một bit lệch là bằng
   chứng nó không thuần, ngay tại PR gây ra.
