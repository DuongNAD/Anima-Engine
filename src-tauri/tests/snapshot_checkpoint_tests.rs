//! G1.2 gate: a snapshot is a checkpoint, not a picture.
//!
//! The gate is `checksum(run N) == checksum(run K → save → load → run N−K)`. It passes only if the
//! snapshot carries every piece of state that decides where the world goes next — including the one
//! that is easiest to forget, the RNG's draw position. Restoring a seed alone restarts the stream,
//! so a resumed run diverges on its very next random draw and this test fails.
//!
//! The save goes through the real path: `serialize_world_state` → `SnapshotEnvelope::seal` →
//! `write_atomic` → a file on disk → `snapshot::read` (checksum verified, schema migrated). The
//! restore goes through `spawn_serialized_agent` and `restore_energy_state`. What is *not* covered
//! is `SimulationEngine::start`'s own wiring of those pieces, which lives inside a 1600-line
//! function; G2 is where that becomes testable.
//!
//! The schedule here is explicitly `.chain()`ed and single-threaded. Bevy's multi-threaded executor
//! picks system order per run, so an uninterrupted run would not even match *itself* — declaring
//! the order is G1.3's job, and until it lands this test declares its own.

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::core::ecs::{
    init_world, EpochManager, Food, Lake, MapBounds, Position, Predator, Prey, Tree,
};
use anima_engine_lib::core::resources::{
    EnvironmentalSpawnSettings, EvolutionQueue, EvolutionReceiver, FoodSpawnSettings,
};
use anima_engine_lib::core::simulation_state::{
    restore_energy_state, serialize_world_state, spawn_serialized_agent, SavedSimulationState,
};
use anima_engine_lib::core::snapshot::{self, world_checksum, SnapshotEnvelope};
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use anima_engine_lib::physics::SpatialCollider;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{ExecutorKind, Schedule};
use std::sync::{Arc, Mutex, RwLock};

type EvolutionMsg = (
    Entity,
    MorphologyGenotype,
    glam::Vec3,
    String,
    u32,
    Vec<String>,
);

fn test_genotype() -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.3,
        mass: 1.0,
    });
    g.add_node(MorphologyNode {
        id: 1,
        length: 1.0,
        radius: 0.3,
        mass: 1.0,
    });
    g.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        joint_anchor: glam::Vec3::new(1.0, 0.0, 0.0),
        joint_axis: glam::Vec3::new(0.0, 0.0, 1.0),
    });
    g
}

/// A world with the engine's resources but no population. Both the genesis path and the restore
/// path start here, so any difference between them is the snapshot's doing rather than the setup's.
fn bare_world() -> (World, crossbeam_channel::Sender<EvolutionMsg>) {
    let mut world = init_world();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(FoodSpawnSettings::default());
    world.insert_resource(EnvironmentalSpawnSettings::default());
    world.insert_resource(EpochManager::default());
    let (tx, rx) = crossbeam_channel::unbounded::<EvolutionMsg>();
    world.insert_resource(EvolutionReceiver(rx));
    world.insert_resource(EvolutionQueue {
        pending_replacements: Vec::new(),
    });
    (world, tx)
}

fn with_genesis() -> (World, crossbeam_channel::Sender<EvolutionMsg>) {
    let (mut world, tx) = bare_world();
    let genotype = test_genotype();
    let bounds = *world.resource::<MapBounds>();
    for i in 0..10 {
        let pos = glam::Vec3::new(
            bounds.min.x + (i as f32 + 1.0) * 3.0,
            0.0,
            bounds.min.z + 10.0,
        );
        let e = decode_genotype(&mut world, &genotype, pos, glam::Quat::IDENTITY);
        // `serialize_world_state`'s agent query requires the full identity bundle. Without it an
        // agent is invisible to the save — which is how the first run of this gate managed to
        // "successfully" serialize zero agents.
        world.entity_mut(e).insert((
            anima_engine_lib::core::agent_systems::AgentGenotype(genotype.clone()),
            anima_engine_lib::core::agent_systems::AgentEvaluation {
                start_position: pos,
                total_distance: 0.0,
                total_energy_expended: 0.0,
                survival_ticks: 0,
                last_position: pos,
            },
            anima_engine_lib::core::components::FeatureTracker::default(),
            anima_engine_lib::core::agent_systems::AgentLineageId(format!("founder-{i}")),
            anima_engine_lib::core::agent_systems::AgentGeneration(0),
            anima_engine_lib::core::components::AgentParentLineageIds(Vec::new()),
        ));
        if i < 7 {
            world.entity_mut(e).insert(Prey);
        } else {
            world.entity_mut(e).insert(Predator);
        }
    }
    for i in 0..8 {
        world.spawn((
            Tree {
                current_fruit: 5.0,
                max_fruit: 50.0,
                fruit_growth_rate: 2.0,
                time_since_last_drop: 0.0,
                seed_drop_cooldown: 1.0e9,
                seed_spread_radius: 5.0,
            },
            Position(glam::Vec3::new(
                bounds.min.x + (i as f32 + 1.0) * 3.0,
                0.0,
                bounds.min.z + 10.0,
            )),
            SpatialCollider { radius: 4.0 },
        ));
    }
    (world, tx)
}

