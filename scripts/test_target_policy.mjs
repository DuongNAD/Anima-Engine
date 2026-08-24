// The owner-approved policy for Rust test targets that run nothing, and Rust tests that are
// deliberately not run.
//
// This is a decision record the gate enforces, not a suppression list. Three rules keep it that way:
//
//  1. Every entry carries a reason. An entry with an empty reason fails the gate.
//  2. Every entry must still apply. A stale entry — an allow-listed empty target that now has tests,
//     or an allow-listed ignore that no longer exists — fails the gate. The list cannot rot into a
//     blanket exemption for things nobody has looked at in a year.
//  3. Nothing outside the list is tolerated. A new zero-test target or a new `#[ignore]` fails until
//     somebody writes down why it is intended.
//
// `scripts/check_test_targets.mjs` reads this. Keep it beside that script, and keep the reasons in
// the language of *why this is designed to be empty/ignored*, not *why it is currently broken*.

/**
 * Targets that legitimately report `running 0 tests`.
 *
 * A target that runs nothing exits 0, which is why this needs a decision rather than a default.
 * Feature-gated targets are NOT here: they carry `required-features` in `Cargo.toml`, so Cargo does
 * not schedule them at all in a configuration where they cannot exist — see `featureGated` below.
 */
export const EMPTY_TARGETS = [
  {
    name: 'src/main.rs',
    reason:
      'The binary target. `main.rs` calls `anima_engine_lib::run()` and does nothing else, so it ' +
      'has nothing to assert; every line it could test lives in the lib target, which runs ~400 ' +
      'tests. A test here would be a test of Tauri`s entry point, which needs a window.',
  },
  {
    name: 'doc-tests:anima_engine_lib',
    reason:
      'Doc-test target for the engine library. The crate documents with prose and links rather ' +
      'than runnable examples: nearly every type needs a live Bevy `World`, a seeded RNG or a ' +
      'terrain artifact to construct, so an example short enough to read would not be an example ' +
      'of anything. An absent doc example is not a missing test.',
  },
  {
    name: 'doc-tests:anima_domain',
    reason:
      'Doc-test target for the shared domain crate. Same reason, plus the domain crate is ' +
      'deliberately small and pure: its behaviour is covered by 20 unit tests in the same file as ' +
      'the code, which is where a reader of `anima-domain` looks.',
  },
];

/**
 * Tests that are deliberately `#[ignore]`d.
 *
 * The bar for an entry is that the test is an **operational or diagnostic entry point**, not a
 * regression test somebody could not make pass. Two ignores that used to be here were removed
 * rather than described, because neither cleared that bar:
 *
 * - `ae210_regenerate_fixtures` wrote fixture files into the repository. A writer nobody runs
 *   guards nothing, and the fixtures had in fact gone stale behind it (they predated ADR-0004`s
 *   `observer` field). It is now `cargo run --example gen_ae_fixtures`, with
 *   `ae210_fixtures_match_the_serializer` as the gate.
 * - `diagnose_round_trip_fidelity` documented itself as *expected to fail*, blaming serde_json`s
 *   f64 writer. The writer was exact; the default parser was not. `float_roundtrip` fixes it and
 *   the diagnostic is now an ordinary passing gate.
 *
 * `name` is matched against the fully-qualified test name libtest prints.
 */
export const IGNORED_TESTS = [
  {
    name: 'child_entry_point',
    target: 'tests/determinism_gate_tests.rs',
    reason:
      'A subprocess entry point, not a test. The G1.3 gate needs two *processes* — `HashMap` ' +
      'iteration order, allocator reuse and `Uuid::new_v4()` are per-process, so two worlds in ' +
      'one process agree with each other while both disagree with tomorrow. The parent tests ' +
      're-execute this binary with `--exact child_entry_point --ignored` and a private `ANIMA_G13_ROLE` ' +
      'env var, then compare the checksums the children print. Running it directly in the ordinary ' +
      'suite would do nothing useful: with no role set it has no scenario to run. The real ' +
      'subprocess behaviour is proven by `two_independent_processes_replaying_the_same_manifest_agree` ' +
      'and `a_checkpoint_continuation_in_another_process_agrees_with_an_uninterrupted_run`, which ' +
      'fail loudly if a child prints no checksum.',
  },
  {
    name: 'diagnose_which_system_moves_the_closed_total',
    target: 'tests/energy_conservation_tests.rs',
    reason:
      'A bisector for conservation bugs, not a gate. It runs each of the ten EU-moving systems in ' +
      'isolation for thousands of ticks and prints the drift of each, so that when the aggregate ' +
      'gate goes red somebody can find out *which* system broke without editing the test. It ' +
      'asserts nothing by design — a print-only run converted into an always-pass assertion would ' +
      'be a slow test that proves nothing. The invariant it helps diagnose is gated by ' +
      '`live_world_conserves_energy_across_births_deaths_and_a_save_load_cycle` and the ' +
      'per-transaction tests beside it.',
  },
];

/**
 * Integration targets that carry `required-features` in `Cargo.toml`.
 *
 * These are the opposite of an empty-target exemption: the point is that they are *absent* from a
 * configuration that cannot build them, rather than present and empty. The gate checks both halves,
 * which is what stops someone "fixing" a missing target by deleting the `required-features` line
 * and getting seven silent empty passes back.
 *
 * `src-tauri/tests/feature_gated_targets_tests.rs` ties each entry to the crate-level
 * `#![cfg(feature = ...)]` in the file itself, so the two statements of one fact cannot drift.
 */
export const FEATURE_GATED_TARGETS = [
  { name: 'tests/adversarial_challenger_tests.rs', feature: 'networking' },
  { name: 'tests/migration_tests.rs', feature: 'networking' },
  { name: 'tests/migration_high_throughput_tests.rs', feature: 'networking' },
  { name: 'tests/migration_narrow_bounds_loop_tests.rs', feature: 'networking' },
  { name: 'tests/migration_panic_tests.rs', feature: 'networking' },
  { name: 'tests/migration_stress_tests.rs', feature: 'networking' },
  { name: 'tests/phase5_websocket_reuse.rs', feature: 'networking' },
  { name: 'tests/phase5_burn_wgpu_fallback.rs', feature: 'ml-wgpu' },
];

/** Which features each named profile turns on. Used to decide "absent" vs "must run". */
export const PROFILES = {
  default: [],
  desktop: ['neo4j', 'networking', 'ml-wgpu'],
};
