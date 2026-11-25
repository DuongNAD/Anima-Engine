# FINDINGS — bug thật tìm được

Đây là metric duy nhất thật sự quan trọng. Mutation score, coverage, số warning
đều chỉ là proxy. Nếu sau một tuần file này vẫn rỗng → verifier chọn sai mục
tiêu, đổi mục tiêu chứ đừng đổ thêm token.

Quy tắc ghi:
- Chỉ ghi khi có test tái hiện được, hoặc log crash cụ thể.
- `Loại` chỉ được là một trong:
  - `BUG` — hành vi sai thật, đã chứng minh
  - `SMELL` — đáng ngờ, chưa chứng minh
  - `TEST-GAP` — code đúng, test thiếu (**không tính vào mục tiêu bug**)
- Không có `file:line` và lệnh tái hiện thì không được ghi.

---

## Mẫu (copy khối này, đừng sửa mẫu gốc)

### F-000 — <tiêu đề một dòng>
- **Ngày:** ____-__-__
- **Loại:** BUG | SMELL | TEST-GAP
- **Vị trí:** `<path>:<line>`
- **Phát hiện bởi:** mutant | fuzz | miri | determinism | dataflow | tay
- **Triệu chứng:** <1–2 câu, hành vi quan sát được sai như thế nào>
- **Tái hiện:** `just ...`
- **Test khoá lại:** `<test file>::<tên test>`
- **Trạng thái:** OPEN | FIXED (<commit>)

---

<!-- ghi mục mới ngay dưới đây -->

### F-081 — `physics_tests::test_spatial_grid_zero_allocation` process-wide tracking flake + `TEST_LOCK` `PoisonError` cascade
- **Ngày:** 2026-11-25
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/tests/physics_tests.rs:528-536` (`test_spatial_grid_zero_allocation` used process-wide `ALLOCATOR.start_tracking()` around single-threaded `grid.raycast`)
  - `src-tauri/tests/physics_tests.rs:24,107,187,267,416,493,541,686,763,855` (`TEST_LOCK.lock().unwrap()` poisoned sibling tests on any panic)
- **Phát hiện bởi:** determinism (`hardening` -> `determinism` on PR `#273` at trunk `a146d2f2`, run `https://github.com/DuongNAD/Anima-Engine/actions/runs/45516447719`)
- **Triệu chứng:** On run 1 of "kiểm ổn định (5 lần)", `physics_tests::test_spatial_grid_zero_allocation` failed with `"Raycast should perform 0 heap allocations, but made 1"`. That panic poisoned `TEST_LOCK`, causing `test_static_equilibrium`, `test_zero_allocation_hot_path`, and `test_zero_joint_axis_no_nan` to fail with `PoisonError`.
  - **Root cause:** As documented in `src-tauri/tests/common/allocator.rs`, `start_tracking()` counts allocations from every thread in the process; in a multi-test binary, libtest's main thread allocates while handling harness events and can land inside the single-threaded `grid.raycast` measurement window. Meanwhile, `.unwrap()` on `TEST_LOCK` turned one failure into a 4-test cascade.
- **Tái hiện:** `cargo test --manifest-path src-tauri/Cargo.toml --release --test physics_tests` (intermittent under `common/allocator.rs` process-wide tracking; CI run `45516447719`).
- **Test khoá lại:** `physics_tests::test_spatial_grid_zero_allocation` (switched to `ALLOCATOR.start_tracking_this_thread()` on the single-threaded raycast window while keeping process-wide `ALLOCATOR.start_tracking()` on both `Schedule` windows `test_zero_allocation_hot_path` and `test_decoupled_systems_zero_allocation`; recovered `TEST_LOCK` via `unwrap_or_else(|e| e.into_inner())`; negative control with `Vec::<u8>::with_capacity(1)` fails with `"made 1"`; `20/20` `--release` stability runs pass).
- **Trạng thái:** FIXED (`P11 PHYSICS-ALLOC-FLAKE`, branch `win/p11-physics-alloc-flake`, 2026-11-25).

### F-080 — `BrainPolicy.lifetime_learning` cannot be enabled from an experiment manifest (`ANIMA_LIFETIME_LEARNING` env-only / always `false` in `LiveWorldConfig`)
- **Ngày:** 2026-11-24
- **Loại:** SMELL
- **Vị trí:**
  - `src-tauri/src/core/live_experiment.rs:2637-2641,2753-2762`: `build_live_world` constructs `BrainPolicy` without enabling `lifetime_learning` (`LifetimeLearning::default()`, i.e. `enabled: false`) and never reads ambient `ANIMA_LIFETIME_LEARNING`; `LiveWorldConfig` defines no `lifetime_learning` field or `live.lifetime_learning` manifest key.
  - `src-tauri/src/core/resources.rs:812-835,861-899`: `BrainPolicy.lifetime_learning` defaults to `enabled: false` and is only populated from `ANIMA_LIFETIME_LEARNING` in `BrainPolicy::from_env()` (requiring `learning && evolved`).
  - `src-tauri/src/ai/model.rs:1115-1122`: `hrrl_learning_system` skips Baldwin/in-life weight updates whenever `!(policy.evolved && policy.lifetime_learning.enabled && policy.lifetime_learning.interval > 0)`.
- **Phát hiện bởi:** tay (biology audit on trunk `071ef25`)
- **Triệu chứng:** Even with `live.evolved_brains = 1.0`, experiment manifests have no declared key to enable or parameterize `BrainPolicy.lifetime_learning`, because `build_live_world` strictly rejects ambient environment variables so manifests stay self-contained.
  - **Impact:** No preregistered headless experiment manifest can test Baldwin-effect hypotheses or interactions between inherited brains (`live.evolved_brains`) and in-life learning.
  - **Frozen experiments / artifacts (`git grep lifetime_learning` & `evolved_brains` in `src-tauri/tests/fixtures` and `artifacts/`):** **0** manifests or artifacts reference `lifetime_learning`; **68** `evolved_brains` files across E-SEL1 (`15`), E2 (`18`), E3 (`13`), E3b (`14`), and SEL2-P (`8`), plus `src-tauri/tests/fixtures/snapshots/schema7_burn013_shared_learner.json`, all run with `lifetime_learning.enabled == false`.
  - **Proposed fix (default-preserving):** Add declared opt-in manifest key(s) such as `"live.lifetime_learning"` (and any needed interval/rate keys) parsed into `LiveWorldConfig`; when absent or `0.0`, keep `LifetimeLearning::default()` (`enabled: false`) so all `68` frozen `evolved_brains` fixtures and shared-brain runs remain bit-identical, and enforce `live.evolved_brains = 1.0` at preflight when `"live.lifetime_learning" = 1.0` (since learning without per-agent brains is a no-op per `resources.rs:892-894`).
- **Tái hiện:** Static audit of `src-tauri/src/core/live_experiment.rs:2637-2641,2753-2762` and `src-tauri/src/core/resources.rs:861-899`.
- **Test khoá lại:** chưa có
- **Trạng thái:** OPEN

### F-079 — Morphology mutation stops adding nodes at `nodes.len() < 15`, whereas crossover and migration allow up to `128`
- **Ngày:** 2026-11-24
- **Loại:** SMELL
- **Vị trí:**
  - `src-tauri/src/evolution/mutation.rs:161-163`: `add_node` mutation only runs while `genotype.nodes.len() < 15`.
  - `src-tauri/src/evolution/crossover.rs:161-163`: structural crossover permits offspring up to `crate::core::components::MAX_MIGRATION_MORPHOLOGY_NODES` (`128`).
  - `src-tauri/src/core/components.rs:189-192` & `src-tauri/src/core/agent_systems.rs:20-25`: `MAX_MIGRATION_MORPHOLOGY_NODES = 128` and `BODY_WALK_CAPACITY = MAX_MIGRATION_MORPHOLOGY_NODES` explicitly note the gap against the `15`-node live mutation cap.
- **Phát hiện bởi:** tay (biology audit on trunk `071ef25`)
- **Triệu chứng:** Morphological complexity growth through live point mutation stops hard at `15` nodes (`mutation.rs:163`), whereas `crossover.rs:161-163` and restore/migration bounds allow up to `128` nodes (`MAX_MIGRATION_MORPHOLOGY_NODES`).
  - **Impact:** Live lineages started from stock `2`- or `3`-segment founders can never evolve beyond `15` nodes via mutation, while imported or crossed morphologies between `15` and `128` nodes behave inconsistently (cannot grow further by mutation) and reflect two undocumented ceilings.
  - **Frozen experiments / artifacts (`git grep` in `src-tauri/tests/fixtures` and `artifacts/`):** **0** manifest keys configure this limit directly, though every frozen evolutionary run with `live.mutation_rate > 0` executes `mutation.rs:163`.
  - **Proposed fix (default-preserving):** Changing `15` unconditionally would alter mutation RNG draws and body size in any lineage reaching `15` nodes. Keep default `15` bit-identical for all existing manifests and either document the deliberate dual ceiling (`15` live mutation vs `128` import/crossover headroom) or add an opt-in manifest key (e.g., `"live.max_mutation_nodes"`, default `15`, validated `<= 128`).
- **Tái hiện:** Static audit of `src-tauri/src/evolution/mutation.rs:161-163`, `src-tauri/src/evolution/crossover.rs:161-163`, and `src-tauri/src/core/components.rs:189-192`.
- **Test khoá lại:** chưa có
- **Trạng thái:** OPEN

### F-078 — Model limitation (not a bug): closed-EU ecosystem omits primary solar energy input, and lake water refills unconditionally without hydrologic conservation
- **Ngày:** 2026-11-24
- **Loại:** SMELL
- **Vị trí:**
  - `src-tauri/src/core/telemetry_export.rs:324-330`: doc comment of `generate_energy_flux_report` documents a strictly closed EU budget with no exogenous solar source; `src-tauri/src/core/environmental_systems.rs:30-85` (`resource_field_regrowth_system`) gates primary plant regrowth strictly on `detritus` EU (`pool.detritus -= grown; pool.plants += grown;`).
  - `src-tauri/src/core/environmental_systems.rs:231-240,337-348`: `lake_replenishment_system` refills lakes unconditionally at a fixed rate up to capacity; drinking transfers water from lakes to agent `hydration` (`337-348`), while dehydration/evaporation/metabolism do not conserve or return water mass.
  - `src-tauri/src/core/aggregate_population.rs:277`: dormant `Cohort.hydration` doc comment explicitly notes water is `"Not a conserved quantity"`.
- **Phát hiện bởi:** tay (biology audit on trunk `071ef25`)
- **Triệu chứng:** **Architectural/model limitation (not a software bug):** (1) The closed-EU model has no continuous solar influx — total world EU is fixed at genesis and vegetation regrows only by recycling `detritus`; (2) lake water regenerates exogenously (`lake_replenishment_system`) rather than conserving global H2O across rainfall, lakes, and agents.
  - **Impact:** Long-run carrying capacity is governed by initial energy endowment plus detritus turnover rather than open solar productivity, and hydrological stress reflects local drinking/refill rate imbalances rather than finite watershed conservation. Analyzing outputs as an open solar food web or closed water budget without stating these assumptions causes misinterpretation.
  - **Frozen experiments / artifacts (`git grep` in `src-tauri/tests/fixtures` and `artifacts/`):** **135** files reference `detritus` (E2 `49`, E3 `22`, E3b `18`, VIAB-1/1b `33`, E-SEL1 `6`, SEL2-P `3`, core experiments `3`, snapshot `1`), **107** files reference `hydration` (`144` match `lake|hydration`) across E-SEL1, E2, E3, E3b, SEL2-P, VIAB-1, VIAB-1b, maps, and snapshots — all built directly on closed-EU and non-conserved hydration dynamics.
  - **Proposed fix (default-preserving):** Keep default closed-EU and current lake replenishment bit-identical (no bug fix should alter closed-EU invariants or water dynamics). Record both abstractions explicitly in ecosystem/contract documentation; if open-energy or closed-hydrology regimes are studied later, gate them strictly behind opt-in world policies (`live.solar_flux`, `live.closed_hydrology`).
- **Tái hiện:** Static audit of `src-tauri/src/core/telemetry_export.rs:324-330`, `src-tauri/src/core/environmental_systems.rs:231-240,337-348`, and `src-tauri/src/core/aggregate_population.rs:277`.
- **Test khoá lại:** chưa có
- **Trạng thái:** OPEN

### F-077 — `crossover_brains` has no production callers (asexual cloning only) while `AgentSex` draws from `SimRng` without affecting mating
- **Ngày:** 2026-11-24
- **Loại:** SMELL
- **Vị trí:**
  - `src-tauri/src/evolution/brain_genotype.rs:700,950-964`: `pub fn crossover_brains` exists and is exercised only by unit tests (`git grep crossover_brains src-tauri/src` shows `0` production callers).
  - `src-tauri/src/core/components.rs:15-18`: `pub enum AgentSex { Male, Female }`.
  - `src-tauri/src/core/agent_systems.rs:419-430,494-525,598-609`: `SpawnGenotypeCommand` and `SpawnOffspringCommand` draw `SimRng::gen_bool(0.5)` to attach `AgentSex`, whereas `autonomous_reproduction_system` reproduces asexually from a single parent.
  - `src-tauri/src/core/environmental_systems.rs:397`, `src-tauri/src/core/telemetry_export.rs:180-191`, `src-tauri/src/core/simulation_state.rs:130-134`: `demographic_census_system` reports sex ratios, and snapshot serialization preserves `sex: Option<AgentSex>` explicitly to maintain the founder vs. offspring ECS archetype swap order across restore (`F-045`).
- **Phát hiện bởi:** tay (biology audit on trunk `071ef25`)
- **Triệu chứng:** All live reproduction is asexual/clonal with mutation, leaving `crossover_brains` unused outside unit tests (`brain_genotype.rs:700,950-964`), while newborn offspring still consume a `SimRng::gen_bool(0.5)` draw and receive an `AgentSex` component (`agent_systems.rs:598-609`) that plays no causal role in mate choice, recombination, or traits.
  - **Impact:** Evolutionary runs lack sexual recombination, and sex-ratio telemetry (`telemetry_export.rs:180-191`) describes inert RNG labels rather than two-parent biology. Crucially, removing the `AgentSex` RNG draw or ECS component by default would alter `SimRng` trajectories and post-restore death table swap order (`F-045`).
  - **Frozen experiments / artifacts (`git grep -E crossover_brains|AgentSex|agent_sex` in `src-tauri/tests/fixtures` and `artifacts/`):** **0** literal matches, though every frozen run with births executes `SpawnOffspringCommand`'s `SimRng::gen_bool(0.5)` draw and `AgentSex` archetype split.
  - **Proposed fix (default-preserving):** Keep default asexual cloning plus the existing `AgentSex` draw and component intact so `SimRng` state and snapshot swap order (`F-045`) remain bit-identical. Document `AgentSex` as demographic-only in asexual runs, and wire `crossover_brains` + two-parent mating only behind a new opt-in key (e.g., `"live.sexual_reproduction": 1.0`).
- **Tái hiện:** Static audit (`git grep crossover_brains src-tauri/src`; `src-tauri/src/core/agent_systems.rs:419-430,598-609`).
- **Test khoá lại:** chưa có
- **Trạng thái:** OPEN

### F-076 — Brain target-vector inputs bypass finite visual/olfactory limits by computing an omniscient global nearest-food/prey vector
- **Ngày:** 2026-11-24
- **Loại:** SMELL
- **Vị trí:**
  - `src-tauri/src/core/agent_systems.rs:1743-1785,1830-1833`: inside `sensory_system`, `local_target_vec` iterates over all `Food` (`food_query`) or all living `Prey` (`prey_query`) globally with unbounded distance (`1743-1772`) and writes it into neural inputs (`1830-1833`), alongside bounded raycast `hit_distance` (`1774-1785`, max `10.0`).
- **Phát hiện bởi:** tay (biology audit on trunk `071ef25`)
- **Triệu chứng:** Every agent's brain receives an accurate local bearing/direction (`local_target_vec`) toward the closest `Food` (or living `Prey` for predators) across the whole map regardless of vision range, terrain occlusion, or sensory morphology.
  - **Impact:** Agents effectively possess omniscient global food/prey localization instead of locally bounded perception, understating search costs and weakening selective pressure on sensory exploration or ray-based navigation.
  - **Frozen experiments / artifacts (`src-tauri/tests/fixtures` and `artifacts/`):** **0** manifest keys configure target-vector range directly, but every frozen brain-driven run (**68** `evolved_brains` files across E-SEL1, E2, E3, E3b, SEL2-P plus shared-brain runs such as `VIAB-1`/`VIAB-1b` and `schema7_burn013_shared_learner.json`) runs `sensory_system` with unbounded `local_target_vec`.
  - **Proposed fix (default-preserving):** Bounding `local_target_vec` by default would alter brain inputs and break bit-identity of all existing and frozen runs. Keep unbounded `local_target_vec` when the opt-in key is absent/`0.0`, and introduce a declared opt-in key (e.g., `"live.local_target_range"` or `"live.bounded_target_vector"`) that zeroes out targets beyond finite visual/sensory range for perceptual-limitation studies.
- **Tái hiện:** Static audit of `src-tauri/src/core/agent_systems.rs:1743-1785,1830-1833`.
- **Test khoá lại:** chưa có
- **Trạng thái:** OPEN

### F-075 — Pheromone/olfaction systems are scheduled, but production spawns never attach `PheromoneReleaser` or `OlfactorySensors`, leaving brain inputs 13-14 at `0` and `action_index::SIGNAL` unread
- **Ngày:** 2026-11-24
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/ai/pheromone.rs:130-182,223-235` & `src-tauri/src/core/simulation_schedule.rs:167-169`: defines `OlfactorySensors` and `PheromoneReleaser`, and schedules `agent_release_pheromone_system`, `update_pheromone_grid_system`, and `agent_read_pheromone_system`, but `git grep` shows `0` production spawn sites attach `PheromoneReleaser` or `OlfactorySensors`.
  - `src-tauri/src/core/agent_systems.rs:1641,1824-1846,1953-1957`: `sensory_system` reads `Option<&OlfactorySensors>`, defaulting brain inputs `13-14` (`left_reading, right_reading`) to `(0.0, 0.0)`; `action_resolution_system` maps slots `4..=6` (`PHEROMONE_EMIT`, `ATTACK_INTENT`, `FEED_INTENT`) to `ActionGates` (`src-tauri/src/core/components.rs:501-509`), never reading slot `7`.
  - `src-tauri/src/evolution/brain_genotype.rs:42-43,50-54`: defines `action_index::SIGNAL = 7` inside `EVOLVED_ARCH` (`15` inputs, `8` outputs); `git grep action_index::SIGNAL src-tauri/src` returns zero readers.
- **Phát hiện bởi:** tay (biology audit on trunk `071ef25`)
- **Triệu chứng:** Although pheromone grids, emission/sensing systems, `ActionGates.pheromone_emit` (`slot 4`), and `action_index::SIGNAL` (`slot 7`) all exist in code, no production spawn attaches `PheromoneReleaser` or `OlfactorySensors`, and nothing consumes `action_index::SIGNAL`. Consequently, `pheromone_grid` remains empty, brain inputs `13-14` are identically `0.0` for all agents, and output slot `7` has zero effect on the world.
  - **Impact:** Chemical signaling and olfaction are completely inert across all runs, evolved brains carry dead input/output dimensions, and researchers inspecting `EVOLVED_ARCH` (`15 -> 8`) may wrongly assume pheromone signaling is active.
  - **Frozen experiments / artifacts (`git grep` in `src-tauri/tests/fixtures` and `artifacts/`):** **1** direct snapshot match (`src-tauri/tests/fixtures/snapshots/schema7_burn013_shared_learner.json` with empty `pheromone_grid`), plus **68** `evolved_brains` files (E-SEL1 `15`, E2 `18`, E3 `13`, E3b `14`, SEL2-P `8`) whose 15-input/8-output networks expect `(0.0, 0.0)` at inputs `13-14`.
  - **Proposed fix (default-preserving):** Attaching `OlfactorySensors` / `PheromoneReleaser` by default would feed non-zero odor signals into inputs `13-14` and diverge all frozen runs. Keep production spawning without those components by default (`0.0` inputs bit-identical), and wire `PheromoneReleaser` + `OlfactorySensors` attachment plus `action_index::SIGNAL` vs `ActionGates.pheromone_emit` semantics behind an explicit opt-in key (e.g., `"live.pheromones": 1.0`).
- **Tái hiện:** Static audit (`git grep PheromoneReleaser src-tauri/src`, `git grep OlfactorySensors src-tauri/src`, `git grep action_index::SIGNAL src-tauri/src`).
- **Test khoá lại:** chưa có
- **Trạng thái:** OPEN

### F-074 — `LINDEMAN_EFFICIENCY = 0.40` contradicts its `"~10% canonical"` doc comment and conflates strike assimilation with whole-trophic-level transfer
- **Ngày:** 2026-11-24
- **Loại:** SMELL
- **Vị trí:**
  - `src-tauri/src/core/ecology.rs:153-155,198,882`: defines `pub const LINDEMAN_EFFICIENCY: f32 = 0.40;` while the doc comment calls `"~10% canonical"`.
  - `src-tauri/src/core/resources.rs:780-797`: `PredatorIncomePolicy::kill_efficiency` defaults to `LINDEMAN_EFFICIENCY` and documents it as strike assimilation efficiency (returning `(removed - gained)` to detritus).
  - `src-tauri/src/core/live_experiment.rs:183-186,424-426,715-725`: `LIVE_KEY_KILL_EFFICIENCY` (`"live.kill_efficiency"`) defaults to `LINDEMAN_EFFICIENCY` (`0.40`).
- **Phát hiện bởi:** tay (biology audit on trunk `071ef25`)
- **Triệu chứng:** `LINDEMAN_EFFICIENCY` is named after Lindeman's ~10% trophic-level ecological efficiency and cites `"~10% canonical"` in its comment (`ecology.rs:153-155`), yet is set to `0.40`. In the engine, `0.40` actually parameterizes *predator assimilation efficiency per successful strike on captured prey biomass* (gross-growth / ecosystem Lindeman efficiency is lower once metabolism and non-predatory deaths are included).
  - **Impact:** Misleads biological interpretation and audits; naively changing `LINDEMAN_EFFICIENCY` from `0.40` to `0.10` would break predation energetics in all frozen and historical experiments.
  - **Frozen experiments / artifacts (`git grep -E LINDEMAN|kill_efficiency` in `src-tauri/tests/fixtures` and `artifacts/`):** **4** preregistration manifests explicitly pin `"live.kill_efficiency": 0.4` (`src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json:124`, `src-tauri/tests/fixtures/experiments_e3b/e3b-preregistration.json:155`, `artifacts/experiments/e3-productivity-gradient-body-mass/confirmation/manifests/e3-preregistration.json:124`, `artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation/manifests/e3b-preregistration.json:155`), and all frozen runs without the key default to `0.40`.
  - **Proposed fix (default-preserving):** Keep the numeric default `0.40` bit-identical. Clarify the doc comments in `ecology.rs:153-155`, `resources.rs:780`, and `live_experiment.rs:183-186` (and optionally alias/rename internally to `STRIKE_ASSIMILATION_EFFICIENCY` while keeping `"live.kill_efficiency"` default `0.40`) to state explicitly that `0.40` is captured-prey assimilation efficiency rather than ecosystem-level Lindeman efficiency, with `"live.kill_efficiency"` available for 10% sensitivity sweeps.
- **Tái hiện:** Static audit of `src-tauri/src/core/ecology.rs:153-155` and `src-tauri/src/core/live_experiment.rs:183-186,424-426`.
- **Test khoá lại:** chưa có
- **Trạng thái:** OPEN

### F-073 — `"Pathogen"` / `PathogenTransmission` mortality cause is never recorded in `DemographicTracker` or `mortality_system`
- **Ngày:** 2026-11-24
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/core/telemetry_export.rs:89,109,225-226`: `DemographicTracker` defines `deaths_pathogen`, handles `"Pathogen"` in `record_death`, and exports `MortalityCauseCount { cause: "Pathogen", ... }`, but `git grep '"Pathogen"' src-tauri/src` finds zero callers passing `"Pathogen"`.
  - `src-tauri/src/core/live_experiment.rs:1349-1405`: executes `InterventionKind::PathogenTransmission` effects on agent `homeo.energy` without tagging pathogen-caused mortality.
  - `src-tauri/src/core/mortality.rs:164-192`: `mortality_system` passes only `"Senescence"` (`lifespan >= 300`) and `"Starvation"` to `DemographicTracker::record_death` (`168-180`; `DemographicTracker` has no `"Dehydration"` cause so thirst deaths are filed under `"Starvation"`), while `VitalStatistics::record_death_by` (`166,187-192`) tracks `DeathCause::{Starvation, Dehydration}`.
- **Phát hiện bởi:** tay (biology audit on trunk `071ef25`)
- **Triệu chứng:** Even when `PathogenTransmission` drains agent energy, resulting deaths are logged to `DemographicTracker` as `"Starvation"` or `"Senescence"` (`mortality.rs:168-180`), and to `VitalStatistics` as `DeathCause::Starvation` (or `Dehydration`), leaving `DemographicTracker.deaths_pathogen` permanently `0`.
  - **Impact:** Demographic telemetry cannot attribute mortality to pathogen interventions, silently reporting `0` pathogen deaths whenever `PathogenTransmission` is tested.
  - **Frozen experiments / artifacts (`git grep -E Pathogen|pathogen` in `src-tauri/tests/fixtures` and `artifacts/`):** **0** files.
  - **Proposed fix (default-preserving):** Runs without `PathogenTransmission` have no pathogen events, so default runs remain bit-identical. Either attach a pathogen-attribution marker/cause in `mortality_system` when `PathogenTransmission` directly kills or drains an infected agent (without changing default starvation/dehydration thresholds or runs lacking `PathogenTransmission`), or emit a preflight/doc note if pathogen mortality attribution is deferred until a dedicated `DeathCause::Pathogen` rule is specified.
- **Tái hiện:** Static audit (`git grep '"Pathogen"' src-tauri/src`; `src-tauri/src/core/live_experiment.rs:1349-1395`; `src-tauri/src/core/mortality.rs:164-192`).
- **Test khoá lại:** chưa có
- **Trạng thái:** OPEN

### F-072 — Death-cause label `"Senescence"` is assigned solely by `lifespan >= 300` without an underlying ageing mechanism
- **Ngày:** 2026-11-24
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/core/mortality.rs:155,164-192`: `mortality_system` only kills agents via `DeathCause::{Starvation, Dehydration}` (and `VitalStatistics` records those true causes), while `164-180` maps `lifespan >= 300` to `"Senescence"` for `DemographicTracker::record_death` (as the code comment acknowledges) and defaults other deaths to `"Starvation"`.
  - `src-tauri/src/core/telemetry_export.rs:85-90,103-112,216-219`: `DemographicTracker` increments `deaths_senescence` on `"Senescence"` and exports `"Senescence"` counts.
- **Phát hiện bởi:** tay (biology audit on trunk `071ef25`)
- **Triệu chứng:** Agents do not experience physiological ageing or age-triggered death; any agent dying of starvation or dehydration after `>= 300` ticks is re-labeled `"Senescence"` in `DemographicTracker` (`mortality.rs:164-180`), while unknown strings default to `"Starvation"`. Note that experiment observables `live.deaths_starvation` and `live.deaths_dehydration` track `DeathCause` directly and remain accurate.
  - **Impact:** Demographic telemetry conflates long-lived starvation and dehydration deaths with senescence, fabricating ageing mortality where none exists in simulation mechanics.
  - **Frozen experiments / artifacts (`git grep Senescence` in `src-tauri/tests/fixtures` and `artifacts/`):** **0** files.
  - **Proposed fix (default-preserving):** Documented label fix only: preserve the actual `DeathCause` string (`"Starvation"` or `"Dehydration"`) passed to `DemographicTracker` (or rename the `lifespan >= 300` bucket label to a documented long-lived cause label such as `"Starvation (Age >= 300)"` / explicit cohort breakdown) without altering mortality timing, `VitalStatistics`, closed-EU flow, or any observable trajectory. Add true age-dependent mortality later only behind an opt-in key (e.g., `"live.senescence": 1.0`).
- **Tái hiện:** Static audit of `src-tauri/src/core/mortality.rs:155,164-192` and `src-tauri/src/core/telemetry_export.rs:85-90,103-112`.
- **Test khoá lại:** chưa có
- **Trạng thái:** OPEN

### F-071 — `InterventionKind::GeneticInjection` writes a ledger entry without modifying any agent genotype or brain
- **Ngày:** 2026-11-24
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/core/live_experiment.rs:1281-1296`: `InterventionKind::GeneticInjection { target_species, trait_modifications, count }` records ledger state without altering morphology genotypes, metabolic efficiency, sensory range, or brain parameters of any agent.
  - `src-tauri/src/core/experiment/interventions.rs:96-123` & `src-tauri/src/core/experiment/manifest.rs:608-645`: validate and digest `GeneticInjection` + `TraitMod` entries.
  - `src-tauri/src/core/world_record/ring.rs:165,209-219,271-278` & `src-tauri/src/core/world_record/chain.rs:92` serialize and replay `GeneticInjection` records; `src-tauri/src/core/scenario.rs:226` (`ReferenceEvolutionWorld::step`) applies scenario-level `GeneticInjection` bookkeeping.
