//! The E2 seam: the treatment arm the preregistration says does not exist yet.
//!
//! `docs/ai/design/2026-07-27-experiment-e2-evolved-brain-default.md` §3 specifies, **before any
//! run**, exactly what E2-B may build: one initial-condition key, a `BrainPolicy` derived from it,
//! founder brains drawn from a stream of their own, and nothing else. This file is the machine-check
//! on each clause of that specification, and on the two properties the whole comparison rests on:
//!
//! - **E2-G1** — a manifest declaring `live.evolved_brains = 1` builds founders carrying
//!   `AgentBrain`; one that does not declare it builds none.
//! - **E2-G3** — the ecology stream (`SimRng`) is in the *identical* state after genesis in both
//!   arms. Without this the arms would differ in the brain **and** in the realised random sequence,
//!   inseparably, and the experiment would measure a bundle rather than a factor (design §4.3).
//!
//! # Seeds
//!
//! Nothing here runs an experimental seed. The excluded smoke seed 999983 and a synthetic seed are
//! the only ones used, so no part of the preregistered ensemble can be executed — let alone
//! previewed — by running the test suite.
//!
//! # Cost
//!
//! Every test builds a live Bevy world, which loads the terrain artifact and runs the app's genesis.
//! None of them steps a tick: the seam is a *construction* property, and stepping would make this
//! file slow without making it stronger.

use anima_engine_lib::core::components::{Agent, AgentBrain};
use anima_engine_lib::core::ecs::Position;
use anima_engine_lib::core::experiment::{
    ExperimentError, ExperimentManifest, InitialConditionSet, WorldLawSet,
};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_EVOLVED_BRAINS,
};
use anima_engine_lib::core::resources::SimRng;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use bevy_ecs::prelude::*;
use rand::RngCore;

/// The calibration seed. Absent from both experimental manifests, so `run_manifest_seed` would
/// refuse it there — using it here cannot contaminate the ensemble.
const SMOKE_SEED: u64 = 999983;

/// A seed belonging to no E2 manifest at all, for the construction checks that only need *a* seed.
const SYNTHETIC_SEED: u64 = 31337;

/// Where the preregistered manifests live, relative to the crate root.
const PREREG_DIR: &str = "tests/fixtures/experiments_e2";

fn prereg_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(PREREG_DIR)
        .join(name)
}

fn load_manifest(name: &str) -> ExperimentManifest {
    let path = prereg_path(name);
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{name} parses: {e}"))
}

/// The E2 initial conditions, with the declared factor set or not — the two arms, by construction,
/// differing in exactly the one key the preregistration allows.
fn arm_initial(evolved: bool) -> InitialConditionSet {
    let mut values = vec![
        ("live.founders".to_string(), 10.0),
        ("live.predator_fraction".to_string(), 0.3),
        ("live.trees".to_string(), 8.0),
        ("live.lakes".to_string(), 2.0),
        ("live.food_cap".to_string(), 50.0),
    ];
    if evolved {
        values.push((LIVE_KEY_EVOLVED_BRAINS.to_string(), 1.0));
    }
    InitialConditionSet::new(values)
}

fn build_arm(evolved: bool, seed: u64) -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &arm_initial(evolved),
        &[],
        seed,
        (16, 16),
        0,
    )
    .expect("both arms must build")
}

fn build_from(manifest: &ExperimentManifest, seed: u64) -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(
        &manifest.laws,
        &manifest.initial_conditions,
        &[],
        seed,
        (16, 16),
        0,
    )
    .unwrap_or_else(|e| panic!("manifest '{}' must build: {e}", manifest.experiment_id))
}

/// How many agents exist, and how many of them carry a per-agent brain.
fn agents_and_brains(adapter: &LiveExperimentAdapter) -> (usize, usize) {
    let mut world = adapter.world();
    let world = &mut *world;
    let agents = {
        let mut q = world.query_filtered::<(), With<Agent>>();
        q.iter(world).count()
    };
    let brains = {
        let mut q = world.query_filtered::<(), (With<Agent>, With<AgentBrain>)>();
        q.iter(world).count()
    };
    (agents, brains)
}

/// The founding layout, as exact bits, in a deterministic order.
fn founder_layout(adapter: &LiveExperimentAdapter) -> Vec<(u32, u32, u32)> {
    let mut world = adapter.world();
    let world = &mut *world;
    let mut q = world.query_filtered::<&Position, With<Agent>>();
    let mut out: Vec<(u32, u32, u32)> = q
        .iter(world)
        .map(|p| (p.0.x.to_bits(), p.0.y.to_bits(), p.0.z.to_bits()))
        .collect();
    out.sort_unstable();
    out
}

