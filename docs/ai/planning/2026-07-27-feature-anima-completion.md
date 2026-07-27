---
phase: planning
feature: anima-completion
title: Plan & Evidence Ledger — Completion & Hardening pass
description: Ordered work packages, each with a gate, an artifact and a commit
status: active
owner: maintainers
last_reviewed: 2026-07-27
requirements: ../requirements/2026-07-27-feature-anima-completion.md
design: ../design/2026-07-27-feature-anima-completion.md
testing: ../testing/2026-07-27-feature-anima-completion.md
---

# Plan & Evidence Ledger — Completion & Hardening pass

> ## 📜 Historical ledger — read the status table, not the numbers
>
> This is the record of **one dated work package** on base `6caeeb4`. Every count in it
> (`746 passed`, `491 warnings`, `75 targets`, …) is a **historical measurement**: it was true when
> the command ran during that package and it does **not** describe the tree today.
>
> **Current measured status lives in exactly one place:**
> [`docs/planning/STATE_OF_THE_PROJECT.md` §1](../../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).
>
> Rows whose *status* has since changed carry a dated **UPDATE** note inline, so a reader is never
> left with a stale `open`.

Branch `feature-anima-completion`, worktree `.worktrees/feature-anima-completion`, base
`6caeeb4`.

## Status legend

`[ ]` not started · `[~]` in progress · `[x]` done, gate green and recorded · `[!]` blocked, blocker
named below.

## 0. Baseline — measured 2026-07-27 at `6caeeb4`

Reproduced, not quoted. Every row is a command run in this worktree on this machine, with the raw
output kept under the session evidence directory and summarised here.

| Gate | Command | Result | Exit |
|---|---|---|---|
| Backend test | `cargo test --features desktop --no-fail-fast` | **746 passed · 0 failed · 4 ignored**, 77 result lines, **0 compile warnings** | 0 |
| Empty targets | `node scripts/check_test_targets.mjs <out>` | **75 targets, 0 empty** | 0 |
| Format | `cargo fmt --check` | clean | 0 |
| DevKit lint | `ai-devkit lint` | all checks passed | 0 |
| DevKit skills | `ai-devkit skill list` | 20 skills, all 19 required present | 0 |

Remaining baseline rows are recorded in [the testing doc](../testing/2026-07-27-feature-anima-completion.md)
as they complete; nothing is marked green here from memory.

**Deviation from the reviewed base's reported numbers: none.** The directive quoted 746/0/4 and that
is what reproduced.

## 1. Work packages, in execution order

Ordered by return, not by difficulty. Each package is a commit or a small run of commits.

| # | Package | Requirement | Definition of done |
|---|---|---|---|
| WP0 | Baseline + lifecycle docs + this ledger | — | Baseline table above filled from fresh runs; requirements/design/planning/testing docs exist |
| WP2 | Grid dimension reconciliation | R4, R5 | `DEFAULT_GRID_DIM` equals `MapSettings::default()`, tied by an assertion; four documents corrected; transform property tests at border/corner/non-default dims green |
| WP5 | Lineage cumulative mutations → compression | R9 | `Option<u32>` field lands with `serde(default)`; UI count identical pre/post compression on one fixture; a v4 save still reads; `compact` reaches the `2·samples` bound |
| WP4 | Security: CSP, save/load, unsafe | R6, R7, R8 | CSP explicit and app-compatible; save/load takes a name resolved under app-data with escape refused and a tested migration reader; both `unsafe impl` proven, encapsulated or removed |
| WP1 | Map manifest becomes evidence | R1, R2 | Manifest generated with a real artifact and real SHA-256; evidence test loads the committed file and checks checksum + existence; MCP gates run on the generated MCP-schema manifest |
| WP3 | E2E isolation + identity | R3 | Own port, own server, Anima fingerprint asserted; wrong app fails rather than skips |
| WP6 | Generated bindings authoritative | — | Handwritten IPC mirrors replaced by generated types; drift fails a gate |
| WP7 | ESLint to zero | R11 | 0 errors, 0 warnings, ratchet baseline lowered, no rule relaxed |
| WP8 | NOTICE + inventory + SBOM | R10 | `NOTICE` generated from the shipped graph; `LICENSE` scope split; inventory includes `burn`/`burn-wgpu`; SBOM reproducible |
| WP9 | Bundle budget gate | — | Heavy 3D deps split; a budget gate fails on regression |

