# CLAUDE.md — Rust backend

## Environment (loaded via dotenvy from `.env`, gitignored)

- `ANIMA_USE_GPU` — burn-wgpu GPU vs ndarray CPU fallback (`ai/model.rs`).
- `GEMINI_API_KEY` — Gemini REST in `evolution/meta_ai.rs`; absent → mock fallback.
- `GEMINI_WEBSESSION_ENDPOINT` — Gemini Web-Session endpoint for `GeminiWebSessionClient`.
- Neo4j credentials (`evolution/lineage.rs`) — falls back to in-memory offline mode when Neo4j is unavailable.
