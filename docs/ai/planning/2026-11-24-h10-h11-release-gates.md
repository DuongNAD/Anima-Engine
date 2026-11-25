---
phase: planning
feature: v1-vertical-slice
title: Checklists H10 and H11 — Map Gate and v1.0 Release Gate
description: Execution checklists, trunk gaps, and negative controls for H10 (animal-map-vision before/after gate, M11.5 / S66 / §13) and H11 (v1.0.0 release gate, HT5.3)
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Checklists `H10` & `H11` — Map Gate (`M11.5`, `S66`) & v1.0 Release Gate (`HT5.3`)

> **Design / release-gate checklists only (`status: proposed`).** No Rust, TypeScript, release tags, or
> `docs/ai/README.md` edits are made in D8 (`V-SPECS-2`).
> All trunk `file:line` anchors below were checked against `origin/feat/ecological-selection` at
> `8d52341549007a50ce033a6d272c5dd22a71b1b1`. Anything not built on trunk yet is labeled
> **`to be created by H10`** / **`to be created before H11`**.

---

## Part A — `H10`: Mandatory Map Gate (`M11.5`, `S66`, `WORLD_SIMULATION_PLAN.md` §13)

### 1. Goal, Dependencies, `START WHEN` & Scope

Run the mandatory visual, manifest, navigation, collision, water, lighting, and ecological placement review (`WORLD_SIMULATION_PLAN.md:915, 1007, 1197-1239`, `M11.5`, `S66`, `2026-11-05-swarm-master-plan.md` Đợt 6 `H10`) with **`before`/`after` image comparison** using the `animal-map-vision` MCP flow and desktop/renderer inspection before `H11` can claim v1.0 readiness.

- **Depends directly on:** **[Card `V8`](2026-11-24-v8-slice-acceptance-spec.md)** (vertical-slice watershed ecosystem finalized; plus any active `WV2-*` Map v2 renderer world per `D-H10-1`).
- **`START WHEN`:** `V8` is merged on trunk, a review session with live `animal-map-vision` MCP access (`WORLD_SIMULATION_PLAN.md:1238`) is open, and `D-H10-1`–`D-H10-3` are settled.
- **Unblocks:** `H11` v1.0 release gate.

### 2. Current Trunk Anchors (`8d523415`) & Honest Gaps

1. **§13 Mandatory 4-Step MCP Sequence (`WORLD_SIMULATION_PLAN.md:1199-1205`):**
   - `1. discover_map_artifacts`
   - `2. validate_map_manifest`
   - `3. prepare_team_review`
   - `4. inspect_map_views`
2. **Eight required views if present (`WORLD_SIMULATION_PLAN.md:1207-1216`):**
   `overview`, `navigation`, `collision`, `lighting`, `spawn`, `water`, `biome transition`, `ecosystem`.
3. **Structured finding schema (`WORLD_SIMULATION_PLAN.md:1218-1227`):** Every finding must include `severity`, `image path`, `region`, `observed evidence`, separated `hypothesis`, `gameplay/ecology impact`, `proposed fix`, and `before/after reproduction check`.
4. **Six non-negotiable completion blockers (`WORLD_SIMULATION_PLAN.md:1229-1236`):** Map work cannot be declared complete if: (a) manifest fails, (b) any `critical`/`high` finding remains open, (c) canonical `before`/`after` views have not been inspected, (d) navigation is unreachable, (e) render/collider/navmesh/simulation/minimap disagree, or (f) known ecological placement contradictions remain.
5. **Existing trunk map tooling & documented gaps:**
   - **Map manifest exporter:** `scripts/gen_map_manifest.ts:1-40, 327, 371-386` generates `animal-map.manifest.json` from real `generateWorld` data (checked via `npm run check:map-vision-manifest`). **Known trunk gap (`gen_map_manifest.ts:15-16, 21`):** `pipeline.panorama` is still a placeholder because Anima renders a 3D mesh rather than an equirectangular panorama.
   - **Canonical fixed-pose capture:** `tests/e2e/canonical_views.spec.ts:19-50` (`npm run capture:views` in `package.json`) produces 8 byte-identical WebGL renders from Vite `landscape.html` across two fresh browser contexts, checked via `tests/frontend/mapManifestEvidence.test.ts:1-20` (`scripts/e2e_spec_policy.mjs:35-40`).
   - **Visual before/after prototype helper:** `scripts/capture_map.mjs:1-31` and [`MAP_VISUAL_CHECK.md`](../../how-to/MAP_VISUAL_CHECK.md) `:13-45` (`npm run capture:map -- --label before` / `--label after --baseline ...`) capture ~8 world-derived views. **Critical honesty limits (`capture_map.mjs:15-22`, `MAP_VISUAL_CHECK.md:13, 37-43`):** `capture:map` is **not** a pass/fail gate, uses synthetic `?replay=synthetic` creatures (not live sim output), and inspects Vite Chrome rather than the shipped Tauri webview.
   - **MCP availability gap (`WORLD_SIMULATION_PLAN.md:1238-1239`):** Standard repo/headless sessions do not carry `animal-map-vision` MCP unless explicitly connected in the review session; `H10` requires that live MCP session or explicit human verification before closing `S66`.

