---
title: Bản nháp Claude — Creature Morphogenesis
status: superseded
archive_state: do-not-use
owner: architecture
last_reviewed: 2026-07-24
superseded_by: ../explanation/CREATURE_MORPHOGENESIS.md
---

# CREATURE_MORPHOGENESIS.md — Tạo hình sinh vật theo môi trường (Ecomorphology)

> **Không dùng bản này để triển khai.** Đây là bản nháp lịch sử của Claude, được giữ
> để truy nguyên quyết định. Nguồn đang có hiệu lực là
> [`docs/explanation/CREATURE_MORPHOGENESIS.md`](../explanation/CREATURE_MORPHOGENESIS.md),
> contract bắt buộc là
> [`docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md),
> và kế hoạch thi công là
> [`docs/planning/CREATURE_MORPHOGENESIS_PLAN.md`](../planning/CREATURE_MORPHOGENESIS_PLAN.md).

> Nghiên cứu thiết kế: **khi một sinh vật mới xuất hiện, môi trường và các yếu tố sinh thái nên định hình ngoại hình + đặc điểm của nó như thế nào.**
>
> Tài liệu này là phần đào sâu (deep-dive) cho các milestone **M5 (Vòng đời & sinh lý động vật)** và **M7 (Hành vi, sinh sản & tiến hóa gắn với môi trường)** trong [`WORLD_SIMULATION_PLAN.md`](WORLD_SIMULATION_PLAN.md). Nó bám vào hiện trạng code thật của engine (trích dẫn `file:line`) và các hợp đồng cứng trong [`SIMULATION_RULES.md`](SIMULATION_RULES.md), [`BIOME_TAXONOMY.md`](BIOME_TAXONOMY.md), [`COORDINATE_CONTRACT.md`](COORDINATE_CONTRACT.md).

---

## 0. Trả lời ngắn (TL;DR)

Không nên "tạo hình sẵn" con vật cho khớp môi trường bằng cách chỉnh thẳng chỉ số cuối — điều đó vi phạm nguyên tắc **"no magic effects"** (§2.2 của WORLD_SIMULATION_PLAN). Cách đúng về mặt sinh học **và** khớp kiến trúc engine là một mô hình **3 tầng**, phản chiếu đúng 3 cơ chế thật của tự nhiên:

| Tầng | Cơ chế sinh học | Thang thời gian | Vai trò | Trạng thái trong engine |
|---|---|---|---|---|
| **1. Chọn lọc (Selection)** | Môi trường lọc cá thể nào sống sót để sinh sản → phân bố gene hội tụ về biome | Nhiều thế hệ | "Thích nghi thật" (adaptation) | **Đã có một phần**: metabolism MTE + MAP-Elites theo trục sinh thái |
| **2. Mềm dẻo phát triển (Developmental plasticity)** | Cùng một genome cho ra phenotype khác nhau tùy nơi lớn lên (norm of reaction / acclimatization) | Một đời cá thể | Ghép nối môi trường→ngoại hình **tức thì, nhìn thấy được** | **Chưa có** |
| **3. Khởi tạo có điều kiện môi trường (Env-conditioned genesis)** | "Warm start" theo địa sinh học — hạt giống loài mới bắt đầu gần một đỉnh thích nghi hợp lý | Tại thời điểm gieo de-novo | Prior/thiên lệch mềm, **không** ghi đè chỉ số cuối | **Chưa có** |

Ngoại hình (body shape, size, proportions, limbs) đã nằm trong genome hình thái hiện tại → chỉ cần **nối dây** với môi trường. Màu sắc (pigmentation) và một số đặc điểm sinh lý (thermal tolerance, hydration efficiency, diet breadth) **chưa được mô hình hóa** → cần thêm trait mới. Phần "công thức cụ thể" ở §5 dùng đúng hằng số engine hiện tại.

**Nguyên tắc vàng:** Tầng 3 chỉ là *điểm xuất phát*; chỉ Tầng 1 mới biến nó thành thích nghi thật (gate **S43** Red-Queen: một trait chỉ được coi là thích nghi khi cải thiện survival/reproduction qua nhiều seed, không chỉ tăng một fitness proxy). Đừng nhầm "sinh ra đã đẹp" với "thích nghi".

---

## 1. Câu hỏi & phạm vi

### "Một sinh vật mới xuất hiện" là sự kiện nào?

Trong engine hiện có **3 con đường** một cá thể ra đời, đều đi qua `decode_genotype` ([`evolution/genotype.rs:50`](src-tauri/src/evolution/genotype.rs)):

1. **Gieo de-novo (genesis)** — quần thể khởi đầu, hoặc người dùng thả một loài mới vào thế giới. *Đây là nơi câu hỏi "tạo hình cho khớp môi trường" mạnh nhất*, vì chưa có tổ tiên để thừa kế.
2. **Sinh sản (birth/reproduction)** — con cái thừa kế genome cha mẹ + mutation ([`simulation_loop.rs:375`](src-tauri/src/core/simulation_loop.rs)). Môi trường tác động **gián tiếp qua chọn lọc**.
3. **Di cư (migration)** — cá thể nhập cư từ shard khác, mang genotype nguyên vẹn ([`world_systems.rs:603`](src-tauri/src/core/world_systems.rs)).

Câu hỏi của bạn phủ cả (1) và (2). Tài liệu tách bạch hai kịch bản vì chúng cần cơ chế khác nhau (§4).

### "Định hình bởi môi trường" gồm những gì?

- **Ngoại hình (appearance):** kiểu cơ thể (body plan / topology), kích thước, tỉ lệ các đoạn, số chi/phần phụ, **màu sắc**.
- **Đặc điểm (characteristics):** sinh lý (ngưỡng chịu nhiệt, hiệu suất giữ nước, tốc độ chuyển hóa), chế độ ăn/guild, chiến lược vòng đời, tầm cảm nhận.

---

## 2. Hiện trạng engine (ground truth) — ta có gì để làm việc

### 2.1. Một "con vật" là gì trong code

Một sinh vật **không phải một entity** mà là **một cơ thể khớp nối nhiều entity**: một `Agent` gốc + N `Segment` con nối bằng khớp vật lý. Genome là **đồ thị** `MorphologyGenotype` ([`genotype.rs:4`](src-tauri/src/evolution/genotype.rs)):

```rust
pub struct MorphologyNode { pub id: u32, pub length: f32, pub radius: f32, pub mass: f32 }   // 1 "gene" = 1 đoạn cơ thể
pub struct MorphologyEdge { pub source_node: u32, pub target_node: u32,
                            pub joint_anchor: Vec3, pub joint_axis: Vec3 }                     // 1 khớp
pub struct MorphologyGenotype { pub nodes: Vec<MorphologyNode>, pub edges: Vec<MorphologyEdge> }
```

`decode_genotype` BFS đồ thị → mỗi node thành một `Segment` entity, mỗi edge thành `JointConstraint` + `JointAxis` + `CpgOscillator` (chỉ node con mới có CPG → cá thể 1-node không tự cử động được).

### 2.2. Cái gì đã được mô hình hóa, cái gì chưa

| Đặc điểm | Trạng thái | Ghi chú |
|---|---|---|
| Kích thước, tỉ lệ, số đoạn, topology | ✅ Có (genome) | `length`, `radius`, `mass`, đồ thị node/edge |
| Khối lượng tổng `total_mass()` | ✅ Có | "Master trait" theo MTE — 1 trong 2 trục MAP-Elites |
| Năng lượng / nước / nhiệt độ cơ thể | ✅ Có (runtime) | `HomeostaticState` ([`ai/hrrl.rs:4`](src-tauri/src/ai/hrrl.rs)), **cứng** lúc spawn: `energy 100, hydration 100, temperature 37, temp_target 37` |
| Guild ăn (predator/prey) | ⚠️ Ngầm định | Chỉ là marker `Predator`/`Prey`, quyết định bằng **index** lúc spawn, không phải gene |
| Chuyển hóa (metabolism) | ✅ Emergent | Suy ra từ mass + body-temp (Kleiber + Arrhenius), **không** phải gene |
| Tốc độ (speed) | ✅ Emergent | Từ mass + hình học + CPG, chỉ là *chỉ số tính ra* trong `AgentEpochStats` |
| **Màu sắc / ngoại hình nhìn thấy** | ❌ Không có | Không có gene color. Frontend render agent sống bằng **tam giác/tròn 2D**, tô theo `agent_type`+`energy` |
| **Ngưỡng chịu nhiệt / hiệu suất nước** | ❌ Không có | Mọi cá thể `temp_target = 37` cố định |
| **Tầm cảm nhận (sensory range)** | ❌ Không có | Raycast **cứng = 10.0**; olfactory sensor định nghĩa nhưng **không bao giờ gắn** → luôn đọc 0 |
| **Bộ não di truyền** | ❌ Không có | Brain là **một MLP actor-critic dùng chung toàn cục** (15→64→4), không per-agent, không có neural genome |
| Vòng đời (tuổi, giai đoạn, giới tính) | ❌ Không có | Gap đã ghi trong WORLD_SIMULATION_PLAN §1.2 |

> **Hệ quả quan trọng:** genome hiện tại **chỉ mã hóa hình thái**. Muốn môi trường định hình *màu sắc* và *đặc điểm sinh lý*, phải **mở rộng genome/phenotype** thêm trait mới — đây là công việc thiết kế, không chỉ nối dây.

### 2.3. Môi trường: giàu dữ liệu nhưng chưa nối vào lúc sinh ra

Engine đã tính và lưu **trường môi trường theo từng ô**, truy vấn được tại mọi vị trí `(x,z)`:

| Biến | Field (`TerrainMap`, [`terrain.rs:116`](src-tauri/src/core/terrain.rs)) | Đơn vị / miền |
|---|---|---|
| Độ cao | `elevations` | `[0,1]` → `y = value·10` |
| Độ ẩm | `moistures` | `[0,1]` |
| Nhiệt độ trường | `temperatures` | `[0,1]` (1 = xích đạo nóng, 0 = cực lạnh) |
| Biome | `biomes` (u8) | 11 legacy backend / 22 canonical frontend |
| Dòng chảy | `flows` | `[0,1]` (tích lũy dòng sông) |

Cộng với **trường tài nguyên NPP** `ResourceField` ([`ecology.rs:269`](src-tauri/src/core/ecology.rs)): `r` (sinh khối đứng hiện tại/ô), `r_max` (sức chứa/ô = `biome_npp · 0.01`); `SeasonClock` (1 năm = 6000 tick); `EnvironmentalEvent` (Drought/TempSpike/Glacial/ToxicDeluge).

**NPP theo biome** ([`ecology.rs:62`](src-tauri/src/core/ecology.rs), g/m²/năm): Rainforest 2200 · TemperateForest 1200 · BorealForest 800 · Grassland 700 · River 500 · Ocean 250 · Snow 140 · DeepOcean 125 · Desert 90 · Beach 60 · MountainRock 50. → Ô rừng mưa chứa được **~44×** năng lượng so với ô núi đá.

> **Khoảng trống cốt lõi:** đường spawn ([`simulation_loop.rs:757`](src-tauri/src/core/simulation_loop.rs)) tạo **10 cá thể y hệt nhau**, cùng một genotype 3-đoạn cứng, đặt thẳng hàng tại `(i·5, 0, 0)`, 7 đầu Prey / 3 cuối Predator. **Không đọc một biến môi trường nào.** Môi trường chỉ được đọc *ngay sau đó* để đặt hồ và cây — không phải để tạo hình sinh vật.
>
> Nhưng lúc **runtime** thì môi trường ĐÃ ảnh hưởng: metabolism đọc body-mass + body-temp; grazing/fruiting đọc NPP theo ô; MAP-Elites chọn cha mẹ theo trục sinh thái `body_mass × foraging_range`. Nghĩa là **bề mặt chọn lọc đã mang tính sinh thái, chỉ có *cơ thể khởi đầu* là generic.**

### 2.4. Ràng buộc cứng phải tôn trọng

Bất kỳ hệ tạo hình nào cũng phải qua các hợp đồng đã machine-check:

- **Năng lượng là pool ĐÓNG "EU"** (biomass-equivalent). Cơ thể/năng lượng của con mới **không được sinh ra từ hư không** — phải trừ vào sổ `EcosystemBiomass { detritus, plants, animals }` (test **S01**). Con non ra đời phải lấy năng lượng từ dự trữ cha mẹ; xác chết → `detritus`.
- **Hai loại nhiệt độ không được trộn:** `field_temperature` chuẩn hóa `[0,1]` (địa lý) ≠ `body_temperature` `°C` miền `[30,45]` (nội môi). Chuyển đổi phải tường minh.
- **Pool nước/dưỡng chất/độc là `DeferredM3`** — khai báo nhưng **chưa bảo toàn**. Trước M3, không được viết cơ chế giả định chúng đã đóng.
- **Tọa độ:** sinh vật sống trong world-space `x,z ∈ [-100,100]`, `y = elevation·10`. Hỏi "con này ở biome/ô nào" phải dùng **cell-bucket** `floor(coord·dim)` (`world_xz_to_cell` / `get_map_indices`), trả `None` khi ra ngoài biên; hỏi "mặt đất cao bao nhiêu để đứng" dùng **bilinear** `get_elevation_at_pos`. Trộn hai cái → lệch nửa ô ở rìa.
- **Taxonomy 22 biome** là nguồn chân lý; thêm biome liên quan sinh vật phải cập nhật cả 2 map 22↔11 + chạy lại **S02**.
- **Hot-loop 0 heap-allocation** — nhưng *spawn không phải hot loop*, cho phép cấp phát lúc tạo cá thể. Việc phải giữ zero-alloc là các system tick (physics/CPG/collision).
- **Test cần đạt:** **S27** (loài cạn không spawn giữa hồ, loài nước không spawn trên núi) · **S40** (mate/nest phải thỏa habitat) · **S41** (inheritance/mutation có bounds) · **S43** (Red-Queen coevolution nhiều seed).

---

## 3. Nguyên lý sinh học: môi trường định hình sinh vật thế nào

Đây là "thư viện quy luật" ecomorphology — mỗi quy luật gắn với **một biến môi trường engine đã có** và **một đòn bẩy phenotype**. Chúng đều là quy luật đã được kiểm chứng trong sinh học.

1. **Quy luật Bergmann** — khí hậu lạnh → cơ thể **to hơn** (tỉ lệ thể tích/diện tích lớn → giữ nhiệt tốt); nóng → nhỏ hơn. → `field_temperature` ↦ `mass`.
2. **Quy luật Allen** — lạnh → **chi/phần phụ ngắn, dày** (giảm diện tích mất nhiệt: cáo Bắc Cực tai ngắn); nóng → **dài, mảnh** (tản nhiệt: cáo fennec tai to). → `field_temperature` ↦ tỉ lệ `length/radius` của đoạn ngoại vi.
3. **Quy luật Gloger / hắc tố nhiệt (thermal melanism)** — vùng ẩm/nóng → sắc tố **đậm hơn**; khô/lạnh → **nhạt**. Ở nơi lạnh nhưng nhiều nắng (núi cao) → đậm để hấp thụ nhiệt. → `moisture` + `field_temperature` ↦ màu.
4. **Năng suất (NPP) → kích thước & mật độ** — nơi giàu (rừng mưa) nuôi được cơ thể **to** và mật độ cao; nơi nghèo (sa mạc) → cơ thể **nhỏ, tiết kiệm năng lượng**, chuyển hóa chậm (island rule: đảo nghèo → dwarfism). → `r_max` (NPP) ↦ ngân sách `total_mass`.
5. **Môi trường vận động (locomotion medium)** — nước → thân **thuôn dài (fusiform), ít cản**, chi thành vây; đào hang → thân trụ, chi tiêu giảm; cạn đồng bằng → chân dài chạy nhanh (cursorial); đá dốc/núi cao → thân thấp, bám tốt, nhảy. → `biome`/`elevation`/`flow` ↦ **topology body plan** (cách sắp xếp node/edge, joint_axis). *Đây chính là S27.*
6. **Ngụy trang (crypsis)** — màu khớp nền biome (sa mạc = cát, rừng = lốm đốm nâu-lục, tuyết = trắng). → màu nền biome ↦ màu cơ thể.
7. **Kinh tế nước (water economy)** — khô → giữ nước: giảm mất nước qua bay hơi, dự trữ mỡ, giảm diện tích bề mặt. → `moisture` ↦ trait "hiệu suất giữ nước" (giảm tốc độ mất hydration).
8. **Độ dốc/gồ ghề địa hình → tỉ lệ chi & dáng đi** — dốc → chân ngắn khỏe, bám (dê núi); phẳng → chân dài chạy. → `slope`/`elevation` ↦ tỉ lệ chi.
9. **Tính mùa (seasonality) → chiến lược vòng đời** — mùa gắt → sinh sản theo mùa đồng bộ đỉnh tài nguyên, tích mỡ, di cư/ngủ đông. → biên độ mùa ↦ chiến lược sinh sản, dự trữ năng lượng.
10. **Bậc dinh dưỡng (trophic) & kích thước con mồi** — NPP + sinh khối con mồi quyết định niche nuôi được thú ăn thịt hay ăn cỏ; kích thước predator co giãn theo kích thước prey. → NPP + mật độ prey ↦ guild + diet breadth + tỉ lệ kích thước.

---

## 4. Kiến trúc đề xuất: 3 tầng morphogenesis

### Tầng 1 — Chọn lọc (emergent, đúng chất ALife)

Đây là **lõi trung thực nhất**: đừng thiết kế con vật, hãy để môi trường *lọc* nó. Cơ chế đã có sẵn phần lớn:
- Cơ thể nặng/không hiệu quả → chi phí chuyển hóa cao (`metabolic_rate = 0.06 · M^0.75 · e^{E/k(1/T_ref − 1/T)}`, [`ecology.rs:42`](src-tauri/src/core/ecology.rs)) → chết sớm nếu môi trường không nuôi nổi.
- MAP-Elites chọn cha mẹ theo `body_mass × foraging_range` → áp lực sinh thái thật.

**Việc cần làm:** *siết* vòng lặp này cho gắn môi trường cục bộ hơn (metabolism đã đọc body-temp, nhưng body-temp chưa bị `field_temperature` cục bộ kéo — xem §7), và thêm gate **S43** để chứng minh trait hội tụ là thích nghi thật qua nhiều seed. **Đây là "thích nghi" theo nghĩa chặt.**

### Tầng 2 — Mềm dẻo phát triển (plasticity, hiệu ứng nhìn-thấy tức thì)

Cùng một genome, khi `decode` tại nơi có môi trường khác nhau, cho ra **phenotype khác nhau** (norm of reaction). Ví dụ: con sinh ra ở ô lạnh → lông dày hơn/thân to hơn *dù gene giống hệt anh em nó ở ô ấm*. Đây là **acclimatization** có thật trong sinh học, và là cách **rẻ nhất để có ngay ghép nối môi trường→ngoại hình** mà không phải chờ tiến hóa.

Điểm nối: một hàm `develop(genotype, local_env) → phenotype` áp *trong* `decode_genotype`, điều biến (modulate) `length/radius/mass/color` của phenotype theo `local_env`, **không đụng vào genome** (nên không di truyền — đúng bản chất plasticity).

### Tầng 3 — Khởi tạo có điều kiện môi trường (genesis prior)

Chỉ áp cho **gieo de-novo** (loài mới, chưa có tổ tiên). Thay vì genotype cứng, **lấy mẫu** genotype khởi đầu từ một **phân bố có điều kiện biome** để hạt giống bắt đầu gần một đỉnh thích nghi hợp lý (không chết ngay). Đây là **prior/thiên lệch mềm** — một *phân bố*, có nhiễu ngẫu nhiên — **không phải** ghi đè chỉ số cuối. Nó tương đương "địa sinh học hợp lý", không phải phép màu, **miễn là** sống sót sau đó vẫn phụ thuộc cơ chế (Tầng 1).

> **Ranh giới đạo đức thiết kế (bám §2.2 "no magic effects"):** Tầng 3 được phép vì nó chỉ đặt *điểm xuất phát* và truyền qua cơ chế trung gian (mass→metabolism→sống/chết). Cái **bị cấm** là: thấy con ở sa mạc thì cộng thẳng `+survival` hay set cứng `is_adapted = true`. Thích nghi phải *thắng ra* ở Tầng 1, không được *tuyên bố* ở Tầng 3.

---

## 5. Bảng ánh xạ cụ thể: biến môi trường → đòn bẩy hình thái

Đây là phần "công thức". Miền giá trị và hằng số lấy đúng từ engine hiện tại. Các công thức là *đề xuất khởi điểm* (cần calibrate bằng benchmark S-tests), không phải hằng số thiêng.

| Biến môi trường (field, miền) | Quy luật | Đặc điểm bị ảnh hưởng | Đòn bẩy engine | Hướng công thức khởi điểm |
|---|---|---|---|---|
| `field_temperature` `[0,1]` | Bergmann | Kích thước cơ thể | `node.mass`, `radius` | `mass_scale = 1 + k_B·(1 − temp)`, ví dụ `k_B≈0.6` → cực lạnh to gấp ~1.6× (giữ trong clamp mass `0.05..10`) |
| `field_temperature` `[0,1]` | Allen | Tỉ lệ chi ngoại vi | `length/radius` của node lá | `slenderness = 0.5 + 1.5·temp` → nóng: dài mảnh; lạnh: ngắn dày (giữ `length 0.1..5`, `radius 0.05..1`) |
| `field_temperature` → °C | Thermal niche | Ngưỡng nhiệt cơ thể | **trait mới** `temp_target` (thay vì 37 cứng) | `temp_target_°C = 33 + 9·temp` rồi clamp `[30,45]`; đặt cả "dải chịu" `±tolerance` |
| `moisture` `[0,1]` | Water economy | Hiệu suất giữ nước | **trait mới** `hydration_loss_mult` | `loss_mult = 0.6 + 0.8·moisture` → khô: mất nước chậm hơn (bù cho ô ít hồ) |
| `moisture` + `field_temperature` | Gloger / melanism | Độ đậm sắc tố | **trait mới** `pigment` | `darkness = 0.5·moisture + 0.5·(1−temp_at_highland)`; ẩm & núi-nắng → đậm |
| màu nền `biome` | Crypsis | Tông màu cơ thể | **trait mới** `base_color` | lerp về `BIOME_RGB[biome]` (bảng đã có ở frontend `worldGen.ts`) với hệ số ngụy trang |
| `r_max` (NPP theo biome) | NPP→size, island rule | Ngân sách khối lượng tổng | `total_mass` mục tiêu | `mass_budget ∝ r_max`; rừng mưa cho phép thân to, sa mạc ép thân nhỏ. **Phải trừ vào sổ EU** khi hiện thực hóa |
| `biome`/`elevation`/`flow` | Locomotion medium | **Topology body plan** | số node, cách nối edge, `joint_axis` | Nước (Ocean/River/Lake, `flow` cao) → chuỗi thuôn 1 trục (fusiform), biên độ CPG dọc; Cạn → node chi tỏa ngang; Núi/đá (`elevation` cao) → ít node, thấp, chắc. *Quyết định S27* |
| `slope` (từ gradient `elevation`) | Ruggedness→gait | Tỉ lệ & độ khỏe chi | `length`/`radius`/`mass` chi | Dốc lớn → chi ngắn-dày (`radius↑`, `length↓`); phẳng → chi dài (cursorial) |
| `r_max` + mật độ prey cục bộ | Trophic | Guild + diet breadth + tỉ lệ kích thước | marker `Predator`/`Prey` (→ **trait `Diet`**) | NPP thực vật đủ cao → cho phép herbivore; đủ sinh khối prey → cho phép predator; predator mass ~ tỉ lệ prey mass |
| biên độ `SeasonClock` | Seasonality→life-history | Chiến lược sinh sản, dự trữ | **trait vòng đời (M5)** | Mùa gắt → `reproduction` đồng bộ đỉnh `seasonal_fertility`, `energy_reserve` cao hơn |

**Ưu tiên hiện thực:** các dòng dùng đòn bẩy **đã tồn tại** (`mass`, `length`, `radius`, `joint_axis`, `total_mass`, `temp_target`) làm được ngay ở Tầng 2/3. Các dòng gắn **"trait mới"** (`pigment`, `base_color`, `hydration_loss_mult`, `Diet`, vòng đời) cần mở rộng genome/phenotype trước — thuộc M5/M7.

---

## 6. Ngoại hình (appearance) — cụ thể

### 6.1. Kiểu cơ thể (body plan / topology) — quan trọng & làm được ngay

Đây là đòn bẩy **mạnh nhất mà genome hiện tại đã hỗ trợ**. `genesis_generator(env)` chọn một **archetype topology** theo môi trường vận động:

- **Thủy sinh** (biome ∈ {Ocean, DeepOcean, River, Lake} hoặc `flow` cao): chuỗi 3–5 node **cùng một trục** (fusiform), `joint_axis` vuông góc trục thân để uốn sóng bơi, `radius` giảm dần về đuôi. *Không được spawn trên núi (S27).*
- **Cạn cursorial** (Grassland/Savanna/Steppe, `slope` thấp): thân ngắn + các node "chi" tỏa ngang, `length` chi lớn để bước dài.
- **Núi/đá** (Rock/Alpine/MountainRock, `elevation` cao): ít node, `mass` dồn thấp, chi ngắn-dày để bám.
- **Rừng rậm** (Forest/Jungle/Taiga): thân vừa, cho phép nhiều node (leo trèo).

Vì `decode_genotype` đã dựng entity-tree từ topology, thay đổi topology **tự động** đổi dáng đi (qua CPG + physics) và chi phí chuyển hóa (qua tổng mass) — **đúng tinh thần "truyền qua cơ chế"**, không phải gán cứng.

### 6.2. Kích thước & tỉ lệ

Bergmann (mass theo `1−temp`) + Allen (slenderness theo `temp`) + NPP budget (`total_mass` theo `r_max`), như bảng §5. Tất cả nằm trong clamp mutation hiện có nên tương thích với tiến hóa về sau.

### 6.3. Màu sắc (cần thêm mới)

Chưa có gene màu, và frontend đang render agent sống bằng **tam giác/tròn 2D** ([`PixiViewport.tsx:526`](src/components/Landscape/PixiViewport.tsx)) — nên đây là chuỗi thay đổi xuyên tầng:

1. Thêm trait `pigment`/`base_color` vào genome hoặc phenotype.
2. Quy tắc: `base_color = crypsis(BIOME_RGB[biome])` điều biến bởi Gloger (`moisture`) + thermal melanism (`temp` ở vùng cao). `BIOME_RGB` đã có sẵn ở [`worldGen.ts`](src/components/Landscape/utils/worldGen.ts) — dùng lại làm nền ngụy trang.
3. Mở rộng `SegmentState` (IPC) thêm trường màu ([`types/index.ts:1`](src/types/index.ts)) và bổ sung một renderer 3D cho agent sống (hiện chỉ có Pixi 2D + cây DOM text; wildlife 3D thì đẹp nhưng **cố định**, không nối backend).

> Precedent tốt: wildlife trang trí ở [`WorldWildlife.tsx`](src/components/Landscape/WorldWildlife.tsx) **đã gate *vị trí* theo biome/temperature/slope** — chỉ chưa đổi *ngoại hình* theo môi trường. Nối màu vào chính chỗ này là bước nhỏ.

---

## 7. Đặc điểm phi hình thái (characteristics)

- **Sinh lý nhiệt:** thay `temp_target = 37` cứng bằng `temp_target` theo `field_temperature` cục bộ (bảng §5). Vì `metabolic_rate` đã phụ thuộc body-temp (Arrhenius, `E_ANIMAL_EV = 0.65`), con có ngưỡng nhiệt lệch biome sẽ *tự* trả giá chuyển hóa → chọn lọc Tầng 1 tự lo. **Giữ đúng đơn vị °C**, không trộn với `field_temperature [0,1]`.
- **Kinh tế nước:** trait `hydration_loss_mult` theo `moisture`; con sa mạc mất nước chậm hơn → sống được nơi ít hồ.
- **Guild & diet:** thay quyết định-bằng-index bằng suy luận từ `r_max` + mật độ prey cục bộ; tiến tới `Diet { guild, edible_resources }` như WORLD_SIMULATION_PLAN §3.1.C.
- **Cảm nhận môi trường (M7.1):** hiện perception = raycast(10, cứng) + olfactory(luôn 0) + homeostasis, **không đọc biome/elevation**. Muốn hành vi thật sự "biết" mình ở đâu (tìm bóng râm khi nóng, tránh nước nếu là loài cạn) phải thêm biome/elevation/water/shade vào sensor vector. Đây là tiền đề để chọn lọc gắn địa điểm.
- **Vòng đời (M5):** tuổi/giai đoạn/sinh sản/tử vong; chiến lược sinh sản đồng bộ `SeasonClock`.

---

## 8. Điểm nối code (wiring) — hiện thực ở đâu

| Việc | Vị trí | Tầng |
|---|---|---|
| Lấy mẫu môi trường tại `initial_pos` (biome, temp, moisture, elev, flow, `r_max`) | ngay đầu `decode_genotype` / `SpawnGenotypeCommand` ([`agent_systems.rs:73`](src-tauri/src/core/agent_systems.rs)) | 2, 3 |
| `develop(genotype, env)` điều biến phenotype (không đụng genome) | trong `decode_genotype` ([`genotype.rs:50`](src-tauri/src/evolution/genotype.rs)) | 2 |
| `genesis_generator(env) → genotype` cho gieo de-novo (thay khối cứng) | thay [`simulation_loop.rs:757`](src-tauri/src/core/simulation_loop.rs) | 3 |
| Đặt `temp_target`/`hydration` theo env thay vì hằng số | chỗ khởi tạo `HomeostaticState` trong `decode_genotype` | 2, 3 |
| Kiểm tra hợp lệ spawn (loài cạn/nước) — **S27** | bọc quanh mọi lối spawn; dùng cell-bucket đọc biome | tất cả |
| Trừ năng lượng con non vào sổ EU (không sinh từ hư không) — **S01** | đường sinh sản ([`simulation_loop.rs:375`](src-tauri/src/core/simulation_loop.rs)) đã trả xác về `detritus`; con non phải *rút* từ dự trữ cha mẹ | 1 |
| Thêm trait `pigment`/`Diet`/vòng đời vào genome/phenotype | `genotype.rs` + `components.rs` | M5/M7 |
| Mở rộng `SegmentState` IPC + renderer 3D cho agent | [`types/index.ts`](src/types/index.ts), frontend | M5/M7 |
| Sensor đọc biome/elevation | model input ([`ai/model.rs`](src-tauri/src/ai/model.rs)) | M7.1 |

Mọi thay đổi trên đường spawn được phép cấp phát heap (spawn **không** là hot loop); chỉ cẩn thận không phá zero-alloc trong system tick.

---

## 9. Lộ trình đề xuất & đánh đổi

**Thứ tự làm (rẻ→đắt, hiệu ứng nhìn-thấy sớm):**

- **Phase A — Plasticity trên hình thái sẵn có (Tầng 2).** Nối `field_temperature`/`moisture`/`r_max` vào `mass`/`length`/`radius`/`temp_target` lúc decode. *Không cần gene mới, không đổi IPC.* Cho hiệu ứng Bergmann/Allen/water-economy tức thì. Rủi ro thấp.
- **Phase B — Genesis prior + archetype topology theo medium (Tầng 3).** Thay khối spawn cứng bằng `genesis_generator`; đạt **S27**. Rủi ro trung bình (cần cell-bucket đúng + trừ sổ EU).
- **Phase C — Trait màu + renderer 3D cho agent sống.** Xuyên tầng (genome→IPC→render). Đây là phần "ngoại hình" đúng nghĩa bạn hỏi, nhưng đắt nhất vì đụng cả 3 lớp.
- **Phase D — Siết chọn lọc (Tầng 1) + S43.** Chứng minh trait hội tụ theo biome qua nhiều seed. Đây mới là "thích nghi thật".

**Đánh đổi trung tâm — imposed vs emergent:** Bạn có một cái núm. Lệch về **imposed** (Tầng 3 mạnh): con vật *ngay lập tức* trông hợp môi trường, đẹp cho demo/explore mode, nhưng rủi ro biến engine thành "máy tô màu theo biome" thay vì mô phỏng tiến hóa — và vi phạm tinh thần S43 nếu lạm dụng. Lệch về **emergent** (Tầng 1 mạnh): trung thực khoa học, nhưng chậm và giai đoạn đầu trông ngẫu nhiên/xấu.

**Khuyến nghị của tôi:** dùng **hybrid** — Tầng 3 chỉ đặt *prior mềm có nhiễu* (đủ để không chết ngay và trông hợp lý), Tầng 2 cho phản hồi tức thì trong đời, và để **Tầng 1 là trọng tài cuối cùng** quyết định trait nào trụ lại. Cụ thể: bắt đầu Phase A + B (được ngay 80% hiệu ứng thị giác với rủi ro thấp), rồi mới Phase C/D. Giữ Tầng 3 *yếu* — nó là điểm xuất phát, không phải kết luận.

---

## 10. Tham chiếu

- Ý định thiết kế gốc: [`WORLD_SIMULATION_PLAN.md`](WORLD_SIMULATION_PLAN.md) §3.1.C (Organism model), §5.4 (plant trait template — mẫu để nhân bản cho động vật), §5.5 (sinh vật), §5.7 (sinh sản/tiến hóa), §6 (ma trận tác động), §2.2 (no magic effects); milestone M5, M7.
- Hợp đồng cứng: [`SIMULATION_RULES.md`](SIMULATION_RULES.md) (năng lượng đóng EU, tách 2 nhiệt độ, DeferredM3), [`BIOME_TAXONOMY.md`](BIOME_TAXONOMY.md) (22↔11), [`COORDINATE_CONTRACT.md`](COORDINATE_CONTRACT.md) (cell-bucket vs bilinear).
- Code neo: [`genotype.rs`](src-tauri/src/evolution/genotype.rs) · [`ecology.rs`](src-tauri/src/core/ecology.rs) · [`terrain.rs`](src-tauri/src/core/terrain.rs) · [`simulation_loop.rs`](src-tauri/src/core/simulation_loop.rs) · [`agent_systems.rs`](src-tauri/src/core/agent_systems.rs) · [`hrrl.rs`](src-tauri/src/ai/hrrl.rs) · [`model.rs`](src-tauri/src/ai/model.rs).
- Test gates: S01 (closed energy), S02 (biome map), S03 (coordinate), S27 (spawn legality), S40 (habitat mating), S41 (bounded mutation), S43 (Red-Queen).
