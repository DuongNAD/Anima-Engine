---
title: Nghiên cứu nâng cấp — Map và mô hình machine cho Anima Engine
status: proposed
owner: architecture
last_reviewed: 2026-07-25
review_cycle: quarterly
---

# Nghiên cứu nâng cấp — Map và mô hình machine

Tài liệu này là **bằng chứng bên ngoài + đề xuất**, không phải hợp đồng. Nguồn chuẩn về
hiện trạng vẫn là code và `docs/reference/`. Khảo sát dependency tổng quát nằm ở
[OPEN_SOURCE_LANDSCAPE.md](OPEN_SOURCE_LANDSCAPE.md); tài liệu này đào sâu **hai mảng người
dùng yêu cầu**: bản đồ/thế giới và mô hình machine (ML/tiến hoá).

Mức quyết định dùng lại thang của khảo sát nguồn mở: **Adopt / Pilot / Oracle / Reference /
Reject now**.

---

## 0. TL;DR — năm phát hiện có đòn bẩy lớn nhất

| # | Phát hiện | Bằng chứng | Đề xuất |
|---|---|---|---|
| 1 | **Toàn bộ agent dùng CHUNG một bộ não.** `BrainModel::new(15, 64, 4)` được `insert_resource` một lần; genome (`MorphologyGenotype`) chỉ có `nodes`/`edges`, **không có gen não**. | `simulation_loop.rs:523`, `evolution/genotype.rs`, grep `weights\|neural` trong `evolution/` → 0 kết quả | Tách não thành **gen per-agent** (B1). Đây là thay đổi đáng giá nhất trong tài liệu này. |
| 2 | Lý do kỹ thuật khiến (1) xảy ra đã có tên trong tài liệu khoa học: **brain–body coupling**. Hình thái đột biến → số sensor/actuator đổi → MLP cố định hình dạng vỡ. | Mertan & Cheney và khảo sát co-design 2024–2025 | **Indirect encoding (CPPN/HyperNEAT)**: gen là hàm sinh trọng số theo toạ độ → hợp với **mọi** body plan (B2). |
| 3 | Map thiếu **tầng kiến tạo**. Bắt đầu từ noise nên không bao giờ có "câu chuyện địa chất" — đây là gốc của cả 3 khoảng trống ở `WORLD_DESIGN.md` §2 (khí hậu 2 cực đối xứng, gió chỉ Đông–Tây, địa mạo tĩnh). | WorldEngine/PyPlatec, World Orogen, Gleba, realistic-planet-generation | Thêm **pass tectonics trước erosion** (A1). |
| 4 | **Nợ phiên bản lớn**: `burn 0.13.2` (hiện tại 0.21.0), `bevy_ecs 0.13.0` (hiện tại 0.19.0). Bevy 0.19 biến `Resource` thành subtrait của `Component` → `impl Resource for BrainModel` thủ công + `unsafe impl Send/Sync` **sẽ vỡ**. | `Cargo.toml`, `ai/model.rs:67-70`, crates.io, migration guide Bevy 0.18→0.19 | Nâng cấp **theo bậc, mỗi bậc một ADR** (C1). Không gộp với thay đổi luật mô phỏng. |
| 5 | **Determinism nứt ở tầng live.** 8 điểm gọi `thread_rng()` trong đường sim sống, trong khi `exotic_energy.rs` / `experiment_runner.rs` đã ghi rõ "never `thread_rng()`". | `agent_systems.rs:187`, `environmental_systems.rs:134`, `simulation_loop.rs:877`, `world_systems.rs:155,526`, `crossover.rs:45`, `mutation.rs:52`, `map_elites.rs:57` | Đưa RNG thành **resource có seed** (C2). Chặn đường replay và mọi thí nghiệm nghiêm túc nếu không sửa. |

**Một kết quả khoa học đáng chú ý cho định hướng "triệu agent":** nghiên cứu 2025 trên
môi trường sinh thái quy mô lớn (60.000+ agent, tiến hoá thuần, không reward) cho thấy một số
hành vi phức tạp — kiếm ăn bằng thị giác, săn mồi, khai thác tài nguyên xa — **chỉ xuất hiện khi
môi trường và quần thể đủ lớn**, và quy mô lớn hơn làm hành vi ổn định hơn. Nghĩa là mục tiêu
quy mô của Anima **không phải khoe hiệu năng mà là điều kiện cần để hiện tượng xuất hiện**. Điều
này nâng ưu tiên của Simulation-LOD từ "tối ưu" lên "điều kiện nghiên cứu".

---

## 1. Baseline đo được (ngày 2026-07-25)

Đo trực tiếp từ mã nguồn, không lấy từ tài liệu cũ:

| Hạng mục | Giá trị |
|---|---|
| Rust backend | 20.624 dòng, 47 file `.rs`, single crate |
| Map "thật" (nơi agent sống) | `terrain.rs`, mặc định **128×128**, **11 biome** (`DeepOcean`..`Snow`) |
| Map "đẹp" (render) | `worldGen.ts`, 1.626 dòng, tới 2048², 22 biome, cache IndexedDB |
| Cầu nối | `WorldArtifact` v2 (magic `ANMW`), TS↔Rust byte-cho-byte, có fixture kiểm chứng |
| Bộ não | **1** `ActorCriticModel` chung: 15 → 64 → 64 → {4 actor, 1 critic}, ~5.5k tham số |
| Học | A2C thủ công (TD-error + Adam) trong `simulation_loop.rs:1426-1477` |
| Tiến hoá | `MorphologyGenotype` (graph nodes/edges kiểu Karl Sims) + MAP-Elites 2D + crossover/mutation |
| QD archive | `HashMap<(i32,i32), EliteIndividual>`, bin cố định, 80 dòng |
| Vận động | CPG oscillator per-segment (`ai/cpg.rs`, 60 dòng) |
| Động lực nội tại | Homeostatic RL (`ai/hrrl.rs`): energy/hydration/temperature |
| ML deps | `burn 0.13.2` + `burn-ndarray` + `burn-wgpu`, GPU qua `ANIMA_USE_GPU` |

