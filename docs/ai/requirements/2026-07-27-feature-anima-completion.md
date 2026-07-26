---
phase: requirements
feature: anima-completion
title: Requirements — Completion & Hardening pass
description: Close every verified P1/P2/P3 finding without regressing a scientific contract
status: active
owner: maintainers
last_reviewed: 2026-07-27
state: ../../planning/STATE_OF_THE_PROJECT.md
---

# Requirements — Completion & Hardening pass

Written in English: this is an audit/evidence artifact commissioned by an English directive, and
the contract docs it cites (`AGENTS.md`, `PROJECT.md`) are English. The living status doc
[`STATE_OF_THE_PROJECT.md`](../../planning/STATE_OF_THE_PROJECT.md) stays Vietnamese and remains
the first thing a new session reads.

## Shared understanding

The engine is in good technical shape — 746 backend tests, a gate suite that targets real failure
modes rather than checkboxes, and a closed-EU world that is bit-exact rather than "within tolerance".
What it lacks is **evidence on the default path**, and a handful of artifacts that *claim* evidence
they do not have.

This pass is therefore not a feature. It is the work of making every claim in the repository either
**true and reproducible**, or **explicitly labelled as not yet proven**. Where a claim is false, the
fix is to make it true; where it cannot be made true in this repository, the fix is to say so and
name the blocker.

## Problem statement

Eight verified problems, grouped by what they cost.

### A. Artifacts that assert evidence they do not have

- **A1 — `map_manifest.json` is a false document.** It declares `artifacts/world_128.anmw` with
  checksum `sha256:000…0`, and eight canonical view PNGs under `map-views/`. Neither directory
  exists. Verified 2026-07-27: `ls artifacts/ map-views/` → *No such file or directory*.
- **A2 — the test that should have caught A1 validates a copy of itself.**
  `src/__tests__/mapManifest.test.ts` builds an inline `validManifest()` fixture and never opens
  the committed file. A manifest can rot arbitrarily far without turning the suite red.
- **A3 — Playwright can adopt an unrelated server.** `tests/e2e/playwright.config.ts` hard-codes
  port 5173 and sets `reuseExistingServer: !process.env.CI`, so any Vite server already on 5173 —
  belonging to any project — is silently accepted as the app under test. Specs then convert the
  resulting mismatch into `test.skip()`, so wrong-app runs report as green-with-skips.

### B. A contract constant that no longer describes the system

- **B1 — `DEFAULT_GRID_DIM = 128` contradicts the running world.**
  [`sim_rules.rs:34`](../../../src-tauri/src/core/sim_rules.rs) declares 128 and documents itself as
  "`MapSettings::default`". `MapSettings::default()` is **256 × 256**
  ([`terrain.rs:97`](../../../src-tauri/src/core/terrain.rs)) and has been since the artifact size
  was matched. The constant is read **nowhere** in `src-tauri/src/`.

  The blast radius is documentation, not arithmetic: every transform in `sim_rules.rs`
  (`cell_index`, `cell_center_uv`, `uv_to_cell`, `cell_center_to_world_xz`, `world_xz_to_cell`)
  takes `width`/`height` as parameters. Nothing computes a scale from the constant at runtime.
  But four documents *do*, and they are wrong by a factor of two:
  [`COORDINATE_CONTRACT.md`](../../../COORDINATE_CONTRACT.md) §4 publishes `200 / 128 = 1.5625`
  units-per-cell for "Backend sim (mặc định)" where the truth is `200 / 256 = 0.78125`;
  [`SIMULATION_RULES.md`](../../../SIMULATION_RULES.md) §5, [`MAP_MANIFEST.md`](../../../MAP_MANIFEST.md)
  and `map_manifest.json` repeat the 128.

### C. Security surface left open

- **C1 — `tauri.conf.json` sets `security.csp: null`.** No content-security policy at all.
- **C2 — save/load accept unconstrained paths.** `save_simulation_state` and
  `load_simulation_state` ([`commands/simulation.rs:32,67`](../../../src-tauri/src/commands/simulation.rs))
  take a raw `file_path: String` straight to `snapshot::read`/`write`.
- **C3 — two `unsafe impl` with no proof.** `unsafe impl Send for BrainModel` / `Sync`
  ([`ai/model.rs:360`](../../../src-tauri/src/ai/model.rs)) — the only two `unsafe` blocks in the
  backend, and neither carries a `// SAFETY:` argument. The type owns a `WgpuDevice`.

