---
title: Khảo sát dự án nguồn mở cho Anima Engine
status: accepted
owner: architecture
last_reviewed: 2026-07-26
review_cycle: quarterly
---

# Khảo sát dự án nguồn mở cho Anima Engine

> **Bản review 2026-07-26.** Đợt review định kỳ đầu tiên. Nó làm ba việc: bổ sung
> [chín ứng viên](#ma-trận-ứng-viên--bổ-sung-2026-07-26) chưa có trong bảng gốc, **sửa lại bốn
> quyết định mà code hiện tại đã bác bỏ tiền đề** (xem
> [§ Điều đã thay đổi](#điều-đã-thay-đổi-kể-từ-2026-07-24)), và ghi nhận rằng license của chính
> Anima Engine đã được quyết định — điều này đổi hạng rủi ro của mọi ứng viên copyleft.

## Kết luận

Anima Engine nên giữ **Rust + `bevy_ecs` làm lõi có thẩm quyền**, bổ sung các thư
viện nhỏ theo vấn đề cụ thể, và dùng các mô hình khoa học Python như **oracle ngoại
tuyến**. Không nên nhúng một engine mô phỏng thứ hai vào runtime: chi phí đồng bộ
trạng thái, tái lập, build đa nền tảng và nâng cấp sẽ lớn hơn lợi ích tái sử dụng.

Mức quyết định:

- **Adopt**: đủ rõ về giá trị, giấy phép và biên tích hợp.
- **Pilot**: chỉ thêm sau benchmark nhỏ và phải có đường hoàn tác.
- **Oracle**: chạy ngoại tuyến để tạo dữ liệu chuẩn/hiệu chuẩn, không đóng gói runtime.
- **Reference**: học mô hình hoặc kiến trúc, không nhập code.
- **Reject now**: không phù hợp ràng buộc hiện tại.

### Ràng buộc license của chính Anima Engine (mới, 2026-07-26)

[`LICENSE`](../../LICENSE) ở thư mục gốc nay tồn tại và là **proprietary, all rights reserved**.
Trước đây [chính sách nguồn mở](../governance/OPEN_SOURCE_POLICY.md) coi việc chưa có license là
blocker quản trị duy nhất; blocker đó đã được gỡ, nhưng nó gỡ theo hướng **thắt chặt**:

- Ứng viên **permissive** (MIT, Apache-2.0, BSD, ISC, Zlib) vẫn theo quy trình cũ.
- Ứng viên **copyleft** (GPL, AGPL, và ở mức độ khác là LGPL/MPL) nay là **chặn cứng cho mọi
  đường tiếp xúc với code**, không phải "cần review thêm". Với các dự án đó, `Reference` nghĩa là
  đọc *bài báo và tài liệu mô tả mô hình*, không phải đọc source rồi viết lại.
- Hạng `Oracle` **không** bị ràng buộc này khi công cụ chạy tách biệt và chỉ output dữ liệu — nhưng
  output vẫn phải kiểm điều khoản, vì license của tool không tự phủ lên output
  ([chính sách nguồn mở](../governance/OPEN_SOURCE_POLICY.md) §"Code, model, data và asset").

Đây là lý do bảng dưới ghi **hạng rủi ro license** cho các ứng viên mới thay vì đoán một SPDX
expression. Không mục nào trong đợt review này được xác minh license theo tag; xem
[§ Việc còn nợ](#việc-còn-nợ-của-đợt-review-này).

## Ma trận ứng viên

| Dự án | Vai trò phù hợp | Giấy phép đã thấy | Quyết định | Biên tích hợp |
|---|---|---:|---|---|
| [Bevy](https://github.com/bevyengine/bevy) | ECS/scheduling Rust | MIT OR Apache-2.0 | Adopt, đang dùng | Giữ `bevy_ecs` 0.13 trong M1–M2; nâng cấp bằng ADR riêng |
| [tracing](https://github.com/tokio-rs/tracing) | Telemetry có cấu trúc | MIT | Adopt — **chưa thực thi** ⚠️ | Span/event, correlation ID; không thay causal ledger |
| [Criterion.rs](https://bheisler.github.io/criterion.rs/book/) | Benchmark Rust | MIT OR Apache-2.0 | ✅ **Adopted 2026-07-26** | `dev-dependency` (`default-features = false`), `src-tauri/benches/tick_systems.rs`; vô hình với `cargo tree -e normal` |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | Audit license/advisory/source | Apache-2.0 OR MIT | Adopt — blocker license đã gỡ; **một phần đã được phủ** | Kiểm tra CI, không vào binary |
| [lychee](https://github.com/lycheeverse/lychee) | Kiểm tra link tài liệu | MIT OR Apache-2.0 | **Superseded** bởi script nội bộ (đổi 2026-07-26) | Không còn cần; `scripts/check_docs_links.mjs` đã là gate |
| [three-mesh-bvh](https://github.com/gkjohnson/three-mesh-bvh) | Raycast/truy vấn mesh Three.js | MIT | **Defer** — tiền đề bị bác bỏ (đổi 2026-07-26) | Frontend; xét lại khi thật sự có `THREE.Raycaster` trên đường nóng |
| [Rapier](https://github.com/dimforge/rapier) | Collision/joint 2D–3D | Apache-2.0 | **Pilot, có tiền điều kiện cứng** (đổi 2026-07-26) | Rust core; **không mở pilot trước khi physics/CPG hết chạy song song** |
| [meshoptimizer](https://github.com/zeux/meshoptimizer) | Tối ưu/simplify/LOD mesh | MIT | **Defer** — sai công cụ cho heightfield (đổi 2026-07-26) | Xét lại khi mesh sinh vật đủ phức tạp, không cho terrain |
| [FlatBuffers](https://github.com/google/flatbuffers) | Serialization đa ngôn ngữ | Apache-2.0 | Defer | Chỉ xét nếu profiling chứng minh `WorldArtifact` (v2) là nút thắt |
| [Arrow Rust](https://github.com/apache/arrow-rs) | Dữ liệu thí nghiệm dạng cột/Parquet | Apache-2.0 | Defer | Export phân tích; không đưa vào save-game mặc định |
| [Virtual Ecosystem](https://github.com/ImperialCollegeLondon/virtual_ecosystem) | Mô hình khí hậu–nước–đất–sinh vật | BSD-3-Clause | Oracle ưu tiên cao | Python ngoại tuyến → golden fixtures/miền hợp lệ |
| [pyrealm](https://github.com/ImperialCollegeLondon/pyrealm) | Năng suất/sinh trưởng thực vật | MIT | Oracle | Hiệu chuẩn producer model ngoại tuyến |
| [Landlab](https://github.com/landlab/landlab) | Thủy văn/xói mòn/địa mạo | MIT | Oracle | Bản đồ nhỏ cố định → fixture độ dốc, dòng chảy, tích tụ |
| [SALib](https://github.com/SALib/SALib) | Sensitivity analysis | MIT | Adopt trong research harness | Đọc scenario output; Sobol/Morris ngoài runtime |
| [Madingley](https://madingley.github.io/) | Hệ sinh thái tổng quát cơ chế | Cần xác minh theo repo/phiên bản | Reference | Học cohort, energy budget; không sao chép code khi chưa rõ license |
| [BioDynaMo](https://github.com/BioDynaMo/biodynamo) | ABM hiệu năng cao | Apache-2.0 | Reference | Học module/scheduler/benchmark; không thêm engine C++ thứ hai |
| [MABE2](https://github.com/mercere99/MABE2) | Tiến hóa mô-đun | MIT | Reference | Học cách tách organism/evaluator/selection/experiment |
| [Neural MMO](https://github.com/NeuralMMO/environment) | Multi-agent environment | MIT | Reference | Học observation/action/task/replay, không nhúng Python runtime |
| [FLAME GPU 2](https://flamegpu.com/download/license/) | GPU ABM | AGPLv3 hoặc thương mại | Reject now | Không phù hợp Windows/iGPU mặc định và chính sách copyleft hiện tại |
| [The Bibites](https://thebibites.itch.io/the-bibites) | Cảm hứng artificial life | Không phải nguồn mở đã xác minh | Reference only | Chỉ tham khảo khái niệm; không tích hợp code |

> Bảng là đánh giá kỹ thuật, không phải tư vấn pháp lý. Giấy phép phải được khóa theo
> đúng tag/commit trước khi merge; repository không có license không được xem là nguồn
> mở chỉ vì truy cập công khai.

## Ma trận ứng viên — bổ sung 2026-07-26

Chín mục dưới đây chưa từng có trong bảng gốc. Cột license ghi **hạng rủi ro cần xác minh**, không
phải SPDX đã kiểm — xem [§ Việc còn nợ](#việc-còn-nợ-của-đợt-review-này).

| Dự án | Vai trò phù hợp | Hạng rủi ro license | Quyết định | Biên tích hợp |
|---|---|---|---|---|
| [burn](https://github.com/tracel-ai/burn) + `burn-wgpu` | Learner tensor/autodiff, backend GPU | Permissive (cần xác minh) | **Adopt, đang dùng** | Đã là runtime dep, ghim `0.13.2`. Bảng gốc **bỏ sót** — xem [§ Điều đã thay đổi](#điều-đã-thay-đổi-kể-từ-2026-07-24) |
| [tskit](https://github.com/tskit-dev/tskit) | Nén phả hệ, MRCA, `simplify()` | Permissive (cần xác minh) | **Adopt thuật toán, Reject crate** | Tự viết `simplify` + MRCA trên `LineageRelation`; **không** lấy binding C |
| Newick / [ape](https://github.com/emmanuelparadis/ape) / [ggtree](https://github.com/YuLab-SMU/ggtree) | Định dạng trao đổi cây phả hệ | **Định dạng — không phải code** | **Adopt định dạng** | Serializer thuần Rust ~40 dòng; 0 dependency. Không nhập code R |
| [SLiM](https://github.com/MesserLab/SLiM) | Selection phụ thuộc mật độ, đột biến đa vị trí | **Copyleft (rủi ro cao)** | Reference — **chỉ qua bài báo/tài liệu** | Học mô hình soft/hard selection; có thể dùng làm oracle nếu chạy tách biệt |
| [Avida](https://github.com/devosoft/avida) | Giao thức **đo** sự kiện tiến hoá | **Copyleft (rủi ro cao)** | Reference — chỉ qua bài báo | Học cách bám "line of descent"; **cần MRCA trước**, nên phụ thuộc mục tskit |
| [ALIEN](https://github.com/chrxh/alien) | Mẫu GPU compute cho particle/genome | **Copyleft (rủi ro cao)** | Reference — chỉ qua bài báo | Không nhập code; không thêm engine GPU thứ hai |
| [Thrive](https://github.com/Revolutionary-Games/Thrive) | Áp lực thích nghi, cân bằng gameplay tiến hoá | **Copyleft (rủi ro cao)**; asset tách riêng | Reference — chỉ qua bài báo | Học thiết kế áp lực chọn lọc, không lấy code/asset |
| Ribossome | Map genotype→phenotype trên wgpu | **Chưa xác minh được upstream** | Reject now | Không định vị được repo chính thức; không đủ hồ sơ để phân hạng |
| [hecs](https://github.com/Ralith/hecs) | ECS thay thế | Permissive (cần xác minh) | **Reject now** | `bevy_ecs` đã là ECS có thẩm quyền; đổi ECS là churn thuần, không giải bài toán nào đang mở |

## Điều đã thay đổi kể từ 2026-07-24

Theo [quy tắc 6 của chính sách tài liệu](../governance/DOCUMENTATION_POLICY.md) — khi code và tài
liệu xung đột thì **mở finding**, không tự coi code là đúng — mục này ghi bằng chứng cho từng thay
đổi thay vì lặng lẽ sửa ô trong bảng.

### F1 — `three-mesh-bvh`: tiền đề bị bác bỏ, hạ từ "Pilot ưu tiên cao" xuống Defer

Bảng gốc xếp đây là ứng viên frontend có tỷ lệ lợi ích/rủi ro tốt nhất, vì "tăng tốc raycasting với
địa hình, picking và line-of-sight". Code hiện tại không có việc đó để tăng tốc:

- Không có `THREE.Raycaster` nào trong toàn bộ `src/`.
- Cao độ địa hình lấy **giải tích** qua `sampleElevation` (`src/components/Landscape/utils/`), không
  raycast vào mesh.
- LOD theo khoảng cách **đã có** ở [`chunkLod.ts`](../../src/components/Landscape/utils/chunkLod.ts).
- `raycasts` trong [`PixiViewport.tsx`](../../src/PixiViewport.tsx) là **telemetry cảm biến từ
  backend vẽ thành đường 2D**, không phải raycasting Three.js. Trùng tên, khác việc.

BVH có lãi khi raycast nhiều lần mỗi frame vào một mesh tĩnh lớn. Điều kiện đó chưa tồn tại. Xét lại
khi có một `THREE.Raycaster` thật trên đường nóng — đó là trigger, không phải lịch.

### F2 — Rapier: giữ Pilot nhưng thêm tiền điều kiện cứng

Solver hiện tại không phải rigid-body tổng quát: `resolve_joints_system`
([`physics/dynamics.rs`](../../src-tauri/src/physics/dynamics.rs)) là điều khiển khớp lái bởi
`CpgOscillator` qua slerp tới góc mục tiêu, cộng spring-damper — 159 dòng, gắn chặt với tầng vận
động. Ba ràng buộc của repo đều chống lại việc thay:

1. Contract tất định (G1.3, [`DETERMINISM_CONTRACT.md`](../reference/DETERMINISM_CONTRACT.md)).
2. Luật zero-alloc trong tick loop (`SIMULATION_RULES.md`).
3. Khớp nối cứng với CPG — Rapier joint motor không ánh xạ thẳng sang pha CPG.

Điểm quyết định: **bug đang mở không phải độ chính xác vật lý.** `DETERMINISM_CONTRACT` §5 ghi rằng
physics/CPG chạy song song nên một run liền mạch còn không khớp chính nó — đó là thứ đang chặn gate
`an_inhabited_run_replays_from_its_trace_without_a_human` của [ADR-0004](../decisions/ADR-0004-observer-as-declared-intervention.md).
Rapier không sửa cái đó; nó chèn thêm một tầng vào giữa và làm việc sửa khó hơn.

**Tiền điều kiện mới cho OSS-040:** không mở pilot Rapier trước khi đường physics/CPG sống đã tất
định. Trước mốc đó, benchmark side-by-side không có nghĩa vì đường cơ sở chưa lặp lại được.

### F3 — `meshoptimizer`: sai công cụ cho dữ liệu hiện có

`chunkLod.ts` giảm chi tiết bằng **lấy mẫu thưa lại heightfield**, đúng phương pháp cho một trường
độ cao có tham số hoá. Mesh simplification là công cụ cho mesh tuỳ ý không có tham số hoá. Xét lại
khi mesh sinh vật (không phải terrain) đủ phức tạp để trở thành ngân sách tam giác thật.

### F4 — `lychee`: đã bị thay bằng công cụ nội bộ

OSS-013 nhắm thêm lychee. Việc đó **đã xong bằng đường khác**:
[`scripts/check_docs_links.mjs`](../../scripts/check_docs_links.mjs) là gate CI thực tế, đi qua
`git ls-files`, bỏ qua `docs/archive/` có chủ đích và fail trên link tương đối gãy. Thêm lychee bây
giờ là dependency thứ hai cho một việc đã có gate. Đóng OSS-013 là **superseded**, không phải "chưa
làm".

### F5 — `cargo-deny`: blocker đã gỡ, phạm vi đã hẹp lại

OSS-012 chờ quyết định license; license đã có. Nhưng phần **advisory** nay đã được `cargo audit`
(config `src-tauri/.cargo/audit.toml`) và `npm audit --audit-level=high` phủ trong CI. Giá trị còn
lại của `cargo-deny` là **licenses** và **bans/sources** — và với một sản phẩm proprietary, kiểm
license tự động của cây phụ thuộc là thứ đáng có. Giữ Adopt, nhưng phạm vi là license/bans, không
phải advisory.

### F6 — `burn` bị bỏ sót khỏi inventory

`burn 0.13.2`, `burn-ndarray` và `burn-wgpu` là **runtime dependency đang chạy** nhưng không có
dòng nào trong ma trận gốc — trong khi ma trận có cả những thứ chưa từng thêm. Đây là lỗi inventory,
đúng loại mà [chính sách nguồn mở](../governance/OPEN_SOURCE_POLICY.md) §"Hồ sơ bắt buộc" tồn tại để
chặn. Đã thêm dòng. Ghi chú vận hành ở [`CLAUDE.md`](../../CLAUDE.md): bản ghim 0.13.2 **không phải**
vấn đề bảo mật, và nâng cấp làm vỡ `ai/model.rs` + `core/training.rs`.

### F7 — Criterion và tracing: quyết định "Adopt" chưa từng được thực thi

Cả hai được chốt Adopt ngày 2026-07-24 (OSS-010, OSS-011). Không cái nào có trong
[`src-tauri/Cargo.toml`](../../src-tauri/Cargo.toml) tính đến 2026-07-26. Hệ quả không còn là chuyện
vệ sinh: §3.2 của [`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) là một mục **P0**
nói rằng tuyên bố "60 FPS real-time" của dự án chưa từng được đo, và `BENCHMARK_BASELINE.md` tự khai
số hiện tại là proxy. Criterion là công cụ đã được duyệt cho đúng việc đó, và nó hợp với ràng buộc
vận hành nặng nhất của dự án — **không chạy full backend trên máy dev** — vì nó bench từng system
headless chứ không boot Tauri.

> **Đã đóng cùng ngày, 2026-07-26.** Criterion đã ship (OSS-010): `dev-dependency` +
> [`src-tauri/benches/tick_systems.rs`](../../src-tauri/benches/tick_systems.rs), 16 số đo thật vào
> [`benchmark_report.json`](../../benchmark_report.json). Xem
> [`docs/how-to/BENCHMARKING.md`](../how-to/BENCHMARKING.md).
>
> **`tracing` (OSS-011) thì chưa**, và đợt đo đã hạ ưu tiên của nó: OSS-010 nằm trên đường tới hạn
> P0 vì nó sinh **bằng chứng**; `tracing` là observability kỹ thuật và không sinh bằng chứng nào cho
> §3.2. Nó vẫn `Adopt`, chỉ không còn khẩn.
>
> **Hai thứ đợt đo tìm ra mà bảng này chưa phản ánh:** phần cứng mục tiêu đã đổi sang i5-14600KF
> (khai báo *Dell Vostro 3530* cũ vô hiệu), và con số ~4,2 ms trong doc comment của
> `ResourceField::REGROWTH_STRIDE` **không tái lập được** — release build cho ~0,36 ms. Cái sau là
> finding cần đối chứng, không phải lỗi đã xác định; việc stride vẫn đúng (đo được 3,97×).

## Việc còn nợ của đợt review này

Ghi ra để không ai nhầm bảng này là đã kiểm license:

- **Chưa mục nào trong bảng bổ sung được xác minh license theo tag/commit.** Cột "hạng rủi ro" là
  phân loại để xếp thứ tự xử lý, **không phải** kết quả đọc file `LICENSE` của upstream. Bắt buộc
  xác minh trước mọi tiếp xúc với code, theo quy trình 9 bước ở
  [chính sách nguồn mở](../governance/OPEN_SOURCE_POLICY.md).
- Ribossome chưa định vị được upstream chính thức. Nếu người yêu cầu có link, mở lại hồ sơ; nếu
  không, giữ Reject vì thiếu hồ sơ chứ không vì đánh giá kỹ thuật.
- SALib giữ hạng "Adopt trong research harness" nhưng **hook point nay đã tồn tại thật**:
  `run_ensemble` / `run_paired_ensemble` (AE-108/AE-S14) trong
  [`core/experiment_runner.rs`](../../src-tauri/src/core/experiment_runner.rs). Điều chưa có là thiết
  kế mẫu Saltelli. Đó là ~100 dòng Rust thuần, không cần dependency Python trong runtime — cần một
  quyết định rõ ràng là viết trong Rust hay chạy SALib ngoại tuyến trên output đã export.

## Phân tích theo miền

### 1. Lõi mô phỏng

`bevy_ecs` đã nằm trong code và phù hợp với nhu cầu entity/component/schedule. Giá trị
lớn nhất hiện nay đến từ việc ổn định luật, seed, tick order, artifact và benchmark,
không phải thay ECS. Bevy phát hành nhanh và có breaking changes; vì vậy nâng từ 0.13
phải là một migration độc lập, không trộn với triển khai chuỗi thức ăn.

`tracing` giải quyết observability kỹ thuật. “Một con sói chết vì sao” vẫn phải là sự
kiện miền có schema, entity ID, tick và chuỗi nguyên nhân; log không phải dữ liệu mô
phỏng có thể truy vấn lâu dài.

### 2. Vật lý, không gian và dựng hình

> **Cập nhật 2026-07-26.** Ba đoạn dưới đây là đánh giá gốc và vẫn đúng *về nguyên tắc*, nhưng
> tiền đề của hai trong ba đã bị code bác bỏ. Đọc [F1](#f1--three-mesh-bvh-tiền-đề-bị-bác-bỏ-hạ-từ-pilot-ưu-tiên-cao-xuống-defer),
> [F2](#f2--rapier-giữ-pilot-nhưng-thêm-tiền-điều-kiện-cứng) và [F3](#f3--meshoptimizer-sai-công-cụ-cho-dữ-liệu-hiện-có)
> trước khi hành động theo mục này.

Rapier phù hợp cho collision, sensor, joint và spatial query tổng quát. Nhưng chuyển
toàn bộ vận động sinh vật sang physics engine có thể làm giảm tính tái lập và tăng
chi phí. Pilot chỉ nên đo collision/query trên cùng fixtures ở 100 và 1.000 tác nhân.

`three-mesh-bvh` là ứng viên frontend có tỷ lệ lợi ích/rủi ro tốt nhất: tăng tốc
raycasting với địa hình, picking và line-of-sight hiển thị mà không thay luật sinh
thái. `meshoptimizer` phù hợp pipeline LOD/chunk; kết quả tối ưu phải được kiểm tra
silhouette, normal, UV và memory trước/sau.

### 3. Khoa học sinh thái

Virtual Ecosystem có phạm vi gần Anima nhất nhưng dùng Python và mục tiêu nghiên cứu.
Giá trị tốt nhất là một bộ oracle:

1. Anima xuất world/scenario nhỏ có seed cố định.
2. Adapter chuyển đổi đơn vị và lưới sang input của mô hình tham chiếu.
3. Oracle tạo output hoặc miền hợp lệ.
4. Kết quả được chuẩn hóa thành fixture nhỏ có provenance.
5. Rust test so sánh invariant, xu hướng và ngưỡng sai số — không đòi từng số giống hệt.

Landlab kiểm chứng thủy văn/xói mòn; pyrealm hiệu chuẩn năng suất thực vật; SALib tìm
tham số nào chi phối kết quả. Cách này giữ runtime gọn mà vẫn tận dụng khoa học mở.

#### 3.1 Landlab: vì sao Oracle chứ không phải dependency (chi tiết, 2026-07-26)

Ghi riêng vì đây là câu hỏi hay bị hiểu nhầm thành "không dùng Landlab". Landlab **được duyệt** và
nằm ở OSS-021. Cái bị loại là **link nó vào runtime**, vì bốn lý do cụ thể với code hiện tại — không
phải vì đánh giá thấp thư viện.

**1. Ngôn ngữ và vòng lặp.** Tick loop là Rust ở 60 FPS với luật zero-alloc. Đưa Landlab vào runtime
nghĩa là nhúng CPython vào binary Tauri và gọi qua nó mỗi tick. Điều này va thẳng vào luật đã ghi ở
[§ Những thứ không nên kết hợp](#những-thứ-không-nên-kết-hợp): *không để Python oracle điều khiển
simulation desktop theo từng tick*.

**2. Tất định.** [`step_water`](../../src-tauri/src/core/dynamic_fields.rs) định tuyến dòng chảy
**đồng bộ** — scatter vào `inflow` rồi mới apply — với ghi chú trong code nói rõ mục đích là *độc
lập thứ tự*. Đó là một quyết định tất định có chủ ý. Flow router của Landlab chạy trên numpy/scipy;
thứ tự rút gọn dấu phẩy động và phiên bản thư viện sẽ trở thành một phần danh tính của run. Dự án
vừa bỏ công gỡ đúng loại rò rỉ này (G1.3, cộng một test quét mã nguồn chặn `thread_rng()` quay lại);
thêm một stack số học Python là mở lại cửa đó ở chỗ khó thấy hơn.

**3. Invariant thật của Anima chặt hơn thứ Landlab tối ưu.** Đọc kỹ `step_water`: mỗi bước ghi sổ
bằng lượng **thực sự** đã dịch chuyển, không phải lượng danh nghĩa, đúng để f32 rounding không làm
rò khối lượng — và `water_budget_residual()` phải ở gần 0 (gate S16). Landlab là thư viện nghiên
cứu, tối ưu cho độ trung thực khoa học, không cho một ngân sách đóng bit-exact trong f32. Thay bước
nước bằng Landlab sẽ phá đúng cái invariant mà dự án đang gate.

**4. Sai thang thời gian.** Landlab được thiết kế cho landscape evolution ở thang địa chất — hàng
nghìn tới hàng triệu năm mỗi run. `step_water`/`step_erosion` chạy trên trường 128×128 **mỗi tick**,
cùng nhịp với agent. Hai thang này không gặp nhau.

**Nhưng khoảng trống mà Landlab chỉ ra là có thật**, và đó chính là giá trị của nó với tư cách
oracle. Hai chỗ cụ thể:

- `downstream[i]` là steepest-descent đơn hướng (kiểu D8), tính một lần. Không có **flow
  accumulation** — nghĩa là sông không lớn dần về hạ lưu, và không có bước lấp/khoét vùng trũng.
- [`step_erosion`](../../src-tauri/src/core/dynamic_fields.rs) là công thức **cục bộ**
  (`K · precip · slope / (1 + root_resist)`) và **không vận chuyển trầm tích**: vật liệu bị xói không
  được mang đi đâu hay bồi ở đâu, nó chỉ đặt một số turbidity. So với mô hình stream-power có
  detachment/deposition thì đây là khoảng trống mô hình thật, không phải đơn giản hoá vô hại.

Đường oracle đóng đúng hai khoảng trống đó mà không tốn một dòng dependency: chạy Landlab **một
lần, ngoại tuyến**, trên một lưu vực 32×32 cố định; xuất flow direction, flow accumulation và water
balance thành fixture đã rút gọn có provenance; rồi viết test Rust so **thứ tự xu hướng và tolerance**
— không đòi từng số giống hệt. Đó đúng là OSS-021 như đã viết.

**Trạng thái trung thực:** OSS-021 **chưa bắt đầu**, và nó bị chặn bởi OSS-020 (định dạng
`scientific-fixture` chưa tồn tại) cộng một chi phí vận hành chưa trả: cần môi trường Python trên
một máy dev đang có ràng buộc tài nguyên thật. Vậy trạng thái đúng là *"đã duyệt, chưa làm, đang bị
chặn"* — không phải *"đã loại"*.

### 4. Dữ liệu và serialization

`WorldArtifact` (v2) đã là hợp đồng đa ngôn ngữ đang phát triển. FlatBuffers chỉ đáng
đưa vào khi benchmark cho thấy serialization/copy là nút thắt thực sự và migration
tool có thể đọc cả phiên bản cũ lẫn mới. Arrow/Parquet chỉ dành cho các lô thí nghiệm
lớn; JSON/CSV dễ debug vẫn là mặc định cho dữ liệu nhỏ.

### 5. Phả hệ và bằng chứng tiến hoá (mới, 2026-07-26)

Đây là miền mà bảng gốc không phủ, và là chỗ có khoảng cách đo được lớn nhất giữa "dự án nói mình
mô phỏng tiến hoá" và "dự án chứng minh được điều đó".

**Hiện trạng.** [`evolution/lineage.rs`](../../src-tauri/src/evolution/lineage.rs) lưu mỗi lần sinh
sản thành một `LineageNode` kèm **bản sao đầy đủ** `MorphologyGenotype`, cộng một `LineageRelation`
cho mỗi cha mẹ. Không có bước prune và không có truy vấn tổ tiên. Hai hệ quả:

1. **Bộ nhớ tăng đơn điệu theo tổng số cá thể từng sống**, không theo số cá thể còn sống. Với một
   run 60 FPS dài, đó là đường tăng không có trần.
2. **Không truy được dòng dõi.** Không có MRCA nghĩa là không trả lời được "hai cá thể này rẽ nhánh
   ở đâu", và cũng không bám được *line of descent* — giao thức mà Avida dùng để **đo** một sự kiện
   tiến hoá thay vì kể lại nó.

**Điều tskit thật sự bán.** Nén phả hệ theo cạnh/khoảng + `simplify()`: bỏ các nhánh không còn hậu
duệ trong tập mẫu, giữ nguyên quan hệ tổ tiên của phần còn lại. Đó chính xác là hai vấn đề trên.

**Vì sao vẫn Reject crate.** Binding Rust của tskit gói thư viện C — thêm toolchain C vào một build
Windows đang có ràng buộc thật về đĩa và thời gian biên dịch. Nặng hơn: mô hình dữ liệu của tskit là
*tree sequence trên khoảng genomic*, dành cho nhiễm sắc thể có tái tổ hợp. Genotype của Anima là đồ
thị node/edge kiểu Karl Sims cộng vector tham số, không có toạ độ genomic. Lấy crate là trả giá cho
một mô hình không dùng tới, và ép dữ liệu vào một hình dạng sai để dùng được API.

Lấy **thuật toán** thì giữ được cả hai: `simplify` + MRCA trên `Vec<LineageRelation>` là code thuần
Rust, tất định, không cấp phát trên đường nóng, và không thêm dòng nào vào inventory dependency.

**Newick là món rẻ nhất trong toàn bộ khảo sát này.** Nó là *định dạng*, không phải thư viện — nên
`ape` (R) và `ggtree` mang license gì cũng không liên quan, vì không có code nào được nhập. Một
serializer ~40 dòng trên đồ thị lineage sẵn có mở ra toàn bộ toolchain phylogenetics của R/Python.
Lợi ích thứ hai đáng kể hơn lợi ích thứ nhất: **một parser bên thứ ba là kiểm tra độc lập cho tính
đúng của phả hệ**. Cây có chu trình, có node mồ côi hoặc nhiều gốc sẽ bị parser ngoài từ chối — đúng
kiểu gate "nhắm vào chế độ hỏng thật" mà bộ gate của dự án đang theo
([`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) §2.1).

**Thứ tự phụ thuộc:** Newick → `simplify`/MRCA → giao thức đo kiểu Avida. Làm ngược thứ tự này sẽ
mắc kẹt, vì không có MRCA thì không có gì để xuất ra cây có nghĩa.

## Những thứ không nên kết hợp

- Không chạy đồng thời Bevy ECS và một ABM engine khác như hai nguồn sự thật.
- Không để Python oracle điều khiển simulation desktop theo từng tick.
- Không dùng renderer hoặc physics engine để xác định quy luật sinh thái.
- Không copy thuật toán/mô hình chỉ từ bài mô tả mà bỏ qua license của code và data.
- Không nâng đồng thời Bevy, artifact format và luật mô phỏng trong một thay đổi.
- Không chấp nhận một dependency chỉ vì “nhanh hơn”; phải có fixture, benchmark, ngân
  sách bộ nhớ, determinism và rollback.

## Tiêu chí tái đánh giá hàng quý

- Dự án upstream còn duy trì, release gần nhất và cảnh báo bảo mật.
- License/tag có thay đổi không.
- API surface Anima đang phụ thuộc có bao nhiêu điểm.
- Benchmark hiện tại còn chứng minh lợi ích không.
- Có thể loại bỏ dependency mà vẫn đọc được save/artifact cũ không.
- Adapter/oracle có provenance và tái tạo được fixture không.

Quy trình thực thi nằm tại
[kế hoạch áp dụng nguồn mở](../planning/OPEN_SOURCE_ADOPTION_PLAN.md); quy tắc nhập
dependency nằm tại [chính sách nguồn mở](../governance/OPEN_SOURCE_POLICY.md).
