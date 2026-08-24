use std::sync::{Arc, RwLock};
use std::time::Duration;

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecs::{
    check_migration_boundaries_system, manual_migration_system, process_inbound_migrations_system,
    Agent, AgentBrain, AgentClass, AgentMigrationData, BevyMigrationTrigger, ChildrenLinks,
    FeatureTracker, InboundMigrationReceiver, MapBounds, OutboundMigration,
    OutboundMigrationSender, Position, Prey, Segment, ShardingConfig, ShardingResource,
    SpawnMigrationCommand, Velocity, MAX_MIGRATION_BRAIN_PARAMETERS,
    MAX_MIGRATION_MORPHOLOGY_NODES,
};
use anima_engine_lib::core::engine::{AgentGeneration, AgentGenotype, AgentLineageId};
use anima_engine_lib::core::resources::{
    inbound_migration_channel, outbound_migration_channel, MigrationHandoffDiagnostics, SimRng,
    INBOUND_MIGRATIONS_PER_TICK, INBOUND_MIGRATION_QUEUE_CAPACITY,
    OUTBOUND_MIGRATION_QUEUE_CAPACITY,
};
use anima_engine_lib::evolution::brain_genotype::{
    ArchSpec, BrainGenotype, BRAIN_GENOTYPE_VERSION,
};
use anima_engine_lib::evolution::genotype::{MorphologyEdge, MorphologyGenotype, MorphologyNode};
use bevy_ecs::prelude::*;
use bevy_ecs::system::Command;
use glam::Vec3;

fn one_node_genotype() -> MorphologyGenotype {
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.2,
        mass: 1.0,
    });
    genotype
}

fn spawn_agent_tree(world: &mut World, position: Vec3) -> (Entity, Entity) {
    let child = world.spawn(ChildrenLinks(Vec::new())).id();
    let brain_arch = ArchSpec::new(1, 1, 1);
    let brain = AgentBrain::from_genotype(
        BrainGenotype::from_weights(brain_arch, vec![0.25; brain_arch.param_count()]).unwrap(),
    );
    let agent = world
        .spawn((
            Agent,
            Prey,
            Position(position),
            Velocity(Vec3::new(1.0, 0.0, 0.0)),
            HomeostaticState {
                energy: 73.0,
                energy_target: 100.0,
                hydration: 61.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
            AgentGenotype(one_node_genotype()),
            AgentLineageId("enqueue-atomicity".to_owned()),
            AgentGeneration(7),
            brain,
            ChildrenLinks(vec![child]),
        ))
        .id();
    (agent, child)
}

fn spawn_wide_agent_tree(world: &mut World, position: Vec3) -> (Entity, Vec<Entity>) {
    const CHILDREN: usize = 65;

    let (agent, first_child) = spawn_agent_tree(world, position);
    let mut children = Vec::with_capacity(CHILDREN);
    children.push(first_child);
    for _ in 1..CHILDREN {
        children.push(world.spawn(ChildrenLinks(Vec::new())).id());
    }
    world.get_mut::<ChildrenLinks>(agent).unwrap().0 = children.clone();
    (agent, children)
}

fn wide_star_genotype() -> MorphologyGenotype {
    star_genotype(66)
}

fn star_genotype(nodes: usize) -> MorphologyGenotype {
    assert!(nodes > 0);
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.2,
        mass: 1.0,
    });
    for id in 1..nodes as u32 {
        genotype.add_node(MorphologyNode {
            id,
            length: 0.5,
            radius: 0.1,
            mass: 0.25,
        });
        genotype.add_edge(MorphologyEdge {
            source_node: 0,
            target_node: id,
            joint_anchor: Vec3::ZERO,
            joint_axis: Vec3::Y,
        });
    }
    genotype
}