/// Explicitly ordered and single-threaded, for the reason in the module docs.
fn deterministic_schedule() -> Schedule {
    use anima_engine_lib::core::agent_systems::apply_staggered_evolution_system;
    use anima_engine_lib::core::environmental_systems::{
        detect_environmental_collisions_system, ecosystem_census_system, fruit_growth_system,
        herbivore_grazing_system, resource_field_regrowth_system,
    };
    use anima_engine_lib::core::world_systems::{
        combat_system, detect_food_collisions_system, metabolic_decay_system, spawn_food_system,
    };

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(ExecutorKind::SingleThreaded);
    schedule.add_systems(
        (
            metabolic_decay_system,
            spawn_food_system,
            detect_food_collisions_system,
            combat_system,
            fruit_growth_system,
            detect_environmental_collisions_system,
            apply_staggered_evolution_system,
            herbivore_grazing_system,
            resource_field_regrowth_system,
            ecosystem_census_system,
        )
            .chain(),
    );
    schedule
}

/// The four handles `serialize_world_state` needs. The lineage tracker falls back to in-memory
/// when Neo4j is absent, which is the case here.
struct SaveDeps {
    chronicle: Arc<RwLock<Vec<anima_engine_lib::core::simulation_state::ChronicleEvent>>>,
    lineage: Arc<anima_engine_lib::evolution::lineage::FallbackLineageTracker>,
    evolution: Arc<Mutex<anima_engine_lib::commands::EvolutionSettings>>,
    grid: Arc<Mutex<anima_engine_lib::commands::MapElitesGridState>>,
}

impl SaveDeps {
    fn new() -> Self {
        Self {
            chronicle: Arc::new(RwLock::new(Vec::new())),
            lineage: Arc::new(
                anima_engine_lib::evolution::lineage::FallbackLineageTracker::new(
                    "bolt://127.0.0.1:1",
                    "neo4j",
                    "password",
                ),
            ),
            evolution: Arc::new(Mutex::new(anima_engine_lib::commands::EvolutionSettings {
                mutation_rate: 0.2,
                selection_bias: 1.2,
                grid_resolution: 30,
            })),
            grid: Arc::new(Mutex::new(anima_engine_lib::commands::MapElitesGridState {
                grid: std::collections::HashMap::new(),
                grid_resolution: 30,
            })),
        }
    }
}

fn save(world: &mut World, tick: u64, deps: &SaveDeps) -> SavedSimulationState {
    serialize_world_state(
        world,
        tick,
        &deps.chronicle,
        &deps.lineage,
        &deps.evolution,
        &deps.grid,
    )
}

