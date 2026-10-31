//! Zero-allocation gate for the agent birth and death command paths (B2).
//!
//! Covers all three sites in `core::agent_systems`:
//!   - Site 1: `agent_mortality_system` (`src/core/agent_systems.rs:394`)
//!   - Site 2: `ReclaimAndDespawnAgentCommand::apply` (`src/core/agent_systems.rs:119`)
//!   - Site 3: `SpawnOffspringCommand::apply` (`src/core/agent_systems.rs:271`)
//!
//! Uses `ThreadTrackingAllocator` (thread-scoped) and a single `#[test]` entry point per
//! `tests/common/allocator.rs` so unrelated process threads cannot contaminate the window.

mod common;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ExecutorKind;
use bevy_ecs::system::Command;
use glam::{Quat, Vec3};

use std::sync::Arc;

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::{
    agent_mortality_system, autonomous_reproduction_system, AgentEvaluation, AgentGeneration,
    AgentGenotype, AgentLineageId, ReclaimAndDespawnAgentCommand, SpawnOffspringCommand,
};
use anima_engine_lib::core::ecology::EcosystemBiomass;
use anima_engine_lib::core::ecs::{
    Agent, AgentClass, AgentParentLineageIds, ChildrenLinks, FeatureTracker, ParentAgent, Prey,
    Segment, Velocity,
};
use anima_engine_lib::core::resources::{BrainPolicy, SimRng};
use anima_engine_lib::core::telemetry_export::DemographicTracker;
use anima_engine_lib::evolution::brain_genotype::BrainGenotype;
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use anima_engine_lib::physics::dynamics::RigidBody;

#[global_allocator]
static ALLOCATOR: common::allocator::ThreadTrackingAllocator =
    common::allocator::ThreadTrackingAllocator::new();

fn two_segment_genotype() -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.25,
        mass: 1.0,
    });
    g.add_node(MorphologyNode {
        id: 1,
        length: 0.8,
        radius: 0.20,
        mass: 0.5,
    });
    g.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        joint_anchor: Vec3::new(0.0, 0.0, 1.0),
        joint_axis: Vec3::X,
    });
    g
}