/// The exact state of the world's one ecology stream: its position, and the next eight draws it
/// would hand to the first system that asks.
fn ecology_stream_state(adapter: &LiveExperimentAdapter) -> (u64, u128, Vec<u64>) {
    let mut world = adapter.world();
    let mut rng = world
        .get_resource_mut::<SimRng>()
        .expect("a live world always has SimRng");
    let seed = rng.seed();
    let pos = rng.stream_pos();
    let draws: Vec<u64> = (0..8).map(|_| rng.rng().next_u64()).collect();
    (seed, pos, draws)
}

// ---- The declared factor -----------------------------------------------------------------------

#[test]
fn the_live_world_honours_exactly_one_new_initial_condition_key() {
    assert!(
        LIVE_KEYS.contains(&LIVE_KEY_EVOLVED_BRAINS),
        "the declared factor must be a key the adapter accepts, or the treatment manifest is \
         refused at model construction"
    );
    assert_eq!(
        LIVE_KEY_EVOLVED_BRAINS, "live.evolved_brains",
        "the key is preregistered and may not be renamed"
    );
    assert_eq!(
        LIVE_KEYS.len(),
        6,
        "design §3 adds ONE key; anything else is a factor nobody registered"
    );
}

#[test]
fn absent_means_false_so_every_existing_live_manifest_keeps_its_behaviour() {
    let without = LiveWorldConfig::from_initial_conditions(&arm_initial(false))
        .expect("the control conditions are valid");
    assert!(
        !without.evolved_brains,
        "an absent key must read as the legacy default, never as 'on'"
    );
    assert!(
        !LiveWorldConfig::default().evolved_brains,
        "the default config is the legacy path"
    );

    // Explicit zero is the same thing said out loud.
    let mut values = arm_initial(false).values;
    values.push((LIVE_KEY_EVOLVED_BRAINS.to_string(), 0.0));
    let explicit_off = LiveWorldConfig::from_initial_conditions(&InitialConditionSet::new(values))
        .expect("0.0 is a legal value");
    assert!(!explicit_off.evolved_brains);
}

#[test]
fn one_means_true_and_nothing_else_is_accepted() {
    let with = LiveWorldConfig::from_initial_conditions(&arm_initial(true))
        .expect("the treatment conditions are valid");
    assert!(with.evolved_brains, "1.0 must request per-agent brains");

    // A boolean carried in an `f64` invites a value that is neither. Rounding 0.5 to *something*
    // would run an arm nobody declared and report it as the declared one.
    for bad in [0.5f64, 2.0, -1.0, 1.5, f64::MAX] {
        let mut values = arm_initial(false).values;
        values.push((LIVE_KEY_EVOLVED_BRAINS.to_string(), bad));
        let err = LiveWorldConfig::from_initial_conditions(&InitialConditionSet::new(values))
            .expect_err("a non-boolean value must be refused, not rounded");
        match err {
            ExperimentError::OutOfRange { field, value, .. } => {
                assert_eq!(field, LIVE_KEY_EVOLVED_BRAINS);
                assert_eq!(value, bad);
            }
            other => panic!("expected OutOfRange for {bad}, got {other}"),
        }
    }

    // Non-finite is refused before the range check, by the same rule every other key follows.
    for bad in [f64::NAN, f64::INFINITY] {
        let mut values = arm_initial(false).values;
        values.push((LIVE_KEY_EVOLVED_BRAINS.to_string(), bad));
        let err = LiveWorldConfig::from_initial_conditions(&InitialConditionSet::new(values))
            .expect_err("a non-finite value must be refused");
        assert!(
            matches!(err, ExperimentError::NotFinite { .. }),
            "expected NotFinite for {bad}, got {err}"
        );
    }
}

// ---- E2-G1: the treatment arm actually has brains ----------------------------------------------

#[test]
fn the_treatment_builds_founders_with_brains_and_the_control_builds_none() {
    let control = build_arm(false, SYNTHETIC_SEED);
    let treatment = build_arm(true, SYNTHETIC_SEED);

    let (c_agents, c_brains) = agents_and_brains(&control);
    let (t_agents, t_brains) = agents_and_brains(&treatment);

    assert_eq!(c_agents, 10, "the declared founding population");
    assert_eq!(t_agents, 10, "both arms found the same number of agents");
    assert_eq!(
        c_brains, 0,
        "the control must run on the shared BrainModel: a control that grew brains would make the \
         comparison empty"
    );
    assert_eq!(
        t_brains, 10,
        "every founder in the treatment carries its own brain — this is precondition P2, and a \
         treatment of brainless agents is the exact failure the preregistration exists to prevent"
    );
}

