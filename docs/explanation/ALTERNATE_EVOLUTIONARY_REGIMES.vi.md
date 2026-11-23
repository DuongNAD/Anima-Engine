---
title: Alternate Evolutionary Regimes — thế giới khác luật, tiến hóa khác lịch sử
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-17
review_cycle: quarterly
contract: ../reference/EVOLUTION_EXPERIMENT_CONTRACT.md
decision: ../decisions/ADR-0002-world-laws-and-exotic-energy.md
language: vi
translation_of: ALTERNATE_EVOLUTIONARY_REGIMES.md
---

# Alternate Evolutionary Regimes

> **Trạng thái triển khai, rà toàn văn 2026-11-17.** Tài liệu này giải thích mô hình đích. Phần đã có
> trong mã là lát headless AE1–AE3: manifest và runner, trường năng lượng ngoại lai có ngân sách, và
> quần thể tham chiếu có chọn lọc; tất cả opt-in. **Thế giới Bevy sống chưa có trường MU**: adapter thí
> nghiệm sống từ chối một manifest khai `laws.exotic_energy`. **World Lab (giao diện) chưa tồn tại**:
> phép so sánh control–treatment hiện chỉ chạy headless (`genesis_fork`, `checkpoint_fork`,
> `run_ensemble`, binary `anima-lab`) trên engine tham chiếu, chưa trên thế giới người dùng nhìn thấy.
> [ADR-0002](../decisions/ADR-0002-world-laws-and-exotic-energy.md) được chấp nhận ngày 2026-11-17, trong
> phạm vi lát headless. Hợp đồng và các
> gate: [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md).
>
> Cách đọc: mục nào mô tả thứ **chưa có trong mã** mang một dòng *Trạng thái mã* nói rõ điều đó. Mục
> không có dòng ấy đã được đối chiếu với mã ngày 2026-11-17.

## Ý tưởng cốt lõi

Mục tiêu không phải “thêm mana để sinh vật mạnh hơn”. Mục tiêu là cho phép người dùng đặt một luật
hoặc nguồn lực mới vào thế giới từ thời điểm sớm nhất, rồi quan sát xem các dòng sống tự tìm con
đường khác qua nhiều thế hệ.

```mermaid
flowchart LR
    W["Cùng World Artifact"] --> C["Control<br/>không có exotic energy"]
    W --> T["Treatment<br/>có Mana field"]
    C --> EC["Hệ sinh thái baseline"]
    T --> F["Field + source/sink MU"]
    F --> P["Pathway có chi phí"]
    P --> R["Performance khác nhau"]
    R --> S["Survival + reproduction"]
    S --> L["Lineage/niche phân nhánh"]
    L --> SP["Ecotype hoặc candidate species"]
    EC --> X["World Lab compare"]
    SP --> X
```

Hai nhánh có thể dùng cùng địa hình, khí hậu ban đầu, sinh khối và seed. Điểm khác duy nhất là
`WorldLawSet`. Nhờ vậy khác biệt về sau có thể được quy về điều kiện được thay, thay vì hai world
không liên quan.

## Vì sao “mana” phải là một substrate tổng quát

Tên game có thể là **Mana**, nhưng mã lõi dùng module tổng quát `core::exotic_energy`: luật
`ExoticEnergyLaw` mang một `EnergySourceId` làm khoá và một `display_name` chỉ để hiển thị, còn trường là
`ExoticEnergyField`. Không có kiểu nào tên `ExoticEnergy` hay `ExoticEnergySource`. Cùng abstraction có
thể mô tả:

- năng lượng địa nhiệt;
- chemosynthesis quanh khe phun;
- bức xạ mà một lineage học cách khai thác;
- tinh thể/hạt hư cấu phát năng lượng;
- một field siêu nhiên trong world fantasy.

Mỗi nguồn có topology, flux, decay, toxicity và cách khai thác khác. Không cần tạo một engine phép
thuật riêng.

## Bốn lớp không được trộn

### 1. World law — điều gì có thể tồn tại

Ví dụ minh hoạ (không phải schema: tên trường thật của `ExoticEnergyLaw` là `source_model`,
`topology`, `diffusion_rate`, `decay_rate`, và luật còn bắt buộc `unit`, `initial_amount`, `max_density`,
`boundary`):

```text
exotic_energy:
  id: arcane_flux
  display_name: Mana
  mode: Renewable
  source_topology: PatchyHotspots
  source_rate: 0.02 MU/cell/ecology-tick
  diffusion: 0.08
  decay: 0.005
```

Đây là luật của run, không phải trait của organism.

### 2. Environment field — ở đâu và lúc nào nguồn có sẵn

`ExoticEnergyField` lưu density/flux theo cell/chunk. Nó có thể đồng đều, patchy, theo mùa, gắn với
địa chất hoặc xuất hiện theo pulse. Field phải có budget và được render/inspect từ chính dữ liệu sim.

*Trạng thái mã:* trường lưu mật độ MU theo ô cùng các bộ đếm của sổ MU; topology là `Uniform` hoặc
`Patchy`. Chưa có chunk, chưa gắn với mùa hay địa chất, và chưa có gì render nó.

