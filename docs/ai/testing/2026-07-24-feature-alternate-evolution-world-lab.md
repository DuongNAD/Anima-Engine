---
phase: testing
feature: alternate-evolution-world-lab
title: Testing — Alternate Evolution & World Lab
description: Contract, budgets, deterministic forks, selection, speciation và observability
status: active
owner: simulation-architecture
last_reviewed: 2026-07-25
requirements: ../requirements/2026-07-24-feature-alternate-evolution-world-lab.md
design: ../design/2026-07-24-feature-alternate-evolution-world-lab.md
contract: ../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md
---

# Testing — Alternate Evolution & World Lab

## Test objective

Test suite phải phân biệt bốn tuyên bố:

1. source/field/budget hoạt động;
2. pathway tạo performance difference;
3. selection đổi trait frequency qua reproduction;
4. lineage đủ bằng chứng để gọi ecotype/candidate species/species.

Pass mức thấp không tự động chứng minh mức cao.

## Coverage goals

- 100% branch cho validator, budget transaction, fingerprint, fork và migration code mới.
- Property tests cho ranges, conservation/balance và deterministic seed derivation.
- Integration tests cho mọi đường genesis/checkpoint/control/treatment/save/load.
- Multi-seed statistical scenarios cho selection/speciation claims.
- Frontend tests cho manifest diff, units, aligned series, failure visibility và inspector.
- E2E smoke cho flow tạo experiment → chạy → inspect → compare → export.
- Map evidence là manual/MCP gate riêng, không được thay bằng unit test.

## Gate matrix

| Gate | Requirement | Cấp test | Pass condition |
|---|---|---|---|
| AE-S01 | SC-01/FR-13 | Regression | Exotic disabled giữ baseline checksum/tolerance |
| AE-S02 | SC-02 | Unit/integration | Same manifest/seed/build → same checksum |
| AE-S03 | FR-01/03 | Unit | Law change → fingerprint change; ordering canonical |
| AE-S04 | SC-03/FR-05 | Property | MU balance error dưới tolerance |
| AE-S05 | SC-03 | Integration | EU total không nhảy vì exotic transaction |
| AE-S06 | SC-04 | Experiment | Pathway costly/neutral khi source absent |
| AE-S07 | SC-05 | Experiment | Source present đổi performance qua transaction |
| AE-S08 | SC-06 | Integration | Genesis forks chỉ khác declared factor |
| AE-S09 | SC-06 | Integration | Checkpoint forks có identical pre-fork checksum |
| AE-S10 | SC-05 | Long-run | Trait frequency đổi qua offspring/selection events |
| AE-S11 | SC-10/FR-11 | Unit/statistical | Morphology-only difference không thành Species |
| AE-S12 | SC-07 | Integration | Trace law/source→field→performance→reproduction→trait |
| AE-S13 | SC-08 | Contract/UI | Same observable id/unit/value backend↔UI |
| AE-S14 | SC-09 | Statistical | Summary chứa N/effect/interval/failures |
| AE-S15 | FR-07 | Persistence | Save/load/migration giữ laws/field/storage |

## Unit and property tests

### World-law schema and validator

- [ ] `disabled_law_round_trips_and_has_stable_fingerprint` — AE-S03.
- [ ] `field_order_or_json_key_order_does_not_change_fingerprint` — canonical encoding.
- [ ] `changing_any_material_law_changes_fingerprint`.
- [ ] Unknown schema/source/unit bị reject, không fallback.
- [ ] Negative source/diffusion/decay hoặc invalid bounds bị reject.
- [ ] `FactorDiff` từ chối undeclared difference.
- [ ] Empty/duplicate seed list và resource limit bị reject.

### Exotic field

- [ ] Uniform initializer tạo đúng total MU.
- [ ] Patchy initializer deterministic theo seed và bounded.
- [ ] Diffusion không tạo/mất MU ngoài rounding.
- [ ] Source/decay cập nhật đúng balance equation.
- [ ] Edge boundary mode rõ và test được.
- [ ] Density luôn finite và trong `[0,max_density]`.
- [ ] Hot update không heap allocation sau init.

### Transactions and budgets

- [ ] Uptake debit field và credit organism storage cùng lượng.
- [ ] Spend debit storage và credit dissipated/exported đúng semantics.
- [ ] Organism death release/storage sink có event rõ.
- [ ] Saturated storage không làm mất MU.
- [ ] MU transaction không sửa closed EU ngoài transfer EU hợp lệ — AE-S05.
- [ ] Budget audit bắt deliberate 1e-3 leak.

### Pathway genotype/phenotype/runtime

- [ ] Legacy default disabled/zero giữ behavior.
- [ ] Mutation/crossover deterministic và bounded.
- [ ] Development materialize capacity/cost đúng một lần.
- [ ] Restore/migration không develop lại.
- [ ] Runtime storage không bị serialize nhầm vào genotype.
- [ ] Maintenance cost tồn tại khi pathway expressed.
- [ ] Toxicity/overload có clamp/failure behavior.

### Fork and provenance