- **Phát hiện bởi:** tay (biology audit on trunk `071ef25`)
- **Triệu chứng:** `GeneticInjection` manifests validate (`interventions.rs:96`), enter canonical run hashes (`manifest.rs:608`), and appear in the applied intervention ledger (`live_experiment.rs:1281-1296`), yet execute as a complete no-op on `AgentGenotype`, metabolic/sensory traits, and `AgentBrain`.
  - **Impact:** Counterfactual or experiment manifests scheduling `GeneticInjection` silently behave identically to unperturbed sham runs while claiming the genetic intervention was applied.
  - **Frozen experiments / artifacts (`git grep -E GeneticInjection|genetic_injection` in `src-tauri/tests/fixtures` and `artifacts/`):** **0** files.
  - **Proposed fix (default-preserving):** Because `0` frozen experiments or artifacts use `GeneticInjection`, runs without it stay bit-identical. Either reject `GeneticInjection` cleanly at preflight (refusing unimplemented sham interventions) until an ADR/spec defines deterministic target ordering and body/brain application of `TraitMod`, or implement deterministic target selection and trait mutation inside `live_experiment.rs:1281-1296` only when a `GeneticInjection` command fires.
- **Tái hiện:** Static audit of `src-tauri/src/core/live_experiment.rs:1281-1296` (no agent queries or genotype/brain writes inside the `GeneticInjection` arm).
- **Test khoá lại:** chưa có
- **Trạng thái:** OPEN

### F-070 — `live.ecological_fitness` has no effect in headless `live_experiment` runs because `LiveChannels.epoch_stats` is drained without epoch selection
- **Ngày:** 2026-11-24
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/core/live_experiment.rs:236-239,370,468,681-692,2689-2696`: validates `"live.ecological_fitness"` (`0.0 | 1.0`) and inserts `FitnessRuleSetting` into `World`.
  - `src-tauri/src/core/live_experiment.rs:2411-2420,3573-3577`: `drain_absent_thread_queues` (`2417-2420`) drains `self.channels.transitions` and `self.channels.epoch_stats` (`stats_rx` paired with `EvolutionSender` at `3573-3577`) in headless runs where no evolution worker thread exists; meanwhile `EvolutionQueue` (`src-tauri/src/core/resources.rs:1021`, fed via `EvolutionReceiver` in `apply_staggered_evolution_system` at `agent_systems.rs:1389`) remains empty.
  - `src-tauri/src/core/agent_systems.rs:1297-1384`: `check_epoch_completion_system` computes `FitnessRuleSetting`-weighted fitness (`1315,1344-1380`) when sending `Vec<AgentEpochStats>` via `EvolutionSender` (`1299,1380`).
- **Phát hiện bởi:** tay (biology audit on trunk `071ef25`)
- **Triệu chứng:** In headless `live_experiment` runs, `FitnessRuleSetting` only feeds `AgentEpochStats` sent over `EvolutionSender` (`check_epoch_completion_system`, `agent_systems.rs:1344-1380`) into `LiveChannels.epoch_stats`, which `drain_absent_thread_queues` (`live_experiment.rs:2417-2420`) drains and discards without running epoch replacement (so `EvolutionQueue` never receives spawns from `EvolutionReceiver`), while live demography runs via `autonomous_reproduction_system` and `mortality_system`. Setting `"live.ecological_fitness": 1.0` therefore produces the exact same trajectory as `0.0` headless.
  - **Impact:** Scientific manifests (such as E3 and E3b) declare `"live.ecological_fitness": 1.0` expecting ecological fitness ranking to operate, when headless execution ignores `FitnessRuleSetting` completely.
  - **Frozen experiments / artifacts (`git grep live\.ecological_fitness` in `src-tauri/tests/fixtures` and `artifacts/`):** **27** files — `artifacts/experiments/e3-productivity-gradient-body-mass` (`6`), `artifacts/experiments/e3b-productivity-gradient-body-mass` (`7`, including `e3b-preregistration.json:122`), `src-tauri/tests/fixtures/experiments_e3` (`7`), `src-tauri/tests/fixtures/experiments_e3b` (`7`, including `e3b-preregistration.json:122`).
  - **Proposed fix (default-preserving):** Refusing `"live.ecological_fitness"` or turning on epoch replacement for `"live.ecological_fitness": 1.0` would break all `27` frozen E3/E3b fixtures and artifacts. Keep existing headless stepping bit-identical for `"live.ecological_fitness"`, emit a preflight warning clarifying that `"live.ecological_fitness"` only affects epoch-worker replacement (inactive in headless `live_world` mode), and gate any future headless epoch-selection path behind a new opt-in key (e.g., `"live.headless_epoch_selection": 1.0`).
- **Tái hiện:** Static audit of `src-tauri/src/core/live_experiment.rs:2411-2420,2689-2696,3573-3577` and `src-tauri/src/core/agent_systems.rs:1297-1384`.
- **Test khoá lại:** chưa có
- **Trạng thái:** OPEN

### F-069 — `live.archive_coverage` always reports `0` in headless runs while bypassing `ensure_observables_supported` preflight
- **Ngày:** 2026-11-24
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/core/live_experiment.rs:1642,1694,1704,1750,1849-1852,1885,3623-3625`: `read_live_observables` (`1750`, wrapped by `live_observables` at `1694` and `live_observables_peek` at `1704`) reads `world.get_resource::<BevyMapElitesArchive>()` and emits `"live.archive_coverage"` from `a.archive.grid.len()` (`MapElitesArchive.grid` at `src-tauri/src/evolution/map_elites.rs:41-48`), defaulting to `0.0`.
  - `src-tauri/src/core/live_experiment.rs:849-890`: `LiveWorldConfig::ensure_observables_supported` validates death and birth observables against active rules, but omits `"live.archive_coverage"`.
  - `src-tauri/src/core/simulation_loop.rs:620-626,832-854,1411-1415,2391-2400,2417`: the evolution worker maintains its own local `MapElitesArchive` (`620-626`) and copies it into `BevyMapElitesGrid` (`832-854`) for viewer display (`2391-2400,2417`), while the world's `BevyMapElitesArchive` receives no insertions in headless `live_experiment` runs.
  - `src-tauri/src/core/experiment/observables.rs:40,365-380`: defines `ER10` observable metadata (`source` field) and records Branch `O-5` / `ensure_observables_supported` rule requiring refusing observables whose producing structure is absent rather than reporting fabricated zeros.
- **Phát hiện bởi:** tay (biology audit on trunk `071ef25`)
- **Triệu chứng:** Because `BevyMapElitesArchive` in the headless `World` is never populated (`simulation_loop.rs:620-626,1411-1415`), `live.archive_coverage` reports `0.0` on every headless tick/checkpoint instead of either computing real coverage or failing `ensure_observables_supported` (`observables.rs:365-380`, `ER10` in `docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md:183-195`).
  - **Impact:** Any downstream analysis reading `live.archive_coverage` from headless runs sees `0.0` (creating false zero-diversity readings unless manually excluded, as noted under `S5` in `docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md:158` and `docs/ai/planning/2026-11-24-e3b-preregistration.md:117`).
  - **Frozen experiments / artifacts (`git grep live\.archive_coverage` in `src-tauri/tests/fixtures` and `artifacts/`):** **54** files — `artifacts/experiments/e2-evolved-brain-default` (`10`), `artifacts/experiments/e3-productivity-gradient-body-mass` (`11`), `artifacts/experiments/e3b-productivity-gradient-body-mass` (`10`), `src-tauri/tests/fixtures/experiments_e3` (`7`), `src-tauri/tests/fixtures/experiments_e3b` (`7`), `src-tauri/tests/fixtures/experiments_esel1` (`6`), `src-tauri/tests/fixtures/experiments_sel2p` (`3`).
  - **Proposed fix (default-preserving):** Rejecting `"live.archive_coverage"` unconditionally in `ensure_observables_supported` would break all `54` frozen manifests/artifacts that request the extended observable set. Keep frozen manifests valid and outputs bit-identical by: (a) emitting a preflight warning when `"live.archive_coverage"` is requested in headless mode without an active archive producer (and gating hard preflight refusal behind an opt-in strict observable contract / schema version bump), and/or (b) syncing headless archive metrics into `BevyMapElitesArchive` only behind an explicit opt-in key (e.g., `"live.archive_sync": 1.0`).
- **Tái hiện:** Static audit (`src-tauri/src/core/live_experiment.rs:849-890,1849-1852,3623-3625`; `src-tauri/src/core/simulation_loop.rs:620-626`).
- **Test khoá lại:** chưa có
- **Trạng thái:** OPEN

### F-068 — `persistence_stress_tests::test_100_save_load_cycles` intermittent snapshot-restore abort (`evolution morphology-node cursor precedes saved genotypes`)
- **Ngày:** 2026-11-24
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/tests/persistence_stress_tests.rs:422-435` (`thread::sleep(Duration::from_millis(15))` at `:422` inside the `1..=100` save/load loop before `engine.save_request_tx.send(tx_cycle)` at `:427-430` and `rx_cycle.recv_timeout` within `:426-435`)
  - `src-tauri/src/core/simulation_state.rs:716,812-814` (`evolution_worker_resume_state` at `:716`; `:812-814`: `if saved.node_id_counter < node_id_counter { return Err("evolution morphology-node cursor precedes saved genotypes".into()); }`)
  - `src-tauri/src/core/simulation_loop.rs:436-454,627` (restore validation calls `evolution_worker_resume_state` at `:436-454`; worker thread `evo_handle` binds `let mut node_id_counter = evolution_worker_resume.node_id_counter;` at `:627`, captures `node_id_counter` (`:673`) and replies to checkpoint requests at `:669-680`, mutates at `:878,:923`, while save/checkpoint drains and checks quiescence at `:2032-2073`)
- **Phát hiện bởi:** `anima-mac` (`WV2-3` review, 2026-11-24; PR `#242` / `docs/ai/planning/2026-11-24-wv2-3.md:120` / hub `test_runs/anima-mac_WV2-3_report.md`)
- **Triệu chứng:**
  - **Confirmed by reading trunk `cf3fb24a`:** `evolution_worker_resume_state` rejects restore at `simulation_state.rs:812-814` with `"evolution morphology-node cursor precedes saved genotypes"` whenever `saved.node_id_counter < node_id_counter`, checked on restore at `simulation_loop.rs:436-454`; the `evo_handle` thread holds a local `mut node_id_counter` (`simulation_loop.rs:627`, incremented at `:878,:923`) synced across channels during checkpoint save (`:669-673`, `:2032-2073`), while `test_100_save_load_cycles` sleeps `15 ms` between `start` and `save_request_tx` (`persistence_stress_tests.rs:422-435`).
  - **Reported only by `anima-mac` (not re-run with Cargo on `anima-dell`):** intermittently aborted snapshot restore on earlier full-suite and isolated runs, while passing `4 passed; 0 failed` in `7.33s` in both Round 1 (`task-4902`) and Round 2 (`task-5034`) `-j 2` runs; deterministic trigger not yet isolated.
- **Tái hiện:** `cd src-tauri && cargo test --features desktop --test persistence_stress_tests test_100_save_load_cycles` (intermittent per `anima-mac` report; not re-run on `anima-dell`)
- **Test khoá lại:** `src-tauri/tests/f068_morphology_cursor_restore_tests.rs` (`f068_live_add_node_with_evo_paused_survives_save_load_cycles` + `f068_corrupt_morphology_cursor_is_still_rejected`), plus 50 repeated runs of `persistence_stress_tests::test_100_save_load_cycles`
- **Trạng thái:** FIXED (`C15 HOTLOOP-PERSIST`, branch `win/hotloop-persist`, 2026-11-24). Synced `NextNodeId` / saved genotype node maxima into `SavedEvolutionWorkerState.node_id_counter` on checkpoint save and across `evo_handle` batches while keeping `evolution_worker_resume_state` strict.

### F-067 — `demographic_census_system` -> `build_demographic_census` allocates on the heap every tick inside `build_tick_schedule`
- **Ngày:** 2026-11-24
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/core/simulation_schedule.rs:192` (`demographic_census_system,` inside the `.chain()` of `build_tick_schedule()` with no `.run_if(...)` cadence guard — runs every tick)
  - `src-tauri/src/core/environmental_systems.rs:391-422` (`let mut agent_data = Vec::new();` at `:405`, `.push(...)` at `:413`, calling `build_demographic_census(&agent_data, tracker.as_deref())` at `:418` unconditionally before checking `if let Some(ref mut res) = census_resource` at `:419`)
  - `src-tauri/src/core/telemetry_export.rs:128-259` (allocates `cohorts = vec![...]` at `:136-157`, `mortality_causes = vec![...]` with 5 `.to_string()` calls at `:207-228`, and `fecundity_by_age = vec![...]` with 3 `.to_string()` calls at `:230-243` per invocation)
- **Phát hiện bởi:** `anima-mac` (`WV2-3` review, 2026-11-24; PR `#242` / `docs/ai/planning/2026-11-24-wv2-3.md:119` / hub `test_runs/anima-mac_WV2-3_report.md`)
- **Triệu chứng:**
  - **Confirmed by reading trunk `cf3fb24a`:** `demographic_census_system` runs every tick (no cadence guard) in `simulation_schedule.rs:192` (`build_tick_schedule` `.chain()` `:131-200`, `55` tuple items at `:133-196` = `49` systems + `6` `apply_deferred`) and allocates heap memory every tick via `agent_data = Vec::new()` (`environmental_systems.rs:405,413`) plus `3` `Vec`s and `8` `String`s inside `build_demographic_census` (`telemetry_export.rs:136-243`) even when `DemographicCensusState` is absent (`environmental_systems.rs:419`), conflicting with the `CLAUDE.md` zero-heap-allocation hot-loop rule.
  - **Reported only by `anima-mac` (not re-run with Cargo on `anima-dell`):** running what `docs/ai/planning/2026-11-24-wv2-3.md:119` describes as the full 52-system `build_tick_schedule` performed `7338` allocations across `50` ticks, leading `world_v2_sim_gate_tests.rs:998` (`GS-4`) to isolate a 7-system World v2 hot schedule.
- **Tái hiện:** run `ThreadTrackingAllocator` over full 52-system `build_tick_schedule` for `50` ticks as reported in `docs/ai/planning/2026-11-24-wv2-3.md:119` (not re-run on `anima-dell`)
- **Test khoá lại:** `src-tauri/tests/f067_census_zero_alloc_tests.rs` (zero heap allocations via `ThreadTrackingAllocator`, resource-present/absent parity vs `build_demographic_census`, and bit-identical trajectory check)
- **Trạng thái:** FIXED (`C15 HOTLOOP-PERSIST`, branch `win/hotloop-persist`, 2026-11-24). `demographic_census_system` now skips work when `DemographicCensusState` is absent and streams living agents directly into `update_demographic_census_from_iter`, updating fixed cohort/mortality/fecundity buffers in place.

### F-066 — Ca sinh trong thế giới live không bao giờ tới lineage tracker, và anh em cùng cha dùng chung một lineage id
- **Ngày:** 2026-11-22
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/core/simulation_loop.rs:956`: lời gọi `add_reproduction` duy nhất là epoch MAP-Elites trên luồng tiến hoá. Sinh sản liên tục (ADR-0005) đi qua `autonomous_reproduction_system` → `SpawnOffspringCommand::apply` (`src-tauri/src/core/agent_systems.rs:478` ở trunk `7111093`), mang id cha nhưng không ghi cạnh nào.
  - `src-tauri/src/core/agent_systems.rs:1129` (trunk `7111093`): id con là `format!("{parent_lineage_id}.{}", parent_generation + 1)`, nên mọi con của cùng một cha có **cùng** id. Đây là gốc của F-047; F-047 được sửa bằng cách ghép phản hồi suy luận theo `(lineage_id, request_id)`, không phải bằng id duy nhất.
  - `src-tauri/src/evolution/lineage.rs`: `LineageNode` không có thời điểm chết, nên không xuất được `destruction_time` theo chuẩn dữ liệu ALife.
- **Phát hiện bởi:** tay (đọc code sau các báo cáo nghiên cứu ngày 2026-11-22; ghi ở `docs/research/OPEN_ENDEDNESS_LINEAGE_SPECIES_RESEARCH_2026-11-22.md` §1, PR #206)
- **Triệu chứng:** Chạy thế giới live có sinh sản thì đồ thị phả hệ rỗng (adapter) hoặc chỉ chứa con MAP-Elites (app), và các cá thể sống có id trùng nhau. Mọi phép đo dòng dõi trên quần thể live (OSS-073 line of descent kiểu Avida, cổng Red-Queen S43, ứng viên loài AE-S11) vì thế không làm được: không có đồ thị, và id cũng không gọi tên được node của nó.
- **Vì sao id duy nhất không nằm ở `AgentLineageId`:** id đó không chỉ là nhãn. `autonomous_reproduction_system` duyệt cha **theo thứ tự `AgentLineageId`**, và thứ tự ứng viên của drift replay cũng khoá theo nó, nên đổi giá trị đó là đổi thứ tự rút `SimRng`, đổi quỹ đạo và checksum. Bản đầu của sửa này đã ghi đè nó, và bật khoá làm lệch quỹ đạo (54 sinh / 26 chết với khoá, 53 / 25 không khoá ở tick 2.500) — trái ADR-0008 động lực 4 ("không phản hồi": ghi quá khứ không được đổi tương lai). Bản sửa giữ `AgentLineageId`/`AgentParentLineageIds` ở giá trị cũ `cha.thế_hệ` trong mọi lượt chạy và đặt id duy nhất trong bảng phụ của `LiveLineage` theo `Entity` (không phải component: chèn component lên founder sau genesis sẽ đổi archetype và thứ tự hàng). **Ghi phả hệ không đổi quỹ đạo**: cùng seed và manifest, có khoá và không khoá cho cùng `world_checksum` và cùng số sinh/chết ở mọi mốc. Khoá `live.lineage_tracking = 1` vẫn opt-in vì tốn bộ nhớ; vắng khoá thì không có resource `LiveLineage`, hai command chỉ kiểm tra sự có mặt của nó (không cấp phát, không rút số), và `LIVE_MODEL_VERSION` không đổi.
- **Sửa:** module mới `src-tauri/src/core/live_lineage.rs` (resource `LiveLineage`). Khi bật khoá: founder là root có `origin_tick = 0`; mỗi ca sinh lấy id `birth-{run:016x}-{n:08x}` (namespace riêng, không đụng con trỏ `lineage-…` của MAP-Elites), thành một node có cạnh từ id riêng của cha (tìm qua `SpawnOffspringCommand::parent_entity`), `origin_tick` và kiểu cạnh `Clone`/`Mutate` theo việc genome thân (`mutate_genotype_reporting`, so từng bit với bản trước) hoặc não (so từng bit trọng số) có thật sự đổi hay không; mỗi ca chết qua `ReclaimAndDespawnAgentCommand` ghi `destruction_tick` và rời bảng phụ. Mọi việc này nằm trong deferred command và trong `LiveLineage::set_tick` (giữa hai tick, nơi chạy nén), không trong system của vòng tick. Ngưỡng nén có sàn tiêm được (`set_compaction_floor`, mặc định `FIRST_COMPACTION_AT`), lưu cùng save. `LineageNode` có thêm `origin_tick`/`destruction_tick` (`Option<u64>`, `#[serde(default)]`); `compact` coi mọi node "mở" (đã sinh, chưa chết) là sample. Snapshot mang đồ thị, `SavedSimulationState::live_lineage` (con trỏ id, ngưỡng nén, sàn nén, số fault) và `SerializedAgent::live_id` của từng cá thể sống, được gắn lại đúng như vậy khi khôi phục (D01, không mint lại); các trường mới được bỏ khỏi bytes khi `None`, nên save của lượt chạy không khoá không đổi. Save cũ không có các trường này vẫn đọc được: con trỏ được khôi phục từ các id có trong save, founder giữ node của nó, còn cá thể con (id cũ không gọi tên node nào) vào đồ thị như một root mới.
- **Tái hiện:** `cd src-tauri && cargo test --features desktop --test live_lineage_tests`. Trên trunk `7111093` file này không biên dịch được (khoá và API chưa có); test `the_legacy_path_records_nothing_and_keeps_parent_generation_ids` cho thấy đường mặc định vẫn tái hiện va chạm id anh em.
- **Test khoá lại:** `src-tauri/tests/live_lineage_tests.rs` (11 test: cạnh từ đúng cha, id anh em khác nhau, thời điểm chết, `run N == run K → snapshot → restore → run N−K` cả checksum, đồ thị lẫn id từng cá thể, save thiếu trường mới với fault = 0, đường legacy, **có khoá == không khoá** về checksum và số sinh/chết ở tick 0/180/1.800/2.500, nén ở cả hai phía điểm khôi phục qua đĩa mà node của mọi cá thể sống vẫn còn, `live.mutation_rate = 0` cho toàn cạnh `Clone` với genotype con bằng cha, nhãn cạnh khớp việc genome đổi) và 6 unit test trong `core/live_lineage.rs`.
- **Chưa phủ:** app desktop chưa đặt khoá (luồng `simulation_loop` vẫn chỉ thấy con MAP-Elites; Neo4j chưa có cột cho hai tick); chế độ viewer W2-lite bỏ qua khoá; di cư và ngủ đông không phải là chết nên node vẫn mở (ngủ đông mang id trong bản ghi dormant; tầng quần thể gộp `DehydrateAgentCommand` và di cư giữa shard không mang id, và adapter không bật hai đường đó); nén nối đường đơn nhánh nên genotype của tổ tiên trung gian bị bỏ (như OSS-071b), một chuỗi genotype kiểu Avida theo từng bước cần thêm tuỳ chọn tắt nén.
- **Trạng thái:** FIXED (opt-in) ở PR sửa F-066.

### F-065 — `pearson_correlation` và `DimensionSpec::Logarithmic::grid_values` dùng ngưỡng tuyệt đối `1e-15`, làm mất tương quan và làm sập lưới quét với số dương cỡ nhỏ
- **Ngày:** 2026-11-21
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/core/sweeps.rs:766-768`: `if var_x <= 1e-15 || var_y <= 1e-15 { return None; }` kiểm tra phương sai chưa chia chuẩn theo độ lớn nên từ chối chuỗi tuyến tính hoàn hảo có biên độ nhỏ (vd. `1e-9`).
  - `src-tauri/src/core/sweeps.rs:196` (cùng mẫu với `Continuous` ở `:185`): `if default_points_per_dim <= 1 || (min - max).abs() < 1e-15` gộp nhầm toàn bộ miền logarit hợp lệ (`[1e-18, 1e-16]`, rộng `100×`) thành đúng **1 điểm duy nhất** dù `DimensionSpec::validate` ở `:110` đã chấp nhận tới `f64::MIN_POSITIVE`.
- **Phát hiện bởi:** tay (kiểm toán mô-đun `sweeps.rs`)
- **Triệu chứng:** Quét tham số trên các hằng số sinh/hấp thụ rất nhỏ (`1e-9`–`1e-18`) bị im lặng co về 1 điểm lưới (bỏ qua 4/5 giá trị ở `points_per_dim = 5`) hoặc trả `None` cho hệ số tương quan `r = 1.0`.
- **Tái hiện:** `just test` hoặc `just verify` (chạy `scientific_pipeline_hardening_tests::f065_sweeps_pearson_and_grid_values_are_scale_invariant`)
- **Test khoá lại:** `scientific_pipeline_hardening_tests::f065_sweeps_pearson_and_grid_values_are_scale_invariant` và `sweeps::tests::{pearson_correlation_is_scale_invariant_and_rejects_constants_and_non_finite, grid_values_handles_extreme_ratios_and_degenerate_ranges, map_normalized_and_sampling_handle_extreme_log_and_continuous_spans}` (khẳng định `Some(r)` ở biên độ `1e-9..1e-250` và `[-1e308, 0, 1e308]`, trả `None` cho `[0.1; 3]`, giữ 5 điểm lưới + `map_normalized`/Sobol/LHS hữu hạn trong `[min, max]` ở `[1e-18, 1e-16]`, `[1e-200, 1e200]` và `[-1e308, 1e308]`). Lưu ý hành vi phi mặc định: các điểm lưới bên trong (interior grid points) có thể thay đổi ở bit cuối do công thức chia tỉ lệ mới (`(max / min).powf(t)` / chia Span).
- **Trạng thái:** FIXED (nhánh `mac/r5-findings-fixes`, R5 Part 1)

### F-064 — Bảng đa thức `SOBOL_POLY_TABLE` chứa đa thức không nguyên thuỷ / khả quy và số định hướng `m_i >= 2^(i+1)`, làm sập phân tầng 2D của chuỗi Sobol tới 75% ô trống
- **Ngày:** 2026-11-21
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/core/sweeps.rs:645-678` (`SOBOL_POLY_TABLE`, `generate_sobol_direction_numbers`).
- **Phát hiện bởi:** tay (kiểm toán toán học bảng Joe & Kuo trên 31 chiều của `SOBOL_POLY_TABLE`)
- **Triệu chứng:** Qua kiểm tra đại số trên $\mathbb{F}_2$ và phép đo phân tầng, bảng `SOBOL_POLY_TABLE` cũ có **7/31** đa thức không nguyên thuỷ hoặc khả quy và **12** giá trị khởi tạo `init_m` vi phạm chặn trên $m_i < 2^{i+1}$, khiến ~50 cặp chiều sụp xuống `~64/256` ô (`~75%` trống) ở `N = 256` (và bảng thay thế tự tạo ban đầu vẫn gây tương quan `|r(24,25)| ≈ 0.74` do không dùng nguyên bản `init_m` của Joe & Kuo). Đã thay bằng bảng nguyên bản Joe & Kuo (`new-joe-kuo-6.21201`, SIAM J. Sci. Comput. 30(5), 2635-2654 (2008)) kèm trích dẫn.
- **Tái hiện:** `just test` hoặc `just verify` (chạy `scientific_pipeline_hardening_tests::f064_sobol_matches_joe_kuo_reference_points_and_decorrelates_high_dims`)
- **Test khoá lại:** `scientific_pipeline_hardening_tests::f064_sobol_matches_joe_kuo_reference_points_and_decorrelates_high_dims` và `sweeps::tests::sobol_poly_table_entries_are_primitive_and_within_bounds` (khẳng định 31 đa thức nguyên thuỷ trên $\mathbb{F}_2$, $m_i$ lẻ $< 2^{i+1}$, khớp từng bit mẫu chuẩn `scipy.stats.qmc.Sobol(d=32, scramble=False)` ở các điểm `1, 2, 3, 8, 16, 64, 256`, `|r(23,24)| < 0.25` và `|r(24,25)| < 0.25` ở `N = 256`, và phân tầng 1D chính xác trên 16 khoảng ở `N = 255`)
- **Trạng thái:** FIXED (nhánh `mac/r5-findings-fixes`, R5 Part 1)

