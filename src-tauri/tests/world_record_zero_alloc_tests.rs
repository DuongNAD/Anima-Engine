//! WR2 hot-loop zero-allocation gate (`C9` / ADR-0008 H2).
//!
//! Uses a single `#[test]` entry point with `ThreadTrackingAllocator::start_tracking`
//! per `tests/common/allocator.rs` so libtest sibling threads and background writer allocations
//! are excluded honestly while every allocation performed on the simulation tick thread (quiet
//! ticks, event/input ticks, segment-seal ticks, and tiny-buffer overflow ticks) is measured.

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::{AgentEvaluation, AgentGeneration, AgentLineageId};
use anima_engine_lib::core::causal::CAUSE_OBSERVER;
use anima_engine_lib::core::components::{
    Agent, AgentParentLineageIds, CombatEvent, Food, Predator, Prey,
};
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField, SeasonClock};
use anima_engine_lib::core::ecs::Position;
use anima_engine_lib::core::intervention::{Curve, InterventionCommand, InterventionKind, Region};
use anima_engine_lib::core::live_experiment::LiveInterventions;
use anima_engine_lib::core::observer::{ObserverAction, ObserverActionRecord, ObserverTrace};
use anima_engine_lib::core::resources::{CombatEvents, EpochManager, SimRng, VitalStatistics};
use anima_engine_lib::core::world_record::{
    record_world_record_tick_system, verify_record, WorldRecordRecorder, WorldRecordWriterConfig,
};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ExecutorKind;
use glam::Vec3;

#[global_allocator]
static ALLOCATOR: common::allocator::ThreadTrackingAllocator =
    common::allocator::ThreadTrackingAllocator::new();

fn homeo(energy: f32) -> HomeostaticState {
    HomeostaticState {
        energy,
        energy_target: 100.0,
        hydration: 90.0,
        hydration_target: 100.0,
        temperature: 37.0,
        temp_target: 37.0,
        previous_deviation: 0.0,
        starving_ticks: 0,
        dehydrating_ticks: Some(0),
    }
}

fn eval_at(pos: Vec3, survival_ticks: u32) -> AgentEvaluation {
    AgentEvaluation {
        start_position: pos,
        total_distance: 0.0,
        total_energy_expended: 0.0,
        total_energy_gained: 0.0,
        prey_energy_gained: 0.0,
        survival_ticks,
        last_position: pos,
    }
}

fn spawn_agent(
    world: &mut World,
    lineage: &str,
    generation: u32,
    pos: Vec3,
    is_predator: bool,
    parent: Option<&str>,
) -> Entity {
    let mut parents = Vec::new();
    if let Some(p) = parent {
        parents.push(p.to_string());
    }
    let mut e = world.spawn((
        Agent,
        homeo(65.0),
        Position(pos),
        AgentLineageId(lineage.to_string()),
        AgentGeneration(generation),
        AgentParentLineageIds(parents),
        eval_at(pos, 0),
    ));
    if is_predator {
        e.insert(Predator);
    } else {
        e.insert(Prey);
    }
    e.id()
}

fn setup_world(
    seed: u64,
    segment_ticks: u64,
    writer_config: WorldRecordWriterConfig,
) -> (World, Schedule, Entity, Entity) {
    let mut world = World::new();
    world.insert_resource(ResourceField::from_biomes(
        &vec![0u8; 256],
        16,
        16,
        0.0,
        0.0,
        16.0,
        16.0,
        0.1,
    ));
    world.insert_resource(EcosystemBiomass {
        plants: 100.0,
        animals: 130.0,
        dormant: 0.0,
        detritus: 12.5,
    });
    world.insert_resource(SimRng::from_seed(seed));
    world.insert_resource(SeasonClock::default());
    world.insert_resource(VitalStatistics::default());
    world.insert_resource(EpochManager {
        ticks_per_epoch: 1_000,
        current_epoch_ticks: 0,
        current_epoch: 0,
    });
    world.insert_resource(CombatEvents {
        events: Vec::with_capacity(64),
        predator_centroids: Vec::new(),
        prey_centroids: Vec::new(),
    });
    world.insert_resource(LiveInterventions::with_capacity(16));
    world.insert_resource(ObserverTrace::with_capacity(256));

    let founder = spawn_agent(
        &mut world,
        "live-founder-0000",
        0,
        Vec3::new(1.0, 0.0, 1.0),
        true,
        None,
    );
    let victim = spawn_agent(
        &mut world,
        "live-founder-0001",
        0,
        Vec3::new(2.0, 0.0, 2.0),
        false,
        None,
    );
    world.spawn((
        Food {
            energy_value: 20.0,
            hydration_value: 10.0,
        },
        Position(Vec3::new(3.0, 0.0, 3.0)),
    ));

    let recorder = WorldRecordRecorder::from_world_with_writer(
        &mut world,
        seed,
        segment_ticks,
        true,
        vec![("live.founders".to_string(), 2.0)],
        vec!["gpu_microstate".to_string()],
        writer_config,
    )
    .expect("writer recorder builds");
    world.insert_resource(recorder);

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(ExecutorKind::SingleThreaded);
    schedule.add_systems(record_world_record_tick_system);
    (world, schedule, founder, victim)
}

