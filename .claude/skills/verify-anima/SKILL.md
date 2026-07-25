---
name: verify-anima
description: Run the full Anima-Engine acceptance check — backend cargo test, frontend Vitest suite, and the tsc/Vite build — and report pass/fail. Use to verify changes before committing or handing off.
---

Run the project's documented acceptance checks in order and report a clear pass/fail summary for each. Do not stop at the first failure — run all three so the user sees the full picture, then summarize.

1. **Backend tests** — from `src-tauri/`: `cargo test --features desktop`
2. **Backend lint** — from `src-tauri/`: `cargo clippy --all-targets --features desktop`
3. **Frontend tests (src suite)** — from repo root: `npm run test`
4. **Frontend tests (tests/ suite)** — from repo root: `npm run test:frontend`
5. **E2E** — from repo root: `npm run test:e2e` (Playwright starts its own Vite server)
6. **Frontend lint** — from repo root: `npm run lint`, then `node scripts/eslint_ratchet.mjs`
7. **Build / typecheck** — from repo root: `npm run build` (`tsc && vite build`)

Notes:
- Do NOT run the full backend app (`npm run tauri dev` / `cargo run`) — it is heavy and has crashed the dev machine. Only `cargo test` and the build are part of verification.
- **`--features desktop` is not optional.** A bare `cargo test` compiles the seven `networking`/`ml-wgpu`-gated test files into empty binaries that report `running 0 tests` and exit 0 — 1,877 lines of migration, cross-shard and GPU-fallback coverage silently skipped. To check for that, capture the output and run `node scripts/check_test_targets.mjs <file>`; it fails on any target that ran zero tests.
- If `cargo test` fails to find a GPU or Neo4j, that is expected — the backend has CPU and offline fallbacks; only treat actual test failures/compile errors as failures.
- After running, report: ✅/❌ per step, plus the key failing output (compile errors, failed test names) if any. End with a one-line overall verdict.
