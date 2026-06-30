# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Anima-Engine is a real-time, GPU-accelerated Artificial Life & Evolution simulator built as a **Tauri v2 desktop app**. Backend is Rust (Bevy ECS + Burn ML) running a 60 FPS background simulation thread; frontend is TypeScript/React/Vite communicating over Tauri IPC. Single Cargo crate (not a workspace); `tests/` is a second npm package with its own `node_modules`.

- `src/` — React/TS frontend (PixiJS 8 for the 2D viewport, three + @react-three/fiber for the 3D landscape).
- `src-tauri/` — Rust backend. Crate `anima-engine`, lib name `anima_engine_lib`. Modules under `src/`: `core/`, `ai/`, `evolution/`, `physics/`, `network/`, `commands/`.
- `tests/` — frontend Vitest (`frontend/`) + Playwright (`e2e/`) suite with its own deps.

## Commands

Frontend (run from repo root):
- `npm run dev` — Vite dev server, fixed port **5173** (`strictPort`).
- `npm run build` — `tsc && vite build`; typechecks first, builds two entries (`index.html`, `landscape.html`).
- `npm run test` — Vitest over `src/**`.
- `npm run test:frontend` — Vitest over the dedicated `tests/` suite (`--root tests`). This is the suite handoff docs use.
- `npm run lint` — ESLint (flat config in `eslint.config.js`). Errors block; legacy `any` and unused-var issues are warnings only.

Backend (run from `src-tauri/`):
- `cargo test` — unit + integration tests.
- `cargo clippy` — Rust linter (ships with the toolchain).
- `cargo build --release` — required before Playwright E2E (it expects the release binary in `src-tauri/target/release/`).

There is no Makefile or CI. `tsc` strict mode runs on build; ESLint (frontend) and clippy (backend) are the linters. Edited `.rs` files are auto-formatted by rustfmt via a PostToolUse hook (`.claude/hooks/rustfmt-on-edit.ps1`).

## Gotchas

- **Test-mode mock aliasing.** When Vitest runs (`mode === 'test'`), `three` and `@react-three/fiber` are aliased to mocks in `tests/mocks/`. These aliases are duplicated in `vite.config.ts`, `tsconfig.json` paths, and `tests/vitest.config.ts` — keep them in sync. Tests run under jsdom; WebGL/R3F are mocked. `PixiViewport.tsx` falls back to a Canvas 2D path under Vitest to avoid jsdom WebGL crashes.
- **Zero-heap-allocation hot loop.** Simulation tick systems (physics, CPG, collision) must not allocate on the heap; use pre-allocated buffers. Tests assert `allocs == 0` — don't introduce allocations in tick paths.
- **Two Vitest configs.** Root `vite.config.ts` includes `src/**`; `tests/vitest.config.ts` includes `tests/frontend/**` with `testTimeout: 15000` and re-pins `react`/`react-dom`.
- **Running the full Bevy/Tauri backend (`npm run tauri dev` / `cargo run`) is heavy and has crashed the dev machine.** For 3D model / rabbit work, serve `rabbit-standalone/` statically instead (e.g. `py -m http.server 8000`).
- Ignore build-artifact noise: `src-tauri/target_*` dirs, `*.log`, `err*.txt`, `out*.txt`, and the root `.agents/` / `$.agents/` vendored cargo dirs.

## Environment (Rust backend, loaded via dotenvy from `.env`, gitignored)

- `ANIMA_USE_GPU` — burn-wgpu GPU vs ndarray CPU fallback (`ai/model.rs`).
- `GEMINI_API_KEY` — Gemini REST in `evolution/meta_ai.rs`; absent → mock fallback.
- `GEMINI_WEBSESSION_ENDPOINT` — Gemini Web-Session endpoint for `GeminiWebSessionClient`.
- Neo4j credentials (`evolution/lineage.rs`) — falls back to in-memory offline mode when Neo4j is unavailable.

## Code style

- TypeScript strict suite: `strict`, `noUnusedLocals`, `noUnusedParameters`, `noImplicitReturns`, `noFallthroughCasesInSwitch`. Path alias `@/*` → `src/*`. No Prettier/ESLint — match surrounding style.
- Rust edition 2021; no `rustfmt.toml` (defaults).

## Tauri IPC contract

Commands and events are documented in `PROJECT.md` ("Interface Contracts") — read it before changing the IPC surface. Commands include `get_simulation_status`, `toggle_simulation`, `get_map_elites_grid`, `get_pheromone_grid`, `get_lineage_graph`, `save_simulation_state`/`load_simulation_state`; events include `simulation-tick`, `map-elites-update`, `pheromone-update`, `chronicle-event`, `migration-event`.