Nhận xét trung thực: **phần sinh thái và tiến hoá hình thái đã khá sâu** (ecology.rs, MTE,
Holling III, closed-energy, MAP-Elites, lineage Neo4j, causal ledger). **Phần "trí tuệ" thì nông
nhất trong toàn hệ** — và đó cũng chính là chỗ có đòn bẩy cao nhất.

---

## 2. Phần A — MAP

### A0. Khoảng trống đã biết vs. khoảng trống mới phát hiện

`WORLD_DESIGN.md` §2 đã liệt kê 10 khoảng trống và §3 đã dẫn 5 nguồn tham chiếu
(Nick McDonald particle hydrology, Génévaux 2013, chunked LOD, voxel cave, Bibites/Framsticks).
Tài liệu này **không lặp lại** mà bổ sung những gì khảo sát cũ chưa có.

### A1. Tầng kiến tạo (tectonics) — *Pilot, ưu tiên cao*

**Vấn đề gốc.** `worldGen.ts` bắt đầu từ fBm noise rồi vá bằng hydrology. Vì thế:
dãy núi không có hướng, không có vòng cung đảo núi lửa, không có thềm lục địa, khí hậu phải
giả lập bằng `lat = 1-|ny|` (xích đạo giữa map, cực ở **cả hai** biên). Ba khoảng trống ở §2
là *triệu chứng* của cùng một nguyên nhân: **thiếu tầng nhân quả địa chất**.

**Bằng chứng bên ngoài.** Các bộ sinh thế giới "đúng vật lý" đều đặt tectonics ở tầng đáy:

- **WorldEngine** (Python, dùng **PyPlatec**) — mô phỏng mảng rồi mới erosion, rain shadow,
  và phân loại **Holdridge life zones**. Đây là gia phả trực tiếp của kiểu worldgen Anima đang làm.
- **World Orogen** — mảng hội tụ/phân kỳ/trượt ngang, ba loại erosion (băng hà, thuỷ lực, nhiệt),
  gió theo mùa, dòng hải lưu, và phân loại **Köppen**. Chạy trong trình duyệt → **đọc được kết quả
  tham chiếu mà không cần cài gì trên máy yếu**.
- **Gleba** — "fantasy world simulator chính xác khoa học": orogeny từ mảng, vận chuyển trầm tích,
  và mô hình khí hậu toàn cầu tính mưa/nhiệt **theo cả năm**.
- **realistic-planet-generation-and-simulation** (FreezeDriedMangos) — mảng + thời tiết + dòng hải lưu,
  giữ lại dữ liệu mảng sau khi sinh để tính tiếp.

**Đề xuất cụ thể cho Anima.** Không cần mô phỏng mảng đầy đủ. Đủ để phá thế đối xứng:

1. Gieo N=8–14 mảng bằng Voronoi trên lưới, mỗi mảng có vector vận tốc + cờ đại dương/lục địa.
2. Phân loại biên: hội tụ (uplift + arc núi lửa), phân kỳ (rift/sống núi giữa đại dương), trượt ngang.
3. Cộng uplift bất đối xứng vào elevation **trước** khi chạy hydrology hiện có.
4. Gán **một trục cực thật** (không phải `1-|ny|`) → khí hậu có Bắc/Nam, mở đường cho gió 2 trục (A3).

Đây là ~200–300 dòng thuần hàm, **test được headless**, không đụng render, và bump
`WORLD_GEN_VERSION` một lần. Gate đề xuất: tỉ lệ đất/biển vẫn ~38%, phân bố hướng dãy núi không
còn đồng nhất, và số biome vẫn phủ 22/22.

### A2. Erosion giải tích/multigrid thay vì lặp hạt — *Pilot*

`WORLD_DESIGN.md` §3 đã chọn hướng **particle hydrology của Nick McDonald**. Hướng đó đúng về
mặt tự nhất quán (sông và hồ sinh từ cùng một quá trình) nhưng **hội tụ chậm** ở 2048².

Nghiên cứu 2024 của INRIA, *"Physically-based analytical erosion for fast terrain generation"*,
tăng tốc hội tụ giữa cao độ và mạng lưới sông bằng phương pháp **lấy cảm hứng multigrid + tối ưu
hoá**, đồng thời gộp cả erosion sườn đồi và erosion nhiệt. Cùng nhóm chủ đề, *"Real-time Terrain
Enhancement with Controlled Procedural Patterns"* (Grenier et al., **Computer Graphics Forum 2024**)
định nghĩa hàm thủ tục **biến thiên theo không gian** để biểu diễn **hoa văn erosion** mà vẫn giữ
được nét sắc và tính ngẫu nhiên nhất quán với địa hình.

**Vì sao quan trọng riêng với máy của bạn.** Hai kỹ thuật này trả lời hai câu hỏi khác nhau:

- A2a (multigrid) — giảm **thời gian sinh**, chạy một lần, cache lại. Phù hợp backend Rust.
- A2b (procedural pattern) — thêm **chi tiết nhìn thấy được** ở thời điểm render, **không tốn
  bước mô phỏng nào**. Đây là cách rẻ nhất để địa hình trông "đã bị bào mòn" trên iGPU.

Khuyến nghị: **làm A2b trước** (rẻ, thuần shader/texture, không đổi data → không bump version),
A2a sau và chỉ khi benchmark chứng minh thời gian sinh là nút thắt.

### A3. Khí hậu: gió hai trục + phân loại chuẩn — *Adopt*

Hiện tại orographic sweep chỉ chạy dọc hàng X → rain shadow chỉ tạo được qua dãy Đông–Tây (§2).
Tài liệu worldbuilding kỹ thuật (Climate Modeling 101; loạt *An Apple Pie from Scratch* phần khí hậu)
mô tả công thức tối thiểu đã đủ thuyết phục:

