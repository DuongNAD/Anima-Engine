---
title: Hợp đồng chế độ tiến hóa và thí nghiệm thế giới
status: proposed
owner: simulation-architecture
last_reviewed: 2026-07-24
review_cycle: per-release
decision: ../decisions/ADR-0002-world-laws-and-exotic-energy.md
feature_requirements: ../ai/requirements/2026-07-24-feature-alternate-evolution-world-lab.md
---

# Hợp đồng chế độ tiến hóa và thí nghiệm thế giới

Tài liệu này là **contract mục tiêu đang được đề xuất**, chưa mô tả tính năng đã có trong runtime.
Nó quy định cách Anima Engine tạo các lịch sử tiến hóa khác nhau từ cùng một thế giới nền, ví dụ
một nhánh không có nguồn năng lượng đặc biệt và một nhánh có nguồn năng lượng được hiển thị với tên
“mana”.

Mọi implementation liên quan `WorldLawSet`, điều kiện `t=0`, nguồn năng lượng mới, khả năng sử dụng
nguồn đó, phân nhánh scenario, quan sát tiến hóa hoặc kết luận “loài mới” phải tuân theo contract này
sau khi ADR-0002 được accepted.

## Thứ tự đọc cho AI agent

1. Tài liệu này — invariant và gate bắt buộc.
2. [ADR-0002](../decisions/ADR-0002-world-laws-and-exotic-energy.md) — quyết định và trade-off.
3. [Giải thích Alternate Evolutionary Regimes](../explanation/ALTERNATE_EVOLUTIONARY_REGIMES.md).
4. Bộ lifecycle của feature:
   - [requirements](../ai/requirements/2026-07-24-feature-alternate-evolution-world-lab.md);
   - [design](../ai/design/2026-07-24-feature-alternate-evolution-world-lab.md);
   - [testing](../ai/testing/2026-07-24-feature-alternate-evolution-world-lab.md);
   - [planning](../ai/planning/2026-07-24-feature-alternate-evolution-world-lab.md).
5. [Creature Development Contract](CREATURE_DEVELOPMENT_CONTRACT.md) trước khi sửa genotype,
   phenotype, birth, save hoặc migration.
6. Code hiện tại tại các symbol được liệt kê bên dưới; không dựa vào line number lịch sử.

## Thuật ngữ chuẩn

| Thuật ngữ | Ý nghĩa |
|---|---|
| `WorldLawSet` | Tập luật bất biến của một run: nguồn năng lượng, budget, mutation, speciation policy |
| Baseline regime | Chế độ tương thích thế giới hiện tại; exotic energy bị tắt |
| Alternate regime | Chế độ có một hoặc nhiều luật/nguồn khác baseline |
| `ExoticEnergyLaw` | Định nghĩa nguồn năng lượng tổng quát; “mana” chỉ là `display_name` |
| `ExperimentManifest` | Toàn bộ đầu vào tái lập: artifact, luật, điều kiện đầu, intervention, seed, metric |
| Genesis fork | Hai run bắt đầu từ cùng artifact/seed nhưng khác luật hoặc điều kiện `t=0` |
| Checkpoint fork | Hai run tiếp tục từ cùng snapshot rồi nhận treatment khác nhau |
| Intervention | Tác động có thời điểm, vùng, cường độ và `cause_id` trong một run |
| Energy pathway | Trait di truyền cho cảm nhận, hấp thụ, lưu trữ hoặc sử dụng một nguồn năng lượng |
| Species cluster | Kết quả phân cụm lineage/genotype/niche/mating; không phải nhãn ngoại hình thủ công |

## Sự thật code hiện tại

| Khả năng | Symbol hiện tại | Giới hạn hiện tại |
|---|---|---|
| Scenario deterministic | `core::scenario::Scenario`, `run_scenario` | Chỉ cấu hình seed/duration/intervention |
| Control/treatment | `control_treatment` | Một seed, model mặc định, chưa có world-law fork |
| Mô hình thử | `ReferenceEcosystem` | Aggregate nhỏ; không phải live Bevy world |
| Live Bevy world qua cùng contract | `core::live_experiment::LiveExperimentAdapter` (`ExperimentModel`) | **Đã có, verified headless** (2026-07-27): cùng manifest/clock/intervention/ledger/registry, trên đúng lịch trình `simulation_schedule::build_tick_schedule` mà app chạy. **Từ chối** `laws.exotic_energy` (không có trường MU) và không có quần thể AE3; chưa chạy app desktop; **không** tuyên bố trùng số với reference — chỉ trùng hướng của luật chung |
| Can thiệp | `InterventionKind`, `InterventionCommand` | Năm kind cố định; chưa có parameter map/version |
| Giải thích nhân–quả | `CausalLedger`, `EffectRecord` | Một parent; multi-cause attribution còn thiếu |
| Luật/đơn vị | `core::sim_rules::STATE_VARIABLES` | Closed EU; chưa có exotic-energy registry |
| World identity | `WorldIdentity` | Seed/generator/checksum; chưa fingerprint `WorldLawSet` |
| Tiến hóa | genotype, mutation/crossover, MAP-Elites | Chưa có energy pathway/species clustering |
| Dashboard | `EcosystemPanel` | Biomass/niche/archive; chưa có experiment comparison |