## 2. Evidence ledger

One row per verified finding. `Status` is only `closed` when the named gate has been run **after**
the fix and its output recorded.

| ID | Finding | Severity | Gate that proves it closed | Status | Commit |
|---|---|---|---|---|---|
| A1 | `map_manifest.json` references a non-existent artifact and 8 non-existent PNGs; checksum is 64 zeroes | High | `mapManifestEvidence.test.ts` — opens the committed file, hashes the real artifact **and every captured PNG** | **closed** | `8e6a165`, corrected `6adf6e3`, completed `fd3dc79` |
| A2 | S05 validates an inline fixture, never the committed file | High | same test; proven red by swapping the old manifest back (4 of 7 fail) | **closed** | `8e6a165` |
| A3 | Playwright adopts any server on 5173 and downgrades app mismatch to a skip | High | own port 5177, `reuseExistingServer: false`, identity assertion throws on a wrong app | **closed** | `f99dfc1` |
| B1 | `DEFAULT_GRID_DIM = 128` vs `MapSettings::default()` = 256; four docs publish a 2× wrong scale | High | `s03_default_grid_dim_tracks_map_settings_default`; proven red by reinstating 128 | **closed** | `250e03d` |
| C1 | `security.csp: null` | Medium | explicit `csp` + `devCsp`; `npm run check:csp` asserts the hardening directives, and **now runs in CI** | **closed** (policy unverified in the live webview — external gate, deployment doc §2.1) | `0da67a4`, CI wiring `766609e` |
| C2 | save/load accept an unconstrained path from the frontend | High | `save_paths` allow-list, 11 tests incl. traversal/UNC/ADS/device-name and the legacy-import resolver | **closed** | `378b4f6`, completed `ae1fb40` |
| C3 | 2 `unsafe impl Send/Sync` with no proof | Medium | `backend` private + `from_backend` the only constructor; `brain_model_is_send_without_an_unsafe_impl`; the `Send` impl deleted as redundant | **closed** | `b27c28b`, completed `0d6b4b2` |
| D1 | Lineage memory unbounded; compression off because the mutation count walks edges | High | count identical pre/post compression + negative control that compression is running | **closed** | `57a8246` |
| D2 | No `NOTICE`; `LICENSE` has no scope split; `burn` absent from inventory | Medium | `NOTICE` from `cargo tree --features desktop` and the npm graph; CycloneDX 1.5 SBOM **validated against the official pinned schema**; `licensing/THIRD_PARTY_LICENSES.txt` reproducing the texts; six `check:*` gates in CI | **closed** as an inventory **and** as text packaging for 439 of 440 distributed components (408 installed + 31 vendored from pinned upstream commits); **1 remains unresolved** (upstream never published a licence file for that release) and stays release-blocking; `LICENSE` scope split still open | `14d7abe`, `766609e`, extended this commit |
| D3 | 491 ESLint warnings, frozen by a ratchet that only blocks growth | Medium | 0 warnings, baseline lowered | **closed** — see the UPDATE below | later commit on this branch |

> **UPDATE 2026-07-27 — D3 is closed.** The row above records the state *during this package*
> (491 → 472, from deletions). A later commit on the same branch took it to **0**. Verified
> 2026-07-27 at `2285a92`: `npm run lint` exit 0, and `node scripts/eslint_ratchet.mjs` reports
> **`eslint: 0 errors, 0 warnings (baseline 0)`**. No rule was relaxed, no file excluded and no
> `eslint-disable` added — see `STATE_OF_THE_PROJECT.md` §3.11. §5 below is left as written, and is
> historical.

### 2.2 Supervisor addendum — the independent review after the first session

[`.agents/anima-completion-supervisor-findings.md`](../../../.agents/anima-completion-supervisor-findings.md).
Numbering is that document's. Several of these are defects the first session introduced or failed to
catch, which is why the rows above carry a second commit.

