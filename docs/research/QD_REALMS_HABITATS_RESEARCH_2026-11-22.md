---
title: Ba môi trường sống QD-REALMS (nước, trời, hang) — đánh giá đề xuất ngoài
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-24
review_cycle: quarterly
---

# Ba môi trường sống QD-REALMS (nước, trời, hang) — đánh giá đề xuất ngoài

> **Phạm vi & tính chất tài liệu.** Đây là bản **đánh giá và chắt lọc** một báo cáo nghiên cứu ngoài
> (*"Kiến Trúc Đa Tầng QD-REALMS"*, soạn bằng công cụ nghiên cứu tự động, nhận ngày 2026-11-22; dưới
> đây gọi là **báo cáo gốc**). Báo cáo gốc đề xuất ba môi trường mới: thuỷ sinh (thuỷ động lực học
> rút gọn, bóng hơi, cột nước 1D), không trung (lý thuyết phần tử cánh, lượn nhiệt, định luật công
> suất bay) và hang động 3D (địa hình lai heightmap + SDF thưa, Surface Nets, định tuyến octree),
> triển khai qua ba ADR mà nó gọi là "ADR-01/02/03". Tài liệu này **không** phải hợp đồng, kế hoạch
> hay quyết định. Số đo của repo chỉ nằm ở
> [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).
> Quyết định QD-REALMS ([`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md), mục `QD-REALMS`) đã định thứ
> tự: **mỗi lần một môi trường, mỗi môi trường một ADR riêng** (cập nhật 2026-11-24: ADR-0011 được
> để dành cho renderer sau NR-SPIKE, ADR-0012 và ADR-0013 đã dùng, nên số trống tiếp theo là ADR-0014
> trở đi), chỉ sau kết quả SEL2-P; trước đó cá, chim, thú hoang và hang vẫn là trang trí. Không thẻ nào
> từ báo cáo gốc được mở trước khi Chặng 4 (VIAB-1 → E3b / SEL2-P2) đóng.

## 0. Nhãn bằng chứng và kết luận ngắn

| Nhãn | Nghĩa |
|---|---|
| **Đã kiểm** | Đối chiếu trực tiếp với mã nguồn và tài liệu của repo ngày 2026-11-22. |
| **Nguồn ngoài** | Công trình có thật mà báo cáo gốc trích; tóm theo báo cáo gốc, chưa đọc lại từng bài trừ khi ghi rõ. |
| **Chưa xác minh** | Con số dẫn về mục trích dẫn số 1 của báo cáo gốc (`unknown_url`) hoặc về một mô phỏng riêng không có mã. |
| **Suy luận** | Đánh giá của tài liệu này. |

**Kết luận (`Suy luận`):**

1. **Phần sinh học và năng lượng có giá trị**: cột nước phân tầng giữ EU kín, chi phí bảo dưỡng giác
   quan, ràng buộc công suất bay, chuỗi thức ăn hang nuôi bằng mùn bã thấm từ mặt đất.
2. **Phần kiến trúc giả định một engine mà repo không có**: thế giới 4.096 m × 512 m ở độ phân giải
   1 m, trọng lực, tiếp xúc nền và hàm `getHeight` trong bộ giải va chạm (§5).
3. **Lực đẩy dưới nước bằng cản dị hướng mâu thuẫn với bằng chứng đã đo ở ADR-0006** nếu không có
   CPG sóng lan truyền.
4. **Mọi con số then chốt (bộ nhớ, trần khối lượng bay, sai số bảo toàn) là `Chưa xác minh`** (§6).
5. Thời điểm: sau Chặng 4 và sau SEL2-P, đúng thứ tự QD-REALMS: nước → trời → hang.

## 1. Thuỷ sinh

### 1.1 Nên giữ

- **Hệ số ngập trơn theo từng đốt** (nội suy bậc ba quanh mặt nước) nhân vào mọi lực chất lưu, để
  qua mặt nước không gây xung lực số học. Sẽ nằm trong `integrate_physics_system`
  ([`dynamics.rs`](../../src-tauri/src/physics/dynamics.rs), từ dòng 121). Repo chưa có gì tương đương.
- **Cản dị hướng theo trục đốt (RFT bậc hai) cộng khối lượng kèm theo đường chéo (Kirchhoff–Lamb)**
  cho lực đẩy và hình thái thuôn. `Nguồn ngoài`: Lamb/Brennen cho hệ số khối lượng kèm theo, Lighthill
  (1971) cho phản lực đuôi. Điều kiện tiên quyết: CPG sóng lan truyền (§5, mục 4).
- **Cột nước 1D (ánh sáng Beer–Lambert, thực vật phù du, mùn bã lắng, oxy hoà tan)**, cập nhật ở nhịp
  thưa chứ không mỗi tick. Đây là phần mở rộng tự nhiên của `ResourceField::step_regrowth_gated`
  ([`ecology.rs`](../../src-tauri/src/core/ecology.rs), dòng 425): tăng trưởng lấy từ ngân sách mùn
  bã, nên không tạo năng lượng từ hư không. Các ngăn mới phải vào `closed_total_eu` (§4).
- **Bóng hơi và lipid như hai gen liên tục**, tạo đánh đổi giữa giữ độ sâu rẻ và di cư thẳng đứng.
  Chỉ có nghĩa khi đã có trọng lực và đơn vị khối lượng thật; hiện chưa có (§5).

### 1.2 Chưa nên làm

- Toàn bộ bảng hệ số khối lượng kèm theo theo hình dạng và mô hình áp suất Boyle ở đơn vị SI: thế
  giới live không có trọng lực, mật độ hay mét (`MapBounds` mặc định 200 × 10 × 200 đơn vị,
  [`resources.rs`](../../src-tauri/src/core/resources.rs) dòng 328-334).

## 2. Không trung

### 2.1 Nên giữ

- **Bay phải đắt tính theo công suất (EU/giây), không chỉ theo quãng đường.** Đường cong công suất
  hình chữ U, bay lơ lửng tốn hơn bay ngang, và công suất cơ khả dụng tăng chậm hơn công suất cần
  khi khối lượng tăng, nên tự sinh trần khối lượng mà không cần hệ số kìm hãm viết tay. Mọi chi phí
  đi qua `total_cost` trong `metabolic_decay_system`
  ([`world_systems.rs`](../../src-tauri/src/core/world_systems.rs), dòng 157).
- **Ngân sách khối lượng cơ bay** (`Nguồn ngoài`: khoảng 20–25 % khối lượng ở chim bay khoẻ): phần đầu
  tư cho cơ bay lấy bớt từ phần khác của cơ thể. Hợp với phát triển một lần lúc sinh (ADR-0001).
- **Trường gió thẳng đứng 2D rẻ** (luồng bốc sườn núi, cột nhiệt) suy từ khí hậu World v2 (S7). Khi
  có trường này, lượn có lợi thế ở đúng chỗ địa hình cho phép.
- `Aerial` đã có trong danh sách giá trị habitat
  ([`CREATURE_MORPHOGENESIS.md`](../explanation/CREATURE_MORPHOGENESIS.md), dòng 187), chưa có mã.

### 2.2 Chưa nên làm

- Lý thuyết phần tử cánh theo số Reynolds, hiệu ứng mặt đất McCormick, khí quyển phân tầng: chi tiết
  quá mức so với vật lý hiện tại (lực lái `target_velocity`, không trọng lực).

## 3. Hang động

### 3.1 Nên giữ

- **Chuỗi thức ăn hang nuôi bằng mùn bã thấm từ ô mặt đất phía trên** (và phân của sinh vật bay đậu
  trong hang): chuyển EU giữa các ngăn, không tạo mới. Chỗ đặt hợp lý là đồ thị hang ANMC của World v2
  (`cave_network.bin`, nút có `light_decay`;
  [`2026-11-20-world-v2-design.md`](../ai/design/2026-11-20-world-v2-design.md), dòng 217-226): các
  ngăn năng lượng theo nút đồ thị thay vì trường SDF.
- **Heightmap + SDF thưa (Paris et al. 2019) và Surface Nets** cho phần hiển thị hang đi vào được.
  Đã ghi ở [`WORLD_SCALE_AND_MAP_GENERATION_RESEARCH_2026-11-22.md`](WORLD_SCALE_AND_MAP_GENERATION_RESEARCH_2026-11-22.md)
  (dòng 120). Thư viện `fast-surface-nets-rs` có thật (§6).

### 3.2 Chưa nên làm

- "Pin hoá trị lưu huỳnh/metan" với hằng số quy đổi 0,787 MJ/mol: trộn đơn vị SI với EU, thêm một
  ngăn mới vào sổ kín mà không có câu hỏi khoa học nào cần nó.
- Định tuyến octree Morton 64-bit (Brewer 2017) và bò vách/trần: điều hướng live hiện là lưới đi được
  2D với bước kéo về ô đi được ([`terrain_navigation.rs`](../../src-tauri/src/core/terrain_navigation.rs),
  dòng 10 và 106).

## 4. Ý tưởng xuyên suốt (`Suy luận`)

- **Chi phí bảo dưỡng giác quan trong `total_cost`.** `Nguồn ngoài`: báo cáo gốc dẫn Moran, Softley &
  Warrant (2015) rằng mắt và thuỳ thị giác tốn khoảng 5–15 % trao đổi chất nghỉ ở cá hang (chưa đọc
  lại). Một số hạng theo kiểu `cognition_cost` (`world_systems.rs`, dòng 147-157) đủ để chọn lọc tự
  đào thải mắt trong bóng tối, không cần hệ số theo vùng. Ý này dùng được cả trước khi có hang.
- **Mọi ngăn mới phải vào `closed_total_eu`**
  ([`energy.rs`](../../src-tauri/crates/anima-domain/src/energy.rs), dòng 375), hiện chỉ cộng thực
  vật, động vật, mùn bã và dormant. Báo cáo gốc gọi kho chung là "EU_abiotic"; repo không có kho đó,
  tương đương gần nhất là ngăn mùn bã.
- **Oxy hoà tan nằm ngoài sổ EU**, giống hydration
  ([ADR-0007](../decisions/ADR-0007-live-ecology-sustainability.md), dòng 35).
- **Mật độ nước, mật độ không khí và các hằng chất lưu là luật thế giới** (`WorldLawSet`,
  [ADR-0002](../decisions/ADR-0002-world-laws-and-exotic-energy.md)): bất biến trong một lượt chạy.
- **`LocomotionMedium` là đặc điểm của sinh vật, cố định lúc sinh** (ADR-0001, `CLAUDE.md`). Lực chất
  lưu được phép phụ thuộc vào mức ngập tại chỗ, nhưng môi trường sống của sinh vật thì không được suy
  từ ô đích; kiểm hợp lệ là phép so hai phía (`CREATURE_MORPHOGENESIS.md`, dòng 182-194).

## 5. Chỗ báo cáo gốc sai về repo hoặc va với quy tắc (`Đã kiểm`)

1. **Đánh số ADR.** "ADR-01/02/03" trùng ADR-0001 (vòng đời phát triển), ADR-0002 (luật thế giới) và
   ADR-0003 (não tiến hoá). Môi trường đầu tiên sẽ là ADR-0014 hoặc số sau đó (ADR-0011 để dành cho renderer; ADR-0012, ADR-0013 đã dùng; kiểm lại chỉ mục ADR khi viết).
2. **Kích thước thế giới.** Báo cáo gốc dùng 4.096 × 4.096 × 512 m ở 1 m/ô. World v2 là khối núi
   40,96 km ở 10 m/px (lưới 4096²), cao độ từ −1.500 m tới +5.700 m; mô phỏng chạy trên `sim_256`
   (256²). Mọi con số bộ nhớ của báo cáo gốc tính cho một thế giới khác.
3. **Không có trọng lực, tiếp xúc nền hay `getHeight` trong Rust.** `integrate_physics_system` chỉ làm
   `velocity += force/mass·dt` rồi `pos += vel·dt` (`dynamics.rs`, khoảng dòng 207-213). `getHeight`
   chỉ có ở frontend (`src/components/Map2/utils/map2TerrainGen.ts`, dòng 310). Sức nổi, Boyle và bóng
   hơi không có gì để tác động.
4. **Lực đẩy từ cản dị hướng.** ADR-0006, bằng chứng (a), đo được **không có lực đẩy** với CPG dao
   động đứng hiện tại, và con mồi tuyệt chủng 3/3 seed; quyết định là QĐ-LOC = A, bộ lái
   ([ADR-0006](../decisions/ADR-0006-locomotion-actuation.md), dòng 146 và 210). Bơi bằng RFT cần CPG
   sóng lan truyền (Phương án B), chưa làm.
5. **"EnvironmentSampler" va tên với `EnvSample`.** Trong hợp đồng phát triển, `EnvSample` /
   `sample_environment` là ảnh chụp môi trường **lúc sinh**, đầu vào của `develop_at_birth`
   ([`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md), dòng 30). Một
   truy vấn môi trường mỗi tick là thứ khác và cần tên khác.
