---
title: Kế hoạch áp dụng nguồn mở
status: active
owner: architecture
last_reviewed: 2026-07-26
review_cycle: quarterly
target: M0-M4
---

# Kế hoạch áp dụng nguồn mở

## Trạng thái thật (cập nhật 2026-07-26, sau khi OSS-010 ship)

Kế hoạch này được lập 2026-07-24. Bảng dưới là đối chiếu giữa kế hoạch và code, để phiên sau không
đọc một `[x]` không tồn tại thành việc đã xong. Bằng chứng chi tiết nằm ở
[§ Điều đã thay đổi](../research/OPEN_SOURCE_LANDSCAPE.md#điều-đã-thay-đổi-kể-từ-2026-07-24)
trong khảo sát.

> **Bản đầu của mục này (2026-07-26, sáng) mở đầu bằng *"chưa có mục nào được thực thi"*.** Câu đó
> đúng khi viết và **sai vài giờ sau**: OSS-010 đã ship trong cùng ngày. Giữ lại ghi chú này vì
> chính nó là bài học — một tài liệu trạng thái viết bằng câu tuyệt đối sẽ hết hạn nhanh hơn tài
> liệu viết theo từng mục có ngày.

| Mục | Kế hoạch nói | Thực tế | Xử lý |
|---|---|---|---|
| OSS-003 license | Blocker quản trị duy nhất | `LICENSE` **đã có**, proprietary | **Gỡ blocker.** Còn nợ: tách phạm vi code/asset/data + `NOTICE` |
| OSS-004 inventory | Tạo inventory ban đầu | Chưa có; và `burn`/`burn-wgpu` bị **bỏ sót** khỏi khảo sát | Đã thêm dòng vào khảo sát; inventory tự động vẫn nợ |
| OSS-010 Criterion | Adopt | ✅ **XONG 2026-07-26** — `dev-dependency` + `src-tauri/benches/tick_systems.rs`, 16 số đo thật | Xem [§ OSS-010 đã ship](#oss-010-đã-ship-2026-07-26) |
| OSS-011 tracing | Adopt | ⬜ Không có trong `Cargo.toml` | Giữ, nay là mục OS1 đáng làm tiếp |
| OSS-012 cargo-deny | Adopt sau khi chọn license | ⬜ Advisory **đã được phủ** bởi `cargo audit` + `npm audit` | Thu hẹp phạm vi còn licenses/bans/sources |
| OSS-013 lychee | Adopt | ✅ `scripts/check_docs_links.mjs` **đã là gate CI** | **Đóng — superseded** |
| OS2 oracle | 2–3 tuần | ⬜ Chưa bắt đầu; OSS-021 bị chặn bởi OSS-020 | Giữ thứ tự, ghi rõ blocker |
| OS3 BVH/meshopt | Pilot | Tiền đề bị code bác bỏ | **Park** — xem OS3 |
| OS4 Rapier | Pilot có feature flag | Đường physics/CPG sống chưa tất định | Thêm tiền điều kiện cứng |
| OS5 Arrow | Khi ≥100 MB/run | Trigger **chưa đạt** | Giữ Defer |
| OS7 phả hệ | *(mới)* | ⬜ Chưa bắt đầu | OSS-070 (Newick) là món rẻ nhất còn lại |

### OSS-010 đã ship (2026-07-26)

Criterion vào làm `dev-dependency` với `default-features = false`, cộng
[`src-tauri/benches/tick_systems.rs`](../../src-tauri/benches/tick_systems.rs). Bảng số và cách diễn
giải: [`docs/how-to/BENCHMARKING.md`](../how-to/BENCHMARKING.md).

Ba điều đáng ghi lại, vì chúng đổi cách đọc các mục khác trong kế hoạch này:

1. **Phần cứng mục tiêu đã đổi.** Nay là Intel Core i5-14600KF; khai báo Dell Vostro 3530 cũ **không
   còn hiệu lực**. Máy capture và máy mục tiêu là một, nên số đo tính là đo-trên-máy-đích.
2. **§3.2 vẫn chưa đóng, nhưng không còn vì phần cứng.** Cái đo được là **cận dưới của một tick**,
   không phải khung hình: chưa gồm suy luận não, lập lịch ECS, emit, va chạm, trao đổi chất. Ba hàng
   đắt nhất trong `BENCHMARK_BASELINE.md` vẫn đòi in-app tick capture.
3. **Một finding mới, chưa giải quyết.** Con số ~4,2 ms trong doc comment của
   `ResourceField::REGROWTH_STRIDE` **không tái lập** — release build cho ~0,36 ms, thấp hơn ~12
   lần. Việc stride vẫn đúng (đo được 3,97×); chỉ con số biện minh cho nó là chưa đối chứng được.

**Mục OS1 đáng làm tiếp là OSS-011 (`tracing`)**, nhưng nó **không** còn nằm trên đường tới hạn P0
như OSS-010 từng nằm. Ưu tiên cao hơn hiện là OSS-070 (xuất Newick) — xem OS7.

## Mục tiêu

Tận dụng nguồn mở để tăng độ tin cậy khoa học, hiệu năng và khả năng bảo trì mà vẫn
giữ bốn thuộc tính: một nguồn sự thật, tái lập theo seed, build desktop gọn và có thể
gỡ bỏ từng tích hợp. Kế hoạch này bổ trợ
[`WORLD_SIMULATION_PLAN.md`](../../WORLD_SIMULATION_PLAN.md), không thay thế nó.

## Nguyên tắc quyết định

Mỗi tích hợp đi qua chuỗi:

```mermaid
flowchart LR
    R["Research + license"] --> A["ADR + ranh giới"]
    A --> F["Fixture và baseline"]
    F --> P["Pilot có feature flag"]
    P --> G{"Qua quality gate?"}
    G -- "Có" --> D["Adopt + lock version"]
    G -- "Không" --> X["Gỡ pilot + lưu kết quả"]
    D --> M["Theo dõi update/CVE/license"]
```

Một pilot không được coi là “adopted” cho đến khi có owner, lock version, NOTICE cần
thiết, test, benchmark, migration/rollback và cập nhật tài liệu nguồn sự thật.

## Trình tự tích hợp

### OS0 — Quản trị tài liệu và giấy phép

**Thời lượng:** 1–2 ngày. **Phụ thuộc:** không. **Trạng thái:** đang khởi tạo.

| ID | Công việc | Bằng chứng hoàn tất |
|---|---|---|
| OSS-001 | Dùng `README.md` và `docs/README.md` làm hai điểm vào duy nhất | Mọi tài liệu chuẩn có đường đi ≤ 2 lần nhấp |
| OSS-002 | Ban hành quy tắc Diátaxis, metadata, ADR và deprecation | Link nội bộ hợp lệ; không có hai nguồn sự thật |
| OSS-003 | Người duy trì chọn license cho chính Anima Engine | ✅ **Xong một phần (2026-07-26).** `LICENSE` proprietary đã có. **Còn nợ:** phạm vi riêng cho code/model/dataset/asset, và `NOTICE` cho thành phần permissive được phân phối |
| OSS-004 | Tạo inventory dependency ban đầu | ⬜ Chưa. Đã lộ một lỗ hổng thật: `burn`/`burn-wgpu` là runtime dep nhưng vắng khỏi khảo sát cho tới 2026-07-26 |

**Gate:** license của mọi thành phần bên thứ ba phải được xác minh **theo đúng tag/commit** trước
khi thêm. Với `LICENSE` proprietary, thành phần copyleft (GPL/AGPL) là **chặn cứng cho mọi đường
tiếp xúc với code**, không còn là "cần review thêm" — xem
[chính sách nguồn mở](../governance/OPEN_SOURCE_POLICY.md).

### OS1 — Tooling ít rủi ro

**Thời lượng:** 3–5 ngày. **Phụ thuộc:** OS0. **Trạng thái: 2/4 xong (2026-07-26).**

| ID | Công việc | Kiểm tra / tiêu chí | Trạng thái |
|---|---|---|---|
| OSS-010 | Thêm Criterion cho tick, spatial query và artifact encode/decode | Benchmark chạy headless, lưu machine metadata và baseline | ✅ **XONG 2026-07-26** |
| OSS-011 | Khai báo trực tiếp `tracing` và quy ước correlation ID | Không log mỗi entity/tick mặc định; overhead tắt < 2% | ⬜ Mục OS1 còn lại đáng làm nhất |
| OSS-012 | Thêm `cargo-deny` cho **licenses/bans/sources** | CI chặn license/source ngoài policy | ⬜ Phạm vi đã hẹp lại: advisory nay do `cargo audit` + `npm audit` phủ |
| OSS-013 | ~~Thêm lychee cho Markdown~~ | — | ✅ **Superseded** bởi [`scripts/check_docs_links.mjs`](../../scripts/check_docs_links.mjs) |

**OSS-010 — bằng chứng hoàn tất, đã kiểm.** Bench phủ `integrate_physics_system`,
`rebuild_spatial_grid_system`, `ResourceField::step_regrowth{,_gated,_gated_strided}`,
`DynamicFields::step_{water,soil,erosion}`, `a2c_loss` và `WorldArtifact::{to,from}_bytes`/`checksum`
— tất cả gọi được ngoài Tauri, không cần GPU, không boot app. Machine metadata và 16 số đo thật nằm
trong [`benchmark_report.json`](../../benchmark_report.json) dưới tiền tố `criterion/`;
`scripts/bench_baseline.mjs` đọc thẳng từ output của Criterion.

**Gate đã xác minh:** `cargo tree --no-default-features -e normal` cho **0 kết quả** cho `criterion`
— nó là `dev-dependency` nên vô hình với đồ thị mà gate tách feature (G2 #2) thật sự kiểm. `fmt`,
`clippy --all-targets` ở cả hai cấu hình feature, và `cargo test --features desktop` (673 pass /
0 fail) đều xanh. Cách gỡ ghi ở [`BENCHMARKING.md`](../how-to/BENCHMARKING.md) §"Tắt / gỡ".

**Một guard được thêm cùng lúc, vì nó chặn đúng loại hỏng im lặng repo này quan tâm:** `target/` bị
gitignore, nên chạy `bench_baseline.mjs` trên một clone mới sẽ thay report đầy số thật bằng proxy —
và kết quả vẫn validate, vẫn trông như baseline. Script nay từ chối và exit ≠ 0, trừ khi
`ANIMA_BENCH_ALLOW_PROXY_ONLY=1`.

### OS2 — Bộ oracle khoa học ngoại tuyến

**Thời lượng:** 2–3 tuần. **Phụ thuộc:** M0, M1 và `WorldArtifact` ổn định.

| ID | Công việc | Phụ thuộc | Bằng chứng |
|---|---|---|---|
| OSS-020 | Định nghĩa `scientific-fixture` manifest: source, version, license, units, seed, checksum | OSS-003 | JSON Schema + fixture mẫu |
| OSS-021 | Adapter Landlab cho lưới lưu vực nhỏ | OSS-020 | Flow direction/accumulation và water balance golden fixture |
| OSS-022 | Adapter pyrealm cho grass productivity | OSS-020 | Xu hướng GPP theo ánh sáng/nhiệt/ẩm nằm trong tolerance |
| OSS-023 | Adapter Virtual Ecosystem cho scenario tích hợp nhỏ | OSS-021, OSS-022 | So sánh miền hợp lệ cho nước–đất–producer |
| OSS-024 | Runner SALib đọc batch output của Anima | M2 scenario runner | Báo cáo Sobol/Morris ổn định theo seed |
| OSS-025 | Gắn provenance vào fixture đã rút gọn | OSS-021–024 | Có script tái tạo; không commit cache môi trường Python |

**Gate:** oracle chỉ chạy trong research/CI tùy chọn; runtime Tauri không phụ thuộc
Python. So sánh invariant, thứ tự xu hướng và tolerance có lý do khoa học, không ép
hai mô hình khác nhau cho ra giá trị tuyệt đối giống nhau.

> **Trạng thái 2026-07-26.** Chưa bắt đầu. **OSS-021 (Landlab) bị chặn bởi OSS-020** — định dạng
> `scientific-fixture` chưa tồn tại — cộng một chi phí vận hành chưa trả: cần môi trường Python trên
> một máy dev đang có ràng buộc tài nguyên thật. Trạng thái đúng là *"đã duyệt, chưa làm, đang bị
> chặn"*, không phải *"đã loại"*.
>
> Khoảng trống mà OSS-021 nhắm vào là **có thật và đã định vị được**: `downstream[i]` trong
> [`dynamic_fields.rs`](../../src-tauri/src/core/dynamic_fields.rs) là steepest-descent đơn hướng,
> không có flow accumulation; và `step_erosion` là công thức cục bộ **không vận chuyển trầm tích**.
> Lý do đầy đủ vì sao đây là oracle chứ không phải dependency nằm ở
> [khảo sát §3.1](../research/OPEN_SOURCE_LANDSCAPE.md).

### OS3 — Pilot truy vấn không gian và LOD — **PARK (2026-07-26)**

**Thời lượng:** 1–2 tuần. **Phụ thuộc:** baseline M1.

> **Park, không phải huỷ.** Cả hai pilot của giai đoạn này đứng trên một tiền đề mà code hiện tại
> bác bỏ: không có `THREE.Raycaster` nào trong `src/`, cao độ địa hình lấy giải tích qua
> `sampleElevation`, và LOD theo khoảng cách **đã có** ở
> [`chunkLod.ts`](../../src/components/Landscape/utils/chunkLod.ts). Bằng chứng ở
> [F1](../research/OPEN_SOURCE_LANDSCAPE.md) và [F3](../research/OPEN_SOURCE_LANDSCAPE.md).
>
> **Trigger mở lại:** OSS-030 (đo đường hiện tại) vẫn đáng làm và nay thuộc về OSS-010 — Criterion
> là công cụ đo. OSS-031/OSS-032 chỉ mở lại khi phép đo đó cho thấy raycast/ngân sách tam giác thật
> sự là nút thắt. Lịch không phải trigger.

| ID | Công việc | Kiểm tra / tiêu chí |
|---|---|---|
| OSS-030 | Benchmark raycast/picking hiện tại trên 3 mật độ terrain | CPU time, allocation, memory |
| OSS-031 | Prototype `three-mesh-bvh` sau adapter nội bộ | Feature flag; cùng hit point/normal trong tolerance |
| OSS-032 | Prototype `meshoptimizer` cho một chunk chuẩn | Kích thước, decode time, silhouette/normal/UV |
| OSS-033 | ADR adopt/reject từng pilot | Lợi ích ≥ 20% ở workload mục tiêu hoặc lý do khác được định lượng |

**Rollback:** adapter không để kiểu dữ liệu của thư viện lan ra component sản phẩm;
tắt flag quay về đường cũ và vẫn đọc artifact hiện có.

### OS4 — Pilot vật lý Rapier

**Thời lượng:** 2–4 tuần. **Phụ thuộc:** M5 animal motion và benchmark ổn định, **cộng tiền điều
kiện cứng dưới đây (thêm 2026-07-26)**.

> **Không mở pilot trước khi đường physics/CPG sống đã tất định.**
> [`DETERMINISM_CONTRACT.md`](../reference/DETERMINISM_CONTRACT.md) §5 ghi rằng physics/CPG chạy
> song song nên một run liền mạch còn **không khớp chính nó** — đó là thứ đang chặn gate
> `an_inhabited_run_replays_from_its_trace_without_a_human` của
> [ADR-0004](../decisions/ADR-0004-observer-as-declared-intervention.md).
>
> Lý do là logic, không phải thận trọng: OSS-040 đòi "chốt fixture và kết quả solver hiện tại" làm
> đường cơ sở. Một đường cơ sở không lặp lại được thì phép so sánh side-by-side ở OSS-042 không có
> nghĩa — bất kỳ sai khác nào cũng quy được cho nhiễu.
>
> Thêm nữa, solver hiện tại không phải rigid-body tổng quát: `resolve_joints_system`
> ([`physics/dynamics.rs`](../../src-tauri/src/physics/dynamics.rs)) là điều khiển khớp lái bởi
> `CpgOscillator`. Rapier không thay được phần đó mà không thiết kế lại tầng vận động — xem
> [F2](../research/OPEN_SOURCE_LANDSCAPE.md).

| ID | Công việc | Kiểm tra / tiêu chí |
|---|---|---|
| OSS-040 | Chốt fixture 100/1.000 tác nhân và collision cases | Kết quả solver hiện tại, seed, tick budget |
| OSS-041 | Viết `PhysicsBackend` nhỏ, giữ backend hiện tại | Không để handle Rapier thành component lưu trữ công khai |
| OSS-042 | Chạy side-by-side Rapier bằng feature flag | Collision correctness, determinism, CPU, memory, save/load |
| OSS-043 | ADR adopt/partial/reject | Không critical regression; lợi ích đủ bù chi phí build/nâng cấp |

Rapier không mặc nhiên sở hữu hunger, energy, damage hay nguyên nhân tử vong; các luật
đó vẫn ở simulation domain.

### OS5 — Dữ liệu thí nghiệm

**Thời lượng:** 1–2 tuần khi cần. **Phụ thuộc:** M2/M8.

| ID | Công việc | Trigger |
|---|---|---|
| OSS-050 | Đo JSON/CSV batch export | ≥ 100 MB mỗi run hoặc phân tích I/O thành nút thắt đo được |
| OSS-051 | Pilot Arrow/Parquet ở output adapter | Chỉ khi OSS-050 kích hoạt |
| OSS-052 | So sánh schema evolution/tooling/size/time | ADR adopt/reject |

Arrow/Parquet không thay `WorldArtifact` hoặc save-game. Nếu chưa vượt trigger, giữ
JSON/CSV để debug và trao đổi dễ hơn.

### OS6 — Mẫu kiến trúc cho quy mô lớn

**Thời lượng:** 3–5 tuần, gắn M9. **Phụ thuộc:** profiler xác nhận nút thắt.

- Học cohort/energy-budget từ Madingley.
- Học modular experiment từ MABE2.
- Học observation/action/task/replay từ Neural MMO.
- Học scheduler/module benchmark từ BioDynaMo.

Đầu ra là ADR, prototype nội bộ và benchmark; không import engine thứ hai.

### OS7 — Phả hệ và bằng chứng tiến hoá (mới, 2026-07-26)

**Thời lượng:** 1–2 tuần. **Phụ thuộc:** không có blocker ngoài — toàn bộ là code nội bộ.

Giai đoạn này không thêm **một** dependency nào; nó lấy *thuật toán* và *định dạng* thay vì thư
viện. Lý do nằm ở [khảo sát §5](../research/OPEN_SOURCE_LANDSCAPE.md).

| ID | Công việc | Phụ thuộc | Bằng chứng hoàn tất | Trạng thái |
|---|---|---|---|---|
| OSS-070 | Xuất Newick từ đồ thị lineage sẵn có | không | Một parser bên thứ ba (`ape`/DendroPy) đọc được output; test từ chối cây có chu trình, node mồ côi hoặc nhiều gốc | ✅ **XONG 2026-07-26** — DendroPy 5.0.10 đọc được; 15 + 8 test |
| OSS-071 | `simplify()` kiểu tskit: prune nhánh không còn hậu duệ sống | OSS-070 | Bộ nhớ lineage trở thành O(cá thể sống), không phải O(tổng từng sống); quan hệ tổ tiên của phần giữ lại **không đổi** | ✅ **XONG 2026-07-26** — 13 + 6 test; **chưa nối vào tracker sống** |
| OSS-072 | Truy vấn MRCA | OSS-071 | Test trên cây đã biết đáp án; tất định | ⬜ |
| OSS-073 | Giao thức đo "line of descent" kiểu Avida | OSS-072 | Bám được dòng dõi của genotype thống trị cuối run | ⬜ |

**Vì sao đáng làm sớm:** [`evolution/lineage.rs`](../../src-tauri/src/evolution/lineage.rs) lưu mỗi
lần sinh sản kèm **bản sao đầy đủ** `MorphologyGenotype` và **không bao giờ prune**. Đó là đường
tăng bộ nhớ không có trần với một run dài. OSS-070 là món rẻ nhất trong toàn kế hoạch
(0 dependency) và lợi ích lớn nhất của nó không phải là interop mà là **kiểm tra độc lập tính đúng
của phả hệ**.

#### OSS-070 — đã ship phần serializer (2026-07-26)

[`src-tauri/src/evolution/newick.rs`](../../src-tauri/src/evolution/newick.rs) +
[`tests/newick_export_tests.rs`](../../src-tauri/tests/newick_export_tests.rs) (14 test) và 8 unit
test. `to_newick` nhận đúng thứ `LineageTracker::get_lineage_graph` trả về, nên chạy được với cả
tracker in-memory lẫn bản restore từ Neo4j.

**Quyết định thiết kế phải nói rõ, vì nó là một đánh đổi chứ không phải chi tiết:** `Crossover` cho
một cá thể **hai** cha mẹ, nên lineage là **DAG**, còn Newick chỉ biểu diễn **cây**. Export giữ
**một** cha mẹ và **đếm** số cạnh không biểu diễn được, trả về ở `dropped_parent_edges`. Đếm thay vì
bỏ im lặng chính là toàn bộ vấn đề: một export lặng lẽ cắt một nửa phả hệ có crossover vẫn parse
được, vẫn vẽ ra được, và vẫn sai. Cha mẹ nào sống sót được chọn theo **id nhỏ nhất**, không theo thứ
tự cạnh — vì thứ tự khác nhau giữa tracker in-memory (push order) và Neo4j (query order).

**Một defect thật do chính test bắt được, ghi lại vì nó dễ tái phát:** kiểm tra generation ban đầu
chạy **trước** kiểm tra chu trình. Nhưng generation không thể tăng đơn điệu vòng quanh một chu
trình, nên **mọi chu trình đều kéo theo đảo generation** — và báo lỗi generation cho một đồ thị có
vòng lặp là báo triệu chứng thay vì nguyên nhân, dẫn người đọc tới một node có bản ghi hoàn toàn
bình thường. Thứ tự nay là: chu trình trước, generation sau.

**Chân "parser bên thứ ba" của DoD đã chạy được.** Gate là **hai nửa trên cùng một file**, và
không nửa nào đứng một mình có giá trị:

```bash
cargo test --test newick_export_tests   # export vẫn sinh ra đúng fixture
python scripts/verify_newick.py         # DendroPy đồng ý fixture là một cây hợp lệ
```

Fixture: [`src-tauri/tests/fixtures/newick/lineage_forest.nwk`](../../src-tauri/tests/fixtures/newick/lineage_forest.nwk).
Nửa Rust ghim output vào file; nửa Python bắt DendroPy đọc **cùng file đó**. Round-trip thuần Rust
chỉ chứng minh serializer nhất quán với chính nó; một parser đọc file cũ không chứng minh gì về code
hiện tại.

Lineage của fixture cố tình khó chịu: hai gốc, một crossover có cạnh không biểu diễn được, một nhãn
chứa dấu cách và một nhãn chứa dấu hai chấm. Một fixture dựng từ chuỗi gọn gàng chỉ chứng minh
export chạy đúng ở ca chưa ai nghi ngờ.

**Đã kiểm rằng gate này thật sự đỏ được** (một gate chưa từng đỏ thì chưa phải gate), bằng cách phá
fixture theo hai cách:

- **Lồng ngoặc ngược chiều** — vẫn là Newick *hợp lệ*, nên parser đọc trót lọt; nhưng khẳng định về
  topology bắt được, và đây là loại lỗi mà so chuỗi phía Rust không thấy.
- **Bỏ quote quanh nhãn có dấu hai chấm** — DendroPy từ chối phân tích. Đây đúng là điều quy tắc
  quote tồn tại để chặn: `child:two` không quote sẽ bị đọc thành branch length và **cắt cụt tên**
  chứ không báo lỗi, trừ khi phần còn lại không phải số. Kiểm chứng từ bên ngoài.

`dendropy` là **dev-only, không có gì trong sản phẩm phụ thuộc Python**. Thiếu nó thì script thoát
kèm hướng dẫn cài, và nửa Rust của gate vẫn chạy.

**Chưa nối vào IPC.** `to_newick` hiện là hàm thư viện, chưa có lệnh Tauri nào gọi. Đó là việc riêng
và cần cập nhật hợp đồng IPC ở `PROJECT.md`; không gộp vào đây.

#### OSS-071 — đã ship phần thuật toán (2026-07-26)

[`src-tauri/src/evolution/simplify.rs`](../../src-tauri/src/evolution/simplify.rs) +
[`tests/lineage_simplify_tests.rs`](../../src-tauri/tests/lineage_simplify_tests.rs) (13 test) và 6
unit test. Hai bước, và **bước hai mới là bước chặn được bộ nhớ**:

1. **Prune** — bỏ node không có hậu duệ nào trong tập sample (thường là quần thể sống).
2. **Nén đường đơn** — node không phải sample, đúng 1 cha và 1 con thì bị nối tắt.

**Bước 1 một mình không đủ**, và điều đó được biến thành phép đo chứ không phải lời khẳng định:
tổ tiên của quần thể sống vẫn kéo về tận genesis, nên prune chỉ bỏ nhánh tuyệt chủng và giữ nguyên
mọi thân cây. Test `pruning_without_compression_would_not_have_been_enough` đo cả hai và đòi chênh
> 3×. Trên cây nhị phân sâu 10 (2.047 node, 16 cá thể sống): còn **31 node** = 16 sample + 15 điểm
rẽ nhánh, đúng cận `2·samples`.

**Chỗ suýt sai âm thầm, và là lý do `SimplifiedEdge` không có `relation_type`:**
`get_mutations_count` trong `commands/evolution.rs` **đếm cạnh `Mutate` dọc đường tổ tiên** để ra con
số đột biến hiển thị trên UI. Gộp 5 cạnh `Mutate` thành 1 cạnh `Mutate` là **giữ đúng kiểu mà làm số
đếm thành 1 thay vì 5** — hữu hạn, hợp lý, sai gấp năm. Nên cạnh mang `events`/`mutations`/
`crossovers`, và là **kiểu riêng** chứ không mở rộng `LineageRelation` (đã persist vào save state và
Neo4j — không nhét khái niệm phân tích vào định dạng lưu trữ).

Hai chỗ **từ chối nén** vì cùng lý do đó: node crossover (2 cha — nối tắt phải chọn một và vứt cái
kia), và hình thoi (nén sẽ gộp hai đường tổ tiên khác nhau thành một cạnh rồi cộng số đếm như thể
chúng nối tiếp).

**Kiểm chứng bằng OSS-070, đúng như DoD nêu.** `newick.rs` được tách thêm `to_newick_from` nhận cặp
(cha, con), để một lineage đã simplify xuất được Newick **mà không ai phải bịa ra một
`relation_type`** — `to_newick` nay là wrapper mỏng nên hai đường không thể lệch. Test
`the_simplified_lineage_is_still_a_tree_a_newick_parser_would_accept` dùng nó để chốt kết quả vẫn
không chu trình, không cạnh mồ côi, không đảo generation.

**Chưa nối vào tracker sống.** `simplify` là hàm thuần trả về một *giá trị*; chưa gì thay thế bộ nhớ
của `InMemoryLineageTracker`, nên bộ nhớ thực tế **chưa giảm**. Bước đó cần một chính sách về *khi
nào* chạy và ai cung cấp danh sách cá thể sống, cộng tương tác với `load_state`/Neo4j — việc riêng,
không gộp vào đây.

**Thứ tự là bắt buộc, không phải gợi ý:** không có MRCA thì không có gì để xuất ra một cây có
nghĩa, nên OSS-073 không thể đi trước OSS-072.

**Ranh giới license:** Newick là *định dạng*, `ape`/`ggtree` không được nhập code — nên license của
chúng không liên quan. Avida là copyleft: chỉ tham khảo qua bài báo, không đọc source rồi viết lại.

## Ma trận ánh xạ với roadmap mô phỏng

| Roadmap | Tích hợp hỗ trợ | Không được thay thế |
|---|---|---|
| M0 Rules/units/determinism | Criterion, tracing, cargo-deny | `SIMULATION_RULES.md` |
| M1 Authoritative world | Criterion, three-mesh-bvh pilot | `WorldArtifact` và Rust authority |
| M2 Scenario/causality | SALib, tracing | Domain causal ledger |
| M3 Climate/water/soil | Landlab, Virtual Ecosystem | Rust runtime model |
| M4 Plants | pyrealm, Virtual Ecosystem | Producer components/systems |
| M5–M7 Animals/food web | Rapier pilot, Madingley reference | Energy/behavior/death rules |
| M8 Disturbance/experiments | SALib, Arrow trigger | Scenario schema |
| M9 Scale/LOD | meshoptimizer, ABM references | Một ECS authority |
| Phả hệ / bằng chứng tiến hoá (OS7) | Newick (định dạng), thuật toán `simplify` kiểu tskit, giao thức đo kiểu Avida | `LineageTracker` là nguồn sự thật; không nhập code copyleft |

## Tiêu chí bắt buộc cho mỗi PR tích hợp

- Link ADR và issue/task ID.
- Upstream URL, version/tag/commit và SPDX license.
- Loại dùng: runtime, dev-only, offline tool, code copied, data hoặc asset.
- Benchmark trước/sau với workload và máy chạy.
- Test correctness, determinism, serialization và cross-language nếu liên quan.
- Ngân sách CPU/memory/binary size.
- Cờ bật/tắt hoặc kế hoạch rollback.
- Cập nhật `NOTICE`/credits và inventory nếu license yêu cầu.
- Owner và lịch kiểm tra update.
- Không có finding critical/high của các gate bản đồ liên quan.

## Ba hành động tiếp theo

> **Cập nhật 2026-07-26 (lần 4).** ~~OSS-010 Criterion~~, ~~OSS-070 Newick~~ và ~~OSS-071 thuật toán
> `simplify`~~ đều đã **xong** trong cùng ngày. Danh sách 2026-07-24 gốc giữ ở cuối làm bản ghi.

1. **Nối `simplify` vào tracker sống.** Đây là chỗ bộ nhớ **thực sự** giảm — hiện `simplify` mới là
   một hàm thuần trả về giá trị, chưa gì thay thế bộ nhớ của `InMemoryLineageTracker`, nên đường
   tăng không trần trong `lineage.rs` **vẫn còn nguyên**. Cần một chính sách về *khi nào* chạy, ai
   cung cấp danh sách cá thể sống, và tương tác với `load_state`/Neo4j.
2. **OSS-072 — truy vấn MRCA.** Phụ thuộc OSS-071, nay đã mở khoá. Đây là thứ mở đường cho OSS-073
   (giao thức đo "line of descent" kiểu Avida), và là thứ hiện **không trả lời được** câu "hai cá
   thể này rẽ nhánh ở đâu".
3. **Đóng phần còn nợ của OSS-003:** tách phạm vi license cho code / model / dataset / asset, và
   tạo `NOTICE` cho các thành phần permissive đang được phân phối.

Ngoài ba việc trên: **OSS-011 (`tracing`)** vẫn mở nhưng đã bị hạ ưu tiên — nó là observability kỹ
thuật, không sinh bằng chứng khoa học nào. Và **nối `to_newick`/`simplify` vào IPC** cần một thay đổi
hợp đồng ở `PROJECT.md`.

Một mục **không** nằm trong ba việc trên nhưng đã mở: đối chứng con số ~4,2 ms trong doc comment của
`ResourceField::REGROWTH_STRIDE`, thứ mà bench không tái lập được (~0,36 ms). Nó là finding, chưa
phải lỗi đã xác định.

<details>
<summary>Danh sách cũ (2026-07-24) — giữ làm bản ghi</summary>

1. **Quyết định license của Anima Engine** và phạm vi riêng cho code, model, dataset,
   screenshot/asset. Đây là blocker quản trị duy nhất trước khi nhận code bên ngoài.
2. Tạo ADR cho **OS1**, chốt baseline rồi thêm Criterion, tracing, lychee và cargo-deny
   thành các PR nhỏ độc lập.
3. Sau khi `WorldArtifact` ổn định, tạo một fixture lưu vực 32×32 để chạy Landlab và
   một fixture grass productivity để chạy pyrealm; chỉ lưu output rút gọn + provenance.

</details>

Danh sách và bằng chứng nghiên cứu nằm trong
[`OPEN_SOURCE_LANDSCAPE.md`](../research/OPEN_SOURCE_LANDSCAPE.md).
