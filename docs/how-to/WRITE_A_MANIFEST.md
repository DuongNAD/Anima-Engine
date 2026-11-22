---
title: Cách viết và kiểm chứng Experiment Manifest
status: active
owner: maintainers
last_reviewed: 2026-11-22
review_cycle: per-release
---

# Cách viết và kiểm chứng Experiment Manifest

Hướng dẫn này mô tả từng bước tạo mới, kiểm chứng và chạy thử một `ExperimentManifest` (đơn lẻ hoặc cặp control/treatment), cũng như cách bọc nó thành `SweepManifest` hoặc `BranchTreeManifest`.

> **Hợp đồng chuẩn:** tài liệu này bám sát các struct thực tế trong [`src-tauri/src/core/experiment/`](../../src-tauri/src/core/experiment) (`manifest.rs`, `laws.rs`, `interventions.rs`, `factor_diff.rs`), [`sweeps.rs`](../../src-tauri/src/core/sweeps.rs) và [`branch_tree.rs`](../../src-tauri/src/core/branch_tree.rs). Các invariant khoa học (`ER01`–`ER11`) nằm tại [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) *(proposed)*.

---

## 1. Chọn đúng loại manifest

| Nhu cầu | Struct / Subcommand | Khi nào dùng |
|---|---|---|
| Chạy 1 cấu hình trên 1 hoặc nhiều seed | `ExperimentManifest` (`anima-lab ensemble`) | Thử nghiệm đơn, baseline hoặc 1 nhánh của cặp đối chứng |
| Chạy đối chứng ghép cặp | 2 file `ExperimentManifest` (`control` + `treatment`) | So sánh cùng `seeds` theo thứ tự, chỉ khác factor đã khai báo (`FactorDiff`) |
| Quét không gian tham số nhiều chiều | `SweepManifest` (`anima-lab sweep`) | Quét `Grid`, `Sobol`, hoặc `LatinHypercube` trên `base_manifest` |
| Phân nhánh đối chứng tại tick `fork_tick` | `BranchTreeManifest` (`anima-lab branch`) | Chia nhánh từ checkpoint tiền tố chung và đo phân kỳ |

---

## 2. Cấu trúc bắt buộc của `ExperimentManifest`

`ExperimentManifest` gắn `#[serde(deny_unknown_fields)]` và `schema_version: 1` (`MANIFEST_SCHEMA_VERSION`), nên bất kỳ khóa lạ hoặc sai kiểu đều bị từ chối ngay khi parse JSON.

| Trường | Bắt buộc | Kiểu & Ràng buộc |
|---|---|---|
| `schema_version` | Có | Số nguyên `1` |
| `experiment_id` | Có | Chuỗi khác rỗng (không ảnh hưởng `manifest_fingerprint`) |
| `name` | Có | Chuỗi khác rỗng (không ảnh hưởng `manifest_fingerprint`) |
| `world_identity` | Có | `{ seed, generator_version, width, height, checksum }` |
| `laws` | Có | `WorldLawSet` (`schema_version: 1`, `baseline_energy`, `exotic_energy`) |
| `initial_conditions` | Có | `InitialConditionSet` (`schema_version: 1`, `values: [[key, f64], ...]`) |
| `interventions` | Có | Mảng `InterventionCommand` (tối đa `4096`, `id` duy nhất) |
| `seeds` | Có | Mảng `u64` khác rỗng, tối đa `1024`, không trùng lặp |
| `duration_ticks` | Có | `1..=100_000_000` |
| `sample_period` | Có | `u64` (`0` = không lấy chuỗi thời gian, vẫn giữ `final_observables`) |
| `observable_ids` | Có | Mảng chuỗi duy nhất, tối đa `4096`, mọi ID phải có trong `ObservableRegistry` tương ứng với `--model` |
| `exotic_interventions` | Tùy chọn | Mặc định `[]`; chỉ được khác rỗng khi `laws.exotic_energy != null` |
| `observer` | Tùy chọn | `ObserverPolicy`, mặc định `{"mode": "absent"}` |