- Ba đai hoàn lưu (Hadley/Ferrel/Polar) → hướng gió đổi theo vĩ độ, không phải một hướng duy nhất.
- Rain shadow tính theo **hướng gió cục bộ**, không theo trục lưới.
- Phân loại biome bằng **Whittaker** (nhiệt độ × lượng mưa) hoặc **Köppen/Holdridge** thay vì ngưỡng ad-hoc.

Điểm quan trọng về **kiến trúc, không phải công thức**: một lời khuyên lặp lại trong cộng đồng
proceduralgeneration là *giữ các mô phỏng độc lập nhau* (kiến tạo, khí hậu, thảm thực vật là các
pass tách rời) và *cho phép click vào ô bất kỳ để xem mọi biến* — vì debug worldgen bằng mắt là
gần như bất khả nếu không có công cụ đó. Anima đã có `mcp-Vision` + `map_manifest.json`; mở rộng
manifest để chứa **per-cell inspection** là bước rẻ và trả nợ ngay.

Đề xuất: chuẩn hoá `BIOME_TAXONOMY.md` sang Whittaker/Holdridge có tên khoa học, để cả 11-biome
backend và 22-biome frontend đều là **phép chiếu của cùng một bảng** thay vì hai bảng độc lập.

### A4. Khuếch đại địa hình bằng ML — *Defer, có điều kiện*

Đây là giao điểm map ∩ ML mà câu hỏi nhắm tới, nên cần trả lời thẳng.

**Có tồn tại và đã trưởng thành:** DEM super-resolution bằng FCN/SRGAN/Transformer đạt tới một bậc
độ phân giải cao hơn so với nội suy; *Terrain Diffusion Network* sinh địa hình **có ý thức khí hậu**
với dẫn hướng bằng phác thảo địa chất; *StyleDEM* và *Multi-theme GAN terrain amplification* cho phép
authoring theo phong cách.

**Nhưng khuyến nghị của tôi là DEFER**, vì ba lý do gắn với chính dự án này:

1. **Determinism.** Anima đã cam kết `WorldArtifact` tái lập byte-cho-byte giữa TS và Rust. Một model
   neural trong đường sinh làm hỏng cam kết đó trừ khi ta pin weights + backend + thứ tự phép toán.
2. **Ngân sách máy.** Inference một model SR trên 2048² tốn nhiều hơn toàn bộ ngân sách GPU ~50–60MB
   mà `WORLD_DESIGN.md` §4.3 đã đặt cho map — và ngân sách đó tồn tại để **nhường chỗ cho brain inference**.
3. **A2b rẻ hơn và đủ tốt.** Hoa văn erosion thủ tục (Grenier 2024) cho ~80% hiệu ứng thị giác với
   ~2% chi phí và **không có rủi ro tái lập**.

Điều kiện để mở lại: khi có build release chạy được trên máy mạnh, và khi ta muốn **tăng độ phân giải
render** chứ không phải tăng độ phân giải **dữ liệu mô phỏng**. Nguyên tắc: ML chỉ được đụng vào lớp
**hiển thị**, không đụng vào `elevation` mà sim đọc.

### A5. Hang thật bằng SDF cục bộ — *Pilot*

M4 đã kết luận đúng: heightmap liên tục không đục lỗ được. Khảo sát bổ sung tìm được ứng viên Rust
sát nhất:

- **bevy-sculpter** — biểu diễn thể tích **dựa trên SDF**, lưu theo chunk, meshing bằng **Surface Nets**
  (mượt, **liền mạch qua biên chunk**), có raycasting. Đây gần như đúng bài toán M4 còn lại.
- **bevy_voxel_world** — meshing đa luồng, spawn/despawn chunk, texture mapping.
- **godot_voxel** (C++) — *Reference*: chứng minh mô hình "địa hình 3D chỉnh sửa được với overhang,
  đường hầm, phân trang chunk vô hạn" là khả thi ở quy mô game.

Đề xuất giữ nguyên kiến trúc hybrid đã ghi trong M4: heightmap cho bề mặt, **SDF chỉ ở túi hang thưa**,
cache mesh. Surface Nets đáng thử trước Marching Cubes vì xử lý biên chunk sạch hơn.

---

## 3. Phần B — MÔ HÌNH MACHINE

### B0. Chẩn đoán

Anima hiện là một **ALife engine có cơ thể tiến hoá nhưng trí tuệ không tiến hoá**. Cụ thể:

```
MorphologyGenotype { nodes, edges }   →  cơ thể     →  DI TRUYỀN, ĐỘT BIẾN, CHỌN LỌC ✓
BrainModel (Resource, dùng chung)     →  hành vi    →  KHÔNG di truyền ✗
```

Hệ quả trực tiếp, có thể suy ra mà không cần chạy:

- MAP-Elites chỉ illuminate **không gian hình thái**; hai elite ở hai ô niche khác nhau vẫn **hành xử
  bằng cùng một bộ não**.
- `niche_divergence` (E11) đo được phân kỳ khối lượng cơ thể nhưng **không thể** đo phân kỳ hành vi —
  vì hành vi không có biến thể để phân kỳ.
- Red Queen predator/prey (S43) chạy được ở tầng hình thái/năng lượng, nhưng "đua vũ trang" đúng nghĩa
  cần **chiến lược** đối kháng nhau.
- Gradient A2C dùng chung kéo mọi agent về **một chính sách trung bình** — nó *chống lại* đa dạng,
  ngược mục tiêu QD của chính dự án.

