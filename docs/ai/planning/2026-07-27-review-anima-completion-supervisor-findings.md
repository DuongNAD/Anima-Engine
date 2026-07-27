---
kind: review-findings
feature: anima-completion
title: Supervisor findings — independent review after the first completion session
description: The acceptance addendum the completion pass was held to, tracked verbatim
status: historical
created: 2026-07-27
owner: maintainers
reconciled_in: ./2026-07-27-feature-anima-completion.md
---

> **Vendored evidence, tracked verbatim.** This document was written outside the repository, in the
> ignored `.agents/` scratch directory, and two lifecycle documents cited it there. That is exactly
> what its own item 18 objects to: an ignored path is absent from a clean checkout, so the citation
> resolved for one machine and for nobody else. It is now tracked. The body below is byte-identical
> to the file it was copied from; only this note and the frontmatter are added.
>
> Its reconciliation — which finding closed, under which gate, in which commit — lives in
> [§2.2 of the plan](./2026-07-27-feature-anima-completion.md). That table was written when this
> document held **twelve** items and covers 1–12; items 13–18 were appended to this file afterwards
> and are not reconciled row by row. Most of them extend a finding already in the table. One
> sub-point is **not** claimed closed anywhere and should be read as open: the capture-reproducibility
> contract in items 13 and 14 — fixed animation time, seeded `WorldSky` star RNG, and the same view
> captured twice from clean loads compared by SHA-256.

# Supervisor findings for the next Claude pass

These are independent review findings discovered after the first Claude session started. They are
hard acceptance inputs, not optional ideas. Do not mark the completion pass done until each item is
fixed with a regression gate or explicitly proven false.

## Critical / high

1. **The newly generated World Artifact is real bytes for the wrong world identity.**
   - Shipped app identity: `src/utils/sharedWorld.ts` declares `SHARED_WORLD_SEED = "seed"`,
     `SHARED_WORLD_SIZE = 2048`, `SHARED_WORLD_SHAPE = "continent"`.
   - Shipped flow: the app generates that 2048² authoritative world and `worldToArtifact` downsamples
     it to the 256² simulation artifact.
   - Commit `8e6a165` instead runs `generateWorld("1337", { size: 256, shape: "continent" })` and
     encodes that directly at 256².
   - Therefore the checksum is non-placeholder but does not identify the world the app renders.
   - Fix the generator to import the shared identity, generate the authoritative 2048² world, and
     downsample it to the backend working dimension. The evidence test must bind the manifest's
     generated identity to `sharedWorld.ts`, not hard-code a second identity.
   - Apply the same identity rule to `gen_map_manifest.ts`; a validator score of 100 for an unrelated
     generated world is not acceptance evidence.
   - Commit `8e6a165` also breaks the production TypeScript gate: `npm run build` exits 2 because
     `src/__tests__/mapManifestEvidence.test.ts` imports `node:fs`, `node:crypto`, `node:path`, and
     `node:url` while the app tsconfig does not include Node types. Keep the evidence test in an
     appropriate Node-aware test project/config (or provide a narrowly scoped type config) without
     polluting browser production types; rerun `tsc && vite build`.

2. **Default walking spawn intersects/abuts solid flora.**
   - Browser reproduction on `landscape.html`, real world, walk mode: camera at approximately
     render `(x=-129,z=-94)` is inside or immediately against a large trunk/canopy, with the central
     view completely occluded.
   - Pre-fix evidence:
     `C:/Users/Admin/.claude/jobs/427d2287/tmp/evidence/codex-browser-captures/spawn-pre-fix.png`.
   - The manifest exporter selects a walkable cell but does not reject flora collider clearance.
   - `findSpawn` / capture spawn and manifest navigation must share a deterministic clearance rule.
     Add a failing test with real flora positions and the real walk collider radius.
   - The current runtime push-out has a second edge case: it only resolves a trunk overlap when
     `d2 > 1e-6`; a player exactly at the trunk center (`d2 == 0`) is never pushed out. Extract one
     shared `isTallFlora`/collider-radius/clearance policy for spawn, manifest, capture, and runtime,
     and choose a deterministic fallback direction for the zero-distance overlap.

