---
phase: requirements
feature: living-world
title: Requirements — Thế giới sống
description: Sinh vật có hình thật, cử động thật, và một thế giới chạy trên sức tính toán góp từ nhiều máy
status: proposed
owner: simulation-architecture
last_reviewed: 2026-08-25
design: ../design/2026-08-25-feature-genome-to-mesh.md
planning: ../planning/2026-08-25-feature-living-world.md
testing: ../testing/2026-08-25-feature-living-world.md
---

# Requirements — Thế giới sống

Ba yêu cầu đi cùng nhau, nên chúng nằm chung một tài liệu. Tách ra thành ba dự án rời sẽ bỏ lỡ
chỗ chúng ràng buộc lẫn nhau — và chỗ đó là nơi rủi ro thật nằm.


## Bộ tài liệu

| Tài liệu | Nội dung |
|---|---|
| **Requirements** (tài liệu này) | cái người dùng muốn, ràng buộc, tiêu chí nghiệm thu |
| [Hợp đồng prompt](../../reference/CREATURE_PROMPT_CONTRACT.md) | ngữ pháp prompt, lexicon, ngân sách ký tự, invariant P01–P10 |
| [Design — genome tới mesh](../design/2026-08-25-feature-genome-to-mesh.md) | kiến trúc archetype, kho asset, cổng nghiệm thu |
| [Design — animation](../design/2026-08-25-feature-creature-animation.md) | gait thủ tục, action overlay, làm mượt |
| [Design — tính toán phân tán](../design/2026-08-25-feature-federated-compute.md) | shard theo thiết bị, ngăn `dormant`, mô hình tin cậy |
| [Plan](../planning/2026-08-25-feature-living-world.md) | thứ tự triển khai, ba việc đầu tiên, rollback |
| [Testing](../testing/2026-08-25-feature-living-world.md) | gate GM · PR · AN · FD |

---

## Shared understanding

Người dùng muốn Anima-Engine trở thành một **thế giới sống nhìn thấy được**, không phải một bảng
số có hình cầu chạy qua lại:

1. Mỗi sinh vật có **model 3D thật**, đẹp, và hình dạng của nó **suy ra từ gene và môi trường** —
   con vùng lạnh có lông dày vì nó sống ở vùng lạnh, không vì ai đó vẽ như thế. Model sinh bằng
   **AI text-to-3D**, với prompt chuẩn hoá, tối ưu, dưới 1000 ký tự.
2. Sinh vật **đi lại, ăn, uống, đánh nhau, ngủ, nghỉ**, và các chuyển động đó **mượt**.
3. Thế giới chạy trên **sức tính toán góp từ nhiều máy**. Máy nào kết nối thì nhận 1–vài sinh vật
   tuỳ cấu hình; máy host cũng chạy 1–nhiều sinh vật tuỳ sức mạnh của nó. Sinh vật **xuất hiện
   hoặc biến mất** trên bản đồ tuỳ theo thiết bị có đang góp sức hay không.

Đây là dự án nghiên cứu, không phải sản phẩm cần bảo mật thương mại. Nhưng nó vẫn cần **kết quả
bảo vệ được** — nếu một xu hướng tiến hoá không phân biệt được với "hôm đó ai online", thì kết quả
đó không dùng được vào việc gì.

---

## Problem statement

### Vấn đề sản phẩm

Hôm nay, mọi sinh vật trong khung cảnh 3D là **hình cầu bán kính 1.6**
([`LiveAgents.tsx:44`](../../../src/components/Landscape/LiveAgents.tsx)), đứng yên trừ việc trượt
theo vị trí, giống hệt nhau bất kể gene hay môi trường. Một thế giới tiến hoá mà mọi cá thể trông
như nhau thì không quan sát được bằng mắt — và quan sát được bằng mắt chính là lý do dựng một
simulator có đồ hoạ.

Đồng thời, toàn bộ quần thể chạy trên **một máy**, nên quy mô bị chặn bởi máy yếu nhất mà người
dùng có.

### Vấn đề kỹ thuật

| # | Vấn đề | Bằng chứng (đo 2026-08-25) |
|---|---|---|
| T1 | IPC payload không chở kích thước | `SegmentState` không có `length`/`radius`/`color` — `core/simulation_state.rs:23` |
| T2 | Không có tầng phenotype | `EnvSample`, `develop_at_birth`, `DevelopedPhenotype`, `spawn_developed`: grep toàn crate, **0 kết quả** |
| T3 | Không có IR hình thái, không có prompt, không có kho asset | chưa có file nào |
| T4 | CPG là **một** sine cho mỗi segment, không ghép nối | `ai/cpg.rs:23` |
| T5 | Renderer không nội suy giữa tick và frame | `LiveAgents.tsx` đọc payload trong `useFrame`, không giữ hai mẫu |
| T6 | Ledger có **ba** ngăn; không có chỗ cho agent ngủ đông | `ENERGY_LEDGER_CONTRACT.md §1` |
| T7 | Sharding có transport nhưng không có phân công theo năng lực | `ShardingConfig` chỉ có ba cổng — `core/components.rs:441` |
| T8 | Không có mô hình tin cậy cho contributor | trần cấu trúc có; kiểm ngữ nghĩa chưa có |

