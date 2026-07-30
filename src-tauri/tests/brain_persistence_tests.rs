//! **EB-S07** and **EB-S10** — an agent's brain survives save/restore and migration, and a save
//! written before brains existed still loads.
//!
//! Invariant D01 of the creature-development contract says restore and migration are **not**
//! development: they reconstitute an individual that already exists. Handing such an agent a freshly
//! rolled brain would produce a different creature wearing the same lineage id — and it would look
//! entirely healthy, which is why this is worth a gate rather than a code review.
//!
//! Invariant D02 says the phenotype travels with the individual. ADR-0003 decision 8 extends that to
//! cognitive state: a creature that forgets everything on crossing a shard boundary is not the same
//! creature.

use anima_engine_lib::core::components::{AgentBrain, AgentMigrationData};
use anima_engine_lib::core::ecs::AgentClass;
use anima_engine_lib::core::resources::{BrainPolicy, SimRng};
use anima_engine_lib::core::simulation_state::{spawn_serialized_agent, SerializedAgent};
use anima_engine_lib::evolution::brain_genotype::{ArchSpec, BrainGenotype, EVOLVED_ARCH};
use anima_engine_lib::evolution::genotype::{MorphologyGenotype, MorphologyNode};
use bevy_ecs::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;

fn brain(seed: u64) -> AgentBrain {
    let mut rng = StdRng::seed_from_u64(seed);
    AgentBrain::from_genotype(BrainGenotype::random(EVOLVED_ARCH, &mut rng).unwrap())
}

fn morphology() -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.5,
        mass: 1.0,
    });
    g
}

