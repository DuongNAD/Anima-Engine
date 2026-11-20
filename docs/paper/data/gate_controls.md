---
title: Negative controls for automated gates (M2)
status: active
owner: maintainers
last_reviewed: 2026-11-20
review_cycle: per-release
---

# Negative controls for every automated gate (Card M2)

**Question.** When each automated gate's target fault is injected into a clean detached checkout, does the gate fail loudly (`exit != 0`) and name the fault in its output, or does it stay green (`VACUOUS`)?

- **Date measured:** `2026-11-20` (UTC)
- **Merged trunk base (`origin/feat/ecological-selection`):** `d90be0bd26fe637e2a3db1ce0c238845cf4211fd`
- **Measured code commit (`dell/m2-gate-controls`):** `408cf448c34dbc90d4ef5264f64e6668bff1650d` (`git status --porcelain` empty before and after)
- **Execution environment:** Quiet Dell laptop (`DESKTOP-IT8UKCD`, `0` concurrent `cargo`/`rustc` processes, `AGENT_RULES.md` §5 Dell guard, `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0`, `ANIMA_USE_GPU=0`, Windows `Idle` process priority).
- **Full suite summary (`node scripts/paper/gate_controls.mjs`):** `55` rows total (`6673.9 s` / `111.2 min` wall time, `2026-11-20T07:44:35.2115915Z` to `2026-11-20T09:35:49.1114507Z`, `exit = 0`): **`5` `VACUOUS`**, **`1` `NOT RUN (no automated gate)`**, **`2` `NOT RUN (time)`**, **`47` `CATCHES`**.

---

## Findings (listed first; reported only, not fixed in Card M2)

1. **`thread_rng_scan` is `VACUOUS` (`hardening.yml:75-76`, exit `0`).** The scan greps only for the literal substring `rand::thread_rng()`, so `use rand::thread_rng; let _ = thread_rng();` in `src-tauri/src/main.rs` reports `thread_rng() literal matches: 0` and exits `0`.
2. **`check_test_targets_unscheduled` is `VACUOUS` (`ci.yml:182`, exit `0`).** `.github/workflows/ci.yml` invokes `node ../scripts/check_test_targets.mjs ../cargo-test-output.txt` without `--profile desktop`. Dropping `--features desktop` from `cargo test` in `.github/workflows/ci.yml:174` omits all `8` feature-gated targets (`adversarial_challenger_tests`, five `migration_*` targets, `phase5_websocket_reuse`, `phase5_burn_wgpu_fallback`), yet the CI command passes with `note: no --profile given, so feature-gated target scheduling was NOT checked. Pass --profile default or --profile desktop.` Passing `--profile desktop` (`check_test_targets_unscheduled_profile`) catches this fault (`exit = 1`).
3. **`ci_ipc_arg_case` is `VACUOUS` (`ci.yml` vs `package.json:34`, exit `0`).** `npm run check:ipc` (`node scripts/check_ipc_arg_case.mjs`) exists and catches a snake_case invoke key (`exit = 1` in `check_ipc_arg_case`), but `.github/workflows/ci.yml` does not invoke it, so the full CI Frontend check chain stays green (`exit = 0`) when `invoke('save_simulation_state', { file_path })` is added to `src/components/Landscape/utils/worldCache.ts`.
4. **`ci_licenses_complete` is `VACUOUS` (`ci.yml:208-211`, exit `0`).** `.github/workflows/ci.yml` runs `npm run check:licenses` without `--require-complete`, so removing `utf8-chars@3.0.7`'s Apache-2.0 election and regenerating `licensing/` leaves `1 unresolved` component in `licensing/UNRESOLVED.md` while CI passes (`exit = 0`). Running `npm run check:licenses-complete` (`check_licenses_complete`) catches the unresolved component (`exit = 1`).
5. **`ci_e3` is `VACUOUS` (`ci.yml` vs `package.json:33`, exit `0`).** `npm run check:e3` (`node scripts/verify_e3_artifacts.mjs`) exists and catches tampering in the committed `artifacts/experiments/e3-productivity-gradient-body-mass/confirmation/effects.json` (`exit = 1` in `check_e3`), whereas `.github/workflows/ci.yml` omits `check:e3` and `tests/frontend/verifyE3Artifacts.test.ts` exercises synthetic temporary directories only, so `npm run check:e2 && npm run test && npm run test:frontend` passes (`exit = 0`).
6. **`conflict_marker_scan` is `NOT RUN (no automated gate)`.** Planned in `CARD M2` (`docs/ai/planning/2026-11-05-swarm-prompts.md`), with no automated check present in `.github/workflows/ci.yml`, `.github/workflows/hardening.yml`, or `package.json`.

