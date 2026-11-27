## Summary

- **Goal & Card / Issue:**
- **Target branch:** `feat/ecological-selection` (never `main`; see [CONTRIBUTING.md](https://github.com/DuongNAD/Anima-Engine/blob/feat/ecological-selection/CONTRIBUTING.md#where-a-change-goes))
- **Scope / touched paths:** *(explicit paths staged with `git add <path>`; note owner `R5-AUTH` line if touching R5 files or `src-tauri/src/**`)*

## Checks Run (`CONTRIBUTING.md` Gates)

Check every gate executed locally before opening or marking ready for review (see [CONTRIBUTING.md](https://github.com/DuongNAD/Anima-Engine/blob/feat/ecological-selection/CONTRIBUTING.md#gates)):

- [ ] Explicit staging (`git add <path>...`, zero untracked scratch/build files) & `git diff --check`
- [ ] Frontend & docs gates (as applicable): `npm run check:docs`, `npm run check:text-hygiene`, `npm run check:root-hygiene`, `npm run check:conflict-markers`, `npm run check:ipc`, `npm run check:e2` / `check:e3` / `check:e3b` / `check:sel2p2`, `npm run lint`, `node scripts/eslint_ratchet.mjs`, `npm run typecheck:tests`, `npm run test`, `npm run test:frontend`, `npm run build` + `check:bundle-closure` / `check:csp` / `check:bundle`, `npm run check:audit`, `node scripts/check_e2e_specs.mjs` + `npm run test:e2e`
- [ ] Rust & compliance gates (from `src-tauri/` / repo root, as applicable): `cargo fmt --check`, `cargo audit`, `cargo deny check`, `cargo clippy` (`--features desktop`, `-p anima-worldgen`, `--no-default-features`), `cargo test --lib export_bindings`, `cargo test --features desktop` + `check_test_targets.mjs`, `cargo test -p anima-worldgen` + `check_test_targets.mjs --profile worldgen-cpu`, `cargo modules` / `verify_dataflow.py`, compliance (`check:compliance`, `check:licenses-complete`, `verify:upstream-licenses`, `check:flora-footprint`)

## Peer Verification & Reviewer Note

- **Peer verdict link / reviewer:** *(e.g. hub `verdicts/<CARD>_<sha8>.md` or human review note)*
- **The most likely way this is wrong:** *(name the highest-risk assumption or place the reviewer should verify first)*

## License (`MIT OR Apache-2.0`)

- [ ] Submitted under the project's dual license (`MIT OR Apache-2.0`, inbound = outbound per [CONTRIBUTING.md](https://github.com/DuongNAD/Anima-Engine/blob/feat/ecological-selection/CONTRIBUTING.md#license-of-contributions-inbound--outbound)); any third-party code or assets are explicitly named with upstream source and compatible license (no GPL, AGPL, SSPL, source-available, or unlicensed code).
