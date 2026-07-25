---
title: Khảo sát dự án nguồn mở cho Anima Engine
status: accepted
owner: architecture
last_reviewed: 2026-07-24
review_cycle: quarterly
---

# Khảo sát dự án nguồn mở cho Anima Engine

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

## Ma trận ứng viên

| Dự án | Vai trò phù hợp | Giấy phép đã thấy | Quyết định | Biên tích hợp |
|---|---|---:|---|---|
| [Bevy](https://github.com/bevyengine/bevy) | ECS/scheduling Rust | MIT OR Apache-2.0 | Adopt, đang dùng | Giữ `bevy_ecs` 0.13 trong M1–M2; nâng cấp bằng ADR riêng |
| [tracing](https://github.com/tokio-rs/tracing) | Telemetry có cấu trúc | MIT | Adopt | Span/event, correlation ID; không thay causal ledger |
| [Criterion.rs](https://bheisler.github.io/criterion.rs/book/) | Benchmark Rust | MIT OR Apache-2.0 | Adopt | `dev-dependency`, benchmark headless |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | Audit license/advisory/source | Apache-2.0 OR MIT | Adopt sau khi chọn license dự án | Kiểm tra CI, không vào binary |
| [lychee](https://github.com/lycheeverse/lychee) | Kiểm tra link tài liệu | MIT OR Apache-2.0 | Adopt | CI/docs-only |
| [three-mesh-bvh](https://github.com/gkjohnson/three-mesh-bvh) | Raycast/truy vấn mesh Three.js | MIT | Pilot ưu tiên cao | Frontend, terrain tĩnh; benchmark và refit/rebuild rõ ràng |
| [Rapier](https://github.com/dimforge/rapier) | Collision/joint 2D–3D | Apache-2.0 | Pilot có feature flag | Rust core; chạy song song với solver hiện tại trước khi thay |
| [meshoptimizer](https://github.com/zeux/meshoptimizer) | Tối ưu/simplify/LOD mesh | MIT | Pilot | Pipeline asset/chunk ngoại tuyến, không chi phối simulation |
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

### 4. Dữ liệu và serialization

`WorldArtifact` (v2) đã là hợp đồng đa ngôn ngữ đang phát triển. FlatBuffers chỉ đáng
đưa vào khi benchmark cho thấy serialization/copy là nút thắt thực sự và migration
tool có thể đọc cả phiên bản cũ lẫn mới. Arrow/Parquet chỉ dành cho các lô thí nghiệm
lớn; JSON/CSV dễ debug vẫn là mặc định cho dữ liệu nhỏ.

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
