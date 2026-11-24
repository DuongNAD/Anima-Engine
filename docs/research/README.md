---
title: Chỉ mục nghiên cứu
status: active
owner: architecture
last_reviewed: 2026-11-14
review_cycle: quarterly
---

# Nghiên cứu

Thư mục này giữ **bằng chứng bên ngoài**: khảo sát, đánh giá ứng viên, nguồn khoa học và số đo
dẫn đến một quyết định. Nó không phải nguồn sự thật về hiện trạng — nguồn đó là code và
[Reference](../reference/README.md). Khi một nghiên cứu ở đây dẫn tới quyết định, quyết định được
ghi thành [ADR](../decisions/README.md), còn tài liệu nghiên cứu giữ lại lý do.

| Tài liệu | Phạm vi | Trạng thái |
|---|---|---|
| [`OPEN_SOURCE_LANDSCAPE.md`](OPEN_SOURCE_LANDSCAPE.md) | Đánh giá từng dự án nguồn mở theo hạng Adopt / Pilot / Oracle / Reference / Reject, kèm biên tích hợp và hạng rủi ro license | `accepted` · review 2026-07-26 |
| [`MAP_AND_ML_UPGRADE_RESEARCH.md`](MAP_AND_ML_UPGRADE_RESEARCH.md) | Đào sâu hai mảng: sinh thế giới/bản đồ, và mô hình machine (ML/tiến hoá) | `proposed` |
| [`RD_TECH_UPGRADE_ASSESSMENT_2026-08-07.md`](RD_TECH_UPGRADE_ASSESSMENT_2026-08-07.md) | Đánh giá ứng viên R&D cho đường dữ liệu, Meta-AI và sinh asset 3D; kế hoạch thử nghiệm ở [planning](../planning/RD_TECH_UPGRADE_PLAN_2026-08-07.md) | `proposed` |
| [`RESEARCH_SYNTHESIS_2026-11-13.md`](RESEARCH_SYNTHESIS_2026-11-13.md) | Tổng hợp năm báo cáo ngoài: thiết kế thí nghiệm, áp lực chọn lọc, tất định, thế giới nhiều thiết bị, phát hành. Ghi rõ điều nào đã kiểm, chưa kiểm hoặc sai; không chứa số đo của repo | `proposed` · 2026-11-13 |
| [`WORLD_SCALE_AND_MAP_GENERATION_RESEARCH_2026-11-22.md`](WORLD_SCALE_AND_MAP_GENERATION_RESEARCH_2026-11-22.md) | Phân tích tỉ lệ bản đồ ↔ kích thước sinh vật (`CREATURE_BODY_SCALE = 0.32`, `MapBounds 200` vs World v2 `40,96 km`), địa mạo học, sinh thái không gian, nén không gian game thế giới mở, khảo sát 10 kho mã nguồn mở tạo/render map, kèm 3 phụ lục (cân chỉnh sinh thái `512²`, kiến trúc `NR-SPIKE` `wgpu`/`bevy_terrain`, mở rộng header `ANMW v3`) | `proposed` · 2026-11-22 |
| [`VIABILITY_PRICE_STATISTICS_RESEARCH_2026-11-22.md`](VIABILITY_PRICE_STATISTICS_RESEARCH_2026-11-22.md) | Sinh thái học khả năng sống sót trong ngân sách năng lượng đóng (Holling III, GUD, DEB capital breeding, detritus) đối chiếu với các số đo SEL2/S9/DIAG-SEL2 trong STATE; đồng nhất thức Price 5 thành phần cho thế hệ chồng lấp + MLS1; ghi chú về cách ghép `Cliff's delta` với thiết kế ghép cặp theo seed; gợi ý thống kê (`r_c`, `HL`, `RMST`, `SACE`) làm đầu vào cho VIAB-1, E3b, SEL2-P2 và `QD-ISLAND`. Các con số lấy từ nguồn ngoài là giả thuyết chưa xác minh, không đặt tiêu chí cổng | `proposed` · 2026-11-22 |
| [`NATIVE_RENDERER_WGPU_TAURI_BAKING_RESEARCH_2026-11-22.md`](NATIVE_RENDERER_WGPU_TAURI_BAKING_RESEARCH_2026-11-22.md) | Đào sâu ứng viên (a) của spike `NR-SPIKE` (trước `ADR-0011`, chưa tồn tại): `wgpu 30` trong cửa sổ Tauri v2 (`DX12` child HWND / `CompositionVisual`, `CAMetalLayer` trên macOS), hit-testing, vòng đời `CurrentSurfaceTexture`, DXC vs FXC, các kỹ thuật cho mục tiêu 30 FPS trên Iris Xe; và các đề xuất cho thẻ nướng `WV2-5` (chạy trên OxideSwarm), ghi rõ chỗ nào khác thiết kế `WV2-D` hiện hành. Mọi lựa chọn là giả thuyết để spike kiểm | `proposed` · 2026-11-22 |
| [`PROCEDURAL_CREATURE_SKINNING_GAIT_PIGMENT_RESEARCH_2026-11-22.md`](PROCEDURAL_CREATURE_SKINNING_GAIT_PIGMENT_RESEARCH_2026-11-22.md) | Bọc da liền mạch cho chuỗi đốt (`sdRoundCone+smin`, Naive Surface Nets), biến dạng `DQS`/LBS (rủi ro sáng chế chưa xác minh, cần rà pháp lý), dáng đi trong giới hạn nguyên tắc B1 "chỉ vẽ cái mô phỏng có" (mỗi kỹ thuật gắn nhãn Trang trí hoặc Tương lai — cần đổi mô phỏng), sắc tố Gray-Scott + Beer–Lambert, và ngân sách đám đông iGPU (LOD hiện vượt trần tam giác, phải cắt) | `proposed` · 2026-11-22 |
| [`BRAIN_BODY_COEVOLUTION_EVODEVO_LOCOMOTION_RESEARCH_2026-11-22.md`](BRAIN_BODY_COEVOLUTION_EVODEVO_LOCOMOTION_RESEARCH_2026-11-22.md) | Đánh giá một báo cáo ngoài về đồng tiến hoá não–thân (Mertan & Cheney, bảo vệ đổi mới hình thái), evo-devo (chuẩn phản ứng, dị hình giới tính, tăng trưởng trong đời) và vận động tiếp đất (ma sát dị hướng, ABA, SPD). Đối chiếu với ADR-0001/0003/0006: não hiện xuất tham số CPG chung nên đổi số đốt không lệch số chiều; vật lý đề xuất là biến thể của Phương án B ADR-0006; mọi số hiệu năng của báo cáo gốc chưa xác minh | `proposed` · 2026-11-22 |
| [`EVOLUTION_OBSERVATORY_RESEARCH_2026-11-22.md`](EVOLUTION_OBSERVATORY_RESEARCH_2026-11-22.md) | Đánh giá một báo cáo ngoài về đài quan sát tiến hoá: biểu đồ Muller (làm hình phân tích ngoại tuyến), bộ lọc sự kiện bền vững kiểu MODES, phép thử nguồn–bẫy Pulliam cho xâm lấn đảo, chuẩn dữ liệu phả hệ ALife, siêu dữ liệu phát hành (RO-Crate, ODD, Zenodo). Ghi rõ chỗ xung đột với hợp đồng tất định (bit-exact đa nền tảng là QĐ-M6, chưa quyết) và các con số chưa xác minh | `proposed` · 2026-11-22 |
| [`OPEN_ENDEDNESS_LINEAGE_SPECIES_RESEARCH_2026-11-22.md`](OPEN_ENDEDNESS_LINEAGE_SPECIES_RESEARCH_2026-11-22.md) | Đánh giá một báo cáo ngoài về đo tiến hoá mở và phả hệ: line of descent (OSS-073) và hai lỗ lúc rà soát (lần sinh live không vào cây phả hệ — đã sửa opt-in ở F-066, #207; genotype trung gian bị giải phóng — vẫn mở), Bedau–Packard so với bóng trung tính SEL2-D, phép thử Red Queen cho S43, quy trình loài hai bước cho AE-S11. Sửa nghĩa các mã AE-S11/AE-S14/S43 mà báo cáo gốc hiểu sai | `proposed` · 2026-11-22 |
| [`QD_REALMS_HABITATS_RESEARCH_2026-11-22.md`](QD_REALMS_HABITATS_RESEARCH_2026-11-22.md) | Đánh giá một báo cáo ngoài về ba môi trường QD-REALMS (nước, không trung, hang): ý đáng giữ cho từng ADR môi trường sau này (cột nước 1D, chi phí giác quan, giới hạn bay từ vật lý, chuỗi thức ăn hang từ mùn bã thấm xuống). Không bắt đầu trước khi đóng Chặng 4; ghi các xung đột (đánh số ADR, kích thước thế giới, vật lý chưa có trọng lực/tiếp đất, `LocomotionMedium`) | `proposed` · 2026-11-22 |
| [`OPEN_WORLD_LOD_STREAMING_RESEARCH_2026-11-19.md`](OPEN_WORLD_LOD_STREAMING_RESEARCH_2026-11-19.md) | Công thức streaming của game thế giới mở (lưới ô, vòng đồng tâm, lớp thô thường trú) áp vào Anima: LOD chỉ ở phía render, mô phỏng không phụ thuộc camera; có đầu trang ghi phần đã cũ | `proposed` · review 2026-11-23 |
| [`GEMINI_PROJECT_EVALUATION_2026-11-21.md`](GEMINI_PROJECT_EVALUATION_2026-11-21.md) | Báo cáo đánh giá dự án và bốn hướng đóng góp của một worker Gemini, kèm tầm nhìn sản phẩm của chủ dự án; có đầu trang của lead ghi phần đã cũ, phần là ý kiến và phần mâu thuẫn với quyết định | `proposed` · review 2026-11-23 |

## Cách đọc

- Bảng quyết định trong khảo sát là **đánh giá kỹ thuật, không phải tư vấn pháp lý**. License phải
  được xác minh theo đúng tag/commit trước khi merge.
- Một mục ghi `Adopt` **không** có nghĩa là nó đã được thêm. Trạng thái thực thi nằm ở
  [kế hoạch áp dụng nguồn mở](../planning/OPEN_SOURCE_ADOPTION_PLAN.md); đợt review 2026-07-26 tìm
  thấy bốn mục `Adopt` chưa từng được thực thi.
- Quy trình nhập một thành phần nằm ở
  [chính sách nguồn mở](../governance/OPEN_SOURCE_POLICY.md).

## Nhịp bảo trì

Review hàng quý theo [chính sách tài liệu](../governance/DOCUMENTATION_POLICY.md): dự án upstream
còn được duy trì không, license/tag có đổi không, benchmark có còn chứng minh lợi ích không, và có
gỡ được dependency mà vẫn đọc được dữ liệu cũ không. Mỗi đợt review phải ghi **bằng chứng cho từng
thay đổi quyết định**, không sửa lặng lẽ ô trong bảng.