fn spawn_dying_two_segment_agent(
    world: &mut World,
    genotype: &MorphologyGenotype,
    lineage: &str,
    survival_ticks: u32,
) -> Entity {
    let root = decode_genotype(world, genotype, Vec3::ZERO, Quat::IDENTITY);
    world.entity_mut(root).insert((
        Agent,
        Prey,
        HomeostaticState {
            energy: 0.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        AgentGenotype(genotype.clone()),
        AgentEvaluation {
            start_position: Vec3::ZERO,
            total_distance: 0.0,
            total_energy_expended: 0.0,
            survival_ticks,
            last_position: Vec3::ZERO,
        },
        FeatureTracker::default(),
        AgentLineageId(lineage.to_string()),
        AgentGeneration(0),
        AgentParentLineageIds(Vec::new()),
    ));
    root
}

fn assert_agent_lineage_id_is_not_unique_across_live_agents() {
    let mut world = World::new();
    world.insert_resource(SimRng::from_seed(4242));
    let genotype = two_segment_genotype();
    let founder = spawn_dying_two_segment_agent(&mut world, &genotype, "founder-0", 10);
    world.get_mut::<HomeostaticState>(founder).unwrap().energy = 100.0;

    let mut repro_schedule = Schedule::default();
    repro_schedule.set_executor_kind(ExecutorKind::SingleThreaded);
    repro_schedule.add_systems((autonomous_reproduction_system, apply_deferred).chain());

    // First birth from `founder-0` (generation 0) -> spawns child with `"founder-0.1"`.
    repro_schedule.run(&mut world);
    // Refill parent reserve so it reproduces a second time -> spawns second child with `"founder-0.1"`.
    world.get_mut::<HomeostaticState>(founder).unwrap().energy = 100.0;
    repro_schedule.run(&mut world);

    let matching_live_children: Vec<Entity> = world
        .query_filtered::<(Entity, &AgentLineageId), With<Agent>>()
        .iter(&world)
        .filter_map(|(e, id)| (id.0 == "founder-0.1").then_some(e))
        .collect();
    assert_eq!(
        matching_live_children.len(),
        2,
        "two distinct live sibling agents produced by autonomous_reproduction_system share the identical AgentLineageId(\"founder-0.1\")"
    );
    assert_ne!(matching_live_children[0], matching_live_children[1]);
}

#[test]
fn birth_and_death_command_paths_allocate_nothing_beyond_decode_genotype() {
    assert_agent_lineage_id_is_not_unique_across_live_agents();

    let mut world = World::new();
    world.insert_resource(EcosystemBiomass::default());
    world.insert_resource(DemographicTracker::default());
    world.insert_resource(SimRng::from_seed(4242));

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(ExecutorKind::SingleThreaded);
    schedule.add_systems((agent_mortality_system, apply_deferred).chain());

    let genotype = two_segment_genotype();
    let inherited_brain: Arc<BrainGenotype> = {
        let mut rng = world.resource_mut::<SimRng>();
        Arc::new(
            BrainGenotype::random(BrainPolicy::default().arch, rng.rng())
                .expect("valid default brain arch"),
        )
    };

    // Warm-up: exercise both `SpawnOffspringCommand::apply` and the death schedule with more
    // concurrent entities than any measured window will hold, and run `schedule.run(&mut world)`
    // AFTER every archetype (including `SpawnOffspringCommand`'s full component bundle) has been
    // created so Bevy's archetype tables, `BlobVec` columns, query states, entity free-list and
    // deferred command buffer do not grow inside the measured windows.
    let mut warm_entities = Vec::new();
    for i in 0..8 {
        let e = spawn_dying_two_segment_agent(&mut world, &genotype, &format!("warm-{i}"), i * 100);
        if i >= 4 {
            world.get_mut::<HomeostaticState>(e).unwrap().energy = 100.0;
        }
        warm_entities.push(e);
    }
    for i in 0..4 {
        SpawnOffspringCommand {
            parent_entity: warm_entities[4 + i],
            genotype: genotype.clone(),
            initial_pos: Vec3::new(i as f32 * 2.0, 0.0, 2.0),
            initial_rot: Quat::IDENTITY,
            agent_class: AgentClass::Prey,
            lineage_id: format!("warm-child-{i}"),
            generation: 1,
            parent_ids: vec![format!("warm-{}", 4 + i)],
            starting_energy: 50.0,
            energy_target: 100.0,
            brain: Some(Arc::clone(&inherited_brain)),
        }
        .apply(&mut world);
    }
    // Run the mortality schedule after all archetypes exist so its internal `QueryState` bitsets
    // cover every archetype table in `world`.
    schedule.run(&mut world);

    // Clean up remaining warm-up agents via ReclaimAndDespawnAgentCommand, then run `schedule` one
    // more time on the warmed world.
    let warm_roots: Vec<Entity> = world
        .query_filtered::<Entity, With<Agent>>()
        .iter(&world)
        .collect();
    for root in warm_roots {
        ReclaimAndDespawnAgentCommand { root }.apply(&mut world);
    }
    schedule.run(&mut world);
    // `EntityWorldMut::despawn` pushes to `world.removed_components` (`Events<RemovedComponentEntity>`),
    // which in standalone `bevy_ecs` is cleared by `world.clear_trackers()`. Clear after warm-up
    // (after warming > 8 despawns into the event buffers) and swap back so the active event buffer
    // has pre-warmed capacity for the measured windows.
    world.clear_trackers();
    world.clear_trackers();
    assert_eq!(world.query::<&Agent>().iter(&world).count(), 0);
    assert_eq!(world.query::<&Segment>().iter(&world).count(), 0);

    // Measure `decode_genotype`'s own intrinsic allocations on the warmed world (its internal
    // `HashSet`/`VecDeque` and the child segment's `ChildrenLinks(Vec<Entity>)` push), then clean up.
    ALLOCATOR.start_tracking();
    let decode_probe = decode_genotype(&mut world, &genotype, Vec3::ZERO, Quat::IDENTITY);
    let decode_baseline_allocs = ALLOCATOR.stop_tracking();
    ReclaimAndDespawnAgentCommand { root: decode_probe }.apply(&mut world);
    assert_eq!(world.query::<&Segment>().iter(&world).count(), 0);

    // ---- Window 1: Death path (Site 1 + Site 2), forcing TWO multi-segment deaths ---------------
    let _d1 = spawn_dying_two_segment_agent(&mut world, &genotype, "dying-z", 15);
    let _d2 = spawn_dying_two_segment_agent(&mut world, &genotype, "dying-a", 350);
    assert_eq!(world.query::<&Agent>().iter(&world).count(), 2);
    assert_eq!(world.query::<&Segment>().iter(&world).count(), 4);

    let deaths_before = world.resource::<DemographicTracker>().total_deaths();

    ALLOCATOR.start_tracking();
    schedule.run(&mut world);
    let death_allocs = ALLOCATOR.stop_tracking();

    let deaths_after = world.resource::<DemographicTracker>().total_deaths();
    assert_eq!(
        deaths_after - deaths_before,
        2,
        "window 1 must force at least two deaths"
    );
    assert_eq!(
        world.query::<&Agent>().iter(&world).count(),
        0,
        "both dying root agents must be despawned"
    );
    assert_eq!(
        world.query::<&Segment>().iter(&world).count(),
        0,
        "both dying agents' child segments must be despawned by ReclaimAndDespawnAgentCommand"
    );

    // ---- Window 2: Combined Birth + Death window (Sites 1, 2, and 3 in one tick) ----------------
    // Forces TWO deaths (`agent_mortality_system` -> `ReclaimAndDespawnAgentCommand::apply`) AND
    // ONE multi-segment birth (`SpawnOffspringCommand::apply`) inside the same measured window.
    let surviving_parent = spawn_dying_two_segment_agent(&mut world, &genotype, "parent-0", 20);
    world
        .get_mut::<HomeostaticState>(surviving_parent)
        .unwrap()
        .energy = 100.0;
    let _dying_1 = spawn_dying_two_segment_agent(&mut world, &genotype, " doomed-b", 25);
    let _dying_2 = spawn_dying_two_segment_agent(&mut world, &genotype, " doomed-a", 400);

    // Pre-construct the birth command's owned payload outside the window so the measurement scopes
    // the tick's mortality system + deferred command application (`ReclaimAndDespawnAgentCommand`
    // x2 and `SpawnOffspringCommand` x1).
    let offspring_cmd = SpawnOffspringCommand {
        parent_entity: surviving_parent,
        genotype: genotype.clone(),
        initial_pos: Vec3::new(5.0, 0.0, -3.0),
        initial_rot: Quat::IDENTITY,
        agent_class: AgentClass::Prey,
        lineage_id: "parent-0.1".to_string(),
        generation: 1,
        parent_ids: vec!["parent-0".to_string()],
        starting_energy: 40.0,
        energy_target: 100.0,
        brain: Some(Arc::clone(&inherited_brain)),
    };

    let deaths_before_combined = world.resource::<DemographicTracker>().total_deaths();

    ALLOCATOR.start_tracking();
    schedule.run(&mut world);
    offspring_cmd.apply(&mut world);
    let combined_window_allocs = ALLOCATOR.stop_tracking();

    let deaths_after_combined = world.resource::<DemographicTracker>().total_deaths();
    assert_eq!(
        deaths_after_combined - deaths_before_combined,
        2,
        "combined window must force two deaths"
    );

    // Verify 2 agents remain (surviving_parent + newborn offspring) and 4 segments (2 each).
    assert_eq!(
        world.query::<&Agent>().iter(&world).count(),
        2,
        "two died and one was born, leaving surviving_parent + offspring"
    );
    assert_eq!(
        world.query::<&Segment>().iter(&world).count(),
        4,
        "surviving_parent (2 segments) + newborn offspring (2 segments) = 4 segments"
    );

    // Verify `SpawnOffspringCommand::apply` synchronised the newborn child segment's Velocity and
    // RigidBody.velocity to the newborn root's non-zero initial velocity.
    let offspring_root = world
        .query::<(Entity, &AgentLineageId)>()
        .iter(&world)
        .find_map(|(e, id)| (id.0 == "parent-0.1").then_some(e))
        .expect("offspring entity must exist");
    let root_vel = world.get::<Velocity>(offspring_root).unwrap().0;
    assert!(
        root_vel.length_squared() > 0.1,
        "offspring root velocity must be non-zero, got {root_vel:?}"
    );
    let children = &world.get::<ChildrenLinks>(offspring_root).unwrap().0;
    assert_eq!(children.len(), 1, "offspring must have 1 child segment");
    let child_seg = children[0];
    assert_eq!(
        world.get::<ParentAgent>(child_seg).unwrap().0,
        offspring_root
    );
    assert_eq!(
        world.get::<Velocity>(child_seg).unwrap().0,
        root_vel,
        "offspring child segment Velocity must be synchronised to root Velocity"
    );
    assert_eq!(
        world.get::<RigidBody>(child_seg).unwrap().velocity,
        root_vel,
        "offspring child segment RigidBody.velocity must be synchronised to root Velocity"
    );

    let command_overhead_allocs = combined_window_allocs.saturating_sub(decode_baseline_allocs);
    assert_eq!(
        (death_allocs, command_overhead_allocs),
        (0, 0),
        "birth/death zero-alloc gate failed: death_window_allocs={death_allocs} (2 multi-segment deaths), combined_window_excess_allocs={command_overhead_allocs} (2 deaths + 1 birth, total={combined_window_allocs}, decode_genotype_baseline={decode_baseline_allocs})"
    );
}
