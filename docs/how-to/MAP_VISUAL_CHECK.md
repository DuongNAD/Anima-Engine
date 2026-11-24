---
title: Capture and Compare 3D Map Views Before and After a Visual Change (capture:map, Prototype)
status: active
owner: maintainers
last_reviewed: 2026-11-19
review_cycle: per-release
---

# Capture and Compare 3D Map Views Before and After a Visual Change

> *Vietnamese version / Bản tiếng Việt: [MAP_VISUAL_CHECK.vi.md](MAP_VISUAL_CHECK.vi.md).*

**Prototype.** A tool for inspecting a change under `src/components/Landscape/` (terrain, water, waterfalls, vegetation, sky, creatures, materials, LOD) without taking screenshots manually. It is **not** a gate: nothing passes or fails based on how the images look.

```powershell
npm run capture:map -- --label before
```

```powershell
npm run capture:map -- --label after --baseline artifacts/map-captures/<before-run-dir>
```

The command is the same in PowerShell and zsh. The script (`scripts/capture_map.mjs`):

1. reuses an existing Vite dev server on `127.0.0.1:5173` if it serves **this exact checkout**, otherwise starts Vite and stops it when done (if another worktree holds the port, the script refuses and suggests `--port`);
2. opens `landscape.html?replay=synthetic&agents=200&segments=5&quality=high` in installed Google Chrome (Playwright `channel: 'chrome'`, no downloads), waiting until world generation finishes (`window.__world` and the terrain mesh exist in the scene);
3. computes about eight viewpoints **from world data**: overview, horizon, shoreline near map center, highest peak, largest river valley, densest forest cell, tallest waterfall, and close-up on one creature (following it). Any viewpoint whose target is absent is recorded as `skipped` with a reason;
4. for each view: captures a PNG image, median and p5 FPS, and draw calls and triangles (`window.__animaFrameCounters`); across the whole run: logs console and page errors, classifying missing-Tauri-backend messages separately;
5. writes under `artifacts/map-captures/<timestamp>/` (gitignored): PNG images, `report.json`, and `index.html` placing the baseline run side by side with FPS deltas.

The camera is positioned from the script via the `window.__worldScene.onBeforeRender` hook already exposed by the page for tooling, so no code in `src/` was added for this tool and a baseline can even be captured on older commits.

## Reading the Results

- FPS is **uncapped by vsync** by default (`--disable-gpu-vsync`) so faster GPUs reveal remaining headroom. Two runs of identical code on Mac M5 varied by up to ~25% in one view (measured 2026-11-19): FPS differences smaller than that are noise. Draw call and triangle counts are stable, except for views containing creatures.
- `environment.softwareGl: true` means the browser is using software rendering; in that case FPS values are meaningless.
- Creatures come from the `?replay=synthetic` fixture: procedurally generated test data, **not** live simulation output.

## What This Tool Does Not Show

- The real desktop app (Tauri webview with the simulation running behind it).
- Performance on the target Iris Xe machine: reported FPS reflects the GPU of whichever machine ran the script.
- Byte-identical reference images: that is handled by `npm run capture:views`, a separate tool.

Instructions for Claude Code sessions live in skill [`.claude/skills/map-visual-check/SKILL.md`](../../.claude/skills/map-visual-check/SKILL.md).
