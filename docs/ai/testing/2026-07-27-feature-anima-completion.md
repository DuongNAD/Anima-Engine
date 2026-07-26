---
phase: testing
feature: anima-completion
title: Testing & Evidence — Completion & Hardening pass
description: Every gate, the exact command, the exit code and the counts it produced
status: active
owner: maintainers
last_reviewed: 2026-07-27
requirements: ../requirements/2026-07-27-feature-anima-completion.md
planning: ../planning/2026-07-27-feature-anima-completion.md
---

# Testing & Evidence — Completion & Hardening pass

Machine: Intel Core i5-14600KF, Windows 11 Pro 26200. Worktree
`.worktrees/feature-anima-completion`, branch `feature-anima-completion`, base `6caeeb4`.
All `cargo` runs from PowerShell, never Git Bash (Git Bash's `PATH` makes 15 feature-gated targets
die at `STATUS_ENTRYPOINT_NOT_FOUND` before running a test).

## 1. Baseline at `6caeeb4`, measured 2026-07-27

| Gate | Command | Result | Exit |
|---|---|---|---|
| DevKit skills | `ai-devkit skill list` | 20 skills, all 19 required present | 0 |
| DevKit lint | `ai-devkit lint` | all checks passed | 0 |
| Backend format | `cargo fmt --check` | clean | 0 |
| Backend test | `cargo test --features desktop --no-fail-fast` | **746 passed · 0 failed · 4 ignored**, 77 result lines, 0 compile warnings | 0 |
| Empty targets | `node scripts/check_test_targets.mjs <out>` | 75 targets, **0 empty** | 0 |
| Clippy desktop | `cargo clippy --all-targets --features desktop -- -D warnings` | clean | 0 |
| Clippy headless | `cargo clippy --all-targets --no-default-features -- -D warnings` | clean | 0 |
| Rust advisories | `cargo audit` | 773 crate deps scanned, no unignored advisory | 0 |
| ESLint | `npm run lint` | **0 errors, 491 warnings** | 0 |
| ESLint ratchet | `node scripts/eslint_ratchet.mjs` | 0 errors, 491 warnings (baseline 491) | 0 |
| Frontend unit (`src/`) | `npm run test` | 13 files, **90 passed** | 0 |
| Frontend integration (`tests/`) | `npm run test:frontend` | **28 failed · 215 passed · 1 skipped** | **1** |
| Build | `npm run build` | pass; largest chunk `react-three-fiber.esm` **856.26 kB** | 0 |
| Doc links | `node scripts/check_docs_links.mjs` | 417 links in 90 files, 0 broken | 0 |

The reviewed base's reported numbers reproduced exactly, **except** the `tests/` suite, which was
red. That is investigated below rather than assumed.

## 2. The `tests/` suite red — diagnosed, not dismissed

The project's status doc records "28 failures" as the known signature of a *false* red caused by a
concurrent `cargo` build. That claim is itself a hypothesis, so it was tested.

| Step | Observation |
|---|---|
| Re-run, whole suite | **28 failed · 215 passed · 1 skipped**, wall 40.3 s — the *same 28 tests*, twice |
| Failure shapes | mixed: `Test timed out in 15000ms`, `expected undefined to be defined`, `Unable to find an element with the text …` (with a real DOM dump) |
| Machine state | `node.exe` PID 35160 = **`E:\Project\LIVA` Vitest run**, 16,311 CPU-seconds, 1.42 GB resident, still climbing |
| One file alone | `vitest run … frontend/phase6_ui.test.tsx` → **4 passed, exit 0**, 9.5 s. Same file was 4/4 failed in the suite |
| Whole suite, `--maxWorkers=4` | **26 files · 243 passed · 1 skipped · 0 failed**, exit 0, wall 47.5 s — *with the LIVA process still running* |