### F-063 — `newick::to_newick` âm thầm vứt bỏ self-loop và chu trình nếu thực thể con đã có một `parent_id` khác nhỏ hơn theo thứ tự từ điển
- **Ngày:** 2026-11-21
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/evolution/newick.rs:206-229`: vòng lặp chọn đúng 1 cha có `id` nhỏ nhất và đẩy các cạnh còn lại vào `dropped_parent_edges` trước khi kiểm tra DAG.
  - `src-tauri/src/evolution/newick.rs:255-272`: kiểm tra chu trình chỉ đi ngược qua mảng `parent_of` (tối đa 1 cha mỗi nút), không thấy các cạnh bị loại bỏ.
- **Phát hiện bởi:** tay (kiểm toán `newick.rs` và giả định crossover DAG)
- **Triệu chứng:** Với đồ thị có cạnh hợp lệ `"A" -> "B"` kèm self-loop `"B" -> "B"`, hoặc chu trình 2 bước + đảo thế hệ `"A" -> "B" -> "C" -> "B"`, do `"A" < "B"` và `"A" < "C"`, `to_newick` giữ cha `"A"` trên `"B"` và vứt cạnh tạo chu trình vào `dropped_parent_edges = 1`, rồi trả về `Ok(NewickExport)` hợp lệ thay vì `Err(NewickError::Cycle { .. })`. Cạnh trùng lặp `"A" -> "B"` hai lần cũng bị đếm sai thành 1 crossover bị loại.
- **Tái hiện:** `just test` hoặc `just verify` (chạy `scientific_pipeline_hardening_tests::f063_newick_detects_cycles_and_self_loops_across_all_edges_and_dedups_duplicates`)
- **Test khoá lại:** `scientific_pipeline_hardening_tests::f063_newick_detects_cycles_and_self_loops_across_all_edges_and_dedups_duplicates` (khẳng định `Err(NewickError::Cycle { .. })` trên nút thực sự thuộc vòng lặp khi bóc nút lá lặp $O(V+E)$, và `dropped_parent_edges == 0` với cạnh trùng lặp)
- **Trạng thái:** FIXED (nhánh `mac/r5-findings-fixes`, R5 Part 1)

### F-062 — `run_branch_tree` làm rơi toàn bộ can thiệp (`interventions`) của nút cha/tổ tiên khi chạy nhánh rẽ con cháu
- **Ngày:** 2026-11-21
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/core/branch_tree.rs:731-735`: `exotic_interventions` được ghép từ `base_manifest` và nhánh con (vẫn thiếu tổ tiên trung gian nếu `nodes >= 3`).
  - `src-tauri/src/core/branch_tree.rs:778-780`: `let mut combined_interventions = manifest.base_manifest.interventions.clone(); combined_interventions.extend(spec.interventions.clone());` — **chỉ lấy** từ `base_manifest` và riêng nhánh hiện tại, hoàn toàn bỏ qua `interventions` của nút cha và các tổ tiên trung gian.
- **Phát hiện bởi:** tay (kiểm toán thực thi rẽ nhánh phản thực tế `branch_tree.rs`)
- **Triệu chứng:** Khi nhánh gốc (Node 0) hoặc nhánh cha (Node 1) mang một can thiệp (ví dụ `RainfallDelta` kéo dài từ tick `20..200`), và nhánh con/cháu (Node 2) rẽ từ tick `50` với `interventions = []` để quan sát tiếp quỹ đạo dưới cùng điều kiện rồi rẽ nhánh đối chứng, từ tick `51` trở đi `InterventionQueue` của nhánh con không còn can thiệp của cha, khiến nhánh con không thêm can thiệp vẫn lệch checksum và lệch pha sinh thái (`plants` hồi phục giả) so với chính cha nó.
- **Tái hiện:** `just test` hoặc `just verify` (chạy `scientific_pipeline_hardening_tests::f062_branch_tree_inherits_ancestor_interventions_and_exotic_forcings`)
- **Test khoá lại:** `scientific_pipeline_hardening_tests::{f062_branch_tree_inherits_ancestor_interventions_and_exotic_forcings, f062_shorter_child_with_exotic_interventions_filters_future_ancestor_forcings_and_rejects_invalid_own, f062_three_level_shorter_child_to_longer_empty_grandchild_restores_ancestor_forcings}` (khẳng định `final_checksum` khớp, `final_distance == 0.0`, kế thừa đủ chuỗi `exotic_interventions` 3 tầng gốc→cha→cháu kể cả khi nhánh cháu có `exotic_interventions = []` sau nhánh cha ngắn hơn, và nhánh con ngắn hơn lọc bỏ forcing tổ tiên ngoài tầm `duration_ticks` trong khi vẫn từ chối forcing riêng ngoài tầm). Lưu ý hành vi phi mặc định: nút con nay kế thừa thêm `interventions` từ các tổ tiên trung gian và `exotic_interventions` từ nút gốc, đồng thời ID trùng lặp giữa tổ tiên và con vẫn chưa được kiểm tra trùng.
- **Trạng thái:** FIXED (nhánh `mac/r5-findings-fixes`, R5 Part 1)

### F-061 — `start_from_manifest_tests.rs:757` dùng `include_str!("../../src/...")` thoát khỏi `src-tauri/`, làm gãy toàn bộ lệnh săn mutant (`just hunt`, `just hunt-file`, `just verify <mutant>`)
- **Ngày:** 2026-11-21
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/tests/start_from_manifest_tests.rs:757`: `let tsx = include_str!("../../src/components/ExperimentWorldControl.tsx");`
  - `justfile:90-98` (`hunt`, `hunt-file`) và `scripts/verify.sh:103` (`verify <mutant>`): chạy `cargo mutants` trong `src-tauri/` (`--copy-target true`).
- **Phát hiện bởi:** mutant (`just hunt-file src/core/terrain_navigation.rs 2 ...`)
- **Triệu chứng:** Khi `cargo mutants` sao chép thư mục crate `src-tauri/` sang sandbox tạm (`/tmp/cargo-mutants-src-tauri-...`), đường dẫn tương đối `../../src/components/ExperimentWorldControl.tsx` không tồn tại bên trong bản sao. Lệnh baseline `cargo test --no-run` thất bại ngay với `os error 2` ở `start_from_manifest_tests`, khiến cả `just hunt`, `just hunt-file` lẫn cổng `just verify <mutant>` đều gãy trên toàn crate `anima-engine`. (`just verify` không đối số không gọi `cargo-mutants` nên vẫn chạy.)
- **Tái hiện:** `just hunt-file src/core/terrain_navigation.rs 2 land_mask_tests` → `ERROR cargo build failed in an unmutated tree, so no mutants were tested` (ghi chi tiết trong `mutants.out/log/baseline.log`).
- **Test khoá lại:** chưa có (cần đổi cách test frontend↔backend kiểm tra fixture từ đường nội bộ crate hoặc qua script npm thay vì `include_str!("../../...")`).
- **Trạng thái:** FIXED (27a1bd7b, #198, test start_from_manifest_tests::bundled_frontend_sel2p_manifest_passes_viewer_preflight_and_matches_fixture)
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** PR `#198` đã merge (`27a1bd7b`, 2026-11-22T13:55:38Z); `src-tauri/tests/start_from_manifest_tests.rs:757` đọc fixture nội bộ crate (`include_str!("fixtures/experiments_sel2p/sel2p-selected.json")`) và `:777-799` kiểm `CARGO_MANIFEST_DIR` lúc runtime; `git grep 'include_str!("\.\./\.\./'` dưới `src-tauri/tests/` trả `0` kết quả. Dòng gốc: OPEN (đã có bản vá ở Draft PR #198, nhánh `mac/fix-f061`)

### F-060 — `DynamicFields::step_soil` trừ `absorbed` bằng `f32` trước khi ép sang `f64`, phá vỡ bảo toàn dinh dưỡng hơn `1.000 NM` trong một tick với `AddNutrient` cường độ lớn
- **Ngày:** 2026-11-21
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/core/dynamic_fields.rs:439-445`: `self.cum_nutrient_leached += (add - absorbed) as f64;`, trong khi `add` và `absorbed` đều là `f32`.
- **Phát hiện bởi:** tay (kiểm toán vòng bảo toàn dinh dưỡng `DynamicFields`)
- **Triệu chứng:** Với một can thiệp `AddNutrient` có `add >= ~1e8_f32` (đủ lớn để lượng hấp thụ `room <= 5.0` nhỏ hơn ULP của `add`), phép trừ `(add - absorbed)` trong `f32` làm tròn triệt tiêu hoàn toàn `absorbed`, ghi `cum_nutrient_leached == cum_nutrient_added`. Hệ quả là `soil_nitrogen` tăng thật tới `NITROGEN_CAP` (`+4,0 NM/ô`), nhưng sổ kết toán ghi nhận `100%` bị rửa trôi, làm `nutrient_budget_residual()` vọt thêm **`> 1.000 NM`** trên lưới `16×16` chỉ sau đúng 1 tick.
- **Tái hiện:** `just test` hoặc `just verify` (chạy `domain_and_fields_hardening_tests::f060_dynamic_fields_step_soil_preserves_nutrient_conservation_with_large_input`)
- **Test khoá lại:** `domain_and_fields_hardening_tests::f060_dynamic_fields_step_soil_preserves_nutrient_conservation_with_large_input` (khẳng định `cum_nutrient_leached < cum_nutrient_added` và `nutrient_budget_residual().abs() < 1e-2` khi giữ zero-alloc)
- **Trạng thái:** FIXED (nhánh `mac/r5-findings-fixes`, R5 Part 1)

### F-059 — `Region::Radius` bình phương `r * r` mà không kiểm tra `r >= 0.0`, khiến bán kính âm khớp mọi điểm như bán kính dương
- **Ngày:** 2026-11-21
- **Loại:** BUG
- **Vị trí:** `src-tauri/crates/anima-domain/src/intervention.rs:48-52`: `Region::Radius { cx, cy, r } => dx * dx + dy * dy <= r * r` không chặn `r <= 0.0` (hoặc `< 0.0`).
- **Phát hiện bởi:** tay (kiểm toán ranh giới `anima-domain::intervention`)
- **Triệu chứng:** Trong khi đường `manifest` đã chặn `r <= 0.0` ở `src-tauri/src/core/experiment/interventions.rs:59`, struct công khai `Region::Radius` ở `anima-domain` và các caller trực tiếp (`DynamicFields::step_climate`, `Scenario`) chấp nhận `r = -4.0` và áp can thiệp lên toàn bộ đĩa tròn bán kính `4.0` y như `+4.0`.
- **Tái hiện:** `just test` hoặc `just verify` (chạy `domain_and_fields_hardening_tests::f059_region_radius_rejects_non_positive_and_non_finite_radii`)
- **Test khoá lại:** `domain_and_fields_hardening_tests::{f059_region_radius_rejects_non_positive_and_non_finite_radii, f059_toxicity_plume_manifest_rejects_non_positive_radius}` (khẳng định `contains` trả `false` với `r <= 0.0` hoặc toạ độ/bán kính phi hữu hạn, và `validate_intervention` từ chối `ToxicityPlume` có `radius <= 0.0` ở `interventions.rs`). Lưu ý hành vi phi mặc định: `Region::Radius` có `r == 0.0` nay không khớp ô nào (trước kia khớp tâm `(cx, cy)`).
- **Trạng thái:** FIXED (nhánh `mac/r5-findings-fixes`, R5 Part 1)

### F-058 — `InterventionCommand::active_at` cộng tràn `u64` ở chế độ `--release` khi `duration_ticks = u64::MAX` (`start_tick > 0`), làm lệnh vô hạn tắt ngấm
- **Ngày:** 2026-11-21
- **Loại:** BUG
- **Vị trí:** `src-tauri/crates/anima-domain/src/intervention.rs:167-169` (`InterventionCommand::active_at`) và nhánh anh em `src-tauri/src/core/exotic_energy/forcings.rs:60` (`ExoticIntervention::active_at`): `tick >= self.start_tick && tick < self.start_tick + self.effective_duration()`.
- **Phát hiện bởi:** tay (kiểm toán ranh giới `anima-domain::intervention`)
- **Triệu chứng:** Đường `manifest` có `checked_add` ở `src-tauri/src/core/experiment/interventions.rs:72`, nhưng `InterventionCommand` và `ExoticIntervention` công khai khi chạy `--release` sẽ cộng tràn `100 + u64::MAX == 99`, khiến `active_at(tick)` trả `false` ở mọi `tick >= 100`.
- **Tái hiện:** `just test` hoặc `just verify` (chạy `domain_and_fields_hardening_tests::f058_intervention_active_at_avoids_u64_overflow_for_open_ended_commands` và `f058_exotic_intervention_active_at_avoids_u64_overflow`)
- **Test khoá lại:** `domain_and_fields_hardening_tests::{f058_intervention_active_at_avoids_u64_overflow_for_open_ended_commands, f058_exotic_intervention_active_at_avoids_u64_overflow}` (khẳng định `active_at(100)`, `active_at(200)`, `active_at(u64::MAX)` trả `true` và biên nửa mở không tràn cho cả `InterventionCommand` lẫn `ExoticIntervention`)
- **Trạng thái:** FIXED (nhánh `mac/r5-findings-fixes`, R5 Part 1)

### F-057 — `start_from_manifest` preflight chấp nhận `founder_layout = 1 | 2` + `land_mask = 1`, nhưng worker `sim` gọi hàm panic `apply_live_genesis_and_policies`
- **Ngày:** 2026-11-21
- **Loại:** SMELL
- **Vị trí:**
  - `src-tauri/src/commands/simulation.rs:910-952` (`load_viewer_terrain_map`, `parse_and_validate_manifest_for_viewer`, `start_from_manifest_with_engine`): trước đây chưa kiểm tra đất và sau đó dựng cả `init_world()`; nay chỉ nạp `TerrainMap` qua `load_viewer_terrain_map()`.
  - `src-tauri/src/core/simulation_loop.rs:1539`: luồng nền `sim` chuyển sang xử lý lỗi sạch từ `try_apply_live_genesis_and_policies`.
  - `src-tauri/src/core/live_experiment.rs:2589-2597`: `apply_live_genesis_and_policies` gọi `.unwrap_or_else(|e| panic!("{e}"))`.
- **Phát hiện bởi:** tay (kiểm toán W2-lite sau PR #194)
- **Triệu chứng:** Trên địa hình tạo mặc định (`256×256`), số ô đất đi được vượt trần `4096` founder nên chưa kích hoạt. Tuy nhiên khi app nạp file thế giới `.anmw` ít đất (`walkable_cells < founders`) — đường nạp mà `README.md` xác nhận vẫn tồn tại — lệnh `parse_and_validate_manifest_for_viewer` chấp nhận manifest và `start_from_manifest` trả `Ok`, sau đó luồng nền `sim` tự dừng bằng `panic!` ở bước genesis thay vì từ chối sạch từ preflight hoặc báo lỗi có cấu trúc.
- **Tái hiện:** `just test` (chạy `post_194_hardening_audit_tests::w2_lite_preflight_vs_try_apply_live_genesis_and_strict_cap_above_ceiling` và `post_194_hardening_audit_tests::f057_scarce_land_artifact_refused_at_preflight_and_non_panicking_in_engine`)
- **Test khoá lại:** `post_194_hardening_audit_tests::{w2_lite_preflight_vs_try_apply_live_genesis_and_strict_cap_above_ceiling, f057_scarce_land_artifact_refused_at_preflight_and_non_panicking_in_engine}` (khẳng định preflight chỉ nạp `TerrainMap` và kiểm tra `validate_founder_land_capacity` mà không dừng engine đang chạy, còn `simulation_loop.rs` dùng `try_apply_live_genesis_and_policies` thoát sạch với `running == false`). Lưu ý: việc xoá `apply_live_genesis_and_policies` khỏi `live_experiment.rs` đang **BLOCKED** vì `src-tauri/tests/start_from_manifest_tests.rs:34,376` và `src-tauri/tests/founder_layout_tests.rs:29,439,458,560,589` nằm ngoài danh sách file được phép sửa của PR #200.
- **Trạng thái:** FIXED (nhánh `mac/r5-findings-fixes`, R5 Part 2; giữ lại `apply_live_genesis_and_policies` do blocker phạm vi file)

### F-056 — `terrain_navigation_system` làm hỏng mọi đốt thân con thành `NaN` dù vừa phục hồi được vị trí `root` về `LastWalkablePosition`
- **Ngày:** 2026-11-21
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/core/terrain_navigation.rs:231-238`: `let delta = target - pos.0;` tính hiệu dịch chuyển từ `pos.0` đang hỏng trước khi ghi đè `pos.0 = target` và `last.revert_delta = delta`.
  - `src-tauri/src/core/terrain_navigation.rs:246-253`: `seg_pos.0 += last.revert_delta;` cộng `delta` vào toàn bộ đốt con (`Without<Agent>`).
