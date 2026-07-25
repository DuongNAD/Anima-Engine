//! Tier two of simulation LOD: distant individuals become per-chunk statistics, and come back as
//! individuals when an observer returns.
//!
//! Tier one ([`simulation_lod`](anima_engine_lib::core::simulation_lod)) skips inference for distant
//! agents but leaves them resident, so it buys CPU and not memory. This tier destroys the body. That
//! makes two things load-bearing, and they are what this file holds:
//!
//! 1. **Energy is conserved.** A dormant animal's reserve has not been eaten or respired; it is
//!    still animal energy, held in a chunk instead of a body. If `ecosystem_census_system` fails to
//!    count it, every dehydration is an EU leak and the closed-energy gate that
//!    `SIMULATION_RULES.md` declares stops being true.
//! 2. **Dormancy is not a trapdoor.** An agent must be genuinely, persistently out of range before
//!    it is destroyed, and everything needed to rebuild it must survive.
//!
//! Both defaults are off — no `DormantCohorts` resource, or no enabled focus, and nothing here
//! happens at all.

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::{AgentGenotype, AgentLineageId};
use anima_engine_lib::core::aggregate_population::{
    dehydrate_cold_agents_system, dormant_cohort_ecology_system, rehydrate_wakeable_chunks_system,
    DormancyWatch, DormantCohorts, ARCHIVE_CAP,
};
use anima_engine_lib::core::components::AgentBrain;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::ecs::{init_world, Agent, FeatureTracker, MapBounds, Prey};
use anima_engine_lib::core::energy_ledger::closed_total_eu;
use anima_engine_lib::core::environmental_systems::{
    ecosystem_census_system, herbivore_grazing_system, resource_field_regrowth_system,
};
use anima_engine_lib::core::simulation_lod::{LodBands, LodFocus};
use anima_engine_lib::core::world_systems::metabolic_decay_system;
use anima_engine_lib::evolution::brain_genotype::{BrainGenotype, EVOLVED_ARCH};
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

/// EU tolerance for a whole dormancy cycle.
///
/// Deliberately tighter than `RESIDUAL_ABS_TOLERANCE_EU` (1e-3), which budgets for a multi-million
/// tick run's census widening. Dormancy moves energy between an `f32` reserve and an `f64` pool a
/// handful of times per agent, and every move is measured rather than assumed, so anything above
/// this is a real leak and not accumulated rounding.
const TOL: f64 = 1e-9;

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

/// Closed EU including the dormant pool.
///
/// Summed from the authoritative stores, not the `EcosystemBiomass` mirrors, so a census that
/// forgot the dormant cohorts shows up here as a mismatch instead of being hidden by the mirror it
/// wrote itself.
fn closed_total(world: &mut World) -> f64 {
    let plants = world
        .get_resource::<ResourceField>()
        .map(|f| f.total_biomass())
        .unwrap_or(0.0);
    let detritus = world
        .get_resource::<EcosystemBiomass>()
        .map(|p| p.detritus)
        .unwrap_or(0.0);
    let dormant = world
        .get_resource::<DormantCohorts>()
        .map(|c| c.total_energy())
        .unwrap_or(0.0);
    let mut animals = 0.0f64;
    let mut q = world.query_filtered::<&HomeostaticState, With<Agent>>();
    for homeo in q.iter(world) {
        animals += homeo.energy.max(0.0) as f64;
    }
    closed_total_eu(plants, animals + dormant, detritus)
}

fn live_agents(world: &mut World) -> usize {
    let mut q = world.query_filtered::<Entity, With<Agent>>();
    q.iter(world).count()
}

/// Heap held by the brains of living agents.
fn live_brain_bytes(world: &mut World) -> usize {
    let mut q = world.query::<&AgentBrain>();
    q.iter(world).map(|b| b.heap_bytes()).sum()
}

/// A world with `n` prey clustered at `spawn`, each with a brain and a full reserve.
///
/// `AgentGenotype` is inserted explicitly because `decode_genotype` does not — it is the component
/// dehydration needs in order to rebuild the body later, and an agent without one is deliberately
/// never dehydrated.
fn build_world(n: usize, spawn: glam::Vec3, brains: bool) -> World {
    build_spaced_world(n, spawn, 0.5, brains)
}

