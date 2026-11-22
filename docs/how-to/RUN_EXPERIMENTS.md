---
title: Cách chạy thí nghiệm headless bằng anima-lab
status: active
owner: maintainers
last_reviewed: 2026-11-22
review_cycle: per-release
---

# Cách chạy thí nghiệm headless bằng `anima-lab`

Hướng dẫn này trình bày các bước thực hành để biên dịch và chạy CLI headless `anima-lab` cho ba chế độ: **ensemble** (chạy nhiều seed theo một manifest), **sweep** (quét không gian tham số nhiều chiều), và **branch** (cây phân nhánh đối chứng nhân–quả).

> **Lộ trình và hợp đồng liên quan:** nếu bạn mới làm quen từ bản clone sạch, xem [chỉ mục Tutorials](../tutorials/README.md). Các bất biến định lượng nằm tại [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) *(proposed)*, [`DETERMINISM_CONTRACT.md`](../reference/DETERMINISM_CONTRACT.md) và [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).

---

## 1. Biên dịch binary `anima-lab` ở chế độ `--release`

`anima-lab` được định nghĩa trong [`src-tauri/Cargo.toml`](../../src-tauri/Cargo.toml) (`[[bin]] name = "anima-lab"`, mã nguồn tại [`src-tauri/src/bin/anima-lab.rs`](../../src-tauri/src/bin/anima-lab.rs)) và dùng bộ `default` features — **không** cần `--features desktop`, Tauri window, Neo4j hay GPU device.

Từ thư mục gốc của repository, biên dịch binary release rồi in hướng dẫn sử dụng:

<!-- doc-test -->
```bash
cargo build --manifest-path src-tauri/Cargo.toml --release --bin anima-lab
./src-tauri/target/release/anima-lab --help
```

> **Trên Windows PowerShell:** đường dẫn file thực thi là `.\src-tauri\target\release\anima-lab.exe`. Luôn biên dịch rồi gọi trực tiếp file nhị phân thay vì dùng `cargo run` hay `npm run tauri:dev`.

Kết quả `--help` quan sát được nguyên văn từ binary (`macOS arm64`, ngày `2026-11-22`, commit `03b9d3d`):

```text
anima-lab — headless experimentation, multidimensional parameter sweeps, and causal branching

Usage:
    anima-lab [ensemble] [--model reference|live] <MANIFEST.json>
    anima-lab sweep [--model reference|live] [--format json|csv] [--threads N] <SWEEP_MANIFEST.json>
    anima-lab branch [--model reference|live] <BRANCH_MANIFEST.json>
    anima-lab --help

Options:
    --model <reference|live>   Simulation execution model (default: reference)
    --format <json|csv>        Output format for sweep (default: json)
    --threads <N>              Rayon thread pool size for sweeps (default: auto)
    -h, --help                 Print this help text

Exit Codes:
    0: All runs/points completed successfully
    1: Preserved runtime failures or invalid manifest
```

Lưu ý từ mã nguồn của CLI:

- Nếu bỏ qua subcommand, `anima-lab` mặc định chọn `ensemble`.
- `--format` và `--threads` **chỉ hợp lệ** cho subcommand `sweep` (truyền cho `ensemble` hoặc `branch` sẽ báo lỗi với exit code `1`).
- Giới hạn kích thước file manifest đầu vào là `16 MiB` (`16 * 1024 * 1024` byte).
- Mọi kết quả chuẩn (`ensemble` JSON, `sweep` JSON/CSV, `branch` JSON) đều được ghi ra **`stdout`**; thông báo lỗi validation ghi ra **`stderr`**. Nếu một seed thất bại ở runtime trong `ensemble`, `anima-lab` vẫn in JSON đầy đủ ra `stdout` (ghi nhận trong trường `failed`) và thoát với mã `1`.

---

## 2. Chạy `ensemble` trên mô hình `reference` (mặc định)

Khi `--model` là `reference` (hoặc bỏ qua), `anima-lab` dùng `ReferenceEvolutionWorld` cùng registry `ObservableRegistry::reference_default()`. Hai fixture có sẵn trong [`src-tauri/tests/fixtures/experiments/`](../../src-tauri/tests/fixtures/experiments) minh họa trường hợp không có exotic energy và trường hợp có trường `Mana` (`MU`):

<!-- doc-test -->
```bash
mkdir -p src-tauri/target/doc2-runs
./src-tauri/target/release/anima-lab \
  src-tauri/tests/fixtures/experiments/baseline-no-exotic.json \
  > src-tauri/target/doc2-runs/baseline.json
./src-tauri/target/release/anima-lab ensemble --model reference \
  src-tauri/tests/fixtures/experiments/mana-patchy-renewable.json \
  > src-tauri/target/doc2-runs/mana.json
node -e "for (const f of ['baseline.json','mana.json']) { const r = JSON.parse(require('fs').readFileSync('src-tauri/target/doc2-runs/' + f, 'utf8')); console.log(f, { tool: r.tool, model: r.model, id: r.experiment_id, completed: r.completed_runs, failed: r.failed.length, metrics: r.metrics.map(m => [m.observable, Number(m.mean.toFixed(4))]) }); }"
```