### 3. Organism pathway — sinh vật khai thác bằng cách nào

Genotype có thể tiến hóa:

- cảm nhận nguồn;
- cơ quan hấp thụ;
- dự trữ;
- chuyển hóa;
- chịu độc;
- hành vi tìm hotspot.

Mỗi khả năng có maintenance/morphology/opportunity cost. Một sinh vật đầu tư vào mana trong thế giới
không có mana có thể thua organism không mang chi phí đó.

*Trạng thái mã:* `EnergyPathwayGenotype` (`core/evolution_pathway.rs`) có sensing, uptake, storage,
utilization, tolerance, maintenance và allocation, và chi phí được trả dù có MU hay không. Nó thuộc quần
thể tham chiếu AE3, là hai cohort gộp chứ không phải cá thể trên bản đồ, nên "hành vi tìm hotspot" chưa
có.

### 4. Evolutionary outcome — cái gì còn lại sau nhiều thế hệ

Field không “ra lệnh” sinh ra loài. Nó chỉ đổi landscape of selection. Loài/morph xuất hiện từ:

```text
variation → differential performance → survival/reproduction
→ trait frequency change → niche partition → lineage persistence
→ reduced gene flow / species-candidate threshold
```

## Mana tương tác với closed EU như thế nào

Current MVP định nghĩa EU là biomass-equivalent và khép kín:

```text
plants + animals + detritus + dormant = constant
```

Ngăn `dormant` giữ EU của sinh vật đang ngủ đông và bằng 0 khi không ai ngủ đông
([`ENERGY_LEDGER_CONTRACT.md`](../reference/ENERGY_LEDGER_CONTRACT.md) §1.1).

Vì thế Mana dùng đơn vị MU riêng. MVP không cho `1 MU → tạo biomass từ hư không`. Thay vào đó MU có
thể:

- trả một phần work cost cho vận động/cảm biến/điều nhiệt;
- tăng rate organism chuyển detritus/nutrient thành biomass, nhưng material vẫn bị debit từ EU pool;
- kích hoạt defense, repair hoặc behavior có trade-off;
- trở thành resource cạnh tranh và tạo niche.

*Trạng thái mã:* bốn cách ghép trên là lựa chọn thiết kế, chưa cách nào được hiện thực. Trong lát đã
có, trường MU không ghi vào các pool EU; MU đã tiêu chỉ chuyển thành performance sinh sản của quần thể
tham chiếu AE3.

Nếu sau này muốn mana materialize vật chất thật, phải tách mass/physical-energy ledger bằng ADR mới.

## Các chế độ nguồn nên hỗ trợ

| Mode | Ý nghĩa | Câu hỏi tiến hóa |
|---|---|---|
| `Disabled` | Không có nguồn; baseline | Pathway có bị loại vì cost không? |
| `Finite` | Trữ lượng ban đầu, không tái tạo | Boom–bust, tranh chấp, extinction debt |
| `Renewable` | Có source rate và sink | Specialist ổn định có hình thành không? |
| `Pulsed` | Xuất hiện theo mùa/biến cố | Storage, dormancy hay migration có lợi hơn? |
| `Patchy` topology | Hotspot không đều | Local adaptation và ecotype có phân vùng không? |

MVP nên làm `Disabled` + `Renewable/Patchy`; các mode khác dùng cùng contract sau.

*Trạng thái mã:* bảng trên là mục tiêu. `ExoticSourceModel` chỉ có `Renewable`. Tắt nguồn là
`exotic_energy = None` trên `WorldLawSet`, và mã **cố ý không có** biến thể `Disabled` để một luật có mặt
không bao giờ bị nhầm với baseline. `Finite` và `Pulsed` chưa hiện thực; xung chỉ có dưới dạng forcing
runtime `ExoticInterventionKind::Pulse`. `Patchy` là một `SourceTopology`, không phải mode.

## Những kiểu sinh vật khác nhau có thể emergent

Không hard-code tên loài, nhưng có thể kỳ vọng các chiến lược:

| Áp lực | Candidate strategy | Chi phí đối trọng |
|---|---|---|
| Mana dày và ổn định | Specialist uptake cao | Cơ quan nặng, maintenance cao |
| Mana loãng/patchy | Sensor + migration/foraging range lớn | Tốn vận động và neural cost |
| Mana pulse | Storage lớn/dormancy | Chậm sinh sản hoặc thân nặng |
| Mana độc ở density cao | Tolerance/detox | Conversion efficiency thấp |
| Nhiều consumer tranh hotspot | Territorial/social defense | Injury/stress/group cost |
| Mana chỉ producers dùng được | Food web dựa trên mana-producer | Phụ thuộc trophic source mới |

Đây là hypothesis để kiểm thử, không phải outcome được script.

## “Tác động từ sớm nhất” có hai nghĩa

### Genesis fork

Đổi luật ngay trước `t=0`. Hai lịch sử chạy hàng trăm/thousands thế hệ. Cách này trả lời:

> Một thế giới luôn có mana sẽ tiến hóa khác baseline ra sao?

### Checkpoint fork