**Một ràng buộc thứ hai, hẹp hơn nhưng dễ bỏ sót — kênh hành vi quá hẹp.** 4 output của actor head
đi thẳng vào `InertiaComponent.cpg_parameters: [f32; 4]` (`core/components.rs:185-192`). Nghĩa là bộ não
hiện tại là **bộ điều khiển dáng đi**, không phải bộ ra quyết định: nó chỉ điều biến CPG. Kể cả khi mỗi
agent có não riêng (B1), hai agent vẫn **không thể** khác nhau ở "ăn hay chạy", "săn hay trốn", "đi theo
pheromone hay bỏ qua" — vì không có kênh nào để biểu đạt. Do đó B1 phải đi kèm **mở rộng không gian hành động**
(ví dụ thêm output rời rạc cho ăn/tấn công/tiết pheromone/sinh sản), nếu không sẽ tốn công tiến hoá não mà
không quan sát được đa dạng hành vi. Đây là điểm khác biệt then chốt so với The Bibites, nơi lớp hành vi có
nhiều output độc lập ngay từ đầu.

### B1. Não thành gen per-agent — *Adopt (thay đổi trọng tâm)* → **[ADR-0003](../decisions/ADR-0003-evolved-per-agent-brains.md)**

**Bằng chứng bên ngoài — cả ba hệ tham chiếu đều làm ngược với Anima:**

| Hệ | Não | Học | Quy mô | Kết quả nổi lên |
|---|---|---|---|---|
| **JaxLife** (ALIFE 2024, Oxford/FLAIR) | Mạng có **attention + LSTM**, riêng từng agent; **không đổi trong đời** | **Không RL.** Sinh sản vô tính: copy trọng số + nhiễu | 128 agent, 32 robot, 1×A100 | Giao thức giao tiếp sơ khai, **nông nghiệp**, dùng công cụ |
| **The Bibites** | **NEAT-like**: node + synapse tự tiến hoá, khởi đầu **não rỗng** | Không RL | Hàng nghìn | Đi theo vệt pheromone để săn, tích trữ thức ăn theo vùng |
| **Ecological environments** (arXiv 2510.18221, 2025) | Mạng tiến hoá riêng từng agent | **Không reward, không giám sát** | **60.000+ agent** | Kiếm ăn bằng thị giác, **săn mồi**, khai thác tài nguyên xa |

Điểm chung: **chọn lọc tự nhiên trên trọng số riêng từng cá thể**, không phải gradient dùng chung.
JaxLife nói rõ hạn chế của họ là *"mọi agent giống hệt nhau về mặt vật lý"* — và đó chính là **thứ
Anima đã có mà họ không có**. Nếu Anima thêm não di truyền, nó đứng ở vị trí **hiếm**: cơ thể tiến hoá
**và** não tiến hoá, trên nền sinh thái năng lượng đóng đã kiểm chứng.

**Đề xuất kỹ thuật tối thiểu (đường ngắn nhất, không cần đổi burn):**

1. Thêm `BrainGenotype { weights: Vec<f32>, arch: ArchSpec }` vào genome cạnh `MorphologyGenotype`.
2. `decode_genotype` gắn `BrainWeights` component cho entity gốc (tuân thủ ADR-0001: development xảy ra
   **một lần** ở genesis/birth, ECS chỉ tiêu thụ phenotype).
3. `mutation.rs` thêm nhiễu Gauss lên `weights` với `sigma` là gen tự điều chỉnh (self-adaptive).
4. Inference: `BrainInferenceBuffer` **đã batch sẵn** — chuyển từ "1 model × N input" thành
   "N model × 1 input" bằng batched matmul thủ công. Với mạng ~5.5k tham số, đây là phép nhân ma trận
   nhỏ; **không cần burn cho đường inference**, chỉ cần `rayon` + buffer tiền cấp phát → **giữ được luật
   zero-alloc**.
5. `A2C` giữ lại nhưng chuyển sang **học trong đời** (xem B4), hoặc tắt sau cờ để so sánh đối chứng.

**Gate đề xuất (đo được, không cần chạy app):** sau N thế hệ, phương sai trọng số giữa các elite trong
archive > 0; `niche_divergence` tính trên **vector hành vi** (không chỉ khối lượng) tăng; và chạy hai
seed cho ra hai quần thể hành vi khác nhau nhưng **cùng seed thì trùng khớp** (gắn với C2).

### B2. Indirect encoding để giải bài toán brain–body — *Pilot*

Nếu làm B1 theo cách trực tiếp (vector trọng số phẳng), ta đâm vào đúng bức tường mà tài liệu co-design
2024–2025 mô tả: **hình thái đột biến → sơ đồ sensor/actuator đổi → chính sách MLP cố định hình dạng trở
nên giòn, buộc phải huấn luyện lại tốn kém**. Anima chắc chắn gặp vì `MorphologyGenotype` có số node/edge
biến thiên trong khi não là 15→64→4 cố định.

**Lời giải chuẩn trong tài liệu: mã hoá gián tiếp.** Trong CPPN, **mạng là genotype còn kết quả sinh ra
là phenotype**; HyperNEAT dùng CPPN nhận **toạ độ hai điểm** và xuất **trọng số kết nối giữa chúng**.
Vì trọng số là *hàm của toạ độ*, cùng một genome sinh được não cho **bất kỳ** body plan nào, ở **bất kỳ
độ phân giải nào** — đúng thứ Anima cần.

**Anima đã có sẵn nửa bài toán:** `MorphologyEdge.joint_anchor: Vec3` và vị trí segment cho ta **substrate
toạ độ** mà HyperNEAT đòi hỏi. Không phải bịa ra hệ toạ độ mới.

**Cảnh báo cần ghi vào ADR nếu chọn hướng này:** nghiên cứu 2025 *"Evolutionary Brain-Body Co-Optimization
Consistently Fails to Select for Morphological Potential"* chỉ ra co-optimization não–thân **thường xuyên
thất bại** trong việc chọn được hình thái có tiềm năng cao, vì hình thái tốt bị loại sớm khi não chưa kịp
thích nghi. Kèm theo là vấn đề **hội tụ sớm** trong co-optimization robot mềm. Đây là lý do **QD (MAP-Elites)
lại càng cần thiết** — archive giữ lại hình thái "hiện tại kém nhưng tiềm năng cao" mà chọn lọc thuần sẽ giết.
Nói cách khác: B2 và B3 phải đi cùng nhau, không tách rời.