Kết quả quan sát được trên `macOS arm64` (chạy ngày `2026-11-22` tại commit `03b9d3d` với hai lệnh `anima-lab` ở trên, rút gọn qua Node):

```text
baseline.json {
  tool: 'anima-lab',
  model: 'reference',
  id: 'exp-1',
  completed: 3,
  failed: 0,
  metrics: [
    [ 'precip', 1 ],
    [ 'temperature', 0.5 ],
    [ 'npp', 1.0017 ],
    [ 'plants', 100.0939 ],
    [ 'herbivores', 36.3957 ],
    [ 'predators', 7.2839 ],
    [ 'detritus', 0.7081 ]
  ]
}
mana.json {
  tool: 'anima-lab',
  model: 'reference',
  id: 'ae-mana-patchy',
  completed: 3,
  failed: 0,
  metrics: [
    [ 'precip', 1 ],
    [ 'temperature', 0.5 ],
    [ 'npp', 1.0017 ],
    [ 'plants', 100.0939 ],
    [ 'herbivores', 36.3957 ],
    [ 'predators', 7.2839 ],
    [ 'detritus', 0.7081 ],
    [ 'exotic.density_total', 109.3622 ],
    [ 'exotic.sourced', 87.9029 ],
    [ 'exotic.dissipated', 128.5406 ],
    [ 'exotic.stored', 0 ],
    [ 'exotic.budget_error', -0 ]
  ]
}
```

Các trường chính ở tầng gốc của `EnsembleReport` JSON: `tool`, `manifest`, `model`, `registry`, `experiment_id`, `manifest_fingerprint`, `requested_runs`, `completed_runs`, `failed`, `seed_order`, `runs` (chứa time series và `final_observables` từng seed), và `metrics` (thống kê tổng hợp theo từng observable).

---

## 3. Chạy `ensemble` trên mô hình `live`

Để lái `LiveExperimentAdapter` (thế giới Bevy headless trên đúng `simulation_schedule::build_tick_schedule`), truyền `--model live`. Registry chuyển sang `ObservableRegistry::live_default()` và `initial_conditions` chỉ chấp nhận danh sách khóa `live.*`:

<!-- doc-test -->
```bash
mkdir -p src-tauri/target/doc2-runs
./src-tauri/target/release/anima-lab --model live \
  src-tauri/tests/fixtures/experiments_e2/e2-smoke-control-shared-brain.json \
  > src-tauri/target/doc2-runs/e2-live.json
node -e "const r = JSON.parse(require('fs').readFileSync('src-tauri/target/doc2-runs/e2-live.json','utf8')); console.log({ model: r.model, registry: r.registry, completed: r.completed_runs, final_count: r.runs[0].final_observables.length, final_head: r.runs[0].final_observables.slice(0, 5) });"
```

Kết quả quan sát được trên `macOS arm64` (chạy ngày `2026-11-22` tại commit `03b9d3d` với lệnh `--model live` ở trên, 1 seed × 18.000 tick):

```text
{
  model: 'live',
  registry: 'core::experiment::ObservableRegistry::live_default',
  completed: 1,
  final_count: 27,
  final_head: [
    [ 'plants', 154951.8024202675 ],
    [ 'detritus', 1019.9974421173067 ],
    [ 'live.animals_eu', 0 ],
    [ 'live.dormant_eu', 0 ],
    [ 'live.closed_eu_total', 155971.7998623848 ]
  ]
}
```

> **Lưu ý giới hạn model:** chạy `--model live` trên `mana-patchy-renewable.json` bị từ chối ngay (`observable 'herbivores' is not in the registry`, exit `1`), và `LiveExperimentAdapter` cũng từ chối mọi manifest có `laws.exotic_energy != null`. Để tái lập đầy đủ thí nghiệm E2 (ghép cặp 12 seed, kiểm chứng `E2-G1..G6`, `npm run check:e2`), xem [REPRODUCE_E2.md](REPRODUCE_E2.md).

---

## 4. Chạy `sweep` (xuất JSON và CSV, kiểm soát `--threads`)

Repository hiện **chưa** có file `.json` fixture riêng cho `SweepManifest` dưới `src-tauri/tests/fixtures/` (các sweep test hiện dựng manifest bằng mã Rust). Để chạy ngay một lượt quét `Sobol` 4 điểm trên hai chiều `precip` và `plants`, ghi manifest vào thư mục gitignored `src-tauri/target/doc2-runs/` rồi chạy cả `--format json` và `--format csv --threads 2`:

<!-- doc-test -->
```bash
mkdir -p src-tauri/target/doc2-runs
cat <<'EOF' > src-tauri/target/doc2-runs/sweep-sobol.json
{
  "schema_version": 1,
  "sweep_id": "doc2-sweep-precip",
  "name": "sobol-precip-plants",
  "base_manifest": {
    "schema_version": 1,
    "experiment_id": "doc2-sweep-base",
    "name": "sweep-base",
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
    "interventions": [],
    "seeds": [1],
    "duration_ticks": 600,
    "sample_period": 300,
    "observable_ids": ["plants", "herbivores"],
    "exotic_interventions": [],
    "observer": { "mode": "absent" }
  },
  "parameters": [
    {
      "name": "precip_init",
      "target": { "type": "InitialCondition", "key": "precip" },
      "spec": { "scale": "Continuous", "min": 0.5, "max": 1.5 }
    },
    {
      "name": "plants_init",
      "target": { "type": "InitialCondition", "key": "plants" },
      "spec": { "scale": "Continuous", "min": 80.0, "max": 120.0 }
    }
  ],
  "sampling": { "strategy": "Sobol", "num_samples": 4, "offset": 0 },
  "max_threads": 2
}
EOF
./src-tauri/target/release/anima-lab sweep --format json \
  src-tauri/target/doc2-runs/sweep-sobol.json \
  > src-tauri/target/doc2-runs/sweep.json
node -e "const s = JSON.parse(require('fs').readFileSync('src-tauri/target/doc2-runs/sweep.json','utf8')); console.log({ sweep_id: s.sweep_id, total_points: s.total_points, completed_points: s.completed_points, failed_runs: s.failed_runs, params: s.parameter_names, plants_corr: s.global_metrics.plants.parameter_correlations });"
./src-tauri/target/release/anima-lab sweep --format csv --threads 2 \
  src-tauri/target/doc2-runs/sweep-sobol.json \
  > src-tauri/target/doc2-runs/sweep.csv
head -n 5 src-tauri/target/doc2-runs/sweep.csv
```

Kết quả quan sát được trên `macOS arm64` (chạy ngày `2026-11-22` tại commit `03b9d3d` với lệnh `sweep` ở trên):

```text
{
  sweep_id: 'doc2-sweep-precip',
  total_points: 4,
  completed_points: 4,
  failed_runs: 0,
  params: [ 'precip_init', 'plants_init' ],
  plants_corr: [ -0.8285714285714282, 1 ]
}
point_index,precip_init,plants_init,completed_runs,failed_runs,detritus_mean,detritus_std,detritus_min,detritus_max,herbivores_mean,herbivores_std,herbivores_min,herbivores_max,...
0,1,100,1,0,2.3280312064081556,0,2.3280312064081556,2.3280312064081556,37.431961750649805,0,...
1,1.25,90,1,0,2.9911463719847187,0,2.9911463719847187,2.9911463719847187,36.67012882527874,0,...
2,0.75,110,1,0,1.6649160408315915,0,1.6649160408315915,1.6649160408315915,38.19379467602087,0,...
3,0.875,95,1,0,2.6595887891964374,0,2.6595887891964374,2.6595887891964374,37.05104528796427,0,...
```

Lưu ý đọc kết quả `sweep`:

- Trong `SweepResult` JSON, `global_metrics` là một object ánh xạ `ten_observable -> { mean, std, min, max, parameter_correlations }`, trong đó `parameter_correlations` xếp đúng theo thứ tự `parameter_names`.
- Khi xuất `--format csv`, mỗi điểm chiếm một hàng gồm `point_index,<tên tham số...>,completed_runs,failed_runs,<obs>_mean,<obs>_std,<obs>_min,<obs>_max` theo thứ tự chữ cái của observable.
- Nếu có bất kỳ run nào trong sweep thất bại (`failed_runs > 0`), CLI thoát với mã `1`.

---

## 5. Chạy `branch` (cây phân nhánh đối chứng nhân–quả)

Tương tự `sweep`, hiện chưa có file JSON `BranchTreeManifest` sẵn trong `src-tauri/tests/fixtures/`. Ví dụ dưới đây dựng một cây hai nhánh (`id: 1` gốc chạy 600 tick; `id: 2` rẽ nhánh tại `fork_tick: 300`, giữ nguyên luồng RNG `InheritStream` và áp dụng `Deforest` từ tick `301` trong `120` tick) rồi tính phân kỳ cặp `(1, 2)`:

<!-- doc-test -->
```bash
mkdir -p src-tauri/target/doc2-runs
cat <<'EOF' > src-tauri/target/doc2-runs/branch-tree.json
{
  "schema_version": 1,
  "tree_id": "doc2-branch-deforest",
  "name": "deforest-counterfactual",
  "base_manifest": {
    "schema_version": 1,
    "experiment_id": "doc2-branch-base",
    "name": "branch-base",
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
    "interventions": [],
    "seeds": [1],
    "duration_ticks": 600,
    "sample_period": 300,
    "observable_ids": ["plants", "herbivores", "detritus"],
    "exotic_interventions": [],
    "observer": { "mode": "absent" }
  },
  "nodes": [
    {
      "id": 1,
      "parent_id": null,
      "fork_tick": 0,
      "duration_ticks": 600,
      "interventions": [],
      "exotic_interventions": [],
      "seed_strategy": { "type": "InheritStream" },
      "label": "baseline"
    },
    {
      "id": 2,
      "parent_id": 1,
      "fork_tick": 300,
      "duration_ticks": 600,
      "interventions": [
        {
          "id": 10,
          "cause_id": 500,
          "kind": "Deforest",
          "region": "Global",
          "start_tick": 301,
          "duration_ticks": 120,
          "intensity": 0.4,
          "signed_negative": false,
          "curve": "Step",
          "reversible": false
        }
      ],
      "exotic_interventions": [],
      "seed_strategy": { "type": "InheritStream" },
      "label": "deforest-t301"
    }
  ],
  "comparisons": [[1, 2]]
}
EOF
./src-tauri/target/release/anima-lab branch \
  src-tauri/target/doc2-runs/branch-tree.json \
  > src-tauri/target/doc2-runs/branch.json
node -e "const b = JSON.parse(require('fs').readFileSync('src-tauri/target/doc2-runs/branch.json','utf8')); console.log({ tree_id: b.tree_id, root_id: b.root_id, nodes: Object.keys(b.nodes).map(k => ({ id: k, label: b.nodes[k].spec.label, status: b.nodes[k].status })), phase_space: b.divergences[0].phase_space, plants_div: b.divergences[0].observables.find(o => o.observable_id === 'plants') });"
```

Kết quả quan sát được trên `macOS arm64` (chạy ngày `2026-11-22` tại commit `03b9d3d` với lệnh `branch` ở trên):

```text
{
  tree_id: 'doc2-branch-deforest',
  root_id: 1,
  nodes: [
    { id: '1', label: 'baseline', status: 'Completed' },
    { id: '2', label: 'deforest-t301', status: 'Completed' }
  ],
  phase_space: {
    mean_distance: 3.854512240980469,
    max_distance: 7.709024481960938,
    final_distance: 7.709024481960938,
    trajectory_length_a: 1.2407503661557848,
    trajectory_length_b: 8.121790624402612
  },
  plants_div: {
    observable_id: 'plants',
    mean_delta: 3.6893211964892245,
    max_abs_delta: 7.378642392978449,
    rmsd: 5.217488072025596,
    relative_divergence: 1.4211062734777986,
    dtw_distance: 7.378642392978449
  }
}
```

---

## 6. Kiểm tra đối chứng âm (Negative Control) với manifest lỗi

Luôn kiểm tra manifest lỗi bị từ chối với mã thoát `1` thay vì chạy thầm lặng. Fixture [`invalid-negative-source.json`](../../src-tauri/tests/fixtures/experiments/invalid-negative-source.json) cố tình đặt `source_rate: -0.05`:

<!-- doc-test -->
```bash
mkdir -p src-tauri/target/doc2-runs
! ./src-tauri/target/release/anima-lab \
  src-tauri/tests/fixtures/experiments/invalid-negative-source.json \
  > src-tauri/target/doc2-runs/err-out.txt \
  2> src-tauri/target/doc2-runs/err.txt
cat src-tauri/target/doc2-runs/err.txt
grep -q "exotic energy source_rate must be >= 0" src-tauri/target/doc2-runs/err.txt
```

Kết quả quan sát được trên `stderr` (`macOS arm64`, chạy ngày `2026-11-22` tại commit `03b9d3d`, exit code `1`, `stdout` rỗng):

```text
anima-lab: ensemble rejected: invalid world law: exotic energy source_rate must be >= 0 (got -0.05)
```

---

## Bước tiếp theo

- Tự viết hoặc chỉnh sửa manifest: [WRITE_A_MANIFEST.md](WRITE_A_MANIFEST.md).
- Tái lập thí nghiệm E2 đã tiền đăng ký: [REPRODUCE_E2.md](REPRODUCE_E2.md).
- Danh sách hợp đồng kiến trúc & thí nghiệm: [chỉ mục Reference](../reference/README.md) và [chỉ mục Explanation](../explanation/README.md).