/// As [`build_world`], with control over how far apart the herd stands.
///
/// A chunk is 200/32 = 6.25 units wide, so a spacing small enough to keep the whole herd inside one
/// chunk is what the cohort-level tests need.
fn build_spaced_world(n: usize, spawn: glam::Vec3, spacing: f32, brains: bool) -> World {
    let mut world = init_world();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(EcosystemBiomass {
        detritus: 500.0,
        plants: 0.0,
        animals: 0.0,
    });

    let genotype = test_genotype();
    let mut rng = rand::rngs::StdRng::from_seed([7u8; 32]);
    for i in 0..n {
        let pos = spawn + glam::Vec3::new(i as f32 * spacing, 0.0, 0.0);
        let e = decode_genotype(&mut world, &genotype, pos, glam::Quat::IDENTITY);
        world.entity_mut(e).insert((
            Prey,
            AgentGenotype(genotype.clone()),
            AgentLineageId(format!("lin-{i}")),
            // Every live spawn path attaches one (genesis, `SpawnGenotypeCommand`, restore), and
            // dormancy reads its per-tick metabolic burn off it. A fixture without one produces
            // cohorts that never respire, which is not how a real agent goes to sleep.
            FeatureTracker::default(),
        ));
        if brains {
            let brain = BrainGenotype::random(EVOLVED_ARCH, &mut rng).unwrap();
            world.entity_mut(e).insert(AgentBrain::from_genotype(brain));
        }
    }
    world
}

use rand::SeedableRng;

/// Switch dormancy on with a short dwell, and put the focus somewhere the agents are not.
fn enable_dormancy(world: &mut World, dwell: u32, focus_at: Option<glam::Vec3>) {
    let bounds = *world.resource::<MapBounds>();
    let mut cohorts = DormantCohorts::from_bounds(1234, &bounds);
    cohorts.dwell_ticks = dwell;
    cohorts.rehydrate_per_tick = 8;
    world.insert_resource(cohorts);
    world.insert_resource(LodBands {
        hot_radius: 5.0,
        warm_radius: 10.0,
        warm_interval: 4,
    });
    world.insert_resource(match focus_at {
        Some(c) => LodFocus::at(c),
        None => LodFocus::default(),
    });
}

fn dormancy_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            dehydrate_cold_agents_system,
            rehydrate_wakeable_chunks_system,
            ecosystem_census_system,
        )
            .chain(),
    );
    schedule
}

fn run(world: &mut World, schedule: &mut Schedule, ticks: usize) {
    for _ in 0..ticks {
        schedule.run(world);
    }
}

// ---- Conservation ----------------------------------------------------------------------

#[test]
fn dehydration_moves_energy_without_creating_or_destroying_any() {
    let spawn = glam::Vec3::new(0.0, 0.0, 0.0);
    let mut world = build_world(6, spawn, true);
    // Focus far away, so every agent is Cold from the first tick.
    enable_dormancy(&mut world, 3, Some(glam::Vec3::new(80.0, 0.0, 80.0)));

    let before = closed_total(&mut world);
    assert!(before > 0.0, "the fixture must actually hold energy");

    let mut schedule = dormancy_schedule();
    run(&mut world, &mut schedule, 12);

    assert_eq!(live_agents(&mut world), 0, "every agent should be dormant");
    assert_eq!(world.resource::<DormantCohorts>().total_dormant(), 6);

    let after = closed_total(&mut world);
    assert!(
        (after - before).abs() < TOL,
        "dehydration changed closed EU by {} (before {before}, after {after})",
        after - before
    );
}

#[test]
fn a_full_sleep_and_wake_cycle_conserves_energy() {
    let spawn = glam::Vec3::new(0.0, 0.0, 0.0);
    let mut world = build_world(6, spawn, true);
    enable_dormancy(&mut world, 3, Some(glam::Vec3::new(80.0, 0.0, 80.0)));
    let before = closed_total(&mut world);

    let mut schedule = dormancy_schedule();
    run(&mut world, &mut schedule, 12);
    assert_eq!(live_agents(&mut world), 0);

    // The observer walks back to where the herd went to sleep.
    world.insert_resource(LodFocus::at(spawn));
    // A generous hot radius so the chunk centre is reachable regardless of where the herd's
    // positions fell inside their chunk.
    world.insert_resource(LodBands {
        hot_radius: 40.0,
        warm_radius: 60.0,
        warm_interval: 4,
    });
    run(&mut world, &mut schedule, 20);

    assert!(
        live_agents(&mut world) > 0,
        "the herd should have come back as bodies"
    );
    let after = closed_total(&mut world);
    assert!(
        (after - before).abs() < TOL,
        "a sleep/wake cycle changed closed EU by {} (before {before}, after {after})",
        after - before
    );
}

