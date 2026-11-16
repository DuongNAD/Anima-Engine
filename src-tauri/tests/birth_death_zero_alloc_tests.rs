//! Zero-allocation gate for the agent birth and death command paths (B2).
//!
//! Covers all three sites in `core::agent_systems`, and both mortality systems the trunk schedule
//! runs (`build_tick_schedule` registers `agent_mortality_system` AND `core::mortality::
//! mortality_system`; QD-A' keeps the second and drops the first only later, so the gate is written
//! against the observable — N agents die, a child is born, the window allocates nothing — rather
//! than against whichever system does the killing):
//!   - Site 1: `agent_mortality_system` (per tick, with deaths)
//!   - Site 2: `ReclaimAndDespawnAgentCommand::apply` (per death, whichever system queues it)
//!   - Site 3: `SpawnOffspringCommand::apply` (per birth)
//!   - Window 3: `core::mortality::mortality_system`, the policy-driven system, which queues the
//!     same Site 2 command.
//!   - Max-size windows: Sites 2 and 3 again, on bodies of `MAX_MIGRATION_MORPHOLOGY_NODES`
//!     segments — the largest restore and migration admit — because a two-segment body never fills
//!     a body-walk buffer and so cannot show one spilling to the heap.
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
use anima_engine_lib::core::components::MAX_MIGRATION_MORPHOLOGY_NODES;
use anima_engine_lib::core::ecology::EcosystemBiomass;
use anima_engine_lib::core::ecs::{
    Agent, AgentClass, AgentParentLineageIds, ChildrenLinks, FeatureTracker, ParentAgent, Prey,
    Segment, Velocity,
};
use anima_engine_lib::core::mortality::mortality_system;
use anima_engine_lib::core::resources::{BrainPolicy, MortalityPolicy, SimRng, VitalStatistics};
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
            starving_ticks: 0,
        },
        AgentGenotype(genotype.clone()),
        AgentEvaluation {
            start_position: Vec3::ZERO,
            total_distance: 0.0,
            total_energy_expended: 0.0,
            survival_ticks,
            last_position: Vec3::ZERO,
            prey_energy_gained: 0.0,
            total_energy_gained: 0.0,
        },
        FeatureTracker::default(),
        AgentLineageId(lineage.to_string()),
        AgentGeneration(0),
        AgentParentLineageIds(Vec::new()),
    ));
    root
}

fn limb_node(id: u32) -> MorphologyNode {
    MorphologyNode {
        id,
        length: 0.5,
        radius: 0.1,
        mass: 0.2,
    }
}

fn limb_edge(source_node: u32, target_node: u32) -> MorphologyEdge {
    MorphologyEdge {
        source_node,
        target_node,
        joint_anchor: Vec3::new(0.0, 0.0, 0.5),
        joint_axis: Vec3::X,
    }
}

/// `nodes` segments in a line: the walk's stack holds one entry at a time, while the set of
/// entities already reached grows as long as the body.
fn chain_genotype(nodes: u32) -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    for id in 0..nodes {
        g.add_node(limb_node(id));
    }
    for id in 1..nodes {
        g.add_edge(limb_edge(id - 1, id));
    }
    g
}

/// A root with `nodes - 1` limbs: the walk pushes every limb onto its stack at once.
fn star_genotype(nodes: u32) -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    for id in 0..nodes {
        g.add_node(limb_node(id));
    }
    for id in 1..nodes {
        g.add_edge(limb_edge(0, id));
    }
    g
}

/// Allocation counts for the largest bodies restore and migration admit.
struct MaxBodyAllocs {
    chain_despawn: usize,
    star_despawn: usize,
    star_birth_excess: usize,
    star_birth_total: usize,
    star_decode_baseline: usize,
}