Conclusion: contention, not a regression, and not specific to `cargo`. A stable failure *set* looked
like a real defect, which is what made the folklore hard to trust; the deciding evidence is that one
file passes alone and all 26 pass under a worker cap.

The failure is misleading rather than merely slow because every file pays for a full
`render(<App />)`; a starved render surfaces as an assertion-shaped "element not found" with a DOM
dump attached. Fixed by pinning `maxWorkers: 4` in `tests/vitest.config.ts`. **No assertion was
relaxed** — all 243 still run.

## 0. Final acceptance run — the committed state at `c5e3c30`+, measured 2026-07-27

Every row run fresh, in this worktree, after the last code change. §3 below is the first session's
mid-pass measurement and is kept for its red-first evidence, not as the current state.

| Gate | Command | Result | Exit |
|---|---|---|---|
| Backend test | `cargo test --features desktop --no-fail-fast` | **775 passed · 0 failed · 4 ignored**, 78 result lines | 0 |
| Empty targets | `node scripts/check_test_targets.mjs cargo-test-output.txt` | **76 targets, 0 empty** | 0 |
| Backend format | `cargo fmt --check` | clean | 0 |
| Clippy desktop | `cargo clippy --all-targets --features desktop -- -D warnings` | clean | 0 |
| Clippy headless | `cargo clippy --all-targets --no-default-features -- -D warnings` | clean | 0 |
| Rust advisories | `cargo audit` | 773 crate deps scanned, no unignored advisory | 0 |
| Generated bindings | `cargo test --lib export_bindings` | **16 passed**, incl. the new `LegacyImportListing` | 0 |
| npm advisories (root) | `npm audit --audit-level=high` | 0 vulnerabilities | 0 |
| npm advisories (tests) | `npm audit --audit-level=high --prefix tests` | 0 vulnerabilities | 0 |
| ESLint | `npm run lint` | **0 errors, 472 warnings** | 0 |
| ESLint ratchet | `node scripts/eslint_ratchet.mjs` | 0 errors, 472 warnings (baseline lowered 483 → 472) | 0 |
| Frontend unit (`src/`) | `npm run test` | **14 files, 107 passed** | 0 |
| Frontend integration (`tests/`) | `npm run test:frontend` | **29 files, 264 passed, 1 skipped** | 0 |
| Build | `npm run build` | pass (`tsc` strict + 2 Vite entries) | 0 |
| CSP compatibility | `npm run check:csp` | 2 shipped HTML files, 0 external origins, 0 inline script bodies | 0 |
| Bundle budget + split | `npm run check:bundle` | 1700.3 / 2000 KiB; **`index.html` 3D renderer: no**, `landscape.html`: yes | 0 |
| NOTICE | `npm run check:notice` | up to date — **419 crates + 45 npm packages** | 0 |
| SBOM | `npm run check:sbom` | up to date — **464 components** (CycloneDX 1.5) | 0 |
| Flora footprint | `npm run check:flora-footprint` | 7 solid types measured against real three, 0 drift | 0 |
| Doc links | `node scripts/check_docs_links.mjs` | 436 links in 94 files, 0 broken | 0 |
| E2E (browser) | `npm run test:e2e` | **17 passed · 0 failed · 0 skipped** | 0 |
| E2E (real backend, required) | `ANIMA_E2E_REQUIRE_BACKEND=1 … real_backend.spec.ts` | **2 failed** — fails closed on both missing preconditions, as designed | 1 |
| Canonical capture | `npm run capture:views` | **8 passed**, all eight PNGs written on hardware GL | 0 |
| DevKit lint (feature) | `npx ai-devkit lint --feature anima-completion` | **All checks passed** (7 lifecycle docs) | 0 |
| MCP — discover | `discover_map_artifacts` | `missingViewKinds: []` (was missing 5) | — |
| MCP — validate | `validate_map_manifest animal-map.manifest.json` | **pass, 100/100, 0 critical/high**, 429 entities, against the *shipped* identity | — |
| MCP — prepare | `prepare_team_review` | brief produced, "Missing canonical views: none" | — |
| MCP — inspect | `inspect_map_views` (all 8) | all eight returned and reviewed | — |