#[test]
fn the_census_mirror_agrees_with_the_authoritative_stores_while_agents_are_dormant() {
    // The failure this catches: a census that sums only live agents. Everything still runs, the
    // numbers stay finite, and the world quietly loses one reserve per dehydration.
    let mut world = build_world(4, glam::Vec3::ZERO, false);
    enable_dormancy(&mut world, 2, Some(glam::Vec3::new(80.0, 0.0, 80.0)));

    let mut schedule = dormancy_schedule();
    run(&mut world, &mut schedule, 10);

    let dormant = world.resource::<DormantCohorts>().total_energy();
    assert!(
        dormant > 0.0,
        "the cohorts should be holding the energy now"
    );
    let mirrored = world.resource::<EcosystemBiomass>().animals;
    assert!(
        (mirrored - dormant).abs() < TOL,
        "census reported {mirrored} EU of animals while {dormant} EU slept in cohorts"
    );
}

// ---- Default off -----------------------------------------------------------------------

#[test]
fn without_the_cohorts_resource_nothing_is_ever_dehydrated() {
    let mut world = build_world(5, glam::Vec3::ZERO, true);
    // Focus enabled and far away — the LOD tier would call every agent Cold — but no cohorts.
    world.insert_resource(LodBands {
        hot_radius: 5.0,
        warm_radius: 10.0,
        warm_interval: 4,
    });
    world.insert_resource(LodFocus::at(glam::Vec3::new(80.0, 0.0, 80.0)));

    let mut schedule = dormancy_schedule();
    run(&mut world, &mut schedule, 50);

    assert_eq!(
        live_agents(&mut world),
        5,
        "the aggregate tier must be inert without its resource"
    );
}

#[test]
fn a_disabled_focus_never_dehydrates_anything() {
    // The second, independent off-switch: cohorts present, but no observer. A disabled focus tiers
    // everything Hot, which is what every headless run does today.
    let mut world = build_world(5, glam::Vec3::ZERO, true);
    enable_dormancy(&mut world, 2, None);

    let mut schedule = dormancy_schedule();
    run(&mut world, &mut schedule, 50);

    assert_eq!(live_agents(&mut world), 5);
    assert_eq!(world.resource::<DormantCohorts>().dehydrated(), 0);
}

// ---- Hysteresis ------------------------------------------------------------------------

#[test]
fn an_agent_is_not_dehydrated_before_the_dwell_elapses() {
    let mut world = build_world(3, glam::Vec3::ZERO, true);
    enable_dormancy(&mut world, 30, Some(glam::Vec3::new(80.0, 0.0, 80.0)));

    let mut schedule = dormancy_schedule();
    run(&mut world, &mut schedule, 10);
    assert_eq!(
        live_agents(&mut world),
        3,
        "10 cold ticks is well short of a 30-tick dwell"
    );

    run(&mut world, &mut schedule, 30);
    assert_eq!(live_agents(&mut world), 0, "past the dwell, they sleep");
}

#[test]
fn an_agent_that_keeps_dipping_back_into_range_never_sleeps() {
    // The property the dwell exists for. Without a reset, an agent oscillating across the boundary
    // accumulates cold ticks and eventually loses its brain — an observer panning a camera would
    // be driving evolution.
    let spawn = glam::Vec3::ZERO;
    let mut world = build_world(3, spawn, true);
    enable_dormancy(&mut world, 10, Some(glam::Vec3::new(80.0, 0.0, 80.0)));

    let mut schedule = dormancy_schedule();
    for _ in 0..20 {
        // Nine ticks cold — one short of the dwell — then one tick back in range.
        world.insert_resource(LodFocus::at(glam::Vec3::new(80.0, 0.0, 80.0)));
        run(&mut world, &mut schedule, 9);
        world.insert_resource(LodFocus::at(spawn));
        run(&mut world, &mut schedule, 1);
    }

    assert_eq!(
        live_agents(&mut world),
        3,
        "a flickering agent accumulated dwell across warm ticks"
    );
    let mut q = world.query::<&DormancyWatch>();
    assert_eq!(
        q.iter(&world).count(),
        0,
        "the watch component must be removed when an agent warms, not merely paused"
    );
}

// ---- What survives, and what does not ---------------------------------------------------