### B3. MAP-Elites → CMA-MAE — *Pilot*

`map_elites.rs` hiện là MAP-Elites nguyên bản: bin cố định, thay thế khi fitness cao hơn, chọn cha mẹ ngẫu
nhiên/tournament.

Tài liệu QD đã chỉ ra ba giới hạn cụ thể của thế hệ CMA-ME và **CMA-MAE** (Covariance Matrix Adaptation
MAP-Annealing, GECCO 2023) được thiết kế để sửa đúng ba điểm đó: **bỏ mục tiêu quá sớm để đổi lấy khám phá,
vật lộn với mục tiêu phẳng, và hiệu năng kém khi archive độ phân giải thấp**. CMA-MAE đạt hiệu năng và độ
bền state-of-the-art.

Cả ba giới hạn đều áp vào Anima: fitness sinh thái **rất phẳng** ở giai đoạn đầu (agent chưa biết ăn thì
fitness ~0 khắp nơi), và archive hiện là lưới 2D thô.

**Đề xuất thực dụng:**
- **Adopt ngay (rẻ):** thêm chiều đo hành vi vào archive (hiện chỉ 2), và thay bin cố định bằng archive
  **không giới hạn/unstructured** khi đo hành vi — có tài liệu 2025 về QD đa mục tiêu trong không gian
  **phi cấu trúc và không chặn**.
- **Oracle (không nhúng runtime):** dùng **pyribs** (thư viện chuẩn của CMA-ME/CMA-MAE/CMA-MEGA) chạy
  ngoại tuyến trên **cùng bài toán benchmark** để biết CMA-MAE thắng MAP-Elites bao nhiêu **trước khi**
  viết lại bằng Rust. Đúng mô hình "oracle ngoại tuyến" mà `OPEN_SOURCE_LANDSCAPE.md` đã chọn cho khoa học sinh thái.

### B4. Giữ RL — nhưng đổi vai thành "học trong đời" — *Adopt*

Đừng vứt A2C. Đổi ngữ nghĩa của nó.

Tài liệu so sánh cho thấy neuroevolution **hội tụ nhanh hơn** nhưng RL huấn luyện đủ lâu có thể vượt về
điểm số; và QD/neuroevolution là **giải pháp cạnh tranh với RL cho khám phá kỹ năng**, vì chính sách deep RL
có xu hướng **overfit vào đặc tả nhiệm vụ cụ thể**.

Kiến trúc đề xuất — ba tầng, khớp với tam tầng đã có trong `CREATURE_MORPHOGENESIS.md`:

| Tầng | Cơ chế Anima hiện có | Vai trò sau nâng cấp |
|---|---|---|
| Tiến hoá (liên thế hệ) | mutation/crossover + MAP-Elites | Quyết định **trọng số khởi tạo** + cấu trúc não |
| Học trong đời (trong đời cá thể) | A2C + HRRL | Tinh chỉnh từ khởi tạo di truyền, **không chia sẻ giữa cá thể** |
| Phản xạ (mỗi tick) | CPG | Giữ nguyên |

Đây chính là **hiệu ứng Baldwin**: cái học được trong đời không di truyền, nhưng **khả năng học nhanh thì có**.
Nó cũng làm cho `HomeostaticState` (energy/hydration/temperature) trở thành reward **nội sinh** đúng nghĩa
thay vì reward do người thiết kế đặt — hợp với triết lý ALife hơn nhiều.

Cảnh báo chi phí: học trong đời cho N agent = N lần backward. Trên máy yếu, đặt sau cờ và **chỉ chạy cho agent
trong active-radius** của Simulation-LOD.

### B5. Tìm kiếm tự động không gian luật thế giới bằng foundation model (ASAL) — *Pilot, độ mới cao*

Đây là đề xuất có **tỉ lệ độc đáo/công sức tốt nhất** mà tôi tìm được, vì Anima đã có sẵn 3/4 mảnh ghép.

**ASAL** (*Automating the Search for Artificial Life with Foundation Models*, Sakana AI + MIT + OpenAI +
IDSIA + **Ken Stanley**, đăng trên tạp chí *Artificial Life* 2025) dùng **vision-language foundation model**
để tự động hoá việc **khám phá** ALife, bằng cách đặt ba bài toán tìm kiếm:

1. Tìm mô phỏng tạo ra **hiện tượng mục tiêu** đã mô tả bằng ngôn ngữ.
2. Tìm mô phỏng liên tục **sinh ra cái mới** (open-ended) — đo bằng độ mới của embedding theo thời gian.
3. **Chiếu sáng** toàn bộ không gian các mô phỏng khác nhau một cách thú vị.

Họ chứng minh trên Boids, Particle Life, Game of Life, Lenia, Neural CA — và tìm ra dạng sống Lenia/Boids
chưa từng thấy. Mã nguồn mở tại `SakanaAI/asal`.

**Anima đã có gì:**

| ASAL cần | Anima đã có |
|---|---|
| Không gian mô phỏng tham số hoá | `WorldLawSet` + experiment manifest (AE1) + `ExoticEnergy` (AE2) |
| Runner tái lập được | `experiment_runner.rs` + checkpoint fork + causal ledger |
| Ảnh render của mô phỏng | Landscape 3D + `map_manifest.json` + MCP `inspect_map_views` |
| Foundation model | `evolution/meta_ai.rs` (Gemini REST + web-session client) |

Nghĩa là mảnh còn thiếu chỉ là **hàm đo độ mới bằng embedding** và **vòng lặp tìm kiếm ngoài runtime**.

**Đề xuất phạm vi hẹp (không ôm đồm):** một harness **ngoại tuyến** đọc output của `experiment_runner`,
render N frame, lấy embedding, và tính **điểm open-endedness = độ mới theo thời gian**. Dùng nó để **xếp
hạng các `WorldLawSet`** thay vì để con người đoán. Điều này tôn trọng hard rule của ADR-0002
(`WorldLawSet` bất biến trong một run — ta *tìm kiếm giữa các run*, không đổi luật giữa chừng).