6. **"EU_abiotic".** Không tồn tại; xem §4. Năng lượng địa nhiệt "trừ từ EU_abiotic" chỉ giữ EU kín
   nếu rút từ một ngăn đã nằm trong `closed_total_eu`.

## 6. Con số chưa xác minh và kiểm tra mẫu

Mục trích dẫn số 1 của báo cáo gốc là `unknown_url`, nhưng nó đỡ những con số quan trọng nhất. Các
con số sau là **giả thuyết**, có vẻ là tính toán riêng của công cụ, chưa ai tái lập:

- Bộ nhớ: 64,0 MB TerrainMap; 436,5 MB và 453,8 MB cho địa hình lai; 17,3 MB SVO; giảm 98,7 %.
  Số học nhất quán nội bộ, nhưng cho thế giới sai (§5, mục 2).
- Trần khối lượng bay: 583–757 g (lơ lửng), 5,95–7,62 kg (vỗ cánh đẳng hướng), khoảng 25,11 kg (dị
  hướng); trần độ cao khoảng 4.800 m cho cá thể 1 kg; 40,0 lần BMR khi lơ lửng.
- Sai số bảo toàn EU 2,27 × 10⁻¹³ (cột nước) và 1,05 × 10⁻¹¹ (hang); đỉnh phù du 1,956 EU/m³; SVD
  "chậm hơn khoảng 20 lần" so với Surface Nets.
