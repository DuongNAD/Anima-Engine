---
phase: implementation
feature: anima-completion
title: Implementation Notes — Completion & Hardening pass
description: What each commit changed, the seam it changed it at, and where the design turned out to be wrong
status: active
owner: maintainers
last_reviewed: 2026-07-27
plan: ../planning/2026-07-27-feature-anima-completion.md
design: ../design/2026-07-27-feature-anima-completion.md
---

# Implementation Notes — Completion & Hardening pass

Two sessions. The first landed twelve commits and left work explicitly unfinished; an independent
browser/code review then produced [an acceptance addendum](../../../.agents/anima-completion-supervisor-findings.md)
of twelve numbered findings, several of which were defects the first session introduced or failed to
catch. This document covers both, because a reader wants the state of the code, not the order it was
arrived at.

The interesting parts are the places where implementing a finding disproved the plan for it. Those
are called out per section rather than collected at the end.

## 1. World identity — the manifest described a different world

**Commit** `6adf6e3`. **Finding 1.**

`gen_world_manifest.ts` ran `generateWorld("1337", { size: 256 })` and encoded the result. Every
figure it emitted was real — real bytes, a real SHA-256, nothing fabricated — and identified a world
the app never renders.

The trap is subtler than a placeholder checksum, and worth stating because the obvious fix is also
wrong. The shipped path generates the authoritative world at `sharedWorld.ts`'s identity (seed
`"seed"`, 2048², continent) and then downsamples with `worldToArtifact(world, 256)`. Generating at
256 directly is **not** a coarser view of that world: the generator samples its noise at whatever
grid it is handed, so it produces a different world. Both generators now import the identity, and
the evidence test binds the manifest to `sharedWorld.ts` and to the artifact's own ANMW header seed
rather than to a second hard-coded copy — a copy would have been just as wrong and just as green.

`gen_map_manifest.ts` had the same defect, which meant every deterministic gate the map-review MCP
scored (biome bboxes, flora ecology, navigation reachability) ran against seed 1337 at 128². Its
previous 100/100 was a score for a world nobody sees.

## 2. Flora clearance — one policy, four consumers

**Commit** `6adf6e3`. **Finding 2.**

Reported: walk mode opened the camera inside foliage at render `(-129, -94)`. That reproduces
exactly. What the reproduction does **not** show is a collider overlap — the position is outside
every trunk collider, so a collider-based clearance rule would have called it clear, and the finding
as written (`reject flora collider clearance`) would not have fixed it.

Measuring the neighbourhood explains it. The nearest solid flora is a broadleaf, `floraScale` 1.255,
canopy radius 1.340, at distance 1.896. A sphere of radius R at distance D subtends `2·asin(R/D)`;
here that is **ninety degrees**. The camera was 0.56 units from the leaf surface at a walking eye
height of 2.1.

So `floraClearance.ts` distinguishes three footprints:

| Footprint | What it is | Who asks |
|---|---|---|
| `collider` | the narrow trunk, `0.45 + floraScale·0.25` | the walk rig, per frame |
| `canopy` | the space the instance visually occupies | diagnostics |
| `spawn` | the canopy widened until it stops filling the frame | `findSpawn`, canonical captures |

The spawn rule is derived, not tuned: `D ≥ 2R` caps the subtended angle at sixty degrees, which
leaves the middle of the frame readable at any sane field of view. `SPAWN_CLEARANCE_MARGIN` still
applies underneath so a small shrub cannot be legal at arm's length.