Rủi ro cần nêu thẳng: gọi Gemini là **phi tất định** và tốn quota; harness phải **cache embedding theo
checksum artifact** và nằm **ngoài** đường sim. Đây là lý do nó là *Pilot ngoại tuyến*, không phải *Adopt runtime*.

### B6. Đồng tiến hoá môi trường–agent — *Reference, mở đường sau*

**POET** sinh ra một tiến trình bất tận các môi trường đa dạng và ngày càng khó, **đồng thời** tối ưu lời giải
cho chúng; **Minimal Criterion Coevolution** (Brant & Stanley) đề xuất mô hình open-endedness thay thế trong đó
**bản thân môi trường phải thay đổi** mới tạo được động lực mở thật sự; **OMNI-EPIC** (ICLR 2025) dùng foundation
model để **tự sinh code đặc tả nhiệm vụ** tiếp theo vừa học được vừa thú vị.

Với Anima, điều này ánh xạ thành: **`Scenario` không nên là danh sách do người viết**, mà là quần thể tiến hoá
song song với quần thể sinh vật. Đây là đích xa; ghi lại để B5 được thiết kế sao cho không chặn đường (harness
xếp hạng world-law của B5 chính là **hàm đánh giá** mà POET/MCC cần).

Tham khảo danh mục cập nhật: `jennyzzt/awesome-open-ended`.

### B7. Thông lượng — bài học từ PufferLib/Neural MMO — *Reference*

PufferLib đạt **1–4 triệu step/giây** với PPO+LSTM nhờ vectorization tối ưu và hỗ trợ multi-agent nguyên
bản; Neural MMO báo cáo một lõi CPU hiện đại mô phỏng nhanh gấp **5.000 lần thời gian thực** trên mỗi agent.

Bài học áp dụng được **mà không cần nhập Python**:
1. **Batch inference là bắt buộc, không phải tối ưu.** Anima đã đúng hướng với `BrainInferenceBuffer`.
2. **Observation phải là SoA phẳng**, không phải struct per-agent — đã đúng (`agent_states: Vec<[f32; 15]>`).
3. Chi phí thật thường nằm ở **thu thập observation**, không phải ở matmul. Cần benchmark tách hai phần này
   trước khi tối ưu bất cứ thứ gì (`BENCHMARK_BASELINE.md` là chỗ đặt số).

---

## 4. Phần C — Nền tảng

### C1. Nợ phiên bản — *Adopt, theo bậc*

| Dep | Anima | Hiện tại | Khoảng cách |
|---|---|---|---|
| `burn` | 0.13.2 | **0.21.0** (07/05/2026) | 8 minor |
| `bevy_ecs` | 0.13.0 | **0.19.0** (19/06/2026) | 6 minor |

Rủi ro cụ thể đã xác định, không phải suy đoán chung chung:

- **Bevy 0.19 biến `Resource` thành subtrait của `Component`**, và `#[derive(Resource)]` implement cả hai;
  không còn derive đôi được nữa. Anima có `impl bevy_ecs::system::Resource for BrainModel {}` **thủ công**
  (`ai/model.rs:70`) → gần như chắc chắn vỡ.
- `unsafe impl Send for BrainModel` / `unsafe impl Sync for BrainModel` (`ai/model.rs:67-68`) là cách lách
  quanh việc `burn-wgpu` không `Send`/`Sync`. Đây là **unsafe không có chứng minh an toàn** — burn mới có thể
  đã sửa gốc vấn đề này, khiến upgrade vừa là dọn nợ vừa là bỏ được unsafe.
- Bevy ghi nhận resources-as-components từng gây **hồi quy hiệu năng do indirection khi tra cứu**, đã sửa trước
  release nhưng là chỗ cần đo đầu tiên khi migrate.

**Thứ tự đề xuất:** `burn` trước (biên hẹp: chỉ `ai/model.rs` + đoạn train trong `simulation_loop.rs`), rồi
`bevy_ecs` sau (biên rộng: mọi system). Mỗi bậc **một ADR + một PR**, không gộp với thay đổi luật mô phỏng —
đúng nguyên tắc "không nâng đồng thời Bevy, artifact format và luật mô phỏng" đã ghi trong khảo sát nguồn mở.

**Lưu ý B1 làm giảm rủi ro này:** nếu inference chuyển sang matmul thủ công (B1 bước 4), phụ thuộc `burn`
co lại chỉ còn đường **training** — biên migrate nhỏ hơn hẳn.

### C2. Sửa determinism ở tầng live — ✅ **ĐÃ LÀM (2026-07-25)**

Tài liệu dự án đã tuyên bố nguyên tắc rõ ràng — `exotic_energy.rs:19` và `experiment_runner.rs:7` đều ghi
"**never `thread_rng()`**" và dùng `StdRng` có seed. Nhưng đường sim sống thì chưa theo:

```
core/agent_systems.rs:187        core/world_systems.rs:155
core/environmental_systems.rs:134 core/world_systems.rs:526
core/simulation_loop.rs:877      evolution/crossover.rs:45
evolution/mutation.rs:52         evolution/map_elites.rs:57
```

`map_elites.rs:57` đặc biệt đáng lưu ý vì `select_parent` được gọi **hai lần mỗi lần sinh sản**
(`simulation_loop.rs:376-377`) → chọn cha mẹ hiện là ngẫu nhiên không tái lập.

**Đã triển khai.** `core/resources.rs` có `SimRng` (resource, `StdRng` + seed, đọc `ANIMA_SIM_SEED`,
mặc định `DEFAULT_SIM_SEED = 1337`) và `derived_rng(stream)` cho code không phải Bevy system. Bốn system
nhận `ResMut<SimRng>`; ba hàm tiến hoá (`mutate_genotype`, `crossover_genotypes`, `select_parent`) nhận
`&mut impl Rng`. Cả 8 điểm `thread_rng()` đã biến mất khỏi `src/`.

