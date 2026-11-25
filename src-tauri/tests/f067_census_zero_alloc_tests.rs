//! Regression and zero-allocation proof for F-067:
//! `demographic_census_system` -> `update_demographic_census_from_iter` must allocate zero times
//! on the tick hot path once fixed census slots are initialized (and immediately when
//! `DemographicCensusState` is absent) while keeping census outputs and trajectories bit-identical.

mod common;

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::AgentEvaluation;
use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::components::{Agent, AgentSex, Predator, Prey};
use anima_engine_lib::core::ecs::AgentType;
use anima_engine_lib::core::environmental_systems::demographic_census_system;
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::LiveExperimentAdapter;
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::telemetry_export::{
    build_demographic_census, DemographicCensusState, DemographicTracker,
};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ExecutorKind;
use glam::Vec3;
use rand::{rngs::StdRng, SeedableRng};

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

fn eval_age(survival_ticks: u32) -> AgentEvaluation {
    AgentEvaluation {
        start_position: Vec3::ZERO,
        total_distance: 0.0,
        total_energy_expended: 0.0,
        total_energy_gained: 0.0,
        prey_energy_gained: 0.0,
        survival_ticks,
        last_position: Vec3::ZERO,
    }
}

fn spawn_census_agent(
    world: &mut World,
    age: u32,
    is_predator: bool,
    sex: Option<AgentSex>,
    energy: f32,
) -> Entity {
    let mut e = world.spawn((Agent, eval_age(age), homeo(energy)));
    if is_predator {
        e.insert(Predator);
    } else {
        e.insert(Prey);
    }
    if let Some(s) = sex {
        e.insert(s);
    }
    e.id()
}

fn reference_census_from_world(world: &mut World) -> DemographicCensusState {
    let mut query = world.query_filtered::<(
        &AgentEvaluation,
        Option<&Predator>,
        Option<&AgentSex>,
        &HomeostaticState,
    ), With<Agent>>();
    let mut rows = Vec::new();
    for (eval, opt_pred, opt_sex, homeo) in query.iter(world) {
        if homeo.energy > 0.0 {
            let agent_type = if opt_pred.is_some() {
                Some(AgentType::Predator)
            } else {
                Some(AgentType::Prey)
            };
            rows.push((eval.survival_ticks, agent_type, opt_sex.copied()));
        }
    }
    let tracker = world.get_resource::<DemographicTracker>();
    build_demographic_census(&rows, tracker)
}

fn assert_census_bit_identical(actual: &DemographicCensusState, expected: &DemographicCensusState) {
    assert_eq!(actual.age_cohorts, expected.age_cohorts);
    assert_eq!(actual.mortality_causes, expected.mortality_causes);
    assert_eq!(actual.fecundity_by_age, expected.fecundity_by_age);
    assert_eq!(actual.shannon.to_bits(), expected.shannon.to_bits());
    assert_eq!(actual.simpson.to_bits(), expected.simpson.to_bits());
    assert_eq!(actual.prey_count, expected.prey_count);
    assert_eq!(actual.predator_count, expected.predator_count);
    assert_eq!(actual.male_count, expected.male_count);
    assert_eq!(actual.female_count, expected.female_count);
    assert_eq!(actual.sex_ratio.to_bits(), expected.sex_ratio.to_bits());
    assert_eq!(actual.total_births, expected.total_births);
    assert_eq!(actual.total_deaths, expected.total_deaths);
}

