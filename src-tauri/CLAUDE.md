# CLAUDE.md — Rust backend

## Environment (loaded via dotenvy from `.env`, gitignored)

- `ANIMA_USE_GPU` — burn-wgpu GPU vs ndarray CPU fallback (`ai/model.rs`).
- `GEMINI_API_KEY` — Gemini REST in `evolution/meta_ai.rs`; absent → mock fallback.
- `GEMINI_WEBSESSION_ENDPOINT` — Gemini Web-Session endpoint for `GeminiWebSessionClient`.
- Neo4j credentials (`evolution/lineage.rs`) — falls back to in-memory offline mode when Neo4j is unavailable.

## Environment (read by the engine thread, not from `.env`)

All three are read once, inside `SimulationEngine::start` — setting them after the window is open
does nothing. Unset is always the legacy behaviour.

- `ANIMA_FOUNDING_POPULATION` — founders genesis creates (`core/resources.rs`, `FoundingPlan`).
  Unset is **10 on the legacy line and bit-identical to before the knob existed**; any value opts
  into a grid inset from `MapBounds`, because `x = i * 5.0` puts the twenty-first founder on the
  map edge. Capped at 10 000; a malformed value is refused on stderr and genesis uses the default,
  so check that line before trusting a benchmark's agent count.
- `ANIMA_TICK_CAPTURE` — starts a tick capture at engine boot (`core/tick_capture.rs`,
  `CaptureConfig::from_env`).
- `ANIMA_TICK_CAPTURE_OUT` — **a name, not a path.** The completed capture writes itself there
  under the app data directory. This is the only way to retrieve a capture from a **release** build:
  the four capture commands are IPC-only, no UI calls them, and `tauri` here has no `devtools`
  feature, so a release binary has no console to invoke them from.