#[test]
fn wr2_hot_loop_recording_allocates_zero_bytes_across_all_phases() {
    let (quiet_allocs, active_allocs, seal_allocs, overflow_allocs, control_allocs) = {
        // ---- Phase 1-3: Steady, active events+inputs, and segment-seal ticks ----
        let (mut world, mut schedule, founder, victim) =
            setup_world(810_001, 20, WorldRecordWriterConfig::default());

        // Warm-up tick 1 primes Bevy schedule caches cleanly before measurement.
        schedule.run(&mut world);

        // Phase 1: Quiet steady-state ticks 2..=10.
        ALLOCATOR.start_tracking();
        for _ in 2..=10 {
            schedule.run(&mut world);
        }
        let quiet_allocs = ALLOCATOR.stop_tracking();

        // Phase 2: Tick 11 with death, birth, combat kill, epoch advance, intervention, and observer action.
        world.despawn(victim);
        let _child = spawn_agent(
            &mut world,
            "birth-1",
            1,
            Vec3::new(1.5, 0.0, 1.5),
            false,
            Some("live-founder-0000"),
        );
        {
            let mut vitals = world.resource_mut::<VitalStatistics>();
            vitals.total_deaths += 1;
            vitals.total_deaths_starvation += 1;
            vitals.total_births += 1;
        }
        world.resource_mut::<EpochManager>().current_epoch = 1;
        world
            .resource_mut::<CombatEvents>()
            .events
            .push(CombatEvent {
                predator_id: founder.index(),
                prey_id: victim.index(),
                damage: 14.5,
                energy_transferred: 8.25,
            });
        {
            let mut interventions = world.resource_mut::<LiveInterventions>();
            interventions.tick = 11;
            interventions.active.push(InterventionCommand {
                id: 7,
                cause_id: 1,
                kind: InterventionKind::RainfallDelta,
                region: Region::Global,
                start_tick: 11,
                duration_ticks: 5,
                intensity: 0.25,
                signed_negative: false,
                curve: Curve::Step,
                reversible: true,
            });
        }
        world
            .resource_mut::<ObserverTrace>()
            .record_action(ObserverActionRecord {
                tick: 11,
                action: ObserverAction::EvolutionToggled { running: false },
                cause_id: CAUSE_OBSERVER,
            });

        ALLOCATOR.start_tracking();
        schedule.run(&mut world);
        let active_allocs = ALLOCATOR.stop_tracking();

        world.resource_mut::<LiveInterventions>().active.clear();
        world.resource_mut::<CombatEvents>().events.clear();

        // Phase 3: Advance ticks 12..=19, inject more events right on seal ticks 20 and 40.
        for _ in 12..=19 {
            schedule.run(&mut world);
        }
        ALLOCATOR.start_tracking();
        schedule.run(&mut world); // Tick 20 -> Segment 0 seal
        let seal0_allocs = ALLOCATOR.stop_tracking();

        for _ in 21..=39 {
            schedule.run(&mut world);
        }
        let _child2 = spawn_agent(
            &mut world,
            "birth-2",
            1,
            Vec3::new(2.5, 0.0, 2.5),
            false,
            Some("live-founder-0000"),
        );
        world.resource_mut::<VitalStatistics>().total_births += 1;
        world
            .resource_mut::<ObserverTrace>()
            .record_action(ObserverActionRecord {
                tick: 40,
                action: ObserverAction::EvolutionSettingsChanged {
                    mutation_rate: 0.05,
                    selection_bias: 1.5,
                    grid_resolution: 50,
                },
                cause_id: CAUSE_OBSERVER,
            });

        ALLOCATOR.start_tracking();
        schedule.run(&mut world); // Tick 40 -> Birth + Observer + Segment 1 seal
        let seal1_allocs = ALLOCATOR.stop_tracking();

        let record = world
            .resource_mut::<WorldRecordRecorder>()
            .finish_writer()
            .expect("writer finishes cleanly");
        assert_eq!(world.resource::<WorldRecordRecorder>().dropped_inputs(), 0);
        assert_eq!(world.resource::<WorldRecordRecorder>().dropped_events(), 0);
        let report = verify_record(&record).expect("async record verifies");
        assert_eq!(report.segment_count, 2);
        assert_eq!(report.first_tick, Some(1));
        assert_eq!(report.last_tick, Some(40));
        assert_eq!(report.total_inputs, 3);
        assert!(report.total_events >= 4);
        assert!(report.is_complete());
        assert!(report.replayable);

        // ---- Phase 4: Tiny buffer (cap=4, paused writer) overflows without blocking or allocating ----
        let paused = Arc::new(AtomicBool::new(true));
        let tiny_cfg = WorldRecordWriterConfig {
            ring_capacity: 4,
            output_dir: None,
            paused: Some(Arc::clone(&paused)),
            ..Default::default()
        };
        let (mut tiny_world, mut tiny_sched, _f2, _v2) = setup_world(810_002, 200, tiny_cfg);
        tiny_sched.run(&mut tiny_world); // Warm-up tick 1

        let mut overflow_allocs = 0usize;
        for t in 2..=35u64 {
            let child_e = spawn_agent(
                &mut tiny_world,
                "overflow-agent",
                1,
                Vec3::new(t as f32, 0.0, 1.0),
                false,
                None,
            );
            tiny_world.resource_mut::<VitalStatistics>().total_births += 1;
            tiny_world
                .resource_mut::<ObserverTrace>()
                .record_action(ObserverActionRecord {
                    tick: t,
                    action: ObserverAction::EvolutionToggled {
                        running: t % 2 == 0,
                    },
                    cause_id: CAUSE_OBSERVER,
                });

            ALLOCATOR.start_tracking();
            tiny_sched.run(&mut tiny_world);
            overflow_allocs += ALLOCATOR.stop_tracking();

            if t % 3 == 0 {
                tiny_world.despawn(child_e);
                tiny_world.resource_mut::<VitalStatistics>().total_deaths += 1;
            }
        }

        let (di, de) = {
            let rec = tiny_world.resource::<WorldRecordRecorder>();
            (rec.dropped_inputs(), rec.dropped_events())
        };
        assert!(di > 0, "tiny ring buffer must count dropped inputs (got 0)");
        assert!(de > 0, "tiny ring buffer must count dropped events (got 0)");

        paused.store(false, Ordering::Release);
        let tiny_rec = tiny_world
            .resource_scope::<WorldRecordRecorder, _>(|world, mut rec| {
                rec.flush_and_finish_writer(world)
            })
            .expect("tiny writer flushes");
        let tiny_report = verify_record(&tiny_rec).expect("overflowed chain still verifies");
        assert!(!tiny_report.is_complete());
        assert!(!tiny_report.replayable);
        assert_eq!(tiny_report.total_dropped_inputs, di);
        assert_eq!(tiny_report.total_dropped_events, de);

        // ---- Phase 5: Allocator self-control ----
        ALLOCATOR.start_tracking();
        std::hint::black_box(Vec::<u8>::with_capacity(64));
        let control_allocs = ALLOCATOR.stop_tracking();

        (
            quiet_allocs,
            active_allocs,
            seal0_allocs + seal1_allocs,
            overflow_allocs,
            control_allocs,
        )
    };

    assert_eq!(
        (quiet_allocs, active_allocs, seal_allocs, overflow_allocs),
        (0, 0, 0, 0),
        "WR2 tick path allocated: quiet={quiet_allocs}, active_events={active_allocs}, \
         seal={seal_allocs}, tiny_overflow={overflow_allocs}"
    );
    assert!(
        control_allocs >= 1,
        "ThreadTrackingAllocator self-control must see Vec allocation (got {control_allocs})"
    );
}