Movement against the first session's numbers: backend 765 → **775**; `src/` 90 → **107**; `tests/`
250 → **264**; E2E 9 passed/5 skipped → **17 passed/0 skipped**; ESLint 483 → **472**; npm
attribution 8 → **45**; canonical views 0 → **8**.

### 0.1 Red-first evidence for this session's changes

| Change | How it was proven red | Observed |
|---|---|---|
| Flora clearance policy | ran the new suite before the module existed | 14 tests failed to resolve the import; then the spawn assertion was checked against the *old* `findSpawn` at 256/512/1024/2048 to find where the defect actually lives — only 2048 |
| `check:flora-footprint` | perturbed `Acacia` 0.95 → 0.85 | **failed**, naming the drift 0.1 > 1e-3 |
| Canonical capture | ran it | **three defects**: HUD composited into every frame; `spawn` framed open ocean; camera Y lifted to 1596 over a 1200-wide map |
| Worldgen ordering | ran `validate_map_manifest` against the shipped world | **3 high** ecology findings; 0 after the fix |
| `BrainModel` encapsulation | made the field private and ran clippy | **failed** — `phase5_burn_wgpu_fallback` was reaching into it |
| `unsafe impl Send` removal | `assert_send::<BrainModel>()` | **compiled**, so the impl was redundant |
| Lineage `Clone` fabrication | fed a plan edge with no original relation | typed error; positive control proves the happy path still carries recorded types |
| Tauri IPC mock | ran the suite against invented payload shapes | app crashed into its error boundary — `Cannot read properties of undefined (reading '0,0')` |
| `parameter_delta` optionality | switched to the generated type | `tsc` error TS18048 — the hand-written mirror had claimed non-optional |
| SBOM determinism | compared `localeCompare` order to a plain sort | **mismatch**; the sort was the bug |
| Bundle route split | measured `/` and `/landscape.html` in a real browser | `/` fetches 17 JS files, **none** the three.js chunk |

## 3. Gates after the change (first session, mid-pass)

| Gate | Command | Result | Exit |
|---|---|---|---|
| Backend format | `cargo fmt --check` | clean | 0 |
| Backend test | `cargo test --features desktop --no-fail-fast` | **759 passed · 0 failed · 4 ignored**, 78 result lines, 0 compile warnings | 0 |
| Empty targets | `node scripts/check_test_targets.mjs` | **76 targets, 0 empty** | 0 |
| Clippy desktop | `cargo clippy --all-targets --features desktop -- -D warnings` | clean | 0 |
| Clippy headless | `cargo clippy --all-targets --no-default-features -- -D warnings` | clean | 0 |
| Rust advisories | `cargo audit` | no unignored advisory | 0 |
| DevKit lint | `ai-devkit lint` | all checks passed | 0 |
| npm advisories (root) | `npm audit --audit-level=high` | clean | 0 |
| npm advisories (tests) | `npm audit --audit-level=high --prefix tests` | clean | 0 |
| ESLint | `npm run lint` | **0 errors, 483 warnings** (was 491) | 0 |
| ESLint ratchet | `node scripts/eslint_ratchet.mjs` | 0 errors, 483 warnings (baseline 491) | 0 |
| Frontend unit (`src/`) | `npm run test` | **13 files, 90 passed** | 0 |
| Frontend integration | `npm run test:frontend` | **27 files, 250 passed, 1 skipped** | 0 |
| Build | `npm run build` | pass | 0 |
| CSP compatibility | `npm run check:csp` | 2 shipped HTML files, 0 external origins, 0 inline script bodies | 0 |
| Bundle budget | `npm run check:bundle` | 23 chunks, **1695.8 / 2000 KiB**; largest `react-three-fiber.esm` 836.2/900 | 0 |
| NOTICE | `node scripts/gen_notice.mjs --check` | 419 crates + 8 npm packages, **0 without a licence** | 0 |
| Doc links | `node scripts/check_docs_links.mjs` | 434 links in 94 files, 0 broken | 0 |
| E2E | `npm run test:e2e` | **9 passed, 0 failed, 5 skipped** (each naming its reason) | 0 |
| MCP manifest gate | `validate_map_manifest animal-map.manifest.json` | **pass, score 100/100, 0 issues**, 408 entities | — |