**Một lỗi thứ hai phát hiện khi sửa, cùng vị trí.** `MapElitesArchive.grid` là `HashMap`, mà `RandomState`
của Rust **tự gieo hạt theo tiến trình** → thứ tự duyệt đổi mỗi lần chạy. `select_parent` duyệt collection
này, nên **kể cả có RNG seed thì chọn cha mẹ vẫn không tái lập**. Đã đổi sang `BTreeMap` (khoá `(i32,i32)`
đã `Ord`; toàn bộ API đang dùng — `len`/`get`/`insert`/`iter`/`clear` — giữ nguyên).

**Vì sao có nhiều stream chứ không phải một.** World setup, Bevy schedule và luồng tiến hoá chạy **đồng
thời**; một stream chung sẽ khiến kết quả phụ thuộc thứ tự lập lịch thread — đúng thứ đang muốn loại bỏ.
Mỗi nơi lấy một sub-stream dẫn xuất (`sim_stream::WORLD_INIT`, `sim_stream::EVOLUTION`).

**Gate:** `tests/sim_determinism_tests.rs` — 11 test, gồm một test **quét mã nguồn** và fail nếu
`thread_rng()` quay lại (bỏ qua dòng comment).

**Còn nợ:** save/load chưa mang seed lẫn vị trí draw, nên nạp lại một run đã lưu **chưa** tiếp tục đúng
chuỗi ngẫu nhiên. `SimRng::reseed` đã có sẵn cho việc này; wiring vào `SavedSimulationState` là bước riêng.

### C3. Đường GPU compute — *Defer, ghi nhận*

Anima **đã** kéo `wgpu` vào qua `burn-wgpu`. Nghĩa là một đường compute shader cho phần va chạm/láng giềng
**không thêm dependency mới**. Tham chiếu: **RDPE** — framework khai báo, mô phỏng **thường trú trên GPU**, truy
vấn láng giềng **O(N) bằng spatial hashing**, biên dịch luật thành **một compute shader duy nhất**, chạy ở mọi
nơi wgpu chạy (desktop, Raspberry Pi, trình duyệt qua WebGPU).

Nhưng **Defer**, vì: (a) `physics/spatial.rs` hiện đã có spatial hash CPU và chưa có benchmark chứng minh nó là
nút thắt; (b) chuyển state lên GPU xung đột trực tiếp với luật zero-alloc + determinism đang phải sửa ở C2;
(c) máy phát triển hiện tại không verify được. Mở lại khi C2 xong và `BENCHMARK_BASELINE.md` có số chỉ đích danh.

---

## 5. Thứ tự đề xuất

Sắp theo **đòn bẩy ÷ rủi ro**, có tính tới ràng buộc "verify được trên máy yếu".

| Bậc | Việc | Vì sao trước | Verify headless? |
|---|---|---|---|
| **1** | **C2** — RNG có seed | Chặn mọi so sánh đối chứng của B1; refactor cơ học, rủi ro thấp nhất trong danh sách | ✅ cargo test |
| **2** | **B1** — não thành gen per-agent **+ mở rộng không gian hành động** | Đòn bẩy cao nhất; mở khoá tiến hoá hành vi, làm MAP-Elites/E11/S43 có nghĩa. Hai phần phải đi cùng — não riêng mà chỉ xuất 4 tham số CPG thì không quan sát được đa dạng | ✅ cargo test |
| **3** | **A1** — pass tectonics | Sửa **gốc** 3 khoảng trống §2 cùng lúc; thuần hàm | ✅ smoke + vitest |
| **4** | **A3** — gió 2 trục + taxonomy Whittaker/Köppen | Ăn theo A1; thống nhất 11↔22 biome | ✅ |
| **5** | **C1a** — nâng `burn` | Biên hẹp lại sau B1; bỏ được `unsafe impl Send/Sync` | ✅ cargo |
| **6** | **B3** — thêm chiều hành vi vào archive (+ pyribs oracle) | Cần B1 mới có hành vi để đo | ✅ |
| **7** | **A2b** — hoa văn erosion thủ tục | Lợi ích thị giác lớn nhất/chi phí, không đổi data | ⚠️ cần nhìn |
| **8** | **B4** — A2C thành học-trong-đời (sau cờ) | Cần B1 xong trước | ✅ |
| **9** | **B5** — harness ASAL ngoại tuyến | Cần AE runner ổn định + có ảnh render | ⚠️ cần quota API |
| **10** | **C1b** — nâng `bevy_ecs` | Biên rộng nhất; làm khi các bậc trên đã ổn định | ✅ cargo |
| Sau | A5 (SDF cave), B2 (CPPN), B6 (POET), A4 (ML terrain), C3 (GPU compute) | Phụ thuộc bậc trên hoặc cần máy mạnh | hỗn hợp |

---

## 6. Những gì tôi khuyên KHÔNG làm

- **Không** nhúng model neural vào đường sinh `elevation` mà sim đọc (phá cam kết tái lập của `WorldArtifact`).
- **Không** chuyển sang mô phỏng mảng kiến tạo đầy đủ theo thời gian địa chất — Anima cần *kết quả* của kiến tạo,
  không cần *quá trình*. Voronoi + uplift biên là đủ.
- **Không** nâng `burn` và `bevy_ecs` trong cùng một PR, và không gộp với B1.
- **Không** thay CPG bằng chính sách học — CPG là tầng phản xạ đúng đắn và rẻ; tài liệu JaxLife còn lập luận rằng
  điều khiển cấp thấp **có thể không cần thiết** để tiến hoá lập luận cấp cao.
- **Không** đưa Python (pyribs, Landlab, Virtual Ecosystem) vào runtime — chỉ dùng làm **oracle ngoại tuyến** đúng
  như chính sách đã chốt.
- **Không** coi một hình thái mới hay một ô MAP-Elites mới là "loài mới" — gate AE-S11/AE-S14 vẫn áp dụng cho mọi
  tuyên bố sinh ra từ B1/B3.