### 3. `H10` Execution Checklist (Agent + Human)

- [ ] **Preflight MCP & manifest parity:** Verify `animal-map-vision` MCP server is active; run `npm run check:map-vision-manifest` (`scripts/gen_map_manifest.ts:386`) and confirm `exit 0` on the target world (`D-H10-1`, addressing `pipeline.panorama` per `D-H10-2`).
- [ ] **Run `animal-map-vision` 4-step pipeline (`WORLD_SIMULATION_PLAN.md:1202-1205`):** Execute `discover_map_artifacts` -> `validate_map_manifest` -> `prepare_team_review` -> `inspect_map_views` covering all `8` canonical view categories (`:1209-1216`).
- [ ] **Capture `before` and `after` canonical views:** Run `npm run capture:views` (`canonical_views.spec.ts:19`) for deterministic byte-identical reference PNGs and `npm run capture:map -- --label before` / `--label after --baseline ...` (`MAP_VISUAL_CHECK.md:15-21`) across any map geometry/shader/biome change.
- [ ] **Audit placement & cross-layer agreement:** Verify terrestrial vs aquatic placement (`S27`), walk-mode tree colliders (`gen_map_manifest.ts:18-20`), BFS navmesh reachability (`:13-14`), and render/collider/simulation/minimap parity (`:1235`), filing any finding with all 8 schema fields (`:1218-1227`).
- [ ] **Zero critical/high gate (`S66`):** Confirm `0` remaining `critical`/`high` findings, resolve/re-capture all `before`/`after` pairs, and archive summary + report path (`D-H10-3`).

### 4. Open Owner Decisions (`D-H10-1`–`D-H10-3`)

| Slot | Question | Options | Recommendation |
|---|---|---|---|
| **`D-H10-1`** | Target world audited in `H10` for v1.0 release: | **(A)** Trunk v1 watershed (`generateWorld` / `animal-map.manifest.json`) matching `V8`.<br>**(B)** World v2 (`ANMW` v3 `Map2` pipeline from `WV2-3..7`). | Resolve alongside `D-V8-4` & `D-H11-3` once `WV2-3..7` vs vertical-slice shipping order is set.<br>**DECIDED (2026-11-24, QD-OPENQ): (A).** |
| **`D-H10-2`** | Handling `pipeline.panorama` placeholder (`gen_map_manifest.ts:15, 21`) during `validate_map_manifest`: | **(A)** Update manifest exporter/schema cleanly (`to be created by H10` if MCP accepts 3D perspective/cubemap or optional panorama).<br>**(B)** Keep explicit exception waiver in `H10` report if `animal-map-vision` validates non-panorama 3D views cleanly. | **(A)** Prefer resolving/documenting schema support cleanly with `animal-map-vision` MCP before `H10` sign-off.<br>**DECIDED (2026-11-24, QD-OPENQ): (A).** |
| **`D-H10-3`** | Where should `H10` inspection findings & `before`/`after` proof live? | **(A)** Committed audit note under `docs/ai/testing/` (images local/archived or lightweight contact sheet).<br>**(B)** Hub `test_runs/` report only. | **(A)** Short committed summary in `docs/ai/testing/` referencing manifest hash/seed so `STATE_OF_THE_PROJECT.md` can link cleanly.<br>**DECIDED (2026-11-24, QD-OPENQ): (A).** |

