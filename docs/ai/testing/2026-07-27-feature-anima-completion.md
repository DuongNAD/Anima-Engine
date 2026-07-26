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

## 3. Gates after the change

| Gate | Command | Result | Exit |
|---|---|---|---|
| Backend format | `cargo fmt --check` | clean | 0 |
| Backend test | `cargo test --features desktop --no-fail-fast` | **759 passed · 0 failed · 4 ignored**, 78 result lines, 0 compile warnings | 0 |
| Empty targets | `node scripts/check_test_targets.mjs` | **76 targets, 0 empty** | 0 |
| Clippy desktop | `cargo clippy --all-targets --features desktop -- -D warnings` | clean | 0 |
| Clippy headless | `cargo clippy --all-targets --no-default-features -- -D warnings` | clean | 0 |
| Frontend unit (`src/`) | `npm run test` | **14 files, 97 passed** | 0 |
| Frontend integration | `npx vitest run --root tests --maxWorkers=4` | **26 files, 243 passed, 1 skipped** | 0 |
| Build | `npm run build` | pass | 0 |
| CSP compatibility | `npm run check:csp` | 2 shipped HTML files, 0 external origins, 0 inline script bodies | 0 |
| MCP manifest gate | `validate_map_manifest animal-map.manifest.json` | **pass, score 100/100, 0 issues**, 408 entities | — |

Backend +13 tests, frontend +7.

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
| The eight canonical map views | **not captured** | No capture pipeline exists. All eight are `captured: false` and no image is claimed; the gate enforces `captured:true ⇒ file exists`. Fabricating PNGs to green a check is the failure this pass exists to remove. |
| Visual map quality | **not reviewable** | The only images in the repo are `public/base_map.png` (a hand-drawn illustration, not a render of the generated world — `PixiViewport.tsx:980` calls it a fallback that was replaced) and `screenshot.png` (a night-time orbit capture, mostly black). Per `AGENTS.md` rule 5, map completion is **not** claimed. |
| Live-Bevy experiment readiness | **out of scope** | §3.3/§3.6 of the status doc; multi-session. CLAUDE.md's prohibition stays in force. |
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