---

## Gate control table (`VACUOUS` first, then `NOT RUN`, then `CATCHES`)

`SUMMARY total=55 VACUOUS=5 CATCHES=47 NOT_RUN_NO_GATE=1 NOT_RUN_TIME=2 NOT_RUN_BASELINE_RED=0`

| gate | CI command | fault | exit code | naming line (verbatim) | verdict |
|---|---|---|---|---|---|
| `thread_rng_scan` | `n=$(grep -rn 'rand::thread_rng()' src-tauri/src --include='*.rs' \| grep -vc ':[[:space:]]*//' \|\| true); [ "$n" -eq 0 ]` | Add `use rand::thread_rng; let _ = thread_rng();` to `src-tauri/src/main.rs` | 0 | `thread_rng() literal matches: 0` | **VACUOUS** |
| `check_test_targets_unscheduled` | `node ../scripts/check_test_targets.mjs ../cargo-test-output.txt` | Drop `--features desktop` from `cargo test` in `.github/workflows/ci.yml:174` | 0 | `note: no --profile given, so feature-gated target scheduling was NOT checked. Pass --profile default or --profile desktop.` | **VACUOUS** |
| `ci_ipc_arg_case` | `npm run lint && node scripts/eslint_ratchet.mjs && npm run typecheck:tests && npm run test && npm run test:frontend && npm run build && node scripts/check_ipc_reference.mjs` | Add `invoke("save_simulation_state", { file_path })` (snake_case) helper in `src/components/Landscape/utils/worldCache.ts` (`check:ipc` absent from `ci.yml`) | 0 | `ipc reference check: 36 commands, 7 events, 0 differences` | **VACUOUS** |
| `ci_licenses_complete` | `npm run check:licenses` | Remove `utf8-chars@3.0.7` election from `licensing/upstream/sources.json` and regenerate `licensing/` (`ci.yml` step "current and complete" omits `--require-complete`) | 0 | `third-party licences check: up to date (545 distributed components, 296 distinct texts, 0 elected, 1 unresolved; election self-test 41 controls ok)` | **VACUOUS** |
| `ci_e3` | `npm run check:e2 && npm run test && npm run test:frontend` | Alter `residual` for seed `731001` in `artifacts/experiments/e3-productivity-gradient-body-mass/confirmation/effects.json` (`check:e3` absent from `ci.yml`) | 0 | `Test Files 14 passed (14)` | **VACUOUS** |
| `conflict_marker_scan` | `(no automated check in ci.yml, hardening.yml, or package.json)` | Conflict-marker scan planned in CARD M2 without implementation | — | `NOT RUN (no automated gate in ci.yml, hardening.yml, or package.json)` | **NOT RUN (no automated gate)** |
| `cargo_test_desktop_full` | `cargo test --features desktop -- --nocapture 2>&1 \| tee ../cargo-test-output.txt` | Full all-target desktop test run per fault exceeds 30 min budget (targeted suites below cover faults) | — | `NOT RUN (time: quiet-machine clean compile took 30m 13s and partial run took 38m 08s [2288.12s] across 52 targets before environmental_elements_stress_tests hit 65.1366ms vs <50ms debug timing limit under Idle priority; >30 min per fault)` | **NOT RUN (time)** |
| `hardening_stability_5x` | `for i in 1 2 3 4 5; do RAYON_NUM_THREADS=1 cargo test --release --quiet; done` | 5x sequential full release test runs per fault exceed 30 min budget | — | `NOT RUN (time: single quiet-machine cargo test --release --quiet took 56m 38s [3398.22s; 159 targets, 1583 passed, 2 ignored, test_exec=264.87s]; 5x implies ~74m+ [compile + 5*test_exec] or ~283m cold, >30 min per fault)` | **NOT RUN (time)** |
| `check_licenses_complete` | `npm run check:licenses-complete` | Remove `utf8-chars@3.0.7` election from `licensing/upstream/sources.json` and regenerate `licensing/` | 1 | `1 distributed component(s) have no obtainable licence text. See licensing/UNRESOLVED.md.` | **CATCHES** |
| `cargo_fmt` | `cargo fmt --check` | Misformat `anima_engine_lib::run( );` in `src-tauri/src/main.rs` | 1 | `Diff in \\?\C:\Users\DELL\AppData\Local\Temp\anima-m2-wt\src-tauri\src\main.rs:1:` | **CATCHES** |
| `cargo_audit` | `cargo audit` | Remove `RUSTSEC-2026-0367` exemption from `src-tauri/.cargo/audit.toml` | 1 | `ID: RUSTSEC-2026-0367` | **CATCHES** |
| `cargo_deny` | `cargo deny check` | Drop `"MIT"` from `licenses.allow` in `src-tauri/deny.toml` | 4 | `error[rejected]: failed to satisfy license requirements` | **CATCHES** |
| `cargo_clippy_desktop` | `cargo clippy --all-targets --features desktop -- -D warnings` | Add `let unused_m2_fault = 1usize;` in `src-tauri/src/main.rs` | 101 | `` error: unused variable: `unused_m2_fault` `` | **CATCHES** |
| `feature_separation` | `cargo clippy --all-targets --no-default-features -- -D warnings && leaked=$(cargo tree --no-default-features -e normal \| grep -cE 'neo4rs\|tokio-tungstenite\|burn-wgpu\|naga' \|\| true); test "$leaked" -eq 0` | Make `tokio-tungstenite` non-optional in `src-tauri/Cargo.toml` | 1 | `gated crates present in a default build: 1` | **CATCHES** |
| `ts_rs_parity` | `cargo test --lib export_bindings && git diff --exit-code -- ../src/types/generated` | Add `#[ts(rename = "x_m2_fault")]` to `SegmentState::x` in `simulation_state.rs` | 1 | `+length: number, radius: number, x_m2_fault: number, y: number, z: number, yaw: number, pitch: number, roll: number, joint_anchor_x: number, joint_anchor_y: number, joint_anchor_z: number, joint_axis_x: number, joint_axis_y: number, joint_axis_z: number, energy: number, hydration: number, head_direction: [number, number, number], agent_type: AgentType \| null, };` | **CATCHES** |
| `check_test_targets_empty` | `node ../scripts/check_test_targets.mjs ../cargo-test-output.txt` | Add `#![cfg(any())]` at top of `src-tauri/tests/shared_domain_law_tests.rs` | 1 | `- test target "tests/shared_domain_law_tests.rs" compiled but ran zero tests, and is not in EMPTY_TARGETS.` | **CATCHES** |
| `check_test_targets_ignored` | `node ../scripts/check_test_targets.mjs ../cargo-test-output.txt` | Add `#[ignore]` to `the_engine_reads_the_domain_crates_values_for_the_time_laws` | 1 | `- test "the_engine_reads_the_domain_crates_values_for_the_time_laws" (tests/shared_domain_law_tests.rs) is #[ignore]d and is not in IGNORED_TESTS.` | **CATCHES** |
| `check_test_targets_unscheduled_profile` | `node ../scripts/check_test_targets.mjs ../cargo-test-output.txt --profile desktop` | Drop `--features desktop` from `cargo test` in `.github/workflows/ci.yml:174` (checked with `--profile desktop`) | 1 | `` - profile "desktop" enables "networking", so tests/adversarial_challenger_tests.rs must run — but it is absent from this capture. Check its `required-features` spelling in Cargo.toml: a feature name Cargo does not recognise makes a target vanish silently. `` | **CATCHES** |
| `feature_gated_targets` | `cargo test --test feature_gated_targets_tests` | Mistype `required-features = ["networking_m2_typo"]` on `migration_tests` in `src-tauri/Cargo.toml` | 101 | `` test target `migration_tests` requires feature `networking_m2_typo`, which [features] does not declare. Cargo would silently never build this target. `` | **CATCHES** |
| `schedule_ambiguity_simrng` | `cargo test --test schedule_ambiguity_tests` | Put `(spawn_food_system, seed_dropping_system)` in parallel in `simulation_schedule.rs` | 101 | `` `ResMut<SimRng>` serialises draws, it does not order them — two orders give two different random sequences per process. Declare the order in `core::simulation_schedule::build_tick_schedule`. `` | **CATCHES** |
| `schedule_ambiguity_energy` | `cargo test --test schedule_ambiguity_tests` | Put `(combat_system, metabolic_decay_system)` in parallel in `simulation_schedule.rs` | 101 | `ENERGY combat_system <-> metabolic_decay_system (2 conflicting components)` | **CATCHES** |
| `zero_alloc` | `cargo test --test emit_zero_alloc_tests` | Allocate `Vec::<u8>::with_capacity(64)` in `refresh_tick_payload` (`emit.rs`) | 101 | `test emit_path_is_allocation_free ... FAILED` | **CATCHES** |
| `closed_eu` | `cargo test --test energy_conservation_tests` | Inject `+ 1.0` EU per entity in `metabolic_decay_system` (`world_systems.rs`) | 101 | `eating food changed the closed total by 10 — food is minting energy` | **CATCHES** |
| `live_experiment_fork` | `cargo test --test live_experiment_tests` | Pass control `manifest.interventions` instead of `effective_treatment.interventions` in `checkpoint_fork_with_exotic` (`experiment_runner/fork.rs`) | 101 | `` assertion `left != right` failed: the treatment factor must change the branch `` | **CATCHES** |
| `live_experiment_saveload` | `cargo test --test live_experiment_tests` | Reset `sim_rng_pos` to `0` on restore in `restore_energy_state` (`simulation_state.rs`) | 101 | `` assertion `left == right` failed: a live run resumed from a checkpoint diverged from an uninterrupted one; some piece of trajectory-relevant state is missing from the snapshot `` | **CATCHES** |
| `determinism_golden_hash` | `cargo test --release --test determinism_tests` | Perturb `SimRng::for_world(world_identity.seed.wrapping_add(1))` in `core/ecs.rs` | 101 | `` assertion `left == right` failed: world_hash doi -> sim mat tinh tat dinh hoac hanh vi da doi `` | **CATCHES** |
| `check_licenses` | `npm run check:licenses` | Flip 1 hex char in `sourceSha256` inside `licensing/third-party-index.json` | 1 | `licensing/ is out of date with the dependency graph: third-party-index.json.` | **CATCHES** |
| `check_sbom` | `npm run check:sbom` | Change `metadata.component.name` to `anima-engine-m2-fault` in `sbom.cdx.json` | 1 | `` sbom.cdx.json is out of date. Regenerate with `node scripts/gen_sbom.mjs` and commit it. `` | **CATCHES** |
| `check_sbom_schema` | `npm run check:sbom-schema` | Add `metadata.timestamp` disallowed by CycloneDX 1.5 strict check in `sbom.cdx.json` | 1 | `x metadata.timestamp is present; the SBOM is generated without one so that it is reproducible` | **CATCHES** |
| `check_notice` | `npm run check:notice` | Delete `- **Rust — ... crates.**` bullet from `NOTICE` | 1 | `` NOTICE is out of date with the dependency graph. Run `node scripts/gen_notice.mjs`. `` | **CATCHES** |
| `check_flora_footprint` | `npm run check:flora-footprint` | Change `FLORA_FOOTPRINT_UNIT_RADIUS[FloraType.Pine]` to `0.99` in `floraClearance.ts` | 1 | `Pine declared 0.9900 measured 0.5500 drift 4.4e-1` | **CATCHES** |
| `verify_dataflow` | `cargo modules dependencies --lib --no-externs --package anima-engine > ../dataflow-truth.dot && python3 ../scripts/verify_dataflow.py ../dataflow.json ../dataflow-truth.dot 1` | Insert invented `physics -> commands` edge into `dataflow.json` | 1 | `physics -> commands` | **CATCHES** |
| `check_npm_audit` | `node scripts/check_npm_audit.mjs` | Add stale withdrawn exemption `GHSA-fc5q-39xc-vjpf` to `scripts/npm_audit_policy.mjs` | 1 | `- policy entry GHSA-fc5q-39xc-vjpf (picomatch) matches no advisory in any package: it is stale, remove it from scripts/npm_audit_policy.mjs` | **CATCHES** |
| `check_text_hygiene` | `npm run check:text-hygiene` | Embed NUL byte `\x00` inside `scripts/npm_audit_policy.mjs` via binary patch | 1 | `x scripts/npm_audit_policy.mjs: control byte(s) at line 1 (byte offset 6)` | **CATCHES** |
| `check_root_hygiene` | `npm run check:root-hygiene` | Add unreviewed `unreviewed_m2_fault.txt` at repository root | 1 | `- unreviewed_m2_fault.txt: at the repository root but not in scripts/root_allowlist.json` | **CATCHES** |
| `check_e2` | `npm run check:e2` | Alter `control_mean` in `artifacts/experiments/e2-evolved-brain-default/effects.json` | 1 | `FAIL analysis: effects.json hashes to 69554db4eb6732f579b21b2f82d0fcc0f895520e8a253a69837c17c660684e91, but checksums.sha256 says 05902fd1047578176683ed3141fc31a7846924d600b24711745ff8ce54bb4453` | **CATCHES** |
| `check_e3` | `npm run check:e3` | Alter `residual` for seed `731001` in `artifacts/experiments/e3-productivity-gradient-body-mass/confirmation/effects.json` | 1 | `- SHA-256 mismatch for effects.json: expected f63f984d89f84e00f04cba3a62204c66dd121621ecb824150578d889bd5b716c, got 4dd55ed0da98efa29e8c1543f42a995b568b9e3e2b961564e818d78b45a3e909` | **CATCHES** |
| `npm_lint` | `npm run lint` | Add empty statement `if (Boolean(1)) {}` to `src/main.tsx` | 1 | `16:17 error Empty block statement no-empty` | **CATCHES** |
| `eslint_ratchet` | `node scripts/eslint_ratchet.mjs` | Add `export const _m2AnyFault: any = 1;` to `src/main.tsx` | 1 | `eslint: 0 errors, 1 warnings (baseline 0)` | **CATCHES** |
| `typecheck_tests` | `npm run typecheck:tests` | Add `export const _m2TypeFault: number = 'bad';` to `tests/frontend/mapManifestEvidence.test.ts` | 2 | `tests/frontend/mapManifestEvidence.test.ts(228,14): error TS2322: Type 'string' is not assignable to type 'number'.` | **CATCHES** |
| `vitest_src` | `npm run test` | Break `FLORA_COLLIDER_BASE_RADIUS` assertion in `src/__tests__/floraClearance.test.ts` | 1 | `FAIL src/__tests__/floraClearance.test.ts > flora clearance policy — the shared definition of "a tree is in the way" > keeps the walk-mode collider radius the rig already used` | **CATCHES** |
| `vitest_tests` | `npm run test:frontend` | Invert `expect(res.ok).toBe(false)` in `tests/frontend/mapManifestEvidence.test.ts` | 1 | `FAIL frontend/mapManifestEvidence.test.ts > map manifest — the committed file, not a copy of it > passes its own validator` | **CATCHES** |
| `npm_build` | `npm run build` | Add `export const _m2BuildFault: number = 'bad';` to `src/main.tsx` | 2 | `src/main.tsx(16,14): error TS2322: Type 'string' is not assignable to type 'number'.` | **CATCHES** |
| `check_bundle_closure` | `npm run build && npm run check:bundle-closure` | Import dev-only `ajv` into `src/main.tsx` so bundle closure shifts | 1 | `licensing/bundle-closure.json differs from the committed copy after a build. The set of npm packages with bytes in` | **CATCHES** |
| `check_csp` | `npm run build && npm run check:csp` | Add external `<script src="https://example.com/m2-fault.js"></script>` to `index.html` | 1 | `- dist\index.html: loads an external origin, which default-src 'self' blocks: src="https://example.com/m2-fault.js"` | **CATCHES** |
| `check_bundle` | `npm run build && npm run check:bundle` | Eagerly import `three` into dashboard root `src/App.tsx` | 1 | `- defineProperty is 501.9 KiB, budget 170 KiB` | **CATCHES** |
| `check_e2e_specs` | `node scripts/check_e2e_specs.mjs` | Narrow `tests/package.json` `test:e2e` script to `e2e/console_hygiene.spec.ts` | 1 | `- SPEC MISSING FROM RUN: committed list names tests/e2e/ipc_contract.spec.ts but test:e2e does not schedule it` | **CATCHES** |
| `playwright_e2e` | `npm run test:e2e` | Change `expect(owned.length).toBe(0)` to `.toBe(999)` in `tests/e2e/console_hygiene.spec.ts` | 1 | `Error: Anima-owned console output on the dashboard:` | **CATCHES** |
| `check_docs_links` | `node scripts/check_docs_links.mjs` | Append broken markdown link labeled `m2-broken` targeting `./m2-missing-target.md` to `docs/README.md` | 1 | `docs/README.md:87: broken link -> ./m2-missing-target.md` | **CATCHES** |
| `check_docs_metadata` | `node scripts/check_docs_metadata.mjs` | Set `status: invalid-m2` in `docs/README.md` front matter | 1 | `docs/README.md: status "invalid-m2" not in {proposed, active, accepted, completed, deprecated, superseded}` | **CATCHES** |
| `check_ipc_reference` | `node scripts/check_ipc_reference.mjs` | Insert nonexistent `m2_ghost_cmd` row in `docs/reference/IPC_CONTRACT.md` | 1 | `` docs/reference/IPC_CONTRACT.md: command `m2_ghost_cmd` is documented but not in the code `` | **CATCHES** |
| `check_ipc_arg_case` | `npm run check:ipc` | Add `invoke("save_simulation_state", { file_path })` (snake_case) helper in `src/components/Landscape/utils/worldCache.ts` | 1 | `` FAIL src\components\Landscape\utils\worldCache.ts:241 invoke('save_simulation_state', { file_path: ... }) — the command declares `file_path` and #[tauri::command] defaults to camelCase, so it expects `filePath`. As written the argument never arrives. `` | **CATCHES** |
| `check_map_vision_manifest` | `npm run check:map-vision-manifest` | Set `seaLevel: 0.99` in `animal-map.manifest.json` | 1 | `` animal-map manifest is stale at C:\Users\DELL\AppData\Local\Temp\anima-m2-wt\animal-map.manifest.json; run `npm run gen:manifest` `` | **CATCHES** |
| `verify_upstream_licenses` | `npm run verify:upstream-licenses` | Flip 1 hex char in `ts-rs` `sha256` inside `licensing/upstream/sources.json` | 1 | `x sources[0] (github.com/Aleph-Alpha/ts-rs/0ba56be43af1e438053af2b422a82fcff252f7ef/LICENSE): sha256 is db7f7f8e7236a2d0b41b855b2501b7b913caebbcda6ce498637a304ad1706ce1, manifest says 0b7f7f8e7236a2d0b41b855b2501b7b913caebbcda6ce498637a304ad1706ce1` | **CATCHES** |
| `compare_checksums_self_test` | `node scripts/paper/compare_checksums.mjs --self-test` | Force `checksumAgrees = true` in `scripts/paper/compare_checksums.mjs` | 2 | `FAIL a checksum difference is named at its tick` | **CATCHES** |

