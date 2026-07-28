use std::sync::{Arc, RwLock};

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecs::{
    check_migration_boundaries_system, manual_migration_system, Agent, AgentBrain,
    BevyMigrationTrigger, ChildrenLinks, MapBounds, OutboundMigration, OutboundMigrationSender,
    Position, Prey, ShardingConfig, ShardingResource, Velocity,
};
use anima_engine_lib::core::engine::{AgentGeneration, AgentGenotype, AgentLineageId};
use anima_engine_lib::core::resources::SimRng;
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

fn insert_migration_resources(world: &mut World) -> crossbeam_channel::Receiver<OutboundMigration> {
    world.insert_resource(MapBounds {
        min: Vec3::new(-100.0, -10.0, -100.0),
        max: Vec3::new(100.0, 10.0, 100.0),
    });
    world.insert_resource(ShardingResource(Arc::new(RwLock::new(ShardingConfig {
        local_port: 8080,
        right_target_port: Some(8081),
        left_target_port: Some(8079),
    }))));

    let (outbound_tx, outbound_rx) = crossbeam_channel::unbounded();
    world.insert_resource(OutboundMigrationSender(outbound_tx));
    outbound_rx
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
}