Chạy chung đến thế hệ G, snapshot, rồi thêm hoặc rút nguồn ở một nhánh. Cách này trả lời:

> Một hệ sinh thái đã ổn định phản ứng, phụ thuộc hoặc phục hồi thế nào khi luật/resource đổi?

Cần cả hai; chỉ bật mana giữa run không trả lời được lịch sử từ genesis.

*Trạng thái mã:* checkpoint fork không đổi luật. Nhánh treatment nhận forcing runtime có `CauseId`
(`AddSource`, `RemoveSource`, `Pulse`) trên một thế giới mà luật **đã** khai nguồn từ genesis; forcing
ngoại lai trên một thế giới không khai nguồn bị từ chối. Vì thế "thêm mana giữa chừng vào một thế giới
baseline" hiện không biểu diễn được. `RemoveSource` chặn dòng tái tạo, không rút MU đang tồn.

## World Lab — người dùng quan sát gì

*Trạng thái mã:* cả mục này là thiết kế đích. Chưa có giao diện World Lab, và backend của app desktop
chưa có lệnh fork hay branch nào. Cây nhánh và phép so sánh có ở dạng headless (`core/branch_tree.rs`,
`experiment_runner`).

### Lớp bản đồ

- density, source flux, uptake và depletion của MU;
- biomass/resource/climate/water/soil hiện có;
- phân bố pathway phenotype;
- vùng sinh, chết, migration và reproductive success;
- niche/species cluster theo thời gian.

### Đồ thị

- source → field → uptake → storage → expenditure;
- EU và MU budget;
- trait/allele frequency;
- population, species richness, extinction;
- phenotype distribution và MAP-Elites coverage;
- food-web edges mới;
- effect size control–treatment với confidence interval.

### Inspector

Click một cell/organism/lineage/species để xem:

- giá trị hiện tại và đơn vị;
- lịch sử;
- parent/root cause;
- genotype vs developed phenotype vs runtime state;
- energy transactions;
- ancestor/descendant;
- lý do candidate species được tạo hoặc bị gộp.

### Branch timeline

World Lab hiển thị cây experiment:

```text
run-0 baseline
  ├─ run-1 mana from genesis
  ├─ run-2 mana patchy from genesis
  └─ checkpoint generation 100
       ├─ run-3 continue
       └─ run-4 remove mana
```

Người dùng có thể replay, đổi một factor, tạo branch và so sánh trên cùng trục thời gian.

## Cách chứng minh khác biệt là tiến hóa

Ba mức bằng chứng:

1. **Mechanism:** MU field và pathway tạo transaction/performance khác đúng contract.
2. **Selection:** pathway thay survival/reproductive success; frequency đổi qua thế hệ.
3. **Divergence/speciation:** lineage/niche/mating khác bền và lặp lại trên ensemble.

Ảnh con vật trông khác, một MAP-Elites cell mới hoặc correlation với hotspot chỉ đạt mức mô tả.

## Vertical slice đề xuất

Mở rộng “lưu vực – đồng cỏ – thỏ – sói” bằng một field Mana patchy:

- control: không Mana;
- treatment A: Mana hỗ trợ producer chuyển detritus → plant biomass nhanh hơn;
- treatment B: thêm consumer pathway có sensing/uptake/storage/cost;
- treatment C: sau 100 thế hệ rút Mana khỏi cùng checkpoint.

Quan sát:

- EU/MU budget;
- plant/herbivore/predator response;
- pathway frequency;
- foraging range và morphology cost;
- niche divergence;
- extinction/recovery;
- causal chain;
- multi-seed effect size.

Vertical slice này cho kết quả quan sát sớm mà chưa cần spell, combat fantasy hoặc taxonomy loài đầy
đủ.

*Trạng thái mã:* treatment A chưa hiện thực (MU chưa ghép với EU). Treatment B có ở dạng quần thể tham
chiếu gộp, chưa phải thỏ và sói trên bản đồ. Treatment C biểu diễn được bằng checkpoint fork với
`RemoveSource`.

## Giới hạn trung thực

- Thế giới Bevy sống đã có adapter thí nghiệm tất định `LiveExperimentAdapter` (`ExperimentModel`), kiểm
  headless qua `tests/live_experiment_tests.rs`. Nó chưa được chạy trong app desktop dưới executor đa
  luồng, từ chối `laws.exotic_energy` và không có quần thể AE3. Trait cũ `SimModel` (M2) vẫn chỉ do
  `ReferenceEcosystem` hiện thực; AE1–AE3 chạy trên `ReferenceEvolutionWorld`.
- Chưa có species clustering hay species policy nào trong mã. Sinh sản liên tục theo ngưỡng năng lượng
  đã có (ADR-0005) nhưng chưa phải vòng đời M5–M7.
- Causal ledger hiện chỉ có một parent và chưa phân bổ multi-cause.
- Bằng chứng bản đồ (Animal Map Vision) chưa có; nó không áp cho lát headless (ADR-0002, mục quyết định
  ngày 2026-11-17).
- Do đó, ngoài lát headless AE1–AE3 nêu ở đầu trang, tài liệu này là thiết kế/experiment proposal, không
  phải tuyên bố feature đã chạy.