---

## 7. Câu hỏi mở cần chốt trước khi triển khai

1. **B1 — kiểu mã hoá não:** vector trọng số phẳng (đơn giản, đâm vào bức tường brain–body) hay CPPN/HyperNEAT
   ngay từ đầu (đúng hơn, tốn công hơn)? Khuyến nghị: **phẳng trước để có bằng chứng, CPPN sau** — vì bức tường
   brain–body chỉ thực sự đau khi hình thái đa dạng cao, mà hiện tại thì chưa.
2. **B4 — có giữ RL không?** Nếu bỏ hẳn (như JaxLife/Bibites/arXiv 2510.18221 đều làm) thì đơn giản hơn nhiều và
   phụ thuộc `burn` gần như biến mất. Cần quyết định vì nó đổi cả C1.
3. **A1 — có bump `WORLD_GEN_VERSION` một lần cho cả A1+A3 không?** Gộp thì cache chỉ invalidate một lần.

---

## Nguồn

**Map / worldgen**
- [WorldEngine (PyPlatec, Holdridge)](https://github.com/esampson/worldengine) · [World Orogen](https://www.orogen.studio/) · [Gleba](https://calandiel.itch.io/gleba) · [realistic-planet-generation-and-simulation](https://github.com/FreezeDriedMangos/realistic-planet-generation-and-simulation) · [WorldMachina](https://github.com/SAED2906/WorldMachina)
- [Physically-based analytical erosion for fast terrain generation (INRIA, 2024)](https://www-sop.inria.fr/reves/Basilic/2024/TGSC24/Analytical_Terrains_EG.pdf) · [Real-time Terrain Enhancement with Controlled Procedural Patterns (CGF 2024)](https://onlinelibrary.wiley.com/doi/10.1111/cgf.14992) · [Interactive Hydraulic Erosion Simulator (GPU)](https://huw-man.github.io/Interactive-Erosion-Simulator-on-GPU/)
- [Terrain Diffusion Network (climatic-aware)](https://arxiv.org/pdf/2308.16725) · [StyleDEM](https://arxiv.org/pdf/2304.09626) · [Multi-theme GAN terrain amplification (ToG)](https://dl.acm.org/doi/10.1145/3355089.3356553) · [DEM super-resolution framework (2024)](https://www.tandfonline.com/doi/full/10.1080/17538947.2024.2356121)
- [Climate Modeling 101](https://medium.com/universe-factory/climate-modeling-101-4544e00a2ff2) · [An Apple Pie from Scratch — Biomes and Climate Zones](https://worldbuildingpasta.blogspot.com/2020/05/an-apple-pie-from-scratch-part-vib.html)
- [bevy-sculpter (SDF + Surface Nets)](https://crates.io/crates/bevy-sculpter) · [bevy_voxel_world](https://github.com/splashdust/bevy_voxel_world) · [godot_voxel](https://github.com/Zylann/godot_voxel)

**Mô hình machine / ALife**
- [JaxLife: An Open-Ended Agentic Simulator (ALIFE 2024)](https://arxiv.org/abs/2409.00853) · [mã nguồn](https://github.com/luchris429/jaxlife)
- [The Emergence of Complex Behavior in Large-Scale Ecological Environments (2025)](https://arxiv.org/pdf/2510.18221)
- [ASAL — Automating the Search for Artificial Life with Foundation Models (Artificial Life 31(3), 2025)](https://direct.mit.edu/artl/article/31/3/368/132866/Automating-the-Search-for-Artificial-Life-With) · [trang dự án](https://sakana.ai/asal/) · [mã nguồn](https://github.com/sakanaai/asal) · [thông báo trên X](https://x.com/SakanaAILabs/status/1871385917342265592)
- [The Bibites — Brain (wiki)](https://the-bibites.fandom.com/wiki/Brain) · [trang chính thức](https://www.thebibites.com/)
- [ALIEN — CUDA artificial life (chrxh)](https://github.com/chrxh/alien)
- [CMA-MAE (GECCO 2023)](https://dl.acm.org/doi/10.1145/3583131.3590389) · [pyribs](https://github.com/icaros-usc/pyribs) · [Multi-Objective QD in Unstructured and Unbounded Spaces (2025)](https://arxiv.org/pdf/2504.03715) · [danh mục paper QD](https://quality-diversity.github.io/papers.html)
- [Neuroevolution is a Competitive Alternative to RL for Skill Discovery](https://arxiv.org/abs/2210.03516) · [Co-evolving morphology and control using a single genome](https://arxiv.org/pdf/2212.11517) · [Evolutionary Brain-Body Co-Optimization Consistently Fails to Select for Morphological Potential (2025)](https://arxiv.org/pdf/2508.17464) · [Premature Convergence in Co-optimization of Morphology and Control](https://arxiv.org/pdf/2402.09231)
- [POET (GECCO 2019)](https://arxiv.org/pdf/1901.01753) · [Enhanced POET](https://arxiv.org/pdf/2003.08536) · [OMNI-EPIC (ICLR 2025)](https://arxiv.org/abs/2405.15568) · [awesome-open-ended](https://github.com/jennyzzt/awesome-open-ended)
- [Neural MMO 2.0](https://arxiv.org/pdf/2311.03736) · [Massively Multiagent Minigames (PufferLib)](https://arxiv.org/pdf/2406.05071)
- [Evolving Neural Networks Reveal Emergent Collective Behavior from Minimal Agent Interactions](https://arxiv.org/html/2410.19718v1)

**Nền tảng**
- [burn trên crates.io](https://crates.io/crates/burn) · [Burn (GitHub)](https://github.com/tracel-ai/burn) · [Bevy 0.19](https://bevy.org/news/bevy-0-19/) · [Migration Guide 0.18 → 0.19](https://bevy.org/learn/migration-guides/0-18-to-0-19/) · [RDPE — GPU-resident declarative simulation trên wgpu](https://crates.io/crates/rdpe)
