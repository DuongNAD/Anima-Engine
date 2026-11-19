---
title: ADR-0010 — Một world artifact ghim, chia ô, là nguồn sự thật cho mô phỏng và bản đồ
status: accepted
owner: architecture
last_reviewed: 2026-11-19
decision_date: 2026-11-19
supersedes: none
superseded_by: none
---

# ADR-0010 — Một world artifact ghim, chia ô, là nguồn sự thật cho mô phỏng và bản đồ

> **Chấp nhận ngày 2026-11-19.** Chủ dự án trả lời "oke theo ý bạn" cho ba đề xuất mà phiên lead nêu cùng ngày:
> chấp nhận ADR này, bỏ thứ tự ở `CREATURE_MORPHOGENESIS_PLAN.md:36`, và giữ 30 FPS ở mức Low trên Iris Xe
> với 1.000 sinh vật (giới hạn số thân liền ở tầng gần). Đó là cách phiên lead hiểu câu trả lời; nếu chủ dự án
> định ý khác, ADR này quay về `proposed`. Thiết kế đầy đủ:
> [Thế giới thật trên bản đồ và thân sinh vật](../ai/design/2026-11-19-feature-real-world-and-bodies.md).

## Bối cảnh

Đọc mã tại `91def40` (thiết kế §2):

- Bản đồ 3D vẽ thế giới do `worldGen.ts` sinh ở frontend: 2048², gồm hồ, sông, hang, thác và 14 loại thực vật.
- Mô phỏng nhận một ANMW v2 256² chỉ có độ cao, độ ẩm, nhiệt độ, dòng chảy và quần xã, đọc từ `$TMPDIR`. Nó chỉ dùng quần xã để tính sức chứa cỏ.
- App đặt hồ và cây ngẫu nhiên, có hồ rơi xuống biển. Thí nghiệm đặt chúng trên lưới đều.
- Hồ, cây và thức ăn của mô phỏng không được vẽ trong 3D.
- Bộ sinh JS không bảo đảm giống nhau giữa các engine, và `determinism_matrix.md` ghi rằng mỗi hệ điều hành ra một thế giới khác.

Chủ dự án muốn bản đồ *là* thế giới mô phỏng, có núi, hang, rừng, sông, và tải theo vùng như một game thế giới mở.

## Quyết định

1. **Một artifact ghim.** Thế giới được sinh **một lần**; byte được commit kèm SHA-256. Mô phỏng và renderer
   đọc cùng các byte đó. Sinh lại nghĩa là một artifact mới với hash mới, không sửa tại chỗ. Bộ sinh có xác
   định giữa các engine hay không vì vậy không còn quan trọng.
2. **ANMW v3, chia ô.** Artifact được cắt thành ô, mỗi ô có các mức chi tiết (mip) và một chỉ mục hash. Mỗi ô có:
   - độ cao, quần xã, mực nước và lưu vực hồ, sông, cửa hang, mật độ và loại thực vật, độ dốc, vùng đi được.
   
   Mô phỏng đọc bản 256² phủ cùng diện tích; renderer đọc các ô ở độ phân giải cao theo camera.
3. **Manifest khai `world_identity`.** Đó là SHA-256 của chỉ mục artifact. Preflight từ chối chạy khi artifact
   tìm thấy không khớp. Không còn đọc thế giới từ `$TMPDIR` cho một run có manifest.
4. **Renderer vẽ thực thể của mô phỏng.** Hồ, cây, thức ăn và mật độ cỏ trên bản đồ đến từ mô phỏng. Cá, chim, thú
   nền và hang chưa có trong mô phỏng được gắn nhãn trang trí cho tới khi một thẻ mô phỏng thêm chúng.
5. **Không đổi ngầm thí nghiệm đã đăng ký.** SEL2-P, E3 và các run đã có giữ nguyên cách đặt hồ, cây và founder.
   Đặt theo artifact chỉ là mặc định của app và của manifest mới khai nó.
6. **Ngân sách tính theo vùng nhìn thấy.** Mục tiêu: 30 FPS ở Low trên Iris Xe với 1.000 sinh vật. Thân liền chỉ
   dùng cho tối đa K con ở tầng gần; K khai theo tier.

## Hệ quả

- Mở các thẻ W1 (ANMW v3), W2 (đặt theo artifact, preflight, IPC `start_from_manifest`), W3 (địa hình bán hiện
  thực) và W4 (tải theo vùng) của thiết kế. V1 và B1 (chỉ frontend) không phụ thuộc ADR này.
- `IPC_CONTRACT.md` đổi khi W2 thêm lệnh; `scripts/check_ipc_reference.mjs` bắt việc đó.
- Thay artifact thì `LIVE_MODEL_VERSION` không đổi, nhưng `world_identity` của manifest đổi, nên hai run khác
  artifact không được so như cùng một thế giới (ADR-0008, quy tắc kỷ engine).
- `CREATURE_MORPHOGENESIS_PLAN.md:36` được bỏ qua cho renderer thân liền (B1). Khi `DevelopedPhenotype` có
  thật, bộ dựng mesh phải đọc phenotype thay vì genotype; đây là rủi ro sửa lại đã biết.

## Không quyết ở đây

- Kích thước thế giới mô phỏng (diện tích, độ phân giải lưới sinh thái). Đổi nó là đổi luật, cần quyết định riêng.
- Hang đi vào được (voxel), cá và chim có thật trong mô phỏng.