| # | Finding | Gate that proves it closed | Status | Commit |
|---|---|---|---|---|
| 1 | The generated artifact is real bytes for the **wrong world identity** (seed 1337 at 256², not the shipped 2048² `sharedWorld.ts`) | `mapManifestEvidence.test.ts` binds `_generated.{seed,shape,sourceSize}` to `sharedWorld.ts` **and** the artifact's own ANMW header seed | **closed** | `6adf6e3` |
| 1b | `gen_map_manifest.ts` has the same defect — a 100/100 MCP score for an unrelated world | the exporter imports the shared identity; `validate_map_manifest` re-run against the shipped world | **closed** | `6adf6e3` |
| 1c | `npm run build` exits 2 — the evidence test imports `node:*` under the browser tsconfig | test moved to `tests/frontend/`; `npm run build` green | **closed** | `c51ca13` (first session) |
| 2 | Default walking spawn intersects/abuts solid flora; the runtime push-out never resolves a `d2 == 0` overlap | `floraClearance.test.ts` (17 tests) — one policy for spawn/manifest/capture/runtime, zero-distance resolved deterministically, `findSpawn` asserted clear at the shipped 2048² identity | **closed** | `6adf6e3` |
| 3 | Canonical visual evidence missing — `discover` reports 5 missing view kinds | `npm run capture:views` produces all eight from the real renderer; `discover` reports `missingViewKinds: []`; `validate` **pass 100/100, 0 critical/high**; all eight inspected | **closed** | `fd3dc79` |
| 4 | Reset teleports to an unsafe ocean origin | reset returns to the validated `findSpawn` result | **closed** | `6adf6e3` |
| 5 | `BrainModel` does not encapsulate the invariant behind `unsafe impl Sync`; `unsafe impl Send` may be redundant | field private, `from_backend` the only constructor, `replace_*_model` re-materialise; `Send` impl deleted and replaced by a compile-time assertion; encapsulation proven by clippy failing on a test that reached into the field | **closed** | `0d6b4b2` |
| 6 | Lineage fabricates `RelationType::Clone` for a missing original relation | `rebuild_relations_from_plan` returns a typed error; 3 tests incl. both controls | **closed** | `0d6b4b2` |
| 7 | Three.js and Pixi deprecation warnings emitted per frame; no console gate | `shadows="percentage"`; PixiViewport migrated to the v8 path-then-style API; `console_hygiene.spec.ts` covers dashboard **and** landscape | **closed** (`THREE.Clock` is r3f-owned and listed individually with its reason) | `9230e1d` |
| 8 | Five backend-dependent E2E skips; the specs are not valid live-backend E2E; CI contract contradicts them; 1 lint error | five specs replaced by `ipc_contract.spec.ts` against a deterministic typed mock; `real_backend.spec.ts` declares nothing unless required and fails closed; CI comment reconciled; **17 passed / 0 failed / 0 skipped** | **closed** | `9230e1d` |
| 9 | Unfinished master scope (bindings, ESLint, NOTICE/SBOM, bundle, live-Bevy, full gates) | see rows below | **partly closed** — ESLint and live-Bevy remain | `766609e`, `c5e3c30` |
| 9a | Generated binding authority + drift gate | `App.tsx` imports the generated types; `ipcBindingAuthority.test.ts` (4 tests, incl. a positive control and a count of the remaining gap); CI already diffs after regeneration | **closed** | `c5e3c30` |
| 9b | ESLint to 0/0 without relaxing rules | `npm run lint` exit 0 + `node scripts/eslint_ratchet.mjs` → `0 errors, 0 warnings (baseline 0)`, verified 2026-07-27 at `2285a92` | **closed** (was `open`, 472 warnings, during this package) | later commit on this branch |
| 10 | The save-path patch does not implement its own accepted migration contract; autosave and `saves/` contradict each other; `PROJECT.md` not updated | `legacy-import` drop directory (user-authorised, read-only, never a write target); autosave moved to `saves/autosave.json` under the same envelope with one-time adoption of the old file; `PROJECT.md` §Persistence; frontend label and placeholder corrected | **closed** | `ae1fb40` |
| 11 | Feature lifecycle incomplete — implementation/deployment/monitoring docs absent; ledger rows all say `open`; DEC-2 obsolete | three docs written and reconciled with commits; `npx ai-devkit lint --feature anima-completion` → **All checks passed**; this table; DEC-2 superseded below | **closed** | this commit |
| 12 | NOTICE is direct-only, not an SBOM, states an unmet obligation; bundle only budgeted; `check:*` not in CI | npm closure 8 → 36 (install) → **21 measured ship closure** with a transitive regression test; CycloneDX 1.5 SBOM (458 components, dependency graph, deterministic serial, schema-validated); licence texts packaged; bundle gate asserts the **route split**, measured; six `check:*` gates wired into CI | **closed** as engineering, including the licence texts for 439 of 440; **1 unresolved** stays release-blocking and cannot be closed by engineering | `766609e`, extended this commit |