#[test]
fn below_the_cap_the_saving_is_the_body_and_the_learned_network_not_the_genome() {
    // The claim "tier two reclaims memory" is conditional, and this is the condition. A cohort at
    // or below ARCHIVE_CAP keeps *every* genome — that is what makes the round trip lossless — so
    // the genome memory is not reclaimed at all. What goes is the ECS body and the `learned`
    // network, which is a second full copy of the weights.
    let mut world = build_world(4, glam::Vec3::ZERO, true);
    {
        let mut rng = rand::rngs::StdRng::from_seed([3u8; 32]);
        let learned = std::sync::Arc::new(BrainGenotype::random(EVOLVED_ARCH, &mut rng).unwrap());
        let entities: Vec<Entity> = {
            let mut q = world.query_filtered::<Entity, With<AgentBrain>>();
            q.iter(&world).collect()
        };
        for e in entities {
            world.get_mut::<AgentBrain>(e).unwrap().learned = Some(learned.clone());
        }
    }
    enable_dormancy(&mut world, 2, Some(glam::Vec3::new(80.0, 0.0, 80.0)));

    let resident_before = live_brain_bytes(&mut world);
    let mut schedule = dormancy_schedule();
    run(&mut world, &mut schedule, 10);

    assert_eq!(
        live_brain_bytes(&mut world),
        0,
        "no brain may stay resident"
    );
    let archived = world.resource::<DormantCohorts>().archive_heap_bytes();
    assert!(
        archived < resident_before,
        "dropping `learned` should roughly halve it: archive {archived} B vs resident \
         {resident_before} B"
    );
    // And the honest half of the statement: the genomes are all still there, so this is a factor
    // of two, not the factor that makes millions of agents possible.
    assert!(
        archived > resident_before / 3,
        "a cohort under the cap keeps every genome, so it cannot be much smaller: {archived} B \
         against {resident_before} B"
    );
}

#[test]
fn above_the_cap_the_archive_stops_growing_with_the_population() {
    // This is the scaling claim, and the only place the memory ceiling actually moves. A chunk's
    // archive is bounded by ARCHIVE_CAP no matter how many individuals sleep in it, so dormant
    // memory is O(chunks) rather than O(agents).
    let n = 60;
    // Tight spacing so the whole herd falls in one chunk.
    let mut world = build_spaced_world(n, glam::Vec3::ZERO, 0.01, true);
    enable_dormancy(&mut world, 2, Some(glam::Vec3::new(80.0, 0.0, 80.0)));

    let resident_before = live_brain_bytes(&mut world);
    let mut schedule = dormancy_schedule();
    run(&mut world, &mut schedule, 12);
    assert_eq!(
        live_agents(&mut world),
        0,
        "the whole herd should be asleep"
    );

    let cohorts = world.resource::<DormantCohorts>();
    assert_eq!(cohorts.total_dormant(), n as u64);
    let archived = cohorts.archive_heap_bytes();
    assert!(
        archived * 4 < resident_before,
        "60 sleeping agents should cost far less than 60 resident ones: archive {archived} B \
         against {resident_before} B"
    );
    // The bound is the cap, not a ratio that happens to hold at this population size.
    let per_genome = resident_before / n;
    assert!(
        archived < per_genome * (ARCHIVE_CAP + 2),
        "the archive ({archived} B) should be bounded by ARCHIVE_CAP genomes (~{} B), not by the \
         population",
        per_genome * ARCHIVE_CAP
    );
}

#[test]
fn a_rehydrated_agent_carries_an_archived_brain_rather_than_a_fresh_one() {
    // ADR-0003 invariant D01: restore paths carry the brain they were given. A re-hydrated
    // individual may be a clone of a different archived one once a cohort exceeds ARCHIVE_CAP, but
    // it is never freshly random — otherwise dormancy would silently reset a lineage's evolution.
    let spawn = glam::Vec3::ZERO;
    let mut world = build_world(3, spawn, true);
    enable_dormancy(&mut world, 2, Some(glam::Vec3::new(80.0, 0.0, 80.0)));

    let mut original: Vec<Vec<f32>> = {
        let mut q = world.query::<&AgentBrain>();
        q.iter(&world).map(|b| b.genotype.weights.clone()).collect()
    };
    original.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());

    let mut schedule = dormancy_schedule();
    run(&mut world, &mut schedule, 10);
    assert_eq!(live_agents(&mut world), 0);

    world.insert_resource(LodFocus::at(spawn));
    world.insert_resource(LodBands {
        hot_radius: 40.0,
        warm_radius: 60.0,
        warm_interval: 4,
    });
    run(&mut world, &mut schedule, 20);

    let mut back: Vec<Vec<f32>> = {
        let mut q = world.query::<&AgentBrain>();
        q.iter(&world).map(|b| b.genotype.weights.clone()).collect()
    };
    assert_eq!(back.len(), 3, "all three should have come back");
    back.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());
    assert_eq!(
        back, original,
        "a cohort at or below ARCHIVE_CAP must return exactly the genomes it swallowed"
    );
}