- [ ] Genesis forks có same artifact/initial state/seed schedule.
- [ ] Checkpoint forks có same snapshot checksum tại fork tick.
- [ ] Parent/fork/run ids round-trip.
- [ ] Cross-law snapshot restore bị reject.
- [ ] Off-target intervention không đổi RNG draw order.

### Observable registry

- [ ] ID unique, unit non-empty, source symbol tồn tại.
- [ ] Conserved variables có conservation metadata.
- [ ] Cadence/aggregation/range hợp lệ.
- [ ] Unknown observable query trả structured error.
- [ ] Registry version/fingerprint đi cùng result.

### Species detector

- [ ] Color-only và morphology-only fixtures không tự thành Species.
- [ ] Cluster nhỏ/ngắn hạn chỉ là Morph.
- [ ] Niche divergence không có lineage persistence chỉ là Ecotype candidate.
- [ ] Threshold/policy version đổi invalidates cached diagnostic.
- [ ] Detector không ghi ngược genotype/fitness/mating state.

## Integration scenarios

### I1 — Baseline compatibility

- [ ] Chạy fixture hiện tại với legacy scenario.
- [ ] Chạy manifest tương đương với `exotic_energy=None`.
- [ ] So checksum, observables, ledger và RNG checkpoints — AE-S01.

### I2 — Renewable patchy Mana reference slice

- [ ] Source tạo field patchy.
- [ ] Producer/consumer pathway uptake theo vị trí.
- [ ] MU budget khép.
- [ ] EU transfer vẫn khép.
- [ ] Causal ledger chứa full vertical chain — AE-S04/05/07/12.

### I3 — Genesis fork

- [ ] Control và treatment cùng artifact/seed/biomass.
- [ ] Treatment chỉ khác `laws.exotic_energy`.
- [ ] Pre-run validator xuất đúng factor diff.
- [ ] Result provenance đủ để replay — AE-S08.

### I4 — Checkpoint removal

- [ ] Chạy tới generation G và snapshot.
- [ ] Hai nhánh restore cùng checksum.
- [ ] Một nhánh remove source bằng intervention có `CauseId`.
- [ ] Đo dependency/recovery/extinction debt — AE-S09/12.

### I5 — Selection, không genotype rewrite

- [ ] Khởi tạo variation giống nhau ở control/treatment.
- [ ] Không system nào sửa pathway genotype trực tiếp ngoài mutation/reproduction.
- [ ] Trait frequency chỉ đổi sau birth/death/reproductive success events — AE-S10.

### I6 — Persistence

- [ ] Save/load giữ law fingerprint, field, budget, storage, provenance.
- [ ] Migration same-law giữ phenotype/storage.
- [ ] Cross-law migration fail/warn theo policy.
- [ ] Legacy save migrate thành exotic disabled — AE-S15.

### I7 — Backend/UI parity

- [ ] Rust registry payload khớp TypeScript schema.
- [ ] Layer value/legend/unit khớp backend sample.
- [ ] Timeline alignment không đổi tick/generation semantics.
- [ ] Export rồi import giữ manifest/result fingerprint — AE-S13.

## Statistical experiments

### E1 — Cost when absent

- [ ] Factor: pathway enabled/disabled × source absent.
- [ ] Ensemble tối thiểu 10 seed ở phase chứng minh.
- [ ] Kỳ vọng: pathway không có positive reproductive effect; maintenance cao phải bất lợi.
- [ ] Báo effect size/interval, không chỉ p-value — AE-S06/14.

### E2 — Benefit when present

- [ ] Cùng initial variation, source patchy on/off.
- [ ] Kiểm tra performance trước trait frequency.
- [ ] Kỳ vọng: local pathway advantage trong hotspot nhưng có trade-off ngoài hotspot — AE-S07/10.

### E3 — Historical contingency

- [ ] Mana từ genesis vs Mana thêm ở generation G.
- [ ] So time-to-adaptation, lineage survival, niche occupancy.
- [ ] Không giả định hai treatment phải hội tụ.

### E4 — Candidate speciation

- [ ] Patchy source + migration/gene flow variants.
- [ ] Theo dõi genotype/niche/gene-flow/persistence.
- [ ] Báo Morph/Ecotype/CandidateSpecies theo policy.
- [ ] Chỉ nâng claim sau ensemble và AE-S11/14.

### E5 — Source removal

- [ ] Fork cùng checkpoint: continue vs remove.
- [ ] Đo population crash, trait decay, alternative adaptation và recovery time.

## End-to-end user flows

- [ ] Tạo experiment từ baseline template, bật Mana treatment, xem manifest diff.
- [ ] Chạy 5-seed ensemble headless, xem progress/failures.
- [ ] Bật Mana layer, click cell và xem density/source/uptake history.
- [ ] Click lineage và xem pathway/reproductive success/ancestry.
- [ ] Chọn trait-frequency delta và trace tới world-law cause.
- [ ] Fork từ checkpoint, remove Mana và compare aligned results.
- [ ] Export JSON/CSV, import/replay và xác nhận checksum.
- [ ] UI không cho sửa world law in-place; yêu cầu branch/restart.

## Fixtures