---

## Part B — `H11`: v1.0 Release Gate Checklist (`HT5.3`, Human-Owned)

### 1. Goal, Dependencies, `START WHEN` & Scope

Complete every automated, compliance, packaging, real-backend E2E, human desktop trial, and authoritative `STATE §1` re-measurement requirement before cutting git tag `v1.0.0` (`HT5.3` in [`COMPLETION_ROADMAP_2026-10-16.md`](../../planning/COMPLETION_ROADMAP_2026-10-16.md) `:321-323`, `2026-11-05-swarm-master-plan.md` Đợt 6 `H11`, extending Stage 5 [`RELEASE_CHECKLIST.md`](../../how-to/RELEASE_CHECKLIST.md) `:29-153` and [`0.9.0.md`](../../release/0.9.0.md)).

- **Depends directly on:** All of **`V8`**, **`V9`**, **`V10`**, **`V11`**, and **`H10`** complete and green on trunk.
- **`START WHEN`:** `V8`–`V11` + `H10` merged/closed and `D-H11-1`–`D-H11-4` decided by the owner.
- **Role:** **Human gate (`🧑`)** — agents may verify individual suites on qualified hosts and prepare draft state updates, while final desktop run, installer validation, `main` sync, and `v1.0.0` tagging require explicit owner sign-off.

### 2. Current Trunk Anchors (`8d523415`) & Critical Blocker Before `H11`

- **Authoritative Stage 5 prerequisite checklist:** [`RELEASE_CHECKLIST.md`](../../how-to/RELEASE_CHECKLIST.md) `:29-153` (Node/docs/lint/ratchet/Vitest/Vite/bundle-closure/CSP/E2E/doc-commands/E2/E3/E3b + Rust fmt/audit/deny/clippy/test-target-policy/dataflow/release build + Windows compliance regen + unsigned installers `.github/workflows/release-build.yml` documented in [`2026-11-24-rel09-ci.md`](2026-11-24-rel09-ci.md)).
- **CRITICAL REAL-BACKEND E2E GAP ON TRUNK (`tests/e2e/real_backend.spec.ts:1-67`, `scripts/e2e_spec_policy.mjs:43-48`):**
  - `COMPLETION_ROADMAP_2026-10-16.md:323` (`HT5.3`) requires **E2E with real backend (`ANIMA_E2E_REQUIRE_BACKEND=1`, release binary + `tauri-driver`)**.
  - On trunk `8d523415`, `tests/e2e/real_backend.spec.ts:38-66` intentionally fails closed when `ANIMA_E2E_REQUIRE_BACKEND=1` if `ANIMA_E2E_WEBDRIVER_URL` is unset, and lines `:52-53` explicitly warn: `"Driving the real app means tauri-driver plus a WebDriver-capable runner, and neither is wired up in this repository."`
  - **Consequence:** A dedicated real-backend WebDriver harness card (**`to be created before H11`** per `D-H11-1`) **must** wire `tauri-driver` + `ANIMA_E2E_WEBDRIVER_URL` and replace/extend the fail-closed check with real end-to-end assertions against the live Tauri binary before `H11` can pass.
- **Tick capture during human app run (`HT5.3`):** `src-tauri/CLAUDE.md:27-29`, `src-tauri/src/core/simulation_loop.rs:1894-1914`, `src-tauri/src/core/tick_capture.rs:360-394` (`ANIMA_TICK_CAPTURE` & `ANIMA_TICK_CAPTURE_OUT` record real desktop tick diagnostics when launched by human tester; respect `CLAUDE.md:52` low-RAM host warning).

### 3. `H11` Release Sign-Off Checklist (Run Only After Final v1.0 Commit)