## Invariant bắt buộc

### ER01 — Luật thế giới khác intervention

`WorldLawSet` được chốt trước genesis và bất biến trong một run. Thay một world law phải:

- tạo genesis fork mới; hoặc
- tạo checkpoint fork mới kèm một intervention có semantics rõ.

UI không được âm thầm đổi luật ở giữa run rồi tiếp tục gọi đó là cùng thí nghiệm.

### ER02 — Tắt exotic energy phải giữ baseline

Khi `exotic_energy = None`:

- không có field, uptake, storage hoặc maintenance cost ẩn;
- genotype legacy nhận pathway disabled bằng default;
- cùng artifact + seed + intervention phải giữ kết quả/checksum baseline theo tolerance đã công bố.

Đây là rollback cứng của toàn feature.

### ER03 — Nguồn mới không được viết thẳng kết quả tiến hóa

Mana/exotic energy **MUST NOT**:

- sửa trực tiếp genotype;
- gán trực tiếp loài;
- tăng trực tiếp population/fitness;
- đổi ngoại hình mỗi frame theo field.

Nó chỉ ảnh hưởng qua cơ chế: field → sensing/uptake → physiology/performance → survival/reproduction
→ allele/trait frequency → lineage/species divergence.

### ER04 — Mọi nguồn, sink và conversion phải có budget

Mỗi `ExoticEnergyLaw` phải khai báo:

- đơn vị riêng, mặc định `MU` (mana unit);
- source model: `Finite`, `Renewable`, `Pulsed` hoặc `Disabled`;
- spatial topology và initial amount/flux;
- uptake, release, decay/dissipation;
- conversion rule nếu có;
- tolerance của balance audit.

Không gọi `MU` là `EU`. Closed EU vẫn là biomass-equivalent
`plants + animals + detritus`. Exotic energy có thể thay đổi **tốc độ chuyển EU giữa các pool**,
nhưng không tạo biomass miễn phí. Nếu một luật tương lai cho phép materialization, luật đó cần ADR
riêng và một mass ledger mới.

### ER05 — Sử dụng exotic energy là trait có chi phí

Energy pathway tối thiểu có:

- sensing affinity;
- uptake rate;
- storage capacity;
- utilization efficiency;
- tolerance/toxicity;
- maintenance cost và morphology cost.

Trong thế giới không có nguồn tương ứng, pathway phải trung tính hoặc bất lợi vì chi phí; không có
universal benefit.

### ER06 — Development và evolution không bị trộn

Nếu exotic energy là cue lúc sinh, nó được ghi vào `EnvSample`/birth snapshot và chỉ tác động qua
`develop_at_birth` đúng một lần. Việc sử dụng năng lượng trong đời là runtime physiology. Thay đổi
trait qua nhiều thế hệ là evolution. Restore/migration không develop lại.

### ER07 — So sánh lịch sử phải khóa điều kiện chung

Control và treatment phải dùng cùng:

- `WorldArtifact` checksum;
- snapshot hoặc initial-condition manifest;
- seed/seed set;
- thời lượng và sampling schedule;
- model/build/schema version.

Chỉ những factor được khai báo mới được khác. RNG draw order không được lệch vì treatment off-target.

### ER08 — Determinism và phân nhánh

Mỗi run có `experiment_id`, `run_id`, `parent_run_id`, `fork_tick` và fingerprint đầu vào. Replay từ
genesis hoặc snapshot phải tạo cùng checksum. Seed stream phải derive từ world seed + system +
entity/lineage + event; không dùng `thread_rng()` trên đường deterministic.

### ER09 — “Loài mới” cần bằng chứng, không chỉ khác hình

Không gọi một phenotype, MAP-Elites cell hoặc màu khác là loài mới. `SpeciesClusterRecord` chỉ được
công bố khi policy đã chọn có bằng chứng tối thiểu:

- lineage bền qua số thế hệ tối thiểu;
- khoảng cách genotype/phenotype vượt threshold đã version;
- niche hoặc energy pathway khác có hiệu quả;
- gene flow/mating compatibility giảm nếu reproduction hỗ trợ;
- kết quả ổn định trên ensemble nhiều seed.

Trước gate đó dùng thuật ngữ “morph”, “ecotype” hoặc “candidate species”.

### ER10 — Mọi biến quan sát được phải có metadata

`ObservableRegistry` phải khai báo cho từng biến:

- stable id, tên hiển thị và đơn vị;
- scope: world/region/cell/organism/lineage/species/run;
- source symbol/component;
- sampling cadence và aggregation;
- valid range/tolerance;
- conservation role;
- causal parents có thể có.

UI không tự suy biến từ màu render hoặc dữ liệu trang trí.

### ER11 — Kết quả thí nghiệm phải tự mô tả

