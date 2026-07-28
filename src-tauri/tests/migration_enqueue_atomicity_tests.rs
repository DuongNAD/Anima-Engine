use std::sync::{Arc, RwLock};
use std::time::Duration;

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecs::{
    check_migration_boundaries_system, manual_migration_system, Agent, AgentBrain,
    BevyMigrationTrigger, ChildrenLinks, MapBounds, OutboundMigration, OutboundMigrationSender,
    Position, Prey, ShardingConfig, ShardingResource, Velocity,
};
use anima_engine_lib::core::engine::{AgentGeneration, AgentGenotype, AgentLineageId};
use anima_engine_lib::core::resources::{
    outbound_migration_channel, MigrationHandoffDiagnostics, SimRng,
    OUTBOUND_MIGRATION_QUEUE_CAPACITY,
};
use anima_engine_lib::evolution::brain_genotype::{ArchSpec, BrainGenotype};
use anima_engine_lib::evolution::genotype::MorphologyGenotype;
use bevy_ecs::prelude::*;
use glam::Vec3;

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
            AgentGenotype(MorphologyGenotype::new()),
            AgentLineageId("enqueue-atomicity".to_owned()),
            AgentGeneration(7),
            brain,
            ChildrenLinks(vec![child]),
        ))
        .id();
    (agent, child)
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
    assert_ne!(diagnostics.snapshot(), Default::default());

    diagnostics.reset();

    assert_eq!(diagnostics.snapshot(), Default::default());
}