3. **Canonical visual evidence is still missing.**
   - MCP discovery after `8e6a165`: missing `overview`, `navigation`, `collision`, `lighting`,
     and `spawn`.
   - `captured:false` is honest but cannot satisfy the AGENTS.md hard gate.
   - Add a deterministic, frontend-only capture mode/harness (no full Tauri backend) that fixes
     seed, camera pose, weather, time, quality, and viewport, then produces the canonical images.
   - Rerun, in order: discover, validate, prepare_team_review, inspect_map_views.
   - Inspect matching canonical before/after captures and resolve all critical/high visual findings.
   - An exploratory pre-fix overview exists at:
     `C:/Users/Admin/.claude/jobs/427d2287/tmp/evidence/codex-browser-captures/overview-pre-fix.png`.

4. **Reset knowingly teleports to an unsafe ocean origin.**
   - `WorldShowcase.tsx` correctly computes a scenic land `findSpawn` on initial load.
   - Its reset handler sets `teleportRef.current = { x: 0, z: 0 }`.
   - The same file comments that the origin is usually open ocean; browser reproduction showed
     readout biome `Đại dương`.
   - Reset must return to the validated, flora-clear spawn, not `(0,0)`.
   - Follow-up: the current repair still uses
     `homeRef.current ?? { x: 0, z: 0 }`. That preserves the unsafe destination during the
     initialization race. If no validated home exists yet, Reset must be disabled/no-op or compute
     a validated home; it must never fall back to the ocean origin.

## Soundness / correctness

5. **`BrainModel` does not encapsulate the invariant behind `unsafe impl Sync`.**
   - Commit `b27c28b` materializes lazy Burn params in constructors, which is necessary.
   - But `BrainModel.backend` is still `pub`, and `BrainModelBackend` is public. External code/tests
     can construct or replace a backend carrying unmaterialized lazy params, invalidating the
     unsafe proof.
   - Make the field private and expose narrow safe access/mutation APIs, or structurally eliminate
     the unsafe invariant.
   - The compiler experiment reportedly produced Sync failures, not Send failures. Remove redundant
     `unsafe impl Send` if a compile-time Send assertion passes.
   - Source-scanning for constructor text is not sufficient as the primary soundness gate.

6. **Lineage must not fabricate `RelationType::Clone`.**
   - In commit `57a8246`, an uncompressed planned edge does:
     `original_type.get(&key).copied().unwrap_or(RelationType::Clone)`.
   - A missing original relation is a broken simplify plan/data invariant, not an observed clone.
   - Fail explicitly or preserve a typed error; add a regression test for the impossible/malformed
     plan case.

## Runtime quality / gates

7. **Three.js deprecation warning is emitted repeatedly while the scene runs.**
   - Browser console showed many repeated:
     `THREE.WebGLShadowMap: PCFSoftShadowMap has been deprecated. Using PCFShadowMap instead.`
   - This is log spam and evidence that the shadow mode no longer matches the installed Three
     release. Select the supported shadow mode once and add a browser smoke assertion for no
     repeated console errors/warnings from Anima-owned code.
   - Fresh Playwright output also emits repeated Pixi deprecation warnings from
     `PixiViewport.tsx` at `beginFill` (~144), `endFill` (~154), and `lineStyle` (~159) on every
     redraw. Migrate to the installed Pixi graphics API and make the no-owned-console-warning
     smoke gate cover both the 2D dashboard and landscape scene.