### 2.3 Findings raised by this pass, not in either brief

| Finding | Severity | Status |
|---|---|---|
| Worldgen chose a flora species from the *sampled* cell and then jittered the instance into a neighbouring one, producing pines in grassland and trees in river cells. Only visible once the MCP manifest described the real world. | High | **closed** — species is chosen from the landing cell; `WORLD_GEN_VERSION` 20 → 21 (`fd3dc79`) |
| Walk mode could leave the terrain: the mesh spans ±600 render units, the camera clamp allowed 900, and `surfaceY` returns sea level off-mesh. A boundary escape. | High | **closed** — walk clamped to the terrain footprint (`fd3dc79`) |
| The MCP manifest declared trunk collider radii in render units while publishing positions in the canonical 200-unit bounds — every collider 6× too wide. | Medium | **closed** — `convertFloraRadius` at the boundary (`6adf6e3`) |
| `ChronicleEvent.parameter_delta` was hand-declared `Record<string, number>`; the Rust `HashMap` produces optional values, and `undefined >= 0` is false, so a missing delta rendered as `rate: undefined` with no sign. | Medium | **closed** — found by switching to the generated type (`c5e3c30`) |
| The canonical camera poses had never been rendered: `spawn` targeted the middle of the map, which on this world is open ocean. | Medium | **closed** — poses derived from the world (`fd3dc79`) |
| `check:csp`, `check:bundle`, `check:notice` existed as package scripts that nothing in CI invoked. | Medium | **closed** — wired into CI (`766609e`) |

### 2.1 Findings raised during the pass, not in the brief

| Finding | Severity | Status |
|---|---|---|
| `npm run gen:manifest` was broken — invoked `esbuild`, which is **not installed** (Vite 8 uses rolldown/oxc). The documented map exporter could not be run by anyone. | High | **closed** — `scripts/run_ts.mjs` (`8e6a165`) |
| `public/ecosystem.html` shipped in every desktop build and loaded three.js + simplex-noise from `cdnjs.cloudflare.com`, unpinned | High | **closed** — build-only Vite plugin (`0da67a4`) |
| The CPU brain path shared unmaterialized lazy `Param` cells across Bevy's parallel `Res<T>` readers | High | **closed** — `materialize_params` (`b27c28b`) |
| `compact` filtered original relations by surviving-node set; with compression on this disconnects survivors | High | **closed before it shipped** — rebuilt from the plan (`57a8246`) |
| `tests/` suite produced 28 misleading failures under CPU contention | Medium | **closed** — `maxWorkers: 4` (`3fb691d`) |
| No bundle budget; Vite's always-on 500 kB warning cannot distinguish 856 kB from 1.4 MB | Medium | **closed** — `npm run check:bundle` |

## 4. What changed in the plan once evidence arrived

Recorded because the design doc was wrong about two things and the corrections are the useful part.

- **The `unsafe impl`s were not redundant.** The design proposed deleting them if the types were
  already `Send + Sync`. Replacing them with a static assertion did not compile:
  `OnceCell<Tensor<..>>` and a `dyn Fn(..) + Send` initializer inside burn 0.13's `Param` make it
  `!Sync` — on **both** backends, so the `WgpuDevice` everyone suspected was innocent. That turned a
  documentation task into a correctness one: only the wgpu constructor drained the lazy
  initialization, so the CPU path handed a value with empty cells to Bevy's parallel readers.
