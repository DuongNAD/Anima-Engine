---
name: map-visual-check
description: Screenshot and FPS check of ~8 views of the 3D world map (overview, horizon, coast, highest peak, river valley, forest, waterfall, creature close-up) with `npm run capture:map`, before and after a change, side by side. Use for any change that alters how the map looks or renders — terrain, water/ocean, waterfalls, vegetation/flora, sky/weather/lighting, creature (live-agent) rendering, materials, textures, LOD or quality tiers under src/components/Landscape/ — and when reviewing another agent's visual PR.
---

# Map visual check (prototype)

`npm run capture:map` opens `landscape.html?replay=synthetic` in Chrome (Playwright, no Tauri
backend), waits for world generation, flies the camera to about eight views derived from the world
data, and writes per view a PNG, median/p5 FPS and draw calls/triangles, plus every console error of
the run, to `artifacts/map-captures/<timestamp>/` (gitignored) with a contact sheet `index.html`.
About one minute on an M-series Mac. Same command on Windows (PowerShell) and macOS (zsh).

## Do this

1. **Before** the change, on the base commit: `npm run capture:map -- --label before`.
   Reviewing a PR: run it once on the base branch and once on the PR branch.
2. **After** the change:
   `npm run capture:map -- --label after --baseline artifacts/map-captures/<before folder>`.
3. **Look at every view.** Read each PNG of both runs (`<run>/NN-<view>.png` and
   `<run>/baseline/NN-<view>.png`) — not only the contact sheet, and not only the views you expect
   to change. Say per view what changed, what got better, what got worse. A view you did not open is
   a view you cannot report on.
4. **Report**: a per-view table of FPS median and p5 with deltas, draw calls and triangles with
   deltas (all in `report.json`), new console errors/page errors (`baseline.newConsole`), any
   `skipped` view with its reason, and the GL renderer line. Attach or link the overview and one
   close-up.

Flags: `--agents N` (200), `--segments K` (5), `--quality high|medium|low|auto` (high), `--viewport WxH`
(1280x720), `--seconds S` per-view FPS window (3), `--views coast,peak,...`, `--headed`, `--hud`
(keep the HUD), `--vsync` (keep the display cap; default is uncapped), `--port P`. `--help` lists them.

## Read the numbers correctly

- **Uncapped FPS is noisy.** Two runs of identical code on an M5 Mac differed by up to ~25% on one
  view (2026-11-19). Treat smaller FPS deltas as noise; use `--seconds 6` for a steadier figure.
  Draw calls and triangles are the stable numbers, except on views with creatures, which move.
- `environment.softwareGl: true` (SwiftShader / llvmpipe) means the FPS figures are meaningless.
- Console messages are classified: `pageErrors`, `errors`, `warnings` are ours to explain;
  `expectedNoBackend` (missing Tauri) and `toolchainNoise` are listed, not hidden.
- Creatures are the `?replay=synthetic` fixture — procedural test data, never simulation output.

## What it cannot show

- The real desktop app: this is Chrome under Vite dev, not the Tauri webview, and there is no
  simulation behind the creatures.
- The performance target: FPS here is this machine's GPU, not the Iris Xe laptop. A view that is
  smooth on an M5 or an RTX card can still be too slow there.
- Pixel-exact evidence: the scene keeps animating while captured. The byte-identical canonical views
  are a different tool, `npm run capture:views`.

## If it fails

- "port 5173 is serving another checkout": another worktree's dev server holds the port. Pass
  `--port 5180` (any free port) rather than stopping someone else's server.
- "no usable browser": install Google Chrome. Downloading Playwright's Chromium
  (`npx playwright install chromium`, ~150 MB) is a download: ask the owner first.
- A view `skipped`: its feature is missing from this world (e.g. no waterfalls) or the creature
  layer changed shape; the reason is in `report.json`. Adjust `pageAnalyseWorld` /
  `pageFollowCreature` in `scripts/capture_map.mjs` if a rendering change renamed the meshes.
- Never launch the desktop app (`tauri dev`, `cargo run`) to get these pictures.