/// Rebuild a world from a saved state, the way the engine's load path does.
fn restore(state: &SavedSimulationState) -> (World, crossbeam_channel::Sender<EvolutionMsg>) {
    let (mut world, tx) = bare_world();
    for agent in &state.agents {
        spawn_serialized_agent(&mut world, agent);
    }
    for food in &state.foods {
        world.spawn((
            Food {
                energy_value: food.energy_value,
                hydration_value: food.hydration_value,
            },
            Position(food.position),
            SpatialCollider { radius: 0.5 },
        ));
    }
    for tree in &state.trees {
        world.spawn((
            Tree {
                current_fruit: tree.current_fruit,
                max_fruit: tree.max_fruit,
                fruit_growth_rate: tree.fruit_growth_rate,
                time_since_last_drop: tree.time_since_last_drop,
                seed_drop_cooldown: tree.seed_drop_cooldown,
                seed_spread_radius: tree.seed_spread_radius,
            },
            Position(tree.position),
            SpatialCollider {
                radius: tree.radius,
            },
        ));
    }
    for lake in &state.lakes {
        world.spawn((
            Lake {
                current_water: lake.current_water,
                max_water: lake.max_water,
                replenishment_rate: lake.replenishment_rate,
            },
            Position(lake.position),
            SpatialCollider {
                radius: lake.radius,
            },
        ));
    }
    restore_energy_state(&mut world, state);
    (world, tx)
}

/// Round-trip a state through the real on-disk format, so the gate covers the envelope, the
/// checksum, the atomic write and the migration path rather than just an in-memory struct copy.
fn through_disk(state: SavedSimulationState, label: &str) -> SavedSimulationState {
    let dir = std::env::temp_dir().join(format!("anima_g12_{}_{}", std::process::id(), label));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("checkpoint.json");
    let envelope = SnapshotEnvelope::seal(state).expect("seal");
    snapshot::write_atomic(&path, &envelope).expect("write");
    let back = snapshot::read(&path).expect("read");
    assert_eq!(
        back.loaded_from_schema,
        snapshot::SCHEMA_VERSION,
        "a snapshot this build wrote must read back at the current schema"
    );
    let _ = std::fs::remove_dir_all(&dir);
    back
}

const N: u64 = 4_000;
const K: u64 = 1_500;

#[test]
fn resuming_from_a_snapshot_is_indistinguishable_from_never_stopping() {
    // Reference: N ticks, uninterrupted.
    let (mut reference, _tx_a) = with_genesis();
    let mut sched_a = deterministic_schedule();
    for _ in 0..N {
        sched_a.run(&mut reference);
    }
    let reference_checksum = world_checksum(&mut reference);

    // Interrupted: K ticks, save to disk, load into a fresh world, N−K more.
    let (mut interrupted, _tx_b) = with_genesis();
    let mut sched_b = deterministic_schedule();
    for _ in 0..K {
        sched_b.run(&mut interrupted);
    }
    let deps = SaveDeps::new();
    let state = save(&mut interrupted, K, &deps);
    let state = through_disk(state, "resume");

    let (mut resumed, _tx_c) = restore(&state);
    let mut sched_c = deterministic_schedule();
    for _ in 0..(N - K) {
        sched_c.run(&mut resumed);
    }
    let resumed_checksum = world_checksum(&mut resumed);

    println!("N={N} K={K} reference={reference_checksum:#010x} resumed={resumed_checksum:#010x}");
    assert_eq!(
        reference_checksum, resumed_checksum,
        "a run resumed from a checkpoint diverged from an uninterrupted one. Some piece of \
         trajectory-relevant state is missing from SavedSimulationState."
    );
}

/// The control for the test above. If the RNG stream position is dropped — the single easiest
/// thing for a snapshot to forget — the resumed run MUST diverge. Without this, a gate that passes
/// proves nothing: it could be passing because the world is insensitive to the RNG.
#[test]
fn dropping_the_rng_stream_position_does_diverge() {
    let (mut reference, _tx_a) = with_genesis();
    let mut sched_a = deterministic_schedule();
    for _ in 0..N {
        sched_a.run(&mut reference);
    }
    let reference_checksum = world_checksum(&mut reference);

    let (mut interrupted, _tx_b) = with_genesis();
    let mut sched_b = deterministic_schedule();
    for _ in 0..K {
        sched_b.run(&mut interrupted);
    }
    let deps = SaveDeps::new();
    let mut state = save(&mut interrupted, K, &deps);

    // Exactly the pre-G1.2 behaviour: keep the seed, forget how far in we were.
    state.sim_rng_pos = 0;

    let (mut resumed, _tx_c) = restore(&state);
    let mut sched_c = deterministic_schedule();
    for _ in 0..(N - K) {
        sched_c.run(&mut resumed);
    }
    let resumed_checksum = world_checksum(&mut resumed);

    assert_ne!(
        reference_checksum, resumed_checksum,
        "restarting the RNG stream produced an identical world, so this gate is not actually \
         sensitive to the RNG and proves less than it appears to"
    );
}