Backend +19 tests (746 → 765). Frontend integration +7 (243 → 250).

### 3.1 Two regressions I introduced and had to fix

Recorded because a pass that only lists successes is not an honest test report.

- **`npm run build` failed with `TS2307: Cannot find module 'node:fs'`.** The new manifest evidence
  test read the filesystem but lived in `src/__tests__/`, and the root `tsconfig` type-checks `src/**`
  against browser libs with no `@types/node` — correctly, since the frontend does not run in node.
  `npm run test` (Vitest) passed the whole time, so only the build caught it. Moved to
  `tests/frontend/`, which is the package that has node types. This is why the src suite reads 90
  again and the `tests/` suite reads 250.
- **ESLint went from 0 errors to 1.** `preserve-caught-error` — the E2E identity check threw a new
  `Error` inside a `catch` without attaching `{ cause }`, discarding the original failure. Fixed by
  attaching it, which is also just better diagnostics.

## 4. Red-first evidence

Each behaviour change was proven to fail without its fix. This is the part that distinguishes a test
from a restatement of the code.

| Change | How it was proven red | Observed |
|---|---|---|
| `DEFAULT_GRID_DIM` 128→256 | reinstated `128`, ran `cargo test --lib -- core::sim_rules` | **2 failed** — `s03_default_grid_dim_tracks_map_settings_default` (`128` vs `256`) and `s03_units_per_cell_at_the_default_resolution`; restoring 256 → 9 passed |
| Lineage compression | negative control inside `survives_compaction_unchanged` | post-compaction edge walk must **under-count**; if it agrees with the stored value, compression is not running and the headline assertion is vacuous |
| `unsafe impl Send/Sync` | replaced with a `T: Send + Sync` static assertion | **did not compile** — `OnceCell<Tensor<NdArray,2>>` and `dyn Fn(..)+Send` are `!Sync`. Hypothesis ("the impls are redundant") refuted by the compiler; see §5 |
| Param materialization | source scan `every_brain_model_constructor_materializes_its_parameters` | caught its own search string on first run (fixed by cutting at the test module); pins the site count at exactly 4 |
| Map manifest evidence | swapped the HEAD manifest back in, re-ran | **4 of 7 failed** — missing artifact, placeholder checksum, header mismatch, gridDim 128≠256. The 3 that still passed are the structural checks that let the defect hide |
| CSP compatibility | ran the gate against the pre-fix `dist/` | **failed naming 4 problems** — 2 external CDN origins and 2 inline script bodies |

## 5. A hypothesis that was wrong, and what replaced it

The design doc proposed removing the two `unsafe impl`s if the underlying types were already
`Send + Sync`. They are not, and the compiler said why:

```
error[E0277]: `std::cell::OnceCell<Tensor<NdArray, 2>>` cannot be shared between threads safely
error[E0277]: `dyn Fn(&WgpuDevice, bool) -> Tensor<..., 2> + Send` cannot be shared between threads safely
```

burn 0.13's `Param<T>` is `{ state: OnceCell<T>, initialization: Option<RwLock<..>> }`. So it is
**`Sync`** that fails, not `Send`, and it fails on the **ndarray backend too** — the `WgpuDevice`
everyone suspected is innocent.