#[test]
fn f067_demographic_census_zero_alloc_and_bit_identical() {
    // Phase 0: positive control ensuring ThreadTrackingAllocator detects heap allocations.
    ALLOCATOR.start_tracking();
    let boxed = std::hint::black_box(Box::new(42u8));
    drop(boxed);
    let ctrl_allocs = ALLOCATOR.stop_tracking();
    assert!(
        ctrl_allocs > 0,
        "positive control failed: Box::new allocated 0 times"
    );

    // Phase 1: Resource absent -> 0 allocations from tick 0 (even without warmup).
    let mut absent_world = World::new();
    absent_world.insert_resource(DemographicTracker::default());
    spawn_census_agent(&mut absent_world, 12, false, Some(AgentSex::Female), 80.0);
    spawn_census_agent(&mut absent_world, 120, true, Some(AgentSex::Male), 85.0);
    spawn_census_agent(&mut absent_world, 250, false, None, 70.0);

    let mut absent_sched = Schedule::default();
    absent_sched.set_executor_kind(ExecutorKind::SingleThreaded);
    absent_sched.add_systems(demographic_census_system);
    // Warm up Bevy schedule initialization once before measuring tick allocations.
    absent_sched.run(&mut absent_world);

    let mut absent_allocs = 0usize;
    for _ in 0..20 {
        ALLOCATOR.start_tracking();
        absent_sched.run(&mut absent_world);
        absent_allocs += ALLOCATOR.stop_tracking();
    }
    assert_eq!(
        absent_allocs, 0,
        "demographic_census_system allocated {absent_allocs} times when DemographicCensusState was absent"
    );

    // Phase 2: Resource present -> warmup once, then 0 allocations across 50 active ticks +
    // doubled population burst, with bit-identical output parity vs reference `build_demographic_census`.
    let mut world = World::new();
    world.insert_resource(DemographicTracker::default());
    world.insert_resource(DemographicCensusState::default());

    let mut toggle_entities = Vec::new();
    for (age, pred, sex) in [
        (10u32, false, Some(AgentSex::Male)),
        (45, false, Some(AgentSex::Female)),
        (90, true, None),
        (190, false, None),
        (380, true, Some(AgentSex::Male)),
        (520, false, Some(AgentSex::Female)),
    ] {
        toggle_entities.push(spawn_census_agent(&mut world, age, pred, sex, 75.0));
    }
    let _always_dead = spawn_census_agent(&mut world, 25, false, Some(AgentSex::Female), 0.0);

    let mut sched = Schedule::default();
    sched.set_executor_kind(ExecutorKind::SingleThreaded);
    sched.add_systems(demographic_census_system);

    // Warm-up tick initializes fixed 4/5/3 slots in DemographicCensusState once.
    sched.run(&mut world);
    let expected_warmup = reference_census_from_world(&mut world);
    assert_census_bit_identical(world.resource::<DemographicCensusState>(), &expected_warmup);

    let mut present_allocs = 0usize;
    for tick in 0..50u32 {
        // Mutate ages, alive/dead status, and tracker tallies outside measurement window.
        let toggle_entity = toggle_entities[(tick as usize) % toggle_entities.len()];
        if let Some(mut eval) = world.get_mut::<AgentEvaluation>(toggle_entity) {
            eval.survival_ticks = eval.survival_ticks.wrapping_add(37);
        }
        if let Some(mut h) = world.get_mut::<HomeostaticState>(toggle_entity) {
            h.energy = if tick % 2 == 0 { 65.0 } else { 0.0 };
        }
        {
            let mut trk = world.resource_mut::<DemographicTracker>();
            trk.record_birth((tick * 40) % 250);
            let cause = match tick % 5 {
                0 => "Starvation",
                1 => "Predation",
                2 => "Senescence",
                3 => "Toxicity",
                _ => "Pathogen",
            };
            trk.record_death(cause);
        }

        ALLOCATOR.start_tracking();
        sched.run(&mut world);
        present_allocs += ALLOCATOR.stop_tracking();

        let expected = reference_census_from_world(&mut world);
        assert_census_bit_identical(world.resource::<DemographicCensusState>(), &expected);
    }
    assert_eq!(
        present_allocs, 0,
        "demographic_census_system allocated {present_allocs} times during 50 warm ticks"
    );

    // Double population outside measurement and verify 20 more ticks allocate 0 times.
    for idx in 0..24u32 {
        spawn_census_agent(
            &mut world,
            idx * 25,
            idx % 4 == 0,
            match idx % 3 {
                0 => Some(AgentSex::Male),
                1 => Some(AgentSex::Female),
                _ => None,
            },
            60.0,
        );
    }
    let mut doubled_allocs = 0usize;
    for _ in 0..20 {
        ALLOCATOR.start_tracking();
        sched.run(&mut world);
        doubled_allocs += ALLOCATOR.stop_tracking();

        let expected = reference_census_from_world(&mut world);
        assert_census_bit_identical(world.resource::<DemographicCensusState>(), &expected);
    }
    assert_eq!(
        doubled_allocs, 0,
        "demographic_census_system allocated {doubled_allocs} times after population doubled"
    );

    // Phase 3: Full live-world trajectory comparison (bit-identical with vs without census resource).
    let laws = WorldLawSet::baseline();
    let initial = InitialConditionSet::new(vec![
        ("live.founders".into(), 20.0),
        ("live.predator_fraction".into(), 0.25),
        ("live.mortality".into(), 1.0),
    ]);
    let horizon = 60u64;
    let seed = 777_067u64;

    let mut adapter_a =
        LiveExperimentAdapter::from_manifest(&laws, &initial, &[], seed, (16, 16), horizon)
            .expect("build baseline adapter");
    let mut adapter_b =
        LiveExperimentAdapter::from_manifest(&laws, &initial, &[], seed, (16, 16), horizon)
            .expect("build census-enabled adapter");

    {
        let mut world_b = adapter_b.world();
        if world_b.get_resource::<DemographicTracker>().is_none() {
            world_b.insert_resource(DemographicTracker::default());
        }
        world_b.insert_resource(DemographicCensusState::default());
    }

    let mut clock_a = SimClock::new();
    let mut clock_b = SimClock::new();
    let mut ledger_a = CausalLedger::new();
    let mut ledger_b = CausalLedger::new();
    let mut rng_a = StdRng::seed_from_u64(seed);
    let mut rng_b = StdRng::seed_from_u64(seed);

    for step in 1..=horizon {
        let _ = clock_a.advance();
        let _ = clock_b.advance();
        adapter_a.step(&clock_a, &[], &mut ledger_a, &mut rng_a);
        adapter_b.step(&clock_b, &[], &mut ledger_b, &mut rng_b);
        assert_eq!(
            adapter_a.checksum(),
            adapter_b.checksum(),
            "trajectory diverged at tick {step} with DemographicCensusState active"
        );
        let expected_live = {
            let mut world_b = adapter_b.world();
            reference_census_from_world(&mut world_b)
        };
        let world_b = adapter_b.world();
        assert_census_bit_identical(world_b.resource::<DemographicCensusState>(), &expected_live);
    }
}