fn deep_wide_frontier_genotype() -> MorphologyGenotype {
    const DEPTH: usize = 5;
    const LEAVES_PER_LEVEL: usize = 14;

    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.2,
        mass: 1.0,
    });

    let mut next_id = 1_u32;
    let mut spine = 0_u32;
    for _ in 0..DEPTH {
        // Leaves are inserted first and the continuation last. A depth-first LIFO traversal keeps
        // the leaves pending while it follows the spine, growing the frontier past 64 without any
        // single node having an extreme fan-out.
        for _ in 0..LEAVES_PER_LEVEL {
            let leaf = next_id;
            next_id += 1;
            genotype.add_node(MorphologyNode {
                id: leaf,
                length: 0.25,
                radius: 0.05,
                mass: 0.1,
            });
            genotype.add_edge(MorphologyEdge {
                source_node: spine,
                target_node: leaf,
                joint_anchor: Vec3::ZERO,
                joint_axis: Vec3::Y,
            });
        }

        let continuation = next_id;
        next_id += 1;
        genotype.add_node(MorphologyNode {
            id: continuation,
            length: 0.5,
            radius: 0.1,
            mass: 0.25,
        });
        genotype.add_edge(MorphologyEdge {
            source_node: spine,
            target_node: continuation,
            joint_anchor: Vec3::ZERO,
            joint_axis: Vec3::Y,
        });
        spine = continuation;
    }

    genotype
}

fn migration_data(genotype: MorphologyGenotype, velocity: Vec3) -> AgentMigrationData {
    AgentMigrationData {
        genotype,
        homeostatic_state: HomeostaticState {
            energy: 73.0,
            energy_target: 100.0,
            hydration: 61.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        position: Vec3::ZERO,
        velocity,
        lineage_id: "wide-inbound".to_owned(),
        generation: 7,
        agent_class: AgentClass::Prey,
        parent_ids: Vec::new(),
        evaluation: None,
        feature_tracker: None,
        last_transition_state: None,
        source_port: 8081,
        brain: None,
    }
}

fn insert_sharding_resources(world: &mut World) {
    world.insert_resource(MapBounds {
        min: Vec3::new(-100.0, -10.0, -100.0),
        max: Vec3::new(100.0, 10.0, 100.0),
    });
    world.insert_resource(ShardingResource(Arc::new(RwLock::new(ShardingConfig {
        local_port: 8080,
        right_target_port: Some(8081),
        left_target_port: Some(8079),
    }))));
    world.insert_resource(MigrationHandoffDiagnostics::default());
}

fn insert_migration_resources(world: &mut World) -> crossbeam_channel::Receiver<OutboundMigration> {
    insert_sharding_resources(world);
    let (outbound_tx, outbound_rx) = crossbeam_channel::unbounded();
    world.insert_resource(OutboundMigrationSender(outbound_tx));
    outbound_rx
}

fn run_tick_with_stalled_receiver(
    mut world: World,
    mut schedule: Schedule,
    stalled_receiver: crossbeam_channel::Receiver<OutboundMigration>,
) -> (bool, World) {
    let (done_tx, done_rx) = crossbeam_channel::bounded(1);
    let tick = std::thread::spawn(move || {
        schedule.run(&mut world);
        done_tx.send(world).unwrap();
    });

    // This is a hang guard, not a performance budget. Five seconds is deliberately far above a
    // one-agent Bevy tick even on a contended CI host, while still releasing a blocking regression.
    let first = done_rx.recv_timeout(Duration::from_secs(5));
    let (finished_without_unblock, world) = match first {
        Ok(world) => (true, world),
        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
            // Release the old blocking implementation so a red test never strands a test thread.
            drop(stalled_receiver);
            let world = done_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("the migration tick must finish after its stalled receiver is dropped");
            (false, world)
        }
        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
            panic!("migration tick exited without returning its world")
        }
    };

    tick.join().expect("migration tick thread must not panic");
    (finished_without_unblock, world)
}