8. **E2E currently has five backend-dependent skips.**
   - Browser isolation improvement is valid: own port 5177, Anima identity asserted, 9 pass / 0 fail.
   - Five live IPC tests still skip because no release binary exists and project safety forbids
     running the heavy full app on this machine.
   - Keep the split explicit: browser E2E must be zero-fail/zero-skip for its declared scope; live
     backend E2E must fail closed when required and be documented as an external/human gate here.
   - The current five "IPC" specs are not valid live-backend E2E even when the binary exists:
     they spawn `src-tauri/target/release/anima-engine` but Playwright still drives an unrelated
     ordinary Vite page, so that page is not connected to the spawned Tauri process or its IPC.
   - The CI workflow says these specs use a page-level Tauri IPC stub and do not require a release
     binary, contradicting the binary gate. Reconcile the contract: convert them to honest browser
     E2E with explicit deterministic Tauri IPC mocks and no binary dependency, or add a separate
     real Tauri WebDriver gate. Do not imply that spawning a detached binary connects it to the page.
   - Remove all layout/assertion catch-and-skip paths (for example missing headings or telemetry
     panels). The global identity check proves the Anima app is being served; absent UI must fail.
     Browser-scope E2E acceptance is zero skips, and any real-backend gate must fail closed when
     explicitly required.
   - Fresh full lint after `f99dfc1` is worse than the baseline contract: it has 1 error in
     `tests/e2e/global-setup.ts:49` (`preserve-caught-error`, rethrow without `cause`) plus
     483 warnings. Fix this regression before any E2E milestone is called green.

9. **Do not forget unfinished master-goal scope.**
   - Save/load path confinement and legacy read-only migration.
   - Generated binding authority/drift gate.
   - ESLint 0 warnings without relaxing rules.
   - NOTICE/license inventory/SBOM.
   - Bundle split/budget.
   - Live-Bevy experiment readiness, persistence and safe in-app tick capture evidence.
   - Full fresh backend/frontend/build/audit/docs/map gates after all follow-up fixes.
   - ESLint baseline decomposition is concrete: 365 `no-explicit-any`, 53 `no-unused-vars`,
     29 `react-hooks/immutability`, 21 `react-hooks/purity`, 14 `exhaustive-deps`, plus 3 other
     hook warnings. The current plan says this was "not attempted"; the master goal requires
     zero warnings without relaxing rules or excluding affected production/test files.

10. **The save-path patch must implement its own accepted migration contract.**
    - The design in `docs/ai/design/2026-07-27-feature-anima-completion.md` explicitly promises:
      existing absolute-path saves remain loadable through a read-only, explicitly opt-in migration
      path, with a regression test.
    - The current in-progress `save_paths.rs` correctly confines normal names under app data, but
      `load_simulation_state` now rejects every old absolute path and provides no migration reader.
      Do not commit or accept that as complete R7/N3 work.
    - Keep legacy import read-only and clearly separate from ordinary save naming. Ensure a
      compromised webview cannot turn an implicit fallback into arbitrary silent file reads; the
      user must explicitly select/authorize the legacy file. Test normal confinement, traversal,
      Windows device names, legacy opt-in import, and that legacy paths are never write targets.
    - Reconcile the existing startup/exit `default_save.json` flow in `src-tauri/src/lib.rs` with
      the new `saves/` directory and the versioned `SnapshotEnvelope`; do not leave two contradictory
      persistence contracts or silently strand the old autosave.
    - `CLAUDE.md` names `PROJECT.md` as the authority for IPC contracts, but commit `378b4f6`
      updates only README. Update the authoritative interface contract and every frontend label/
      placeholder/error state so users understand they enter a save name, not an arbitrary path.

11. **The feature lifecycle is not complete.**
    - Fresh `ai-devkit lint --feature anima-completion` fails because the required implementation,
      deployment, and monitoring documents are absent.
    - Create the correctly dated feature documents, reconcile them with actual commits and evidence,
      and rerun the feature lint. Do not use placeholder `YYYY-MM-DD` files or claim full lifecycle
      completion while this gate is red.
    - The planning/evidence ledger still marks every completed finding `open` and retains an
      obsolete decision that canonical map capture is out of scope. Reconcile each row with its
      actual commit and fresh gate output. The master goal and `AGENTS.md` make canonical captures
      a hard acceptance gate, so the local plan may not waive them.