#[test]
fn the_committed_e2_manifests_build_the_arms_they_declare() {
    // The smoke manifests are the same construction as the experimental pair and carry the excluded
    // seed, so this checks the committed bytes without touching an experimental seed.
    let control = load_manifest("e2-smoke-control-shared-brain.json");
    let treatment = load_manifest("e2-smoke-treatment-evolved-brain.json");

    let (c_agents, c_brains) = agents_and_brains(&build_from(&control, SMOKE_SEED));
    let (t_agents, t_brains) = agents_and_brains(&build_from(&treatment, SMOKE_SEED));

    assert_eq!((c_agents, c_brains), (10, 0), "committed control arm");
    assert_eq!((t_agents, t_brains), (10, 10), "committed treatment arm");
}

// ---- E2-G3: the arms differ in the brain and in nothing stochastic ------------------------------

#[test]
fn founder_brains_leave_the_ecology_stream_exactly_where_the_control_left_it() {
    let control = build_arm(false, SYNTHETIC_SEED);
    let treatment = build_arm(true, SYNTHETIC_SEED);

    let (c_seed, c_pos, c_draws) = ecology_stream_state(&control);
    let (t_seed, t_pos, t_draws) = ecology_stream_state(&treatment);

    assert_eq!(
        c_seed, t_seed,
        "both arms seed the ecology stream identically"
    );
    assert_eq!(
        c_pos, t_pos,
        "the treatment must not consume ecology draws to build its brains. Drawing 10 × 5,769 f32 \
         out of SimRng would displace every later food position and seed drop, and the arms would \
         then differ in the brain AND in the realised random sequence — inseparably (design §4.3)"
    );
    assert_eq!(
        c_draws, t_draws,
        "the next draws the world would make must be identical, or 'same seed' silently means \
         'same seed and same number of prior draws'"
    );
}

#[test]
fn the_founding_population_differs_only_by_the_brain() {
    let control = build_arm(false, SYNTHETIC_SEED);
    let treatment = build_arm(true, SYNTHETIC_SEED);
    assert_eq!(
        founder_layout(&control),
        founder_layout(&treatment),
        "founder placement is a deterministic lattice and must be bit-identical across arms"
    );
}

#[test]
fn both_arms_observe_the_same_world_identity() {
    // Finding E2-F2: the manifest's declared `world_identity` is inert on this path, so "same
    // world" has to be *observed* rather than trusted. Gate E2-G6 records both and voids the run if
    // they differ; this is the mechanism that makes recording possible.
    let control = build_arm(false, SYNTHETIC_SEED);
    let treatment = build_arm(true, SYNTHETIC_SEED);
    let read = |a: &LiveExperimentAdapter| -> WorldIdentity {
        a.world()
            .get_resource::<WorldIdentity>()
            .copied()
            .expect("init_world always inserts the world's identity")
    };
    assert_eq!(
        read(&control),
        read(&treatment),
        "both arms must run on the same world, and the check is on the world that was built"
    );
}

// ---- Safety: the reproduction command may not launch the app -----------------------------------

#[test]
fn the_preregistered_reproduction_never_launches_the_app() {
    // `cargo run --example` is forbidden on this machine: the owner's standing rule bars starting
    // the desktop app or the full backend by any route, and `cargo run` is one. The correction is a
    // transport change only — build, then execute the compiled headless binary — and it is pinned
    // here so it cannot be quietly reverted into a command nobody is allowed to type.
    let raw = std::fs::read_to_string(prereg_path("e2-preregistration.json"))
        .expect("read e2-preregistration.json");
    let doc: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");
    let cmd = doc["reproduction_command"]
        .as_str()
        .expect("a reproduction command is registered");
    assert!(
        !cmd.contains("cargo run"),
        "the reproduction command must never use `cargo run`: {cmd}"
    );
    assert!(
        cmd.contains("cargo build"),
        "the reproduction command builds the example rather than running it through cargo: {cmd}"
    );
    assert!(
        cmd.contains("--example run_e2_brain_experiment"),
        "the reproduction command must name the headless example it builds: {cmd}"
    );
}