#[test]
fn what_an_individual_learned_does_not_survive_dormancy() {
    // No Lamarck (ADR-0003 decision 2). Dormancy destroys the body, so lifetime learning goes with
    // it; carrying `learned` through the archive would make an acquired trait heritable via the
    // back door — and it is also the half of the memory this tier exists to reclaim.
    let spawn = glam::Vec3::ZERO;
    let mut world = build_world(2, spawn, true);
    {
        let mut rng = rand::rngs::StdRng::from_seed([9u8; 32]);
        let learned = std::sync::Arc::new(BrainGenotype::random(EVOLVED_ARCH, &mut rng).unwrap());
        let entities: Vec<Entity> = {
            let mut q = world.query_filtered::<Entity, With<AgentBrain>>();
            q.iter(&world).collect()
        };
        for e in entities {
            world.get_mut::<AgentBrain>(e).unwrap().learned = Some(learned.clone());
        }
    }
    enable_dormancy(&mut world, 2, Some(glam::Vec3::new(80.0, 0.0, 80.0)));

    let mut schedule = dormancy_schedule();
    run(&mut world, &mut schedule, 10);
    world.insert_resource(LodFocus::at(spawn));
    world.insert_resource(LodBands {
        hot_radius: 40.0,
        warm_radius: 60.0,
        warm_interval: 4,
    });
    run(&mut world, &mut schedule, 20);

    let mut q = world.query::<&AgentBrain>();
    let woken: Vec<&AgentBrain> = q.iter(&world).collect();
    assert!(!woken.is_empty(), "nothing woke up to check");
    for brain in woken {
        assert!(
            brain.learned.is_none(),
            "a re-hydrated individual inherited another's lifetime learning"
        );
    }
}

#[test]
fn an_agent_with_no_morphology_is_left_alone_rather_than_deleted() {
    // Dehydration is only safe because the body can be rebuilt. An agent missing `AgentGenotype`
    // cannot be, so destroying it would be a one-way deletion — of the agent and of its energy.
    let mut world = build_world(2, glam::Vec3::ZERO, true);
    let entities: Vec<Entity> = {
        let mut q = world.query_filtered::<Entity, With<Agent>>();
        q.iter(&world).collect()
    };
    for e in &entities {
        world.entity_mut(*e).remove::<AgentGenotype>();
    }
    enable_dormancy(&mut world, 2, Some(glam::Vec3::new(80.0, 0.0, 80.0)));

    let before = closed_total(&mut world);
    let mut schedule = dormancy_schedule();
    run(&mut world, &mut schedule, 20);

    assert_eq!(live_agents(&mut world), 2, "they must still be here");
    assert_eq!(world.resource::<DormantCohorts>().dehydrated(), 0);
    assert!((closed_total(&mut world) - before).abs() < TOL);
}

#[test]
fn an_agent_outside_the_chunk_grid_is_never_destroyed() {
    // `absorb` refuses an off-grid position; the command must honour that refusal rather than
    // despawning an agent whose energy went nowhere.
    let mut world = build_world(2, glam::Vec3::ZERO, true);
    enable_dormancy(&mut world, 2, Some(glam::Vec3::new(80.0, 0.0, 80.0)));
    // Shrink the cohort grid to a corner of the world so the agents at the origin fall outside it.
    let mut cohorts = DormantCohorts::new(1234, -1000.0, -1000.0, -900.0, -900.0);
    cohorts.dwell_ticks = 2;
    world.insert_resource(cohorts);

    let before = closed_total(&mut world);
    let mut schedule = dormancy_schedule();
    run(&mut world, &mut schedule, 20);

    assert_eq!(live_agents(&mut world), 2);
    assert_eq!(world.resource::<DormantCohorts>().total_dormant(), 0);
    assert!(
        (closed_total(&mut world) - before).abs() < TOL,
        "a refused dehydration must not move any energy"
    );
}