| Fixture | Nội dung |
|---|---|
| `baseline-no-exotic.json` | Baseline current contract |
| `mana-uniform-renewable.json` | Field đơn giản cho unit/integration |
| `mana-patchy-renewable.json` | Hotspot deterministic |
| `invalid-negative-source.json` | Validator failure |
| `checkpoint-generation-100.bin` | Fork parity |
| `legacy-save-no-world-laws.json` | Migration default |
| `pathway-cost-population.json` | Mixed genotype variation |
| `morphology-only-clusters.json` | Species false-positive guard |
| `ensemble-with-failure.json` | Summary không drop failed run |

Fixtures phải ghi schema version, seed, artifact checksum và expected invariant; không phụ thuộc port
hoặc network.

## Performance tests

- [ ] 128²/256² exotic field update theo ecology band.
- [ ] 1k full agents và 10k reduced/cohort pathway transactions.
- [ ] 5/10/30-seed headless ensemble.
- [ ] 1M state samples qua chunk/downsample query.
- [ ] Causal record rate với threshold/top-K.
- [ ] Snapshot size/time với field + organism storage.
- [ ] Frontend layer/timeline ở target sample count.

Report phải ghi build mode, hardware, schema/model version, seed count và workload. Không khóa con số
trước khi có baseline trên Dell Vostro 3530.

## Manual and map validation

- [ ] Labels/units/legends dễ đọc, không dùng màu đơn độc cho trạng thái.
- [ ] Keyboard/focus cho builder, run tree, charts và inspector.
- [ ] Warning khi ensemble nhỏ, run fail hoặc budget drift.
- [ ] Canonical overview/spawn/ecosystem views qua Animal Map Vision.
- [ ] Field hotspot, organism placement, collider/nav và minimap cùng tọa độ.
- [ ] Không critical/high ecological contradiction.

Trạng thái cho AE2.5: **out of scope** — không có map claim nào được đưa ra. Phiên thực hiện AE6/map
phải kiểm tra MCP lúc đó và chạy đầy đủ Animal Map Vision gate trước mọi tuyên bố visual/ecology.

## Planned verification commands

Khi implementation bắt đầu, phase owner phải thay placeholder filter bằng tên module/test thật:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib -- ae_
npm run test
npm run test:frontend
npm run build
npx ai-devkit@latest lint --feature alternate-evolution-world-lab
```

Khối lệnh trên là template từ giai đoạn design ban đầu. Production headless đã có; bằng chứng hiện
hành và tên test thật nằm ở các mục AE1–AE2/AE2.5 bên dưới. Không dùng placeholder `ae_` để thay thế
full gate.

## AE1–AE2 headless verification evidence (2026-07-25)

Backend Rust only; frontend/`npm` commands and the map gate remain out of scope for this slice
(AE6/AV). Real module/test names below replace the placeholder `ae_` filter. Commands run with
`cargo` on Windows (debug profile), exit code 0 unless stated.

### Commands and results

| Command | Result |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` (pre-feature baseline) | 73 passed, 0 failed |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` (after the **AE2.5 audit pass**) | **173 passed, 0 failed, 1 ignored** |
| `cargo test --manifest-path src-tauri/Cargo.toml --test exotic_energy_zero_alloc_tests` | **2 passed** (field + forcing hot paths) |
| `cargo test … --lib` (AE2.5 first pass — superseded) | 153 passed, 0 failed, 1 ignored |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` (AE1–AE2, fifth pass — superseded) | 133 passed, 0 failed |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib -- p1_ p2_ p3_` (focused) | 7 passed, 0 failed |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib defect_` (focused) | 8 passed, 0 failed |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib core::experiment_runner` (focused) | 24 passed, 0 failed |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | clean (exit 0) |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --lib` | 4 warnings total; **0 in the new AE modules**. All 4 pre-existing/untouched: `dynamic_fields.rs:180`, `scenario.rs:409`, `sim_clock.rs:66`, `sim_clock.rs:77` |
| `git diff --check` | clean, exit 0 (only LF/CRLF advisory) |

No pre-existing lib failure at baseline; every AE test is net-new and all 60 pass (29 first pass + 11
second pass + 5 third-pass edge-case + 8 fourth-pass contract-hardening + 7 fifth-pass
provenance/transaction tests).

### Independent closure verification (2026-07-25)

Fresh after the Codex closure refinements:

| Gate | Result |
|---|---|
| Full Rust lib | **173 passed, 0 failed, 1 ignored** (fixture regenerator) |
| `d1_` paired contract | **8 passed** |
| `d2_` source-suppression contract | **6 passed** |
| `d3_` cadence/grid contract | **2 passed** |
| `d4_` causal attribution | **2 passed** |
| `ck_` checkpoint filter | **5 passed** (4 checkpoint-channel tests + existing `p1_fork…` match) |
| `ae210_m5` fixtures | **3 passed** |
| zero-allocation integration test | **2 passed** |
| `cargo fmt … --check` | clean |
| `cargo clippy … --lib` | exit 0; 4 pre-existing warnings, 0 in AE modules |
| base AI DevKit docs lint | all checks passed |
| feature AI DevKit lint | all seven feature docs `[OK]`; only the expected feature-branch convention is `[MISS]` because this shared 83-entry dirty tree was intentionally not moved/branched |