fn assert_complete_agent_state_is_preserved(world: &World, agent: Entity, child: Entity) {
    assert_eq!(world.get::<HomeostaticState>(agent).unwrap().energy, 73.0);
    assert_eq!(
        world.get::<AgentLineageId>(agent).unwrap().0,
        "enqueue-atomicity"
    );
    assert_eq!(world.get::<AgentGeneration>(agent).unwrap().0, 7);
    assert_eq!(
        world.get::<ChildrenLinks>(agent).unwrap().0.as_slice(),
        &[child]
    );
    assert_eq!(
        world.get::<AgentBrain>(agent).unwrap().genotype.weights,
        vec![0.25; ArchSpec::new(1, 1, 1).param_count()]
    );
    assert!(world.get_entity(child).is_some());
}

#[test]
fn automatic_migration_keeps_agent_tree_when_outbound_queue_is_disconnected() {
    let mut world = World::new();
    let outbound_rx = insert_migration_resources(&mut world);
    drop(outbound_rx);
    let (agent, child) = spawn_agent_tree(&mut world, Vec3::new(101.0, 0.0, 0.0));

    let mut schedule = Schedule::default();
    schedule.add_systems(check_migration_boundaries_system);
    schedule.run(&mut world);

    assert_complete_agent_state_is_preserved(&world, agent, child);
    assert!(
        world.get::<Position>(agent).unwrap().0.x < 100.0,
        "a rejected handoff must return the agent inside its local shard"
    );
    assert!(
        world.get::<Velocity>(agent).unwrap().0.x < 0.0,
        "a rejected right-boundary handoff must reflect velocity inward"
    );
    let reflected_position = world.get::<Position>(agent).unwrap().0;
    let reflected_velocity = world.get::<Velocity>(agent).unwrap().0;

    schedule.run(&mut world);
    assert_complete_agent_state_is_preserved(&world, agent, child);
    assert_eq!(world.get::<Position>(agent).unwrap().0, reflected_position);
    assert_eq!(world.get::<Velocity>(agent).unwrap().0, reflected_velocity);
    assert_eq!(
        world
            .resource::<MigrationHandoffDiagnostics>()
            .snapshot()
            .disconnected_rejections,
        1
    );
}

#[test]
fn rejected_left_boundary_handoff_reflects_agent_inward() {
    let mut world = World::new();
    let outbound_rx = insert_migration_resources(&mut world);
    drop(outbound_rx);
    let (agent, child) = spawn_agent_tree(&mut world, Vec3::new(-101.0, 0.0, 0.0));
    world.get_mut::<Velocity>(agent).unwrap().0.x = -1.0;

    let mut schedule = Schedule::default();
    schedule.add_systems(check_migration_boundaries_system);
    schedule.run(&mut world);

    assert_complete_agent_state_is_preserved(&world, agent, child);
    assert!(world.get::<Position>(agent).unwrap().0.x > -100.0);
    assert!(world.get::<Velocity>(agent).unwrap().0.x > 0.0);
}

#[test]
fn manual_migration_keeps_agent_tree_when_outbound_queue_is_disconnected() {
    let mut world = World::new();
    let outbound_rx = insert_migration_resources(&mut world);
    drop(outbound_rx);
    world.insert_resource(SimRng::from_seed(0xA70C));

    let (trigger_tx, trigger_rx) = crossbeam_channel::unbounded();
    trigger_tx.send(8081).unwrap();
    world.insert_resource(BevyMigrationTrigger(trigger_rx));

    let (agent, child) = spawn_agent_tree(&mut world, Vec3::ZERO);

    let mut schedule = Schedule::default();
    schedule.add_systems(manual_migration_system);
    schedule.run(&mut world);

    assert_complete_agent_state_is_preserved(&world, agent, child);

    schedule.run(&mut world);
    assert_complete_agent_state_is_preserved(&world, agent, child);
    assert_eq!(
        world
            .resource::<MigrationHandoffDiagnostics>()
            .snapshot()
            .disconnected_rejections,
        1
    );
}