> **Giới hạn bộ nhớ preflight:** manifest ước lượng vượt quá `MAX_ENSEMBLE_RESULT_BYTES = 2 GiB` (`seeds × samples × observables`) bị từ chối trước khi chạy.

---

## 3. Khai báo `laws` (`WorldLawSet`)

- **`baseline_energy`:** bắt buộc `{"eu_unit": "EU"}`.
- **`exotic_energy`:** đặt `null` cho baseline regime, hoặc khai báo `ExoticEnergyLaw` (chỉ hỗ trợ trên `--model reference`; `--model live` từ chối mọi manifest có `exotic_energy != null`):

```json
"exotic_energy": {
  "id": "arcane_flux",
  "display_name": "Mana",
  "unit": "MU",
  "source_model": "Renewable",
  "topology": { "Patchy": { "hotspot_count": 4, "radius_cells": 2.5 } },
  "initial_amount": 150.0,
  "source_rate": 0.05,
  "diffusion_rate": 0.1,
  "decay_rate": 0.01,
  "max_density": 10.0,
  "boundary": "Closed"
}
```

Ràng buộc kiểm tra trong `ExoticEnergyLaw::validate()`:
- `id` và `display_name` khác rỗng; `unit` khác rỗng và **không** được là `"EU"`.
- `source_model` hiện chỉ hỗ trợ `"Renewable"`.
- `topology` là `"Uniform"` hoặc `{"Patchy": {"hotspot_count": >= 1, "radius_cells": > 0.0}}`.
- `boundary` là `"Closed"` hoặc `"Open"`.
- `initial_amount >= 0.0`, `source_rate >= 0.0`, `0.0 <= diffusion_rate <= 0.25`, `0.0 <= decay_rate <= 1.0`, `max_density > 0.0` (tất cả phải hữu hạn).

---

## 4. Khai báo `initial_conditions` theo model

`InitialConditionSet` gồm `{"schema_version": 1, "values": [["<key>", <f64>], ...]}` — không trùng khóa và mọi giá trị phải hữu hạn.

### 4.1 Với `--model reference`

- **7 khóa sinh thái nền:** `precip`, `temperature`, `npp`, `plants`, `herbivores`, `predators`, `detritus` (khóa nào bỏ trống dùng mặc định của `ReferenceEcosystem`).
- **13 khóa tiến hóa AE3 (tùy chọn, opt-in):** `ae3.population_total`, `ae3.population_capacity`, `ae3.pathway_fraction`, `ae3.generation_ticks`, `ae3.base_performance`, `ae3.mutation_rate`, `ae3.pathway_sensing`, `ae3.pathway_uptake`, `ae3.pathway_storage`, `ae3.pathway_efficiency`, `ae3.pathway_tolerance`, `ae3.pathway_maintenance`, `ae3.pathway_allocation`. Lưu ý: nếu `observable_ids` yêu cầu `evolution.*` mà thiếu `ae3.population_total`, validation thất bại ngay.

### 4.2 Với `--model live`

`LiveExperimentAdapter` áp dụng danh sách **29 khóa `LIVE_KEYS` chặt chẽ** (`src-tauri/src/core/live_experiment.rs`); bất kỳ khóa lạ nào (kể cả `plants` hay `precip`) đều bị từ chối:

- Quần thể & bản đồ sáng lập: `live.founders`, `live.founder_layout`, `live.predator_fraction`, `live.trees`, `live.lakes`, `live.lake_layout`, `live.food_cap`, `live.land_mask`, `live.resource_capacity_scale`, `live.plant_reserve`.
- Não, hành vi & sinh học: `live.evolved_brains`, `live.mortality`, `live.reproduction`, `live.locomotion`, `live.drive_floor`, `live.thirst_steering`, `live.grass_steering`, `live.mutation_rate`, `live.starve_ticks`, `live.dehydration_ticks`, `live.ecological_fitness`, `live.w_surplus`, `live.kill_hydration`, `live.kill_efficiency`, `live.target_min_energy`, `live.prey_cap`, `live.strict_prey_cap`.
- Đối chứng drift: `live.drift_control`, `live.drift_reset_ticks`.