- **Phát hiện bởi:** tay (kiểm toán `LAND1` sau PR #166)
- **Triệu chứng:** Khi `pos.0` của `root` trở thành phi hữu hạn (`NaN` hoặc `±inf`) trong khi `LastWalkablePosition` vẫn giữ toạ độ đất hợp lệ, `terrain_navigation_system` phục hồi đúng `pos.0` của `root` về `LastWalkablePosition`, nhưng giữ `revert_delta = NaN` và cộng thẳng vào mọi đốt thân con, biến toàn bộ chuỗi đốt con thành `NaN` dù `root` đã lành (tick vật lý kế tiếp kéo ngược `root` hỏng theo).
- **Tái hiện:** `just verify` hoặc `just test` (chạy `post_194_hardening_audit_tests::terrain_navigation_nan_root_poisons_child_segments_even_after_root_recovery`)
- **Test khoá lại:** `post_194_hardening_audit_tests::terrain_navigation_nan_root_poisons_child_segments_even_after_root_recovery` (khẳng định `is_pos_walkable` từ chối toạ độ phi hữu hạn kể cả ở `y`, mọi trạng thái `NaN`/`±inf` phục hồi `root` sạch với `revert_delta == Vec3::ZERO` và đốt con giữ nguyên toạ độ hữu hạn hoặc phục hồi về `LastWalkablePosition`, không làm lệch đường bờ hữu hạn ở `land_mask_tests`)
- **Trạng thái:** FIXED (nhánh `mac/r5-findings-fixes`, R5 Part 2)

### F-055 — `grass_steering` dưới `LAND1` dẫn con mồi lao ra thức ăn trên biển (`DeepOcean`), kích hoạt bẫy giật lùi bờ biển dù dưới chân có cỏ giàu năng lượng
- **Ngày:** 2026-11-21
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/core/locomotion.rs:116-120` (`select_prey_grass_target`, Ưu tiên 2): chọn `Food` cục bộ trong `local_food_radius_sq` mà không kiểm tra `grass.is_walkable(food_pos)`, trong khi cùng hàm ở `:147` lọc mọi ô cỏ qua `grass.is_walkable(center)`.
  - `src-tauri/src/core/locomotion.rs:165` (fallback Ưu tiên 3): trả về `nearest_food` toàn cục cũng không kiểm tra `grass.is_walkable(food_pos)`.
- **Phát hiện bởi:** tay (kiểm toán tương tác `live.grass_steering` × `live.land_mask` sau PR #182)
- **Triệu chứng:** Dù `spawn_food_system` đã né nước khi bật `LAND1`, bất kỳ thực thể `Food` nào nằm trên ô `DeepOcean`/`Ocean` (ví dụ khôi phục từ save tiền-LAND1 hoặc kịch bản có thức ăn trên mặt nước) đều khiến con mồi đang đứng trên ô cỏ đất liền giàu năng lượng (`r >= GRASS_STAY_FRACTION * r_max`) bỏ qua cỏ để lái thẳng về phía `Food` dưới biển, rồi bị `terrain_navigation_system` giật lùi lặp đi lặp lại ở đường bờ (`reverted_this_tick == true`). Fallback toàn cục `:165` khi hết cỏ trong vùng `7×7` cũng mắc cùng lỗi.
- **Tái hiện:** `just verify` hoặc `just test` (chạy `post_194_hardening_audit_tests::grass_steering_with_land_mask_steers_prey_into_ocean_food_instead_of_grazing_land_grass`)
- **Test khoá lại:** `post_194_hardening_audit_tests::grass_steering_with_land_mask_steers_prey_into_ocean_food_instead_of_grazing_land_grass` (khẳng định `vel_priority2 == Vec3::ZERO`, `revert_count == 0`, `vel_fallback == Vec3::ZERO`, bỏ qua `Food` phi hữu hạn, và đối chứng dương vẫn chọn `Food` đi được trên đất liền kể cả khi có `Food` trên biển ở gần hơn; giữ nguyên 0 heap allocation ở `locomotion_steering_zero_alloc_tests` và `grass_steering_tests`)
- **Trạng thái:** FIXED (nhánh `mac/r5-findings-fixes`, R5 Part 2)

### F-054 — `check:bundle` nhận ra chunk 3D chỉ bằng tên file, nên không thấy three.js bị import thẳng vào trang chính
- **Ngày:** 2026-11-20
- **Loại:** BUG
- **Vị trí:** `scripts/check_bundle_budget.mjs:170` (`THREE_D_CHUNK = /^react-three-fiber/`), dùng ở `:180-198` cho kiểm tra tách route.
- **Phát hiện bởi:** tay (người thẩm định độc lập thẻ M2, PR #188; lead tái hiện lại)
- **Triệu chứng:** Chú thích ở `:147-158` nói kiểm tra tách route bắt đúng trường hợp có người thêm `import * as THREE from 'three'` vào trang chính. Khi làm đúng như vậy, rolldown gom three.js vào một chunk tên `defineProperty-*.js` (502 KiB) và `index.html` tải nó, nhưng kiểm tra in `entry index.html ... 3D renderer: no` và không báo lỗi, vì tên chunk không bắt đầu bằng `react-three-fiber`. Cổng vẫn đỏ, nhưng chỉ nhờ ngân sách byte: chunk `defineProperty` vượt 170 KiB, tổng chỉ vượt 3,2 KiB. Một thay đổi nhỏ hơn về byte sẽ lọt qua.
- **Tái hiện:** trên trunk `4794b6f`: `git show 82a7a604:scripts/paper/gate_faults/check_bundle.patch | git apply && npm run build && npm run check:bundle`. Kết quả có dòng `entry index.html 12 static chunk(s), 3D renderer: no` và `OVER defineProperty 501.9 KiB`. Hoàn tác bằng `git checkout -- src/App.tsx`.
- **Test khoá lại:** chưa có. Bản sửa nên nhận ra three.js theo nội dung hoặc theo module gốc trong manifest của Vite, không theo tên chunk; đối chứng âm chính là patch trên.
- **Trạng thái:** FIXED (03b9d3d9, #197, test tests/frontend/checkBundleBudget.test.ts)
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** PR `#197` đã merge (`03b9d3d9`, 2026-11-22T13:56:28Z); `scripts/check_bundle_budget.mjs:261-264` quét `collectStaticChunkClosure` + `hasThreeJsSignature` (`:116,124`), khoá bởi 7 test trong `tests/frontend/checkBundleBudget.test.ts:73,113,223`. Dòng gốc: OPEN (đã có bản vá ở Draft PR #197, nhánh `mac/fix-vacuous-gates-f054`).

### F-053 — `live.mean_lifespan` trong `final_observables` luôn bằng 0: cửa sổ đã bị lần lấy mẫu cuối rút cạn trước khi đọc
- **Ngày:** 2026-11-20
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/core/live_experiment.rs:1700-1710`: lần lấy mẫu chuỗi thời gian gọi `VitalStatistics::sample_window()`, hàm này đóng và xoá cửa sổ sinh/chết.
  - `src-tauri/src/core/experiment_runner.rs:548` (`final_observables = model.observables()`): đọc sau mẫu cuối, đi qua `peek_window()` trên một cửa sổ đã trống, nên `mean_lifespan = None`, ghi ra thành `0.0`.
- **Phát hiện bởi:** tay (người thẩm định độc lập thẻ E3-X-run)
- **Triệu chứng:** Trong E3-X (#181), `live.mean_lifespan` ở `final-observables.json` bằng `0.0` ở cả 36/36 lượt, dù các cá thể có chết. Endpoint phụ S6 của E3 vì thế đọc một cửa sổ rỗng và được ghi là `reported_not_interpreted_structural_zero`. Thực nghiệm kiểm chứng trên `HD_SEED = 0x5EED` (`duration_ticks = 3_000`, `sample_period = 60` so với đối chứng không thẳng hàng `sample_period = 2_000` có cùng `final_checksum`) xác nhận **cả 5** observable cửa sổ (`live.births`, `live.deaths`, `live.mean_lifespan`, `live.deaths_starvation`, `live.deaths_dehydration`) trong `final_observables` đều bị rút về `0.0`.
- **Tái hiện:** `python3 -c "import json;d=json.load(open('artifacts/experiments/e3-productivity-gradient-body-mass/confirmation/final-observables.json'));print(d)" | grep -o "live.mean_lifespan[^,]*"` (cả 36 giá trị đều là `0.0`), hoặc chạy `just test` (`post_194_hardening_audit_tests::f053_final_observables_drains_all_five_windowed_vital_metrics_on_aligned_duration` và `observables_tests::cumulative_mean_lifespan_matches_bit_exact_across_disk_save_load_fork`).
- **Test khoá lại:** `post_194_hardening_audit_tests::f053_final_observables_drains_all_five_windowed_vital_metrics_on_aligned_duration`, `observables_tests::{cumulative_mean_lifespan_tracks_all_windows_without_draining_and_respects_zero_deaths, cumulative_mean_lifespan_matches_bit_exact_across_disk_save_load_fork}`, và `prereg_sel2p_manifest_tests::drift_arm_omits_cause_split_observables_and_smoke_manifests_are_blind_to_body_mass` (khẳng định `live.cumulative_mean_lifespan` tích luỹ `[0, tick]` không bị rút cạn và khớp từng bit qua `run K -> save -> load -> run N-K`, trong khi 5 chỉ số cửa sổ cũ và toàn bộ artifact đã commit `E2`/`E3`/`E-SEL1`/`SEL2-P` giữ nguyên không đổi).
- **Trạng thái:** mitigated (`live.cumulative_mean_lifespan`, nhánh `mac/r5-findings-fixes`, R5 Part 2 — thẻ `F053-FIX`). **Phần còn OPEN:** cả 5 bộ đếm cửa sổ cũ trong `final_observables` (`live.births`, `live.deaths`, `live.deaths_starvation`, `live.deaths_dehydration`, và `live.mean_lifespan` cũ) vẫn bị rút về `0.0` khi `duration_ticks % sample_period == 0`. **Khai báo tác động registry:** do bổ sung `live.cumulative_mean_lifespan` vào `ObservableRegistry::live_default()`, mọi lượt **chạy lại** tương lai của `E2`, `E3`, `E-SEL1`, hoặc `SEL2-P` sẽ có thêm 1 cột chuỗi thời gian và `run_id` mới (registry fingerprint thay đổi); các artifact đã commit giữ nguyên không đổi.
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** PR `#200` đã merge (`d0747e37`, 2026-11-23T12:51:48Z); hai test `post_194_hardening_audit_tests::f053_final_observables_drains_all_five_windowed_vital_metrics_on_aligned_duration` và `observables_tests::cumulative_mean_lifespan_matches_bit_exact_across_disk_save_load_fork` tồn tại trên thân cây; giữ nguyên `mitigated` vì `live_experiment.rs:1840,1845` (`sample_window`/`peek_window`) và `experiment_runner.rs:548` vẫn rút 5 bộ đếm cửa sổ cũ về `0.0` theo thiết kế giữ tương thích bit cho artifact cũ.

### F-052 — `prey_cap` chỉ là cửa chặn, không phải giới hạn: bộ đếm quần thể không tăng theo số ca sinh đã xếp trong cùng tick
- **Ngày:** 2026-11-20
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/core/agent_systems.rs`, `autonomous_reproduction_system` (khoảng l.865-906 trước FIX-CAP): `prey_count`/`pred_count` được đếm một lần trước vòng lặp các con mẹ, và không tăng khi xếp lệnh `SpawnOffspringCommand`. Cổng thú săn (`pred_count >= 8`, tỉ lệ con mồi/thú săn) bị cùng lỗi.
- **Phát hiện bởi:** đo (thẻ DIAG-SEL2, chẩn đoán vụ sụp đổ của smoke SEL2-P)
- **Triệu chứng:**
  - Khi N vừa xuống dưới `prey_cap`, mọi con mẹ đủ điều kiện cùng sinh trong một tick.
  - Đã đo trên seed 870001-870006:
    - nền với cap 80 lên tới 142-151 con;
    - 60 sáng lập với cap 80 thành 120 ngay ở tick 1;
    - 200 sáng lập với cap 320 thành 400 ngay ở tick 1.
  - Mọi lượt chạy trước đây có dùng `prey_cap` (S12-S15, SEL2-P, E3) đã đo đúng những gì chúng chạy. Chúng chưa từng đo một quần thể bị giữ đúng ở cap đã ghi.
- **Tái hiện:** `cd src-tauri && cargo test --features desktop --test strict_prey_cap_tests strict_prey_cap_holds_with_births_while_legacy_overshoots`. Đối chứng dương trong test cho thấy đường cũ vượt cap.
- **Test khoá lại:** `strict_prey_cap_tests::{strict_prey_cap_holds_with_births_while_legacy_overshoots, strict_cap_holds_for_predators_ceiling_and_ratio, drift_control_replay_respects_strict_cap_while_legacy_bypasses_it, absent_and_zero_produce_identical_checksums}`
- **Trạng thái:** FIXED dạng opt-in: khoá `live.strict_prey_cap` (FIX-CAP #177, FIX-CAP2 #180). Đường cũ cố ý giữ nguyên từng bit để các bản đăng ký đã khoá và kết quả cũ vẫn tái lập được; thí nghiệm mới bật khoá này.

### F-047 — Khôi phục `pending_inference` của desktop app khoá theo `AgentLineageId` đơn lẻ làm rẽ nhánh quỹ đạo khi các con cùng bố mẹ trùng định danh
- **Ngày:** 2026-11-19
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/core/simulation_loop.rs:1713-1738` (trước khi sửa: `entities_by_lineage: HashMap<String, Entity>` ghi đè thực thể anh/chị trước bằng anh/chị sau cùng `lineage_id`, làm gán sai thực thể hoặc không giao phản hồi `PendingInference`);
  - `src-tauri/src/core/snapshot.rs:678-715` (trước khi sửa: `pending_by_lineage` lấy khoá `response.lineage_id` đơn lẻ nên đếm thiếu và trả `InvalidData` khi hai anh/chị cùng `PendingInference`);
  - `src-tauri/src/core/simulation_state.rs:414-428` (trước khi sửa: `SavedSharedLearningState::validate` từ chối `lineage_id` lặp lại trong `pending_inference`);
  - `src-tauri/src/core/agent_systems.rs:1066` (`execute_reproduction_birth` gán mọi con của cùng một cha một chuỗi `AgentLineageId` giống nhau `{parent_lineage_id}.{parent_generation + 1}`).
- **Phát hiện bởi:** tay (cô lập F047 sau chẩn đoán F046)
- **Triệu chứng:** Khi desktop app save/restore tại tick có nhiều anh/chị cùng `AgentLineageId` và ít nhất một cá thể đang chờ `PendingInference(ticket_id)`, bản đồ `entities_by_lineage` ghi đè handle thực thể trước bằng thực thể sau; phản hồi mang `request_id` của cá thể trước bị gửi nhầm sang cá thể sau (không khớp `ticket_id` trong `action_resolution_system`), khiến cá thể thật giữ `PendingInference` sang tick sau hoặc tới timeout, làm lệch `action_gates` / `cpg_parameters` rồi rẽ nhánh `world_checksum` (hoặc khiến `snapshot::read` từ chối save hợp lệ bằng `InvalidData` nếu cả hai sibling đang chờ suy luận).
- **SMELL liên quan:** siblings share `AgentLineageId` — `execute_reproduction_birth` (`src-tauri/src/core/agent_systems.rs:1066`) đặt `child_lineage = format!("{parent_lineage_id}.{}", parent_generation + 1)` khiến mọi anh/chị em cùng bố mẹ không thể phân biệt chỉ bằng `AgentLineageId` ở bất kỳ bảng ánh xạ nào chỉ khoá theo `lineage_id` (lựa chọn F048: sửa bộ ghép khôi phục theo `(lineage_id, request_id)` thay vì đổi chuỗi sinh).
- **Tái hiện:** `cd src-tauri && cargo test --features desktop --test f047_restore_divergence_tests -- --nocapture` (và bài test đĩa hai sibling `f048_two_pending_siblings_roundtrip_disk_snapshot_and_route_by_ticket`, chạy hoàn chỉnh trong `30.82s` / tổng `42.71s` ở debug trên macOS).
- **Test khoá lại:** `f047_restore_divergence_tests::{f047_app_harness_r2a_restore_checkpoints, f047_app_harness_r3_restore_checkpoints, f048_two_pending_siblings_roundtrip_disk_snapshot_and_route_by_ticket, f047_live_adapter_r3_like_restore_matches_uninterrupted, f047_live_adapter_s12_s13_world_restore_matches_uninterrupted}`
- **Trạng thái:** FIXED ở thẻ F048, PR #146 (merge 2026-11-19, kèm F047 và S15). Đường thí nghiệm `LiveExperimentAdapter` không bị ảnh hưởng: F047 đo 13 điểm khôi phục, gồm R3 có kẻ săn mồi và thế giới S12/S13, đều khớp. Bài học về test: bản F047 đầu tiên khẳng định "khôi phục R2a ở K=899 phải lệch", đúng trên macOS arm64 nhưng sai trên Ubuntu x86_64 (job `determinism` đỏ); sau F048 mọi test chỉ khẳng định khớp. SMELL ở trên (anh em ruột chung `AgentLineageId`) vẫn mở; chỉ phần ghép khôi phục được sửa.

### F-045 — Khôi phục checkpoint thế giới sống làm sai ranh giới lập chỉ mục `SpatialHashGrid`, gộp bảng ECS `AgentSex` và đặt lại `NextNodeId(3)`
- **Ngày:** 2026-11-18
- **Loại:** BUG
- **Vị trí:**
  - `src-tauri/src/core/live_experiment.rs:2466` (`populate_spatial_grid` trước khi sửa chạy `rebuild_spatial_grid_system` trên toàn bộ thực thể thay vì lọc các thực thể sinh sau pha rebuild của tick `K`);
  - `src-tauri/src/physics/spatial.rs:15,357-429` (thiếu dấu `IndexedInGrid` cập nhật tại chỗ trong `rebuild_spatial_grid_system`);
  - `src-tauri/src/core/simulation_state.rs:134,954-956,1050-1118,1321-1381` (`SerializedAgent` bỏ qua `AgentSex`, gộp chung founder `None` và con non `Some` vào một bảng ECS);
  - `src-tauri/src/core/live_experiment.rs:2764-2767` và `src-tauri/src/core/simulation_loop.rs:1389-1393` (`install_runtime_resources` luôn đặt `NextNodeId(3)` khi khôi phục, còn `simulation_loop` chỉ tính từ nút đang sống).
- **Phát hiện bởi:** đo (kiểm chứng tương đương khôi phục S10 trên cấu hình nền S6, `seed = 3`)
- **Triệu chứng:** Trước khi sửa (trên `38e8d1d`), khôi phục thế giới nền S6 (`seed = 3`) lệch ở hai mốc độc lập:
  1. Khôi phục ở tick `1` -> chạy tới tick `2`: lưới khôi phục chứa `240` collider thay vì `100` (vì con non sinh sau `rebuild_spatial_grid_system` bị lập chỉ mục sớm), làm checksum tick `2` lệch ngay (`0x109047fe` so với `0xf5876d6e`);
  2. Khôi phục ở tick `600` -> chạy tới `1.200`: `sex_res = 0` so với `sex_orig = 43` (mất tách bảng `AgentSex`), các ca chết `swap_remove` làm đảo thứ tự duyệt bảng từ sau tick `600` dẫn tới rẽ nhánh đầu ở tick `859` (`1332331413` so với `4057091745`) và kết thúc tại `0x1d0183a6` so với `0x21424343`;
  3. Khôi phục sau đột biến cấu trúc `AddNode` (tick `394`, `NextNodeId = 6`) đặt lại `NextNodeId(3)`, và nạp save cũ không khôi phục bộ đếm an toàn từ cả `agents` lẫn `dormant_agents`.
- **Tái hiện:** `cd src-tauri && CARGO_BUILD_JOBS=4 cargo test --features desktop --test f045_restore_equivalence_tests -- --nocapture --test-threads=1`
- **Test khoá lại:** `src-tauri/tests/f045_restore_equivalence_tests.rs` — 5 test (`f045_probe1_tick1_to_tick2_restore_matches_uninterrupted`, `f045_tick600_to_tick1200_restore_matches_uninterrupted_every_tick`, `f045_structural_add_node_mutation_next_node_id_survives_restore`, `f045_old_save_without_new_fields_loads_and_falls_back_cleanly`, `f045_uninterrupted_s6_base_trajectory_fingerprint`). Trên base `38e8d1d` có 4 test đầu FAIL và test thứ 5 PASS; sau khi sửa cả 5 test PASS và quỹ đạo liền mạch giữ nguyên từng bit (`0x30ea8bbf`, `0xf5876d6e`, `0xbc114750`, `0x21424343`).
- **Trạng thái:** FIXED ở thẻ F045, PR #140 (2026-11-19; lead kiểm lại trên macOS: base 1/5 → head 5/5, quỹ đạo liền mạch giữ nguyên, `verdicts/F045_a87f1a68.md` trên hub). Còn một khe ngoài phạm vi thẻ: đường genesis và khôi phục của app desktop (`core/simulation_loop.rs`) sinh cây, hồ, thức ăn chưa có `IndexedInGrid`, nên một lần khôi phục trong app có thể tách bảng ECS khác lượt chạy liền mạch, cùng cơ chế với `AgentSex`. F046 đã đo (2026-11-19): marker không đổi checksum ở 8 chế độ của app, vì mã thật không bao giờ xoá `Tree`/`Lake`; không cần sửa. F046 tìm ra một lệch khác ở app, là F-047.

### F-044 — Ân hạn chết đói và ân hạn chết khát dùng chung một bộ đếm, nên chúng không độc lập khi khác nhau
- **Ngày:** 2026-11-17
- **Loại:** SMELL
- **Vị trí:** `src-tauri/src/core/mortality.rs:57-58` (`starved = energy <= 0 && starving_ticks >= starve_ticks`, `dehydrated = hydration <= 0 && starving_ticks >= dehydration_ticks`); `src-tauri/src/core/world_systems.rs` trong `metabolic_decay_system` (`starving_ticks` tăng khi năng lượng **hoặc** nước ở đáy, về 0 khi cả hai dương); `mortality.rs:71-76` (chết khát ghi vào `DemographicTracker` là "Starvation"/"Senescence")
- **Phát hiện bởi:** tay (chẩn đoán thăm dò sau E-SEL1, rồi đọc lại code)
- **Triệu chứng:** khi `live.starve_ticks` khác `live.dehydration_ticks`, mức ân hạn của đáy này bị tính từ lúc chạm đáy kia. Ví dụ `starve_ticks` 1.000 và `dehydration_ticks` 10: một con đã đói 500 tick (nước vẫn còn) chết khát **ngay** tick nước chạm 0, không có 10 tick ân hạn nào. Với mặc định 120 / 120, quy tắc tương đương "120 tick liên tiếp ở đáy, bất kể đáy nào", nên mọi lần chạy đã có (E2, E-SEL1 smoke, cấu hình mặc định) không bị ảnh hưởng. `dual_mortality_grace_tests` đặt `dehydration_ticks` 65.536 nên cũng không chạm tới. Chết khát bị ghi sai nguyên nhân chỉ ảnh hưởng telemetry; code tự chú thích điều đó.
- **Tái hiện:** `cd src-tauri && cargo test --features desktop -j 2 --test f044_shared_bottom_counter_tests -- --nocapture`. Một founder với `starve_ticks` 65.536 và `dehydration_ticks` 10, không có gì để ăn hay uống, được cho nước 200 tick. Nếu nó đói suốt 200 tick đó, nó chết **ngay** tick nước cạn: dòng `F044 hungry: counter_at_thirst=200 ticks_survived_after_thirst=0`. Nếu nó no (đối chứng), nó sống trọn ân hạn khát: `F044 fed: ... ticks_survived_after_thirst=9`.
- **Test khoá lại:** `f044_shared_bottom_counter_tests::f044_a_long_hungry_animal_gets_no_thirst_grace` là test đặc tả, ghim hành vi hiện tại, và phải được **đảo** khi tách bộ đếm. `f044_control_a_fed_animal_does_get_its_thirst_grace` là đối chứng, cho thấy ân hạn khát hoạt động khi đứng một mình.
- **Trạng thái:** FIXED ở thẻ S8, PR #106 (`f3e9e47`, 2026-11-17), theo quyết định của chủ dự án cùng ngày:
  hai bộ đếm riêng, `starving_ticks` cho năng lượng và `dehydrating_ticks` cho nước; test
  `f044_a_long_hungry_animal_still_gets_its_full_thirst_grace` khẳng định hành vi đã sửa. Văn bản gốc của
  dòng này: OPEN. Sửa đúng là hai bộ đếm riêng. Việc đó thêm một trường vào `HomeostaticState`, chạm định dạng lưu và đổi quỹ đạo của mọi cấu hình có hai mức ân hạn khác nhau, nên cần quyết định của chủ dự án và một thẻ riêng.

### F-043 — `live.food_cap` không có tác dụng: `apply_environmental_effects_system` ghi đè trần thức ăn bằng hằng 50 ở mỗi tick
- **Ngày:** 2026-11-17
- **Loại:** BUG (yếu tố khai báo trong manifest không tới được thế giới)
- **Vị trí:** `src-tauri/src/core/world_systems.rs:21` (trước khi sửa: `food_settings.max_food_count = (50.0 * max_food_multiplier) as usize`, chạy mỗi tick trong `build_tick_schedule`); `src-tauri/src/core/live_experiment.rs` (adapter chỉ đặt `FoodSpawnSettings { max_food_count: config.food_cap }` lúc genesis, nên giá trị khai báo sống đúng tới tick đầu tiên)
- **Phát hiện bởi:** smoke E-SEL1 lần 1 (commit `1320939`, seed 999971, bản release, `scripts/esel1_smoke_driver.mjs`)
- **Triệu chứng:** nhánh khan hiếm (`live.food_cap` 25, 17, 13, 6) và nhánh dồi dào (50) kết thúc ở **cùng một checksum cuối** sau 18.000 tick, bit cho bit; `min live.agent_count` 13 ở cả hai, trung bình `live.standing_crop` nửa sau 123.973,79 ở cả hai, nên tỉ số r của bước C bằng đúng 1 ở cả bốn c, và smoke dừng theo thuật toán đã đóng băng. Không giá trị khối lượng cơ thể nào được đọc hay ghi (runner cưỡng chế ở chế độ `--smoke`).
- **Hệ quả:** mọi manifest live khai `live.food_cap` khác 50 đã chạy với trần 50. Không artifact nào đã commit bị ảnh hưởng (E2 khai đúng 50). Tiền đề "không có thức ăn" của `dual_mortality_grace_tests` (`food_cap = 0`) trước đây không đúng (thức ăn vẫn sinh ra); test vẫn xanh sau khi sửa. Test tự chèn `FoodSpawnSettings` rồi chạy hệ này mà không qua adapter (vd. `meta_ai_tests`) vẫn dùng cơ số 50 như trước, đúng như chúng đang khẳng định.
- **Tái hiện:** `cd src-tauri && cargo test --features desktop -j 2 --test live_food_cap_tests` trên code cũ: 3/4 test đỏ (đã đo bằng cách trả dòng cũ về tạm thời); bản ghi smoke ở `artifacts/experiments/e-sel1-smoke-record/run-1-stopped/`.
- **Test khoá lại:** `live_food_cap_tests::{a_declared_food_cap_is_the_cap_in_force_on_every_tick, a_scarce_cap_changes_the_world_and_feeds_it_less, a_resumed_run_keeps_its_declared_cap}` (đỏ trên code cũ) và `omitting_the_key_and_declaring_the_default_cap_run_the_same_world` (chốt đường mặc định, xanh ở cả hai phía). Không test nào ghim checksum tuyệt đối.
- **Trạng thái:** FIXED bởi commit `fix(live): live.food_cap reaches the world instead of being reset to 50 every tick (F-043)` (nhánh `fix/f043-live-food-cap`). Adapter chèn tài nguyên `LiveFoodCap(config.food_cap)` trong `install_runtime_resources`, chạy cả lúc genesis lẫn lúc restore; hệ đọc nó qua `Option<Res<..>>` và nhân hệ số sự kiện lên cơ số đó. Thế giới không có tài nguyên này (mọi lần chạy app) giữ hằng 50, nên đường mặc định không đổi; vì thế không bump `LIVE_MODEL_VERSION`. Bài học cho tiền đăng ký: smoke phải có một kiểm tra thao tác (manipulation check), vì thế driver smoke giờ dừng khi control và treatment của cùng một lượt thử có cùng checksum cuối.

### F-042 — Sinh vật không tự di chuyển được: lực khớp là nội lực, vật lý không có lực ngoài, và không hệ nào ghi `target_velocity`
- **Ngày:** 2026-11-16
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/physics/dynamics.rs:110-111` (`child_body.force += f_spring; parent_body.force -= f_spring`: nội lực, tổng bằng 0, không dịch được khối tâm); `src-tauri/src/physics/dynamics.rs:121` và `:195-212` (`integrate_physics_system` chỉ làm `velocity += force/mass*dt`, không có tiếp xúc nền, ma sát, cản dị hướng hay lực ngoài nào khác); `src-tauri/src/physics/dynamics.rs:153-156` (lực hướng tới `InertiaComponent.target_velocity` là lực ngoài duy nhất); `src-tauri/src/core/components.rs:465` và `:475` (`target_velocity` mặc định `Vec3::ZERO`); `src-tauri/src/core/simulation_state.rs:196` (chỉ kiểm tra hữu hạn, không ghi); toàn bộ `src/` không có chỗ nào ghi `target_velocity`; nguồn dịch chuyển duy nhất là xung sinh trong `src-tauri/src/core/agent_systems.rs:478-494` (hướng ngẫu nhiên, 2.0 đơn vị/s thú săn, 1.5 con mồi) áp lên mọi segment tại `:569`
- **Phát hiện bởi:** đo (điều tra thú săn tuyệt chủng khi bật sinh sản, thăm dò)
- **Triệu chứng:** CPG chạy thật (biên độ khoảng 1.3, tần số khoảng 2.8-2.9, vận tốc segment khoảng 1 đơn vị/s dao động) nhưng gốc của founder gần như không đi đâu. Seed 1, adapter live, 10 founder (3 thú săn + 7 con mồi, thân 3 segment giống nhau): độ dịch ròng của gốc qua tick 0-200 khoảng 0.035-0.040, qua tick 0-1.000 khoảng 0.025-0.028 (đường đi khoảng 0.5), CẢ HAI lớp; seed 2 ròng khoảng 0.000. Hệ quả: thú săn founder không bao giờ tới được con mồi (năng lượng thu từ con mồi bằng 0 tới khi chết đói ở khoảng 2.000 tick, seed 1 và 3), và fitness sinh thái mặc định (Locomotion) không có vận động nào để chọn lọc. Con non sinh ra chỉ trôi đạn đạo theo xung sinh (không có lực cản) tới khi năng lượng về 0 thì vận tốc bị xoá. Xung ngẫu nhiên ban đầu KHÔNG phải cách sửa: đã đo, nó làm thú săn chết đói nhanh hơn.
- **Tái hiện:** `cd src-tauri && cargo test --features desktop -j 2 --test no_locomotion_characterization_tests -- --nocapture` (dòng `F042 ticks=1000 founders=10 max_net=0.030283825 mean_path=0.5258204`)
- **Test khoá lại:** `no_locomotion_characterization_tests::founders_oscillate_but_do_not_translate`. Test này GHIM hành vi hiện tại (max_net < 0.5 sau 1.000 tick trong khi CPG hoạt động) và phải được ĐẢO khi có vận động.
- **Trạng thái:** ĐÃ CÀI ĐẶT, OPT-IN (ADR-0006 phương án A, QĐ-LOC = A), nhánh `feat/locomotion-steering`, commit `feat(physics): opt-in steering drive so creatures can move (ADR-0006, F-042)`. Cơ chế: `core/locomotion.rs::steering_drive_system`, đặt giữa `resolve_joints_system` và `integrate_physics_system` trong chuỗi toàn phần; bật bằng khoá manifest `live.locomotion = 1` (tài nguyên `LocomotionPolicy`). **Mặc định không đổi**: khoá vắng hoặc `0` thì hệ trả về ngay, `target_velocity` giữ 0, checksum bằng đúng thế giới không có khoá (so sánh các lần chạy với nhau, không ghim giá trị tuyệt đối); `no_locomotion_characterization_tests` (thế giới mặc định) vẫn xanh nguyên văn, nên KHÔNG cần bump `LIVE_MODEL_VERSION` cho đường mặc định. Hình học dùng lại của `sensory_system` (Euclid không torus, thú săn tới con mồi sống gần nhất, còn lại tới thức ăn gần nhất; cache `Local` riêng vì cache của `sensory_system` là `Local` không đọc được); tốc độ = tối đa của lớp (2.0 / 1.5) × biên độ CPG trung bình của các segment con trực tiếp / 1.5, nên tốc độ di truyền qua CPG/não; dừng trong bán kính 0.3. Đo (seed 1, 10 founder, 1.000 tick, bật vận động): độ dịch ròng lớn nhất 27.4, đường đi trung bình 22.9 (so với 0.03 và 0.53 ở F-042), tốc độ thực hiện / tốc độ đích trung bình 0.89 (nên không phải chỉnh hệ số lực trong `dynamics.rs`). Với tử vong + sinh sản, thú săn nhận năng lượng từ con mồi lần đầu ở tick 1.025 (tổng 85 EU trong 4.000 tick). EU kín hằng (lệch tương đối 4e-15); 3 tiến trình cho cùng checksum; `steering_drive_system` không cấp phát khi đã ấm. Còn MỞ và là câu hỏi riêng: thú săn có tồn tại lâu hay không (bằng chứng thăm dò: phần lớn thú săn vẫn chết đói vì cân bằng năng lượng và không có nguồn nước). Chưa bật mặc định trong app.
- **Cập nhật (ADR-0007 phương án A, QĐ-ECO = A):** thêm hai khoá opt-in, chỉ có tác dụng khi `live.locomotion = 1`: `live.drive_floor` (sàn trên tỉ lệ lực lái, `max(drive CPG, sàn)`) và `live.thirst_steering` (hydration < 50% mục tiêu, `locomotion::THIRST_STEERING_FRACTION`, thì nhắm hồ còn nước gần nhất). Khoá vắng = hành vi cũ. Chỉ con mồi, 6.000 tick, seed 1-5: còn sống ở t=6.000 là 3/5 seed khi không khoá, 5/5 khi bật cả hai. Sàn lái làm tốc độ bớt di truyền; không diễn giải là tiến hoá vận động.
- **Cập nhật (ADR-0007 thẻ B, 2026-11-17):** thêm ba khoá opt-in `live.kill_hydration` (hydration từ giết, k × ΔE), `live.kill_efficiency` (thay 0.40 trong cả hai nhánh đòn đánh) và `live.target_min_energy` (bộ lái thú săn chỉ nhắm con mồi có E ≥ ngưỡng; cần `live.locomotion = 1`). Khoá vắng = hành vi cũ; B không thành mặc định. Cùng tồn tại 2/5 seed ở (k, e, E) = (1, 0.6, 10); xem ADR-0007.

### F-041 — Thế giới live cùng seed cho checksum khác nhau giữa các tiến trình khi có sinh sản: `spawn_food_system` không có cạnh thứ tự với `detect_food_collisions_system`, và còn hàng chục cặp ambiguity khác ngoài hai lớp gate đang canh
- **Ngày:** 2026-11-16
- **Loại:** BUG (vi phạm hợp đồng tất định; cùng họ F-029/F-030)
- **Vị trí:** `src-tauri/src/core/simulation_schedule.rs:182` (`spawn_food_system` chỉ `.after(apply_environmental_effects)` và `.after(manual_migration)`), `src-tauri/src/core/world_systems.rs:273` (`detect_food_collisions_system` despawn `Food` bằng `Commands`), `src-tauri/src/core/world_systems.rs:210` (`spawn_food_system` đếm `Query<&Food>` rồi bù lên `max_food_count`)
- **Phát hiện bởi:** determinism (nhiều tiến trình con, ghi checksum từng tick)
- **Triệu chứng:** seed 1, 10 founder, `live.mortality=1`, `ReproductionPolicy{enabled:true}`, 3.000 tick, **12 tiến trình chạy tuần tự, không tải gì khác**: checksum cuối `0x5bb50ab7` x10 và `0x99190d0a` x2. Vậy KHÔNG phải do tải: đó là biến thiên theo tiến trình (hash seed của `ScheduleGraph::topsort`, giống F-029), và "6/6 giống nhau khi không tải" trước đó là may. Các lần đo khác còn thấy `0xc98c39fd`, `0x9ac299ad`. Có lần checksum cuối của hai nhóm tiến trình trùng nhau (700 tick) dù tick 626 đã khác: lệch một tick rồi hội tụ.
- **Tick phân kỳ đầu tiên:** tick 626 (chỉ số 625 trong danh sách checksum). Dump trạng thái: toàn bộ agent bit-giống nhau; chỉ khác (a) `SimRng::stream_pos` 174 so với 176 và (b) một thực thể `Food` (energy 30.0, hai lần rút `gen_range`) có mặt ở nhóm tiến trình này tại tick 626 nhưng nhóm kia chỉ có nó từ tick 627. Cùng một lần bù thức ăn, lệch đúng một tick.
- **Nguyên nhân (giả thuyết mạnh, chưa chứng minh tới từng lệnh flush):** số `Food` mà `spawn_food_system` đếm phụ thuộc việc lệnh despawn của `detect_food_collisions_system` (chỗ duy nhất despawn `Food`) đã được flush qua một `apply_deferred` tự chèn trước hay sau nó. Hai system không có cạnh nào giữa chúng và không xung đột thành phần, nên không phải cặp mà gate ambiguity hiện tại nhìn thấy; thứ tự flush do topsort theo tiến trình quyết định. Thử riêng `.before(detect_food_collisions_system)` cho `spawn_food_system` chỉ chuyển biến thiên sang tick 0 (vẫn ba giá trị cuối khác nhau), nên một cạnh đơn lẻ không đủ và bản thử đó đã được hoàn tác.
- **Phạm vi:** `report_every_ambiguous_pair_in_the_tick_schedule` báo **128** cặp, khoảng 80 cặp trong đó có xung đột thành phần thật (vd. `detect_environmental_collisions` x `detect_food_collisions`, `check_migration_boundaries` x `metabolic_decay`/`combat`/`herbivore_grazing`, `autonomous_reproduction` x `mortality`, `update_agent_evaluation` x gần như mọi system ghi). Gate hiện chỉ canh hai lớp: system chuyển EU và system rút `SimRng`. Lớp thứ ba, "system spawn/despawn bằng `Commands` mà system khác đếm hoặc query", không ai canh. Sửa đúng là khai thứ tự toàn lịch (hoặc một chuỗi tổng thứ tự), đổi checksum vàng, nên không vá cục bộ ở đây.
- **Đã loại trừ:** (1) rayon: đường tick không dùng `par_iter` (chỉ `terrain.rs:234`, thuần theo chỉ số, chạy một lần khi sinh địa hình, và `sweeps.rs` ngoài đường tick). (2) executor: adapter luôn `DeterministicMode::on()` nên `SingleThreaded` (`live_experiment.rs:1248`, `simulation_schedule.rs:80`). (3) `HashMap` trên đường tick: `SpatialHashGrid` chỉ duyệt theo kiểu không quan trọng thứ tự, các `HashSet` trong `mutation.rs`/`genotype.rs` chỉ dùng `contains`, `EntityHashMap` có hasher cố định. (4) tải: tuần tự không tải đã tách được.
- **Chưa đo:** đối chứng tắt sinh sản với `live.founders=60` (12 tuần tự + 12 đồng thời) và nhóm 12 tiến trình đồng thời ở 3.000 tick. Nếu cơ chế đúng như trên thì nó áp dụng cả khi không có sinh sản (chỉ cần có agent ăn thức ăn), nên khả năng cao ảnh hưởng mọi lần chạy `ANIMA_FOUNDING_POPULATION`; cần đo trước khi nói chắc.
- **Mức độ (đo 2026-11-16):** mỗi cấu hình chạy 12 tiến trình con tuần tự, không tải, checksum từng tick. (A) thế giới mặc định của trunk (seed 1, founder mặc định, không tử vong, không sinh sản): 3.000 tick và 18.000 tick đều 12/12 giống nhau (`0x379b2291`, `0x3c8ef51d`), không lệch. (B) tử vong bật, sinh sản tắt, `live.founders=60`, 3.000 tick: checksum cuối 12/12 giống nhau (`0xba2e0df1`) nhưng **các tiến trình đã lệch nhau tạm thời từ tick 2** rồi hội tụ lại, tức là biến thiên theo tiến trình có tồn tại ở quần thể lớn ngay cả khi không sinh sản, chỉ là chưa để lại dấu ở checksum cuối. (C) manifest E2 control (`e2-control-shared-brain.json`: 10 founder, predator 0.3, 8 cây, 2 hồ, food_cap 50, seed 700001, không khai tử vong) qua adapter, 18.000 tick: 12/12 giống nhau (`0x823ed631`). (D) cổng hardening `determinism_tests` (`RAYON_NUM_THREADS=1`, xanh): schedule tự khai của nó chỉ có `spawn_food_system` và `wrap_coordinates_system`, không có `detect_food_collisions_system` hay cặp spawn/despawn nào bằng `Commands`, nên không thể bắt F-041; cổng tuần tự khác (`determinism_gate_tests`) cũng dùng lịch khai tay, không phải `build_tick_schedule`. Kết luận: biến thiên đã thấy chắc chắn khi bật sinh sản (10/12 so với 2/12 ở 3.000 tick) và thoáng qua ở B; chưa thấy ở A, C trong 18.000 tick. B hội tụ nên chưa chứng minh được một lần chạy hàng nghìn tick của `ANIMA_FOUNDING_POPULATION` bị lệch kết quả cuối.
- **Tái hiện:** test con theo mẫu `tests/determinism_gate_tests.rs`: `LiveExperimentAdapter::from_manifest` (seed 1, `live.mortality=1`), chèn `ReproductionPolicy{enabled:true}`, chạy 700 tick, in checksum từng tick; chạy 12 tiến trình tuần tự, chỉ số 625 là nơi chúng tách nhau.
- **Test khoá lại:** `schedule_ambiguity_tests::the_tick_schedule_has_no_ambiguous_pair_at_all` (0 cặp ambiguity, gồm cả cặp chỉ xung đột qua `Commands`, vì mọi cặp đều có thứ tự), `declared_order_keeps_the_causal_sequences` và `spawns_are_flushed_before_their_observers` (thứ tự năng lượng, rút `SimRng`, điểm flush); đa tiến trình qua `build_tick_schedule`: `schedule_total_order_tests::{turnover_world_is_one_world_in_six_processes, crowded_world_is_one_world_in_four_processes, default_and_e2_worlds_stay_one_world_in_three_processes}`; lấp lỗ hổng test: `determinism_tests::live_schedule::*` chạy `build_tick_schedule` (bổ sung, không thay test cũ). Không test nào ghim checksum tuyệt đối (khác nhau theo nền tảng).
- **Cách sửa:** `build_tick_schedule` giờ là MỘT chuỗi `.chain()` duy nhất cho cả 52 nút kể cả 5 `apply_deferred` (chuỗi lồng nhau: mỗi nhóm con có `.chain()` riêng, vì `((a,b),(c,d)).chain()` chỉ sắp thứ tự giữa các nhóm), `auto_insert_apply_deferred = false` để điểm flush cũng được khai báo. Thứ tự chọn là một thứ tự thực sự do tiến trình thuộc nhóm đa số (9/12 thứ tự khác nhau cho cùng checksum ở cấu hình sinh sản) chạy ra, nên các thế giới vốn bất biến giữ nguyên quỹ đạo. Không cạnh `.after/.before` nào còn lại (chuỗi bao hàm chúng). Bài học: `Schedule::systems()` trả đúng thứ tự thực thi, nên đọc được thứ tự từng tiến trình để so (12 tiến trình, 12 thứ tự khác nhau, 485 cặp đảo chiều).
- **Đo sau sửa (12 tiến trình tuần tự mỗi cấu hình, harness `schedule_total_order_tests`, bản release, 3.000 tick trừ khi ghi khác):** sinh sản + tử vong (seed 1, 10 founder): trước `0xbe784d0d` x8, `0x4fe479c0`, `0x8aae02c6`, `0xf618c034` (4 giá trị) -> sau `0xbe784d0d` x12. Mặc định: `0x27914ceb` x12 -> `0x27914ceb` x12; 18.000 tick `0x570663b7` -> `0x570663b7`; E2 18.000 tick `0x79f8485a` -> `0x79f8485a`; tử vong + 60 founder `0xba2e0df1` -> `0xba2e0df1`. Không checksum nào đang bất biến bị đổi; golden `determinism_hash_ghim` giữ nguyên. (Con số 0x379b2291/0x3c8ef51d/0x823ed631/0x5bb50ab7 ở trên đo bằng harness khác nên không khớp tuyệt đối với harness này; so sánh trước/sau ở đây cùng harness.)
- **Trạng thái:** FIXED bởi commit `fix(sched): give the tick schedule one total order so a seed means one world in every process (F-041)` (nhánh `fix/f041-total-schedule-order`, xếp trên `finding/f041-schedule-order`).

### F-040 — Restore một thế giới live đã có ca chết cho checksum khớp tại điểm lưu nhưng rẽ nhánh ngay tick kế tiếp (thứ tự hàng ECS không được khôi phục); anh em cùng cha còn dùng chung một lineage id
- **Ngày:** 2026-11-16
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/core/live_experiment.rs:1464` (`from_snapshot` -> `build` dựng lại thế giới từ đầu, thứ tự hàng bảng ECS khác thế giới gốc); `src-tauri/src/physics/dynamics.rs:43` (`resolve_joints_system` duyệt `joint_query` theo thứ tự bảng, kết quả phụ thuộc thứ tự); thứ yếu: `src-tauri/src/core/agent_systems.rs:~682` (`sort_unstable_by` trên khoá lineage không duy nhất, có cấp phát `Vec`/`String` mỗi tick khi bật sinh sản, SMELL) và `:~793` (`child_lineage = "{cha}.{thế hệ+1}"`, mọi con của một cha trùng id); `src-tauri/src/core/simulation_state.rs:105` (`SerializedAgent` không mang `AgentSex`, chỉ ảnh hưởng census/telemetry)
- **Phát hiện bởi:** đo (Card S1 test (d): fork và save->load sau ca chết đầu tiên và ca sinh đầu tiên)
- **Triệu chứng:** Với `live.mortality=1` và sinh sản bật (seed 1, 10 founder), `checkpoint_fork` tại tick 1328 cho checksum control 3384047760, trong khi chạy liền mạch cho 1538591415. Ngay sau `from_snapshot` checksum vẫn bằng bản gốc; tick kế tiếp thì lệch (detritus 1452.4685 so với 1452.4746, `animals_eu` 494.5514 so với 494.5453, tức khoảng 0.006 EU, chỉ do một agent). Diff JSON hai snapshot cho thấy cùng dữ liệu nhưng hai segment của agent đó đảo thứ tự (`segment_id` 2,1 so với 1,2). Quét checkpoint mỗi 25 lần lặp: chỉ sinh sản (không ai chết) 0/100 lệch; chỉ tử vong lệch ở mẫu 1850 và 1875; tử vong + sinh sản lệch 41/100 mẫu (từ 1275). **Giả thuyết ban đầu "anh em cùng id làm thứ tự sort_unstable khác sau restore" bị bác:** có hai agent cùng id `live-founder-0003.1.2` tại tick 1328 (và trùng đầu tiên ở tick 1001), nhưng thứ tự truy vấn của chúng giống hệt ở hai thế giới và khoá phụ (vị trí, năng lượng) thêm vào bản scratch không đổi checksum nào (vẫn 3384047760 so với 1538591415). Lệch cũng xảy ra khi chỉ có tử vong, nơi không có ca sinh nào. Cơ chế còn là chỉ dấu, chưa chứng minh: thứ tự hàng bảng phụ thuộc lịch sử despawn, restore dựng lại từ đầu, và hệ phụ thuộc thứ tự (giải khớp nối) biến hoán vị thành quỹ đạo khác.
- **Tái hiện:** `cd src-tauri && RAYON_NUM_THREADS=1 cargo test --features desktop --test restore_after_death_order_tests -- --nocapture --test-threads=1` (dòng `F040 mortality-only diverging checkpoints: [1850, 1875]` và `F040 first duplicate lineage id at tick 1001: live-founder-0003.1.2`); ca fork gốc: `live_turnover_tests::a_fork_and_a_save_load_after_the_first_death_and_birth_match_the_uninterrupted_run` trên nhánh `swarm/s1-population-turnover`
- **Test khoá lại:** `restore_after_death_order_tests::a_restore_after_deaths_follows_the_original_on_the_next_tick` và `a_restore_after_deaths_and_births_follows_the_original_on_the_next_tick` (danh sách lệch rỗng; trước khi sửa là `[1850, 1875]` và 41/100), `without_deaths_the_same_restore_does_follow_the_original` (đối chứng: không ai chết thì khớp), `siblings_share_one_lineage_id_which_makes_the_reproduction_sort_key_non_unique` (ghim id trùng). Không có `#[ignore]`.
- **Trạng thái:** FIXED bởi commit `fix(physics): resolve_joints_system no longer depends on ECS table order, so a restore after deaths follows the original (F-040)` (nhánh `fix/f040-restore-order`). Nguyên nhân: `resolve_joints_system` cộng lực lò xo vào thân cha và thân con theo thứ tự duyệt `joint_query` (thứ tự hàng bảng); phép cộng float không kết hợp, còn thứ tự hàng sau despawn theo lịch sử swap_remove trong khi restore dựng lại theo thứ tự serialize. Hệ nay duyệt khớp nối theo `Segment.id` (bộ đệm `Local<Vec>` giữ dung lượng nên không cấp phát khi đã ấm). Đã đo: quét tử vong 0 điểm lệch (trước `[1850, 1875]`), tử vong + sinh sản 0/100 lệch (trước 41/100); test đã đảo. Chưa đo bằng hash từng hệ; cơ chế được xác nhận bằng việc sửa đúng một hệ đó là đủ. Ghi chú còn mở, tách riêng: (1) anh em cùng cha dùng chung lineage id (`child_lineage`), khoá sort sinh sản không duy nhất, chưa là nguyên nhân nhưng là mối nguy; hướng (d)/(e) ở trên còn áp dụng; (2) `SerializedAgent` không mang `AgentSex`, chỉ ảnh hưởng census/telemetry.
### F-039 — Sinh sản liên tục không ghi vào `VitalStatistics`, nên `live.births` báo 0 trong khi vật nuôi đẻ
- **Ngày:** 2026-11-16
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/core/agent_systems.rs:795` (`autonomous_reproduction_system` chỉ gọi `DemographicTracker::record_birth`); `src-tauri/src/core/agent_systems.rs:1029` (chỗ duy nhất gọi `VitalStatistics::record_birth`, là đường thay thế tiến hoá theo kỷ nguyên); `src-tauri/src/core/live_experiment.rs:1086` (`live.births` rút từ `VitalStatistics::sample_window`)
- **Phát hiện bởi:** đo (S1 bước 0, thăm dò, bản release)
- **Triệu chứng:** Trong thế giới live (`LiveExperimentAdapter::from_manifest`, `live.mortality=1`) với `ReproductionPolicy{enabled:true}` đặt lại sau khi dựng, qua 6.000 tick `DemographicTracker` đếm 74 / 232 / 32 / 239 ca sinh (seed 1, 2, 3 và seed 1 với 60 founder), còn `VitalStatistics` (nguồn của `live.births`) luôn 0. Số ca chết thì hai bên khớp. Cùng loại số 0 bịa như F-037. Hiện ở trạng thái ngầm trên trunk vì adapter ép `ReproductionPolicy::disabled()` cho tới QĐ-4, và thành lỗi thật ngay khi bật sinh sản.
- **Tái hiện:** `cd src-tauri && cargo test --features desktop --test live_births_bookkeeping_tests -- --nocapture` (dòng `F039 ticks=1000 seed=1 tracker_births=9 live_births_summed=...`; trước khi sửa là `0`, sau khi sửa là `9`)
- **Test khoá lại:** `live_births_bookkeeping_tests::continuous_births_reach_live_births` (tổng `live.births` bằng số ca sinh của `DemographicTracker` và > 0). Đối chứng `disabled_reproduction_keeps_live_births_zero`: tắt sinh sản thì `live.births` vẫn 0 và hai lần chạy trong cùng tiến trình cho cùng checksum. Việc sửa không đổi đường tắt được **đo** chứ không ghim: trên macOS arm64, checksum sau 1.000 tick là `0x94692e63` ở cả commit cha lẫn commit sửa; checksum tuyệt đối phụ thuộc nền tảng (CI Windows và Linux ra số khác cho cùng commit) nên không đưa vào assert.
- **Trạng thái:** FIXED bởi commit `fix(core): count continuous births in VitalStatistics so live.births is not zero (F-039)` (nhánh `fix/f039-live-births`, xếp trên `finding/f037-dual-mortality-grace`). `autonomous_reproduction_system` nhận thêm `Option<ResMut<VitalStatistics>>` và ghi sinh ngay cạnh `DemographicTracker`; thiếu resource thì không làm gì.

### F-038 — Lai ghép tạo được thân lớn hơn mức restore và migration chấp nhận, và không chỗ nào ở giữa kiểm kích thước
- **Ngày:** 2026-11-16
- **Loại:** SMELL
- **Vị trí:** `src-tauri/src/evolution/crossover.rs:32` (`crossover_genotypes`: đổi cây con của A lấy cây con của B, con có thể tới `|A| - 1 + |B|` node, không có trần); `src-tauri/src/core/simulation_loop.rs:801` (worker tiến hoá gọi nó, không kiểm kích thước sau đó); `src-tauri/src/evolution/map_elites.rs:151` (ô archive là `floor(body_mass / resolution)`, trục khối lượng không có trần, nên một con nặng hơn mọi elite luôn rơi vào ô trống và luôn được giữ); `src-tauri/src/core/components.rs:192` (`MAX_MIGRATION_MORPHOLOGY_NODES = 128`, mà restore, đánh thức, migration và nạp archive đều cưỡng chế); `src-tauri/src/core/simulation_loop.rs:1404` (restore huỷ **toàn bộ** khi một agent bị từ chối)
- **Phát hiện bởi:** tay (verifier của commit `5cfa5eb` nêu đường `crossover_genotypes` chưa qua kiểm tra)
- **Triệu chứng:** Từ thân 15 node, cỡ lớn nhất `mutate_genotype` dựng được, lai ghép lặp lại với chọn lọc "giữ con nặng hơn" vượt trần 128 sau **7 thế hệ** (138 node), mọi bước đều là cây hợp lệ. Nếu một lần chạy thật đi tới đó, thế giới vẫn lưu được (serialize không kiểm), nhưng không restore được nữa. Agent đó cũng không di cư được, và không đánh thức được nếu nó ngủ đông. Vòng thân của G1 (`BODY_WALK_CAPACITY`) sẽ cấp phát trên heap cho thân ấy.
- **Tái hiện:** `cd src-tauri && cargo test --features desktop --test crossover_body_size_cap_tests -- --nocapture` (dòng `F038 generations=7 nodes=138 cap=128 start=15`)
- **Test khoá lại:** `crossover_body_size_cap_tests::repeated_crossover_from_a_mutation_sized_body_never_exceeds_the_restore_cap` (đã đảo: 50 thế hệ, mọi con ≤ 128 node) và `an_oversized_child_is_refused_with_parent_a_and_the_counter_unchanged`. Việc từ chối thân quá trần đã được ghim ở `migration_enqueue_atomicity_tests.rs:863`.
- **Chưa chứng minh:** "giữ con nặng hơn" chỉ đứng thay cho trục khối lượng không trần của archive, không phải chính archive. Chưa đo xem một lần chạy thật có chạm 128 không. Vì vậy loại là SMELL, chưa phải BUG.
- **Trạng thái:** FIXED bởi commit `fix(evolution): refuse crossover children the restore path would reject (F-038)` (nhánh `fix/f037-single-mortality`). Chủ dự án chọn hướng (a) ngày 2026-11-16: `crossover_genotypes` từ chối con vượt `MAX_MIGRATION_MORPHOLOGY_NODES` (node) hoặc có số cạnh >= mức đó, giống nhánh genotype không hợp lệ: khôi phục bộ đếm id và trả `parent_a`; không đổi lần rút RNG nào. Loại vẫn là SMELL vì chưa đo được một lần chạy thật có chạm trần hay không.

### F-037 — Khi bật tử vong, ân hạn `starve_ticks` không bao giờ chạy và `live.deaths` báo 0 trong khi sinh vật chết đói
- **Ngày:** 2026-11-16
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/core/simulation_schedule.rs:246` và `:273` (lịch đăng ký **cả hai** hệ tử vong); `src-tauri/src/core/agent_systems.rs:606-624` (`agent_mortality_system`: chỉ dừng khi policy có mặt **và tắt**, giết ngay khi `energy <= 0.0`, chỉ ghi vào `DemographicTracker`); `src-tauri/src/core/mortality.rs:54-62` (`mortality_system`: chờ `starving_ticks >= starve_ticks`, là nơi **duy nhất** ghi `VitalStatistics`); `src-tauri/src/core/live_experiment.rs:1088` (`live.deaths`, `live.mean_lifespan` rút từ `VitalStatistics`)
- **Phát hiện bởi:** tay (khi xem QĐ-A′ còn treo)
- **Triệu chứng:** Khi khai báo luật chết (`live.mortality = 1` trong manifest, `ANIMA_MORTALITY` trong app), `agent_mortality_system` cũng chạy và giết con vật đúng tick năng lượng chạm đáy, trước khi `mortality_system` kịp đếm tới `starve_ticks`. Đo với một con, không thức ăn, `live.starve_ticks = 30`: nó chết ở tick 167 mà không sống thêm tick nào ở đáy, `DemographicTracker` ghi 1 ca chết đói, còn `live.deaths` cộng qua mọi cửa sổ là **0**. `ensure_observables_supported` (`live_experiment.rs:322`) từ chối `live.deaths` khi không có luật chết, chính để không bao giờ báo một số 0 bịa; có luật chết thì nó vẫn báo số 0 đó. Các test sẵn có (`observables_tests.rs`, `sentence_to_death` ở dòng 165) không bắt được vì chúng giết bằng **mất nước**, giữ năng lượng 500, mà `agent_mortality_system` không xét mất nước. Hệ quả: mọi run có tử vong trên lịch hiện tại đếm chết đói bằng 0 và bỏ qua ân hạn đã khai.
- **Tái hiện:** `cd src-tauri && cargo test --features desktop --test dual_mortality_grace_tests -- --nocapture` (dòng `F037 death_tick=167 most_bottom_ticks_alive=0 starve_ticks=30 tracker_starvation=1 live_deaths=0`)
- **Test khoá lại:** `dual_mortality_grace_tests::a_declared_starvation_grace_is_honoured_and_live_deaths_counts_the_death` (trước đây ghim hành vi lỗi `(0, 1, 0.0)`, nay đã đảo) khẳng định `(29, 1, 1.0)`. `dual_mortality_grace_tests::mortality_system_alone_honours_the_grace_and_records_the_death` là đối chứng: chạy riêng, luật của `mortality_system` đúng (sống 29 tick ở đáy, chết ở tick 30, có trong `VitalStatistics`), nên lỗi nằm ở việc lịch chạy hai hệ cạnh nhau, không nằm ở luật.
- **Trạng thái:** FIXED bởi commit `fix(core): schedule one mortality system so the starvation grace holds (F-037, QĐ-A′)` (nhánh `fix/f037-single-mortality`). `build_tick_schedule` không còn đăng ký `agent_mortality_system` (hàm giữ nguyên trong `agent_systems.rs`, chỉ không được lập lịch); `mortality_system` thành hệ tử vong duy nhất, và nay cũng ghi ca chết vào `DemographicTracker` (cùng quy tắc nguyên nhân: Senescence nếu `survival_ticks >= 300`, ngược lại Starvation) để telemetry nhân khẩu không đọc thành 0.

### F-035 — `run_inference_batch` bỏ qua WGPU cho mọi `BrainGenotype` tiến hoá riêng từng agent
- **Ngày:** 2026-11-14
- **Loại:** SMELL
- **Vị trí:** `src-tauri/src/ai/model.rs:356-394` và `src-tauri/src/ai/model.rs:421-445`
- **Phát hiện bởi:** tay
- **Triệu chứng:** `run_inference_batch` lọc `req.brain.is_none()` trước khi xếp batch cho `BrainModel` (`BrainModelBackend::Wgpu`); mọi request có `Some(BrainGenotype)` đều bỏ qua `BrainModel` và chạy tuần tự trên CPU qua `brain.forward_into`, đồng thời có sẵn parity với batched 3D tensor matmul `[N, 1, 15] -> [N, 15, 64] -> [N, 64, 64] -> [N, 64, 8]`.
- **Tái hiện:** `just verify`
- **Test khoá lại:** `gpu_brain_batch_parity_tests::evolved_brains_in_run_inference_batch_bypass_shared_brain_model`, `gpu_brain_batch_parity_tests::batched_3d_tensor_forward_matches_per_agent_brain_genotype_loop_on_ndarray`, `gpu_brain_batch_parity_tests::batched_3d_tensor_forward_on_wgpu_matches_per_agent_brain_genotype_loop`
- **Trạng thái:** OPEN
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** `src-tauri/src/ai/model.rs:384-406` vẫn rẽ nhánh `(Some(brain), _)` sang `brain.forward_into(...)` trên CPU bỏ qua `BrainModel`; giữ nguyên `OPEN`.

### F-034 — PR #40 đóng 8 gate đỏ nhưng làm đỏ 5 gate đang xanh, và một trong năm cái đó **không phải** staleness
- **Ngày:** 2026-11-01
- **Loại:** BUG (hồi quy gate, một nửa là quyết định chưa ai ra)
- **Vị trí:** `licensing/bundle-closure.json`, `scripts/check_bundle_budget.mjs:189`,
  `package-lock.json` (vite `8.1.5 → 8.3.5`, rolldown `1.1.5 → 1.2.16`,
  `@oxc-project/runtime` `0.139.0 → 0.156.0`)
- **Phát hiện bởi:** tay — đọc hai log CI thật thay vì tin bản báo cáo
- **Triệu chứng:** đo trước/sau từ chính log CI, không đo lại trên máy:
  - PR #39 (`03b331f`, run `41204212880`): đỏ **đúng 8** gate — `npm audit` (root),
    `npm audit` (tests), ESLint ratchet, `cargo fmt --check`, `cargo audit`, `cargo deny check`,
    `cargo clippy --all-targets`, và gate tách feature. Năm gate compliance **xanh**.
  - PR #40 (`78b2e85`, run `41215060684`): tám gate trên **xanh**, và năm gate compliance
    **đỏ** — `check:licenses`, `check:sbom`, `check:notice`, `check:bundle-closure`,
    `check:bundle`.
  Bản báo cáo của Gemini đoán trước **bốn** cái, thiếu `check:bundle` (Bundle budget).
- **Đo thật về bundle — và nó **không** phình:**
  | | chunk | tổng | `main` | chunk mới |
  |---|---|---|---|---|
  | trước (xanh) | 28 | 1849.5 KiB | 160.1 KiB (80% / 200) | — |
  | sau (ĐỎ) | 24 | 1846.6 KiB | **267.7 KiB (134% / 200)** | **`defineProperty` 137.9 KiB (172% / 80)** |
  Tổng dịch **−2.9 KiB**. `client` (130.4), `CanvasRenderer` (84.2) và `init` (24.1) **biến mất**.
  Nghĩa là rolldown 1.2.16 **chia lại chunk**, không ai thêm code. Bảng budget khoá theo *tên*
  chunk, nên ba tên có budget mất đi còn một tên mới xuất hiện và ăn budget `default` 80 KiB.
  Chính rolldown in lời khuyên trong cùng log đó:
  `Use build.rolldownOptions.output.codeSplitting to improve chunking`.
- **Hệ quả:** quyết định trước mặt chủ repo **không phải** "nhận thêm 107 KiB" (không có 107 KiB
  nào) mà là "khoá lại bảng budget theo tên chunk mới, và/hoặc cấu hình `codeSplitting` để lái
  chunk về hình cũ". Rẻ hơn hẳn, nhưng vẫn là quyết định — CLAUDE.md: nâng budget phải nâng trong
  **cùng commit** gây ra nó, kèm lý do.
- **`check:bundle-closure` KHÔNG thuộc R12, và đóng được ngay trên macOS:** R12 chỉ nêu `NOTICE`,
  `licensing/third-party-index.json`, `sbom.cdx.json`, và lý do của nó là `cargo tree` resolve
  theo host target. Gate này chạy ở job **frontend trên `ubuntu-latest`** (`ci.yml:313`,
  `runs-on: ubuntu-latest` ở `:219`), và file đã commit ghi **đúng 20 package JS**, `grep -c
  'binding-'` → **0**, tức không có binding theo nền tảng nào. Gemini áp R12 quá rộng rồi bỏ đỏ
  một gate mà nó vừa **được phép** vừa **làm được**.
- **Tái hiện:** `gh run view 41204212880 --log | grep KiB` so với
  `gh run view 41215060684 --log-failed | grep -E 'bundle budget|KiB'`
- **Trạng thái:** FIXED bởi thẻ T1 (B3b), PR #44 (`62750f3`), vào thân cây qua #45. Ghi nhận
  2026-11-17 từ lượt CI ở STATE §1.g: năm gate compliance cùng xanh. Dòng gốc: OPEN — ba gate
  licensing cần Windows (A7); `bundle-closure` đóng được tại chỗ; `check:bundle` chờ quyết định chủ repo

### F-033 — Miễn trừ `tokio` vừa viết vào `deny.toml` có **bản vá** trong tầm với: `1.53.3`
- **Ngày:** 2026-11-01
- **Loại:** BUG (lỗ hổng được miễn trừ thay vì được sửa)
- **Vị trí:** `src-tauri/deny.toml:107-127` (`[advisories] ignore = ["RUSTSEC-2026-0386"]`),
  `src-tauri/Cargo.lock` (`tokio 1.53.1`)
- **Phát hiện bởi:** tay — kiểm chính file advisory trong `~/.cargo/advisory-db` thay vì tin lập luận
- **Triệu chứng:** `RUSTSEC-2026-0386` khai rõ
  `patched = [">= 1.51.6, < 1.52.0", ">= 1.53.3"]`; lockfile đang ở `1.53.1`, tức **thấp hơn bản vá
  đúng hai patch**. `cargo update -p tokio --precise 1.53.3 --dry-run` →
  `Updating tokio v1.53.1 -> v1.53.3`, resolve sạch, không kéo theo gì khác
  (`159 unchanged dependencies`). Một dòng lệnh đóng được advisory mà một khối miễn trừ 20 dòng
  vừa được viết để sống chung với nó.
- **Hai chỗ lập luận trong chú thích yếu hơn vẻ ngoài:**
  1. Chú thích viết `AsyncFd` nằm trong `tokio::io::unix` (`#[cfg(unix)]`, *"absent on the primary
     `x86_64-pc-windows-msvc` target"*) — đúng với **một** trong **ba** target mà gate thật sự soi.
     `deny.toml [graph] targets` liệt kê cả `x86_64-unknown-linux-gnu` và `aarch64-apple-darwin`,
     đều là unix, nên ở hai target đó `AsyncFd` **có** được biên dịch.
  2. Nửa còn lại — không ai dựng `AsyncFd` — thì **đúng**: `grep -rn AsyncFd src-tauri/src
     src-tauri/tests src-tauri/crates` → 0 khớp. Nhưng "không ai gọi" là lập luận dành cho advisory
     **không có** bản vá. Tiền lệ trong CLAUDE.md cũng vậy: hai miễn trừ Rust đang có là
     *lockfile-only*, tức không build nào chạm tới.
- **Và nó không nhẹ như chữ `unsound` gợi ra:** cùng file advisory ghi
  `categories = ["memory-corruption"]`, `keywords = ["io-safety", "use-after-free"]`.
  `cargo audit` coi `unsound` là informational nên im; `cargo deny` thì đỏ — đó là lý do khối miễn
  trừ xuất hiện, chứ không phải vì không sửa được.
- **Tái hiện:**
  `cat ~/.cargo/advisory-db/crates/tokio/RUSTSEC-2026-0386.md` rồi
  `cd src-tauri && cargo update -p tokio --precise 1.53.3 --dry-run`
- **Cách sửa, đã đo trên CẢ HAI nhánh — một lệnh, không cần quyền `deny.toml`:**
  `cd src-tauri && cargo update -p tokio --precise 1.53.3`
  | nhánh | trước | sau bump |
  |---|---|---|
  | `feat/ecological-selection` | `cargo deny check` exit **1**, `grep -cE '^error'` → **1** | exit **0**, `^error` → **0** |
  | `fix/ci-gates-green` (PR #40, còn khối `ignore`) | `advisories ok` (nhờ miễn trừ) | `advisories ok`, miễn trừ thành warning `advisory-not-detected` tại `deny.toml:125` |
  `cargo audit` vẫn exit 0 và vẫn đếm **773 crate** sau bump, tức không kéo thêm/bớt dependency nào.
  Trên PR #40 nên xoá luôn khối `ignore` cho khỏi để lại warning — việc đó mới cần quyền `deny.toml`,
  và nó là vệ sinh, không phải điều kiện để xanh.
- **Trạng thái:** OPEN
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** `src-tauri/Cargo.lock` đã nâng lên `tokio = "1.53.3"` và `src-tauri/deny.toml:118` để `ignore = []` (xoá ngoại lệ `RUSTSEC-2026-0386` từ commit `62750f3` / PR `#44` `8388e326` → `#45` `27c82736`; CI chạy `cargo deny check` tại `.github/workflows/ci.yml:122`); giữ nguyên `OPEN` do chưa có unit regression test đặt tên riêng cho advisory này.

### F-032 — `deny.toml` phản chiếu miễn trừ của `audit.toml`, hai công cụ không soi cùng một thứ — nhưng đó **không** phải lý do gate đỏ
- **Ngày:** 2026-10-31 (sửa chẩn đoán 2026-11-01)
- **Loại:** SMELL (miễn trừ đặt sai file; chỉ sinh **warning**, không làm đỏ gate)
- **SỬA NGÀY 2026-11-01 — mục này chẩn đoán sai, lần thứ hai cùng một kiểu.** Đo lại trên chính
  `feat/ecological-selection`, đếm theo mức chẩn đoán chứ không theo chữ "FAILED":
  `cargo deny check` in **đúng một** dòng `error`, và nó là
  `error[unsound]: AsyncFd::new and related constructors violate I/O safety` — tức
  `RUSTSEC-2026-0386` của **`tokio`**, một advisory thật, thấy được trong graph. Hai dòng
  `no crate matched advisory criteria` cho cặp `gix` là **`warning`**, không phải `error`:
  `grep -cE '^error' → 1`. Vậy `advisories FAILED` do tokio, **không** do hai miễn trừ phản chiếu.
  Chứng minh trực tiếp: `cargo update -p tokio --precise 1.53.3` (một lệnh, không sửa `deny.toml`)
  → `cargo deny check` **exit 0**, `grep -cE '^error' → 0`, hai warning kia vẫn còn nguyên tại chỗ.
  **Hệ quả:** gate này **không** cần quyền sửa `deny.toml` để xanh — xem [F-033]. Việc xoá hai entry
  khỏi `deny.toml` vẫn đúng (chúng ở sai file và sẽ còn kêu warning), nhưng là **vệ sinh**, không
  phải điều kiện. Phần phân tích phạm vi hai công cụ bên dưới vẫn đúng; chỉ kết luận
  "đỏ vì miễn trừ" là sai.
- **Loại cũ (đã bỏ):** BUG (gate đỏ vì tiền đề sai, không vì lỗ hổng)
- **Vị trí:** `src-tauri/deny.toml:104-111` (`[advisories] ignore`, kèm chú thích
  *"Mirrors `.cargo/audit.toml`"*), đối chiếu `src-tauri/.cargo/audit.toml`
- **Phát hiện bởi:** tay — đo trạng thái 8 gate đỏ của main để viết prompt B3/B4, rồi soát lại
  chính khẳng định của mình
- **Triệu chứng:** `cd src-tauri && cargo deny check` → exit 1,
  `advisories FAILED, bans ok, licenses ok, sources ok`, kèm hai **warning**
  `advisory-not-detected` tại `deny.toml:109` và `:110` (`RUSTSEC-2024-0350`,
  `RUSTSEC-2025-0021`) với `no crate matched advisory criteria`. Cùng cây,
  `cargo audit` → exit **0**. (Khi viết mục này tôi đọc hai warning đó là nguyên nhân của exit 1;
  xem khối sửa ở trên — chúng không phải.)
- **Chẩn đoán sai đã loại bỏ:** hai miễn trừ đó **không** hết hạn. `gix-fs 0.8.1` và
  `gix-features 0.36.1` vẫn nằm trong `Cargo.lock` **đúng phiên bản advisory nêu**. Đo (có kiểm mã
  thoát, bài học F-025 — lệnh in rỗng mà exit 0 không phải kết quả âm):
  | lệnh | kết quả |
  |---|---|
  | `cargo tree -i gix-fs -e normal` | exit 0, `nothing to print` |
  | `cargo tree -i gix-fs -e normal --features desktop` | exit 0, `nothing to print` |
  | `cargo tree -i gix-fs -e all --target all` | exit 0, `nothing to print` |
- **Nguyên nhân thật:** crate có trong lockfile và **không có trong graph nào**. `cargo audit` đọc
  `Cargo.lock` → thấy crate → miễn trừ khớp → vô hại. `cargo deny` **dựng graph** → không thấy
  crate → miễn trừ không khớp gì → deny coi đó là **error**. Hai công cụ, một lockfile, hai phạm vi
  khác nhau.
- **Lỗi nằm ở tiền đề của `deny.toml`:** chú thích của nó tự nhận *"Mirrors `.cargo/audit.toml`"*.
  Nó **không được** phản chiếu. Phần đầu `audit.toml` tồn tại chính là để ghi loại miễn trừ
  chỉ-có-trong-lockfile (*"a lockfile is feature-agnostic ... an advisory can be reported against a
  crate that is provably never built"*), và một miễn trừ loại đó **vô nghĩa** trong công cụ soi
  graph. Một miễn trừ đúng ở file này là lỗi ở file kia.
- **Xuất xứ:** `a8e6b0e` viết hai entry vào `audit.toml` — đúng chỗ. `043930c` (commit đổi giấy
  phép) chép chúng sang `deny.toml` — sai chỗ. **A6 (`f73086c`) không gây ra việc này**; tôi đã nói
  ngược hai lần trước khi đo, và đó là cùng một kiểu lỗi với F-025: đọc một dòng grep khớp rồi suy
  ra nguyên nhân thay vì kiểm.
- **Tái hiện:** `cd src-tauri && cargo deny check; echo $?` → 1, rồi
  `cargo tree -i gix-fs -e all --target all; echo $?` → exit 0 không in gì.
- **Cách sửa:** xoá hai entry khỏi `deny.toml` và sửa chú thích "Mirrors" — hai file phải nói rõ
  mỗi advisory được miễn trừ ở công cụ **thực sự thấy** crate đó, không phải ở cả hai. Chưa làm:
  `deny.toml` là build config, `AGENTS.md` chặn, cần người chủ mở quyền. **Không còn chặn gate:**
  việc này chỉ dọn hai warning; muốn gate xanh thì bump `tokio` ([F-033]), không cần file này.
- **Trạng thái:** OPEN (hạ xuống vệ sinh — không chặn CI)
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** `src-tauri/deny.toml:112-118` đã bỏ toàn bộ bảng phản chiếu `.cargo/audit.toml` (`ignore = []`, kèm chú thích *Do NOT mirror `.cargo/audit.toml` here* từ commit `62750f3` / PR `#44` → `#45`); giữ nguyên `OPEN (hạ xuống vệ sinh — không chặn CI)` do là cấu hình CI không có unit test riêng.

### F-031 — `thread_tracking_allocator_tests` đỏ trên macOS: chính hai `Barrier::wait()` của test nằm trong cửa sổ đo
- **Ngày:** 2026-10-31
- **Loại:** TEST-GAP (lỗi tính khả chuyển của dụng cụ đo, không phải lỗi hot loop)
- **Vị trí:** `src-tauri/tests/thread_tracking_allocator_tests.rs:23-28` (cửa sổ
  `ALLOCATOR.start_tracking()` … `stop_tracking()`), assert ở `:31`
- **Phát hiện bởi:** agent làm task B1b, trong `cargo test --features desktop --no-fail-fast`
  trên macOS (`aarch64-apple-darwin`)
- **Triệu chứng:** `current_thread_tracking_ignores_allocations_from_other_threads` đỏ với
  `left: 3, right: 1`. Test muốn khẳng định cửa sổ đo chỉ đếm cấp phát **của luồng đo**, nhưng
  `start.wait()` (`:24`) và `finished.wait()` (`:27`) **nằm bên trong** cửa sổ đó. Trên macOS,
  `std::sync::Barrier` cấp phát lười một lần cho mỗi barrier ở lần `wait()` đầu — hai barrier, hai
  lần cấp phát — cộng một `Box` cố ý là **3**. Trên Windows (`SRWLOCK` + `CONDITION_VARIABLE`) cả
  hai lần `wait()` không cấp phát, nên CI xanh và lỗi này chưa từng lộ.
- **Điều này KHÔNG nói:** nó không nói hot loop cấp phát. `ThreadTrackingAllocator` và các gate
  `allocs == 0` của tick system không liên quan; đây là **self-test của dụng cụ đo** tự làm nhiễu
  cửa sổ của chính nó.
- **Vì sao vẫn phải ghi:** trên macOS nó làm suite đỏ, và một người gặp suite đỏ ở đúng file này
  rất dễ "sửa" bằng cách nới assert thành `<= 3` hoặc `>= 1` — đúng kiểu vi phạm R2 mà repo này
  đã gặp năm lần. Ghi ra trước để lần sau không ai nới.
- **Tái hiện:** `cd src-tauri && cargo test --features desktop -p anima-engine --test thread_tracking_allocator_tests`
  trên macOS → `0 passed; 1 failed`.
- **Sửa đúng cách (thuộc B2, không thuộc B1b):** đưa đồng bộ ra **ngoài** cửa sổ đo — hoặc hâm nóng
  cả hai barrier bằng một cặp `wait()` bỏ đi trước `start_tracking()`, rồi mới đo. Tuyệt đối không
  nới assert: con số 1 chính là điều test tồn tại để khẳng định.
- **Trạng thái:** FIXED (`c8afb3f`), ghi nhận 2026-11-17: test nay chạy một vòng khô trên cả hai
  barrier **trước** `start_tracking()`, và assert vẫn là `== 1`, không bị nới. Dòng gốc: OPEN — agent
  B1b gặp, báo ở SURPRISES và **không** sửa (đúng R2). Đây là đầu vào trực tiếp cho B2 (allocation gate).

### F-030 — Gate `SimRng` mới chỉ thấy system **khai** `ResMut<SimRng>`; hai `Command` rút cùng stream qua `&mut World` thì nó không thấy
- **Ngày:** 2026-10-31
- **Loại:** TEST-GAP
- **Vị trí:** `src-tauri/src/core/agent_systems.rs:182`, `:246` (`SpawnGenotypeCommand::apply`),
  `:275`, `:333`, `:383` (`SpawnOffspringCommand::apply`); gate ở
  `src-tauri/tests/schedule_ambiguity_tests.rs::no_two_systems_that_draw_simrng_are_left_unordered`
- **Phát hiện bởi:** tay — soát lại biên của gate B1b sau khi nó xanh
- **Triệu chứng:** gate lọc cặp ambiguity theo một danh sách tên cứng gồm **năm** system, và năm
  cái đó đúng là toàn bộ system **khai** `ResMut<SimRng>` trong signature (đã đếm:
  `grep -rn "ResMut<[a-z_:]*SimRng>" src/` cho 5 signature + 2 dòng chú thích). Nhưng
  `SpawnGenotypeCommand::apply` và `SpawnOffspringCommand::apply` rút từ **cùng stream** bằng
  `world.get_resource_mut::<SimRng>()`, và `Command` không có signature cho Bevy đọc. Bộ phát hiện
  ambiguity của Bevy không thấy lần rút đó, nên gate cũng không.
- **Hôm nay vẫn đúng, vì sao:** `autonomous_reproduction_system` xếp trong một `.chain()` có
  `apply_deferred` ngay sau nó (`core/simulation_schedule.rs:203`), nên command được flush ở một
  điểm **đã có thứ tự**. Lần rút xảy ra đúng một chỗ xác định.
- **Vì sao vẫn là lỗ:** nếu một system thứ hai cũng xếp `SpawnOffspringCommand` (hoặc một command
  mới rút `SimRng`) và cả hai flush ở **cùng** `apply_deferred`, thứ tự rút khi đó do thứ tự hàng
  đợi command quyết định, tức do thứ tự chạy của hai system xếp chúng — và hai system đó có thể
  **không** khai `ResMut<SimRng>` nên gate cho qua. Đây đúng là hình dạng của F-029, chỉ khác là
  lần này ẩn sau một `Command`.
- **Tái hiện:** `grep -rn "get_resource_mut::<crate::core::resources::SimRng>" src-tauri/src/` —
  5 chỗ, cả 5 trong `impl Command`, không chỗ nào nằm trong `SIMRNG_SYSTEMS`.
- **Test khoá lại cần có:** một source-scan gate (dạng như scan `thread_rng()` đã có) khẳng định
  mọi chỗ chạm `SimRng` qua `&mut World` đều nằm trong một `impl Command` được flush ở
  `apply_deferred` đã khai thứ tự — hoặc rẻ hơn: khẳng định tập `impl Command` chạm `SimRng`
  **không tăng** so với danh sách đã kiểm.
- **Trạng thái:** OPEN — gate B1b vẫn là bước tiến thật (nó bắt đúng 6 cặp của F-029); đây là biên
  của nó, phải nói ra chứ không được để người đọc sau tưởng gate phủ mọi lần rút.
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** `src-tauri/tests/schedule_ambiguity_tests.rs:49-55` vẫn chỉ liệt kê `SIMRNG_SYSTEMS: [&str; 5]` (chưa có source-scan gate cho `Command` rút `SimRng` qua `&mut World`); giữ nguyên `OPEN`.

### F-029 — Thứ tự rút `SimRng` của thế giới sống phụ thuộc tiến trình: `1d301c3` thêm system thứ năm không có cạnh thứ tự, và gate ambiguity chỉ canh năng lượng
- **Ngày:** 2026-10-31
- **Loại:** BUG (vi phạm hợp đồng tất định)
- **Vị trí:** `src-tauri/src/core/agent_systems.rs:441` (`autonomous_reproduction_system`),
  `src-tauri/src/core/world_systems.rs:190`, `:761`, `environmental_systems.rs:217`,
  `agent_systems.rs:677`; lịch dựng ở `core/simulation_schedule.rs`; gate ở
  `src-tauri/tests/schedule_ambiguity_tests.rs`
- **Phát hiện bởi:** agent làm task B1, trong lúc truy nguyên hai gate tái lập đỏ (báo ở SURPRISES)
- **Triệu chứng:** **năm** system nhận `ResMut<SimRng>` trên `origin/main`, và `1d301c3` thêm
  `autonomous_reproduction_system` **không** khai cạnh thứ tự nào với `spawn_food_system` /
  `seed_dropping_system` — những cái cũng rút cùng stream ở tick 1. Bevy topological-sort các
  system xung đột mà không có cạnh theo một thứ tự phụ thuộc hash seed **của từng tiến trình**.
  Hệ quả đo được: cùng seed, cùng manifest, con non `live-founder-0005.1` sinh ra ở `x = 75.77`
  hoặc `x = 71.47` tuỳ tiến trình.
- **Sửa lại con số (2026-10-31, đo khi khoá F-029):** mô tả ban đầu ở trên nói "khoảng một nửa
  tiến trình chạy thứ tự này, nửa còn lại chạy thứ tự kia" — **sai**. Sáu cặp không có thứ tự cho
  phép **nhiều** thứ tự sort, không phải hai. Đo trực tiếp bằng cách chạy
  `a_fork_from_a_known_tick_shares_its_prefix_and_diverges_only_afterwards` (test này **đã in sẵn**
  checksum 180 tick) trong từng tiến trình riêng, trên lịch chưa sửa: **tám tiến trình cho bốn thế
  giới khác nhau** — `3667697632`, `3611111189`, `1299407149`, `67815791`. Sau khi khai chuỗi thứ
  tự: sáu tiến trình cho `3611111189` cả sáu lần. Bài học lặp lại: hai mẫu không đủ để gọi một
  phân bố là nhị thức.
- **Vì sao nó lọt:** chú thích của chính `SimRng` (`core/resources.rs:347`) phát biểu bất biến
  **có điều kiện** và điều kiện đó đã bị phá: *"same seed plus **same tick order** yields the same
  run"*, và *"Systems take `ResMut<SimRng>`, which makes Bevy serialise them against one another."*
  Serialisation chặn hai system **rút song song** — nó **không** quyết định thứ tự giữa chúng. Thứ
  tự phải do `build_tick_schedule` khai tường minh, và `1d301c3` không khai.
- **Vì sao gate không đỏ:** `tests/schedule_ambiguity_tests.rs` có đúng hai test —
  `report_every_ambiguous_pair_in_the_tick_schedule` chỉ **in ra**, không assert; và
  `no_two_systems_that_move_energy_are_left_unordered` chỉ assert về system **chuyển EU**. Phạm vi
  đó là một quyết định có chủ ý, ghi trong module doc: *"An ambiguity is only a bug if the two
  orders give different answers. Most do not."* Đúng — nhưng system rút RNG là lớp **còn lại** mà
  thứ tự luôn có nghĩa, và không gì canh nó.
- **Quan hệ với bản sửa B1:** bản sửa B1 (populate `SpatialHashGrid` khi restore) làm
  `from_snapshot` khớp `uninterrupted` dưới **cả hai** thứ tự, nên hai test xanh 10/10 lượt. Nó
  **không** sửa F-029: hai thứ tự vẫn cho hai thế giới khác nhau. Bộ test không thấy vì mỗi tiến
  trình test tự nhất quán bên trong nó.
- **Tái hiện:** chạy `live_experiment_tests` nhiều tiến trình và in vị trí `live-founder-0005.1` ở
  tick 1; hoặc đọc `build_tick_schedule` và tìm cặp system cùng nhận `ResMut<SimRng>` mà không có
  `.before()`/`.after()` giữa chúng.
- **Test khoá lại cần có:** mở rộng `schedule_ambiguity_tests.rs` thêm một test đối xứng với test
  năng lượng — *không hai system nào rút `SimRng` được để không có thứ tự*. Đây là gate rẻ và nó
  bắt đúng lớp lỗi này ở lần sau.
- **Trạng thái:** FIXED trên nhánh `fix/declare-simrng-draw-order` (commit `03b331f`, xếp trên
  PR #38). `build_tick_schedule` khai chuỗi
  `manual_migration → spawn_food → seed_dropping → autonomous_reproduction → check_epoch_completion`,
  và `schedule_ambiguity_tests.rs::no_two_systems_that_draw_simrng_are_left_unordered` canh lại.
  Checksum 180 tick đổi — đúng như R8 dự liệu — và không test nào ghim nó thành hằng số (đã soát:
  mọi so sánh trong `tests/` là run-vs-run, không có literal 9–10 chữ số nào). Biên của gate đó ghi
  ở F-030.

### F-028 — `origin/main` không compile được trên macOS: 4/5 icon là file text `placeholder`
- **Ngày:** 2026-10-31
- **Loại:** BUG (chặn build trên một nền tảng)
- **Vị trí:** `src-tauri/icons/` trên `origin/main`; panic ở `src-tauri/src/lib.rs:194`
- **Phát hiện bởi:** dựng worktree tại `origin/main` để tái hiện hai gate tái lập đỏ (task B1)
- **Triệu chứng:**
  ```
  error: proc macro panicked
     --> src/lib.rs:194:16
      |  .build(tauri::generate_context!())
      = help: message: failed to read icon .../src-tauri/icons/32x32.png: Invalid PNG signature.
  error: could not compile `anima-engine` (lib) due to 1 previous error
  ```
  Bốn trong năm icon trên `origin/main` là **file text 12 byte chứa đúng chữ `placeholder`**
  (`32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`); `icon.ico` là 72 byte `\x01` lặp.
  Không liên quan Git LFS — `.gitattributes` có **0** dòng `filter=lfs`.
- **Vì sao không ai thấy:** CI chạy job Rust **chỉ trên `windows-latest`**, và ở đó
  `generate_context!` không chết vì cùng mấy file đó. Lượt CI `39644804767` trên `main` compile
  được và chạy test (15 pass, 2 fail), nên nhìn từ CI thì build hoàn toàn bình thường.
- **Không phải hồi quy của đợt 22/10:** merge-base `043930c` (2026-08-08) **đã** là placeholder, và
  `git log 043930c..origin/main -- src-tauri/icons` **trống**. Tức `main` chưa từng build được trên
  macOS kể từ ít nhất 08-08. Nhánh `feat/ecological-selection` build được chỉ vì `fddbdee`
  (2026-08-10, *"commit toàn bộ công việc dở dang"*) tình cờ đưa icon thật vào.
- **Hệ quả thực tế:** không thể bisect hay chạy test `main` trên máy macOS/Linux mà không cấp icon
  hợp lệ trước. Đây là tiền đề của task B1.
- **Tái hiện:**
  ```
  git worktree add --detach /tmp/wt-main origin/main
  cd /tmp/wt-main/src-tauri && cargo test --features desktop --test live_experiment_tests
  ```
- **Test khoá lại:** chưa có. Gate đúng là một job CI trên `ubuntu-latest` hoặc `macos-latest` chỉ
  cần `cargo check --features desktop` — rẻ, và nó bắt đúng lớp lỗi "chỉ đỏ ở nền tảng không ai
  chạy CI". Đây là lần thứ **tư** trong một phiên gặp hình dạng đó (xem F-025, F-026, F-027).
- **Trạng thái:** FIXED, ghi nhận 2026-11-17. Chủ dự án quyết QĐ-F028 ngày 2026-11-14: commit icon
  thật. `main` mang icon PNG thật từ lần fast-forward lên thân cây ngày 2026-11-16
  (`file src-tauri/icons/32x32.png` → `PNG image data, 32 x 32`). Job `determinism` của workflow
  `hardening` biên dịch crate trên `ubuntu-latest`, nên lớp lỗi "chỉ đỏ ngoài Windows" nay có một job
  nhìn thấy. Dòng gốc: OPEN — cần quyết định: icon thật là tài sản của dự án (thì commit chúng) hay bị
  cố ý lược khỏi bản nguồn mở (thì `generate_context!` phải chịu được placeholder, hoặc build phải
  khai rõ yêu cầu). Chủ dự án quyết.

### F-026 — `justfile:151` gọi `verify_dataflow.py --count`, một cờ không tồn tại, nên dòng tổng kết luôn in bản dự phòng
- **Ngày:** 2026-10-31
- **Loại:** SMELL
- **Vị trí:** [`justfile:151`](justfile), [`scripts/verify_dataflow.py`](scripts/verify_dataflow.py)
- **Phát hiện bởi:** agent làm task S0.2
- **Triệu chứng:** recipe `dataflow-truth` kết thúc bằng
  `python3 scripts/verify_dataflow.py --count dataflow-truth.dot 2>/dev/null || echo '(chay dataflow-verify de xem)'`,
  nhưng script **không có cờ `--count`** (chuỗi "count" xuất hiện **0** lần trong file) và nó coi
  `sys.argv[1]` là đường dẫn file. Nên nhánh `||` luôn chạy, và dòng tổng kết **luôn** in bản dự
  phòng thay vì con số.
- **Tái hiện:** `grep -c count scripts/verify_dataflow.py` → `0`; `just dataflow-truth` → luôn in
  `canh 'uses' muc module: (chay dataflow-verify de xem)`
- **Vì sao đáng ghi:** đây là chỗ **thứ ba** trong đúng một recipe mười dòng mà một lệnh thất bại
  lặng lẽ: producer thiếu `--package` (sửa ở S0.2), gate chưa bao giờ vào CI (sửa ở S0.2), và dòng
  tổng kết này. `2>/dev/null ||` biến mọi thất bại thành một chuỗi trông bình thường — cùng hình
  dạng với F-025 và với lỗi `webpki-roots` của phiên này: **một lệnh fail và không in gì không phải
  một kết quả âm.**
- **Test khoá lại:** chưa có. Sửa là một trong hai: cài cờ `--count` vào script, hoặc bỏ dòng echo
  (gate `dataflow-verify` đã in con số). Không nên giữ nguyên.
- **Trạng thái:** OPEN — nhỏ, chưa chặn gì, nhưng đừng để nó thành chỗ thứ tư.
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** PR `#104` đã merge (`e490bfa4` / `b8d1fa7`, 2026-11-17T08:56:19Z): `scripts/verify_dataflow.py:66-86` đã cài `count_truth` / `--count` và `justfile` không còn `2>/dev/null || echo '(chay dataflow-verify de xem)'`; giữ nguyên `OPEN` do chưa có unit test riêng kiểm tra cờ `--count`.

### F-027 — `cargo modules` chỉ sinh cạnh `uses` khi có item tham chiếu symbol, không phải khi có `use`
- **Ngày:** 2026-10-31
- **Loại:** SMELL (hành vi công cụ — cần biết trước khi tin một phép đo đồ thị)
- **Vị trí:** [`scripts/verify_dataflow.py`](scripts/verify_dataflow.py), [`dataflow.json`](dataflow.json)
- **Phát hiện bởi:** negative control của task S0.2
- **Triệu chứng:** thêm `use crate::physics::spatial as _;` vào một file **không** phải `cfg(test)`
  dưới `src-tauri/src/evolution/` **không** tạo ra cạnh `evolution -> physics` nào. Phải có một
  item thật tham chiếu symbol — ví dụ `pub type _NegativeControl = crate::physics::SpatialCollider;`
  — thì cạnh mới xuất hiện và gate mới đỏ đúng (`FAIL: 1 edge BO SOT: evolution -> physics`).
- **Hệ quả:** một negative control viết bằng `use` trần sẽ **không** chứng minh được gate còn sống,
  và người chạy dễ kết luận sai rằng gate đã chết. Bản ghi này tồn tại để lần sau không mất một
  lượt vào đó. Nó cũng nói rằng `dataflow.json` mô tả **tham chiếu symbol thật**, không phải danh
  sách `use` — đúng thứ đáng khai trong một hợp đồng phụ thuộc.
- **Tái hiện:** xem báo cáo S0.2 — có cả lượt đỏ và lượt xanh sau khi revert.
- **Trạng thái:** CLOSED-INFORMATIONAL — không phải lỗi, là tính chất công cụ, ghi để khỏi đo sai.

### F-025 — `cargo tree --target X` không độc lập host: proc-macro được resolve theo host, nên artifact licence không sinh được từ máy khác
- **Ngày:** 2026-10-31
- **Loại:** SMELL (hành vi cargo, không phải lỗi dự án)
- **Vị trí:** `scripts/lib/licensing.mjs:195` (`cargo tree`), `:207` (`cargo metadata`), guard `:243`
- **Phát hiện bởi:** thí nghiệm A7′ — thử sinh artifact licence Windows từ máy macOS
- **Triệu chứng:** ghim cả hai lệnh vào `x86_64-pc-windows-msvc` (`cargo tree --target`,
  `cargo metadata --filter-platform`) làm `npm run gen:compliance` **exit 1** ở guard có sẵn:
  *"cargo tree lists base64@0.21.7 but cargo metadata does not resolve it. The two commands
  disagree about the graph; do not publish an inventory from this state."*
- **Nguyên nhân:** `cargo tree -e normal --target <X>` resolve dependency thường theo target X,
  **nhưng resolve crate proc-macro và toàn bộ cây con của chúng theo HOST**, vì proc-macro chạy
  trong tiến trình compiler trên host. `tauri-utils` khai `swift-rs` dưới
  `[target.'cfg(target_os = "macos")'.dependencies]`; đi qua proc-macro `tauri-macros`, cargo đánh
  giá `cfg(target_os = "macos")` theo **host macOS** → true → kéo `swift-rs@1.0.7` và
  `base64@0.21.7` vào. `cargo metadata --filter-platform` thì lọc mọi node theo **một** platform và
  bỏ qua chuyện host-vs-target của proc-macro, nên loại cả hai. Hai lệnh bất đồng theo thiết kế.
- **Tái hiện:**
  ```
  cd src-tauri
  cargo tree -i base64@0.21.7 --features desktop --target x86_64-pc-windows-msvc -e normal
      -> base64 v0.21.7 <- swift-rs v1.0.7 <- tauri-utils <- tauri-macros (proc-macro) <- tauri
  cargo metadata --format-version 1 --features desktop --filter-platform x86_64-pc-windows-msvc
      -> không có swift-rs; chỉ có base64 0.22.1
  ```
- **Hệ quả tốt — con số 419 nay được chứng minh, không còn là "số đã commit":** đếm tập crate của
  `cargo tree --target x86_64-pc-windows-msvc` trên macOS cho **423**, và nó phân tách đúng:
  **2** crate workspace (`anima-engine`, `anima-domain`) + **2** crate rò theo host
  (`swift-rs@1.0.7`, `base64@0.21.7`) + **419** third-party = 423. Hiệu với index đã commit **chỉ**
  là ba cặp đổi phiên bản TLS (`rustls` 0.23.42→0.23.46, `rustls-webpki` 0.103.13→0.103.16,
  `webpki-roots` 1.0.9→1.0.11), tức **A6 không thêm crate nào**.
- **Vì sao không sửa bằng `-e no-proc-macro`:** cờ đó tồn tại và sẽ làm hai lệnh khớp lại, nhưng nó
  loại crate proc-macro khỏi inventory — mà `tauri-macros`, `syn`, `quote`, `proc-macro2`,
  `serde_derive`, `thiserror-impl` **đang có trong** `third-party-index.json`. Bỏ chúng là đổi điều
  tài liệu tuân thủ khẳng định về phạm vi, tức **quyết định pháp lý**, không phải sửa lỗi kỹ thuật.
- **Đường thứ ba cũng đã đóng (đo 2026-10-31):** bỏ `cargo tree`, dùng
  `cargo metadata --filter-platform` làm nguồn duy nhất, cũng **không** được —
  `metadata` lọc được **nền tảng** nhưng **không lọc feature**. Đi từ root theo dep bình thường
  trên metadata đã filter cho **530** third-party thay vì 419; 114 crate dư toàn là optional
  dependency không bật (`burn-candle`, `burn-dataset`, `burn-tch`, `burn-train`, `aes`, `bs58`…).
  Đây đúng điều `.cargo/audit.toml` đã ghi về lockfile feature-agnostic, và cũng là lý do
  `zip`/`zstd` hiện trong `cargo audit` mà không có trong `cargo tree`.
  **Kết luận: hai lệnh bù nhau theo bắt buộc** — `tree` lọc feature, `metadata` lọc nền tảng —
  nên muốn cả hai đúng cho một nền tảng không phải host thì phải chạy trên nền tảng đó.
  (Có thể hình dung một allow-list hai crate để bỏ qua phần rò proc-macro, nhưng nó phải được bảo
  trì mỗi lần đồ thị đổi, và nó là dạng vá cho gate xanh mà R3 cấm về tinh thần. Không đề xuất.)
- **Test khoá lại:** guard `licensing.mjs:243` đã là test khoá — nó bắt đúng chế độ hỏng này và
  **chặn** việc publish một inventory sai. Đây là lần nó chứng minh giá trị.
- **Trạng thái:** CLOSED-WONTFIX — artifact licence **phải** sinh trên Windows (task A7). Bản thử
  A7′ còn trong `git stash` (`stash@{0}`) nếu cần đọc lại.

### F-023 — `adversarial_coverage_tests` đỏ flaky: counter toàn tiến trình bị test anh em nhiễu
- **Ngày:** 2026-08-26
- **Loại:** BUG
- **Vị trí:** `src-tauri/tests/adversarial_coverage_tests.rs:138,172,276` (điểm mở cửa sổ đo); gốc rễ:
  `src-tauri/tests/common/allocator.rs` — `TrackingAllocator::start_tracking()` đếm **toàn tiến trình**
- **Phát hiện bởi:** determinism (full `just test`, chạy release không feature)
- **Triệu chứng:** `test_spatial_grid_rebuild_allocates_on_new_cell` failed 1 lần trong full suite với
  `Expected zero allocations on out of bounds insert — left: 4, right: 0`; chạy riêng file đó thì xanh
  7/7 ở CẢ cấu hình desktop lẫn trần. Nguyên nhân đúng như doc comment của `allocator.rs` đã cảnh báo:
  libtest cho mỗi `#[test]` một thread, `TEST_LOCK` chỉ tuần tự hoá thân một phần test, và counter toàn
  tiến trình gán mọi allocation trong cửa sổ đo — kể cả của thread test khác đang chạy song song.
  Đây là một trong các suite "latent" mà tài liệu nêu tên: đường đo đơn-thread nhưng vẫn dùng counter
  rộng. Cả ba đường được đo (`rebuild_spatial_grid_system` ×2, `combat_system`) đều không nhả worker
  thread (không rayon/spawn trong `physics/spatial.rs`), nên chế độ per-thread là đúng policy, không
  phải nới lỏng — assertion `allocs == 0` giữ nguyên.
- **Tái hiện:** lặp `cd src-tauri && cargo test --release` đủ nhiều lần trên máy đang load → fail rải rác
  ở một trong ba test đo allocation của file này; `cargo test --release --test adversarial_coverage_tests`
  đứng riêng thì xanh ổn định.
- **Test khoá lại:** `src-tauri/tests/adversarial_coverage_tests.rs` — ba cửa sổ đo chuyển sang
  `ALLOCATOR.start_tracking_this_thread()`; assertion giữ nguyên.
- **Trạng thái:** FIXED (commit kèm theo trên `feat/ecological-selection`)

### F-024 — Cổng `verify` tự hủy im lặng khi diff không chứa file test nào
- **Ngày:** 2026-08-26
- **Loại:** BUG
- **Vị trí:** `scripts/verify.sh:41`
- **Phát hiện bởi:** tay (`bash -x ./scripts/verify.sh`, trace dừng đúng tại dòng 41, EXIT=1, không một byte output)
- **Triệu chứng:** khi cây làm việc không có thay đổi nào dưới `src-tauri/**/tests/` (ví dụ: kiểm tra lại một trạng thái ĐÃ commit, hoặc diff chỉ chạm tài liệu/nguồn), `grep -E "$GUARD_RE"` trong pipeline của `GUARD_DIRS` không khớp dòng nào → exit 1. Với `set -o pipefail` + `set -e` và pipeline KHÔNG có `|| true` (khác mọi pipeline khác trong cùng script), phép gán thất bại và script thoát ngay với mã 1, **trước cả luật 2**, không in `TU CHỐI` và cũng không bao giờ tới được `QUA CONG`. Hệ quả: cổng chống gian lận không thể phán "đạt" cho bất kỳ công việc nào mà diff của nó không kèm file test mới — kể cả các việc hợp lệ như sửa tài liệu hay commit lại file nguồn bị thiếu.
- **Tái hiện:** commit sạch toàn bộ thay đổi (hoặc đứng ở HEAD sau 893098b) rồi chạy `just verify` → exit 1, stdout/stderr rỗng. So sánh: trước khi commit, cùng lệnh in `TU CHỐI: ...` bình thường vì law 1 chặn sớm trước dòng 41.
- **Đề xuất sửa (cho chủ dự án — `scripts/` nằm ngoài vùng ghi của agent):** thêm `|| true` cho phép gán ở dòng 41, giống cách đã làm với `CHANGED`/`BAD`/`NONKIT`; hoặc đổi thành `grep ... | sed ... | sort -u || true`. Không thay đổi ngữ nghĩa luật nào.
- **Trạng thái:** OPEN
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** PR `#104` đã merge (`e490bfa4` / `b8d1fa7`, 2026-11-17T08:56:19Z): `scripts/verify.sh:43` đã thêm `|| true` với chú thích `(F-024)` ở `:41-42`; giữ nguyên `OPEN` do chưa có unit test tự động cho `scripts/verify.sh`.

### F-018 — Sinh thái không nối vào chọn lọc: cả crate có đúng một fitness và nó là quãng đường
- **Ngày:** 2026-08-25
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/core/agent_systems.rs:528` (định nghĩa fitness duy nhất),
  `src-tauri/src/core/world_systems.rs:171` (`tick_count` tăng vô điều kiện)
- **Phát hiện bởi:** tay (khảo sát toàn bộ mã nguồn, lần theo dữ liệu thay vì theo tài liệu)
- **Triệu chứng:** `fitness = cumulative_distance + tick_count`, và `tick_count` tăng mỗi tick cho
  mọi agent có `FeatureTracker` bất kể trạng thái sinh lý. Sau một epoch, `tick_count` gần như bằng
  nhau ở mọi cá thể, nên xếp hạng fitness rút gọn thành xếp hạng quãng đường. Toàn bộ tầng sinh
  thái — NPP theo biome, Kleiber, Boltzmann, Holling II/III, Lindeman, mùa vụ — **không có đường
  nào** chạm tới hàm này. Một cá thể ăn no và một cá thể kiệt sức từ tick 300 có cơ hội truyền gene
  bằng nhau nếu đi được quãng đường như nhau.
- **Tái hiện:** `grep -rn 'fitness =' src-tauri/src --include='*.rs'` → đúng 1 kết quả gán.
- **Test khoá lại:** chưa có. Gate đề xuất: **CA-S01** ở
  `docs/ai/testing/2026-08-25-feature-ecological-selection.md` — đổi một tham số sinh thái phải đổi
  phân bố tính trạng, và cùng test đó phải **không** đổi trên chế độ fitness cũ.
- **Trạng thái:** ĐÃ CÀI ĐẶT, OPT-IN (ghi nhận 2026-11-17). `evolution/fitness.rs` (`26a11f6`) có
  `EcologicalFitness`, bật bằng khoá `live.ecological_fitness`; gate FI-S01…FI-S04 và CA-S01 ở
  `tests/fitness_tests.rs`. `FitnessRuleSetting::default()` vẫn là `Locomotion`, nên một run không
  khai khoá vẫn chấm bằng quãng đường như mô tả trên. CA-S01 tự khai phạm vi hẹp hơn dạng đầy đủ ở
  đầu file test. Dòng gốc: OPEN — feature `ecological-selection` nhánh F.

### F-019 — Không có cái chết: `energy <= 0` chỉ `continue`, nên không có áp lực chọn lọc nào
- **Ngày:** 2026-08-25
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/core/agent_systems.rs:452`
- **Phát hiện bởi:** tay (cùng lượt khảo sát với F-018)
- **Triệu chứng:** cạn năng lượng hoặc cạn nước không kết thúc gì — hệ đánh giá chỉ `continue`, agent
  vẫn đi, vẫn trao đổi chất, vẫn được thay thế đúng lịch ở cuối epoch. Không tử vong nghĩa là không
  có sức chứa môi trường thật, không bùng–sụp, không Red Queen, không phân hoá r/K. Đáng chú ý:
  cơ chế trả EU của xác về `detritus` (`ReclaimAndDespawnAgentCommand`,
  `src-tauri/src/core/agent_systems.rs:112-152`) **đã viết xong, nguyên tử, và đã có gate** — thứ
  thiếu chỉ là điều kiện gọi nó.
- **Tái hiện:** `grep -rn 'energy <= 0' src-tauri/src --include='*.rs'`
- **Test khoá lại:** chưa có. Gate đề xuất: **CA-S02** — NPP = 0 phải làm quần thể sập về 0, và với
  tử vong tắt thì **không** sập.
- **Trạng thái:** ĐÃ CÀI ĐẶT, OPT-IN (ghi nhận 2026-11-17). `core/mortality.rs` (`a1c3885`) với
  `MortalityPolicy`, bật bằng khoá `live.mortality`; gate CA-S02 kèm đối chứng âm ở
  `tests/mortality_tests.rs`. `MortalityPolicy::default()` vẫn là tắt, nên thế giới mặc định vẫn
  không có cái chết. Các lỗi sau đó của chính cơ chế này: F-037, F-044. Dòng gốc: OPEN — feature
  `ecological-selection` nhánh M.
- **Ghi chú ranh giới:** invariant `"starvation must not kill"` tại
  `src-tauri/src/core/aggregate_population.rs:1653` nói về quần thể tổng hợp AE3, một mô hình khác
  có hợp đồng khác. Nó **không** phải mục tiêu sửa của finding này.

### F-020 — `AgentEvaluation` tính đúng đại lượng cần cho fitness sinh thái rồi bị vứt đi
- **Ngày:** 2026-08-25
- **Loại:** SMELL
- **Vị trí:** `src-tauri/src/core/agent_systems.rs:523` (bind `_eval, _homeo`),
  `:452-462` (`survival_ticks` có điều kiện sống)
- **Phát hiện bởi:** tay
- **Triệu chứng:** `AgentEvaluation.survival_ticks` chỉ tăng khi `energy > 0 && hydration > 0` — tức
  đúng nghĩa "số tick sống được", đại lượng mà một fitness sinh thái cần, và nó đã được tính mỗi
  tick từ trước. Nhưng `check_epoch_completion_system` bind nó thành `_eval` rồi đọc `FeatureTracker`
  thay vì nó. Đây là lý do F-018 rẻ để sửa: phần khó đã tồn tại, chỉ là không ai đọc.
- **Tái hiện:** đọc `src-tauri/src/core/agent_systems.rs:523` và `:452`.
- **Test khoá lại:** chưa có. Gate đề xuất: **FI-S02**.
- **Trạng thái:** FIXED cho luật sinh thái (ghi nhận 2026-11-17): `EcologicalFitness` đọc
  `eval.survival_ticks` (task F-2, F-3; gate FI-S02). Luật `Locomotion` mặc định cố ý không đọc nó.
  Dòng gốc: OPEN — feature `ecological-selection` nhánh F, task F-3.

### F-021 — Trục MAP-Elites thứ hai bằng đúng hàm fitness trừ một hằng số
- **Ngày:** 2026-08-25
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/core/agent_systems.rs:531` (`foraging_range = cumulative_distance`) so với
  `:528` (`fitness = cumulative_distance + tick_count`); mô tả trục tại
  `src-tauri/src/core/ecology.rs:161-174`
- **Phát hiện bởi:** tay
- **Triệu chứng:** MAP-Elites hoạt động được nhờ mô tả hành vi **độc lập** với fitness; đó là cách nó
  soi sáng một không gian thay vì leo một đỉnh. Ở đây trục 2 là fitness trừ một hằng số, nên kho
  thực chất chỉ khám phá **một** chiều (khối lượng cơ thể). Kho không hỏng — nó làm ít hơn một nửa
  việc mà doc comment tại `ecology.rs:163-168` mô tả ("rewards ECOLOGICAL diversity … instead of a
  single locomotion optimum"). Chưa đối chứng: `FORAGING_REFERENCE = 200.0` có `clamp`, nên trục còn
  có thể bão hoà — chưa đo.
- **Tái hiện:** `sed -n '526,532p' src-tauri/src/core/agent_systems.rs`
- **Test khoá lại:** chưa có. Gate đề xuất: **QD-S02** — tương quan hạng giữa trục 2 và fitness phải
  **không** đơn điệu trên một quần thể fixture.
- **Trạng thái:** FIXED (ghi nhận 2026-11-17): trục 2 của `ecological_descriptors` nay là
  `trophic_ratio` (task Q-3), kho lưu mang `descriptor_set_id` và từ chối bộ mô tả lệch (Q-4); gate
  QD-S01…QD-S03 ở `tests/qd_tests.rs`. `foraging_range` còn trong telemetry nhưng không còn là trục.
  Dòng gốc: OPEN — feature `ecological-selection` nhánh Q. Sửa F-018 trước; nếu công thức
  fitness mới không chứa quãng đường thì finding này đóng theo.

### F-022 — Bảy hệ thống chạy O(agent × segment) trong khi mẫu sửa đúng đã có sẵn trong cùng file
- **Ngày:** 2026-08-25
- **Loại:** SMELL
- **Vị trí:** `src-tauri/src/core/world_systems.rs:121, 236, 248, 337, 436, 454`,
  `src-tauri/src/core/agent_systems.rs:468, 877, 890`,
  `src-tauri/src/core/environmental_systems.rs:282`
- **Phát hiện bởi:** tay
- **Triệu chứng:** các hệ thống lặp **toàn bộ** segment trong thế giới ở **bên trong** vòng lặp agent,
  lọc bằng `if parent.0 == agent_entity`. Với 10 agent (mặc định) điều đó vô hại; với 1000 agent × 3
  segment, riêng `metabolic_decay_system` là 3 triệu vòng mỗi tick. `sensory_system` còn quét tuyến
  tính toàn bộ danh sách thức ăn và mồi cho **từng** agent, trong khi `SpatialHashGrid` đã được dựng
  lại mỗi tick và đang được dùng cho raycast **ngay bên dưới trong cùng hàm**. Mẫu sửa đúng đã có
  trong repo: `combat_system` (`world_systems.rs:300-340`) dựng `centroid_index: Local<EntityHashMap>`
  một lần rồi quét segment đúng một lượt — O(A + S), không cấp phát trong tick path.
- **Tái hiện:** `grep -rn 'segment_query.iter()' src-tauri/src --include='*.rs'`
- **Test khoá lại:** chưa có. Gate đề xuất: **PF-S04** — thời gian mỗi tick tăng **tuyến tính** theo
  số agent, không bậc hai; và **PF-S01** — cùng seed cho cùng checksum trước và sau (refactor thuần).
- **Trạng thái:** FIXED (0b29312) — H-1..H-4 đã xong, refactor thuần, checksum trùng bit
  (`tests/h_probe.rs`, 1500 tick, seed `0x5EED` = 4013327208 trên cả cây trước và sau H).
  Còn lại H-5: đo `full_tick` p50 ở 1000 agent trên **bản release** qua `ANIMA_TICK_CAPTURE` —
  bốn lệnh capture là IPC-only nên cần một lần chạy tay của người, chưa đo được.
- **Bẫy khi sửa:** `EntityHashMap` không duyệt theo thứ tự chèn. Chỗ nào kết quả phụ thuộc thứ tự
  duyệt (ai giành miếng ăn cuối trong một ô) phải sắp xếp tường minh, nếu không checksum sẽ đổi.

### F-017 — `scripts/check_test_targets.mjs` đã commit với ba khối merge conflict còn nguyên, gate xác minh chết ngay khi chạy
- **Ngày:** 2026-08-25
- **Loại:** BUG
- **Vị trí:** `scripts/check_test_targets.mjs:78` (`<<<<<<< ours`), `:103/:108`
  (`=======`, khối hai), `:155/:273` (khối ba) — cả ba khối conflict đều nằm
  trong bản đã commit trên nhánh `feat/living-world`.
- **Phát hiện bởi:** tay (chạy cổng xác minh của living-world sau khi xong M3/F1).
- **Triệu chứng:** script chết ngay khi parse — `node scripts/check_test_targets.mjs`
  ném `SyntaxError: Unexpected token '<<'`. Hệ quả kép: (1) cổng
  "check_test_targets" của vòng lặp xác minh không chạy được chút nào; (2)
  `npm run lint` báo `Parsing error: Merge conflict marker encountered` cho file
  này, góp phần làm rổ eslint gate. Ngoài ra cùng lần lint đó, `playground/` và
  `tests/frontend/` có sẵn 9 lỗi eslint khác (chủ yếu `no-useless-assignment`,
  `@ts-ignore`) cũng đã commit — baseline ratchet 0 không phản ánh cây mã hiện tại.
- **Tái hiện:** `node scripts/check_test_targets.mjs /tmp/anima-test.txt --profile desktop`
  → SyntaxError; `npm run lint` → 82 problems.
- **Test khoá lại:** không áp dụng được — `scripts/` cấm sửa theo AGENTS.md.
- **Cách sửa đề xuất cho owner:** resolve ba khối conflict trong
  `check_test_targets.mjs` (giữ nhánh `theirs` có chú thích PowerShell-wrap vì
  đó là bản mới hơn có đo đạc), rồi chạy lại `npm run lint`; quyết định riêng
  về 9 lỗi eslint tồn đọng ở `playground/` + `tests/frontend/`.
- **Trạng thái:** FIXED (`bef7768`), ghi nhận 2026-11-17: file không còn conflict marker nào và
  gate chạy trong CI (STATE §1.g, hàng "Chính sách target"). Dòng gốc: OPEN — chờ owner sửa `scripts/`.

### F-014 — Không thể quét mutation cho anima-core: crate nằm trong workspace cha nhưng không thuộc members/exclude
- **Ngày:** 2026-08-24
- **Loại:** BUG
- **Vị trí:** `src-tauri/anima-core/Cargo.toml` (thiếu bảng `[workspace]`) và/hoặc
  `src-tauri/Cargo.toml:members = ["crates/*"]` (không chứa `anima-core`)
- **Phát hiện bởi:** tay (`just hunt-file src/mutation.rs 2 mutation_operator_tests,mutation_invariants anima-core`)
- **Triệu chứng:** mọi lệnh cargo chạy trong `src-tauri/anima-core` đều chết ngay:
  `cargo locate-project failed: current package believes it's in a workspace
  when it's not`. Hệ quả: không tái tạo được quét mutation cho anima-core —
  `mutants.out/missed.txt` (25 dòng, 22:33) là dữ liệu cũ từ TRƯỚC khi
  `tests/mutation_operator_tests.rs` được thêm; header file test này khẳng định
  nó giết đúng 25 mutant đó nhưng KHÔNG có bằng chứng tươi. Mutation score của
  anima-core hiện không thể đo = số đo nói dối.
- **Tái hiện:** `just hunt-file src/mutation.rs 2 "" anima-core` → error trên.
- **Test khoá lại:** không áp dụng được — cả hai file đều là config build, cấm
  sửa theo AGENTS.md.
- **Cách sửa đề xuất cho owner** (một trong hai):
  (a) thêm bảng `[workspace]` rỗng vào cuối `src-tauri/anima-core/Cargo.toml`
  (tách khỏi workspace cha — đúng tinh thần "crate thuần, build độc lập"), hoặc
  (b) thêm `"anima-core"` vào `workspace.exclude` của `src-tauri/Cargo.toml`.
  Sau khi sửa, chạy lại: `just hunt-file src/mutation.rs 2
  mutation_operator_tests,mutation_invariants anima-core` để có missed.txt tươi.
- **Trạng thái:** OPEN — chờ owner sửa config.
- **FIXED (2026-08-24):** owner chỉ thị "làm nốt" → chọn phương án (a): thêm
  bảng `[workspace]` rỗng (kèm chú thích) vào `src-tauri/anima-core/Cargo.toml`.
  Không ảnh hưởng build crate chính (src-tauri không phụ thuộc anima-core).
  Quét tươi mở khoá — kéo theo phát hiện F-015.
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** bảng `[workspace]` rỗng hiện diện tại `src-tauri/anima-core/Cargo.toml:30` (chú thích F-014 tại `:23-29`, commit `1ec6ac5`, owner duyệt 2026-08-24); giữ nguyên dòng trạng thái gốc do không có PR merge riêng và không có unit test.

### F-015 — anima-core không biên dịch được sau merge 157f85d: import path stale + validator tham chiếu ngược crate cha
- **Ngày:** 2026-08-24
- **Loại:** BUG
- **Vị trí:** `src-tauri/anima-core/src/crossover.rs:1-2`, `src/mutation.rs:1`,
  `src/map_elites.rs:1,82` (path `crate::evolution::…` / `crate::core::…` không
  tồn tại trong crate tách rời), và `tests/evolution_robustness_tests.rs:1-4`,
  `tests/map_elites_tests.rs:1-6` (import nhầm `anima_engine_lib` — crate cha),
  `tests/crossover_logic_tests.rs:45,93,141,219,258` (dùng `child.nodes` trên
  `Result` — viết cho API cũ trả thẳng genotype).
- **Phát hiện bởi:** mutation scan (F-014 vừa mở khoá thì baseline build chết:
  `ERROR cargo build failed in an unmutated tree`).
- **Triệu chứng:** 5 lỗi E0433 ở lib + 13+ lỗi ở test → toàn bộ fuzz/miri/
  mutation của anima-core tê liệt từ merge 157f85d; mutation score "70%" trước
  đó không thể tái tạo. `map_elites.rs:82` gọi
  `crate::core::components::validate_morphology_payload` — hàm này là
  `pub(crate)` ở crate cha, không thể gọi chéo (sẽ đảo dependency).
- **Tái hiện:** `just hunt-file src/mutation.rs 2 "" anima-core` (trước fix) →
  E0433 hàng loạt.
- **Cách sửa (owner ủy quyền qua chỉ thị "làm nốt đi" sau khi F-014 được báo):**
  1. Sửa import path về module top-level của anima-core (`crate::genotype`,
     `crate::mutation`).
  2. Port `validate_morphology_payload` thành
     `anima-core/src/components.rs` (thuần, trả `Result<(), String>`, giữ
     `MAX_MORPHOLOGY_NODES = 128` đồng bộ bản gốc); call site đổi sang
     `crate::components::…`.
  3. Sửa test: thêm `.expect()` cho API `Result` mới; đổi import về
     `anima_core::`; derive `PartialEq` cho 3 struct genotype (additive);
     thêm `[dev-dependencies] serde_json` cho test round-trip JSON.
  4. Viết thêm 6 test khoá 17 mutant sống (xem dưới).
- **Kết quả mutation (quét tươi, chỉ test 2 suite mutation):** 83 mutant của
  `src/mutation.rs`: 76 caught, 7 unviable, **0 missed** — 100% mutant khả thi
  bị giết (test mới: `mutation_operator_tests.rs::op1_giu_nguyen_anchor…`,
  `op2_phat_anchor…`, `op3_khong_them_node…`, `op4_khong_bo_node…`,
  `op3_anchor_z_cua_con…`, `loi_node_id_exhausted…`).
- **Bài học:** khi tách crate, phải chạy `cargo check` trên crate MỚI ngay —
  copy file giữ nguyên path cũ thì mọi cổng phía sau đều đo trên xác chết.
- **BỔ SUNG (2026-08-24) — quét TOÀN crate:** `just hunt-file src 2 '' anima-core`
  (81 mutant): lần đầu 26 missed → viết test khoá lại cho crossover/map_elites/
  components → còn 4 → phân tích thêm → **0 missed**. Chi tiết các test mới:
  `crossover_logic_tests::graft_subtree_giu_do_thi_hop_le_tren_seed_sweep`,
  `map_elites_tests::from_saved_tu_choi_*` (3 test),
  `map_elites_tests::add_individual_khong_thay_the…`,
  `map_elites_tests::select_parent_replay_khop_mo_phong_rut`,
  `components_validation_tests.rs` (4 test). Mutant cuối cùng là equivalent —
  xem F-016.
- **Trạng thái:** FIXED (commit kèm biên bản này).

### F-016 — Mutant "&&"→"||" ở prune cạnh khi ghép subtree là tương đương trên miền input là cây
- **Ngày:** 2026-08-24
- **Loại:** SMELL
- **Vị trí:** `src-tauri/anima-core/src/crossover.rs:121`
  (`child.edges.retain(|e| !v_sub_ids.contains(&e.source_node) && !v_sub_ids.contains(&e.target_node))`)
- **Phát hiện bởi:** mutant (sống sót 3 vòng test sweep 128 seed)
- **Triệu chứng:** thoạt nhìn `&&`→`||` giữ thêm cạnh lơ lửng vào node đã bị
  xoá → phá validity. Nhưng: (1) `child` tại điểm prune luôn là CÂY (xây từ
  hai cây), (2) cạnh incoming vào gốc subtree `v` đã bị `edges.remove()` ở
  dòng 113 TRƯỚC khi retain. Trên cây, cạnh duy nhất cắt qua ranh giới một
  subtree chính là cạnh vào gốc của nó → sau khi bỏ cạnh đó, không còn cạnh
  nào chạm v_sub ngoài các cạnh nội bộ → `!A||!B` và `!A&&!B` chọn cùng tập
  cạnh. Không tồn tại test hành vi phân biệt được trên miền input hiện có.
- **Tái hiện:** `just hunt-file src 2 '' anima-core` → mutant trên báo Missed
  (trước khi skip).
- **Test khoá lại:** không có — không thể có trên miền input là cây.
- **Đã xử lý:** `just skip 'crossover\.rs:121:57: replace && with \|\| in
  crossover_genotypes' …` thêm vào `exclude_re` của
  `src-tauri/anima-core/.cargo/mutants.toml`.
- **Điều kiện gỡ ra:** khi `child` tại điểm prune cho phép DAG/nhiều cạnh vào
  cùng subtree (không còn là cây đơn) — khi đó cross-edge nhiều hơn một và
  mutant PHẢI bị giết bằng test.
- **Trạng thái:** OPEN (đã loại khỏi mutation gate, chờ điều kiện gỡ)
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** điều kiện gỡ chưa đạt — `src-tauri/anima-core/src/crossover.rs:120-122` giữ nguyên biểu thức prune cây và `src-tauri/anima-core/.cargo/mutants.toml:10` còn ngoại lệ; giữ nguyên `OPEN`.



### F-013 — Mutant "replace + with -" ở z_down wrap toroidal KHÔNG tương đương, đã giết bằng test hành vi
- **Ngày:** 2026-08-24
- **Loại:** TEST-GAP
- **Vị trí:** `src-tauri/src/ai/pheromone.rs:196` (`let z_down = (z + 1) % GRID_SIZE;`;
  chuỗi mutant đầy đủ sau merge 157f85d:
  `src/ai/pheromone.rs:196:25: replace + with - in update_pheromone_grid_system`.
  `missed.txt` cũ ghi `197:29` là số dòng TRƯỚC merge — đã lỗi thời.)
- **Phát hiện bởi:** mutant (`src/ai/pheromone.rs:196:25: replace + with - in update_pheromone_grid_system` sống sót trong quét trước merge)
- **Triệu chứng:** khác với mutant tương đương ở dòng 190 (F-011), mutant này
  **phân biệt được**: đổi `(z + 1)` thành `(z - 1)` thì với z = 0, `(0 - 1)`
  usize underflow wrap thành `2^64 - 1 ≡ 127 (mod 128)`, nên ô (z=0) đọc láng
  giềng `down` từ hàng 127 thay vì hàng 1. Không có test hành vi nào đặt
  pheromone lệch để phân biệt hai hướng wrap theo trục z.
- **Tái hiện:** `cd src-tauri && cargo mutants --file src/ai/pheromone.rs --baseline skip`
  → mutant trên báo Missed.
- **Test khoá lại:** `src-tauri/tests/pheromone_tests.rs::test_pheromone_diffusion_z_wraps_forward_not_backward_at_z_zero`
  (đặt 10.0 chỉ tại hàng 1, chạy system 1 tick, assert ô (z=0,x=0) = 1.0;
  mutant cho đúng 0.0).
- **Trạng thái:** FIXED bằng test (chờ `just hunt` xác nhận mutant chết).

### F-011 — Mutant "replace + with -" ở wrap toroidal là tương đương, đã loại bằng skip
- **Ngày:** 2026-08-24
- **Loại:** SMELL
- **Vị trí:** `src-tauri/src/ai/pheromone.rs:195` (`let z_up = (z + GRID_SIZE - 1) % GRID_SIZE;`)
- **Phát hiện bởi:** mutant (`src/ai/pheromone.rs:190:23: replace + with - in update_pheromone_grid_system` sống sót qua quét shard, thấy trong `src-tauri/mutants.out/missed.txt`)
- **Triệu chứng:** với usize và overflow-checks tắt (mặc định profile release,
  thứ mà cargo-mutants dùng), `(z - GRID_SIZE - 1) % GRID_SIZE` cho kết quả
  **giống hệt** `(z + GRID_SIZE - 1) % GRID_SIZE` với mọi z ∈ [0,128): phép trừ
  underflow wrap thành `2^64 - (129 - z)` mà `2^64 ≡ 0 (mod 128)` và
  `-129 ≡ 127 (mod 128)`, nên sau `% GRID_SIZE` hai biểu thức trùng khít. Không
  tồn tại test hành vi nào phân biệt được — mọi giá trị thay thế đều cho cùng
  output lưới.
- **Tái hiện:** `cd src-tauri && cargo mutants --file src/ai/pheromone.rs --baseline skip`
  → mutant trên báo Missed.
- **Test khoá lại:** không có — không thể có, đây là equivalent mutant.
- **Đã xử lý:** `just skip 'pheromone\.rs:190:23: replace \+ with - in update_pheromone_grid_system' ...`
  thêm vào `exclude_re` của `src-tauri/.cargo/mutants.toml`.
- **BỔ SUNG (2026-08-24):** merge 157f85d đổi số dòng, biểu thức `z_up` giờ ở
  `pheromone.rs:195` nên regex cũ ghim `190:23` không còn khớp — mutant tương
  đương sẽ tái xuất hiện dạng Missed ở quét tươi. Đã thêm regex mới bằng lệnh
  chính thức `just skip 'pheromone\.rs:195:23: replace \+ with - in
  update_pheromone_grid_system' ...`. Regex cũ để nguyên trong toml (dead
  pattern, vô hại). Bài học: exclusion ghim theo số dòng dễ vỡ khi src đổi —
  các exclude sau này nên neo theo nội dung thay vì `file:line:col`.
- **Điều kiện gỡ ra:** GRID_SIZE không còn là luỹ thừa của 2; hoặc bật
  overflow-checks trong profile release/test; hoặc đổi sang số có dấu +
  `rem_euclid`. Khi đó mutant không còn tương đương và PHẢI bị giết bằng test.
- **Trạng thái:** OPEN (đã loại khỏi mutation gate, chờ điều kiện gỡ)
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** điều kiện gỡ chưa đạt — `src-tauri/src/ai/pheromone.rs:195` giữ nguyên `(z + GRID_SIZE - 1) % GRID_SIZE` và `src-tauri/.cargo/mutants.toml:20` còn ngoại lệ; giữ nguyên `OPEN`.

### F-012 — Merge 157f85d đồng bộ src theo origin/main nhưng bỏ sót 11+ suite test
- **Ngày:** 2026-08-24
- **Loại:** BUG
- **Vị trí:** 11 file trong `src-tauri/tests/`: flora_tests (39 chỗ dùng API đã
  xoá), flora_stress_boundary_tests (31), challenger_terrain_tests (18),
  milestone5_challenger_stress_tests (17), env_physics_tests (16),
  adversarial_terrain_challenger_tests (10), challenger_m5_settings_tests (10),
  milestone4_tests (9), terrain_tests (5), challenger_stress_tests (3),
  challenger_m6_weather_tests (2). Ngoài ra conflict marker còn nguyên trong
  `src-tauri/tests/common/allocator.rs:21-55` (đã giải trong session này).
- **Phát hiện bởi:** tay (`just test` exit 101)
- **Triệu chứng:** commit merge 157f85d thay toàn bộ `src-tauri/src` bằng phiên
  bản origin/main (xoá Flora/FloraSpawnSettings/spawn/grow/wither_flora_system,
  TerrainGrid → TerrainMap, EnvironmentState bị xoá/đổi tên, core::rng không
  còn được khai báo trong core/mod.rs dù file rng.rs vẫn tồn tại) nhưng KHÔNG
  cập nhật vùng test. Hệ quả: nhiều test binary không biên dịch được
  (`E0432/E0422/E0425/E0433`), `just test` exit 101, cổng verify luật 7 không
  thể qua. 11 file trên KHÔNG tồn tại trên origin/main — chúng là suite mồ côi
  của kiến trúc cũ. 4 file khác (meta_ai_stress_tests, challenger_meta_ai_tests,
  hrrl_tests, ecs_simulation_tests) đã đồng bộ về bản main (diff không xoá
  assertion nào).
- **Tái hiện:** `just test` → exit 101; ví dụ một lỗi:
  `tests/flora_stress_boundary_tests.rs:6` import
  `anima_engine_lib::core::ecs::Flora` → E0432.
- **Test khoá lại:** không áp dụng được — bế tắc cấu trúc: cổng verify cấm xoá
  assertion đã có (luật 3) nên không được xoá/gọt suite mồ côi; đồng thời cấm sửa
  `src-tauri/src` nên không được hồi sinh API cũ. Cần chủ sở hữu kit quyết định
  một trong hai hướng: (a) gỡ bỏ chính thức các suite mồ côi khỏi vùng guard
  kèm biên bản, hoặc (b) chấp nhận revert phần xoá flora/terrain API trong src.
- **Trạng thái:** OPEN — chờ quyết định kiến trúc phía owner.
- **QUYẾT ĐỊNH OWNER (2026-08-24):** chọn phương án (a) — xoá 11 suite mồ côi.
  Ủy quyền cho agent kèm tiêu chí bắt buộc (cả hai phải thoả đồng thời):
  (i) API được test đã biến mất khỏi `src-tauri/src`, và (ii) file không tồn tại
  trên nhánh gốc `origin/main`. Đã đối chiếu: cả 11 file thoả cả hai tiêu chí
  (`git cat-file -e origin/main:<path>` → KHÔNG tồn tại; số chỗ dùng API đã đếm
  ở trên). Việc xoá nằm trong commit riêng kèm mục FINDINGS này làm biên bản;
  ngoài 11 file này không được xoá/gọt bất kỳ assertion nào khác.
- **BỔ SUNG (2026-08-24):** quét mở rộng theo symbol phát hiện thêm ở vòng build
  sau (`get_default_presets`, `build_preset_genotype`,
  `process_preset_spawns_system`, `BevyPresetSpawnTrigger`, `BevyLineageTracker`,
  `commands::EnvironmentSettings`, `sync_environment_settings_system`,
  `receive_manual_disasters_system`, `BevyEnvironmentSettings`,
  `BevyManualDisaster`, `update_day_night_cycle_system` — đều xác nhận không còn
  trong `src-tauri/src`) cho thấy đúng 1 file thứ 12 cùng cảnh ngộ:
  `src-tauri/tests/organism_library_tests.rs`, không tồn tại trên origin/main →
  xoá theo cùng uỷ quyền và tiêu chí.
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** cả `12` suite test mồ côi nêu trên vẫn đã được xoá khỏi `src-tauri/tests/` (`770b5f3` xoá `11` suite đầu, `f021fd7` xoá `organism_library_tests.rs`); giữ nguyên `OPEN` do chưa hợp nhất thành một PR độc lập kèm regression test.


### F-010 — verify.sh chết im lặng (exit 1, không output) khi không có thay đổi nào trong thư mục tests
- **Ngày:** 2026-08-24
- **Loại:** BUG
- **Vị trí:** `scripts/verify.sh:41` (`GUARD_DIRS=$(...)`)
- **Phát hiện bởi:** tay
- **Triệu chứng:** dòng 41 là một phép gán qua pipeline
  `grep -E "$GUARD_RE" | sed | sort -u` mà KHÔNG có `|| true`. Khi working tree
  sạch (hoặc mọi thay đổi đều ngoài vùng tests), `grep` trả exit 1 vì không có
  dòng nào khớp; `set -eo pipefail` ở đầu script khiến nó thoát ngay tại đó,
  exit 1, không in bất kỳ thông báo nào — người chạy chỉ thấy
  `error: recipe 'verify' failed` từ just. Nghịch lý: cổng "chống gian lận"
  tự chết đúng khi agent KHÔNG gian lận (tree sạch).
- **Tái hiện:** trên working tree sạch: `just verify` → exit 1, log trống.
  Trace xác nhận: `bash -x ./scripts/verify.sh "" ""` dừng ngay sau `+ GUARD_DIRS=`.
- **Test khoá lại:** không áp dụng được (script thuộc hardening kit, cấm sửa theo
  AGENTS.md; cần người sở hữu kit sửa bằng cách thêm `|| true` hoặc xử lý rỗng
  tường minh trước khi gán `GUARD_DIRS`).
- **Trạng thái:** OPEN
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** trùng với [F-024]; PR `#104` đã merge (`e490bfa4` / `b8d1fa7`) thêm `|| true` vào `GUARD_DIRS` tại `scripts/verify.sh:43`; giữ nguyên `OPEN` do chưa có unit test tự động cho `scripts/verify.sh`.

### F-001 — MAP-Elites chọn cha mẹ trên HashMap nên sim không tái lập được
- **Ngày:** 2026-08-11
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/evolution/map_elites.rs:14` (khai báo) và `:52` (`select_parent`)
- **Phát hiện bởi:** determinism
- **Triệu chứng:** `select_parent` gọi `self.grid.values().choose(rng)` trên
  `std::collections::HashMap`. `choose` lấy mẫu bằng cách duyệt tuần tự iterator,
  mà thứ tự duyệt HashMap phụ thuộc `RandomState` — được gieo lại từ OS **mỗi lần
  khởi động tiến trình**. Hệ quả: cùng một seed RNG, cùng một archive, hai lần
  chạy vẫn chọn ra cha mẹ khác nhau. Seed RNG một mình KHÔNG sửa được lỗi này.
  Toàn bộ nhánh tiến hoá sau đó phân kỳ.
- **Tái hiện:** trước bản vá, chạy `cargo test --release --test evolution_robustness_tests`
  nhiều lần và in ra phần tử được chọn — thứ tự đổi giữa các lần chạy.
- **Test khoá lại:** `src-tauri/tests/evolution_robustness_tests.rs::test_selection_bias_tournament`
- **Trạng thái:** FIXED — đổi `HashMap` sang `BTreeMap` (khoá `(i32,i32)` vốn đã `Ord`)

### F-002 — Mỗi file test tự lắp World riêng nên thêm resource nào cũng vỡ
- **Ngày:** 2026-08-11
- **Loại:** SMELL
- **Vị trí:** 23 chỗ `World::new()` rải trong 7 file `src-tauri/tests/`
- **Phát hiện bởi:** determinism
- **Triệu chứng:** không file test nào dùng `init_world()`; mỗi file tự chèn tập
  resource vừa đủ cho riêng nó. Thêm một resource bắt buộc vào bất kỳ system nào
  làm chúng panic **lúc chạy** chứ không phải lúc build, và thông báo lỗi
  (`Resource requested by ... does not exist`) không chỉ ra file test nào sai.
  Thêm `SimRng` đã làm vỡ 1 test kiểu này.
- **Tái hiện:** thêm một `Res<T>` mới vào `spawn_food_system` rồi `cargo test`
- **Test khoá lại:** chưa có — cần một helper `test_world()` dùng chung
- **Phạm vi thật (người kiểm bổ sung):** không chỉ một dòng. `mark_offline()` cũng
  equivalent vì nó chỉ đặt `is_online=false` (vốn đã false) và gán `neo4j_graph=None`
  (field private, chỉ đọc trong nhánh `if self.is_online()` không bao giờ chạy).
  Mọi mutant trong nhánh online của `FallbackLineageTracker` đều cùng loại này.
- **Đã xử lý:** thêm `exclude_re` vào `src-tauri/.cargo/mutants.toml` kèm lý do, để
  chúng không sinh task rác mỗi lượt quét. Gỡ ra khi nào struct nhận được mock qua DI.
- **Trạng thái:** OPEN
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** `git grep -l "World::new()" -- src-tauri/tests/` vẫn khớp `39` file test lắp `World::new()` riêng; giữ nguyên `OPEN`.

### F-003 — Luật xử lý hoà fitness trong MAP-Elites chưa từng được quyết định
- **Ngày:** 2026-08-11
- **Loại:** TEST-GAP
- **Vị trí:** `src-tauri/src/evolution/map_elites.rs:43` (`add_individual`)
  và `:73` (`select_parent`)
- **Phát hiện bởi:** mutant
- **Triệu chứng:** cả hai chỗ dùng `>`; đổi thành `>=` thì không test nào phát
  hiện. Nghĩa là hành vi khi hai cá thể **hoà fitness** — giữ cá thể cũ hay thay
  bằng cá thể mới — chưa ai chọn có ý thức, nó chỉ là mặc định của lần gõ đầu.
  Với MAP-Elites, thay khi hoà làm archive xoay vòng giữa các cá thể ngang tài,
  khiến lineage nhiễu mà chất lượng không tăng; tournament lấy ứng viên bốc sau
  thì thiên vị theo thứ tự bốc thay vì theo fitness.
- **Tái hiện:** `just verify "src/evolution/map_elites.rs:43:35: replace > with >= in MapElitesArchive::add_individual"`
- **Test khoá lại:**
  `tests/map_elites_tests.rs::add_individual_hoa_fitness_thi_giu_ca_the_cu`
  `tests/map_elites_tests.rs::select_parent_hoa_fitness_thi_giu_ung_vien_truoc`
- **Trạng thái:** FIXED — hành vi hiện tại (giữ cái cũ / giữ ứng viên trước) nay
  đã được khoá bằng test, không còn đổi ngầm được

### F-004 — FallbackLineageTracker::is_online không thể test được vì thiếu mock/DI cho Neo4j
- **Ngày:** 2026-08-11
- **Loại:** TEST-GAP
- **Vị trí:** `src-tauri/src/evolution/lineage.rs:186` và `:190` — và **cả nhánh online** của struct
- **Phát hiện bởi:** mutant
- **Triệu chứng:** `FallbackLineageTracker` chỉ chuyển sang `is_online == true` khi kết nối thành công tới dịch vụ Neo4j thật qua mạng. Trong môi trường unit test (không có server Neo4j), `is_online` luôn giữ giá trị mặc định `false`. Do đó, đột biến `replace FallbackLineageTracker::is_online -> bool with false` trả về `false` hoàn toàn trùng khớp với giá trị thực tế trong môi trường test, không thể tạo ra hành vi quan sát được sai từ API công khai nếu không refactor code nguồn để hỗ trợ injection/mocking.
- **Tái hiện:** `just verify "src/evolution/lineage.rs:187:9: replace FallbackLineageTracker::is_online -> bool with false"`
- **Test khoá lại:** chưa có — cần refactor `FallbackLineageTracker` (thêm constructor nhận status mock hoặc trait `Neo4jClient`) để test được.
- **Trạng thái:** OPEN
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** `src-tauri/src/evolution/lineage.rs:679-688` (`is_online` / `mark_offline`) và ngoại lệ `src-tauri/.cargo/mutants.toml:34` chưa đổi (chưa có DI/mock); giữ nguyên `OPEN`.

### F-005 — Ba cặp phụ thuộc vòng giữa các module lõi
- **Ngày:** 2026-08-11
- **Loại:** SMELL
- **Vị trí:** `src-tauri/src/{ai,core,physics}/`
- **Phát hiện bởi:** dataflow
- **Triệu chứng:** đồ thị phụ thuộc thật (`cargo modules dependencies --lib`) cho
  thấy ba cặp trỏ lẫn nhau: `ai ↔ core`, `ai ↔ physics`, `core ↔ physics`.
  Nghĩa là không module nào trong ba module lõi có thể test, đọc hay thay thế độc
  lập với hai module kia. Đây cũng là lý do F-002 khó sửa: muốn có `test_world()`
  dùng chung thì phải quyết định nó thuộc về đâu, mà cả ba đều phụ thuộc nhau.
- **Tái hiện:** `just dataflow-truth && just dataflow-verify`
- **Đo lại 2026-10-31:** không phải ba mà **sáu** cặp vòng trên 16 cạnh mức module: thêm `ai ↔ evolution`,
  `core ↔ evolution` và `commands ↔ core`
  ([`CRATE_SPLIT_PLAN_2026-10-31.md`](docs/planning/CRATE_SPLIT_PLAN_2026-10-31.md) §1).
- **Test khoá lại:** có từ `3c06491` (2026-10-31). `dataflow.json` khai đồ thị hiện tại, kể cả sáu vòng,
  và job Rust của CI so nó với `cargo modules` ở bước "Module dependency graph matches dataflow.json".
  Gate này chặn một cạnh **mới**; nó không gỡ sáu vòng đang có.
- **Trạng thái:** OPEN — sáu vòng vẫn còn. Gỡ chúng là việc của kế hoạch tách crate (S1 trở đi), đang chờ
  QĐ-M2. Cập nhật 2026-11-18 (bước S0.4 của kế hoạch đó).
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** `dataflow.json` vẫn giữ đủ `6` cặp hai chiều (`('ai','core'), ('ai','evolution'), ('ai','physics'), ('commands','core'), ('core','evolution'), ('core','physics')`); giữ nguyên `OPEN`.

### F-006 — get_bin_coords nuốt NaN và gán vào ô gốc, im lặng
- **Ngày:** 2026-08-11
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/evolution/map_elites.rs:29` (`get_bin_coords`)
- **Phát hiện bởi:** fuzz (giả thuyết fuzz, kiểm bằng tay vì cargo-fuzz bị chặn — xem F-007)
- **Triệu chứng:** `(f / resolution).floor() as i32` với `f = NaN` cho ra `0`, vì
  `as i32` trong Rust là saturating cast và NaN quy về 0. Kết quả: mọi vector đặc
  trưng chứa NaN đều rơi vào **ô (0,0)** — cùng ô với gốc toạ độ hợp lệ. Một agent
  có đặc trưng hoá NaN sẽ tranh ô với một elite thật và có thể đẩy nó ra nếu
  fitness cao hơn. Không panic, không log, không test nào bắt được.
  Kèm theo: mọi giá trị ≥ ~2^31 × resolution đều bão hoà về cùng một ô biên, nên
  các hành vi rất khác nhau bị gộp làm một.
- **Tái hiện:**
  ```rust
  let a = MapElitesArchive::new(0.5);
  assert_eq!(a.get_bin_coords(&[f32::NAN, f32::NAN]), a.get_bin_coords(&[0.0, 0.0]));
  ```
- **Test khoá lại:** chưa có
- **Đề xuất sửa:** `get_bin_coords` trả `Option<(i32,i32)>` và trả `None` khi đặc
  trưng không hữu hạn; `add_individual` bỏ qua cá thể như vậy và đếm vào một
  counter để thấy được. Sửa im lặng bằng cách clamp là giấu lỗi, không phải sửa.
- **Trạng thái:** OPEN — chưa chứng minh được NaN thật sự phát sinh trong đường
  chạy sim, nhưng hành vi khi nó phát sinh thì sai rõ ràng.
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** `src-tauri/src/evolution/map_elites.rs:147-154` (`get_bin_coords`) vẫn trả `(i32, i32)` ép `.floor() as i32` chưa lọc NaN/non-finite; giữ nguyên `OPEN`.

### F-007 — Logic sim hàn chặt vào vỏ Tauri, chặn cả fuzz lẫn Miri
- **Ngày:** 2026-08-11
- **Loại:** SMELL
- **Vị trí:** `src-tauri/src/lib.rs:352` — `.run(tauri::generate_context!())`
- **Phát hiện bởi:** fuzz
- **Triệu chứng:** `cargo fuzz build` thất bại vì `generate_context!` cần toàn bộ
  pipeline build của Tauri. Cùng gốc rễ khiến `cargo miri` không chạy được (Miri
  không chạy FFI). Hệ quả: **hai lớp công cụ tìm bug mạnh nhất đều không dùng
  được**, và mỗi mutant phải build lại cả tauri + wgpu + burn.
- **Tái hiện:** `cd src-tauri && cargo +nightly fuzz build bin_coords`
- **Đã sửa:** tách `evolution::{genotype,mutation,crossover,map_elites}` ra crate
  `anima-core`. `ai` và `physics` KHÔNG tách được — cả hai dính `core::ecs`.
  Năm tham chiếu ngược của nhóm evolution nằm gọn trong một hàm `decode_genotype`,
  đã chuyển sang `core/spawn.rs`.
- **Kết quả đo được:**
  | | trước | sau |
  |---|---|---|
  | `cargo fuzz` | hỏng khi build | 191.792.110 lượt / 91 giây |
  | `cargo miri` | không chạy | chạy được |
  | mutation | 24 mutant / 55 phút | 135 mutant / 2 phút |
- **Trạng thái:** FIXED (b6c636b)

### F-008 — Không test nào khẳng định genotype hỏng bị TỪ CHỐI
- **Ngày:** 2026-08-11
- **Loại:** TEST-GAP
- **Vị trí:** `src-tauri/anima-core/src/mutation.rs:6` (`is_valid_genotype`)
- **Phát hiện bởi:** mutant (`replace is_valid_genotype -> bool with true` sống sót)
- **Triệu chứng:** mọi lời gọi `is_valid_genotype` trong toàn bộ test đều khẳng
  định `true` — không một chỗ nào khẳng định `false`. Hàm có thể luôn trả `true`
  mà cả suite vẫn xanh. Nguy hiểm thật chứ không chỉ thiếu điểm: `mutate_genotype`
  dùng chính hàm này để quyết định giữ hay hoàn tác một đột biến, nên hàm hỏng
  nghĩa là genotype không hợp lệ được nhận, rồi `decode_genotype` dựng sinh vật lỗi.
- **Tái hiện:** `cd src-tauri/anima-core && cargo mutants --file src/mutation.rs`
- **Test khoá lại:** `anima-core/tests/genotype_validity.rs` — 6 test, phủ cả 5
  điều kiện từ chối (rỗng, nhiều gốc, thành phần rời rạc, chu trình, cạnh trỏ tới
  node không tồn tại) kèm một ca đối chứng khẳng định `true`
- **Trạng thái:** FIXED — bản thân hàm vốn đúng, 6/6 test pass ngay lần đầu

### F-009 — Một phần mutant sống là equivalent do khoảng lấy mẫu đối xứng
- **Ngày:** 2026-08-11
- **Loại:** SMELL
- **Vị trí:** `anima-core/src/{mutation,crossover}.rs` — 10 chỗ dạng
  `x + rng.gen_range(-a..a)`
- **Phát hiện bởi:** mutant
- **Triệu chứng:** đổi `+` thành `-` không phân biệt được, vì khoảng lấy mẫu đối
  xứng quanh 0 nên hai biểu thức cho **cùng một phân phối**. Không test hành vi
  nào bắt được; chỉ test ghim giá trị theo seed mới bắt, mà test đó giòn và không
  khẳng định điều gì có nghĩa.
- **Ý nghĩa:** trần thực tế của mutation score **không phải 100%**. Trước khi ép
  score lên, phải phân loại mutant sống thành "thiếu test" và "equivalent" — nếu
  không sẽ đốt công vào việc bất khả thi.
- **Đã xử lý:** `exclude_re` trong `anima-core/.cargo/mutants.toml` kèm điều kiện
  gỡ ra (nếu khoảng lấy mẫu trở thành bất đối xứng)
- **Trạng thái:** OPEN
- **Đối chiếu thân cây (2026-11-24, `cf3fb24a`):** điều kiện gỡ chưa đạt — `src-tauri/anima-core/.cargo/mutants.toml:14` còn ngoại lệ `replace \+ with -`; giữ nguyên `OPEN`.