#[test]
fn automatic_migration_transfers_ownership_after_enqueue_succeeds() {
    let mut world = World::new();
    let outbound_rx = insert_migration_resources(&mut world);
    let (agent, child) = spawn_agent_tree(&mut world, Vec3::new(101.0, 0.0, 0.0));

    let mut schedule = Schedule::default();
    schedule.add_systems(check_migration_boundaries_system);
    schedule.run(&mut world);

    let outbound = outbound_rx
        .try_recv()
        .expect("successful automatic migration must enqueue its payload");
    assert_eq!(outbound.target_port, 8081);
    assert_eq!(outbound.data.lineage_id, "enqueue-atomicity");
    assert_eq!(outbound.data.homeostatic_state.energy, 73.0);
    assert!(outbound.data.brain.is_some());
    assert!(world.get_entity(agent).is_none());
    assert!(world.get_entity(child).is_none());
    assert_eq!(
        world
            .resource::<MigrationHandoffDiagnostics>()
            .snapshot()
            .queued,
        1
    );
}

#[test]
fn manual_migration_transfers_ownership_after_enqueue_succeeds() {
    let mut world = World::new();
    let outbound_rx = insert_migration_resources(&mut world);
    world.insert_resource(SimRng::from_seed(0xA70C));

    let (trigger_tx, trigger_rx) = crossbeam_channel::unbounded();
    trigger_tx.send(8090).unwrap();
    world.insert_resource(BevyMigrationTrigger(trigger_rx));

    let (agent, child) = spawn_agent_tree(&mut world, Vec3::ZERO);

    let mut schedule = Schedule::default();
    schedule.add_systems(manual_migration_system);
    schedule.run(&mut world);

    let outbound = outbound_rx
        .try_recv()
        .expect("successful manual migration must enqueue its payload");
    assert_eq!(outbound.target_port, 8090);
    assert_eq!(outbound.data.lineage_id, "enqueue-atomicity");
    assert_eq!(outbound.data.homeostatic_state.energy, 73.0);
    assert!(outbound.data.brain.is_some());
    assert!(world.get_entity(agent).is_none());
    assert!(world.get_entity(child).is_none());
    assert_eq!(
        world
            .resource::<MigrationHandoffDiagnostics>()
            .snapshot()
            .queued,
        1
    );
}

#[test]
fn automatic_migration_never_blocks_when_the_outbound_queue_is_full() {
    let mut world = World::new();
    insert_sharding_resources(&mut world);
    let (outbound_tx, stalled_receiver) = crossbeam_channel::bounded(0);
    world.insert_resource(OutboundMigrationSender(outbound_tx));
    let (agent, child) = spawn_agent_tree(&mut world, Vec3::new(101.0, 0.0, 0.0));

    let mut schedule = Schedule::default();
    schedule.add_systems(check_migration_boundaries_system);
    let (finished_without_unblock, world) =
        run_tick_with_stalled_receiver(world, schedule, stalled_receiver);

    assert!(
        finished_without_unblock,
        "a full migration queue must apply backpressure without blocking the simulation tick"
    );
    assert_complete_agent_state_is_preserved(&world, agent, child);
    assert_eq!(
        world
            .resource::<MigrationHandoffDiagnostics>()
            .snapshot()
            .full_rejections,
        1
    );
}

#[test]
fn manual_migration_never_blocks_when_the_outbound_queue_is_full() {
    let mut world = World::new();
    insert_sharding_resources(&mut world);
    world.insert_resource(SimRng::from_seed(0xA70C));
    let (outbound_tx, stalled_receiver) = crossbeam_channel::bounded(0);
    world.insert_resource(OutboundMigrationSender(outbound_tx));

    let (trigger_tx, trigger_rx) = crossbeam_channel::unbounded();
    trigger_tx.send(8081).unwrap();
    world.insert_resource(BevyMigrationTrigger(trigger_rx));
    let (agent, child) = spawn_agent_tree(&mut world, Vec3::ZERO);

    let mut schedule = Schedule::default();
    schedule.add_systems(manual_migration_system);
    let (finished_without_unblock, world) =
        run_tick_with_stalled_receiver(world, schedule, stalled_receiver);

    assert!(
        finished_without_unblock,
        "a full migration queue must not let a manual request block the simulation tick"
    );
    assert_complete_agent_state_is_preserved(&world, agent, child);
    assert_eq!(
        world
            .resource::<MigrationHandoffDiagnostics>()
            .snapshot()
            .full_rejections,
        1
    );
}

