---
title: Nghiên cứu tỉ lệ bản đồ, kích thước sinh vật và kiến trúc tạo thế giới
status: proposed
owner: architecture
last_reviewed: 2026-11-24
review_cycle: quarterly
---

# Nghiên cứu tỉ lệ bản đồ, kích thước sinh vật và kiến trúc tạo thế giới

> **Phạm vi & tính chất tài liệu.** Tài liệu này là **tổng hợp bằng chứng bên ngoài, đối chiếu mã nguồn và phân tích kiến trúc đề xuất** về bài toán tỉ lệ không gian (`MapBounds`, kích thước địa lý, tỉ lệ sinh vật ↔ cây cối ↔ địa hình), thời điểm mở rộng bản đồ, cùng các bài báo khoa học và kho mã nguồn mở liên quan tới tạo/dựng hình thế giới lớn. Đây **không** phải hợp đồng hay kế hoạch đã duyệt; trạng thái đo được hiện tại của dự án nằm duy nhất tại [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền). Các khảo sát nền trước đó nằm ở [`MAP_AND_ML_UPGRADE_RESEARCH.md`](MAP_AND_ML_UPGRADE_RESEARCH.md), [`OPEN_SOURCE_LANDSCAPE.md`](OPEN_SOURCE_LANDSCAPE.md) và [`RESEARCH_SYNTHESIS_2026-11-13.md`](RESEARCH_SYNTHESIS_2026-11-13.md).

---

## 0. Câu hỏi nghiên cứu, quy ước bằng chứng và TL;DR

### Câu hỏi trung tâm

1. **Có nên mở rộng map sớm hơn** để xử lý hiện tượng *"đảo hơi bé thì phải vẽ sinh vật bé"* (hoặc tách tỉ lệ thân so với vị trí) hay không?
2. Các công trình **địa mạo học tính toán & đồ hoạ địa hình**, **sinh thái học không gian**, **nền tảng ALife**, **game thế giới mở** và **kho mã nguồn mở** giải quyết bài toán quy mô bản đồ, tỉ lệ sinh vật và kiến trúc dựng hình thế giới lớn như thế nào?

### Quy ước phân loại bằng chứng

| Nhãn | Nghĩa trong tài liệu này |
|---|---|
| **Đã kiểm** | Đọc trực tiếp từ mã nguồn, hợp đồng và tài liệu thiết kế của Anima Engine ngày `2026-11-22`. |
| **Nguồn ngoài — bài báo** | Kết quả/phương pháp từ công bố khoa học có DOI hoặc thông tin xuất bản đã xác minh ngày `2026-11-22`. |
| **Nguồn ngoài — mã nguồn mở / ngành game** | Thông số kho mã nguồn mở (URL, ngôn ngữ, giấy phép kiểm ngày `2026-11-22`) và thực tiễn thiết kế thế giới mở đã kiểm chứng. |
| **Suy luận** | Tính toán định lượng và suy luận kiến trúc rút ra cho Anima Engine từ các nguồn trên; cần kiểm chứng lại bằng benchmark/thử nghiệm trước khi nâng thành ADR. |

### TL;DR — năm kết luận cốt lõi

1. **Vấn đề "đảo bé → vẽ sinh vật bé / lệch va chạm" là có thật trong mã (`Đã kiểm`).** Hộp mô phỏng mặc định chỉ rộng `200 × 200` sim-units ([`COORDINATE_CONTRACT.md`](../../COORDINATE_CONTRACT.md)), trong khi một founder 3 đốt đã dài cỡ `3,0` sim-units (`~1,5%` chiều ngang cả thế giới). Để thân sinh vật không to ngang cây và hồ khi vẽ trên `RENDER_SIZE = 1200`, renderer buộc phải gán `CREATURE_BODY_SCALE = 0.32` ([`agentPose.ts`](../../src/components/Landscape/utils/agentPose.ts)), khiến sinh vật nhỏ hơn khoảng cách vị trí thực tới ~3 lần và hai cá thể chạm nhau (`r = 0,2`) bị vẽ hở ~4 bán kính thân.
2. **Mở rộng quy mô địa lý của World v2 lên `40,96 km` không tự giải quyết tỉ lệ sinh vật (`Đã kiểm` + `Suy luận`).** Thiết kế `WV2-D` ([`2026-11-20-world-v2-design.md`](../ai/design/2026-11-20-world-v2-design.md)) mở rộng bề ngang địa hình thành `40.960 m` (`1.677,7 km²`), nhưng vẫn ánh xạ vào `MapBounds = 200 × 200` (`1 sim-unit = 204,8 m`). Nếu vẽ đúng 1:1 theo vị trí, mỗi con vật dài hơn `600 m`; nếu vẽ đúng kích thước sinh học `1–2 m`, nó biến mất dưới độ phân giải `10 m/ô` của heightmap. Vì vậy `WV2-D` (`LD-1`) buộc phải tách rời tỉ lệ sinh vật khỏi địa hình theo kiểu RTS/sa bàn.
3. **Địa mạo học không ép đảo phải rộng `40 km`, nhưng đa đai khí hậu nhiệt đới có băng hà thì có (`Nguồn ngoài — bài báo` + `Suy luận`).** Theo Montgomery & Dietrich (1988, 1989, 1992) và định luật Hack, mạng sông dạng cành cây (dendritic) tự tương tự xuống tới lưu vực vài trăm mét – vài km; một hòn đảo `4–8 km` vẫn đạt chuẩn mạng sông `G-SHAPE`. Tuy nhiên, để chuyển tiếp mượt từ biển nhiệt đới lên núi tuyết vĩnh cửu (`ELA ~4.900 m`) mà núi không thành gai nhọn thì sườn cần rộng ~`20–40 km`.
4. **Sinh thái học không gian cho thấy giới hạn hai đầu (`Nguồn ngoài — bài báo` + `Suy luận`).** Quá nhỏ so với tầm di chuyển gây đồng bộ bùng nổ–sụp đổ (Huffaker 1958; Durrett & Levin 1994) và thiếu vùng sống cho thú săn khối lượng lớn (Kelt & Van Vuren 2001; Jetz et al. 2004; Tamburello et al. 2015). Ngược lại, mở rộng bản đồ khi mật độ cá thể ít sẽ rơi vào "lời nguyền pha loãng" (curse of dilution).
5. **Khuyến nghị thời điểm & hướng đi (`Suy luận`).** **Không** mở rộng map v1 (`worldGen.ts`) và **không** thay đổi `MapBounds = 200` trước khi đóng Chặng 4 (`VIAB-1 → E3b / SEL2-P2`). Thay vào đó: (a) giữ khả năng tham số hoá `world_extent_m` và `sim_bounds_extent` trong kiến trúc World v2 / `ANMW v3`; (b) sau Chặng 4, cân nhắc mở thêm preset **Đảo 1:1 cỡ vừa (`4–8 km`, `MapBounds = 512–800`, lưới sim `512²`, `CREATURE_BODY_SCALE = 1.0`)** cho giai đoạn Quan sát Quần đảo (`QD-ISLAND`); và (c) đưa `kurtkuehnert/bevy_terrain` (`MIT OR Apache-2.0`) làm ứng viên Pilot chính cho pha renderer native (`NR-SPIKE` / ADR-0011) kèm bắt buộc **Floating Origin / camera-relative `f32`**.