---

## Yêu cầu chức năng

### R1 — Hình thái nhìn thấy được

- **R1.1** Sinh vật hiển thị đúng kích thước và tỉ lệ của chính nó, không phải hằng số chung.
- **R1.2** Ngoại hình (bao phủ, màu, hoa văn, tai/đuôi, mắt, hàm) suy ra từ gene + môi trường lúc
  sinh, theo lexicon có kỷ luật.
- **R1.3** Model 3D sinh bằng text-to-3D API, prompt ≤ **1000 ký tự**, chuẩn hoá và versioned.
- **R1.4** Ngoại hình **không đổi** khi cá thể đi sang biome khác (D08).
- **R1.5** Sinh vật xuất hiện **ngay**, không chờ mạng.

### R2 — Cử động

- **R2.1** Sáu nhóm trạng thái: di chuyển (idle/walk/run/swim/fly), ăn, uống, đánh, ngủ, nghỉ.
- **R2.2** Chuyển giữa các trạng thái mượt: không giật pha, không giật theo nhịp tick.
- **R2.3** Cử động đúng với hình thái thật, kể cả hình thái tiến hoá bất thường (0/2/4/6+ chi).
- **R2.4** Trạng thái animation **dẫn xuất** từ trạng thái mô phỏng, không có state riêng.

### R3 — Tính toán phân tán

- **R3.1** Nhiều máy cùng tick một thế giới; host điều phối.
- **R3.2** Ngân sách agent của mỗi máy **đo được** từ phần cứng, tối thiểu 1.
- **R3.3** Máy host cũng chạy một phần quần thể theo sức của nó.
- **R3.4** Thiết bị vào/ra làm sinh vật xuất hiện/biến mất trên bản đồ.
- **R3.5** **Không mất EU** ở bất kỳ đường vào/ra nào, kể cả ngắt đột ngột.
- **R3.6** Chế độ chỉ-góp-CPU cho kết quả **bằng** chạy một máy.

---

## Yêu cầu phi chức năng

| # | Yêu cầu | Ngưỡng |
|---|---|---|
| N1 | Residual năng lượng | ≤ `1e-3` EU trong **mọi** cấu hình liên bang |
| N2 | Không cấp phát heap trong tick path | `allocs == 0` |
| N3 | Chi phí API mỗi run | ≤ 200 lần gọi (trần cứng, cấu hình được) |
| N4 | Prompt | ≤ 1000 ký tự cứng, p95 ≤ 700 |
| N5 | Độ trễ spawn nhìn thấy | 0 — không `await` nào giữa quyết định spawn và xuất hiện |
| N6 | `AgentCue` trên dây | ≤ 20 byte/agent/tick, giãn cách được |
| N7 | Đường mặc định vẫn chạy khi gỡ hết tính năng mới | app chạy đủ, checksum không đổi |

---

## Ngoài phạm vi

- Sinh model 3D **cho từng cá thể** — kiến trúc là theo archetype ([design §1](../design/2026-08-25-feature-genome-to-mesh.md)).
- Thư viện animation clip authoring — gait là thủ tục, cố ý.
- Lưới shard 2-D — v1 giữ phân dải 1-D như `ShardingConfig` hiện có.
- Mạng tình nguyện **mở ra internet công cộng** — cần auth + TLS trước, Slice F4.
- Chống đối thủ có động cơ. T1–T5 chặn shard hỏng và gian lận nghiệp dư, không hơn.
- Nâng burn/bevy, đổi WorldArtifact — không trộn vào cùng PR.

---

## Tiêu chí nghiệm thu

Không tuyên bố hoàn tất nếu thiếu bất kỳ mục nào:

1. Ảnh before/after cho thấy sinh vật **khác kích thước và khác hình** theo gene/môi trường.
2. Clip cho thấy sáu nhóm trạng thái, chuyển mượt, trên ít nhất ba hình thái khác nhau.
3. Hai máy cùng chạy một thế giới; rút mạng một máy giữa chừng; **residual vẫn ≤ `1e-3`**.
4. `contribute_matches_solo` xanh.
5. Gỡ toàn bộ kho asset ⇒ app chạy đủ, checksum run **không đổi**.
6. Toàn bộ gate hiện có vẫn xanh — không nới ngưỡng nào để đạt các mục trên.

---

## Câu hỏi mở

| # | Câu hỏi | Ai quyết | Chặn cái gì |
|---|---|---|---|
| Q1 | Nhà cung cấp text-to-3D nào | sau khi đo ở Slice A3 | không chặn Slice A1–A2 |
| Q2 | `Inhabit` có phải chế độ mặc định khi có nhiều người không | chủ dự án | không chặn `Contribute` |
| Q3 | Contributor chạy trong Tauri app hay trong trình duyệt | đo trước | ảnh hưởng probe và transport |
| Q4 | Ngăn `dormant` có vào save file không | thiết kế snapshot | chặn Slice F2 |

Q4 có câu trả lời gợi ý và cần chốt sớm: **có**. Một save không mang agent ngủ đông là một save
làm mất EU khi load, và đó đúng là loại lỗi mà `lock_baseline` không thể phát hiện sau khi đã chốt.
