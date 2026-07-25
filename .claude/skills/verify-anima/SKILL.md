---
name: verify-anima
description: Run the full Anima-Engine acceptance check — backend cargo test, frontend Vitest suite, and the tsc/Vite build — and report pass/fail. Use to verify changes before committing or handing off.
---

Run the project's documented acceptance checks in order and report a clear pass/fail summary for each. Do not stop at the first failure — run all three so the user sees the full picture, then summarize.

1. **Backend tests** — from `src-tauri/`: `cargo test`
2. **Backend lint** — from `src-tauri/`: `cargo clippy`
3. **Frontend tests** — from repo root: `npm run test:frontend`
4. **Frontend lint** — from repo root: `npm run lint`
5. **Build / typecheck** — from repo root: `npm run build` (`tsc && vite build`)

Notes:
- Do NOT run the full backend app (`npm run tauri dev` / `cargo run`) — it is heavy and has crashed the dev machine. Only `cargo test` and the build are part of verification.
- If `cargo test` fails to find a GPU or Neo4j, that is expected — the backend has CPU and offline fallbacks; only treat actual test failures/compile errors as failures.
- After running, report: ✅/❌ per step, plus the key failing output (compile errors, failed test names) if any. End with a one-line overall verdict.