- **Enabling lineage compression needed more than the status doc's two steps.** §3.15.1 described
  adding the count and flipping the flag. It does not mention that `compact` filters *original*
  relations by surviving-node set — correct while nothing is spliced, and silently graph-destroying
  the moment something is. Both edges of `A → B → C` name the removed `B`, so `C` becomes a root.

## 5. Not done, and why

Each row states what would close it, so none of these reads as "we ran out of time" without a next
step attached.

- **D3 / ESLint 472 → 0.** **Was open during this package; closed later on the same branch** —
  verified 2026-07-27 at `2285a92` by `npm run lint` (exit 0) and `node scripts/eslint_ratchet.mjs`
  (`0 errors, 0 warnings (baseline 0)`). The paragraph below is the state **at the time of this
  package** and is kept as the historical record of why it was hard. 491 → 483 → 472, and every one of those reductions came from
  deleting files rather than fixing a warning. The remaining 472 decompose as 365
  `no-explicit-any`, 53 `no-unused-vars`, 29 `react-hooks/immutability`, 21 `react-hooks/purity`, 14
  `exhaustive-deps` and 3 other hook warnings, across roughly a hundred files. No rule has been
  relaxed and no file excluded; the ratchet blocks growth. **To close:** replace `any` in production
  code with the generated `ts-rs` types where they exist — that work overlaps with 9a, which is now
  done, so the types are in place — then the hook rules file by file. It is mechanical, it is large,
  and it is the one master-scope item this pass did not reach.
- **CSP verified in the live webview.** **External gate.** `check:csp` validates shipped artifacts
  against the declared policy and now runs in CI; it cannot prove the app boots under it, because
  that needs the Tauri webview and CLAUDE.md records that running the full backend here has crashed
  the machine. **To close:** one `npm run tauri:dev` by a human. Deployment doc §2.1.
- **Real-backend E2E.** **External gate, fail-closed.** `real_backend.spec.ts` needs a release build
  *and* a `tauri-driver` WebDriver session; neither exists here. It declares no test unless
  `ANIMA_E2E_REQUIRE_BACKEND=1` and fails on every missing precondition when set, so the gap is
  loud rather than amber. Deployment doc §2.2.
- **Licence texts packaged — 439 of 440; 1 blocked.** **Updated 2026-07-27 (second pass); still
  release-blocking for that one.** `licensing/THIRD_PARTY_LICENSES.txt` reproduces the texts
  verbatim with a SHA-256 per source file: 408 read out of the installed artifact and 31 vendored
  from the upstream repository at the **immutable commit that release was published from**, stored
  under `licensing/upstream/` with the evidence in `upstream/sources.json` and read fail-closed by
  `scripts/lib/upstream_licenses.mjs`. What remains is one component: `hexf-parse` 0.2.1, whose
  project has never published a licence file for that release and whose only later `LICENSE` is a
  *different* licence. Engineering cannot invent a copyright holder's notice, and did not.
  **To close:** a legal decision on whether the crates.io CC0-1.0 declaration suffices for a
  public-domain dedication that imposes no attribution condition. Gate:
  `npm run check:licenses-complete`. Deployment doc §3.1.
- **Live-Bevy experiment readiness.** **Out of scope and unchanged.** Multi-session work requiring
  `WorldLawSet`, `ExperimentManifest`, `CausalLedger` and `SimClock` to become live-engine
  resources. CLAUDE.md's prohibition on claiming the live world is experiment-ready stays in force,
  and nothing in this pass approached it. Recorded so its absence is not mistaken for progress.
- **`LineageGraphState` / `MigrationPayload` have no ts-rs source.** Small and concrete: derive `TS`
  on the Rust definitions and lower the count in `ipcBindingAuthority.test.ts` in the same commit.

## 3. Decisions that are not mine to take

Recorded per operating rule 7: alternatives, evidence, recommendation, and the exact blocked task.
Independent work continues around them.

### DEC-1 — EB-S04 re-baseline, and the per-agent-brain default