#[test]
fn a_cohort_past_the_archive_cap_still_returns_the_right_number_of_bodies() {
    // Above the cap the genomes are sampled, but the *population* is not: everyone who went to
    // sleep must come back, and with the energy they collectively took in.
    let spawn = glam::Vec3::ZERO;
    let n = ARCHIVE_CAP + 6;
    let mut world = build_world(n, spawn, true);
    enable_dormancy(&mut world, 2, Some(glam::Vec3::new(80.0, 0.0, 80.0)));
    let before = closed_total(&mut world);

    let mut schedule = dormancy_schedule();
    run(&mut world, &mut schedule, 12);
    assert_eq!(live_agents(&mut world), 0);
    assert!(
        world.resource::<DormantCohorts>().genomes_dropped() > 0,
        "past the cap, some genomes must be reported lost"
    );

    world.insert_resource(LodFocus::at(spawn));
    world.insert_resource(LodBands {
        hot_radius: 60.0,
        warm_radius: 80.0,
        warm_interval: 4,
    });
    run(&mut world, &mut schedule, 60);

    assert_eq!(
        live_agents(&mut world),
        n,
        "the population size must survive even though individual genomes did not"
    );
    let after = closed_total(&mut world);
    assert!(
        (after - before).abs() < TOL,
        "closed EU moved by {} across a lossy cycle",
        after - before
    );
}

#[test]
fn the_wake_budget_counts_individuals_not_chunks() {
    // `rehydrate_per_tick` is documented as individuals per tick. Issuing one command per *chunk*
    // instead would still wake everyone eventually and still conserve energy, so nothing would fail
    // — a crowded chunk would just take one tick per individual while the setting said four.
    let spawn = glam::Vec3::ZERO;
    let n = 12;
    let mut world = build_spaced_world(n, spawn, 0.01, false);
    let bounds = *world.resource::<MapBounds>();
    let mut cohorts = DormantCohorts::from_bounds(1234, &bounds);
    cohorts.dwell_ticks = 2;
    cohorts.rehydrate_per_tick = 4;
    world.insert_resource(cohorts);
    world.insert_resource(LodBands {
        hot_radius: 5.0,
        warm_radius: 10.0,
        warm_interval: 4,
    });
    world.insert_resource(LodFocus::at(glam::Vec3::new(80.0, 0.0, 80.0)));

    let mut schedule = dormancy_schedule();
    run(&mut world, &mut schedule, 8);
    assert_eq!(live_agents(&mut world), 0, "the herd should be asleep");
    assert_eq!(
        world.resource::<DormantCohorts>().total_dormant(),
        n as u64,
        "and all of it in one chunk's cohort"
    );

    world.insert_resource(LodFocus::at(spawn));
    world.insert_resource(LodBands {
        hot_radius: 40.0,
        warm_radius: 60.0,
        warm_interval: 4,
    });
    // 12 individuals at 4 per tick is three ticks. A per-chunk budget would need twelve.
    run(&mut world, &mut schedule, 3);
    assert_eq!(
        live_agents(&mut world),
        n,
        "a budget of 4/tick should have woken all 12 in three ticks"
    );
}
// ---- Aggregate ecology: time passes where nobody is looking ------------------------------

/// A schedule that also runs the dormant cohorts' own ecology, in the order the engine uses:
/// dormant metabolism and grazing sit between live grazing and regrowth, so both consumers draw on
/// the same standing field before it grows back.
fn ecology_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            metabolic_decay_system,
            herbivore_grazing_system,
            dehydrate_cold_agents_system,
            rehydrate_wakeable_chunks_system,
            dormant_cohort_ecology_system,
            resource_field_regrowth_system,
            ecosystem_census_system,
        )
            .chain(),
    );
    schedule
}

/// Strip every cell of the resource field, so nothing can graze.
///
/// `init_world` always builds a field from the terrain, so a test that wants a starving population
/// has to empty it rather than simply not adding one.
fn starve_the_field(world: &mut World) {
    let mut field = world.resource_mut::<ResourceField>();
    field.r.iter_mut().for_each(|c| *c = 0.0);
    field.r_max.iter_mut().for_each(|c| *c = 0.0);
    field.growth_rate = 0.0;
}

/// A world with a real resource field under it, so grazing has something to eat.
fn with_field(world: &mut World) {
    let bounds = *world.resource::<MapBounds>();
    let side = 64;
    world.insert_resource(ResourceField {
        width: side,
        height: side,
        min_x: bounds.min.x,
        min_z: bounds.min.z,
        max_x: bounds.max.x,
        max_z: bounds.max.z,
        r: vec![4.0; side * side],
        r_max: vec![4.0; side * side],
        growth_rate: 0.0,
    });
}