- [ ] **All upstream v1.0 cards green:** `V0`–`V11`, `H10` (`S66`), and the real-backend WebDriver harness (`D-H11-1`) merged and verified on trunk.
- [ ] **Full `RELEASE_CHECKLIST.md` re-run (`§1`–`§4`):** Execute all Node hygiene/lint/ratchet/Vitest/build/CSP/bundle/docs/IPC/`npm run check:doc-commands` gates (`RELEASE_CHECKLIST.md:29-69`), all Rust fmt/audit/deny/clippy/dataflow/test-target-policy (`--profile desktop` & `--profile worldgen-cpu`) / `--release` builds (`:73-111`), and Windows compliance regeneration (`npm run check:licenses-complete` & `npm run check:sbom`, `:115-143`).
- [ ] **Real-backend WebDriver E2E (`ANIMA_E2E_REQUIRE_BACKEND=1`):** Launch built release binary + `tauri-driver` + `ANIMA_E2E_WEBDRIVER_URL` and confirm `tests/e2e/real_backend.spec.ts:38-66` executes real IPC/UI flows green with zero skips (`COMPLETION_ROADMAP_2026-10-16.md:323`).
- [ ] **Human desktop app verification (`HT5.3`):** Human launches packaged installer on clean target environment (`QD-M4` / `QD-M8` resolved per `D-H11-2`) with CSP verified and `ANIMA_TICK_CAPTURE` / `ANIMA_TICK_CAPTURE_OUT` trace inspected (`simulation_loop.rs:1894-1914`), confirming `V7` causal timeline & charts show real live/artifact data with zero console/page errors.
- [ ] **Authoritative `STATE_OF_THE_PROJECT.md` `§1` re-measurement (`HT5.3`):** Record exact commit-pinned measurements (hardware, exact counts, exit statuses, release artifact digests, same-OS determinism parity; never pin raw cross-OS `world_checksum`s per `ADR-0008:175, 283` and `src-tauri/tests/cross_platform_checksum_probe.rs:235`).
- [ ] **Version bump, `main` fast-forward & `v1.0.0` release:** Bump `0.1.0`/`0.9.0` -> `1.0.0` across `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json:1-15`, `CITATION.cff`; regenerate installers (`release-build.yml`); sync `main` per `D-H11-4`; tag `v1.0.0` and publish release.

### 4. Applicable [`CLAUDE.md`](../../../CLAUDE.md) Traps & Invariants

- **Never fabricate or copy unmeasured metrics into `STATE §1` (`CLAUDE.md:22, 178-182`):** Every `H11` §1 number must come from fresh runs on the exact tagged/candidate commit on named hardware, stating explicitly headless-adapter vs desktop-app evidence (`CLAUDE.md:179`).
- **Low-RAM Windows host guard (`CLAUDE.md:52`, `RELEASE_CHECKLIST.md:75`):** Never launch full Tauri desktop/Cargo release builds on constrained dev machines (`FreePhysicalMemory < 5 GiB`).
- **No `burn` `Device::default()` / autotune feature widening (`CLAUDE.md:41`):** Re-verify release builds use `Device::flex()` deterministic CPU path unless `ANIMA_USE_GPU` is requested, keeping `LIVE_MODEL_VERSION` (`/2`) clean and `check:licenses-complete` at 0 unresolved (`0.9.0` policy).
- **World laws, zero-alloc, `total_cost`, No Lamarck, checksum honesty (`CLAUDE.md:50, 97-98, 105-107, 145-153`):** Upstream `V8`–`V10` gates prove conservation/zero-alloc/No-Lamarck invariants; `H11` forbids pinning cross-OS checksums in release acceptance gates (`ADR-0008:175, 283`).

### 5. Open Owner Decisions (`D-H11-1`–`D-H11-4`)