**Blocked task:** §3.1 of the status doc — flipping `BrainPolicy::default().evolved` to `true`.

**Evidence.** 11 of 12 EB gates pass. EB-S04 demands a bit-identical trajectory against a baseline
captured *before* shared-model initialisation became seeded. It fails because the initialisation
was deliberately improved (random → seeded, so two runs of one world stop diverging invisibly), not
because anything regressed.

**Alternatives.**

1. *Re-baseline EB-S04 against the seeded build*, record in ADR-0003 why the old reference was
   discarded, then decide the default separately. Cost: the old baseline stops being a regression
   net for anything else it happened to cover.
2. *Keep EB-S04 as-is and leave it permanently amber.* Cost: a gate nobody can turn green stops
   being read at all, which is worse than no gate.
3. *Reconstruct a bit-identical seeded-equivalent baseline.* Cost: substantial, and it re-encodes a
   comparison the seeding change made meaningless.

**Recommendation:** (1). A gate that cannot pass by correct code is a broken gate, and the honest
repair is an explicit re-baseline with the reason written down.

**Why I am not taking it:** it discards a scientific reference point, and §3.1 states the default
decision must be recorded as an ADR decision item. That is an owner's call, not an implementer's.

> **UPDATE 2026-07-27 — RESOLVED, option (1).** The owner chose the explicit re-baseline. EB-S04's
> reference point is now *the seeded build with `BrainPolicy::evolved = false`*, requiring two
> bit-identical statements: same seed across two runs, and gates-open versus the pre-step-4
> component layout. Recorded in
> [ADR-0003](../../decisions/ADR-0003-evolved-per-agent-brains.md) as decision 12 plus a dated
> "Cập nhật 2026-07-27" section that states why the old marker was abandoned — it was not merely
> hard to measure, it never existed, because the pre-ADR build materialised its weights lazily from
> a self-advancing process-global RNG and so had no deterministic trajectory to match. Measured at
> `2b61d07`: `cargo test --features desktop --test brain_controlled_comparison_tests` → 11 passed,
> 0 failed. The gate moves 🟡 → ✅ and the EB tally becomes 12/12.
>
> **The default decision is deliberately still open**, and is now a *scientific* task rather than a
> flag flip: it requires a preregistered, multi-seed controlled comparison. That preregistration is
> its own dated package — see the E2 lifecycle docs — and nothing has been run.

### DEC-2 — Canonical map view capture — **SUPERSEDED, and it was not mine to defer**

**Original position:** declare the eight views uncaptured, gate the invariant
(`captured: true ⇒ file exists`), and treat building a capture harness as a follow-up feature.

**Why it was wrong.** `AGENTS.md` makes canonical before/after views a **hard acceptance gate** for
any map work, and a local planning doc cannot waive a repository-level acceptance rule. Marking the
views `captured: false` was honest — it was not a gate, and the finding it was recording remained
open with a label on it. The supervisor addendum (finding 3) made the same point.

The reasoning was also wrong on its merits. "A harness nobody has validated produces screenshots
that look like evidence" argues for validating the harness, not for skipping it: option (1) was
taken, and the first run immediately produced three defects (the HUD compositing into every frame, a
`spawn` view aimed at open ocean, camera Y converted by the terrain exaggeration factor) plus, once
the manifest described the real world, three high ecology findings and a boundary escape. None of
those were reachable without capturing.

**Resolution.** Option (1), in `fd3dc79`. `npm run capture:views` drives `landscape.html` in real
Chromium with real WebGL — a frontend Vite entry, so CLAUDE.md's prohibition on running the full
Tauri backend is untouched. It needs a GPU (0.27 fps on SwiftShader against 46.7 on hardware, and a
different rasteriser besides), so it fails closed rather than falling back, and CI holds the
artifact instead: the manifest carries each PNG's SHA-256.

**What remains a genuine external gate:** the capture must be re-run on a GPU machine whenever
`WORLD_GEN_VERSION`, the world identity, or a canonical camera pose changes. That is documented in
[the deployment doc](../deployment/2026-07-27-feature-anima-completion.md) §2.3, not deferred here.