- Nguồn yếu: Reddit, YouTube, Scribd, Studocu, và một tệp `CLAUDE.md` của dự án `isomesh`.

Kiểm tra mẫu (web, 2026-11-22):

| Khẳng định của báo cáo gốc | Kết quả |
|---|---|
| `fast-surface-nets-rs` đạt khoảng 20 triệu tam giác/giây trên một nhân i7 2,5 GHz | **Khớp** README của thư viện. |
| Cá hang Pachón tiết kiệm 27 % năng lượng (chu kỳ sáng–tối) và cá mặt đất tốn hơn 38 % (tối hoàn toàn) | **Khớp** tóm tắt Moran et al. (2014). Ý "sống thọ gấp 3 lần" chưa kiểm. |
| Hang Movile có 37 loài đặc hữu | **Khớp một nguồn** (Brad, Iepure & Sarbu); nguồn khác ghi 34–35. Theo tóm tắt thứ cấp. |
| Cá mập nổi âm bơi nhanh hơn khoảng 30 % và tốn chi phí vận chuyển hơn khoảng 40 % (Iosilevskii & Papastamatiou 2016) | **Không có trong tóm tắt**; chưa đọc thân bài. `Chưa xác minh`. |

## 7. Việc nên làm tiếp (`Suy luận`, chờ chủ dự án)

1. **Không** mở thẻ code nào từ báo cáo gốc trước khi Chặng 4 đóng và SEL2-P có kết quả.
2. Khi mở môi trường nước: một ADR riêng (ADR-0014+), bắt đầu từ CPG sóng lan truyền (ADR-0006 Phương
   án B), rồi lực chất lưu opt-in, rồi cột nước giữ EU kín; bump `LIVE_MODEL_VERSION` theo ADR-0008.
3. Ứng viên rẻ, độc lập với môi trường: chi phí bảo dưỡng giác quan trong `total_cost`, sau khoá opt-in
   như `brain_metabolic_cost`.
4. Với World v2: chỉ giữ đồ thị hang dạng dữ liệu (S9); ghi chú khả năng nối mùn bã thấm vào đồ thị
   cho ADR môi trường hang sau này.