12. **The in-progress NOTICE generator is not yet a complete inventory or SBOM.**
    - Its JavaScript section reads only direct `package.json.dependencies`; Vite also bundles
      production transitive dependencies. Resolve the actual production dependency closure from
      the lockfile/npm graph, deduplicate exact name+version identities, and test a known transitive
      component so the generator cannot regress to a direct-only list.
    - An inventory grouped by license string is not an SBOM. Produce a deterministic standard SBOM
      (for example CycloneDX JSON) for the shipped Rust + npm graph, wire a freshness/check gate,
      and document its scope.
    - The generated NOTICE itself states that required license texts and copyright holders are not
      packaged. Do not mark R10/distribution readiness closed while the artifact says the legal
      obligation remains unmet. Package the applicable license/notice texts or keep release blocked
      with an exact, honest external legal-review gate.
    - The bundle task currently adds only a ceiling around the existing ~840 KiB Three/R3F chunk.
      That is useful regression protection but does not satisfy the master workstream's requested
      split/improvement by itself. Measure loading behavior and either land a meaningful safe split
      with a tighter post-change budget, or document with evidence why the already lazy chunk is the
      correct architectural boundary and score the remaining size debt honestly.
    - `check:notice` and `check:bundle` must run in CI after their prerequisites; merely adding
      package scripts does not make them release gates.
    - Treat edits to proprietary `LICENSE` as legal text, not ordinary documentation. Do not assert
      ownership or relicensing facts that repository evidence cannot establish. Preserve an
      explicit maintainer/legal approval gate for the scope language and distribution obligations.

13. **The first data-derived canonical-view set is technically captured but not visually accepted.**
    - Independent inspection of the latest eight `map-views/*.png` rejects `collision.png`,
      `water.png`, `biome_transition.png`, and `ecosystem.png`: all expose the hard square world
      boundary prominently along the right edge. Move the camera inside the render bounds and/or
      turn it inward while keeping the evidence subject visible.
    - `navigation.png` is a scenic aerial view but contains no route, reachable endpoints, navmesh,
      or other visible navigation evidence. Add a deterministic route/debug overlay or a paired
      machine-verifiable reachability record; a landscape photograph alone does not prove the
      navigation gate.
    - `collision.png` shows a dense canopy from above, not collider behavior or trunk clearance.
      Add deterministic collider/clearance visualization or bind the image to a complementary
      machine-verifiable collision record. Do not label density alone as collision evidence.
    - The harness claims the clock is fixed, but only the day/night state is paused. `WorldWater`
      advances shader `uTime` from R3F's elapsed clock, vegetation and wildlife also use elapsed
      time, and `WorldSky` seeds stars with `Math.random()`. Capture the same view twice from clean
      page loads and compare SHA-256. If it differs, add an explicit fixed render-time/randomness
      contract for capture mode before describing the PNGs as reproducible.
    - Regenerate `map_manifest.json` immediately after the final accepted captures and have its
      evidence test verify PNG byte size and SHA-256, not merely existence. Commit the matching
      tracked `artifacts/world_256.anmw`; commit `6adf6e3` currently points the manifest at the
      working artifact checksum but omitted that artifact, so a clean checkout is inconsistent.

14. **Commit `fd3dc79` closes F3 with stale poses and unproven reproducibility.**
    - Worldgen changed from version 20 to 21 after the camera poses were derived. The v21 check
      reports actual `findSpawn = (-128.7, -93.5)` in render coordinates, but
      `CANONICAL_VIEW_CAMERAS.spawn` and its comment still claim `(-100.5, -86.5)`. Rerun
      `scripts/derive_view_cameras.ts` after the final worldgen change, update all poses/reasons,
      recapture all images, and regenerate both manifests. Camera evidence may not describe a
      superseded world version.
    - The commit message says the capture clock is pinned, but `WorldWater` still uses
      `state.clock.getElapsedTime()`, vegetation/wildlife animate from the same render clock, and
      `WorldSky` still initializes stars with `Math.random()`. Run the same view twice from clean
      page loads and compare its SHA-256. A mismatch is a failing reproducibility gate, not an
      acceptable GPU variation; introduce a fixed capture animation time and seeded capture RNG.
    - The new walk-boundary clamp was implemented after visual discovery without a regression
      test. Extract a testable boundary policy and prove walk is confined to `renderSize/2` while
      spectator modes retain the wider framing limit.
    - The committed images still prominently show the square terrain edge in `collision.png`,
      `water.png`, `biome_transition.png`, and `ecosystem.png`. Runtime walk confinement closes
      the boundary escape but does not repair those canonical compositions. Reframe/turn inward
      and recapture; the acceptance criterion also says no visible severe seam/edge artifact.
    - `map_manifest.json._generated.note` now says the binary world artifact is gitignored, but
      `artifacts/world_256.anmw` is tracked and committed. Correct the generated note and test the
      clean-checkout artifact/manifest relationship.