**Ràng buộc ghép đôi trong `--model live`:**
- `live.lake_layout = 1.0` yêu cầu `live.lakes >= 1`.
- `live.drift_control = 1.0` yêu cầu cả `live.mortality = 1.0` và `live.reproduction = 1.0`.
- `live.drift_reset_ticks > 0` yêu cầu `live.drift_control = 1.0`.
- `live.grass_steering = 1.0` yêu cầu `live.locomotion = 1.0`.
- Yêu cầu các observable `live.deaths*` / `live.mean_lifespan` đòi hỏi `live.mortality = 1.0` (và không bật `live.drift_control` đối với `deaths_starvation` / `deaths_dehydration`); `live.births` đòi hỏi `live.reproduction = 1.0`.

---

## 5. Khai báo `interventions`, `exotic_interventions` và `observer`

- **`InterventionCommand`:** mỗi phần tử có `id` duy nhất, `cause_id` (khác `CAUSE_OBSERVER`), `kind` (1 trong 8 biến thể như `"RainfallDelta"`, `"TemperatureDelta"`, `"Deforest"`, `"RemovePredators"`, `"AddNutrient"`, hoặc các biến thể có payload như `GeneticInjection`, `ToxicityPlume`, `PathogenTransmission`), `region` (`"Global"`, `{"Cell": {"x": ..., "y": ...}}`, `{"Rect": {...}}`, `{"Radius": {...}}`), `start_tick`, `duration_ticks` (`0` = 1 tick), `intensity` (`>= 0.0`, hữu hạn; chiều tăng/giảm do `signed_negative` quyết định), `signed_negative`, `curve` (`"Step"`, `"RampUp"`, `"RampDown"`, `"Triangle"`), `reversible`. Cửa sổ `[start_tick, start_tick + max(duration_ticks, 1))` phải giao với `1..=duration_ticks`.
- **`ExoticIntervention`:** dùng riêng cho trường `MU` (`AddSource`, `RemoveSource`, `Pulse`), đòi hỏi `laws.exotic_energy != null` và cửa sổ hoạt động phải phủ ít nhất một tick `ECOLOGY_PERIOD`.
- **`ObserverPolicy`:** dùng `{"mode": "absent"}` cho chạy headless chuẩn; `{"mode": "spectate"}` phải khớp quỹ đạo với `Absent`; `{"mode": "inhabit", "cause_id": <u32>}` cấm `cause_id == 0` (`CAUSE_BACKGROUND`).

---

## 6. Ghép cặp Control / Treatment (`FactorDiff`)

Khi tạo cặp manifest `control` và `treatment` (như bộ fixture E2):
1. `FactorDiff::diff_paths` bỏ qua `experiment_id` và `name`, nhưng kiểm tra toàn bộ 12 đường dẫn nhân tố còn lại (`observer`, `world_identity`, `laws.*`, `initial_conditions`, `interventions`, `exotic_interventions`, `seeds`, `duration_ticks`, `sample_period`, `observables`).
2. `run_paired_ensemble_with_control` bắt buộc `control.seeds == treatment.seeds` **cùng thứ tự**.
3. Chỉ nhân tố đã đăng ký mới được phép khác giữa hai file.

---

## 7. Mở rộng thành `SweepManifest` hoặc `BranchTreeManifest`

- **`SweepManifest`** (`#[serde(deny_unknown_fields)]`): bọc `base_manifest` cùng `parameters` (1–32 chiều, mỗi chiều gồm `name`, `target` là `InitialCondition` / `ExoticLawField` / `InterventionIntensity` / `InterventionStartTick` / `InterventionDuration`, và `spec` là `Continuous` / `Logarithmic` / `IntegerRange` / `Discrete`), `sampling` (`Grid`, `Sobol`, hoặc `LatinHypercube`), và `max_threads` tùy chọn. Tổng số điểm tối đa `100_000`.
- **`BranchTreeManifest`** (`#[serde(deny_unknown_fields)]`): bọc `base_manifest`, mảng `nodes` (1–1024 node, đúng 1 node gốc `parent_id: null` với `fork_tick: 0`, các node con thỏa mãn `parent.fork_tick <= child.fork_tick <= parent.duration_ticks`, mọi intervention trên node con phải có `start_tick >= fork_tick` và chạy trong `(fork_tick, duration_ticks]`), và `comparisons: [[id_a, id_b], ...]`. `seed_strategy` hỗ trợ `InheritStream`, `DerivedStream { stream_id }`, hoặc `ExplicitSeed { seed }`.

