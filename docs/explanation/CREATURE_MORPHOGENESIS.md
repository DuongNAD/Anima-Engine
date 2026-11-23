---
title: Creature Morphogenesis — genotype, môi trường và chọn lọc
status: accepted
owner: simulation-architecture
last_reviewed: 2026-11-17
review_cycle: quarterly
contract: ../reference/CREATURE_DEVELOPMENT_CONTRACT.md
plan: ../planning/CREATURE_MORPHOGENESIS_PLAN.md
---

# Creature Morphogenesis

Tài liệu này giải thích cách Anima Engine tạo một sinh vật mới sao cho có quan hệ hợp
lý với môi trường nhưng vẫn giữ được tiến hóa emergent. Nó không phải contract API;
quy tắc bắt buộc nằm trong
[`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md).

## Câu trả lời ngắn

Không “tô” một sinh vật theo biome tại mỗi frame. Anima dùng ba cơ chế khác nhau:

1. **Genesis prior:** khi tạo một dòng sống hoàn toàn mới, environment bias nhẹ phân
   bố genotype ban đầu để quần thể không chết ngay.
2. **Developmental plasticity:** genotype + environment lúc sinh tạo phenotype một lần.
3. **Natural selection:** environment quyết định phenotype nào sống và sinh sản qua
   nhiều thế hệ.

Chỉ cơ chế 3 chứng minh thích nghi tiến hóa. Prior và plasticity tạo điểm xuất phát
hoặc phản ứng trong đời; chúng không tự chứng minh fitness.

```mermaid
flowchart LR
    E["Birth environment"] --> D["develop_at_birth"]
    G["Heritable genotype"] --> D
    P["Weak genesis prior"] --> G
    D --> H["Versioned DevelopedPhenotype"]
    H --> S["spawn_developed"]
    S --> L["Life: physics, metabolism, behavior"]
    L --> F["Survival + reproduction"]
    F --> N["Selection / next generation"]
    H --> V["Save + migration + renderer"]
```

## Hiện trạng engine

> **Rà toàn văn 2026-11-17.** Tài liệu viết tháng 7; cả trang đã được đối chiếu với mã trên thân cây.
> Ba điểm của hiện trạng đã đổi:
>
> - Sổ cái closed-EU nay có **bốn** ngăn: thêm `dormant` cho agent ngủ đông
>   ([hợp đồng sổ cái §1.1](../reference/ENERGY_LEDGER_CONTRACT.md)).
> - Sinh sản không còn chỉ là epoch replacement. `autonomous_reproduction_system` (sinh sản liên tục
>   theo ngưỡng năng lượng, vốn của con non trừ từ reserve của cha/mẹ) nằm trong lịch tick chung.
>   **Trong app desktop nó chạy mặc định**, vì `ReproductionPolicy` vắng mặt nghĩa là bật. **Trong
>   adapter thí nghiệm sống nó tắt** cho tới khi manifest khai `live.reproduction`
>   ([ADR-0005](../decisions/ADR-0005-live-population-turnover.md)). Cái chết vì đói và khát tắt mặc
>   định ở cả hai nơi: app bật bằng `ANIMA_MORTALITY`, adapter bằng `live.mortality`. Đây chưa phải
>   vòng đời M5: con non vẫn được dựng bằng `decode_genotype`, không qua một bước development nào.
> - Các tên của vòng đời đích (`DevelopedPhenotype`, `EnvSample`, `EnvironmentSnapshot`, `RuntimeState`,
>   `LocomotionMedium`, `develop_at_birth`, `spawn_developed`, `development_version`) **vẫn chưa có
>   trong mã**. Mục "Điểm nguy hiểm hiện tại" bên dưới vì thế vẫn nguyên hiệu lực, và `decode_genotype`
>   nay có **bảy** call-site thay vì bốn: thêm `SpawnOffspringCommand::apply`, `genesis` của
>   `live_experiment` và `RehydrateCommand::apply`
>   ([bảng trong hợp đồng](../reference/CREATURE_DEVELOPMENT_CONTRACT.md)).
>
> Cách đọc phần còn lại: từ "Bốn lớp dữ liệu" trở xuống là **thiết kế đích**. Chỗ nào mã hiện tại đã có
> một phần, hoặc làm khác đi, có một dòng *Trạng thái mã*.

### Đã có

- `MorphologyGenotype`: graph node/edge với length, radius, mass, joint anchor/axis.
- `decode_genotype`: dựng root/child entities, rigid body, collider, CPG và homeostasis.
- `TerrainMap`: elevation, moisture, temperature, biome và flow.
- `ResourceField`: resource hiện tại `r` và carrying capacity `r_max`.
- Closed-EU ledger: plants, animals, detritus, và từ đó tới nay thêm ngăn `dormant`.
- MAP-Elites, mutation/crossover, lineage và migration.
- Pixi 2D simulation view và Three.js landscape view.

### Chưa có

- Representation phenotype tách khỏi genotype.
- Species/habitat profile độc lập với environment.
- Development version và birth-environment snapshot.
- Pigment/thermal strategy/plasticity strength có thể di truyền.
- Reproduction M5 thật sự. Tháng 7 chỉ có epoch replacement; nay có thêm sinh sản liên tục theo ngưỡng
  năng lượng (xem ghi chú đầu mục), vẫn chưa phải M5.
- Sensor terrain/water/shade của M7.1.
- Save/migration cho phenotype.

### Điểm nguy hiểm hiện tại

`decode_genotype` được dùng cho cả cá thể mới và cá thể đã tồn tại. Vì vậy environment
không được đọc ngầm trong hàm này. Nếu không tách lifecycle, migration sang shard nóng
sẽ làm cá thể “sinh lại” thành hình khác.

## Bốn lớp dữ liệu

### 1. Genotype — cái có thể di truyền

- Morphology node/edge gốc.
- Pigment gene hoặc `None` cho legacy.
- Developmental reaction slopes.
- Habitat/locomotion preference.
- Thermal strategy khi M5.4 bắt đầu.

Genotype không chứa environment hiện tại và không chứa kết quả cuối của development.

### 2. Birth environment — cue phát triển

`EnvSample` là snapshot nhỏ:

```text
legacy_biome, field_temperature, moisture, elevation, flow,
resource_capacity, standing_resource
```

MVP dùng backend 11-biome projection. Khi cần ecotone/taxonomy 22-biome đầy đủ, phải mở
rộng artifact/backend contract; không giả vờ dữ liệu đã còn nguyên sau downsample.

### 3. Developed phenotype — kết quả ổn định

- Developed nodes/edges.
- Total mass thực tế.
- Collider/anchor-compatible geometry.
- Display color.
- Habitat/thermal phenotype.
- Development version và birth-environment fingerprint.

Đây là dữ liệu physics, MAP-Elites descriptor và renderer phải cùng đọc.

### 4. Runtime state — cái thay đổi mỗi tick

- Position/rotation/velocity.
- Energy/hydration/body temperature.
- CPG phase, brain transition, evaluation và feature tracker.

Save/load ghép `DevelopedPhenotype` với `RuntimeState`; không tái phát triển.

## Các quy luật sinh học chỉ là policy có phạm vi

Bergmann, Allen và Gloger hữu ích để tạo prior/response ban đầu nhưng không phải luật
phổ quát:

- Bergmann/Allen liên quan mạnh nhất tới endotherm và phản ứng có thể phi tuyến, đảo
  chiều ở miền nhiệt cực đoan.
- Gloger có nhiều cách diễn giải; humidity/temperature không cho một công thức màu
  đúng với mọi clade.
- Resource productivity thường giới hạn tổng biomass, density và trophic structure;
  nó không mặc định quyết định body mass từng cá thể.

Vì vậy mỗi response phải có:

| Thành phần | Yêu cầu |
|---|---|
| Cue | Field nào, đơn vị/miền nào |
| Scope | Thermal strategy/medium nào được áp |
| Heritable strength | Slope/bound nằm trong genotype |
| Cost/trade-off | Benefit ở môi trường A phải có giới hạn hoặc chi phí |
| Clamp | Hình học/sinh lý không ra ngoài miền hợp lệ |
| Evidence | Property test + experiment, không chỉ screenshot |

Nguồn khoa học nền:

- [Temperature-dependent plasticity và giới hạn của Bergmann/Allen](https://pmc.ncbi.nlm.nih.gov/articles/PMC10503470/)
- [Review Gloger’s rule](https://onlinelibrary.wiley.com/doi/10.1111/brv.12503)
- [Costs và limits của adaptive plasticity](https://pmc.ncbi.nlm.nih.gov/articles/PMC2842679/)
- [Local adaptation và reciprocal transplant](https://www.nature.com/articles/nrg3522)

## Policy MVP

### Morphology

Developmental slopes mặc định bằng 0 cho legacy. Genesis mới lấy mẫu slope nhỏ có
bound. Một response được áp đúng một lần:

```text
developed_value = clamp(genetic_value × response(genetic_slope, cue))
```

Không áp cùng Bergmann multiplier trong cả prior và development. Prior thay phân bố
`genetic_value`/slope; development dùng slope.

### Habitat

Medium thuộc sinh vật:

```text
Aquatic | Terrestrial | Amphibious | Arboreal | Alpine | Aerial
```

Environment được phân loại riêng thành water/land/elevation/slope class. S27 là phép
so sánh hai phía, không phải suy medium từ ô rồi kiểm lại chính ô.

*Trạng thái mã:* `LocomotionMedium` chưa tồn tại. Sáu giá trị trên mới chỉ được liệt kê ở trang này;
hợp đồng phác `HabitatPreference { medium, elevation_range }` mà không liệt kê biến thể.

*Ứng viên chi phí (chưa quyết định):* hai số hạng có thể đi cùng medium, cả hai cộng vào `total_cost`
trong `metabolic_decay_system` như `cognition_cost`, không bao giờ trừ riêng: chi phí bảo dưỡng giác
quan (mắt tốn nhiều hơn râu hay thụ thể hoá học, để chọn lọc tự đào thải mắt trong bóng tối) và ngân
sách khối lượng cơ bay cho `Aerial`, cố định lúc sinh. Nguồn và giới hạn: xem
[`QD_REALMS_HABITATS_RESEARCH_2026-11-22.md`](../research/QD_REALMS_HABITATS_RESEARCH_2026-11-22.md).

### Pigment

Pigment gene tạo màu cơ sở. Developmental slope có thể tác động nhẹ theo cue. Màu
phenotype được lưu lúc sinh và không đổi khi cá thể bước qua biên biome. Crypsis chỉ
được gọi là thích nghi khi reciprocal-transplant/selection chứng minh fitness.

Legacy genotype dùng `pigment: None`; renderer tiếp tục fallback theo predator/prey,
tránh save cũ biến thành màu đen do `Default`.

*Trạng thái mã:* chưa có trường pigment nào. Cả hai renderer hiện tô màu theo predator/prey.

### Nhiệt và nước

Phase đầu chỉ lưu strategy/trait; chưa nối trực tiếp vào survival.

- Body temperature chỉ bị environment tác động khi có heat-exchange mechanism M3/M5.
- `temp_target` là preferred range/strategy, không đơn giản là tuyến tính từ climate.
- Hydration efficiency phải có cost/trade-off và không giả định surface-water pool đã
  được bảo toàn trước M3.

## Genesis, birth và replacement

### Genesis

Genesis là điều kiện biên `t=0`:

1. Chọn spawn region hợp habitat.
2. Sample environment.
3. Lấy mẫu genotype từ prior yếu.
4. Develop một lần.
5. Spawn.
6. Chốt baseline `plants + animals + detritus + dormant`.

*Trạng thái mã:* chỉ bước 6 đã có (`EnergyLedger::lock_baseline`, ở lần census đầu). Genesis hiện đặt
founder trên một đường thẳng, hoặc trên một lưới khi đặt `ANIMA_FOUNDING_POPULATION`, với cùng một
genotype; không chọn vùng theo habitat, không lấy mẫu môi trường, không có prior và không có bước
development.

### Birth M5

Birth cần parent entities, maturity và energy transaction. Con non không lấy 100 EU
miễn phí. Đây là M5.3/S29 và không được “vá” vào epoch replacement.

*Trạng thái mã:* một đường sinh có giao dịch năng lượng đã tồn tại: `autonomous_reproduction_system` →
`SpawnOffspringCommand`, một cha/mẹ, vốn của con non trừ từ reserve của cha/mẹ. Nó không có điều kiện
trưởng thành, không có bạn tình, và con non vẫn được dựng bằng `decode_genotype`, nên nó chưa phải M5.3.

### Evolutionary replacement legacy

Cơ chế hiện tại thay cá thể cuối epoch bằng genotype từ archive
(`apply_staggered_evolution_system` → `SpawnGenotypeCommand`). `EvolutionaryReplacement` là tên gọi của
hợp đồng; trong mã không có kiểu nào mang tên đó. Reserve của cá thể bị thay trả về detritus, và cá thể
mới rút vốn từ detritus qua sự kiện sổ cái `EnergyEvent::Replacement`; nếu detritus không đủ, phần thiếu
được đếm là `refused` thay vì thế giới tự có thêm năng lượng. Việc thay thế chưa được ghi vào
`CausalLedger`. Cơ chế này có thể tiếp tục phục vụ MAP-Elites trước khi reproduction M5 hoàn chỉnh.

## Save, load và migration

```mermaid
sequenceDiagram
    participant B as Birth/Genesis
    participant D as develop_at_birth
    participant P as DevelopedPhenotype
    participant S as Save/Migration
    participant R as Restore/Target shard

    B->>D: genotype + EnvSample + seeded RNG
    D->>P: versioned phenotype
    P->>S: serialize phenotype + runtime state
    S->>R: payload
    R->>R: spawn_developed(payload phenotype)
    Note over R: Không gọi develop_at_birth
```

*Trạng thái mã:* sơ đồ trên là đích. Hôm nay `SerializedAgent` lưu genotype, và restore lẫn migration
gọi lại `decode_genotype`.

Save cũ không có phenotype đi qua một migration versioned duy nhất. Migration này có
thể phát triển từ genotype + saved position để tạo phenotype legacy, nhưng phải ghi
`development_version` và không lặp ở lần load sau.

## Determinism

Nguồn seed chuẩn là `WorldIdentity.seed`. Mỗi stream có key:

```text
derive_seed(world_seed, event_kind, tick_or_epoch, lineage_key)
```

Mutation/crossover/development nhận RNG từ caller. `thread_rng()` và UUID v4 không
được dùng trong test path deterministic; lineage ID có thể sinh từ seed + counter/hash.

*Trạng thái mã:* chưa có hàm `derive_seed` bốn thành phần. Mỗi stream hiện được tách bằng
`derived_seed(run_seed, stream)` (`core/resources.rs`) với một hằng số cho từng stream. Mutation và
crossover đã nhận RNG từ caller, và mã không gọi `thread_rng()`. UUID v4 vẫn còn ở đường chạy mặc định
của app; chế độ tất định sinh id từ `RunIdentity`.

## Render và IPC

Rust `SegmentState` (`core/simulation_state.rs`) đã mang `length` và `radius`; còn thiếu `color` và
`phenotype_version`. Kiểu TypeScript tương ứng được sinh bằng ts-rs
(`src/types/generated/SegmentState.ts`) và không sửa tay: đổi struct Rust rồi sinh lại. Phía frontend phải
đồng bộ:

- `src/utils/agentHierarchy.ts` (`RenderSegment`, `buildAgentHierarchy`);
- `src/types/index.ts::RenderSegment`;
- `src/PixiViewport.tsx`;
- renderer 3D `src/components/Landscape/LiveAgents.tsx`;
- `PROJECT.md` Interface Contracts và mocks/tests.

Danh sách tháng 7 còn kể `src/hooks/useSimulation.ts` và bản sao kiểu trong `src/App.tsx`. File đầu đã bị
xoá, và `App.tsx` nay nhập từ `utils/agentHierarchy`.

Renderer không tự sample biome để đổi phenotype. Nó chỉ render dữ liệu Rust gửi.

## Chứng minh thích nghi

Ba mức bằng chứng:

1. **Mechanism:** cue đổi phenotype đúng bound.
2. **Performance:** phenotype tạo khác biệt intake/cost/movement đúng cơ chế.
3. **Adaptation:** genotype địa phương có reproductive fitness cao hơn genotype ngoại
   lai trong reciprocal transplant qua nhiều seed.

Trait–biome correlation chỉ là mô tả, không đủ cho mức 3. S43 vẫn dành cho coevolution
Red-Queen predator–prey; local adaptation dùng CM-S11.

## Ngoài phạm vi đợt đầu

- Full 22-biome preservation ở backend.
- Dynamic camouflage.
- Aerial/swimming physics hoàn chỉnh.
- Species taxonomy sinh học thực.
- Reproduction, age/life stage và mate choice hoàn chỉnh.
- Hydrology/water balance trước M3.

Các phần này được mở bằng task/ADR riêng, không chèn ngầm vào Phase đầu.