15. **The in-progress legacy-import listing and resolver disagree on file identity.**
    - `list_legacy_saves` accepts any filename for which `sanitize_save_name(name).is_ok()` and
      returns the original directory-entry name.
    - `resolve_legacy_import_path` then normalizes that returned name again. For example, a real
      drop-file named `old.txt` is listed as `old.txt`, but importing it resolves
      `legacy-import/old.txt.json`, which does not exist. Only list names that are already in their
      canonical `.json` form, or return the canonical identity while ensuring it still names the
      actual file.
    - Add an end-to-end filesystem test that creates a legacy bare save, imports it, proves the
      destination is a current `SnapshotEnvelope`, and byte-compares the source before/after. The
      current resolver-only tests do not prove the accepted read-only migration behavior.
    - The test named `nothing_writes_into_the_legacy_import_directory` currently demonstrates that
      `resolve_save_path(&import_dir(), ...)` *can* produce a write path there; comments are not a
      structural prohibition. Test the real import command/helper with separate fixed source and
      destination roots instead of claiming this resolver property proves the command cannot write
      to its source.
    - Commit `ae1fb40` registers `list_legacy_saves` and `import_legacy_save` but adds no frontend
      control that calls either command. A user cannot discover the drop directory, refresh its
      contents, select a legacy file, choose `save_as`, or see import errors. The migration is not
      an explicitly usable product path until this UI and its mocked IPC/E2E coverage exist.

16. **Commit `766609e` improves inventory coverage but overstates “what ships.”**
    - The npm inventory uses the complete `package.json` production dependency closure, not the
      modules reachable in Vite's emitted bundle. The repository currently misclassifies
      `@playwright/test` and `@types/three` as production dependencies even though they are
      test/type tooling; the SBOM consequently includes Playwright, browser binaries' support
      packages, and TypeScript declarations as if Vite bundled them. Move non-runtime packages to
      `devDependencies` and/or distinguish conservative production install closure from the actual
      bundle closure instead of saying every listed npm package is inside `dist/`.
    - Validate `sbom.cdx.json` against the CycloneDX 1.5 JSON schema, not only self-authored shape
      assertions. The generator currently accepts many raw manifest license strings as SPDX
      expressions without an SPDX parser; a schema/semantic validator should fail malformed output.
    - NOTICE still explicitly says required MIT/BSD license texts and copyrights are not packaged.
      That is honest, but F12/release readiness remains blocked; task status must not call the
      distribution/legal work complete. Either package the applicable texts with verifiable bundle
      configuration or retain an explicit release-blocking owner/approval gate and score it open.

17. **Binding authority must cover every IPC boundary, not document selected hand mirrors as gaps.**
    - The current F9 work explicitly plans to leave `LineageNode`, `LineageLink`,
      `LineageGraphState`, and `MigrationPayload` hand-written in `App.tsx` because their Rust
      sources lack `ts-rs`. That is the defect F9 is meant to remove, not an acceptable end state.
    - Derive/export the missing Rust payloads (including nested enums/status/direction types), use
      the generated types in the frontend, and make the existing regeneration/drift CI gate cover
      them. Do the same audit for every `invoke` result and Tauri event payload; zero untracked
      hand-mirrored IPC types is the acceptance criterion.

18. **Lifecycle docs must not depend on ignored supervisor scratch state or close open findings.**
    - The in-progress implementation doc links
      `../../../.agents/anima-completion-supervisor-findings.md`; `.agents/` is ignored and absent
      from a clean checkout, so this is not durable evidence and may fail the documentation link
      gate. Reconcile findings into tracked lifecycle/evidence docs and link those instead.
    - Do not describe reset/map capture/binding/licensing/lint work as completed while findings
      13–17 remain open. Lifecycle lint proves file presence/frontmatter, not factual accuracy.