/// A snapshot must survive the disk round trip byte-for-byte in the fields that matter, not merely
/// produce a world that behaves similarly.
#[test]
fn the_on_disk_round_trip_preserves_the_checkpoint_fields() {
    let (mut world, _tx) = with_genesis();
    let mut sched = deterministic_schedule();
    for _ in 0..250 {
        sched.run(&mut world);
    }
    let deps = SaveDeps::new();
    let before = save(&mut world, 250, &deps);
    let after = through_disk(before.clone(), "fields");

    assert_eq!(before.sim_rng_seed, after.sim_rng_seed);
    assert_eq!(before.sim_rng_pos, after.sim_rng_pos);
    assert_ne!(before.sim_rng_pos, 0, "the run must actually have drawn");
    assert_eq!(before.season_phase.to_bits(), after.season_phase.to_bits());
    assert_eq!(before.season_rate.to_bits(), after.season_rate.to_bits());
    assert_eq!(before.eco_detritus.to_bits(), after.eco_detritus.to_bits());
    assert_eq!(before.eco_plants.to_bits(), after.eco_plants.to_bits());
    assert_eq!(before.eco_animals.to_bits(), after.eco_animals.to_bits());
    assert_eq!(before.resource_field_r, after.resource_field_r);
    assert_eq!(before.energy_baseline, after.energy_baseline);
    assert!(
        before.energy_baseline.is_some(),
        "the energy ledger must have locked a baseline by now"
    );
    assert_eq!(before.agents.len(), after.agents.len());
}

/// Restoring must not re-baseline the energy ledger. If it did, a leak that happened before the
/// save would be forgiven by the act of saving.
#[test]
fn restoring_keeps_the_original_energy_baseline() {
    let (mut world, _tx) = with_genesis();
    let mut sched = deterministic_schedule();
    for _ in 0..300 {
        sched.run(&mut world);
    }
    let deps = SaveDeps::new();
    let state = through_disk(save(&mut world, 300, &deps), "baseline");
    let original = state.energy_baseline.expect("baseline was locked");

    let (resumed, _tx2) = restore(&state);
    let restored = resumed
        .resource::<anima_engine_lib::core::energy_ledger::EnergyLedger>()
        .baseline()
        .expect("restore must carry the baseline forward");
    assert_eq!(
        original.to_bits(),
        restored.to_bits(),
        "the resumed run re-baselined instead of keeping genesis's baseline"
    );
}

/// Diagnostic: does restore reproduce the world *before* any further ticks? Separates "the
/// snapshot is lossy" from "the resumed trajectory diverges".
#[test]
fn restore_reproduces_the_world_with_zero_further_ticks() {
    let (mut world, _tx) = with_genesis();
    let mut sched = deterministic_schedule();
    for _ in 0..K {
        sched.run(&mut world);
    }
    let deps = SaveDeps::new();
    let state = through_disk(save(&mut world, K, &deps), "zero");
    let (mut resumed, _tx2) = restore(&state);

    let before = world_checksum(&mut world);
    let after = world_checksum(&mut resumed);
    if before != after {
        // Narrow it down for whoever reads the failure.
        let agents_a = world
            .query_filtered::<(), bevy_ecs::prelude::With<anima_engine_lib::core::ecs::Agent>>()
            .iter(&world)
            .count();
        let agents_b = resumed
            .query_filtered::<(), bevy_ecs::prelude::With<anima_engine_lib::core::ecs::Agent>>()
            .iter(&resumed)
            .count();
        let food_a = world.query::<&Food>().iter(&world).count();
        let food_b = resumed.query::<&Food>().iter(&resumed).count();
        let pool_a = *world.resource::<anima_engine_lib::core::ecology::EcosystemBiomass>();
        let pool_b = *resumed.resource::<anima_engine_lib::core::ecology::EcosystemBiomass>();
        let rng_a = world
            .resource::<anima_engine_lib::core::resources::SimRng>()
            .stream_pos();
        let rng_b = resumed
            .resource::<anima_engine_lib::core::resources::SimRng>()
            .stream_pos();
        panic!(
            "restore is lossy.\n  agents {agents_a} vs {agents_b}\n  food {food_a} vs {food_b}\n\
             pool {pool_a:?} vs {pool_b:?}\n  rng_pos {rng_a} vs {rng_b}"
        );
    }
}