That turned a documentation task into a correctness one. `linear_from_parts` builds parameters with
`Param::uninitialized`, so the cells are genuinely empty at construction; the wgpu constructor
already forced a forward pass before returning, the ndarray constructor did not. A `BrainModel` is
inserted as a Bevy resource, and Bevy runs `Res<T>` readers in parallel — so the CPU path was
handing out a value whose first concurrent touch is a data race, with `unsafe impl Sync` compiling
it away. Fixed by `materialize_params` on all four construction sites; the impls remain (they are
required) but now carry the derivation and the conditions.

## 6. What is NOT proven, and why

Stated plainly so no reader over-reads the green above.

| Claim | Status | Why |
|---|---|---|
| The app runs under the new CSP | **unverified** | Needs the Tauri webview. CLAUDE.md forbids running the full backend on this machine (it has crashed it). `npm run check:csp` validates *shipped artifacts against the declared policy*, not the running app. A human `npm run tauri:dev` is the missing step. |
| The eight canonical map views | ~~not captured~~ → **captured** | Superseded. `npm run capture:views` renders all eight from the real scene on hardware GL; the manifest carries each PNG's SHA-256 and the evidence test verifies them. What remains external is *re-capturing*, which needs a GPU (0.27 fps on SwiftShader against 46.7) — a machine requirement, not an absent pipeline. |
| Visual map quality | ~~not reviewable~~ → **reviewed** | Superseded. Eight canonical renders of the shipped world were inspected through the MCP in the mandatory order, and the review found real defects (§0.1). The deterministic gate is pass 100/100, 0 critical/high, against the *shipped* identity rather than a stand-in. |
| Real-backend E2E | **not run** | Needs a release build and a `tauri-driver` session; neither exists here. `real_backend.spec.ts` fails closed under `ANIMA_E2E_REQUIRE_BACKEND=1` rather than skipping, so the gap is loud. |
| Licence texts packaged | **not done** | `NOTICE` attributes 464 components and says so itself. An inventory is a prerequisite for discharging the MIT/BSD obligation, not the discharge. Release-blocking, owner/legal. |
| ESLint at zero | **not reached** | 472 warnings remain, and the 483 → 472 movement came from deleting files rather than fixing warnings. No rule relaxed, no file excluded, baseline lowered to lock in what was gained. |
| Live-Bevy experiment readiness | **out of scope** | §3.3/§3.6 of the status doc; multi-session. CLAUDE.md's prohibition stays in force, and this pass did not approach it. |
| Per-agent evolved brains as default | **blocked on a decision** | DEC-1 in the planning doc. |

## 7. Findings raised during this pass that were not in the brief

| Finding | Severity | Status |
|---|---|---|
| `npm run gen:manifest` was broken — invoked `esbuild`, which is **not installed** (Vite 8 uses rolldown/oxc). The documented "real map manifest exporter" could not be run by anyone following the instructions. | High | Fixed — both generators route through `scripts/run_ts.mjs` (rolldown) |
| `public/ecosystem.html` ships in every desktop build and loads three.js + simplex-noise from `cdnjs.cloudflare.com`, unpinned, no integrity attribute | High | Fixed — build-only Vite plugin drops it and `webgl-test.html` from `dist/`; CSP would block it regardless |
| The CPU brain path shared unmaterialized lazy `Param` cells across Bevy's parallel readers | High | Fixed — see §5 |
| `compact` filtered original relations by surviving-node set; with compression on this disconnects survivors (`A→B→C` loses both edges when `B` is spliced) | High | Fixed before it shipped — relations rebuilt from the simplify plan; `compaction_leaves_no_orphans` is the gate |
| A runaway `E:\Project\LIVA` Vitest process (16k CPU-seconds, 1.4 GB) is degrading this machine | Informational | **Not touched** — not this project's process |
| Port 5173 is held by a LIVA Vite dev server, so Playwright's `reuseExistingServer` would silently test a different app *right now* | High | Open — WP3 |