---

## Quiet-machine single-run baselines for `NOT RUN (time)` gates

Both single-run baselines ran on a quiet machine (`tasklist | findstr /i "cargo rustc"` empty twice 60 s apart, `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0`, `ANIMA_USE_GPU=0`, Windows `Idle` priority) in a detached LF worktree at commit `568d139fa84af3d5c0fa2a75506db03c9cfc8f51` (same Rust sources as `408cf448`):

1. **`cargo_test_desktop_full` (`cargo test --features desktop -- --nocapture`):**
   - UTC window: `2026-11-20T05:42:01.4104674Z` to `2026-11-20T06:20:09.5304561Z` (**`2288.12 s` / `38 m 08 s`** wall time).
   - Compilation alone took **`30 m 13 s`** (`Finished test profile [unoptimized] target(s) in 30m 13s`), already over the 30-minute per-fault cap.
   - At target `#52` (`tests/environmental_elements_stress_tests.rs:186:13`), `environmental_elements_stress_contracts` panicked on its wall-clock debug threshold (`Average tick time should be under 50ms, got 65.1366` at `Idle` priority; Cycles 1–3: `17.8858 ms`, `40.7153 ms`, `65.1366 ms`), aborting before `migration_*`, `phase5_*`, and `doc-tests`. That single-run measurement confirms why full all-target desktop suites are `NOT RUN (time)` per fault while targeted cargo gates cover the individual failure modes.