`cargo llvm-cov` is not installed in this workspace, so no numeric coverage percentage is claimed.
Contract and regression pass counts above are the coverage evidence for this slice.

### AE2.5 audit-pass coverage (2026-07-25, test-first; each observed failing first)

| Concern | Passing test(s) |
|---|---|
| **D1** paired same-seed runner, ordered seeds, both fingerprints | `core::experiment_runner::tests::d1_paired_runner_uses_same_ordered_seeds_and_reports_paired_stats` |
| D1 known paired sign/magnitude (explicit control) | `…::d1_paired_effects_have_known_sign_and_magnitude` |
| D1 `n=1` ⇒ means defined, spread `None` | `…::d1_paired_single_pair_defines_means_but_not_spread` |
| D1 one-sided failure preserved, excluded from effects | `…::d1_one_sided_failure_is_preserved_and_excluded_from_effects` |
| D1 `n=0` ⇒ all statistics `None` | `…::d1_zero_complete_pairs_yields_all_none_statistics` |
| D1 one-sided observable listed, not compared | `…::d1_observable_missing_on_one_side_is_listed_not_compared` |
| D1 report JSON round-trip (no NaN/inf) | `…::d1_paired_report_json_round_trips_without_nan` |
| D1 preflight rejection before model/RNG | `…::d1_paired_runner_rejects_bad_inputs_before_any_model_or_rng` |
| **D2** no drain when `source_rate = 0` | `core::exotic_energy::tests::d2_remove_source_never_drains_existing_mu_when_there_is_no_source` |
| D2 only avoided injection; lowers `cum_sourced` | `…::d2_remove_source_only_avoids_source_injection_and_lowers_cum_sourced` |
| D2 suppression capped at the source | `…::d2_suppression_is_capped_at_the_source_contribution` |
| D2 overlapping suppressions deterministic + jointly capped | `…::d2_overlapping_suppressions_are_deterministic_and_jointly_capped` |
| D2 Add/Pulse remain injections | `…::d2_add_and_pulse_remain_external_injections` |
| D2 `apply_forcing` refuses `RemoveSource` | `…::d2_apply_forcing_rejects_remove_source` |
| **D3** window must contain an ecology firing (boundary-exact) | `…::d3_forcing_window_must_contain_an_ecology_firing_tick` |
| D3 grid applicability (Cell/full Rect contained; Radius centre contained with edge clipping) | `…::d3_grid_applicability_is_validated_against_the_field` |
| D3 zero-alloc on the final forcing hot path | `exotic_energy_zero_alloc_tests::test_exotic_forcing_hotloop_zero_heap_allocations` |
| **D4** injection not double-counted in the world-law delta | `core::reference_world::tests::d4_add_forcing_movement_is_not_also_counted_in_the_world_law_delta` |
| D4 suppression attributed without pretending MU moved | `…::d4_remove_source_is_attributed_as_suppression_without_pretending_mu_moved` |
| **Checkpoint** all Add/Remove/Pulse kinds: identical through tick before first firing, then divergence + identities + EU isolation | `core::experiment_runner::tests::ck_exotic_channel_diverges_only_after_the_fork_and_keeps_identities` |
| Checkpoint determinism + structured serde round-trip | `…::ck_exotic_channel_replays_deterministically_and_round_trips` |
| Checkpoint invalid/non-post-fork/duplicate/no-field/out-of-grid rejection before model construction | `…::ck_exotic_channel_rejects_invalid_and_non_post_fork_extras` |
| Legacy `checkpoint_fork` unchanged | `…::ck_legacy_checkpoint_fork_still_works_unchanged` |

**Deleted (not adjusted):** `ae209_m2_remove_source_forcing_withdraws_and_books_as_dissipated` and
`ae209_m2_removal_cannot_drive_density_negative_and_stays_closed` encoded the old **drain** semantics
and were removed so that contract cannot be re-asserted; the `d2_*` suite supersedes them.

### AE-210 fixture size evidence (2026-07-25)

Manifest fixtures only — **not** a runtime or binary-artifact benchmark, and not a performance claim.

| Fixture | Bytes |
|---|---:|
| `src-tauri/tests/fixtures/experiments/baseline-no-exotic.json` | 911 |
| `src-tauri/tests/fixtures/experiments/invalid-negative-source.json` | 1349 |
| `src-tauri/tests/fixtures/experiments/mana-patchy-renewable.json` | 1543 |
| **total** | **3803** |

- Fresh Windows command: `Get-ChildItem src-tauri/tests/fixtures/experiments/*.json |
  Select-Object Name,Length` (sum via `Measure-Object Length -Sum`)
- Build context: Windows, `cargo` debug profile, rustc 1.95.0; files produced by the real serializer
  via `cargo test --lib ae210_regenerate_fixtures -- --ignored` (`serde_json::to_string_pretty`).
- Ceiling asserted in `ae210_m5_fixtures_are_small_enough_to_stay_reviewable` (< 8 KiB each), so a
  schema change that balloons a fixture fails the suite rather than passing unnoticed.
- Scope limitation: these pin the **manifest schema** (parse → validate → round-trip → fingerprint
  stability). No paired-report fixture was added — the `PairedEnsembleReport` schema is pinned by the
  in-process round-trip test instead, which avoids committing a large generated artifact.