The policy previously existed in three independent copies (the rig's inline `TALL` set and radius,
the manifest exporter's restatement of both, and `findSpawn`, which did not know flora existed).
A fourth consumer — the capture harness — was about to restate them again.

**Canopy radii are measured, not declared.** `check:flora-footprint` rebuilds each geometry with
real three and fails on drift beyond 1e-3. It is a script rather than a test because both Vitest
configs alias `three` to a mock, so a test would measure the mock. Proven red by perturbing one
radius.

The runtime push-out also gained the case it used to skip. `if (d2 < r*r && d2 > 1e-6)` guarded the
division that follows, but the guard *skips* the singular case rather than handling it: a player
exactly on a trunk centre — the most-overlapped position possible, and exactly what a spawn picked
from a flora cell produces — was the one position never resolved. `resolveFloraOverlap` takes a
documented fixed direction (+X) when the offset is degenerate.

### The manifest exporter's collider radii were 6× too wide

Found while wiring it to the shared policy. Flora radii are calibrated for the 1200-unit landscape
scene; the manifest publishes positions in the canonical 200-unit bounds. It emitted the render-space
number unconverted.

## 3. Reset returned to open ocean

**Commit** `6adf6e3`. **Finding 4.** `WorldShowcase` computed a scenic `findSpawn` on load and then
its reset handler set `{ x: 0, z: 0 }` — the middle of the map, which this file's own comment two
screens up says is usually open ocean. The browser reproduction confirmed it: biome readout
`Đại dương`. It now returns to the validated spawn.

## 4. Canonical view capture

**Commit** `fd3dc79`. **Finding 3.**

Eight real WebGL renders of the shipped world, driven through `landscape.html` by Playwright. No
Tauri process and no Bevy, so CLAUDE.md's prohibition on running the full backend here is untouched.

**It needs a GPU and says so.** Measured on this machine: default headless Chromium renders this
scene at **0.27 fps** on SwiftShader; with `--use-angle=d3d11` on the RTX 5060 Ti, **46.7 fps**. A
173× difference — and the software result is not the same picture, because it is a different
rasteriser. So the capture is a *producer* with its own config (`npm run capture:views`), it fails
closed on detecting a software renderer, and what CI holds is the artifact: `map_manifest.json`
carries each PNG's SHA-256 and `mapManifestEvidence.test.ts` verifies all eight.

Determinism is five pinned things — world identity, camera pose, clock, weather, viewport — plus a
settle wait counted in **rendered frames**, because "has the terrain mesh finished building" is a
question about frames, not milliseconds.

### Three defects the first capture found

That is what capturing is for.

- **The HUD was in the shot.** Playwright's element screenshot captures the page *region*, not the
  element's pixel buffer, so the control panel, compass, biome banner and minimap composited into
  all eight images. Capture mode now renders no HTML overlay.
- **The poses had never been looked through.** `spawn` targeted canonical `(10, 1, 10)` — the middle
  of the map, which on this world is sea — so the spawn view was a photograph of the ocean.
  `overview` clipped the continent at two edges. All eight are now derived from the world by
  `scripts/derive_view_cameras.ts` and committed as literals, because a canonical view must be
  repeatable: a pose recomputed from world state would move whenever the world moved, and two images
  framing different places cannot be compared.
- **Camera Y used the terrain exaggeration factor.** `CANONICAL_MAX_Y` describes how high *terrain*
  goes, not where a camera may sit. `overview` is authored at `[0, 95, 95]` — a 45° look — and the
  terrain factor lifted it to y=1596 over a 1200-wide map. A pose is a point and an angle; only a
  uniform scale preserves the angle.

## 5. Worldgen planted species in the wrong cell

**Commit** `fd3dc79`. Raised by the MCP gate once it ran against the real world.

Three high ecology findings: pines in grassland, pines and broadleaf in river. The cause is an
ordering: worldgen chose a species from the sampled cell, then jittered the instance up to half a
stride away and re-checked only that the landing cell was not water. A pine chosen in taiga could
come to rest in the grassland next door. (River cells passed the water guard because `classify`
marks a cell River at a lower `riverAmt` than the guard's threshold.)

The species is now chosen from the cell the instance **lands** in, and the treeline and slope rules
read that cell too. The mismatch is unrepresentable rather than filtered. `WORLD_GEN_VERSION` 20 → 21,
which invalidates the IndexedDB cache key and changes the artifact checksum — both expected.

## 6. A boundary escape, found by reading the captures

**Commit** `fd3dc79`.

The terrain mesh spans ±600 render units; the camera clamp allowed 900; `surfaceY` returns sea level
for anything off-mesh. So a walker could leave the world and keep going for 300 units across open
water. Walk mode is now clamped to the terrain footprint; spectator cameras still pull back to frame
the continent.

## 7. `BrainModel` did not encapsulate its own unsafe invariant

**Commit** `0d6b4b2`. **Finding 5.**

The `unsafe impl Sync` is sound only while every lazy `Param` has been materialised on the
constructing thread. That was enforced by four constructors each remembering to call
`materialize_params`, checked by a **source scan**, and the field was `pub` — so any caller could
assign a fresh backend and the argument became false with nothing to say so. The inference worker did
exactly that: it swapped a trained model straight into `brain_model.backend`.

`backend` is private, `from_backend` is the only constructor and materialises before returning, and
the `replace_*_model` methods re-materialise after a swap. That the encapsulation is real is not a
claim — `cargo clippy --all-targets` failed on `phase5_burn_wgpu_fallback` reaching into the field.

**`unsafe impl Send` was redundant and is gone.** The compiler experiment that produced the safety
argument reported only `Sync` failures. `assert_send::<BrainModel>()` compiles without the impl,
which is the proof, and it is kept as a test so a burn upgrade that genuinely loses the bound fails
the build rather than being absorbed by an `unsafe` nobody re-derived.

## 8. Lineage fabricated `RelationType::Clone`

**Commit** `0d6b4b2`. **Finding 6.** An uncompressed plan edge did
`original_type.get(&key).copied().unwrap_or(RelationType::Clone)`. `Clone` is not a neutral default:
it claims a child is an unmutated copy of a parent, and it would be persisted, read back by the
diagnostics, and counted as a reproduction event that never occurred — feeding the per-node mutation
count the rest of that file exists to protect. `rebuild_relations_from_plan` now returns a typed
error naming the edge.

## 9. E2E: five specs that tested nothing

**Commit** `9230e1d`. **Findings 7 and 8.**

Each spawned `src-tauri/target/release/anima-engine`, slept a second, then drove an ordinary Vite
page. Nothing connected the two — the page had no `__TAURI_INTERNALS__`, so every `invoke` rejected,
while the spawned process rendered into a webview Playwright never touched. The specs caught the
resulting failures and called `test.skip()`. CI's own comment meanwhile described them as using a
page-level IPC stub needing no release build. Both descriptions were in the repository at once and
the run was always green, so neither could be checked.

`tests/e2e/tauri-mock.ts` implements the three functions `@tauri-apps/api` routes through, plus
`__TAURI_EVENT_PLUGIN_INTERNALS__`, which `unlisten()` calls unguarded. Replies are **typed by the
generated ts-rs bindings**, which caught two shapes invented while writing it — the app crashed into
its error boundary with `Cannot read properties of undefined (reading '0,0')`. A mock describing an
API that never existed is the same class of problem this whole pass is about.

Real-backend coverage is `real_backend.spec.ts`, which declares **no test at all** unless
`ANIMA_E2E_REQUIRE_BACKEND=1` — so the suite reports zero skips because there is nothing to skip —
and fails closed on every missing precondition when set. Verified in both modes.

### Two deprecation streams, both firing per frame

`shadows` (bare) asks react-three-fiber for `PCFSoftShadowMap`, which three 0.184 deprecated and
silently replaces with `PCFShadowMap`. The warning was true: the soft filter had not been in use for
some time.

`PixiViewport` preferred `beginFill`/`endFill`/`lineStyle`, which pixi 8.19 keeps only as
deprecation stubs. The two APIs are not a rename — v7 is stateful, v8 is path-then-style — so the
adapter keeps the v7 call shape and defers, applying styles when a batch closes. The `dirty` set is
what makes the minimap's three-ring border work; without flushing between `lineStyle` calls all three
rings would come out at one width.

A console gate covers both surfaces. It found a third warning: `THREE.Clock`, constructed by
react-three-fiber itself. Silencing that needs r3f 9 (React 19) — a framework upgrade, not a
hardening step — so it is listed individually with its owner and reason rather than pattern-matched
away.

## 10. Persistence: the migration the confinement promised

**Commit** `ae1fb40`. **Finding 10.**

The obvious implementation of "old absolute-path saves stay loadable" is a command taking an absolute
path, and it hands straight back the capability the confinement removed. So "explicitly opt-in" is
made to mean *the user*: the authorising act is copying the old file into `<app data>/legacy-import/`
with a file manager, which the webview cannot do. `import_legacy_save` addresses it by name through
the same allow-list, reads it, re-seals it into the current envelope, and writes it into `saves/`.
The legacy file is never written, truncated or deleted.

**Two persistence contracts existed and only one was versioned.** The autosave went to
`app_data_dir/default_save.json` as a bare `serde_json` dump written with `fs::write` — which
truncates before writing a byte, so a crash on exit destroyed the autosave the user already had in
order to fail at producing a new one. It is now `saves/autosave.json`, same envelope, same atomic
write; startup adopts the old location once and never writes back.

## 11. Licensing, SBOM, bundle

**Commit** `766609e`. **Finding 12.**

npm attribution was the direct dependency list — eight names. Vite bundles the transitive graph, so
what ships includes `scheduler`, `earcut`, `eventemitter3`, `zustand` and the rest, each carrying the
same obligation. Now the production closure: **8 → 45**.

An inventory grouped by licence string is not an SBOM. `scripts/gen_sbom.mjs` emits CycloneDX 1.5
over the same two graphs — 464 components, every one with a purl — deterministically, because a BOM
that differs on every run cannot be diffed.

The bundle gate checked a number where the property is a route. Measured with a real browser against
`vite preview`: `/` fetches 17 JS files and **not** the three.js chunk; `/landscape.html` fetches it.
So the chunk is already a route boundary — the correct answer, since it is the three.js runtime and
only one page renders 3D. The gate now asserts the entry graphs, which is the regression that can
actually happen; the 836 KiB is still 836 KiB and that debt is scored, not resolved.

**Four gates were package scripts nothing invoked.** `check:csp`, `check:bundle`, `check:notice` and
`check:sbom` all existed and none ran in CI.

## 12. Generated bindings made authoritative

**Commit** `c5e3c30`. **Finding 9 (partial).**

`App.tsx` carried nine hand-written copies of Rust structs. All correct — which is what made them
dangerous, since a correct copy and a generated one are indistinguishable until the Rust side
changes. The switch found a divergence on the first compile: `ChronicleEvent.parameter_delta` was
declared `Record<string, number>` where the Rust `HashMap` produces `{ [k: string]?: number }`, and
`undefined >= 0` is false, so a missing delta rendered as `rate: undefined` with no sign.

`LineageGraphState` and `MigrationPayload` still have no ts-rs source. That is a gap, not a decision,
and `ipcBindingAuthority.test.ts` pins the count at four names so it can only shrink.

## What is deliberately not done

Recorded here so it is not mistaken for oversight; the ledger in
[the planning doc](../planning/2026-07-27-feature-anima-completion.md) carries the same rows with
their gates.

- **ESLint to zero.** 483 → 472 warnings, all from deletions rather than fixes. The remaining 472 are
  a mechanical sweep across ~100 files (365 `no-explicit-any`, 53 unused vars, 50 hook-rule
  warnings). No rule was relaxed and no file excluded; the ratchet still blocks growth.
- **Licence texts packaged.** The inventory is a prerequisite for release, not the discharge of the
  MIT/BSD obligation. `NOTICE` says so and a test asserts that statement survives.
- **Live-Bevy experiment readiness.** Unchanged: CLAUDE.md's prohibition stands and this pass did not
  approach it.
- **EB-S04 re-baseline.** An owner decision about discarding a scientific reference point (DEC-1).
