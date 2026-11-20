// The owner-approved exemptions for the npm advisory gate.
//
// This is a decision record the gate enforces, not a suppression list. It follows the same rules as
// `scripts/test_target_policy.mjs` and `src-tauri/.cargo/audit.toml`:
//
//  1. Every entry carries a reason. An entry with an empty reason fails the gate.
//  2. Every entry is conditional on a fact the gate re-checks on every run: the advisory must not
//     reach a shipped dependency. `scripts/check_npm_audit.mjs` runs `npm audit --omit=dev` for each
//     package and fails if an exempted advisory shows up there.
//  3. Every entry must still apply. An exemption whose advisory no longer appears in any package
//     fails the gate, so the entry is removed in the same change that makes it unnecessary and the
//     list cannot rot into a blanket exemption.
//  4. Nothing outside the list is tolerated. A new high or critical advisory fails until somebody
//     either fixes it or writes down here why it cannot hurt a user.
//
// What an exemption does NOT mean: that the advisory is harmless. It means the vulnerable code runs
// only on a developer's or CI's machine, against the project's own files, and never in the app a
// user installs. If that stops being true, rule 2 turns the gate red.

/**
 * Advisories of severity high or critical that are tolerated because they reach development tooling
 * only. `advisory` is the GHSA id, the last path segment of the advisory URL `npm audit` prints.
 */
export const DEV_ONLY_EXEMPTIONS = [
  // Empty since 2026-11-19. The last entry, GHSA-fc5q-39xc-vjpf (picomatch ReDoS), was removed
  // when GitHub withdrew that advisory (withdrawn_at 2026-11-18T15:38:56Z): `npm audit` stopped
  // reporting it, and rule 3 failed the gate on the stale entry.
];