### D. Backlog the status doc already ranked

- **D1 — lineage memory has no O(alive) bound.** `LineageTracker::compact` runs with compression
  off because `get_mutations_count` derives the UI number by walking `RelationType` per edge; a
  compressed edge represents a *path*, so enabling compression today would silently under-count.
  This is item 1 of [§3.15.1](../../planning/STATE_OF_THE_PROJECT.md).
- **D2 — licensing evidence incomplete.** `LICENSE` is proprietary and does not separate
  code / model / dataset / asset scope; there is no `NOTICE` attributing the permissive components
  that ship in the binary (§3.16). `burn`/`burn-wgpu` are running runtime dependencies absent from
  the open-source inventory matrix (§3.17).
- **D3 — ESLint debt is frozen, not shrinking.** 0 errors, 491 warnings; the ratchet blocks growth
  but never forces a decrease (§3.11).

## Requirements

### Functional

| ID | Requirement |
|---|---|
| R1 | The committed map manifest must reference artifacts that exist, with a checksum computed from those bytes. |
| R2 | A test must load the **committed** manifest from disk, verify its checksum against the real artifact, and fail when a referenced file is missing. |
| R3 | E2E must start its own server on an isolated port and prove the served app is Anima before running a spec. A wrong-app fingerprint is a failure, never a skip. |
| R4 | The declared backend grid dimension must equal the dimension the backend actually runs, in code and in every document that derives a number from it. |
| R5 | Coordinate conversions must be proven at borders, corners, and at non-default dimensions — not only at the default. |
| R6 | The app must run under an explicit, least-privilege CSP. |
| R7 | Save/load must not accept an arbitrary filesystem path from the frontend. |
| R8 | Every `unsafe impl` must carry a proof, be encapsulated safely, or be removed. |
| R9 | `LineageNode` must carry a cumulative mutation count capable of the real range, old saves must keep reading, and compaction must then run with compression enabled. |
| R10 | A `NOTICE` must attribute every permissive component distributed in the binary, and the dependency inventory must include what actually ships. |
| R11 | ESLint must reach 0 errors and 0 warnings without relaxing a rule or raising the ratchet baseline. |

### Non-functional / contract preservation

Nothing below may regress. Each is already machine-checked; the check is named so a reviewer can
re-run it rather than trust this table.

| ID | Contract | Existing gate |
|---|---|---|
| N1 | Determinism for identical seed/config/action stream | `sim_determinism_tests.rs` (incl. the source-scan that bans `thread_rng()`) |
| N2 | Closed energy accounting | `energy_conservation_tests.rs`, bit-exact |
| N3 | Versioned persistence + explicit migration | `snapshot` schema tests; `MIN_SUPPORTED_SCHEMA = SCHEMA_VERSION - 2` |
| N4 | Zero heap allocation in tick systems | `allocs == 0` assertions (EB-S03 and peers) |
| N5 | Rust↔TS contract parity | `ts-rs` generation + `git diff --exit-code` |
| N6 | Feature-split: desktop deps stay out of headless | CI `cargo tree` check |
| N7 | No empty test targets | `scripts/check_test_targets.mjs` |
| N8 | Causal order env → sensors → brain → action → physics → metabolism → telemetry | schedule ordering tests |

## Out of scope, and why

- **Enabling per-agent evolved brains by default (§3.1).** The blocking item is a *decision* about
  re-baselining EB-S04, not an implementation. Recorded as a decision item with alternatives and a
  recommendation; the flag is not flipped without that decision being taken.
- **Live-Bevy experiment readiness (§3.3/§3.6).** Multi-session work requiring `WorldLawSet`,
  `ExperimentManifest`, `CausalLedger` and `SimClock` to become live-engine resources. CLAUDE.md's
  prohibition on claiming the live world is experiment-ready stays in force.
- **Framework upgrades (§3.13).** `burn` 0.13→0.14 breaks the numerically sensitive learner and buys
  nothing on the advisory front (recorded in CLAUDE.md). Not attempted inside a hardening pass.

## Acceptance

Every executable gate in [§5 of the status doc](../../planning/STATE_OF_THE_PROJECT.md) green, plus
the new gates R2/R3/R5/R11 introduced here, with counts recorded in
[the testing doc](../testing/2026-07-27-feature-anima-completion.md) from a fresh run — not quoted
from this document.