| Slot | Gate Question | Options | Recommendation |
|---|---|---|---|
| **`D-H11-1`** | When and by which card is the missing `tauri-driver` + `ANIMA_E2E_WEBDRIVER_URL` real-backend E2E harness built (`real_backend.spec.ts:52-64`)? | **(A)** Schedule a dedicated pre-`H11` engineering card (`E2E-BACKEND`) after `V7`/`V8`.<br>**(B)** Bundle WebDriver harness creation into `H11` prep PRs. | **(A)** Dedicated pre-`H11` card (`E2E-BACKEND`) so `H11` strictly executes verified gates without writing new test-harness code at tag time.<br>**DECIDED (2026-11-24, QD-OPENQ): (A).** |
| **`D-H11-2`** | v1.0 code signing/notarization (`QD-M8`) & officially supported OS matrix (`QD-M4`): | **(A)** Sign/notarize Windows + macOS installers before `v1.0.0`.<br>**(B)** Ship Windows signed + macOS best-effort (or unsigned with advisory per `0.9.0.md:20`). | **(A)** Resolve `QD-M8` & `QD-M4` explicitly prior to `v1.0.0` release notes.<br>**DECIDED (2026-11-24, QD-OPENQ): (A)** (`QD-M8` before `v1.0`; `QD-M4` macOS unsigned if `NM-2` matches per `QD-OPENC`). |
| **`D-H11-3`** | Does `v1.0.0` require World v2 integration (`WV2-3..7`) complete, or can `v1.0.0` ship on the `V0`–`V11` vertical-slice world if `WV2` continues on a separate timeline? | **(A)** Ship `v1.0.0` on unified `WV2` world once `WV2-3..7` merge.<br>**(B)** Gate `v1.0.0` strictly on `V0`–`V11` + `H10` vertical slice (`2026-11-05-swarm-master-plan.md` Đợt 6). | Escalate to owner before starting `H10`/`H11` so `D-V8-4` and `D-H10-1` align cleanly.<br>**DECIDED (2026-11-24, QD-OPENC): (A)** — `v1.0.0` requires World v2 in the app. |
| **`D-H11-4`** | `main` branch sync & public GitHub Release / DOI steps: | **(A)** Fast-forward `main` to the green release candidate commit, cut signed/annotated `v1.0.0` tag, publish installers on GitHub Releases.<br>**(B)** Tag directly on `feat/ecological-selection`. | **(A)** Fast-forward `main` first (`0.9.0.md:22`) so `CITATION.cff` and GitHub Releases point cleanly at `main`.<br>**DECIDED (2026-11-24, QD-OPENQ): (A)** (follows `QD-REL09` G4). |

### 6. `DONE WHEN` & Negative Controls (`A3 #2` for `H10`/`H11`)

- **`DONE WHEN` (`H10` + `H11`):** Every checklist item in Part A (`H10`, `0` critical/high findings, all `8` canonical views inspected `before`/`after`) and Part B (`H11`, full `RELEASE_CHECKLIST.md` green, `ANIMA_E2E_REQUIRE_BACKEND=1` real-backend WebDriver green, human desktop run + `ANIMA_TICK_CAPTURE` verified, `STATE §1` re-measured, `v1.0.0` tagged on synced `main`) is recorded with command outputs and owner sign-off.
- **Negative Controls (Must Fail Red, `A3 #2`):**
  1. **Real-backend E2E without WebDriver:** Run `ANIMA_E2E_REQUIRE_BACKEND=1` with `ANIMA_E2E_WEBDRIVER_URL` unset (`real_backend.spec.ts:58-64`) -> test suite fails loudly instead of skipping.
  2. **Stale map manifest (`check:map-vision-manifest`):** Perturb biome/world generation without re-running `npm run gen:manifest` -> `scripts/gen_map_manifest.ts:382-384` exits `1`.
  3. **Incomplete `check:docs` / `IPC_CONTRACT.md` drift:** Add/remove a Tauri command without updating `IPC_CONTRACT.md` -> `npm run check:docs` exits `1`.
  4. **`#[ignore]` or excluded E2E spec drift:** Add an unapproved `#[ignore]` Rust test or un-reasoned Playwright spec -> `node scripts/check_test_targets.mjs` or `node scripts/check_e2e_specs.mjs` exits `1`.