#[test]
fn a_dormant_cohort_respires_into_detritus_and_conserves_the_total() {
    let mut world = build_world(4, glam::Vec3::ZERO, false);
    with_field(&mut world);
    enable_dormancy(&mut world, 2, Some(glam::Vec3::new(80.0, 0.0, 80.0)));

    let mut schedule = ecology_schedule();
    // Live ticks first, so `FeatureTracker` accumulates the burn rate dormancy will inherit.
    run(&mut world, &mut schedule, 30);

    let before = closed_total(&mut world);
    let dormant_energy_before = world.resource::<DormantCohorts>().total_energy();
    assert!(
        dormant_energy_before > 0.0,
        "the herd should be asleep by now"
    );
    let detritus_before = world.resource::<EcosystemBiomass>().detritus;

    run(&mut world, &mut schedule, 60);

    let dormant_after = world.resource::<DormantCohorts>().total_energy();
    assert!(
        dormant_after < dormant_energy_before,
        "a dormant cohort must keep burning: {dormant_energy_before} to {dormant_after}"
    );
    assert!(
        world.resource::<EcosystemBiomass>().detritus > detritus_before,
        "what it burned has to arrive in detritus"
    );
    let after = closed_total(&mut world);
    assert!(
        (after - before).abs() < TOL,
        "dormant metabolism moved closed EU by {}",
        after - before
    );
}

#[test]
fn dormant_herbivores_graze_and_the_field_loses_exactly_what_they_gain() {
    let mut world = build_world(4, glam::Vec3::ZERO, false);
    with_field(&mut world);
    enable_dormancy(&mut world, 2, Some(glam::Vec3::new(80.0, 0.0, 80.0)));

    let mut schedule = ecology_schedule();
    run(&mut world, &mut schedule, 20);
    assert_eq!(live_agents(&mut world), 0, "the herd should be asleep");

    // Starve the cohort so it is definitely hungry, then let it eat.
    let field_before = world.resource::<ResourceField>().total_biomass();
    let before = closed_total(&mut world);
    run(&mut world, &mut schedule, 120);

    let field_after = world.resource::<ResourceField>().total_biomass();
    assert!(
        field_after < field_before,
        "dormant herbivores should have grazed the field: {field_before} to {field_after}"
    );
    let after = closed_total(&mut world);
    assert!(
        (after - before).abs() < TOL,
        "dormant grazing moved closed EU by {}",
        after - before
    );
}

#[test]
fn the_plants_mirror_still_tracks_the_field_after_dormant_grazing() {
    // `plants` is carried incrementally rather than re-summed each tick, so every consumer has to
    // report what it took. A dormant grazer that ate without reporting would desynchronise the
    // mirror from the store it describes, and the census would go on looking perfectly healthy.
    let mut world = build_world(4, glam::Vec3::ZERO, false);
    with_field(&mut world);
    {
        let standing = world.resource::<ResourceField>().total_biomass();
        let mut pool = world.resource_mut::<EcosystemBiomass>();
        pool.plants = standing;
    }
    enable_dormancy(&mut world, 2, Some(glam::Vec3::new(80.0, 0.0, 80.0)));

    let mut schedule = ecology_schedule();
    run(&mut world, &mut schedule, 150);

    let standing = world.resource::<ResourceField>().total_biomass();
    let mirrored = world.resource::<EcosystemBiomass>().plants;
    assert!(
        (mirrored - standing).abs() < 1e-6,
        "plants mirror says {mirrored}, the field holds {standing}"
    );
}

#[test]
fn a_starving_dormant_cohort_keeps_its_members() {
    // The live world does not despawn an agent at zero energy, so neither may this one. Aggregate
    // mortality would be the observer's route through the world deciding who dies.
    let mut world = build_world(3, glam::Vec3::ZERO, false);
    // Strip the field bare so there is nothing to graze and the cohort can only burn down.
    // `init_world` always provides a resource field, so "no food" has to be arranged, not assumed.
    starve_the_field(&mut world);
    enable_dormancy(&mut world, 2, Some(glam::Vec3::new(80.0, 0.0, 80.0)));

    let mut schedule = ecology_schedule();
    run(&mut world, &mut schedule, 30);
    let before = closed_total(&mut world);
    // Long enough to burn the whole reserve: three agents at ~0.026 EU/tick against 300 EU takes
    // roughly 11,700 ticks, so this runs past the floor rather than up to it.
    run(&mut world, &mut schedule, 15_000);

    let cohorts = world.resource::<DormantCohorts>();
    assert_eq!(
        cohorts.total_dormant(),
        3,
        "starvation must not remove dormant individuals"
    );
    assert!(
        cohorts.total_energy() < 1e-6,
        "and they should have burned down to nothing: {}",
        cohorts.total_energy()
    );
    let after = closed_total(&mut world);
    assert!(
        (after - before).abs() < TOL,
        "burning down to zero moved closed EU by {}",
        after - before
    );
}