> **Cập nhật 2026-11-24 — những gì đã đổi sau ngày viết (không đổi kết luận nào ở trên).**
> - **Founder 2 đốt trong thí nghiệm.** Founder 3 đốt vẫn là mặc định của code, nhưng E3b và SEL2-P2
>   dùng founder 2 đốt (genotype genesis bỏ lá đuôi) qua khoá opt-in `live.founder_genotype = 1`
>   (QĐ-E3B D5 và QĐ-SEL2P2 D1 trong [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md)). Con số
>   "founder dài `~2,0–3,0` sim-units" ở kết luận 1 và §1 là của founder mặc định; founder 2 đốt ngắn
>   hơn khoảng một đốt (`length = 1.0`), nên lệch tỉ lệ khi vẽ giảm nhưng không mất.
> - **MAP-D4: artifact World v2 đầu tiên chưa qua cổng.** Thẻ WV2-4 (PR #249, lúc viết chưa merge) đo
>   ở `4096²` không ứng viên nào qua đủ 15 cổng (C01 qua 2/15). Gốc là mạng kênh bị cụt
>   (`omega_max = 3`), kéo theo không có thác, diện tích hồ dưới sàn 11–31 lần, bờ biển trơn không
>   vách và khoảng 4.600 gai nhọn ở 10 m. Chủ dự án quyết dùng tạm C01 với `"status": "provisional"`,
>   giữ nguyên cổng và ngưỡng, và để việc sửa vật lý (khởi phát kênh và dòng chảy mặt, thác ở
>   knickpoint, hồ, vách biển, làm mượt gai) cho thẻ sau **WV2-PHYS2** (mục `MAP-D4` trong
>   [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md)). `Suy luận`: điều này không mâu thuẫn với §2.2
>   (Montgomery & Dietrich đòi ngưỡng $A S^n \ge C$, không đòi bề ngang), nhưng cho thấy khẳng định
>   "đảo `4–8 km` vẫn đạt sông bậc Strahler 3–4 và thác ghềnh" ở §2.2 là giả thuyết chưa đo, và generator
>   hiện tại chưa đạt điều đó ngay ở `40,96 km`. Phần khởi phát kênh của §2.2 và `landlab` làm oracle
>   (§5, dòng 2 của bảng kho mã) là đầu vào trực tiếp cho WV2-PHYS2.

---

## 1. Chẩn đoán định lượng hiện trạng tỉ lệ trong Anima Engine (`Đã kiểm`, 2026-11-22)

### 1.1 Các hằng số không gian đang chi phối mã nguồn

Đối chiếu trực tiếp các nguồn sự thật trong repository cho thấy tỉ lệ khó nhìn hiện nay xuất phát từ việc **ba hệ quy chiếu** (địa lý của map, hộp vật lý mô phỏng, và kích thước cơ thể sinh vật) đang bị bó vào cùng một khung `200 × 200` sim-units:

| Thành phần | Vị trí trong mã (`Đã kiểm`) | Giá trị hiện tại | Hệ quả trực tiếp |
|---|---|---|---|
| Miền mô phỏng ngang | [`sim_rules.rs`](../../src-tauri/src/core/sim_rules.rs) (`WORLD_MIN_XZ`, `WORLD_MAX_XZ`), [`COORDINATE_CONTRACT.md`](../../COORDINATE_CONTRACT.md) | `[-100, +100]²` → **`200 × 200` sim-units** | Toàn bộ thế giới sống nằm trong hình vuông cạnh `200`. |
| Miền mô phỏng dọc | [`sim_rules.rs`](../../src-tauri/src/core/sim_rules.rs) (`WORLD_MIN_Y`, `WORLD_MAX_Y`) | `[0, 10]` sim-units | Độ cao chuẩn hoá `[0, 1]` nhân `10`. |
| Lưới địa hình & cỏ sim | [`sim_rules.rs`](../../src-tauri/src/core/sim_rules.rs) (`DEFAULT_GRID_DIM`) | `256 × 256` (`65.536` ô) | Mỗi ô sim rộng `200 / 256 = 0,78125` sim-units. |
| Map v1 (frontend TS) | [`sharedWorld.ts`](../../src/utils/sharedWorld.ts), [`WorldShowcase.tsx`](../../src/components/Landscape/WorldShowcase.tsx), [`floraClearance.ts`](../../src/components/Landscape/utils/floraClearance.ts) | `2048²` (`supercontinent`), `RENDER_SIZE = 1200`, `HEIGHT_RATIO = 0.14` | `simScale = 1200 / 200 = 6` render-units / sim-unit (`liveAgentTransform.ts`). |
| Kích thước founder | [`simulation_loop.rs`](../../src-tauri/src/core/simulation_loop.rs) | 3 đốt, mỗi đốt `length = 1.0`, `radius = 0.2`, `joint_anchor.x = 1.0` | Chuỗi 3 đốt trải dài ~`2,0–3,0` sim-units (`1,0%–1,5%` cạnh cả thế giới!). |
| Hệ số co thân khi vẽ | [`agentPose.ts`](../../src/components/Landscape/utils/agentPose.ts) (`CREATURE_BODY_SCALE`) | **`0.32`** | Thân vẽ ở `6 × 0,32 = 1,92` render-units / sim-unit, còn tâm vị trí vẫn đi ở `6` render-units / sim-unit. |
| Tốc độ & tầm quét | [`locomotion.rs`](../../src-tauri/src/core/locomotion.rs) | `PREDATOR_MAX_SPEED = 2.0`, `PREY_MAX_SPEED = 1.5` unit/s; `ARRIVAL_RADIUS = 0.3`; `GRASS_SCAN_RADIUS = 3` (`7×7` ô ≈ `5,47` units) | Con mồi chạy thẳng xuyên `200` units mất ~`133` sim-giây (~`8.000` tick ≈ `1,33` năm sim); thú săn mất ~`100` sim-giây (`1` năm sim). |
| World v2 (đang dựng) | [`anima-worldgen/src/lib.rs`](../../src-tauri/crates/anima-worldgen/src/lib.rs), [`2026-11-20-world-v2-design.md`](../ai/design/2026-11-20-world-v2-design.md) | `WORLD_EXTENT_M = 40_960.0` m, `ELEV_MIN_M = -1_500.0` m, `ELEV_MAX_M = +5_700.0` m, `4096²` render / `256²` sim | Vẫn map vào `[-100, +100]²`: `1 sim-unit = 204,8 m`, `1 ô sim = 160 m`, `1 ô render = 10 m`. |

### 1.2 Tại sao `CREATURE_BODY_SCALE = 0.32` gây lệch cảm giác thị giác?

Ghi chú trực tiếp trong [`agentPose.ts`](../../src/components/Landscape/utils/agentPose.ts) giải thích đúng nguyên nhân phải thu nhỏ sinh vật:
- Nếu vẽ thân theo đúng `simScale = 6`, một đốt dài `1,0` sim-unit sẽ thành `6` render-units (founder dài ~`12–18` render-units), to ngang hàng cây và bờ hồ trong `WorldShowcase`.
- Khi ép `CREATURE_BODY_SCALE = 0.32`, đường kính sinh vật trở nên vừa mắt bên cạnh cây cối và mặt hồ (thân vẽ nhỏ bằng `0,32×` so với kích thước hồ mô phỏng), nhưng **vị trí gốc vẫn giữ nguyên thang `6`**. Hệ quả là hai sinh vật đang đứng sát nhau trong vật lý (bán kính va chạm `r = 0,2` sim-unit) lại hiện trên màn hình **cách nhau ~4 lần bán kính thân**, gây cảm giác "đánh từ xa" hoặc sinh vật quá nhỏ so với khoảng trống xung quanh.

### 1.3 So sánh định lượng 3 cấu hình quy mô (`Đã kiểm` + `Suy luận`)

| Tiêu chí | Map v1 hiện tại (`Đã kiểm`) | World v2 `WV2-D` (`Đã kiểm`) | Đảo 1:1 hậu-Chặng 4 *(giả định `Suy luận`)* |
|---|---|---|---|
| Bề ngang vật lý | Không định nghĩa mét rõ (đóng vào `RENDER_SIZE = 1200`) | **`40.960 m`** (`40,96 km`) | **`4.096 – 8.192 m`** (`4,1 – 8,2 km`) |
| Tổng diện tích hộp | — | **`1.677,7 km²`** (đất ~`840–1.000 km²`) | **`16,8 – 67,1 km²`** (đất ~`8–35 km²`) |
| `MapBounds` (`sim-units`) | `200 × 200` | `200 × 200` | **`600 × 600` đến `800 × 800`** |
| Lưới mô phỏng (`ResourceField`) | `256²` (`0,78125` u/ô) | `256²` (`0,78125` u/ô) | **`512²`** (`1,17 – 1,56` u/ô) |
| Độ phân giải địa hình sim | — | `160 m / ô` | `8 – 16 m / ô` |
| Độ phân giải địa hình render (`4096²`) | `2048²` (`worldGen.ts`) | `10 m / ô` | **`1 – 2 m / ô`** |
| Quy đổi vật lý (`m / sim-unit`) | — | **`204,8 m / sim-unit`** | **`~5 – 10 m / sim-unit`** *(tuỳ tỉ lệ loài)* |
| Founder `~2,5` sim-units quy ra đất thật | ~`1,25%` bề ngang bản đồ | **`~512 m`** nếu 1:1 vị trí *(nên phải tách tỉ lệ `LD-1`)* | Tương ứng **`1` đơn vị thân = 1:1 địa hình** *(nếu 1 sim-unit ≈ `1 m`–`2 m`)* |
| Hệ số co thân (`CREATURE_BODY_SCALE`) | `0.32` (lệch va chạm `~4r`) | Tách tỉ lệ hoàn toàn (`LD-1`) | **`1.0`** (va chạm khớp hình vẽ 1:1) |
| Tác động lên Chặng 4 (`VIAB-1`, `E3`, `SEL2-P2`) | Baseline đang chạy | Giữ nguyên `sim_256` (`200×200`) → an toàn | **Phá vỡ cân bằng tốc độ, 16 hồ, tìm cỏ** |

### 1.4 Ràng buộc trực tiếp từ Chặng 4: Vì sao KHÔNG mở rộng trước khi đóng Chặng 4?

Theo [`STATE_OF_THE_PROJECT.md` §3.00](../planning/STATE_OF_THE_PROJECT.md#300-việc-kế-tiếp--cập-nhật-2026-11-17):
- Hàng loạt biến số sinh thái của Chặng 4 — từ việc chia lưới `16` hồ (S12–S15), `GRASS_SCAN_RADIUS = 3` (`7 × 7` ô trên `256²`), khoảng cách đi thẳng xuyên bản đồ ~`100–133` sim-giây, cho tới sức chứa `prey_cap` và đợt khảo sát khả năng sống sót **VIAB-1** (chuẩn bị đăng ký **E3b** và **SEL2-P2**) — đều đang hiệu chuẩn trực tiếp trên `MapBounds = 200 × 200` và `256²`.
- Nếu tăng `MapBounds` lên `3–4×` trong khi giữ nguyên tốc độ di chuyển (`1.5–2.0`) và nhịp mất nước/ngưỡng chết khát (nguồn gây chết 71–80% ở S9 khi hồ bố trí lệch), thời gian tìm hồ tăng tỉ lệ thuận với khoảng cách bình quân → gây chết khát hàng loạt và làm vô hiệu toàn bộ dữ liệu thăm dò từ S6–S15.
- Riêng trên **map v1 (`worldGen.ts`)**, mở rộng bây giờ là công sức bỏ đi vì crate Rust [`anima-worldgen`](../../src-tauri/crates/anima-worldgen/src/lib.rs) đã đi qua `WV2-1a` (`#193`) và `WV2-1b` (`#194`) để thay thế map v1.

---

## 2. Nghiên cứu Địa mạo & Tạo địa hình thủ tục (`Nguồn ngoài — bài báo`)

Các bài báo nền tảng về mô hình hoá tiến hoá cảnh quan (Landscape Evolution Models — LEM) và đồ hoạ địa hình giúp trả lời câu hỏi quan trọng: **Mạng sông, thung lũng và khí hậu cần kích thước vật lý tối thiểu bao nhiêu để vẫn "thật"?**

### 2.1 Tổng quan phương pháp và bộ giải cốt lõi

1. **Phân loại toàn diện mô hình hoá địa hình:**
   **Galin, Guérin, Peytavie, Cordonnier, Cani, Benes & Gain (2019)** — *"A Review of Digital Terrain Modeling"* (*Computer Graphics Forum*, DOI: [10.1111/cgf.13657](https://doi.org/10.1111/cgf.13657)) hệ thống hoá 3 nhánh tạo địa hình: (i) **thủ tục** (noise/fractal), (ii) **mô phỏng vật lý** (xói mòn thuỷ lực, nhiệt, băng hà, phong hoá), và (iii) **dựa trên dữ liệu mẫu/ML**; chỉ rõ sai lầm phổ biến của nhánh thuần noise là không tái tạo được sự phụ thuộc không gian của lưu vực thoát nước.
2. **Thuật toán cốt lõi cho địa hình quy mô lớn:**
   - **Braun & Willett (2013)** — *"A very efficient $O(n)$, implicit and parallel method to solve the stream power equation governing fluvial incision and landscape evolution"* (*Geomorphology*): đưa ra bộ giải ngầm tuyến tính $O(n)$ trên cây dòng chảy, giải phóng bước thời gian lớn khỏi giới hạn ổn định CFL cổ điển.
   - **Cordonnier, Braun, Cani, Benes, Galin, Peytavie & Guérin (2016)** — *"Large Scale Terrain Generation from Tectonic Uplift and Fluvial Erosion"* (*Computer Graphics Forum / Eurographics*, DOI: [10.1111/cgf.12820](https://doi.org/10.1111/cgf.12820)): ứng dụng trực tiếp phương trình Stream Power kèm nâng kiến tạo vào đồ hoạ máy tính để sinh dãy núi và mạng sông tự nhất quán trên vùng rộng lớn. Đây là nền tảng lý thuyết trực tiếp của `WV2-1b` trong Anima.
   - **Barnes, Lehman & Mulla (2014)** — *"Priority-Flood: An Optimal Depression-Filling and Watershed-Labeling Algorithm for Digital Elevation Models"* (*Computers & Geosciences*, DOI: [10.1016/j.cageo.2013.04.024](https://doi.org/10.1016/j.cageo.2013.04.024)): khử hố trũng giả và gán nhãn lưu vực tối ưu ở $O(N \log N)$ cho lưới DEM thực (đã cài trong `anima-worldgen/src/hydrology.rs`).

### 2.2 Ngưỡng quy mô tối thiểu của địa mạo thật vs. đa đai khí hậu

Một nhầm lẫn phổ biến là nghĩ rằng phải có bản đồ `40 km` thì sông ngòi mới ra hình cành cây thật. Bằng chứng địa mạo học cho thấy sự tách biệt rõ rệt giữa **quy mô thuỷ văn** và **quy mô khí hậu–cao độ**:

- **Khởi phát kênh dòng & Định luật Hack:**
  **Montgomery & Dietrich (1988, 1989, 1992)** — *Nature* (DOI: [10.1038/336232a0](https://doi.org/10.1038/336232a0)), *Water Resources Research* (DOI: [10.1029/WR025i008p01907](https://doi.org/10.1029/WR025i008p01907)), và *Science* (DOI: [10.1126/science.255.5046.826](https://doi.org/10.1126/science.255.5046.826)) chứng minh dòng chảy chỉ khởi phát thành kênh xói mòn khi tích giữa diện tích lưu vực hứng nước $A$ và độ dốc cục bộ $S$ vượt ngưỡng tới hạn:
  $$A S^n \ge C$$
  Đồng thời, kết hợp **Định luật Hack** ($L = C_h \cdot A^h$, với $h \approx 0,6$), tính chất phân nhánh hình cành cây (dendritic) giữ nguyên dạng hình học từ các khe suối đầu nguồn dài vài trăm mét cho tới lưu vực sông lớn hàng trăm km.
- **Hệ quả cho quy mô đảo của Anima (`Suy luận`):**
  1. **Mạng sông & thung lũng V/U:** Một hòn đảo đường kính chỉ **`4–8 km`** (với lưới `4096²` đạt **`1–2 m/ô`**) vẫn đủ không gian để đạt ngưỡng khởi phát dòng và tạo ra các lưu vực `2–15 km²` có đầy đủ sông nhánh bậc Strahler 3–4, thác ghềnh và bãi bồi — thậm chí rõ chi tiết bờ suối hơn bản `40,96 km` (`10 m/ô`) khi camera zoom sát sinh vật.
  2. **Rào cản nằm ở dải cao độ & chuyển tiếp 22 biome:** Nếu cố nhồi đỉnh núi `5.700 m` (như đỉnh Sierra Nevada de Santa Marta của `WV2-D` để chạm đường tuyết nhiệt đới `ELA ~4.900 m`) vào một hòn đảo chỉ rộng `4 km` (bán kính sườn ~`1,5–2 km`), độ dốc trung bình phải lên tới $\approx 300\%$ ($> 70^\circ$) — cả hòn đảo biến thành một **ngọn tháp gai** phi tự nhiên và không còn đất bằng cho đồng bằng, đầm lầy hay sa mạc.
  3. **Kết luận thiết kế địa mạo:**
     - Muốn **1 đảo duy nhất có đủ đai từ rừng mưa nhiệt đới tới sông băng vĩnh cửu (`5.700 m`)**: chiều rộng `~40 km` của `WV2-D` là hợp lý về mặt vật lý.
     - Muốn **đảo nhỏ `4–8 km` để zoom sát sinh vật 1:1**: phải bỏ yêu cầu nhồi cả tuyết nhiệt đới `5.700 m` vào một khối đơn lẻ — thay bằng (i) nâng vĩ độ mô phỏng lên **ôn đới / cận Bắc cực** (nơi đường tuyết hạ xuống `1.200–2.000 m`), hoặc (ii) dùng **cụm đảo / chọn lát cắt sinh cảnh** (mỗi đảo một vùng khí hậu đại diện).

### 2.3 Các công trình bổ trợ chất lượng ở cự ly gần và hang động

Khi camera đứng sát sinh vật, heightmap `10 m/ô` bộc lộ giới hạn độ chi tiết bề mặt. Bốn bài báo mới giải quyết trực tiếp phần này:

- **Paris, Galin, Peytavie, Guérin & Gain (2019)** — *"Terrain Amplification with Implicit 3D Features"* (*ACM Transactions on Graphics / SIGGRAPH Asia*, DOI: [10.1145/3342765](https://doi.org/10.1145/3342765)): kết hợp heightmap 2.5D với trường hàm ẩn 3D (implicit / SDF cục bộ) để bổ sung vách đá nhô (overhang), vòm đá và hang hốc ngay trên khung địa hình sẵn có. Phù hợp trực tiếp với bước mở rộng hang đi vào được ở `QD-REALMS` sau này.
- **Schott, Paris, Fournier, Guérin & Galin (2023)** — *"Large-scale Terrain Authoring through Interactive Erosion Simulation"* (*ACM Transactions on Graphics / SIGGRAPH*, DOI: [10.1145/3592403](https://doi.org/10.1145/3592403)): dịch chuyển thao tác chỉnh sửa từ độ cao sang **miền nâng kiến tạo (uplift domain)** kèm ước lượng diện tích thoát nước nhanh, cho phép điêu khắc/điều chỉnh đảo mà không làm gãy mạng sông.
- **Cordonnier, Jouvet et al. (2023)** — *"Forming Terrains by Glacial Erosion"* (*ACM Transactions on Graphics / SIGGRAPH*, DOI: [10.1145/3592422](https://doi.org/10.1145/3592422)): bổ sung địa mạo băng hà (thung lũng chữ U, hốc cirque, hồ băng) cho phần đỉnh cao trên `ELA`.
- **Grenier, Sauvage, Guérin, Galin & Rohmer (2024)** — *"Real-time Terrain Enhancement with Controlled Procedural Patterns"* (*Computer Graphics Forum*, DOI: [10.1111/cgf.14992](https://doi.org/10.1111/cgf.14992)): tổng hợp hoa văn vi mô biến thiên theo độ dốc/hướng chảy (rãnh nhỏ, sỏi, vân đá) ngay trong shader thời gian thực mà không cần tăng độ phân giải lưu trữ của heightmap — cực kỳ quan trọng nếu giữ thế giới rộng mà vẫn cần mặt đất nhìn rõ nét dưới chân sinh vật nhỏ.

---

## 3. Sinh thái học Không gian, Tỉ lệ Quần thể & So sánh Nền tảng ALife

### 3.1 Ngưỡng kích thước không gian tối thiểu để tránh tuyệt chủng đồng loạt (`Nguồn ngoài — bài báo`)

Bài toán *"hòn đảo hơi bé"* không chỉ là vấn đề thẩm mỹ 3D mà chạm thẳng vào **tính ổn định động học quần thể**:

- **Huffaker (1958)** — *"Experimental studies on predation: Dispersion factors and predator-prey oscillations"* (*Hilgardia*, DOI: [10.3733/hilg.v27n14p343](https://doi.org/10.3733/hilg.v27n14p343)): thí nghiệm kinh điển trên nhện ăn thực vật và nhện săn mồi chứng minh rằng trong một không gian nhỏ/đồng nhất, thú săn quét sạch con mồi rồi cả hai cùng tuyệt chủng sau vài chu kỳ bùng nổ–sụp đổ. Chỉ khi môi trường được chia thành **nhiều mảng cách ly một phần với tốc độ phát tán hữu hạn**, con mồi mới có "lối thoát không gian" (spatial refuges) để hồi phục ở mảng khác trong khi mảng cũ sụp đổ.
- **Durrett & Levin (1994)** — *"The importance of being discrete (and spatial)"* (*Theoretical Population Biology*, DOI: [10.1006/tpbi.1994.1032](https://doi.org/10.1006/tpbi.1994.1032)): chứng minh toán học trên hệ hạt tương tác rời rạc rằng:
  - Khi kích thước miền không gian nằm **dưới ngưỡng tới hạn** (hoặc tầm di chuyển/trộn lẫn quá nhanh so với kích thước miền), các mảng bị đồng bộ pha → toàn bộ thế giới sụp đổ giống một cái bình trộn đều duy nhất.
  - Khi miền đủ lớn để nhiều chu kỳ cục bộ lệch pha nhau (tạo sóng đuổi bắt và vùng khảm động), cùng tồn tại thú săn–con mồi trở nên bền vững.
- **Liên hệ tới thất bại smoke SEL2-P và E3-X (`Suy luận`):** Trong nhật ký Chặng 4 ([`STATE_OF_THE_PROJECT.md` §3.00](../planning/STATE_OF_THE_PROJECT.md#300-việc-kế-tiếp--cập-nhật-2026-11-17)), khi `MapBounds = 200 × 200` còn tốc độ di chuyển `1.5–2.0` unit/s (tức một cá thể chạy hết chiều ngang thế giới chỉ trong `~1` năm sim!), khả năng cô lập mảng tự nhiên tương đối yếu: nếu thiếu `live.grass_steering` hoặc cấu trúc hồ tốt, quần thể dễ bùng nổ đồng loạt rồi sụp sạch (hình thái Huffaker dưới ngưỡng miền tối thiểu).

### 3.2 Quy luật ngoại suy vùng sống theo khối lượng cơ thể & Quy luật đảo (`Nguồn ngoài — bài báo`)

- **Allometric Home-Range Scaling:**
  - **Kelt & Van Vuren (2001)** — *"The ecology and macroecology of mammalian home range area"* (*The American Naturalist*, DOI: [10.1086/320621](https://doi.org/10.1086/320621)).
  - **Jetz, Carbone, Fulford & Brown (2004)** — *"The scaling of animal space use"* (*Science*, DOI: [10.1126/science.1102138](https://doi.org/10.1126/science.1102138)).
  - **Tamburello, Côté & Dulvy (2015)** — *"Energy and the scaling of animal space use"* (*The American Naturalist*, DOI: [10.1086/682070](https://doi.org/10.1086/682070)).
  - **Kết quả chung:** Diện tích vùng sống $H$ tăng siêu tuyến tính hoặc gần tuyến tính theo khối lượng cơ thể $M$ ($H \propto M^{\beta}$, với $\beta \approx 1,0\text{–}1,36$ tuỳ lớp động vật và cơ chế kiếm ăn), và **thú ăn thịt cần vùng sống rộng hơn thú ăn cỏ cùng khối lượng nhiều lần** do tổn thất năng lượng qua bậc dinh dưỡng (~10–30%).
  - **Hệ quả cho Anima (`Suy luận`):** Nếu muốn thử nghiệm **tiến hoá kích thước cơ thể trên dải rộng** (từ sinh vật nhỏ tới megafauna) và **chuỗi thú săn–con mồi dài hạn**, diện tích vùng sống của vài chục thú săn cỡ lớn đòi hỏi tỉ số `(diện tích đất sống) / (diện tích thân × tầm kiếm ăn)` phải lớn hơn đáng kể so với `200 × 200` hiện tại — nếu không, mọi cá thể cỡ lớn sẽ chồng vùng kiếm ăn lên nhau.
- **Quy luật đảo (Island Rule):**
  - **Benítez-López et al. (2021)** — *"The island rule explains consistent patterns of body size evolution in terrestrial vertebrates"* (*Nature Ecology & Evolution*, DOI: [10.1038/s41559-021-01426-y](https://doi.org/10.1038/s41559-021-01426-y)) khẳng định trên quy mô toàn cầu rằng đảo nhỏ làm thú lớn thu nhỏ (dwarfism) và thú nhỏ to lên (gigantism), chịu chi phối bởi cả **diện tích đảo**, **độ cô lập** và **khí hậu**.
  - Lưu ý ranh giới thiết kế của Anima: đề xuất [`2026-11-19-feature-island-evolution-observatory.md`](../ai/requirements/2026-11-19-feature-island-evolution-observatory.md) (`QD-ISLAND`) định nghĩa *"kích thước đảo = sức chứa quần thể ($K$)"* chứ không bắt buộc thay đổi diện tích hình học ngay, nhưng về dài hạn việc mở rộng `MapBounds` giúp kiểm chứng thêm hiệu ứng **phân mảnh không gian thật** tách biệt khỏi hiệu ứng sức chứa $K$ thuần tuý.

### 3.3 Bẫy ngược chiều: "Lời nguyền pha loãng" (Curse of Dilution) & đối chiếu 6 nền tảng ALife

Mở rộng bản đồ không đồng nghĩa với mở rộng không điều kiện. Trong các hệ sinh vật nhân tạo, nếu tăng diện tích gấp $9\times$–$16\times$ mà giữ nguyên số founder ban đầu (`10–80`) và tốc độ di chuyển/tầm nhìn, xác suất hai cá thể gặp nhau giảm mạnh → **không giao phối, không săn mồi, áp lực chọn lọc sụp đổ về nhiễu chết khát dọc đường**.

Cách các hệ ALife mã nguồn mở xử lý đánh đổi **kích thước thế giới ↔ kích thước cá thể ↔ mật độ quần thể** (`Nguồn ngoài — mã nguồn mở` + `Đã kiểm`):

| Nền tảng | Quy mô không gian & tỉ lệ cá thể | Cách giải quyết xung đột quy mô | Bài học cho Anima |
|---|---|---|---|
| **Polyworld** (Yaeger) | Đấu trường 2D/3D khép kín cỡ vừa | Giữ thế giới nhỏ vừa đủ cho tương tác dày đặc, chạy đa giác đơn giản trên CPU | Xác nhận đấu trường bé chỉ hợp quần thể nhỏ; muốn thế giới lớn phải giảm chi phí mỗi cá thể |
| **The Bibites** | Mặt phẳng 2D nhỏ–trung bình (nhiều đảo thức ăn/biome cục bộ) | Tách nhiều túi tài nguyên, giữ kích thước tương đối giữa bibite và viên thức ăn vừa mắt; cho phép thay đổi kích thước theo gene | Không cần bản đồ 40 km để có sinh thái sâu; chỉ cần **tỉ lệ `vùng kiếm ăn / đường kính bản đồ` hợp lý** |
| **Framsticks** | Đấu trường 3D nhỏ, sinh vật nhiều đốt/khớp (rất gần cấu trúc Anima) | Hy sinh số lượng cá thể & độ rộng địa hình để giữ vật lý khớp chính xác | Cho thấy gánh nặng `O(segments)` nếu không có Simulation-LOD |
| **ALIEN** (CUDA) | Hàng triệu hạt mềm trên lưới lớn | Đẩy toàn bộ vật lý hạt + thần kinh tế bào lên compute shader GPU | Cho thấy ngưỡng chi phí CPU khi mô phỏng dày đặc |
| **Neural MMO** | Lưới ô rộng lớn (`128²`–`1024²`), hàng nghìn agent | Bỏ vật lý khớp, rời rạc hoá tương tác theo ô lưới để giữ chi phí gần tuyến tính | Anima giữ vật lý đốt khớp nên không thể rời rạc hoá hoàn toàn, phải dựa vào **LOD + `aggregate_population`** |
| **Anima Engine** (`Đã kiểm`) | `200 × 200` sim (`256²`), founder `~2,5` units, `CREATURE_BODY_SCALE = 0.32`, mục tiêu `1.000`–`46.500+` agent | Đã có `Simulation-LOD` (HOT/WARM/COLD) + `aggregate_population` (chunk `32×32`) + `REGROWTH_STRIDE = 4` | Cần tránh tăng `MapBounds` mà không tăng **mật độ quần thể khởi điểm** hoặc **tốc độ/tầm cảm biến** tương ứng |

---

## 4. Nén không gian trong Game Thế giới mở & Kiến trúc Render Thế giới lớn

### 4.1 Đối chiếu quy mô bản đồ thực tế trong ngành game (`Nguồn ngoài — ngành game`, chưa xác minh)

> Diện tích và thời gian di chuyển của các game dưới đây là số ước lượng phổ biến trong cộng đồng người chơi, **không có nguồn trích dẫn** trong tài liệu này và chưa được đo lại. Chỉ dùng để so bậc độ lớn, không dùng làm thông số thiết kế.

Một so sánh trực tiếp cho thấy nguyên nhân gốc của cảm giác lệch tỉ lệ: **World v2 (`40,96 km × 40,96 km = 1.677,7 km²`) đang lớn hơn tới ~20–40 lần hầu hết các game thế giới mở AAA có tỉ lệ nhân vật 1:1**:

| Tựa game / Hệ thống | Diện tích bản đồ ước tính | Tỉ lệ nhân vật ↔ địa hình | Thời gian đi bộ / di chuyển xuyên map |
|---|---|---|---|
| **Subnautica** | ~`2–4 km²` (lưới ~`2 × 2 km`, sâu `1,5–2 km`) | 1:1 (người bơi dưới biển) | `10–15 phút` bơi; `2–3 phút` bằng tàu ngầm |
| **The Elder Scrolls V: Skyrim** | ~`37–39 km²` đất chơi | 1:1 (nén địa lý đậm đặc) | `25–35 phút` đi bộ chéo bản đồ |
| **Zelda: Breath of the Wild** | ~`60–80 km²` | 1:1 (tạo độ thoáng nhờ núi & tầm nhìn) | `45–60 phút` đi bộ; `~20 phút` cưỡi ngựa |
| **Red Dead Redemption 2** | ~`45–50 km²` đất (~`75–80 km²` tổng) | 1:1 (chuyển động chân thực) | `1,5–2 giờ` đi bộ; `25–30 phút` phi ngựa |
| **Valheim** | ~`300+ km²` (quần đảo đường kính ~`20 km`) | 1:1 (chia nhỏ thành từng đảo biome `1–4 km`) | `15–30 phút` chạy hết 1 đảo; đi thuyền vượt biển giữa các đảo |
| **Anima World v2 (`WV2-D`)** *(đã kiểm)* | **`1.677,7 km²`** (`40,96 × 40,96 km`, ~`840–1.000 km²` đất) | **Cách điệu (`LD-1`)** vì `1 sim-unit = 204,8 m` | Nếu đi ở tốc độ `1,5` u/s (để 1:1 thì phải mất hàng chục giờ người thật); ở tỉ lệ sim là `133` sim-giây |

**Bài học từ bảng đối chiếu (`Suy luận`):**
- Ngay cả những thế giới cảm giác khổng lồ như *Skyrim* (`38 km²`) hay *Subnautica* (`4 km²`, dùng chiều dọc + sương mù để tạo cảm giác bao la) cũng nhỏ hơn rất nhiều so với `1.677,7 km²`.
- Thiết kế **Valheim** gợi ý một mẫu hình rất hợp với Anima: thay vì một siêu lục địa `40 km` nơi nhân vật bị nuốt chửng nếu vẽ 1:1, chia thế giới thành **từng đảo kích thước vừa (`1–5 km` mỗi đảo)** giúp người quan sát nhìn rõ từng cá thể đi giữa rừng cây và sông suối mà vẫn có đa dạng biome qua các đảo khác nhau.

### 4.2 Bốn kỹ thuật nén không gian thường gặp trong thiết kế thế giới mở (`Suy luận`, chưa có nguồn)

> Đây là tổng hợp thực hành thiết kế thường được nhắc tới, không phải một "chuẩn GDC" có văn bản; tài liệu này không trích được bài nói hay bài báo cụ thể nào cho bốn mục dưới đây.

Để một bản đồ kích thước hợp lý (cỡ `4–16 km²` hoặc `40 km` có 2 tầng quan sát) cho cảm giác vừa rộng lớn vừa rõ sinh vật khi zoom gần:

1. **Neo công thái học 1:1 quanh sinh vật (Ergonomic Anchoring):** Mọi vật thể lân cận — gốc cây, bụi cỏ, hòn đá, bờ suối, tầm đánh/gặm — bắt buộc phải đo theo kích thước thân sinh vật (`CREATURE_BODY_SCALE = 1.0`), tuyệt đối không thu nhỏ thân vật để nhét vừa cây trang trí như giải pháp tạm ở B1.
2. **Phóng đại chiều dọc cục bộ (Localized Vertical Exaggeration):** Phóng đại độ cao núi ở rìa chân trời hoặc khối núi nền để tạo đường nét hùng vĩ, nhưng giữ độ dốc vùng đi lại ở tỉ lệ ~1:1 để tránh làm sai dáng đi khớp đốt và lỗi tràn bờ nước.
3. **Che khuất đường chân trời (Horizon Occlusion) + Tán xạ khí quyển:** Dùng đồi thấp, rừng rậm và sương mù theo khoảng cách để tách biệt trực quan các sinh cảnh mà không cần hàng km đất trống vô nghĩa giữa chúng.
4. **Rào chắn địa hình cho chuyển tiếp biome:** Thay vì chuyển thẳng từ sa mạc sang đồng cỏ trong 50 m, dùng dãy núi chắn mưa (orographic rain shadow — vốn có trong `WV2-2`), hẻm núi hoặc sông lớn làm ranh giới tự nhiên để hợp lý hoá chuyển tiếp sinh thái.

### 4.3 Ba trụ cột kỹ thuật render thế giới lớn mượt trên iGPU (`Nguồn ngoài — bài báo` + `Suy luận`)

Khi kích thước thế giới thực lên tới vài km hoặc `40,96 km` mà camera lại có thể zoom xuống sát một sinh vật dài `0,5–2 m`, renderer (cả WebGL fallback lẫn native `wgpu` tương lai) cần đúng 3 cơ chế kinh điển:

1. **Geometry Clipmaps** — **Losasso & Hoppe (2004)** (*ACM TOG / SIGGRAPH*, DOI: [10.1145/1015706.1015799](https://doi.org/10.1145/1015706.1015799)): các lưới vuông đồng tâm lồng nhau bám quanh camera, cập nhật theo vòng đệm toroidal, giữ chi phí đỉnh gần như không đổi bất kể tổng diện tích bản đồ (`4096²` hay `16384²`).
2. **CDLOD (Continuous Distance-Dependent LOD)** — **Strugar (2009)** (*Journal of Graphics, GPU, and Game Tools*, DOI: [10.1080/2151237X.2009.10129287](https://doi.org/10.1080/2151237X.2009.10129287)): cây tứ phân (quadtree) chọn mức chi tiết liên tục theo khoảng cách 3D và nội suy vị trí đỉnh trong vertex shader (morphing), triệt tiêu hiện tượng nứt mép tile và giật LOD khi bay sát mặt đất.
3. **Floating Origin / Camera-Relative Rendering** — **Thorne (2005)** (*"Using a Floating Origin to Improve Fidelity and Performance of Large Distributed Virtual Worlds"*, *4th Int. Conf. on Cyberworlds*):
   - **Vì sao bắt buộc cho Anima nếu tăng map + zoom sát sinh vật (`Suy luận`):** ở hệ toạ độ `f32`, sai số làm tròn khoảng $\sim 10^{-7} \cdot |x|$. Khi toạ độ thế giới lớn (vài km đến `40 km` theo đơn vị mét) mà sinh vật hoặc chi tiết mặt đất lại ở cỡ milimet–centimet, camera zoom sát sẽ xuất hiện **rung lắc đỉnh (vertex jitter), z-fighting bóng đổ và nứt khớp xương**.
   - **Giải pháp chuẩn:** lưu vị trí mô phỏng/camera bằng `f64` (hoặc `(tile_coord, local_f32)`), trước khi đẩy ma trận xuống GPU (`wgpu` / Three.js) thì **dời gốc về đúng vị trí camera (`p_gpu = (p_world - p_camera) as f32`)**. Mọi vật thể quanh mắt người xem luôn nằm gần `(0, 0, 0)` với độ chính xác `f32` tối đa.

---

## 5. Khảo sát 10 Kho Mã nguồn mở Liên quan đến Tạo Map & Render Thế giới lớn

Tất cả URL, ngôn ngữ và giấy phép dưới đây được kiểm tra từ nguồn upstream ngày **`2026-11-22`**. Bảng là đánh giá kỹ thuật; mọi quyết định đưa bất kỳ dependency nào vào `Cargo.toml` hoặc `package.json` phải tuân thủ quy trình rà soát pháp lý tại [`OPEN_SOURCE_POLICY.md`](../governance/OPEN_SOURCE_POLICY.md).

| # | Dự án & Upstream | Ngôn ngữ | Giấy phép *(kiểm 2026-11-22)* | Điểm mạnh / Cơ chế đáng học | Khuyến nghị cho Anima |
|---|---|---|---|---|---|
| 1 | [`fastscape-lem/fastscape`](https://github.com/fastscape-lem/fastscape) & [`fastscapelib`](https://github.com/fastscape-lem/fastscapelib) | Python / C++ / Fortran | `BSD-3-Clause` (`fastscape`) · `GPL-3.0` (`fastscapelib`) | Chuẩn tham chiếu cài đặt Fastscape SPL $O(n)$, xói mòn–bồi tụ, nâng kiến tạo và dòng chảy | **Oracle / Reference.** Không link `fastscapelib` (`GPL-3.0`) vào Anima (`MIT OR Apache-2.0`); chỉ dùng đối chiếu toán học cho `anima-worldgen`. |
| 2 | [`landlab/landlab`](https://github.com/landlab/landlab) | Python | `MIT` | Bộ công cụ LEM chuẩn earth-science (thuỷ văn, trượt đất, khởi phát kênh, Strahler/Hack) | **Oracle (đã duyệt OSS-021, xem [`OPEN_SOURCE_LANDSCAPE.md` §3.1](OPEN_SOURCE_LANDSCAPE.md#31-landlab-vì-sao-oracle-chứ-không-phải-dependency-chi-tiết-2026-07-26)).** Bổ sung vai trò mới: làm bộ test kiểm định ngoại tuyến cho cổng địa mạo `G-SHAPE` của `anima-worldgen`. |
| 3 | [`r-barnes/richdem`](https://github.com/r-barnes/richdem) | C++ / Python | `GPL-3.0` | Cài đặt chuẩn mực Priority-Flood, D8/D∞/MFD và định tuyến lưu vực hiệu năng cao | **Oracle / Reference.** Không link/chép mã do `GPL-3.0`; dùng để kiểm chéo tính đúng đắn của `hydrology.rs` (tích luỹ dòng chảy, diện tích lưu vực). |
| 4 | [`erosiv/geotransport`](https://github.com/erosiv/geotransport) & [`erosiv/soillib`](https://github.com/erosiv/soillib) | C++ / CUDA | `LGPL-3.0` (`soillib`) | Mô phỏng vận chuyển địa mạo ngẫu nhiên (Nick McDonald & Guillaume Cordonnier), cấu trúc lớp đất/trầm tích | **Reference.** Tham khảo kiến trúc tầng đất–đá–trầm tích và chuyển pha lắng đọng; không ghép CUDA runtime vào Anima. |
| 5 | [`open-terra/terra`](https://github.com/open-terra/terra) | C++ | `GPL-3.0` | Triển khai tường minh bài báo Cordonnier et al. (2016) từ nâng kiến tạo tới xói mòn | **Reference.** Tham khảo cấu trúc tham số & kiểm chứng định tính; không dùng mã nguồn do `GPL-3.0`. |
| 6 | [`redblobgames/mapgen4`](https://github.com/redblobgames/mapgen4) | JS / WebGL | `Apache-2.0` | Sinh đảo trên lưới Voronoi/Delaunay, mô phỏng gió–mưa địa hình, lưu vực sông đa giác | **Reference / Tương thích license.** Ý tưởng hay về cách bố trí hình dáng đảo & sông chảy mượt không phụ thuộc lưới vuông (hữu ích nếu làm trình chỉnh sửa đảo). |
| 7 | [`Azgaar/Fantasy-Map-Generator`](https://github.com/Azgaar/Fantasy-Map-Generator) | JavaScript | `MIT` | Sinh quần đảo, khí hậu theo đới gió, tách ô điều tra (per-cell inspector) cực trực quan | **Reference.** Đáng học cách thiết kế UX thanh tra dữ liệu ô bản đồ (khí hậu, lượng mưa, biome, sông) phục vụ gỡ lỗi sinh thái. |
| 8 | [`veloren/veloren`](https://gitlab.com/veloren/veloren) | Rust | `GPL-3.0-or-later` | Pipeline worldgen Rust hoàn chỉnh ở tỉ lệ nhân vật 1:1 (kiến tạo/lục địa → lưu vực → biome → chunk streaming đa luồng) | **Reference kiến trúc (cấm chép mã vì GPLv3).** Hình mẫu thực tế bằng Rust chứng minh cách tổ chức dữ liệu thế giới phân tầng theo chunk mà không cần tính lại toàn bộ map lúc runtime. |
| 9 | [`kurtkuehnert/bevy_terrain`](https://github.com/kurtkuehnert/bevy_terrain) | Rust (`wgpu` / Bevy) | `MIT OR Apache-2.0` | Render địa hình cực lớn trực tiếp trên `wgpu`: quadtree/UDLOD driven trên GPU, morphing chống nứt, streaming heightmap thực | **Pilot ưu tiên cao cho Pha 2 (`NR-SPIKE` / ADR-0011).** Cùng giấy phép `MIT OR Apache-2.0` và cùng ngăn xếp Rust + `wgpu` / Bevy với Anima. Cần đọc kỹ cách cài đặt morphing + tile cache để thử nghiệm ở spike native renderer. |
| 10 | [`TokisanGames/Terrain3D`](https://github.com/TokisanGames/Terrain3D) | C++ (Godot 4 GDExtension) | `MIT` | Clipmap GPU nhẹ, splatmap ít lớp, pha trộn khoảng cách xa chống lặp texture | **Reference / Đối chứng Pha 2.** Rất gọn nhẹ ở shader vật liệu địa hình (học kỹ thuật giảm độ lặp texture & clipmap cho mục tiêu `30 FPS` trên Iris Xe); cũng là công cụ sẵn có nếu Pha 2 benchmark ứng viên Godot 4 theo kế hoạch Map v2 §3. |

---

## 6. Phân tích 3 Phương án Kiến trúc & Thời điểm cho Anima Engine (`Suy luận`)

### 6.1 Kết luận về thời điểm: Không mở rộng sớm trên v1 hoặc giữa Chặng 4

| Câu hỏi thời điểm | Khuyến nghị | Lý do kỹ thuật & bằng chứng |
|---|---|---|
| Mở rộng map v1 (`worldGen.ts`)? | **KHÔNG** | Map v1 sắp bị thay thế bởi `anima-worldgen` (`WV2-1a..WV2-7`). Mọi chỉnh sửa v1 đều tạo nợ kép và có thể làm lệch cổng `COMPOSITION_BANDS` (`mapEvidence.test.ts`). |
| Tăng `MapBounds` lên `600–800` ngay bây giờ? | **KHÔNG** | Sẽ phá vỡ toàn bộ tham số đã hiệu chuẩn của Chặng 4 (`VIAB-1`, 16 hồ S12–S15, `GRASS_SCAN_RADIUS`, tốc độ di chuyển `1.5–2.0`). Cần đóng xong Chặng 4 (`E3b` / `SEL2-P2`) trước khi đụng `MapBounds`. |
| Mở rộng **phần tham số hoá thiết kế** ngay từ World v2? | **CÓ — NÊN LÀM SỚM** | Chi phí cực thấp nếu làm trước khi đóng băng định dạng `ANMW v3` (`WV2-3`) và viewer (`WV2-6`): chỉ cần không đóng cứng hằng số duy nhất `40.96 km` + `200×200` vào cấu trúc không đổi được. |

### 6.2 So sánh 3 phương án dài hạn cho tỉ lệ Map ↔ Sinh vật

| Trục so sánh | Phương án 1: Giữ nguyên `WV2-D` (`40,96 km`, `200×200`, tỉ lệ cách điệu) | Phương án 2: Đổi sang Đảo nhỏ 1:1 (`4–8 km`, `MapBounds = 600–800`, `512²`) | Phương án 3 *(Khuyến nghị)*: **Hai quy mô tham số hoá** (`40,96 km` đại cảnh + Preset Đảo 1:1 hậu-Chặng 4) |
|---|---|---|---|
| **Cảm giác nhìn gần** | Cách điệu kiểu RTS/sa bàn (`LD-1`); vẫn phải giữ hệ số co thân hoặc tách xa kích thước đất/thú | **Xuất sắc:** sinh vật, cây cối, bờ suối và va chạm khớp **1:1** (`CREATURE_BODY_SCALE = 1.0`) | Đạt **cả hai**: có đại cảnh `40,96 km` đầy đủ biome và chế độ/bản đồ Đảo `1:1` khi zoom sát |
| **Địa mạo & 22 biome** | Rất tự nhiên (`5.700 m` núi tuyết + nhiệt đới chân thực) | Không đủ chỗ cho `5.700 m` tuyết nhiệt đới nếu là 1 đảo đơn (phải đổi sang khí hậu ôn đới hoặc nhiều đảo) | Thế giới đại cảnh giữ nguyên `22` biome SNSM; preset Đảo 1:1 chuyên sâu từng vùng sinh cảnh / quần đảo |
| **Sinh thái không gian (Huffaker)** | Giữ nguyên cân bằng Chặng 4 trên `256²` (`200×200`) | Diện tích tương đối tăng `9–16×` giúp giảm sụp đổ đồng loạt *nhưng* phải hiệu chuẩn lại toàn bộ tốc độ, hồ và mật độ founder; chi phí lưới `512²` gấp `4×` | **Chặng 4 hoàn toàn an toàn** trên preset `200×200`; preset mở rộng tách riêng ở pha sau với manifest và hiệu chuẩn riêng |
| **Yêu cầu renderer** | Tile streaming + impostors + GPU culling | Bắt buộc **Floating Origin** + **Tile Streaming** + **micro-detail shader** (Grenier 2024) | Tận dụng chung hạ tầng tile + `Floating Origin` trong Pha 2 (`NR-SPIKE`) |
| **Rủi ro tiến độ hiện tại** | Thấp nhất | Cao nếu áp dụng trước khi xong Chặng 4 | Thấp: không chặn `WV2-2..WV2-7` hay `VIAB-1` |

---

## 7. Phụ lục tính toán A — Cân chỉnh thông số sinh thái & hiệu năng khi mở rộng `200×200 (256²)` → `600–800 (512²)` (`Đã kiểm` + `Suy luận`)

Mục này tính toán định lượng các thông số sẽ bị xô lệch nếu sau Chặng 4 dự án bật một preset bản đồ mở rộng (`600 × 600` hoặc `800 × 800` sim-units trên lưới `512²`, `CREATURE_BODY_SCALE = 1.0`). Mọi hằng số nền bên trái đều đọc trực tiếp từ mã nguồn (`Đã kiểm` ngày `2026-11-22`).

### 7.1 Động học nước & quy luật mật độ hồ (tránh chết khát hàng loạt)

- **Các hằng số gốc (`Đã kiểm`):**
  - [`hrrl.rs`](../../src-tauri/src/ai/hrrl.rs): `HomeostaticState::default()` khởi tạo `energy = 100.0`, `hydration = 100.0`, `hydration_target = 100.0`.
  - [`locomotion.rs`](../../src-tauri/src/core/locomotion.rs): `PREY_MAX_SPEED = 1.5`, `PREDATOR_MAX_SPEED = 2.0`, `THIRST_STEERING_FRACTION = 0.5` (cá thể chỉ chuyển hướng về hồ gần nhất khi `hydration < 50.0`), `GRASS_SCAN_RADIUS = 3` (`7 × 7` ô).
  - [`world_systems.rs`](../../src-tauri/src/core/world_systems.rs): `k_velocity = 0.2`, `k_force = 0.3`; tốc độ mất nước mỗi bước là `decay_hyd = (0.05 + 0.15 * total_cost + sweat_rate) * dt`.
  - [`environmental_systems.rs`](../../src-tauri/src/core/environmental_systems.rs): cá thể uống nước từ các thực thể `Lake` khi chạm `SpatialCollider` (chưa uống trực tiếp từ ô `BiomeType::River`, dù [`terrain_navigation.rs`](../../src-tauri/src/core/terrain_navigation.rs) đã đánh dấu `River` là ô nước đi bộ được).
- **Hệ quả hình học khi mở rộng cạnh theo hệ số $k = L_{\text{new}} / 200$ (`Suy luận`):**
  - Trên chuẩn `200 × 200` với `16` hồ rải lưới `4 × 4` (S12–S15), khoảng cách tâm giữa hai hồ là `50` sim-units; khoảng cách cực đại từ một điểm bất kỳ tới hồ gần nhất là $25\sqrt{2} \approx 35,4$ sim-units (~`23,6` sim-giây di chuyển thẳng ở tốc độ tối đa `1,5` của con mồi).
  - Nếu **giữ nguyên `16` hồ** trên bản đồ `600 × 600` ($k = 3$): bước lưới hồ tăng lên `150` sim-units, khoảng cách cực đại tới hồ tăng thành **`106,1` sim-units** (~`71` sim-giây chạy thẳng, chưa kể đường vòng địa hình và mồ hôi do `total_cost`). Trên `800 × 800` ($k = 4$), con số này lên tới **`141,4` sim-units** (~`94` sim-giây) → cá thể rất dễ cạn nửa bình nước còn lại (`50.0`) trước khi tới bờ hồ.
  - **Hai cách xử lý bảo toàn bất biến khoảng cách nước:**
    1. *Giữ nguyên cơ chế `Lake` hiện tại:* tăng số điểm hồ tỉ lệ theo diện tích $N_{\text{lake, new}} \approx k^2 \cdot N_{\text{lake, base}}$ (`~144` điểm hồ trên `600×600`, `~256` điểm trên `800×800`).
    2. *Giải pháp tự nhiên hơn trên World v2 (`Suy luận`):* cho phép uống trực tiếp tại các ô `BiomeType::River` (đã được nối liền 8 hướng từ sông Strahler $\omega \ge 4$ theo `WV2-D §3.4`), kết hợp mạng `16–36` hồ lớn ở lưu vực/miệng núi lửa. Như vậy động vật tự men theo mạng sông dendritic tự nhiên thay vì phụ thuộc vào ma trận điểm hồ nhân tạo.

### 7.2 Tổng sinh khối `ResourceField`, vùng quét cỏ và ngân sách CPU ở `512²`

- **Quy luật sinh khối hiện hành (`Đã kiểm`, [`ecology.rs`](../../src-tauri/src/core/ecology.rs)):**
  - Sức chứa mỗi ô tính bằng `biome_carrying_capacity(biome) = biome_npp(biome) * NPP_TO_CAPACITY` với `NPP_TO_CAPACITY = 0.01` (`Grassland = 7.0`, `TemperateForest = 12.0`, `Rainforest = 22.0` EU/ô). Chú ý: **công thức gắn vào số lượng ô chứ không nhân với diện tích mỗi ô**.
  - `ResourceField` dùng `REGROWTH_STRIDE = 4` (mỗi tick cập nhật `1/4` số ô bằng `step_regrowth_gated_strided`).
- **Hệ quả khi lên `512²` (`262.144` ô, tăng `4×` số ô) kết hợp `MapBounds = 600` ($k=3$, diện tích tăng `9×`) hoặc `800` ($k=4$, diện tích tăng `16×`) (`Suy luận`):**
  - Tổng sức chứa sinh khối cả bản đồ tăng **`4×`** theo số ô nếu giữ `NPP_TO_CAPACITY = 0.01`, nhưng mật độ sinh khối trên mỗi `sim-unit²` biến đổi theo hệ số $4 / k^2$: giảm `2,25×` ở `600×600` (mỗi ô rộng `1,172` u thay vì `0,781` u) và giảm `4×` ở `800×800` (mỗi ô rộng `1,5625` u). Điều này thực chất **hợp lý về mặt sinh thái**: mỗi ô cỏ không bị quá đậm đặc ("Paradox of Enrichment") khi kích thước ô tăng lên ngang cỡ nửa thân vật.
  - Bán kính cửa sổ tìm cỏ `GRASS_SCAN_RADIUS = 3` (`7 × 7` ô) tự động đổi từ `2,73` sim-units (`256² / 200`) sang **`4,10` sim-units** (`512² / 600`) hoặc **`5,47` sim-units** (`512² / 800`) — giữ nguyên quanh `1,5–2×` chiều dài thân founder (`~2,5–3` u), không cần viết lại thuật toán quét `7×7`.
  - **Chi phí bộ nhớ và tick (`Đã kiểm` + `Suy luận`):** Trong doc comment của [`ecology.rs`](../../src-tauri/src/core/ecology.rs) (ghi ngày `2026-11-17` trên Apple M5 — dẫn làm tham chiếu lịch sử, không phải phép đo mới của tài liệu này), `256²` với `REGROWTH_STRIDE = 4` xử lý `16.384` ô/tick với trung vị Criterion `0,039 ms`. Ở `512²`, giữ `REGROWTH_STRIDE = 4` sẽ xử lý `65.536` ô/tick (`4×`); nếu nâng `REGROWTH_STRIDE = 16` thì số ô xử lý mỗi tick quay về đúng **`16.384` ô/tick** (với `16·dt` cho mỗi ô tới lượt), giữ ngân sách tick gần tương đương `256² / 4` (cần đo Criterion xác nhận hành vi L2/L3 cache).

| Thông số | Chuẩn hiện tại (`200×200`, `256²`) | Đề xuất Đảo A (`600×600`, `512²`) | Đề xuất Đảo B (`800×800`, `512²`) |
|---|---|---|---|
| Diện tích sim (`sim-units²`) | `40.000` (`1×`) | `360.000` (`9×`) | `640.000` (`16×`) |
| Kích thước 1 ô sim | `0,78125` u (`0,61 u²`) | `1,172` u (`1,37 u²`) | `1,5625` u (`2,44 u²`) |
| Tỉ lệ `chiều dài founder (~2,5u) / ô` | ~`3,2` ô | ~`2,1` ô | ~`1,6` ô |
| RAM `ResourceField` (`r`, `r_max`: `2×f32`) | `512 KiB` | `2,0 MiB` | `2,0 MiB` |
| RAM `TerrainMap` (`4×f32 + 1×u8`) | ~`1,06 MiB` | ~`4,25 MiB` | ~`4,25 MiB` |
| Ô regrowth / tick (`stride 4` / `stride 16`) | `16.384` / `4.096` | `65.536` / `16.384` | `65.536` / `16.384` |
| Bán kính quét cỏ (`GRASS_SCAN_RADIUS = 3`) | `2,73` u | `4,10` u | `5,47` u |
| Nguồn nước tối thiểu giữ cùng quãng khát | `16` hồ (S12–S15) | ~`144` điểm nước **hoặc** uống ở `River` + `16–36` hồ | ~`256` điểm nước **hoặc** uống ở `River` + `25–49` hồ |
| Founder khởi điểm & chiến lược bố trí | `10–80` rải toàn đảo | `90–150` hoặc `3–5` cụm × `25` cá thể | `160–240` hoặc `4–6` cụm × `30` cá thể |

### 7.3 Chiến lược bố trí Founder tránh "lời nguyền pha loãng" (`Suy luận`)

- Nếu giữ `10` founder rồi rải đều trên `600 × 600`, diện tích trung bình mỗi con là `36.000 u²` (khoảng cách bình quân ~`190` sim-units) — quần thể gần như cô lập hoàn toàn và chết trước khi giao phối/săn mồi.
- **Hai cấu hình khởi tạo thay thế cho preset mở rộng:**
  1. **Nhân mật độ theo diện tích ($N_0 \propto k^2$):** tăng số founder lên `90–160` và `prey_cap` lên `800–1.500` (nằm trong vùng `1.000` agent đã đo đường cong scaling M3 ở [`STATE_OF_THE_PROJECT.md` §3.00](../planning/STATE_OF_THE_PROJECT.md#300-việc-kế-tiếp--cập-nhật-2026-11-17)).
  2. **Gieo theo cụm sinh thái (Multi-Core Founder Patches):** giữ số founder vừa phải (`~75–150` con) nhưng phân vào `3–5` cụm địa lý cách nhau `150–250` sim-units quanh các thung lũng có sông/hồ. Cách này vừa đảm bảo mật độ gặp gỡ, giao phối và săn mồi ngay từ tick 1, vừa tạo khoảng trống di cư giữa các cụm để kích hoạt đúng cơ chế lệch pha không gian của Huffaker (1958).

---

## 8. Phụ lục kiến trúc B — So sánh sâu `bevy_terrain` vs. `wgpu` tự viết vs. `Godot 4 + Terrain3D` cho Pha 2 (`NR-SPIKE` / ADR-0011)

Trong yêu cầu [`2026-11-20-feature-map-v2.md`](../ai/requirements/2026-11-20-feature-map-v2.md) §3 và §3b, dự án đặt mục tiêu đo **Pha 2 Native Renderer (`NR-SPIKE` / ADR-0011)** giữa 3 ngả với ngân sách khung hình nghiêm ngặt: **Iris Xe Low `33,3 ms`** (`Terrain 6 ms`, `Vegetation 6 ms`, `Water 3 ms`, `Creatures (1.000) 5 ms`, VRAM thường trú `~35–55 MiB`) và **RTX 5060 Ti High `16,6 ms`** (`Terrain 3 ms`).

### 8.1 Ràng buộc cứng của ngăn xếp Anima Engine (`Đã kiểm`)

1. **Bevy ECS hiện tại:** Anima dùng **`bevy_ecs` 0.13** độc lập làm bộ lập lịch mô phỏng, hoàn toàn không kéo `bevy_render` hay `bevy_pbr`.
2. **Ghim phiên bản `wgpu`:** Feature `ml-wgpu` (`burn 0.22.1`) ghim cứng **`wgpu = "30.0.2"`** trong `Cargo.lock` ([`2026-11-20-world-v2-design.md` §1.1](../ai/design/2026-11-20-world-v2-design.md#11-workspace-crate-src-tauricratesanima-worldgen--strict-g2-feature-contract)). Nếu kéo thêm crate dùng `wgpu` khác phiên bản, build sẽ biên dịch 2 cây `wgpu`/`naga` hoặc không chia sẻ được `Device`/`Queue` nếu cần sau này.
3. **Cổng tách feature G2:** `cargo tree -p anima-engine --no-default-features` cấm `wgpu`/`naga` rò vào bản build headless thí nghiệm.
4. **Cửa sổ Tauri v2:** Ứng dụng hiện dùng HTML/React UI (các panel Ecosystem, Evolution, Lineage, Chronicle, ResearchDashboard) nằm trong WebView của Tauri v2.

### 8.2 Phân tích chuyên sâu từng ứng viên (`Nguồn ngoài` + `Suy luận`)

#### Ứng viên A: Dùng trực tiếp `kurtkuehnert/bevy_terrain` (`MIT OR Apache-2.0`) cùng Bevy renderer
- **Điểm mạnh đã kiểm chứng:** Cài đặt rất hoàn chỉnh pipeline GPU-driven cho địa hình lớn — chia lưới **Quadtree** kết hợp **UDLOD** (Uniform Distance-Dependent LOD), **morphing trong vertex shader** chống giật/nứt mép, và **Virtual Texturing / Attachments** chỉ nạp page nhìn thấy vào VRAM.
- **Vướng mắc tích hợp lớn (`Suy luận`):**
  1. *Lệch phiên bản Bevy & rủi ro phá lõi:* `bevy_terrain` nhắm tới **Bevy 0.14** (dòng repo chính phát triển từ 0.9 lên 0.14; phiên bản Bevy mục tiêu hiện tại của nhánh chính chưa được kiểm lại khi rà soát tài liệu này), trong khi Anima đang ở `bevy_ecs 0.13`. Nâng toàn bộ mô phỏng lên Bevy `0.14+` và kéo `bevy_render` đồng thời chạm trực tiếp rủi ro đã cảnh báo ở [`MAP_AND_ML_UPGRADE_RESEARCH.md`](MAP_AND_ML_UPGRADE_RESEARCH.md) (các ràng buộc `Resource` trên `BrainModel`, thay đổi schedule/system, tăng mạnh thời gian build CI Windows — vốn đã mất ~31–95 phút khi build lạnh).
  2. *Xung đột `wgpu`:* Bevy `0.14` kéo phiên bản `wgpu` riêng của Bevy `0.14` (cũ hơn `wgpu 30.0.2` mà `burn 0.22.1` đang dùng trong `Cargo.lock`).
  3. *Lồng vòng lặp cửa sổ:* Nhúng nguyên Bevy app (`WinitPlugin` + `RenderPlugin`) vào cùng cửa sổ Tauri v2 phức tạp hơn nhiều so với tạo thẳng `wgpu::Surface` từ handle của Tauri window.
- **Kết luận về `bevy_terrain` (`Suy luận`):** Rất giá trị để **Pilot thuật toán và port phần nhân shader/UDLOD/Virtual Texturing** (hợp lệ theo `MIT OR Apache-2.0` kèm ghi nhận `NOTICE`), nhưng **không** nên phụ thuộc trực tiếp vào crate `bevy_terrain` như một Bevy plugin trọn gói.

#### Ứng viên B: Pipeline `wgpu 30.0.2` tự viết gọn nhẹ trong tiến trình Tauri (gợi ý chính cho `NR-SPIKE`)
- **Kiến trúc:** Một renderer module riêng (sau feature flag `native-renderer`, tách biệt hoàn toàn khỏi `--no-default-features` để giữ sạch G2) sử dụng chính **`wgpu = "30.0.2"`** sẵn có trong `Cargo.lock`, đọc thẳng snapshot/buffer tư thế đốt xương từ `bevy_ecs 0.13` trong bộ nhớ (bỏ 100% serialize JSON qua Tauri IPC mỗi tick).
- **Kỹ thuật tích hợp:** Học trực tiếp mô hình **Quadtree + UDLOD + vertex morphing** từ `bevy_terrain` và hệ **nướng tĩnh từ `WV2-D` §3.2** (chỉ 6 render pass nhỏ gọn: height/normal clipmap → terrain splat ≤ 4 lớp + `terrain_lighting` horizon AO → instanced vegetation + impostors → nước depth/shore SDF → skinned creatures + contact shadows → bầu trời LUT + upscale 720p→1080p cho Iris Xe).
- **Hỗ trợ Floating Origin ngay từ gốc:** Lưu toạ độ thế giới ở `f64` (hoặc tile index + offset) và trừ vị trí camera trước khi ghi uniform buffer `f32` — đảm bảo camera zoom sát sinh vật 1:1 ở mép bản đồ `40 km` hay `8 km` không bị rung đỉnh.
- **Ba bẫy kỹ thuật đặc thù của Tauri v2 + `wgpu` (`Nguồn ngoài — tài liệu kỹ thuật`):**
  1. *Bẫy WebView trong suốt trên Windows (`transparent: true`):* WebView2 khi bật `transparent: true` để lộ lớp `wgpu` vẽ phía dưới dễ gặp lỗi nền đen/trắng hoặc nhấp nháy z-fighting với compositor của trình duyệt; giải pháp dự phòng chuẩn trên Windows là tạo **child `HWND` riêng** qua `raw-window-handle` gắn swapchain `wgpu` và điều khiển thứ tự xếp chồng bằng Win32 `SetWindowPos`, đặt vùng HUD React ở panel nổi hoặc cửa sổ con đồng bộ.
  2. *Input pass-through (hit-testing):* Phải phân tách rõ vùng UI HTML nhận chuột (`pointer-events: auto` trên các thẻ HUD, `pointer-events: none` trên nền trống) hoặc lọc sự kiện chuột native để thao tác xoay/zoom camera 3D không bị WebView nuốt mất.
  3. *Đồng bộ Resize & DPI:* Bắt buộc lắng nghe `tauri::WindowEvent::Resized` và yếu tố tỉ lệ DPI để gọi `surface.configure(...)` khớp đúng kích thước vật lý từng khung hình, tránh kéo giãn pixel hoặc panic swapchain trên Vulkan/DX12/Metal.

#### Ứng viên C: Godot 4 + `TokisanGames/Terrain3D` (`MIT`) làm viewer tách tiến trình
- **Ưu điểm:** Có sẵn clipmap `Terrain3D`, sương mờ, shadow cascade, vật liệu PBR → thời gian dựng mẫu thử đạt chuẩn nhìn `G-REF` ngắn nhất.
- **Nhược điểm:** Phải duy trì giao thức đồng bộ trạng thái hàng nghìn đốt xương liên tiến trình (IPC/shared memory), tăng kích thước gói cài đặt (mang theo runtime Godot 4), và khó giữ nguyên hệ giao diện React/HTML sẵn có của Anima mà không phải viết lại HUD trong GDScript hoặc nhúng lại cầu web.

### 8.3 Ma trận so sánh nhanh & 5 phép đo bắt buộc của `NR-SPIKE` (`Suy luận`)

| Tiêu chí đánh giá | A. Kéo nguyên `bevy_terrain` (Bevy 0.14) | B. `wgpu 30.0.2` tự viết (port ý tưởng `bevy_terrain`) | C. Godot 4 + `Terrain3D` (tiến trình phụ) |
|---|---|---|---|
| Khớp `wgpu 30.0.2` & `bevy_ecs 0.13` | 🔴 Lệch Bevy `0.14` & `wgpu` khác bản | 🟢 Khớp 100% `Cargo.lock`, giữ `bevy_ecs 0.13` | 🟡 Tách binary hoàn toàn khỏi Rust crate |
| Kiểm soát ngân sách Iris Xe (`33,3 ms`) | 🟡 Nặng bộ khung `bevy_pbr` khó cắt gọt | 🟢 Tối ưu từng pass đúng bảng `§3b` | 🟢 `Terrain3D` clipmap rất tối ưu sẵn |
| Tích hợp HUD React / Tauri v2 | 🔴 Khó lồng Winit/Bevy với Tauri WebView | 🟡 Cần giải bài toán `HWND`/input Tauri v2 | 🔴 Tách HUD hoặc viết lại UI bên Godot |
| Độ trễ & chi phí truyền dữ liệu sinh vật | 🟢 Đọc bộ nhớ trực tiếp nếu chung app | 🟢 Zero-IPC: đọc thẳng slice/buffer bộ nhớ | 🔴 Phải stream bone matrix qua IPC / shm |
| Hỗ trợ Floating Origin (`f64` → `f32`) | 🟡 Tuỳ chỉnh camera plugin Bevy | 🟢 Tự kiểm soát ma trận MVP camera-relative | 🟡 Godot 4 có tùy chọn build `double` |
| Chi phí build CI Windows & kích thước cài | 🔴 Tăng rất mạnh (cả Bevy render stack) | 🟢 Nhỏ, thêm chủ yếu `wgpu` sẵn có | 🔴 Kèm binary Godot export template |

**5 phép đo bắt buộc trong `NR-SPIKE` (cả Iris Xe và RTX 5060 Ti) trước khi chốt `ADR-0011`:**
1. Đo khung hình thực (`GPU timestamp query`) từng pass Terrain + `1.000` sinh vật `SkinnedMesh`/instanced ở chế độ `Low` (720p upscale) trên Iris Xe và `High` trên RTX.
2. Kiểm chứng chuyển động zoom từ **toàn cảnh đảo → sát mặt đất cạnh sinh vật `1 m`** với Floating Origin (`0` jitter đỉnh, `0` nứt khớp ở biên `x = ±400` / `±20 km`).
3. Thử nghiệm ổn định Tauri v2 + `wgpu` surface trên cả **Windows 11 (WebView2 + DX12/Vulkan)** và **macOS (WKWebView + Metal)** ở các thao tác resize, DPI `100% / 150% / 200%`, và click xuyên HUD.
4. Đo đỉnh bộ nhớ VRAM thường trú của tile cache so với trần `~35–55 MiB` của mức Low (`WV2-D §3.2`).
5. Đo thời gian biên dịch CI lạnh trên `windows-latest` để bảo đảm không vượt ngưỡng `timeout-minutes: 120`.

---

## 9. Phụ lục định dạng C — Đề xuất mở rộng 12 byte cho header `ANMW v3` (`WV2-3`) để hỗ trợ đa quy mô (`Đã kiểm` + `Suy luận`)

Đối chiếu đặc tả nhị phân `44` byte của `sim_256.anmw3` trong [`2026-11-20-world-v2-design.md` §3.3.1](../ai/design/2026-11-20-world-v2-design.md#331-binary-header--circular-hash-fix-sim_256anmw3-fix-4--ld-2) (`Đã kiểm`):

- Các byte `8..12` (`width`), `12..16` (`height`) và `36..40` (`world_scale_bits = f32::to_bits(200.0)`) **đã là trường động trong header**, tức là riêng phần lưới sim (`256²` hay `512²`) và `MapBounds` (`200.0` hay `600.0`) đã không bị đóng cứng vào cấu trúc file.
- Ngược lại, **3 thông số vật lý** là `WORLD_EXTENT_M = 40_960.0`, `ELEV_MIN_M = -1_500.0`, `ELEV_MAX_M = +5_700.0` hiện đang nằm ở dạng hằng số biên dịch trong [`anima-worldgen/src/lib.rs`](../../src-tauri/crates/anima-worldgen/src/lib.rs), chưa được ghi vào header `ANMW v3`.

### Đề xuất bổ sung nhỏ trước khi `WV2-3` khoá định dạng `ANMW v3` (`Suy luận`)

Mở rộng header cố định từ `44` byte lên **`56` byte** (giữ nguyên hoàn toàn bố cục `0..44`, trong đó `20..24` vẫn là `sim_seed_u32` để `peek_seed` khớp offset v2, chỉ nối thêm 3 trường `f32` LE ở cuối header):

| Offset (byte) | Trường đề xuất | Kiểu | Giá trị cho `WV2-D` (đại cảnh) | Giá trị ví dụ cho Đảo 1:1 (ôn đới) |
|---|---|---|---|---|
| `0..44` | *(Giữ nguyên 100% đặc tả `WV2-D` §3.3.1)* | — | `256 × 256`, `world_scale = 200.0` | `512 × 512`, `world_scale = 600.0` |
| `44..48` | `world_extent_m_bits` | `u32` LE (`f32::to_bits`) | `40_960.0` (`40,96 km`) | `4_096.0` hoặc `8_192.0` |
| `48..52` | `elev_min_m_bits` | `u32` LE (`f32::to_bits`) | `-1_500.0` | `-300.0` |
| `52..56` | `elev_max_m_bits` | `u32` LE (`f32::to_bits`) | `+5_700.0` | `+2_100.0` |

**Ba lợi ích trực tiếp khi làm trước `WV2-3`:**
1. **Không phải tạo `ANMW v4` về sau:** Bộ đọc (`AnmwV3Sim`) và `index.json` đọc trực tiếp quy mô vật lý từ file thay vì giả định ngầm `40,96 km`.
2. **Mịn gấp 3 lần độ phân giải đứng của `R16_UNORM` trên đảo 1:1:** Với khối núi `[-1.500 m, +5.700 m]` (`7.200 m`), 1 bước lượng tử 16-bit là `7200 / 65535 ≈ 0,110 m`. Khi chuyển sang đảo ôn đới `[-300 m, +2.100 m]` (`2.400 m`), bước chiều cao tự động mịn xuống **`~0,037 m`** (dưới 4 cm) mà không tốn thêm byte VRAM nào — đủ mượt khi camera zoom sát chân sinh vật.
3. **Tương thích `WV2-D` hiện có:** Với world v2 đại cảnh hiện tại (`sim_256.anmw3`), 3 trường mới nhận đúng giá trị mặc định của `WV2-1a` mà không đổi bất kỳ công thức hay thuật toán nào của `WV2-1a`/`WV2-1b`.

---

## 10. Đề xuất hành động cụ thể & Câu hỏi cần chủ dự án quyết định

### Lộ trình 4 bước đề xuất (theo thứ tự ưu tiên)

1. **Giữ nguyên quỹ đạo Chặng 4 (`VIAB-1 → E3b / SEL2-P2`) trên `MapBounds = 200 × 200`:** không xáo trộn cân bằng sinh thái khi đợt khảo sát khả năng sống sót đang chạy.
2. **Tham số hoá quy mô trong đặc tả `ANMW v3` (`WV2-3`) và `anima-worldgen`:** xem xét mở rộng header lên `56` byte theo [§9](#9-phụ-lục-định-dạng-c--đề-xuất-mở-rộng-12-byte-cho-header-anmw-v3-wv2-3-để-hỗ-trợ-đa-quy-mô-đã-kiểm--suy-luận) (`world_extent_m`, `elev_min_m`, `elev_max_m`) cùng với `world_scale` để hỗ trợ cả thế giới đại cảnh `40,96 km` lẫn các đảo nhỏ `4–8 km` sau này mà không phải phá bỏ định dạng v3 (`ADR-0010`).
3. **Đưa 5 phép đo của [§8.3](#83-ma-trận-so-sánh-nhanh--5-phép-đo-bắt-buộc-của-nr-spike-suy-luận) vào Pha 2 Native Renderer Spike (`NR-SPIKE` / ADR-0011):**
   - Thử nghiệm port thuật toán **Quadtree + UDLOD + Virtual Texturing** của `kurtkuehnert/bevy_terrain` (`MIT OR Apache-2.0`) lên **`wgpu 30.0.2`** dùng chung `Cargo.lock`, đối chứng với `Terrain3D` nếu cần.
   - Kiểm chứng độ ổn định khi zoom sát mặt đất cỡ `0,5–2 m` bằng **Floating Origin / camera-relative `f32`** kết hợp kỹ thuật vi chi tiết địa hình thời gian thực (Grenier et al. 2024) và bài test resize/transparency trên Tauri v2.
4. **Thử nghiệm hiệu chuẩn sinh thái quy mô mở rộng (chỉ sau khi đóng Chặng 4):** dùng bộ thông số trong [§7](#7-phụ-lục-tính-toán-a--cân-chỉnh-thông-số-sinh-thái--hiệu-năng-khi-mở-rộng-200200-256²--600800-512²-đã-kiểm--suy-luận) chạy một đợt đo thăm dò riêng so sánh `256² / MapBounds 200` với `512² / MapBounds 600` (cùng `CREATURE_BODY_SCALE = 1.0`, `REGROWTH_STRIDE = 16`, bổ sung uống ở `River` hoặc mật độ hồ tương xứng, và gieo cụm founder) cho hệ sinh thái **Quần đảo (`QD-ISLAND`)**.

### Câu hỏi cần chủ dự án chốt khi mở thẻ tiếp theo

1. **Mục tiêu thẩm mỹ ưu tiên sau Chặng 4:** Bạn muốn ưu tiên **thế giới đại cảnh `40,96 km` đủ mọi biome từ biển lên núi tuyết** (chấp nhận sinh vật nhìn sát mang tính cách điệu kiểu RTS), hay muốn ưu tiên **đảo cỡ `4–8 km` để góc nhìn sát đất giữa sinh vật – cây – sông – va chạm khớp nhau 1:1** (hoặc chuyển sang mô hình **quần đảo kiểu Valheim**, mỗi đảo một vài km thuộc một đới khí hậu)?
2. **Phạm vi `ANMW v3` (`WV2-3`):** Có đồng ý mở rộng header `ANMW v3` từ `44` lên `56` byte theo [§9](#9-phụ-lục-định-dạng-c--đề-xuất-mở-rộng-12-byte-cho-header-anmw-v3-wv2-3-để-hỗ-trợ-đa-quy-mô-đã-kiểm--suy-luận) ngay ở thiết kế thẻ `WV2-3` để không phải bump `ANMW v4` khi sinh đảo nhỏ hơn sau này không?
3. **Thí nghiệm Quần đảo (`QD-ISLAND`):** Sau khi xong **E3b / SEL2-P2**, bạn có muốn dùng `MapBounds` lớn hơn (`600–800`) cho bài toán tiến hoá trên đảo thật hay giữ `200 × 200` (sức chứa $K$ thay diện tích) cho toàn bộ loạt thí nghiệm đầu tiên?

---

## 11. Danh mục tài liệu tham khảo (tác giả soạn ghi đã xác minh ngày `2026-11-22`)

> Rà soát ngày 2026-11-22: các hằng số và đường dẫn mã gắn nhãn `Đã kiểm` ở §1, §7.1–§7.2 và §9 đã được đối chiếu lại với mã nguồn và khớp. Danh mục dưới đây chưa được kiểm lại từng DOI; số trang/số bài (ví dụ "Article 145" của Paris et al. 2019) có thể cần xác nhận trước khi trích dẫn ra ngoài.

### 11.1 Địa mạo học tính toán & Đồ hoạ địa hình

1. Montgomery, D. R., & Dietrich, W. E. (1988). *Where do channels begin?* Nature, 336(6196), 232–234. DOI: [10.1038/336232a0](https://doi.org/10.1038/336232a0)
2. Montgomery, D. R., & Dietrich, W. E. (1989). *Source areas, drainage density, and channel initiation.* Water Resources Research, 25(8), 1907–1918. DOI: [10.1029/WR025i008p01907](https://doi.org/10.1029/WR025i008p01907)
3. Montgomery, D. R., & Dietrich, W. E. (1992). *Channel initiation and the problem of landscape scale.* Science, 255(5046), 826–830. DOI: [10.1126/science.255.5046.826](https://doi.org/10.1126/science.255.5046.826)
4. Braun, J., & Willett, S. D. (2013). *A very efficient $O(n)$, implicit and parallel method to solve the stream power equation governing fluvial incision and landscape evolution.* Geomorphology, 180–181, 170–179.
5. Barnes, R., Lehman, C., & Mulla, D. (2014). *Priority-Flood: An optimal depression-filling and watershed-labeling algorithm for digital elevation models.* Computers & Geosciences, 62, 117–127. DOI: [10.1016/j.cageo.2013.04.024](https://doi.org/10.1016/j.cageo.2013.04.024)
6. Cordonnier, G., Braun, J., Cani, M.-P., Benes, B., Galin, E., Peytavie, A., & Guérin, E. (2016). *Large Scale Terrain Generation from Tectonic Uplift and Fluvial Erosion.* Computer Graphics Forum, 35(2), 165–175. DOI: [10.1111/cgf.12820](https://doi.org/10.1111/cgf.12820)
7. Galin, E., Guérin, E., Peytavie, A., Cordonnier, G., Cani, M.-P., Benes, B., & Gain, J. (2019). *A Review of Digital Terrain Modeling.* Computer Graphics Forum, 38(2), 553–577. DOI: [10.1111/cgf.13657](https://doi.org/10.1111/cgf.13657)
8. Paris, A., Galin, E., Peytavie, A., Guérin, E., & Gain, J. (2019). *Terrain Amplification with Implicit 3D Features.* ACM Transactions on Graphics, 38(5), Article 145. DOI: [10.1145/3342765](https://doi.org/10.1145/3342765)
9. Schott, H., Paris, A., Fournier, L., Guérin, E., & Galin, E. (2023). *Large-scale Terrain Authoring through Interactive Erosion Simulation.* ACM Transactions on Graphics, 42(4). DOI: [10.1145/3592403](https://doi.org/10.1145/3592403)
10. Cordonnier, G., Jouvet, G., et al. (2023). *Forming Terrains by Glacial Erosion.* ACM Transactions on Graphics, 42(4). DOI: [10.1145/3592422](https://doi.org/10.1145/3592422)
11. Grenier, C., Sauvage, B., Guérin, E., Galin, E., & Rohmer, D. (2024). *Real-time Terrain Enhancement with Controlled Procedural Patterns.* Computer Graphics Forum, 43(1). DOI: [10.1111/cgf.14992](https://doi.org/10.1111/cgf.14992)

### 11.2 Render thế giới lớn, Độ chính xác toạ độ & Tauri surface

12. Losasso, F., & Hoppe, H. (2004). *Geometry clipmaps: terrain rendering using nested regular grids.* ACM Transactions on Graphics, 23(3), 769–776. DOI: [10.1145/1015706.1015799](https://doi.org/10.1145/1015706.1015799)
13. Thorne, C. (2005). *Using a Floating Origin to Improve Fidelity and Performance of Large Distributed Virtual Worlds.* Proceedings of the 4th International Conference on Cyberworlds (CW'05).
14. Strugar, F. (2009). *Continuous Distance-Dependent Level of Detail for Rendering Heightmaps.* Journal of Graphics, GPU, and Game Tools, 14(4), 57–74. DOI: [10.1080/2151237X.2009.10129287](https://doi.org/10.1080/2151237X.2009.10129287)
15. Tauri v2 Official Documentation (windowing, `raw-window-handle`, multiwebview/child surface patterns): https://v2.tauri.app/

### 11.3 Sinh thái học không gian & Quy luật kích thước

16. Huffaker, C. B. (1958). *Experimental studies on predation: Dispersion factors and predator-prey oscillations.* Hilgardia, 27(14), 343–383. DOI: [10.3733/hilg.v27n14p343](https://doi.org/10.3733/hilg.v27n14p343)
17. Durrett, R., & Levin, S. A. (1994). *The importance of being discrete (and spatial).* Theoretical Population Biology, 46(3), 363–394. DOI: [10.1006/tpbi.1994.1032](https://doi.org/10.1006/tpbi.1994.1032)
18. Kelt, D. A., & Van Vuren, D. H. (2001). *The ecology and macroecology of mammalian home range area.* The American Naturalist, 157(6), 637–645. DOI: [10.1086/320621](https://doi.org/10.1086/320621)
19. Jetz, W., Carbone, C., Fulford, J., & Brown, J. H. (2004). *The scaling of animal space use.* Science, 306(5694), 266–268. DOI: [10.1126/science.1102138](https://doi.org/10.1126/science.1102138)
20. Tamburello, N., Côté, I. M., & Dulvy, N. K. (2015). *Energy and the scaling of animal space use.* The American Naturalist, 186(2), 196–211. DOI: [10.1086/682070](https://doi.org/10.1086/682070)
21. Benítez-López, A., Santini, L., Gallego-Zamorano, J., Milá, B., Walkden, P., Huijbregts, M. A. J., & Tobias, J. A. (2021). *The island rule explains consistent patterns of body size evolution in terrestrial vertebrates.* Nature Ecology & Evolution, 5, 768–786. DOI: [10.1038/s41559-021-01426-y](https://doi.org/10.1038/s41559-021-01426-y)

### 11.4 Kho mã nguồn mở đã khảo sát

22. `fastscape-lem/fastscape` (`BSD-3-Clause`): https://github.com/fastscape-lem/fastscape
23. `fastscape-lem/fastscapelib` (`GPL-3.0`): https://github.com/fastscape-lem/fastscapelib
24. `landlab/landlab` (`MIT`): https://github.com/landlab/landlab
25. `r-barnes/richdem` (`GPL-3.0`): https://github.com/r-barnes/richdem
26. `erosiv/geotransport` & `erosiv/soillib` (`LGPL-3.0` cho `soillib`): https://github.com/erosiv/geotransport
27. `open-terra/terra` (`GPL-3.0`): https://github.com/open-terra/terra
28. `redblobgames/mapgen4` (`Apache-2.0`): https://github.com/redblobgames/mapgen4
29. `Azgaar/Fantasy-Map-Generator` (`MIT`): https://github.com/Azgaar/Fantasy-Map-Generator
30. `veloren/veloren` (`GPL-3.0-or-later`): https://gitlab.com/veloren/veloren
31. `kurtkuehnert/bevy_terrain` (`MIT OR Apache-2.0`, nhắm tới Bevy `0.14`): https://github.com/kurtkuehnert/bevy_terrain
32. `TokisanGames/Terrain3D` (`MIT`): https://github.com/TokisanGames/Terrain3D