---

## 8. Kiểm chứng thực tế bằng `anima-lab` (kèm negative control)

Đoạn lệnh sau tự dựng một manifest hợp lệ có `InterventionCommand` (`RainfallDelta` giảm dần theo `RampUp`), chạy qua `anima-lab`, và xác minh manifest lỗi bị từ chối:

<!-- doc-test -->
```bash
cargo build --manifest-path src-tauri/Cargo.toml --release --bin anima-lab
mkdir -p src-tauri/target/doc2-runs
cat <<'EOF' > src-tauri/target/doc2-runs/custom-manifest.json
{
  "schema_version": 1,
  "experiment_id": "doc2-custom-drought",
  "name": "drought-test",
  "world_identity": {
    "seed": 1337,
    "generator_version": 20,
    "width": 128,
    "height": 128,
    "checksum": 3735928559
  },
  "laws": {
    "schema_version": 1,
    "baseline_energy": { "eu_unit": "EU" },
    "exotic_energy": null
  },
  "initial_conditions": {
    "schema_version": 1,
    "values": [
      ["precip", 1.0],
      ["temperature", 0.5],
      ["plants", 100.0],
      ["herbivores", 40.0],
      ["predators", 8.0],
      ["detritus", 0.0]
    ]
  },
  "interventions": [
    {
      "id": 1,
      "cause_id": 100,
      "kind": "RainfallDelta",
      "region": "Global",
      "start_tick": 120,
      "duration_ticks": 300,
      "intensity": 0.3,
      "signed_negative": true,
      "curve": "RampUp",
      "reversible": true
    }
  ],
  "seeds": [1, 2],
  "duration_ticks": 600,
  "sample_period": 300,
  "observable_ids": ["precip", "plants", "herbivores"],
  "exotic_interventions": [],
  "observer": { "mode": "absent" }
}
EOF
./src-tauri/target/release/anima-lab \
  src-tauri/target/doc2-runs/custom-manifest.json \
  > src-tauri/target/doc2-runs/custom-out.json
node -e "const r = JSON.parse(require('fs').readFileSync('src-tauri/target/doc2-runs/custom-out.json','utf8')); console.log({ id: r.experiment_id, fingerprint: r.manifest_fingerprint, completed: r.completed_runs, metrics: r.metrics.map(m => [m.observable, Number(m.mean.toFixed(4))]) });"
! ./src-tauri/target/release/anima-lab \
  src-tauri/tests/fixtures/experiments/invalid-negative-source.json \
  2> src-tauri/target/doc2-runs/manifest-err.txt
grep -q "exotic energy source_rate must be >= 0" src-tauri/target/doc2-runs/manifest-err.txt
```

Kết quả quan sát được trên `macOS arm64` (chạy ngày `2026-11-22` tại commit `03b9d3d` với chuỗi lệnh `anima-lab` ở trên):

```text
{
  id: 'doc2-custom-drought',
  fingerprint: 12352719949087382000,
  completed: 2,
  metrics: [
    [ 'precip', 1 ],
    [ 'temperature', 0.5 ],
    [ 'npp', 0.9672 ],
    [ 'plants', 94.803 ],
    [ 'herbivores', 36.4096 ],
    [ 'predators', 7.825 ],
    [ 'detritus', 3.2859 ]
  ]
}
```

---

## Bước tiếp theo

- Chạy `ensemble`, `sweep`, `branch`: [RUN_EXPERIMENTS.md](RUN_EXPERIMENTS.md).
- Thêm biến quan sát mới: [ADD_AN_OBSERVABLE.md](ADD_AN_OBSERVABLE.md).
- Thêm can thiệp mới: [ADD_AN_INTERVENTION.md](ADD_AN_INTERVENTION.md).