/// A saved state survives a JSON round trip **exactly**, field for field.
///
/// This was a `#[ignore]`d diagnostic that documented itself as *expected to fail* on `eco_animals`,
/// with the explanation "serde_json's f64 round trip is not bit-exact". Half of that was right and
/// the wrong half mattered: serde_json **writes** floats with `ryu`, which is shortest-round-trip
/// and exact. It was the **reader** that was lossy — without the `float_roundtrip` feature,
/// serde_json's float parser is a fast approximation that may land 1 ULP away from the decimal it
/// was given. Enabling that feature (see `Cargo.toml`) makes the round trip exact and turns a
/// diagnostic that was allowed to fail into a gate that is not.
///
/// The envelope still hashes the raw bytes rather than a re-serialization, and still should: that
/// protects against map iteration order and formatting choices, which are a different failure from
/// this one.
#[test]
fn a_saved_state_round_trips_through_json_exactly() {
    let (mut world, _tx) = with_genesis();
    let mut sched = deterministic_schedule();
    for _ in 0..300 {
        sched.run(&mut world);
    }
    let deps = SaveDeps::new();
    let before = save(&mut world, 300, &deps);

    // A float that is awkward for an approximate parser, planted so the gate does not depend on the
    // run happening to produce one. This exact value is the one the old diagnostic named.
    let mut before = before;
    before.eco_animals = 990.5102615356445;

    let text = serde_json::to_string_pretty(&before).expect("serialize");
    let after: SavedSimulationState = serde_json::from_str(&text).expect("deserialize");

    let va = serde_json::to_value(&before).expect("value before");
    let vb = serde_json::to_value(&after).expect("value after");
    if va != vb {
        let (oa, ob) = (
            va.as_object().expect("object"),
            vb.as_object().expect("object"),
        );
        let mut report = String::new();
        for (k, v) in oa {
            if ob.get(k) != Some(v) {
                let sa = serde_json::to_string(v).expect("field before");
                let sb = serde_json::to_string(ob.get(k).expect("field present")).expect("field");
                report.push_str(&format!(
                    "\n  FIELD `{k}`\n    before: {}\n    after:  {}",
                    &sa[..sa.len().min(220)],
                    &sb[..sb.len().min(220)]
                ));
            }
        }
        panic!("a saved state did not survive a JSON round trip:{report}");
    }

    // And the bit pattern specifically, not just the serde_json `Value` comparison.
    assert_eq!(
        before.eco_animals.to_bits(),
        after.eco_animals.to_bits(),
        "an f64 changed bit pattern across a JSON round trip"
    );
    assert_eq!(before.sim_rng_pos, after.sim_rng_pos);
    assert_eq!(before.resource_field_phase, after.resource_field_phase);
}

/// The narrow property the test above rests on, isolated so a regression names the cause instead of
/// pointing at a 20-field struct: `f64 -> JSON -> f64` is the identity for every bit pattern.
#[test]
fn serde_json_round_trips_every_awkward_f64_bit_for_bit() {
    let awkward: [f64; 12] = [
        // The two values that actually turned up in this repository: the `eco_animals` the old
        // diagnostic named, and a capture mean that differed by 1 ULP across a file.
        990.5102615356445,
        184_889.583_333_333_37,
        0.1,
        1.0 / 3.0,
        f64::MIN_POSITIVE,
        f64::MAX,
        -f64::MAX,
        1e308,
        1e-308,
        123_456_789.123_456_79,
        // Built from bits rather than written as decimals: the extremes are exactly where an
        // approximate parser is most likely to land on the wrong side, and a decimal literal for
        // them is both unreadable and something clippy rightly objects to.
        f64::from_bits(1),                     // smallest positive subnormal
        f64::from_bits(0x000F_FFFF_FFFF_FFFF), // largest subnormal
    ];
    for v in awkward {
        let text = serde_json::to_string(&v).expect("serialize");
        let back: f64 = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(
            v.to_bits(),
            back.to_bits(),
            "{v:?} serialized to {text} and read back as {back:?}"
        );
    }
}
