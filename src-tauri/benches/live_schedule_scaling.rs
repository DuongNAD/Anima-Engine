//! Card M3 — Headless tick cost scaling across population sizes on the real live schedule.
//!
//! Runs [`LiveExperimentAdapter`] (which drives
//! [`anima_engine_lib::core::simulation_schedule::build_tick_schedule`]) rather than a hand-rolled
//! `.chain()` subset, with `live.mortality = 0.0` and `live.reproduction = 0.0` so the live agent
//! count stays fixed across every warm-up tick and every Criterion sample.
//!
//! Numbers produced here are a **headless lower bound**, NOT desktop app frame times (no Tauri
//! window, no WebGL renderer, no IPC serialization, and no background learner/evolution threads).

use std::time::Duration;

use bevy_ecs::prelude::World;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use anima_engine_lib::core::agent_systems::{
    AgentEvaluation, AgentGeneration, AgentGenotype, AgentLineageId,
};
use anima_engine_lib::core::components::{AgentParentLineageIds, FeatureTracker, Predator, Prey};
use anima_engine_lib::core::ecs::MapBounds;
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::LiveExperimentAdapter;
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};

const SEED: u64 = 20_261_118;
const WORLD_WARMUP_TICKS: usize = 20;
const MANIFEST_FOUNDERS_CAP: usize = 4_096;
const PREDATOR_FRACTION: f64 = 0.30;
const AGENT_COUNTS: [usize; 5] = [10, 100, 1_000, 4_096, 5_000];

fn default_morph_genotype() -> MorphologyGenotype {
    let mut genotype = MorphologyGenotype::new();
    for (id, mass) in [(0u32, 1.5f32), (1, 1.0), (2, 0.8)] {
        genotype.add_node(MorphologyNode {
            id,
            length: 1.0,
            radius: 0.2,
            mass,
        });
    }
    for (source_node, target_node) in [(0u32, 1u32), (1, 2)] {
        genotype.add_edge(MorphologyEdge {
            source_node,
            target_node,
            joint_anchor: glam::Vec3::new(1.0, 0.0, 0.0),
            joint_axis: glam::Vec3::new(0.0, 0.0, 1.0),
        });
    }
    genotype
}

/// Spawn founders `MANIFEST_FOUNDERS_CAP..target_count` prior to tick 1 when benchmarking above
/// `live.founders`'s declared manifest ceiling (`4096.0`), matching `live_experiment::genesis`
/// components and exact predator fraction so the live schedule itself remains unmodified.
fn spawn_overflow_founders(
    world: &mut World,
    existing_founders: usize,
    existing_predators: usize,
    target_count: usize,
) {
    if target_count <= existing_founders {
        return;
    }
    let bounds = *world.resource::<MapBounds>();
    let span_x = bounds.max.x - bounds.min.x;
    let span_z = bounds.max.z - bounds.min.z;
    let target_predators = ((target_count as f64) * PREDATOR_FRACTION).round() as usize;
    let extra_needed = target_count - existing_founders;
    let extra_predators = target_predators.saturating_sub(existing_predators);
    let genotype = default_morph_genotype();

    for offset in 0..extra_needed {
        let global_idx = existing_founders + offset;
        let t = (offset as f32 + 1.0) / (extra_needed as f32 + 1.0);
        let pos = glam::Vec3::new(bounds.min.x + t * span_x, 0.0, bounds.min.z + 0.45 * span_z);
        let entity = decode_genotype(world, &genotype, pos, glam::Quat::IDENTITY);
        world.entity_mut(entity).insert((
            AgentGenotype(genotype.clone()),
            AgentEvaluation {
                start_position: pos,
                total_distance: 0.0,
                total_energy_expended: 0.0,
                total_energy_gained: 0.0,
                prey_energy_gained: 0.0,
                survival_ticks: 0,
                last_position: pos,
            },
            FeatureTracker::default(),
            AgentLineageId(format!("live-founder-{global_idx:04}")),
            AgentGeneration(0),
            AgentParentLineageIds(Vec::new()),
        ));
        if offset < extra_predators {
            world.entity_mut(entity).insert(Predator);
        } else {
            world.entity_mut(entity).insert(Prey);
        }
    }
}

fn build_scaling_adapter(target_count: usize) -> LiveExperimentAdapter {
    let manifest_founders = target_count.min(MANIFEST_FOUNDERS_CAP);
    let initial = InitialConditionSet::new(vec![
        ("live.founders".to_string(), manifest_founders as f64),
        ("live.predator_fraction".to_string(), PREDATOR_FRACTION),
        ("live.trees".to_string(), 8.0),
        ("live.lakes".to_string(), 2.0),
        ("live.food_cap".to_string(), 50.0),
        ("live.mortality".to_string(), 0.0),
        ("live.reproduction".to_string(), 0.0),
        ("live.locomotion".to_string(), 1.0),
        ("live.drive_floor".to_string(), 0.5),
        ("live.thirst_steering".to_string(), 1.0),
    ]);

    let mut adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        SEED,
        (256, 256),
        100_000,
    )
    .expect("M3 scaling manifest builds cleanly");

    if target_count > manifest_founders {
        let existing_predators = adapter.config().predators();
        let mut world = adapter.world();
        spawn_overflow_founders(
            &mut world,
            manifest_founders,
            existing_predators,
            target_count,
        );
    }

    for _ in 0..WORLD_WARMUP_TICKS {
        adapter.run_schedule_once();
    }
    adapter
}

fn live_agent_count(adapter: &LiveExperimentAdapter) -> f64 {
    ExperimentModel::observables_unsampled(adapter)
        .into_iter()
        .find(|(k, _)| k == "live.agent_count")
        .map(|(_, v)| v)
        .expect("live.agent_count observable present")
}

fn bench_live_schedule_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("live_schedule_scaling");
    group.warm_up_time(Duration::from_secs(3));

    for &count in &AGENT_COUNTS {
        let mut adapter = build_scaling_adapter(count);
        assert_eq!(
            live_agent_count(&adapter),
            count as f64,
            "agent count must match target prior to benchmark for N={count}"
        );

        if count <= 1_000 {
            group.sample_size(50);
            group.measurement_time(Duration::from_secs(8));
        } else {
            group.sample_size(20);
            group.measurement_time(Duration::from_secs(12));
        }

        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                adapter.run_schedule_once();
                black_box(adapter.tick());
            });
        });

        assert_eq!(
            live_agent_count(&adapter),
            count as f64,
            "agent count drifted during measurement for N={count} (mortality=0, reproduction=0)"
        );
    }

    group.finish();
}

criterion_group!(benches, bench_live_schedule_scaling);
criterion_main!(benches);