/// Sites 2 and 3 on bodies of `MAX_MIGRATION_MORPHOLOGY_NODES` segments, in a world of their own.
///
/// An agent has one entity per node, so this is the most a body walk can hold for any morphology
/// the engine admits. Both shapes go through the death command — the chain fills the reached set,
/// the star fills the stack — and the star goes through the birth command, where it fills both.
fn measure_max_size_bodies() -> MaxBodyAllocs {
    let max_nodes = MAX_MIGRATION_MORPHOLOGY_NODES as u32;
    let chain = chain_genotype(max_nodes);
    let star = star_genotype(max_nodes);
    let offspring = |lineage: &str| SpawnOffspringCommand {
        parent_entity: Entity::PLACEHOLDER,
        genotype: star.clone(),
        initial_pos: Vec3::new(3.0, 0.0, 3.0),
        initial_rot: Quat::IDENTITY,
        agent_class: AgentClass::Prey,
        lineage_id: lineage.to_string(),
        generation: 1,
        parent_ids: Vec::new(),
        starting_energy: 40.0,
        energy_target: 100.0,
        brain: None,
    };

    let mut world = World::new();
    world.insert_resource(EcosystemBiomass::default());
    world.insert_resource(SimRng::from_seed(4243));

    // Warm-up: two of every body alive at once, then all killed, so the archetype tables, the
    // entity free list and the removal-event buffers have room for more entities than any window
    // below touches.
    for round in 0..2 {
        spawn_dying_two_segment_agent(&mut world, &chain, &format!("warm-chain-{round}"), 0);
        spawn_dying_two_segment_agent(&mut world, &star, &format!("warm-star-{round}"), 0);
        offspring(&format!("warm-child-{round}")).apply(&mut world);
    }
    let warm_roots: Vec<Entity> = world
        .query_filtered::<Entity, With<Agent>>()
        .iter(&world)
        .collect();
    assert_eq!(warm_roots.len(), 6);
    for root in warm_roots {
        ReclaimAndDespawnAgentCommand { root }.apply(&mut world);
    }
    // Two swaps leave the removal-event buffer that the warm-up grew as the active one (see the
    // same step in the main test).
    world.clear_trackers();
    world.clear_trackers();
    assert_eq!(world.query::<&Segment>().iter(&world).count(), 0);

    fn despawn_window(world: &mut World, genotype: &MorphologyGenotype, name: &str) -> usize {
        let root = spawn_dying_two_segment_agent(world, genotype, name, 0);
        assert_eq!(
            world.query::<&Segment>().iter(&*world).count(),
            MAX_MIGRATION_MORPHOLOGY_NODES,
            "{name}: one entity per node"
        );
        ALLOCATOR.start_tracking();
        ReclaimAndDespawnAgentCommand { root }.apply(world);
        let allocs = ALLOCATOR.stop_tracking();
        assert_eq!(
            world.query::<&Segment>().iter(&*world).count(),
            0,
            "{name}: every segment despawned"
        );
        world.clear_trackers();
        world.clear_trackers();
        allocs
    }
    let chain_despawn = despawn_window(&mut world, &chain, "max-chain");
    let star_despawn = despawn_window(&mut world, &star, "max-star");

    ALLOCATOR.start_tracking();
    let probe = decode_genotype(&mut world, &star, Vec3::ZERO, Quat::IDENTITY);
    let star_decode_baseline = ALLOCATOR.stop_tracking();
    ReclaimAndDespawnAgentCommand { root: probe }.apply(&mut world);
    world.clear_trackers();
    world.clear_trackers();

    let cmd = offspring("max-star-child");
    ALLOCATOR.start_tracking();
    cmd.apply(&mut world);
    let star_birth_total = ALLOCATOR.stop_tracking();

    // The walk must still reach every limb, not merely stop allocating.
    let child = world
        .query::<(Entity, &AgentLineageId)>()
        .iter(&world)
        .find_map(|(e, id)| (id.0 == "max-star-child").then_some(e))
        .expect("max-size offspring exists");
    let root_vel = world.get::<Velocity>(child).unwrap().0;
    let limbs = &world.get::<ChildrenLinks>(child).unwrap().0;
    assert_eq!(limbs.len(), MAX_MIGRATION_MORPHOLOGY_NODES - 1);
    for limb in limbs {
        assert_eq!(
            world.get::<RigidBody>(*limb).unwrap().velocity,
            root_vel,
            "every limb of a max-size newborn must get the root's velocity"
        );
    }

    MaxBodyAllocs {
        chain_despawn,
        star_despawn,
        star_birth_excess: star_birth_total.saturating_sub(star_decode_baseline),
        star_birth_total,
        star_decode_baseline,
    }
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
    let max_body = measure_max_size_bodies();

    let mut world = World::new();
    world.insert_resource(EcosystemBiomass::default());
    world.insert_resource(DemographicTracker::default());
    world.insert_resource(SimRng::from_seed(4242));
    // Enabled and instant (`starve_ticks == 0`): `mortality_system` kills any agent whose energy is
    // already zero. `agent_mortality_system` only returns early when the policy is *disabled*, so
    // an enabled policy leaves its behaviour, and Windows 1 and 2, unchanged.
    world.insert_resource(MortalityPolicy {
        enabled: true,
        starve_ticks: 0,
        dehydration_ticks: u32::MAX,
    });
    world.insert_resource(VitalStatistics::default());

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(ExecutorKind::SingleThreaded);
    schedule.add_systems((agent_mortality_system, apply_deferred).chain());

    // The policy-driven death system the trunk keeps (QD-A'), in its own schedule so a window can
    // exercise exactly one mortality system at a time.
    let mut policy_schedule = Schedule::default();
    policy_schedule.set_executor_kind(ExecutorKind::SingleThreaded);
    policy_schedule.add_systems((mortality_system, apply_deferred).chain());

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
    // Warm the policy schedule the same way: initialise its systems and size its command buffer
    // with several concurrent deaths. The run above only killed the zero-energy warm agents, so
    // spawn fresh zero-energy ones for it.
    for i in 0..4 {
        spawn_dying_two_segment_agent(&mut world, &genotype, &format!("warm-policy-{i}"), 0);
    }
    policy_schedule.run(&mut world);

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
    policy_schedule.run(&mut world);
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

    // ---- Window 3: Policy-driven death path (`mortality_system` + Site 2) -----------------------
    // The mortality system the trunk keeps under QD-A'. It queues the same
    // `ReclaimAndDespawnAgentCommand`, so Site 2 must be clean under it too.
    let _p1 = spawn_dying_two_segment_agent(&mut world, &genotype, "policy-z", 15);
    let _p2 = spawn_dying_two_segment_agent(&mut world, &genotype, "policy-a", 350);
    assert_eq!(world.query::<&Agent>().iter(&world).count(), 2);
    assert_eq!(world.query::<&Segment>().iter(&world).count(), 4);
    let policy_deaths_before = world.resource::<VitalStatistics>().total_deaths;

    ALLOCATOR.start_tracking();
    policy_schedule.run(&mut world);
    let policy_death_allocs = ALLOCATOR.stop_tracking();

    assert_eq!(
        world.resource::<VitalStatistics>().total_deaths - policy_deaths_before,
        2,
        "window 3 must force at least two deaths through mortality_system"
    );
    assert_eq!(world.query::<&Agent>().iter(&world).count(), 0);
    assert_eq!(
        world.query::<&Segment>().iter(&world).count(),
        0,
        "mortality_system's deaths must take their child segments with them"
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
    // One assertion over every window, so a red run reports every count at once and the pattern
    // names the site: Window 1 alone => Site 1 or 2 under `agent_mortality_system`; Window 3 as
    // well => Site 2; only the combined window => Site 3; only a max-size window => a body-walk
    // buffer too small for the largest admitted body (Site 2 for the despawns, Site 3 for the birth).
    let MaxBodyAllocs {
        chain_despawn,
        star_despawn,
        star_birth_excess,
        star_birth_total,
        star_decode_baseline,
    } = max_body;
    assert_eq!(
        (
            death_allocs,
            policy_death_allocs,
            command_overhead_allocs,
            chain_despawn,
            star_despawn,
            star_birth_excess
        ),
        (0, 0, 0, 0, 0, 0),
        "birth/death zero-alloc gate failed: \
         window 1 [agent_mortality_system + ReclaimAndDespawnAgentCommand::apply, 2 deaths] allocs={death_allocs}; \
         window 3 [mortality_system + ReclaimAndDespawnAgentCommand::apply, 2 deaths] allocs={policy_death_allocs}; \
         window 2 [2 deaths via agent_mortality_system + 1 birth via SpawnOffspringCommand::apply] excess allocs={command_overhead_allocs} (total={combined_window_allocs}, decode_genotype_baseline={decode_baseline_allocs}); \
         max-size chain [ReclaimAndDespawnAgentCommand::apply, {MAX_MIGRATION_MORPHOLOGY_NODES} segments] allocs={chain_despawn}; \
         max-size star [ReclaimAndDespawnAgentCommand::apply, {MAX_MIGRATION_MORPHOLOGY_NODES} segments] allocs={star_despawn}; \
         max-size star birth [SpawnOffspringCommand::apply] excess allocs={star_birth_excess} (total={star_birth_total}, decode_genotype_baseline={star_decode_baseline})"
    );
}

/// Correctness companion to the gate. `ReclaimAndDespawnAgentCommand` and `SpawnOffspringCommand`
/// now find an agent's segments by walking `ChildrenLinks` instead of scanning every entity for
/// `ParentAgent`. That is only the same set if the two links agree, so pin it on a branched,
/// multi-level tree, and on bodies past `MAX_MIGRATION_MORPHOLOGY_NODES`, the inline capacity of
/// the walk's buffer: no admitted body is that large, but `decode_genotype` called directly will
/// build one, and the walk must spill to the heap rather than lose entities. The star is the case
/// that actually spills, since it holds every limb on the stack at once; the chain is the long one.
#[test]
fn despawn_takes_every_segment_of_branched_and_oversized_trees() {
    // Branched: 0 -> {1, 2}, 1 -> {3, 4}, 2 -> 5. Six nodes, three levels.
    let mut branched = MorphologyGenotype::new();
    for id in 0..6 {
        branched.add_node(limb_node(id));
    }
    for (s, t) in [(0, 1), (0, 2), (1, 3), (1, 4), (2, 5)] {
        branched.add_edge(limb_edge(s, t));
    }
    let oversized = MAX_MIGRATION_MORPHOLOGY_NODES as u32 + 2;
    let segments_past_root = MAX_MIGRATION_MORPHOLOGY_NODES + 1;

    for (name, genotype, expected_segments) in [
        ("branched", branched, 5),
        (
            "chain-oversized",
            chain_genotype(oversized),
            segments_past_root,
        ),
        (
            "star-oversized",
            star_genotype(oversized),
            segments_past_root,
        ),
    ] {
        let mut world = World::new();
        world.insert_resource(EcosystemBiomass::default());
        let root = spawn_dying_two_segment_agent(&mut world, &genotype, name, 0);
        let bystander =
            spawn_dying_two_segment_agent(&mut world, &two_segment_genotype(), "bystander", 0);

        let by_parent_agent: Vec<Entity> = world
            .query::<(Entity, &ParentAgent)>()
            .iter(&world)
            .filter_map(|(e, p)| (p.0 == root && e != root).then_some(e))
            .collect();
        assert_eq!(
            by_parent_agent.len(),
            expected_segments,
            "{name}: ParentAgent scan must find every segment"
        );

        ReclaimAndDespawnAgentCommand { root }.apply(&mut world);

        assert!(world.get_entity(root).is_none(), "{name}: root despawned");
        for e in &by_parent_agent {
            assert!(
                world.get_entity(*e).is_none(),
                "{name}: segment {e:?} found by ParentAgent survived despawn"
            );
        }
        assert!(
            world.get_entity(bystander).is_some(),
            "{name}: another agent's entity must not be despawned"
        );
        assert_eq!(
            world.query::<&Segment>().iter(&world).count(),
            2,
            "{name}: only the bystander's root and its one child segment remain"
        );
    }
}
