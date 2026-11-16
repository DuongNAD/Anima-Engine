// The committed list of Playwright specs that `npm run test:e2e --prefix tests` must schedule.
//
// This is a decision record the gate (`scripts/check_e2e_specs.mjs`) enforces, modelled on
// `scripts/test_target_policy.mjs`. It exists because `test:e2e` was once narrowed to
// `playwright test e2e/tiers/`: `ipc_contract`, `console_hygiene`, `phase3_adversarial` and
// `phase6_adversarial_stress` silently stopped running while CI stayed green. `ipc_contract` is the
// spec that had caught five wrong IPC parameter keys across four commands.
//
// Three rules keep it a record and not a suppression list:
//
//  1. A spec the run schedules but this list does not name fails the gate.
//  2. A spec this list names but the run does not schedule fails the gate (the narrowing case).
//  3. A `*.spec.ts` file on disk that is neither scheduled nor named in `EXCLUDED_SPECS` with a
//     reason fails the gate, so a spec cannot be orphaned by a config change either.
//
// Paths are relative to the repository root.

/** Specs that `playwright test --config e2e/playwright.config.ts` must schedule. */
export const SCHEDULED_SPECS = [
  'tests/e2e/console_hygiene.spec.ts',
  'tests/e2e/ipc_contract.spec.ts',
  'tests/e2e/phase3_adversarial.spec.ts',
  'tests/e2e/phase6_adversarial_stress.spec.ts',
  'tests/e2e/rw_budget.spec.ts',
  'tests/e2e/tiers/tier1_features.spec.ts',
  'tests/e2e/tiers/tier2_boundaries.spec.ts',
  'tests/e2e/tiers/tier3_combinations.spec.ts',
  'tests/e2e/tiers/tier4_realworld.spec.ts',
  'tests/e2e/tiers/tier5_adversarial.spec.ts',
];

/** Spec files that exist on disk and are deliberately not part of `test:e2e`. Each needs a reason. */
export const EXCLUDED_SPECS = [
  {
    name: 'tests/e2e/canonical_views.spec.ts',
    reason:
      'A GPU-bound producer of the canonical map view PNGs, run by `npm run capture:views` with ' +
      '`e2e/capture.config.ts`. `playwright.config.ts` lists it in `testIgnore` because on a CI ' +
      'runner it would fall back to a software rasteriser and produce a different picture. CI ' +
      'verifies the committed PNGs through `tests/frontend/mapManifestEvidence.test.ts` instead.',
  },
  {
    name: 'tests/e2e/real_backend.spec.ts',
    reason:
      'Declares no test unless ANIMA_E2E_REQUIRE_BACKEND=1, and then fails closed without a ' +
      'release binary and a tauri-driver session. It is scheduled as a file with zero tests, so ' +
      'it never appears in the list of scheduled specs.',
  },
];