### AE2.5 first-pass coverage (2026-07-25, written test-first)

Every row below was observed failing before its implementation landed.

| Concern | Passing test(s) |
|---|---|
| Forcing schema/validation (amount, geometry, overflow, never-active window) | `core::exotic_energy::tests::ae209_m1_forcing_validates_structurally` |
| Deterministic queue order + duplicate-id rejection | `core::exotic_energy::tests::ae209_m1_queue_is_deterministic_and_rejects_duplicate_ids` |
| Forcing serde round-trip | `core::exotic_energy::tests::ae209_m1_forcing_serde_round_trips` |
| Add / remove book MU into the ledger (budget stays closed) | `core::exotic_energy::tests::{ae209_m2_add_source_forcing_injects_and_books_mu, ae209_m2_remove_source_forcing_withdraws_and_books_as_dissipated}` |
| Removal cannot go negative / over-remove | `core::exotic_energy::tests::ae209_m2_removal_cannot_drive_density_negative_and_stays_closed` |
| Region scoping + `max_density` bound | `core::exotic_energy::tests::ae209_m2_forcing_is_region_scoped_and_bounded_by_max_density` |
| Inactive / zero forcing is a no-op | `core::exotic_energy::tests::ae209_m2_inactive_or_zero_forcing_is_a_noop` |
| Pulse curve shaping | `core::exotic_energy::tests::ae209_m2_pulse_curve_shapes_the_injection_over_its_window` |
| **Law immutability (ER01)**: forcing changes field, not the law fingerprint | `core::reference_world::tests::ae209_m3_forcing_changes_field_but_never_the_world_law` |
| **AE-S04/S05** under a forcing: MU closed, EU byte-identical | `core::reference_world::tests::ae209_m3_forcing_keeps_mu_closed_and_eu_untouched` |
| **AE-S12** forcing recorded under its own `CauseId` | `core::reference_world::tests::ae209_m3_forcing_is_recorded_in_the_causal_ledger_under_its_own_cause` |
| **AE-S02** forced run replays deterministically | `core::reference_world::tests::ae209_m3_forced_run_replays_deterministically` |
| Structured rejection of invalid/duplicate/field-less forcings | `core::reference_world::tests::ae209_m3_invalid_forcing_fails_the_manifest_structurally` |
| **AE-S14 effect size**: difference, Hedges' *g*, interval, one-sided observables | `core::experiment_runner::tests::ae_s14_m4_effect_size_reports_difference_g_and_interval` |
| Effect size sign/magnitude on a real treatment | `core::experiment_runner::tests::ae_s14_m4_effect_size_detects_a_real_difference_with_correct_sign` |
| Failures preserved + degenerate variance ⇒ `g = 0` (no NaN/inf) | `core::experiment_runner::tests::ae_s14_m4_effect_size_preserves_failed_runs_and_degenerate_variance` |
| **AE-210** fixtures parse/validate/round-trip/fingerprint-stable | `core::experiment::tests::ae210_m5_baseline_and_mana_fixtures_round_trip_and_validate` |
| Invalid fixture rejected structurally | `core::experiment::tests::ae210_m5_invalid_fixture_is_rejected_with_a_structured_error` |
| Fixture size record | `core::experiment::tests::ae210_m5_fixtures_are_small_enough_to_stay_reviewable` |

### Fifth-pass provenance & transaction coverage (2026-07-25, written test-first)

| Concern | Passing test(s) |
|---|---|
| Treatment provenance uses **effective** manifest fingerprint (control/prefix stay base) | `core::experiment_runner::tests::p1_treatment_provenance_uses_effective_manifest_fingerprint` |
| No extras → control/treatment fingerprints equal | `core::experiment_runner::tests::p1_no_extras_keeps_control_and_treatment_fingerprints_equal` |
| Report carries **structured** `treatment_extra`, JSON round-trips | `core::experiment_runner::tests::p1_report_carries_structured_extras_that_survive_json_roundtrip` |
| Determinism + exact fork-tick semantics preserved | `core::experiment_runner::tests::p1_fork_remains_deterministic_and_tick_exact` |
| `genesis_fork` registry preflight (no model/RNG work) | `core::experiment_runner::tests::p2_genesis_fork_validates_registry_before_any_model_work` |
| AE-205 uptake rejects NaN/∞/negative inputs without mutating | `core::exotic_energy::tests::p3_uptake_rejects_non_finite_and_negative_inputs_without_mutating` |
| AE-205 spend rejects NaN/∞/negative + corrupt ledger slots | `core::exotic_energy::tests::p3_spend_storage_rejects_non_finite_and_negative_inputs_without_mutating` |

### Fourth-pass contract-hardening coverage (2026-07-25, written test-first)