#[test]
fn production_outbound_migration_queue_has_a_finite_capacity() {
    let (sender, receiver) = outbound_migration_channel();

    assert_eq!(sender.capacity(), Some(OUTBOUND_MIGRATION_QUEUE_CAPACITY));
    assert_eq!(receiver.capacity(), Some(OUTBOUND_MIGRATION_QUEUE_CAPACITY));
}

#[test]
fn production_inbound_migration_queue_has_a_finite_capacity() {
    let (sender, receiver) = inbound_migration_channel();
    for index in 0..INBOUND_MIGRATION_QUEUE_CAPACITY {
        sender
            .try_send(migration_data(one_node_genotype(), Vec3::X))
            .unwrap_or_else(|_| panic!("slot {index} must fit"));
    }
    assert!(matches!(
        sender.try_send(migration_data(one_node_genotype(), Vec3::X)),
        Err(crossbeam_channel::TrySendError::Full(_))
    ));
    assert_eq!(receiver.len(), INBOUND_MIGRATION_QUEUE_CAPACITY);
}

#[test]
fn inbound_reconstruction_obeys_the_per_tick_budget() {
    let mut world = World::new();
    world.insert_resource(MigrationHandoffDiagnostics::default());
    let (sender, receiver) = inbound_migration_channel();
    world.insert_resource(InboundMigrationReceiver(receiver));
    for index in 0..(INBOUND_MIGRATIONS_PER_TICK + 3) {
        let mut data = migration_data(one_node_genotype(), Vec3::X);
        data.lineage_id = format!("budgeted-inbound-{index}");
        sender.try_send(data).unwrap();
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(process_inbound_migrations_system);
    schedule.run(&mut world);
    let mut agents = world.query_filtered::<Entity, With<Agent>>();
    assert_eq!(agents.iter(&world).count(), INBOUND_MIGRATIONS_PER_TICK);
    assert_eq!(world.resource::<InboundMigrationReceiver>().0.len(), 3);

    schedule.run(&mut world);
    assert_eq!(agents.iter(&world).count(), INBOUND_MIGRATIONS_PER_TICK + 3);
    assert!(world.resource::<InboundMigrationReceiver>().0.is_empty());
}

#[test]
fn migration_burst_is_capped_and_excess_agents_remain_local() {
    const EXCESS: usize = 9;

    let mut world = World::new();
    insert_sharding_resources(&mut world);
    let (outbound_tx, outbound_rx) = outbound_migration_channel();
    world.insert_resource(OutboundMigrationSender(outbound_tx));

    for index in 0..(OUTBOUND_MIGRATION_QUEUE_CAPACITY + EXCESS) {
        let x = 101.0 + index as f32 * 0.001;
        spawn_agent_tree(&mut world, Vec3::new(x, 0.0, 0.0));
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(check_migration_boundaries_system);
    schedule.run(&mut world);

    assert_eq!(outbound_rx.len(), OUTBOUND_MIGRATION_QUEUE_CAPACITY);
    assert_eq!(
        world.resource::<MigrationHandoffDiagnostics>().snapshot(),
        anima_engine_lib::core::resources::MigrationHandoffSnapshot {
            queued: OUTBOUND_MIGRATION_QUEUE_CAPACITY as u64,
            full_rejections: EXCESS as u64,
            disconnected_rejections: 0,
            invalid_rejections: 0,
            inbound_backpressure_events: 0,
            connection_limit_rejections: 0,
        }
    );
    let mut remaining = world.query_filtered::<(&Position, &Velocity), With<Agent>>();
    let remaining = remaining.iter(&world).collect::<Vec<_>>();
    assert_eq!(remaining.len(), EXCESS);
    assert!(
        remaining
            .iter()
            .all(|(position, velocity)| position.0.x < 100.0 && velocity.0.x < 0.0),
        "every agent rejected by backpressure must be reflected into the local shard"
    );
}

#[test]
fn migration_handoff_diagnostics_reset_between_runs() {
    let diagnostics = MigrationHandoffDiagnostics::default();
    diagnostics.record_queued();
    diagnostics.record_full_rejection();
    diagnostics.record_disconnected_rejection();
    diagnostics.record_invalid_rejection();
    assert_ne!(diagnostics.snapshot(), Default::default());

    diagnostics.reset();

    assert_eq!(diagnostics.snapshot(), Default::default());
}

#[test]
fn automatic_migration_despawns_every_segment_of_a_wide_agent() {
    let mut world = World::new();
    let outbound_rx = insert_migration_resources(&mut world);
    let (agent, children) = spawn_wide_agent_tree(&mut world, Vec3::new(101.0, 0.0, 0.0));

    let mut schedule = Schedule::default();
    schedule.add_systems(check_migration_boundaries_system);
    schedule.run(&mut world);

    outbound_rx
        .try_recv()
        .expect("the wide agent must enter the outbound queue");
    assert!(world.get_entity(agent).is_none());
    assert!(
        children
            .iter()
            .all(|&child| world.get_entity(child).is_none()),
        "migration must not leave a segment orphaned when one node has more than 64 children"
    );
}

#[test]
fn manual_migration_despawns_every_segment_of_a_wide_agent() {
    let mut world = World::new();
    let outbound_rx = insert_migration_resources(&mut world);
    world.insert_resource(SimRng::from_seed(0xA70C));
    let (trigger_tx, trigger_rx) = crossbeam_channel::unbounded();
    trigger_tx.send(8090).unwrap();
    world.insert_resource(BevyMigrationTrigger(trigger_rx));
    let (agent, children) = spawn_wide_agent_tree(&mut world, Vec3::ZERO);

    let mut schedule = Schedule::default();
    schedule.add_systems(manual_migration_system);
    schedule.run(&mut world);

    outbound_rx
        .try_recv()
        .expect("the wide agent must enter the outbound queue");
    assert!(world.get_entity(agent).is_none());
    assert!(
        children
            .iter()
            .all(|&child| world.get_entity(child).is_none()),
        "manual migration must not leave a segment orphaned past the fixed-stack boundary"
    );
}

#[test]
fn inbound_migration_applies_velocity_to_every_segment_of_a_wide_agent() {
    let mut world = World::new();
    let velocity = Vec3::new(3.0, -2.0, 1.0);
    SpawnMigrationCommand {
        data: migration_data(wide_star_genotype(), velocity),
    }
    .apply(&mut world);

    let mut segments = world.query_filtered::<&Velocity, With<Segment>>();
    let velocities = segments.iter(&world).map(|v| v.0).collect::<Vec<_>>();
    assert_eq!(velocities.len(), 66);
    assert!(
        velocities.iter().all(|&actual| actual == velocity),
        "every reconstructed segment must inherit the migrating creature's velocity"
    );
}

#[test]
fn inbound_migration_starts_a_new_learning_transition_boundary() {
    let mut world = World::new();
    let mut data = migration_data(one_node_genotype(), Vec3::X);
    data.last_transition_state = Some(anima_engine_lib::ai::hrrl::LastTransitionState {
        state: [0.25; 15],
        action: [0.5; 4],
        has_last: true,
        pending_state: Some([0.75; 15]),
    });

    SpawnMigrationCommand { data }.apply(&mut world);

    let mut agents = world.query_filtered::<Entity, With<Agent>>();
    let root = agents
        .get_single(&world)
        .expect("the migrated morphology must have one agent root");
    let transition = world
        .get::<anima_engine_lib::ai::hrrl::LastTransitionState>(root)
        .expect("decoded agents carry transition state");

    assert_eq!(
        transition.state, [0.25; 15],
        "historical observations remain available for diagnostics"
    );
    assert_eq!(
        transition.action, [0.5; 4],
        "historical actions remain available for diagnostics"
    );
    assert!(
        !transition.has_last,
        "a reset controller must not learn across the migration discontinuity"
    );
    assert_eq!(
        transition.pending_state, None,
        "an inference request cannot follow an entity to another shard"
    );
}

#[test]
fn inbound_then_outbound_migration_leaves_no_segment_past_a_deep_wide_frontier() {
    let mut world = World::new();
    SpawnMigrationCommand {
        data: migration_data(deep_wide_frontier_genotype(), Vec3::X),
    }
    .apply(&mut world);

    let mut agents = world.query_filtered::<Entity, With<Agent>>();
    let root = agents
        .get_single(&world)
        .expect("the decoded morphology must have one agent root");
    world.get_mut::<Position>(root).unwrap().0.x = 101.0;

    let outbound_rx = insert_migration_resources(&mut world);
    let mut schedule = Schedule::default();
    schedule.add_systems(check_migration_boundaries_system);
    schedule.run(&mut world);

    outbound_rx
        .try_recv()
        .expect("the reconstructed agent must enter the outbound queue");
    let mut segments = world.query_filtered::<Entity, With<Segment>>();
    assert_eq!(
        segments.iter(&world).count(),
        0,
        "a production-decoded hierarchy must not leave residual segments after migrating out"
    );
}

#[test]
fn automatic_migration_rejects_an_invalid_genotype_without_losing_local_ownership() {
    let mut world = World::new();
    let outbound_rx = insert_migration_resources(&mut world);
    let (agent, child) = spawn_agent_tree(&mut world, Vec3::new(101.0, 0.0, 0.0));
    world.get_mut::<AgentGenotype>(agent).unwrap().0 = MorphologyGenotype::new();

    let mut schedule = Schedule::default();
    schedule.add_systems(check_migration_boundaries_system);
    schedule.run(&mut world);

    assert!(
        outbound_rx.try_recv().is_err(),
        "an invalid scientific payload must never cross the ownership boundary"
    );
    assert_complete_agent_state_is_preserved(&world, agent, child);
    assert_eq!(
        world
            .resource::<MigrationHandoffDiagnostics>()
            .snapshot()
            .invalid_rejections,
        1
    );
    let reflected_position = world.get::<Position>(agent).unwrap().0;
    let reflected_velocity = world.get::<Velocity>(agent).unwrap().0;
    schedule.run(&mut world);
    assert_eq!(world.get::<Position>(agent).unwrap().0, reflected_position);
    assert_eq!(world.get::<Velocity>(agent).unwrap().0, reflected_velocity);
    assert_eq!(
        world
            .resource::<MigrationHandoffDiagnostics>()
            .snapshot()
            .invalid_rejections,
        1,
        "one rejected ownership attempt must be counted once, not once per tick"
    );
}

#[test]
fn automatic_invalid_kinematics_are_made_finite_before_ownership_is_retained() {
    let mut world = World::new();
    let outbound_rx = insert_migration_resources(&mut world);
    let (agent, child) = spawn_agent_tree(&mut world, Vec3::new(f32::NAN, f32::NAN, 0.0));
    world.get_mut::<Velocity>(agent).unwrap().0 = Vec3::new(f32::NAN, 1.0, 2.0);

    let mut schedule = Schedule::default();
    schedule.add_systems(check_migration_boundaries_system);
    schedule.run(&mut world);

    assert!(outbound_rx.try_recv().is_err());
    assert_complete_agent_state_is_preserved(&world, agent, child);
    assert!(world.get::<Position>(agent).unwrap().0.is_finite());
    assert!(world.get::<Velocity>(agent).unwrap().0.is_finite());
    assert_eq!(
        world
            .resource::<MigrationHandoffDiagnostics>()
            .snapshot()
            .invalid_rejections,
        1
    );
}

#[test]
fn manual_migration_rejects_an_invalid_genotype_without_losing_local_ownership() {
    let mut world = World::new();
    let outbound_rx = insert_migration_resources(&mut world);
    world.insert_resource(SimRng::from_seed(0xA70C));
    let (trigger_tx, trigger_rx) = crossbeam_channel::unbounded();
    trigger_tx.send(8090).unwrap();
    world.insert_resource(BevyMigrationTrigger(trigger_rx));
    let (agent, child) = spawn_agent_tree(&mut world, Vec3::ZERO);
    world.get_mut::<AgentGenotype>(agent).unwrap().0 = MorphologyGenotype::new();

    let mut schedule = Schedule::default();
    schedule.add_systems(manual_migration_system);
    schedule.run(&mut world);

    assert!(
        outbound_rx.try_recv().is_err(),
        "manual migration must not hand an invalid payload to the network worker"
    );
    assert_complete_agent_state_is_preserved(&world, agent, child);
    assert_eq!(
        world
            .resource::<MigrationHandoffDiagnostics>()
            .snapshot()
            .invalid_rejections,
        1
    );
    schedule.run(&mut world);
    assert_eq!(
        world
            .resource::<MigrationHandoffDiagnostics>()
            .snapshot()
            .invalid_rejections,
        1,
        "the consumed manual request must not become a rejection rate"
    );
}

#[test]
fn inbound_migration_rejects_an_empty_genotype_instead_of_panicking() {
    let mut world = World::new();
    world.insert_resource(MigrationHandoffDiagnostics::default());
    SpawnMigrationCommand {
        data: migration_data(MorphologyGenotype::new(), Vec3::X),
    }
    .apply(&mut world);

    let mut agents = world.query_filtered::<Entity, With<Agent>>();
    assert_eq!(agents.iter(&world).count(), 0);
    assert_eq!(
        world
            .resource::<MigrationHandoffDiagnostics>()
            .snapshot()
            .invalid_rejections,
        1
    );
}

#[test]
fn migration_validation_rejects_an_overflowing_brain_architecture_without_panicking() {
    let mut data = migration_data(one_node_genotype(), Vec3::X);
    data.brain = Some(AgentBrain::from_genotype(BrainGenotype {
        version: BRAIN_GENOTYPE_VERSION,
        arch: ArchSpec::new(usize::MAX, usize::MAX, usize::MAX),
        weights: Vec::new(),
    }));

    assert!(data.validate().is_err());
}

#[test]
fn migration_validation_rejects_nonfinite_derived_state_and_negative_accumulators() {
    let mut overflowing_deviation = migration_data(one_node_genotype(), Vec3::X);
    overflowing_deviation.homeostatic_state.energy = f32::MAX;
    assert!(overflowing_deviation.validate().is_err());

    let mut impossible_history = migration_data(one_node_genotype(), Vec3::X);
    impossible_history.homeostatic_state.previous_deviation = -1.0;
    assert!(impossible_history.validate().is_err());

    let mut negative_accumulator = migration_data(one_node_genotype(), Vec3::X);
    negative_accumulator.feature_tracker = Some(FeatureTracker {
        cumulative_distance: -1.0,
        cumulative_energy_decay: 0.0,
        tick_count: 1,
    });
    assert!(negative_accumulator.validate().is_err());
}

#[test]
fn migration_validation_pins_morphology_and_brain_size_limits() {
    let at_limit = migration_data(star_genotype(MAX_MIGRATION_MORPHOLOGY_NODES), Vec3::X);
    assert!(at_limit.validate().is_ok());

    let over_limit = migration_data(star_genotype(MAX_MIGRATION_MORPHOLOGY_NODES + 1), Vec3::X);
    assert!(over_limit.validate().is_err());

    let mut oversized_brain = migration_data(one_node_genotype(), Vec3::X);
    let arch = ArchSpec::new(128, 128, 8);
    let parameters = arch.checked_param_count().unwrap();
    assert!(parameters > MAX_MIGRATION_BRAIN_PARAMETERS);
    oversized_brain.brain = Some(AgentBrain::from_genotype(BrainGenotype {
        version: BRAIN_GENOTYPE_VERSION,
        arch,
        weights: Vec::new(),
    }));
    assert!(oversized_brain.validate().is_err());
}