#[test]
fn sleeping_is_not_cheaper_than_being_watched() {
    // The artifact this whole design is arranged against. If a dormant cohort were charged a
    // modelled maintenance-only rate, unobserved regions would quietly support larger populations
    // and the observer's attention would become an ecological variable.
    //
    // Two identical worlds with the field stripped bare, so the only thing moving energy is
    // metabolism — the quantity under comparison. One is watched throughout; the other is abandoned
    // after the same warm-up. Their populations should burn energy at comparable rates.
    let warmup = 60;
    let compare = 240;

    let mut watched = build_world(4, glam::Vec3::ZERO, false);
    let mut sleeping = build_world(4, glam::Vec3::ZERO, false);
    starve_the_field(&mut watched);
    starve_the_field(&mut sleeping);
    // Watched: dormancy enabled but the focus sits on the herd, so nothing ever sleeps.
    enable_dormancy(&mut watched, 2, Some(glam::Vec3::ZERO));
    watched.insert_resource(LodBands {
        hot_radius: 500.0,
        warm_radius: 600.0,
        warm_interval: 1,
    });
    enable_dormancy(&mut sleeping, 2, Some(glam::Vec3::new(80.0, 0.0, 80.0)));

    let mut s1 = ecology_schedule();
    let mut s2 = ecology_schedule();
    run(&mut watched, &mut s1, warmup);
    run(&mut sleeping, &mut s2, warmup);
    assert_eq!(live_agents(&mut watched), 4, "the watched herd stays awake");
    assert_eq!(live_agents(&mut sleeping), 0, "the other one sleeps");

    let animal_energy = |w: &mut World| -> f64 {
        let dormant = w
            .get_resource::<DormantCohorts>()
            .map_or(0.0, |c| c.total_energy());
        let mut q = w.query_filtered::<&HomeostaticState, With<Agent>>();
        let live: f64 = q.iter(w).map(|h| h.energy.max(0.0) as f64).sum();
        live + dormant
    };

    let watched_start = animal_energy(&mut watched);
    let sleeping_start = animal_energy(&mut sleeping);
    run(&mut watched, &mut s1, compare);
    run(&mut sleeping, &mut s2, compare);

    let watched_burn = watched_start - animal_energy(&mut watched);
    let sleeping_burn = sleeping_start - animal_energy(&mut sleeping);
    assert!(
        watched_burn > 0.0 && sleeping_burn > 0.0,
        "both should have burned something: watched {watched_burn}, sleeping {sleeping_burn}"
    );
    // The aggregate rate is a lifetime mean rather than the instantaneous cost, so exact equality
    // is not the claim. What must not happen is dormancy being *systematically* cheap.
    let ratio = sleeping_burn / watched_burn;
    assert!(
        (0.5..=2.0).contains(&ratio),
        "sleeping burned {sleeping_burn} against the watched herd's {watched_burn} (ratio {ratio:.3}) \
         — dormancy has become a different metabolism"
    );
}

#[test]
fn the_ecology_step_is_inert_without_the_cohorts_resource() {
    let mut world = build_world(3, glam::Vec3::ZERO, false);
    with_field(&mut world);
    world.insert_resource(LodBands {
        hot_radius: 5.0,
        warm_radius: 10.0,
        warm_interval: 4,
    });
    world.insert_resource(LodFocus::at(glam::Vec3::new(80.0, 0.0, 80.0)));

    let before = closed_total(&mut world);
    let mut schedule = ecology_schedule();
    run(&mut world, &mut schedule, 60);

    assert_eq!(live_agents(&mut world), 3);
    // Live metabolism still runs, so EU moves between compartments — but the total is untouched and
    // nothing dormant exists to move it.
    let after = closed_total(&mut world);
    assert!(
        (after - before).abs() < TOL,
        "closed EU moved by {}",
        after - before
    );
}