| Concern | Passing test(s) |
|---|---|
| Registry ranges JSON-safe (finite) | `core::experiment::tests::defect_a_reference_registry_is_json_safe_and_finite` |
| Strict spec validation (non-finite bounds, `cadence_period==0`, empty display/cadence/source) | `core::experiment::tests::defect_a_registry_validation_rejects_malformed_specs` |
| Completed baseline **and** treatment `RunResult` JSON round-trip | `core::experiment_runner::tests::defect_a_run_result_json_round_trips_for_baseline_and_treatment` (floats to serde_json's ±1 ULP; structural data exact) |
| Manifest-path intervention validation | `core::experiment::tests::defect_b_manifest_rejects_invalid_intervention_values` |
| `treatment_extra` values/geometry + unique ids (within extras & vs base) | `core::experiment_runner::tests::defect_b_checkpoint_validates_treatment_extra_values_and_ids` |
| Combined `MAX_INTERVENTIONS` ceiling | `core::experiment_runner::tests::defect_b_checkpoint_enforces_combined_intervention_limit` |
| Law rejects empty `display_name` | `core::exotic_energy::tests::defect_c_law_rejects_empty_display_name` |
| `from_law` defensive validation + zero/overflow grid rejection | `core::exotic_energy::tests::defect_c_from_law_validates_defensively_and_rejects_bad_grids` |

### Third-pass edge-case coverage (2026-07-25)

| Concern | Passing test(s) |
|---|---|
| Undeclared seed rejected pre-model/RNG | `core::experiment_runner::tests::{run_manifest_seed_rejects_seed_not_in_manifest, checkpoint_fork_rejects_seed_not_in_manifest}` |
| Ensemble validates once (empty seeds / invalid) | `core::experiment_runner::tests::ensemble_rejects_invalid_and_empty_inputs_at_ensemble_level` (`run_ensemble` now returns `Result`) |
| Result covers union of emitted (transient) observables | `core::experiment_runner::tests::result_describes_transient_series_only_observable_with_missing_spec_warning` |
| Checkpoint post-fork window guard | `core::experiment_runner::tests::checkpoint_rejects_treatment_extra_outside_post_fork_window` (boundaries `fork_tick`, `+1`, `duration`, `duration+1`) |
| Checkpoint prefix-divergence is structural | `core::experiment_runner::tests::checkpoint_fork_fails_structurally_when_prefix_diverges` |

### Gate → test mapping (fresh, passing)

| Gate | Passing test(s) |
|---|---|
| **AE-S01** | `core::experiment_runner::tests::ae_s01_baseline_manifest_matches_legacy_scenario_checksum`; `core::reference_world::tests::baseline_world_is_bit_identical_to_reference_ecosystem` |
| **AE-S02** | `core::experiment::tests::ae_s02_reordered_non_semantic_input_has_same_manifest_fingerprint`; `core::experiment_runner::tests::ae_s02_same_manifest_and_seed_give_same_checksum`; `core::reference_world::tests::ae_m4_either_branch_replays_deterministically` |
| **AE-S03** | `core::experiment::tests::{ae_s03_disabled_law_round_trips_and_has_stable_fingerprint, ae_s03_changing_any_material_law_changes_fingerprint, ae_s03_manifest_fingerprint_tracks_world_law_change}` |
| **AE-S04** | `core::exotic_energy::tests::{closed_diffusion_conserves_mu, source_decay_diffusion_closes_the_mu_budget, open_boundary_exports_mu_as_a_declared_sink, uptake_and_spend_conserve_mu_end_to_end, deliberate_leak_is_detected_by_the_budget}`; `core::reference_world::tests::ae_m4_treatment_produces_a_field_and_a_closed_mu_ledger` |
| **AE-S05** | `core::reference_world::tests::ae_m4_genesis_fork_isolates_the_exotic_factor_and_leaves_eu_unchanged` (EU delta exactly 0) |
| **AE-S08** | `core::experiment::tests::{ae_s08_control_variant_differs_only_in_exotic_law, ae_s08_undeclared_difference_is_rejected}`; `core::reference_world::tests::ae_m4_genesis_fork_isolates_the_exotic_factor_and_leaves_eu_unchanged` |
| **AE-S09** (headless checkpoint fork) | `core::experiment_runner::tests::{ae_s09_checkpoint_continuation_equals_uninterrupted_run, ae_s09_post_fork_treatment_diverges_only_after_the_fork, checkpoint_fork_rejects_bad_fork_tick}` |
| **AE-S12** (partial: law→field) | `core::reference_world::tests::ae_m4_treatment_produces_a_field_and_a_closed_mu_ledger` (chain roots at `CAUSE_EXOTIC_WORLD_LAW`) |
| **AE-S14 — complete for the headless reference slice** (same ordered seeds, paired effect/CI, failures retained) | `core::experiment_runner::tests::{d1_paired_runner_uses_same_ordered_seeds_and_reports_paired_stats, d1_paired_effects_have_known_sign_and_magnitude, d1_one_sided_failure_is_preserved_and_excluded_from_effects, d1_paired_report_json_round_trips_without_nan}` |
| MU construction (no silent clamp) | `core::exotic_energy::tests::over_capacity_initial_amount_is_rejected_not_silently_clamped`; `core::reference_world::tests::over_capacity_law_fails_construction_with_structured_error` |
| Ledger-exact uptake (f32/f64) | `core::exotic_energy::tests::fractional_and_repeated_uptake_is_ledger_exact` |
| Disabled == `exotic_energy = None` only (no `ExoticSourceModel::Disabled` variant exists; `Some(law)` is always `Renewable`) | `core::reference_world::tests::exotic_none_is_the_only_baseline_path` |
| Runner hardening (no silent exec) | `core::experiment_runner::tests::{run_manifest_seed_fails_on_unknown_observable_not_silently, run_manifest_seed_fails_on_invalid_manifest_and_registry, ensemble_rejects_invalid_and_empty_inputs_at_ensemble_level, result_is_self_describing_for_every_emitted_observable}` |
| Structured validation errors (AE-101/103) | `core::experiment::tests::{validator_accepts_a_well_formed_manifest, validator_rejects_structured_failures}` |
| Observable registry (AE-109) | `core::experiment::tests::{observable_registry_validates_and_is_unique, observable_registry_rejects_duplicate_ids}` |
| Field init determinism/bounds (AE-203) | `core::exotic_energy::tests::{uniform_field_has_the_declared_initial_total_and_is_bounded, patchy_field_is_deterministic_by_seed_and_bounded}` |
| Zero-alloc hot loop (AE-203) | `exotic_energy_zero_alloc_tests::test_exotic_field_hotloop_zero_heap_allocations` |
| Law validation (AE-202) | `core::exotic_energy::tests::law_rejects_eu_unit_and_bad_ranges` |

## AE3 verification evidence (2026-07-25)

Backend Rust only. The table below is the authoritative independent-audit snapshot from the shared
Windows tree after the concurrent deterministic-RNG refactor stopped writing files. Focused counts
isolate AE3; the full-lib count also includes 17 unrelated unit tests added by that concurrent work.
Details and original Claude-run counts remain in the
[AE3 goal progress log](../planning/2026-07-25-claude-overnight-goal-ae3.md#progress-log-2026-07-25-claude-code-implementation-run).

| Command | Result |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` | **224 passed, 0 failed, 1 ignored** |
| `… --lib evolution_pathway` | **20 passed** |
| `… --lib ae3` | **34 passed** |
| `… --lib ae30` (AE-301…310) | **24 passed** |
| `cargo test … --test exotic_energy_zero_alloc_tests` | **3 passed** (field, forcing, **AE3 physiology**) |
| `cargo fmt … --all -- --check` | clean |
| `cargo clippy … --lib` | 4 warnings, all pre-existing, **0 in any AE module** (incl. new `evolution_pathway.rs`) |
| `git diff --check` | clean (LF/CRLF advisory only) |
| AI DevKit base lint | pass |
| AI DevKit feature-doc lint | all seven docs pass; command exits 1 only because branch `feature-alternate-evolution-world-lab` does not exist in this intentionally shared dirty tree |
| `cargo test … --test sim_determinism_tests` | **15 passed** on the isolated run; this suite belongs to concurrent RNG work, not AE3 |
| `cargo test --manifest-path src-tauri/Cargo.toml` | **not green as a whole**: 224 lib tests passed, then Windows failed to start unrelated `adversarial_challenger_tests` with `STATUS_ENTRYPOINT_NOT_FOUND` before its assertions ran |

`cargo llvm-cov` is still not installed here, so **no coverage percentage is claimed**.

### AE3 gate → test mapping (fresh, passing)

| Gate | Passing test(s) |
|---|---|
| **AE-S01** (AE3 regression) | `core::reference_world::tests::ae3_s01_a_world_without_ae3_keys_stays_bit_identical_to_the_legacy_baseline` |
| **AE-S02** | `…::ae3_s02_a_population_run_replays_deterministically`; `core::evolution_pathway::tests::ae302_population_variation_replays_deterministically_from_its_own_seed` |
| **AE-S04/S05** | `…::ae3_s04_s05_organism_uptake_keeps_mu_closed_and_leaves_eu_byte_identical`; `core::evolution_pathway::tests::{ae305_spend_books_mu_as_dissipated_and_keeps_the_ledger_closed, ae307_reproduction_releases_stored_mu_so_the_ledger_stays_closed}` |
| **AE-S06** | `core::evolution_pathway::tests::{ae306_maintenance_cost_is_paid_even_when_no_mu_exists, ae306_absent_source_drives_a_costly_pathway_down_not_up}`; `core::reference_world::tests::ae310_factorial_absent_source_never_gives_a_pathway_a_free_advantage` |
| **AE-S07** | `core::evolution_pathway::tests::ae307_performance_gain_requires_a_real_spend_not_mere_source_presence`; `core::reference_world::tests::ae310_factorial_present_source_advantage_flows_through_the_transaction` |
| **AE-S10** | `core::evolution_pathway::tests::{ae310_physiology_ticks_cannot_change_frequency_generation_or_genotype, ae310_reproduction_delta_equals_the_resolved_offspring_composition}` |
| **AE-S12** (full chain) | `core::reference_world::tests::ae309_s12_pathway_frequency_traces_back_to_the_exotic_world_law` (asserts the ordered chain frequency → births → performance → uptake → density_total, rooted at `CAUSE_EXOTIC_WORLD_LAW`) |
| **AE-S12** (sole effective forcing) | `core::reference_world::tests::ae309_s12_pathway_frequency_traces_to_a_sole_effective_forcing` (forcing → density → uptake → performance → births → frequency, rooted at the forcing `CauseId`) |
| **AE-S12** (no fabricated cause) | `…::ae309_absent_source_cost_roots_at_background_not_a_fabricated_mana_cause` |
| **AE-S14** (AE3 paired) | `…::ae_s14_ae3_paired_multi_seed_reports_a_finite_effect_and_interval` (5 same-seed pairs; finite delta/SD/SE/CI/*d_z*; EU paired effect exactly 0) |
| Genotype JSON round-trip | `core::evolution_pathway::tests::ae301_genotype_serde_round_trips_without_trait_or_source_loss` |
| Seeded bounded variation / source compatibility | `core::evolution_pathway::tests::{ae302_mutation_is_replay_deterministic_and_bounded, ae302_crossover_is_replay_deterministic_and_bounded, ae302_crossover_rejects_incompatible_source_ids}` |
| One-time development | `…::ae303_development_materializes_capacities_once_from_the_genotype` |
| Legacy default disabled/zero-cost | `…::ae301_legacy_genotype_is_disabled_and_zero_cost` |
| Numeric hardening | `…::{ae301_genotype_normalizes_non_finite_and_out_of_range_inputs, ae306_performance_is_finite_and_non_negative_under_hostile_inputs}` |
| Structural population validation | `…::ae307_population_config_rejects_impossible_states_structurally`; `core::experiment::tests::ae3_initial_conditions_reject_malformed_and_unknown_keys` |
| Registry completeness / no missing spec / cumulative-birth semantics | `core::experiment::tests::ae308_registry_fully_describes_every_ae3_observable` (also pins cumulative `evolution.births` to `Aggregation::Instant`); `core::reference_world::tests::ae308_population_observables_are_emitted_only_when_the_population_exists` (asserts `warnings` is empty) |
| No fabricated AE3 zero | `core::experiment::tests::ae308_manifest_rejects_an_ae3_observable_without_an_enabled_population` |
| Checkpoint restore of population + RNG | `core::reference_world::tests::ae3_checkpoint_restore_preserves_population_and_rng_state` |
| Zero-alloc physiology hot path | `exotic_energy_zero_alloc_tests::test_ae3_population_physiology_hotloop_zero_heap_allocations` |

### Process honesty for AE3

- **M1–M4 were genuinely test-first**: each batch was observed failing first (`cannot find type
  EnergyPathwayGenotype`, `cannot find type PathwayCohort`, `cannot find type
  ReferencePopulation{,Config}`, `cannot find value AE3_KEY_*`, `no method named population`).
- **M2 produced a real behavioural red→green**: `MU is held before the boundary` failed because
  metabolic demand had been scaled to the uptake surface, draining storage every tick and making
  `storage_capacity` a dead trait. Demand is now a fraction of the reserve.
- **M5 was NOT a red/green cycle** — the three factorial/AE-S14 tests passed on first execution
  because M1–M4 had already built the mechanism. Their discriminating power was instead proved by
  **deliberate mutation**: replacing the earned MU gain with a flat bonus keyed on
  `developed.expressed` (the forbidden "fitness from presence" shortcut) failed **6 tests**; the
  mutation was reverted and Claude's implementation snapshot returned to 204 passed.
- **Independent closure defects were test-first:** missing genotype serde failed to compile; an
  incompatible-source crossover test failed against the old return type; cumulative births declared
  as `Sum` failed metadata validation; and the forcing-frequency chain incorrectly rooted at
  `CAUSE_EXOTIC_WORLD_LAW`. Each now passes. The births and forcing regressions were also
  mutation-checked after green and failed again under the old behavior before restoration.

### Not covered by AE3 (still open)

- **AE-S11** species detection (AE5), **AE-S13** UI parity (AE6), **AE-S15** persistence (AE4), map gate.
- `crossover` is tested but not used as a reproduction mechanism (clonal-within-strategy inheritance).
- Generational death is implicit full-cohort turnover; there is no individual mortality process or
  separate death observable in this reference slice.
- Forcing ancestry is claimed only for the isolated case where the field was empty, source rate was
  zero, and exactly one forcing injected MU. Mixed-origin attribution awaits a multi-parent ledger.
- The population is a two-cohort aggregate, not live ECS entities; no adaptation claim extends beyond
  the headless reference slice, and 5 seeds is an ensemble size, not a confidence claim.

### Not covered this slice (out of scope / deferred)

- **AE-S11** — species detection: AE5, not started; the word "species/adaptation/ecotype" is not
  claimed from this evidence.
- **AE-S13** — backend/UI observable parity: AE6 (no frontend touched).
- **AE-S15** — save/load/migration: AE4 (persistence untouched).
- **Map gate** — outside this headless slice. Khi bắt đầu AE6/map work, phải chạy workflow Animal Map
  Vision bắt buộc và ghi bằng chứng manifest/canonical views; không suy availability của MCP từ
  phiên headless này.

## Traceability

Mọi AE-S01…AE-S15 có ít nhất một task trong
[planning](../planning/2026-07-24-feature-alternate-evolution-world-lab.md). Không đánh dấu task hoàn
tất chỉ vì code merged; phải gắn output test/report tương ứng.