`ExperimentResult` phải chứa hoặc tham chiếu:

- input manifest/fingerprint;
- code/model/schema versions;
- seed set và trạng thái từng run;
- time series và final checksum;
- budget audit;
- causal ledger;
- lineage/speciation events;
- summary statistic + confidence interval;
- cảnh báo/failure, không bỏ run lỗi khỏi ensemble một cách im lặng.

### ER12 — Map evidence là gate riêng

Kiến trúc/headless tests không thay thế kiểm định map. Mọi thay đổi hiển thị/placement của exotic
field, organism hoặc ecosystem phải qua `animal-map-vision` theo AGENTS.md. Khi MCP thiếu, trạng thái
phải là `blocked: map evidence unavailable`, không phải pass.

## Data model mục tiêu

```rust
pub struct WorldLawSet {
    pub version: u16,
    pub baseline_energy: BaselineEnergyLaw,
    pub exotic_energy: Option<ExoticEnergyLaw>,
    pub evolution: EvolutionLaw,
    pub speciation: SpeciationPolicy,
}

pub struct ExoticEnergyLaw {
    pub id: String,
    pub display_name: String, // ví dụ "Mana"
    pub unit: String,         // MVP: "MU"
    pub source_model: ExoticSourceModel,
    pub initial_distribution: FieldInitializer,
    pub diffusion_rate: f32,
    pub decay_rate: f32,
    pub max_density: f32,
}

pub struct ExperimentManifest {
    pub schema_version: u16,
    pub name: String,
    pub world_artifact: WorldArtifactRef,
    pub world_laws: WorldLawSet,
    pub initial_conditions: InitialConditionSet,
    pub interventions: Vec<InterventionCommand>,
    pub seeds: Vec<u64>,
    pub duration_ticks: u64,
    pub sampling: SamplingPlan,
    pub observables: Vec<ObservableId>,
}

pub struct EnergyPathwayGenotype {
    pub source_id: String,
    pub sensing_affinity: f32,
    pub uptake_rate: f32,
    pub storage_capacity: f32,
    pub utilization_efficiency: f32,
    pub tolerance: f32,
    pub maintenance_cost_eu: f32,
}
```

Tên field có thể đổi khi implement nếu ADR cập nhật, nhưng ranh giới world law / field /
genotype / phenotype / runtime / experiment result không được gộp.

## Hai kiểu thí nghiệm bắt buộc

### A. Genesis fork — “nếu thế giới từ đầu đã khác”

```text
same artifact + seed set + initial biomass
  ├─ control: exotic_energy=None
  └─ treatment: exotic_energy=Renewable("Mana")
```

Dùng để đo lịch sử tiến hóa khác nhau, niche mới, trait frequency và candidate speciation.

### B. Checkpoint fork — “nếu tác động vào một thế giới đã tiến hóa”

```text
same checkpoint at generation G
  ├─ control: continue unchanged
  └─ treatment: add/remove/pulse exotic source with CauseId
```

Dùng để đo resilience, dependency, extinction debt và khả năng tái thích nghi.

## Gate của feature

| Gate | Bằng chứng bắt buộc |
|---|---|
| **AE-S01** | Exotic energy disabled giữ baseline checksum/tolerance |
| **AE-S02** | Cùng manifest + seed tạo cùng result checksum |
| **AE-S03** | Manifest/fingerprint thay khi world law thay |
| **AE-S04** | MU balance = initial + sources − sinks − storage delta trong tolerance |
| **AE-S05** | Closed EU không nhảy khi uptake/conversion exotic chạy |
| **AE-S06** | Pathway có chi phí: absent-world không nhận free fitness |
| **AE-S07** | Present-world tạo performance difference qua cơ chế, không sửa fitness trực tiếp |
| **AE-S08** | Genesis fork giữ mọi factor ngoài treatment |
| **AE-S09** | Checkpoint fork restore cùng snapshot trước treatment |
| **AE-S10** | Trait frequency/lineage thay đổi là kết quả reproduction/selection |
| **AE-S11** | Species detector không gắn nhãn chỉ vì morphology/color khác |
| **AE-S12** | Causal trace đi từ world law/intervention tới performance rồi reproduction/trait frequency |
| **AE-S13** | UI layer/chart đọc cùng observable registry với backend result |
| **AE-S14** | Ensemble nhiều seed báo effect size, interval và cả run thất bại |
| **AE-S15** | Save/load/migration giữ world-law fingerprint, exotic field và organism storage |

## Cổng chấp nhận

Không gọi feature hoàn tất hoặc tuyên bố “mana làm xuất hiện loài mới” nếu:

- AE-S01…AE-S15 chưa pass theo phase tương ứng;
- live Bevy simulation chưa implement cùng experiment contract với reference model;
- chỉ có một seed hoặc chỉ có ảnh chụp;
- budget EU/MU còn drift không giải thích;
- species policy/threshold chưa version;
- causal chain dừng ở correlation;
- map placement/render chưa qua required map gate.