fn serialized_agent(brain: Option<AgentBrain>) -> SerializedAgent {
    SerializedAgent {
        genotype: morphology(),
        class: AgentClass::Prey,
        lineage_id: "lin-1".to_string(),
        generation: 3,
        parent_ids: vec![],
        evaluation: anima_engine_lib::core::agent_systems::AgentEvaluation {
            start_position: glam::Vec3::ZERO,
            total_distance: 1.0,
            total_energy_expended: 2.0,
            survival_ticks: 10,
            last_position: glam::Vec3::ZERO,
        },
        feature_tracker: Default::default(),
        root_position: glam::Vec3::ZERO,
        root_rotation: glam::Quat::IDENTITY,
        root_velocity: glam::Vec3::ZERO,
        homeostatic_state: anima_engine_lib::ai::hrrl::HomeostaticState {
            energy: 50.0,
            energy_target: 100.0,
            hydration: 50.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        last_transition_state: anima_engine_lib::ai::hrrl::LastTransitionState {
            state: [0.0; 15],
            action: [0.0; 4],
            has_last: false,
        },
        cognitive_state: Default::default(),
        inertia: Default::default(),
        action_gates: None,
        segments: vec![],
        brain,
    }
}

fn migration_payload(brain: Option<AgentBrain>) -> AgentMigrationData {
    AgentMigrationData {
        genotype: morphology(),
        homeostatic_state: anima_engine_lib::ai::hrrl::HomeostaticState {
            energy: 40.0,
            energy_target: 100.0,
            hydration: 40.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        position: glam::Vec3::new(1.0, 0.0, 2.0),
        velocity: glam::Vec3::ZERO,
        lineage_id: "lin-2".to_string(),
        generation: 5,
        agent_class: AgentClass::Prey,
        parent_ids: vec![],
        evaluation: None,
        feature_tracker: None,
        last_transition_state: None,
        source_port: 0,
        brain,
    }
}

fn spawn_migrated(data: AgentMigrationData) -> (World, Entity) {
    use bevy_ecs::system::Command;
    let mut world = World::new();
    anima_engine_lib::core::world_systems::SpawnMigrationCommand { data }.apply(&mut world);
    let entity = world
        .query_filtered::<Entity, With<anima_engine_lib::core::ecs::Agent>>()
        .iter(&world)
        .next()
        .expect("migration must spawn an agent");
    (world, entity)
}

// --- save / restore -----------------------------------------------------------------------------

#[test]
fn a_restored_agent_keeps_the_brain_it_was_saved_with() {
    let original = brain(11);
    let payload = serialized_agent(Some(original.clone()));

    let mut world = World::new();
    spawn_serialized_agent(&mut world, &payload).expect("valid saved agent");

    let entity = world
        .query_filtered::<Entity, With<anima_engine_lib::core::ecs::Agent>>()
        .iter(&world)
        .next()
        .unwrap();
    assert_eq!(world.entity(entity).get::<AgentBrain>(), Some(&original));
}

#[test]
fn restore_carries_the_brain_rather_than_rolling_a_new_one() {
    // The failure this catches looks like success: an agent that was handed a fresh brain still has
    // a brain, still behaves plausibly, and still reports the right lineage id. Only comparing two
    // independent restores of the *same* payload shows whether anything was regenerated.
    let payload = serialized_agent(Some(brain(21)));

    let read_back = |world: &mut World| {
        spawn_serialized_agent(world, &payload).expect("valid saved agent");
        let e = world
            .query_filtered::<Entity, With<anima_engine_lib::core::ecs::Agent>>()
            .iter(world)
            .next()
            .unwrap();
        world.entity(e).get::<AgentBrain>().cloned().unwrap()
    };

    let a = read_back(&mut World::new());
    let b = read_back(&mut World::new());
    assert_eq!(a, b);
    assert_eq!(a.genotype.weights, payload.brain.unwrap().genotype.weights);
}

#[test]
fn a_legacy_agent_restores_without_a_brain() {
    // `None` must stay `None`: restoring must not quietly upgrade an agent onto the evolved path.
    let mut world = World::new();
    spawn_serialized_agent(&mut world, &serialized_agent(None)).expect("valid legacy agent");

    let entity = world
        .query_filtered::<Entity, With<anima_engine_lib::core::ecs::Agent>>()
        .iter(&world)
        .next()
        .unwrap();
    assert!(world.entity(entity).get::<AgentBrain>().is_none());
}

#[test]
fn an_unreadable_brain_refuses_the_individual_instead_of_changing_its_identity() {
    // Learned weights whose length no longer matches the architecture mean the save came from a
    // build with a different layout. Loading them anyway would produce finite, meaningless output.
    let mut corrupt = brain(31);
    // A learned network whose architecture no longer matches its genome: the save came from a build
    // with a different layout, and running it would produce finite, meaningless output.
    corrupt.learned = Some(std::sync::Arc::new(
        BrainGenotype::random(ArchSpec::new(3, 4, 2), &mut StdRng::seed_from_u64(1)).unwrap(),
    ));
    assert!(corrupt.validate().is_err());

    let mut world = World::new();
    let error = spawn_serialized_agent(&mut world, &serialized_agent(Some(corrupt)))
        .expect_err("corrupt identity must be refused");
    assert!(error.to_string().contains("brain is unreadable"));

    assert_eq!(
        world
            .query_filtered::<Entity, With<anima_engine_lib::core::ecs::Agent>>()
            .iter(&world)
            .count(),
        0,
        "dropping a corrupt evolved brain to the shared model would create a different individual"
    );
}

#[test]
fn an_empty_saved_body_is_refused_before_decode_can_panic() {
    let mut payload = serialized_agent(None);
    payload.genotype = MorphologyGenotype::new();
    let mut world = World::new();

    let error =
        spawn_serialized_agent(&mut world, &payload).expect_err("empty body must be refused");
    assert!(error.to_string().contains("root node"));

    assert_eq!(
        world
            .query_filtered::<Entity, With<anima_engine_lib::core::ecs::Agent>>()
            .iter(&world)
            .count(),
        0
    );
}

#[test]
fn a_save_written_before_brains_existed_still_loads() {
    // EB-S10 / invariant D09. The serialized form is checked directly rather than through a fixture
    // file so this keeps working when unrelated fields are added.
    let with_brain = serde_json::to_value(serialized_agent(Some(brain(41)))).unwrap();
    let mut legacy = with_brain.clone();
    legacy.as_object_mut().unwrap().remove("brain");
    assert!(
        legacy.get("brain").is_none(),
        "the field must really be gone"
    );

    let decoded: SerializedAgent = serde_json::from_value(legacy).unwrap();
    assert!(
        decoded.brain.is_none(),
        "a missing brain field must default to the legacy shared model"
    );
}

#[test]
fn learned_weights_survive_a_round_trip() {
    // Lifetime learning is off today, but the slot has to work before anything writes to it —
    // otherwise the first run with learning enabled silently loses it at the first save.
    let mut learned_brain = brain(51);
    let mut learned = (*learned_brain.genotype).clone();
    for (i, w) in learned.weights.iter_mut().enumerate() {
        *w = i as f32 * 1e-4;
    }
    learned_brain.set_learned(learned.clone());
    learned_brain.validate().unwrap();

    let json = serde_json::to_string(&serialized_agent(Some(learned_brain))).unwrap();
    let back: SerializedAgent = serde_json::from_str(&json).unwrap();
    let brain = back.brain.unwrap();

    assert_eq!(brain.learned.as_deref(), Some(&learned));
    assert_eq!(
        brain.live_weights(),
        learned.weights.as_slice(),
        "the learned network, not the genome, is what inference should use"
    );
}

#[test]
fn learning_never_writes_back_into_the_genome() {
    // The Baldwin effect evolves the *capacity* to learn; inheriting what was learned would be
    // Lamarckian (ADR-0003 decision 2). The type keeps them separate — this pins that they stay so.
    let mut b = brain(61);
    let genome_before = b.genotype.weights.clone();

    let mut learned = (*b.genotype).clone();
    learned.weights.iter_mut().for_each(|w| *w = 9.0);
    b.set_learned(learned);

    assert_eq!(b.genotype.weights, genome_before);
    assert_ne!(b.live_weights(), genome_before.as_slice());
    assert_eq!(b.live_weights(), vec![9.0; EVOLVED_ARCH.param_count()]);
}

// --- migration ----------------------------------------------------------------------------------

#[test]
fn a_migrating_agent_arrives_with_the_brain_it_left_with() {
    let original = brain(71);
    let (world, entity) = spawn_migrated(migration_payload(Some(original.clone())));
    assert_eq!(world.entity(entity).get::<AgentBrain>(), Some(&original));
}

#[test]
fn a_legacy_agent_migrates_without_gaining_a_brain() {
    let (world, entity) = spawn_migrated(migration_payload(None));
    assert!(world.entity(entity).get::<AgentBrain>().is_none());
}

#[test]
fn a_migration_payload_round_trips_over_the_wire() {
    let payload = migration_payload(Some(brain(81)));
    let bytes = serde_json::to_vec(&payload).unwrap();
    let back: AgentMigrationData = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(back.brain, payload.brain);
}

#[test]
fn a_migration_payload_without_a_brain_field_still_decodes() {
    let mut value = serde_json::to_value(migration_payload(Some(brain(91)))).unwrap();
    value.as_object_mut().unwrap().remove("brain");
    let back: AgentMigrationData = serde_json::from_value(value).unwrap();
    assert!(back.brain.is_none());
}

// --- policy -------------------------------------------------------------------------------------

#[test]
fn evolved_brains_are_off_by_default() {
    let policy = BrainPolicy::default();
    assert!(!policy.evolved);
    assert_eq!(policy.arch, EVOLVED_ARCH);
    assert!(
        policy.new_brain(&mut StdRng::seed_from_u64(1)).is_none(),
        "the default must be the ADR-0003 baseline, not the new behaviour"
    );
}

#[test]
fn an_enabled_policy_mints_reproducible_brains() {
    let policy = BrainPolicy {
        evolved: true,
        arch: EVOLVED_ARCH,
        ..Default::default()
    };
    let mint = |seed: u64| policy.new_brain(SimRng::from_seed(seed).rng()).unwrap();

    assert_eq!(
        mint(5),
        mint(5),
        "the same run seed must found the same population"
    );
    assert_ne!(mint(5), mint(6));
    mint(5).validate().unwrap();
}

#[test]
fn the_evolved_architecture_carries_the_action_gates() {
    use anima_engine_lib::evolution::brain_genotype::action_index;

    // The widened action space is what makes an evolved brain worth having: without the gate
    // outputs, per-agent weights could still only vary an agent's gait.
    assert_eq!(EVOLVED_ARCH.outputs, action_index::COUNT);
    assert_eq!(EVOLVED_ARCH.outputs, ArchSpec::LEGACY.outputs + 4);

    // Every gate must sit past the CPG block, inside the output vector, and on its own slot. The
    // distinctness check is the one that matters: an off-by-one that aliased "eat" onto "attack"
    // would still produce a running, plausible-looking simulation.
    let gates = [
        action_index::PHEROMONE_EMIT,
        action_index::ATTACK_INTENT,
        action_index::FEED_INTENT,
        action_index::SIGNAL,
    ];
    assert!(gates
        .iter()
        .all(|i| (action_index::CPG_LEN..action_index::COUNT).contains(i)));

    let mut seen = gates.to_vec();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), gates.len(), "two gates share an output slot");
}