2. **`hardening_stability_5x` (`cargo test --release --quiet`, `RAYON_NUM_THREADS=1`):**
   - UTC window: `2026-11-20T06:25:41.1968506Z` to `2026-11-20T07:22:19.4195340Z` (**`3398.22 s` / `56 m 38 s`** wall time, **`exit = 0`**).
   - Result: `159` test targets/doctests, `1583 passed; 0 failed; 2 ignored`; sum of per-target execution times = **`264.87 s`** (`~4.41 min`), implied compile/link/setup = **`~3133.35 s`** (`~52.22 min`).
   - Implied 5-iteration lower bound: `~4457.7 s` (`~74.3 min` with 1 fault recompile + `5 × 264.87 s` execution) or `~16,991.1 s` (`~283.2 min` uncached), well above the 30-minute per-fault cap.

---

## Command to reproduce

From the repository root with a clean git working tree, `node_modules` and `tests/node_modules` installed, pinned Rust toolchain (`1.98.1`), `cargo-audit`, `cargo-deny`, `cargo-modules`, and `python` available:

```bash
node scripts/paper/gate_controls.mjs
```

Individual controls can be inspected with `--gate <id1>,<id2>` and the full list with `--list`. Each control injects one patch from [`scripts/paper/gate_faults/`](../../../scripts/paper/gate_faults/) into a temporary detached worktree under `os.tmpdir()`, leaving the repository clean. On Windows hosts (`< 24 GiB` RAM), `scripts/paper/gate_controls.mjs` enforces `AGENT_RULES.md` §5 before every cargo command (`0` `cargo`/`rustc` processes twice 60 s apart, `>= 5 GiB` free RAM, `>= 20 GiB` free on the target drive, `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, low priority).

---

## Machine and toolchain specification

Measured inside the `m2` worktree (`E:\03_Development_Projects\project\anima-wt\m2`) on `2026-11-20`:

| Component | Measured command & value |
|---|---|
| CPU | `13th Gen Intel(R) Core(TM) i7-1355U` (`NumberOfCores = 10`, `NumberOfLogicalProcessors = 12`, `MaxClockSpeed = 1700` MHz via `Get-CimInstance Win32_Processor`) |
| Memory | `TotalVisibleMemorySize = 16454352` KB (`15.69 GiB` via `Get-CimInstance Win32_OperatingSystem`) |
| OS | `Microsoft Windows 11 Home Single Language`, `Version = 10.0.26300`, `BuildNumber = 26300` |
| Power | `BatteryStatus = 2` (AC mains), `EstimatedChargeRemaining = 100` (`Get-CimInstance Win32_Battery`) |
| Rust | `rustc 1.98.1 (48a229cea 2026-09-01)`, `cargo 1.98.1 (797e8a9bc 2026-08-05)` |
| Node / npm | `v22.22.2` / `11.5.2` |
| Python / Git | `Python 3.13.3` / `git version 2.48.1.windows.1` |

---

## Scope and limitations

- All runs were executed on a single Windows host (`DESKTOP-IT8UKCD`). Cross-OS CI behavior (such as `ubuntu-latest` Frontend / hardening runners) was mirrored locally using the exact commands in `.github/workflows/ci.yml`, `.github/workflows/hardening.yml`, `.github/workflows/determinism-matrix.yml`, and `package.json` as of trunk `d90be0bd26fe637e2a3db1ce0c238845cf4211fd`.
- Standalone verification scripts `scripts/verify_sel2p_artifacts.mjs` and `scripts/verify_esel1_artifacts.mjs` exist in `scripts/` without entries in `package.json` or `.github/workflows/ci.yml` and are outside the `ci.yml` / `package.json` gate inventory covered here.
- `check_test_targets_empty` and `check_test_targets_ignored` run the 12 policy test targets plus `--workspace --doc` (`POLICY_TEST_TARGETS` in `scripts/paper/gate_controls.mjs`) rather than all 153 desktop targets because full all-target desktop compilation/testing exceeds 30 minutes per fault on this machine.
