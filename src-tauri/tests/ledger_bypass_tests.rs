//! G5 / brief B6(c): the ledger bypass and the suspected `PathogenTransmission` EU leak.
//!
//! Two claims in the B6 brief were **read from source, not measured**. This file measures them.
//!
//! 1. `PathogenTransmission` debits `pool.animals` and credits `pool.detritus` through
//!    `EnergyLedger::transfer`, but `pool.animals` is a *mirror*: `ecosystem_census_system`
//!    re-snapshots it from the agents' reserves at the end of every tick, and the intervention culls
//!    no agent. If that reading is right, the debit evaporates at the census, the detritus credit
//!    stays, and the closed total grows by the culled amount every time the forcing fires.
//! 2. `autonomous_reproduction_system` routes the reproductive overhead `(1 - eta) * invested`
//!    straight into `pool.detritus`, outside `EnergyLedger`. The arithmetic may balance, in which case
//!    the closed total is untouched, but the ledger's own books (`granted`, `settled_count`) never
//!    see the flow.
//!
//! Every conservation assertion here reads the closed total from the **authoritative stores** (the
//! resource field's cells, every living agent's reserve, the detritus pool, the dormant pool), never
//! from the `EcosystemBiomass` mirrors, and compares it with the baseline the census locked. The
//! tolerance is `RESIDUAL_ABS_TOLERANCE_EU`; it is a property of the arithmetic and is not raised
//! to make a test pass.
//!
//! Each conservation test has a **control** run in the same configuration with no intervention, so
//! a residual that appears with the forcing is attributed to the forcing and not to the world.

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::autonomous_reproduction_system;
use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::ecs::Agent;
use anima_engine_lib::core::energy_ledger::{
    closed_total_eu, EnergyLedger, RESIDUAL_ABS_TOLERANCE_EU,
};
use anima_engine_lib::core::environmental_systems::ecosystem_census_system;
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::intervention::{
    Curve, InterventionCommand, InterventionKind, InterventionQueue, Region,
};
use anima_engine_lib::core::live_experiment::LiveExperimentAdapter;
use anima_engine_lib::core::sim_clock::SimClock;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use rand::rngs::StdRng;
use rand::SeedableRng;

const SEED: u64 = 4242;

/// The ecology band fires every 60 base ticks, and every stock-moving forcing acts only on it. A
/// forcing declared for tick 60 therefore fires exactly once in a run this long, and the 30 ticks
/// after it are enough for the census to have re-snapshotted `pool.animals` many times over.
const FIRE_TICK: u64 = 60;
const RUN_TICKS: u64 = 90;

/// Each run builds a 256x256 field and a Bevy world; running them in parallel threads only makes
/// the debug build slower. The same lock `live_experiment_tests.rs` uses.
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn initial(extra: &[(&str, f64)]) -> InitialConditionSet {
    let mut values = vec![
        ("live.founders".to_string(), 6.0),
        ("live.predator_fraction".to_string(), 0.5),
        ("live.trees".to_string(), 4.0),
        ("live.lakes".to_string(), 2.0),
        ("live.food_cap".to_string(), 50.0),
    ];
    for (key, value) in extra {
        // An override replaces the base value rather than duplicating the key, which the adapter
        // would read as the same key declared twice.
        values.retain(|(k, _)| k != key);
        values.push(((*key).to_string(), *value));
    }
    InitialConditionSet::new(values)
}

fn build(extra: &[(&str, f64)]) -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial(extra),
        &[],
        SEED,
        (64, 64),
        RUN_TICKS,
    )
    .expect("the live world must build from a valid manifest")
}

/// The world's closed EU summed from the authoritative stores. Deliberately not read off the
/// `EcosystemBiomass` mirrors: `pool.animals` is the very thing under suspicion.
fn closed_total(world: &mut World) -> f64 {
    let plants = world
        .get_resource::<ResourceField>()
        .map(|f| f.total_biomass())
        .unwrap_or(0.0);
    let (detritus, dormant) = world
        .get_resource::<EcosystemBiomass>()
        .map(|p| (p.detritus, p.dormant))
        .unwrap_or((0.0, 0.0));
    let mut animals = 0.0f64;
    let mut q = world.query_filtered::<&HomeostaticState, With<Agent>>();
    for homeo in q.iter(world) {
        animals += homeo.energy.max(0.0) as f64;
    }
    closed_total_eu(plants, animals, detritus, dormant)
}

/// One tick-by-tick run: what the closed total did, and whether the forcing did anything at all.
struct Run {
    /// `(tick, authoritative closed EU - the baseline the census locked)`.
    residuals: Vec<(u64, f64)>,
    /// Whether the causal ledger recorded at least one effect, i.e. the forcing actually acted.
    forcing_acted: bool,
}

impl Run {
    fn worst(&self) -> (u64, f64) {
        self.residuals
            .iter()
            .copied()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .expect("a run records at least one tick")
    }

    /// The residual left after the forcing's tick and every later census.
    fn last(&self) -> (u64, f64) {
        *self
            .residuals
            .last()
            .expect("a run records at least one tick")
    }

    fn at(&self, tick: u64) -> f64 {
        self.residuals
            .iter()
            .find(|(t, _)| *t == tick)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("tick {tick} was not recorded"))
    }
}

fn run_live(extra: &[(&str, f64)], interventions: Vec<InterventionCommand>) -> Run {
    let mut adapter = build(extra);
    let mut clock = SimClock::new();
    let mut causal = CausalLedger::new();
    let mut rng = StdRng::seed_from_u64(SEED);
    let queue = InterventionQueue::new(interventions);

    let mut baseline: Option<f64> = None;
    let mut residuals = Vec::with_capacity(RUN_TICKS as usize);
    for _ in 0..RUN_TICKS {
        let tick = clock.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        adapter.step(&clock, &active, &mut causal, &mut rng);

        let mut world = adapter.world();
        // The baseline the census locked on its first run (D06): the one number every later
        // residual in this file is measured against.
        let locked = baseline.get_or_insert_with(|| {
            world
                .resource::<EnergyLedger>()
                .baseline()
                .expect("the census locks a baseline on its first tick")
        });
        let locked = *locked;
        residuals.push((tick, closed_total(&mut world) - locked));
    }
    Run {
        residuals,
        forcing_acted: !causal.is_empty(),
    }
}

fn command(id: u32, kind: InterventionKind, intensity: f32) -> InterventionCommand {
    InterventionCommand {
        id,
        cause_id: 100 + id,
        kind,
        region: Region::Global,
        start_tick: FIRE_TICK,
        // One tick: active only on tick 60, which is an ecology-band firing.
        duration_ticks: 1,
        intensity,
        signed_negative: false,
        curve: Curve::Step,
        reversible: false,
    }
}

/// A 25% epidemic: `intensity * rate * mortality * 0.01` = 1.0 * 10.0 * 2.5 * 0.01.
fn pathogen() -> InterventionCommand {
    command(
        3,
        InterventionKind::PathogenTransmission {
            pathogen_id: 1,
            transmission_rate: 10.0,
            mortality_multiplier: 2.5,
        },
        1.0,
    )
}

/// `None` when the run stayed on its baseline, otherwise a description of how it left it.
fn residual_problem(label: &str, run: &Run) -> Option<String> {
    let (worst_tick, worst) = run.worst();
    let (last_tick, last) = run.last();
    if worst.abs() <= RESIDUAL_ABS_TOLERANCE_EU && last.abs() <= RESIDUAL_ABS_TOLERANCE_EU {
        return None;
    }
    Some(format!(
        "{label}: closed EU left the baseline. worst residual {worst:.9} at tick {worst_tick}, \
         final residual {last:.9} at tick {last_tick}, tolerance {RESIDUAL_ABS_TOLERANCE_EU}. \
         Energy is being created or destroyed outside EnergyLedger."
    ))
}

fn assert_within_tolerance(label: &str, run: &Run) {
    if let Some(problem) = residual_problem(label, run) {
        panic!("{problem}");
    }
}

/// The flag configurations the conservation claim is made for: the legacy genesis, continuous
/// reproduction (ADR-0005, which is what the desktop app runs), and reproduction plus the death
/// rule, so births, starvation deaths and the forcing all move EU in one run.
const CONFIGS: &[(&str, &[(&str, f64)])] = &[
    ("baseline", &[]),
    ("reproduction", &[("live.reproduction", 1.0)]),
    (
        "reproduction+mortality",
        &[("live.reproduction", 1.0), ("live.mortality", 1.0)],
    ),
];

// ---- 1. PathogenTransmission ---------------------------------------------------------------------

#[test]
fn pathogen_transmission_does_not_create_energy_in_any_flag_configuration() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Every configuration is measured before the test fails, so one red run reports all of them
    // rather than only the first.
    let mut problems: Vec<String> = Vec::new();
    for (name, extra) in CONFIGS {
        // The control comes first: if the world itself drifts in this configuration, a residual in
        // the treatment run says nothing about the forcing and the test must say so.
        let control = run_live(extra, vec![]);
        problems.extend(residual_problem(
            &format!("[{name}] control (no intervention)"),
            &control,
        ));

        let treatment = run_live(extra, vec![pathogen()]);
        if !treatment.forcing_acted {
            problems.push(format!(
                "[{name}] the epidemic never acted, so a residual of zero would prove nothing"
            ));
            continue;
        }
        let jump = treatment.at(FIRE_TICK) - treatment.at(FIRE_TICK - 1);
        problems.extend(residual_problem(
            &format!(
                "[{name}] PathogenTransmission at tick {FIRE_TICK} (residual moved by {jump:.6} \
                 EU on the firing tick)"
            ),
            &treatment,
        ));
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

// ---- The other three forcings that move stocks, as guards ----------------------------------------

#[test]
fn the_stock_moving_forcings_conserve_closed_eu() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // The brief notes that no test fires these against the ledger. Whatever the PathogenTransmission
    // result, these are cheap to pin down in the legacy configuration, and a failure here is a
    // finding of its own rather than something to fold into the epidemic.
    let cases: Vec<(&str, InterventionCommand)> = vec![
        ("Deforest", command(1, InterventionKind::Deforest, 0.5)),
        (
            "AddNutrient",
            command(2, InterventionKind::AddNutrient, 1.0),
        ),
        (
            "ToxicityPlume",
            command(
                4,
                InterventionKind::ToxicityPlume {
                    center_x: 128.0,
                    center_y: 128.0,
                    radius: 400.0,
                    toxicity_level: 10.0,
                    decay_rate: 0.0,
                },
                1.0,
            ),
        ),
        (
            "RemovePredators",
            command(5, InterventionKind::RemovePredators, 1.0),
        ),
    ];
    for (name, cmd) in cases {
        let run = run_live(&[], vec![cmd]);
        assert!(
            run.forcing_acted,
            "{name} never acted in this world, so a zero residual would prove nothing"
        );
        assert_within_tolerance(name, &run);
    }
}

// ---- 2. Reproduction overhead --------------------------------------------------------------------

/// What one reproduction tick did, measured around a schedule that contains **only**
/// `autonomous_reproduction_system`, so nothing else in the tick can move the ledger's counters.
struct Birth {
    /// How many parents were above the reproduction threshold, hence how many births to expect.
    births: usize,
    closed_before: f64,
    closed_after: f64,
    /// The `pool.animals` mirror as the system found it.
    mirror_before: f64,
    /// EU the parents' reserves actually lost (measured, not computed from the formula).
    invested: f64,
    /// EU the newborns were endowed with (measured off the children's reserves).
    endowment: f64,
    granted_delta: f64,
    settled_delta: u64,
    refused_delta: f64,
}

/// `parents` full prey reproduce in one tick of `autonomous_reproduction_system`.
///
/// `refresh_mirror = true` runs the census first, so `pool.animals` is a fresh snapshot of the
/// reserves, which is the state of every tick after the first. `false` leaves the mirror as genesis
/// built it, which is the state of the **first** tick: the census has not run, `pool.animals` is
/// still zero, and every founder above the threshold reproduces in that tick.
fn births(parents: usize, refresh_mirror: bool) -> Birth {
    // All prey, so no predator-gating; only the first `parents` agents are above the threshold.
    let adapter = build(&[("live.reproduction", 1.0), ("live.predator_fraction", 0.0)]);
    let mut world = adapter.world();

    let mut agents: Vec<Entity> = {
        let mut q = world.query_filtered::<Entity, With<Agent>>();
        q.iter(&world).collect()
    };
    agents.sort_unstable();
    assert!(
        agents.len() > parents,
        "the world needs {parents} parents and at least one bystander"
    );
    for (i, e) in agents.iter().enumerate() {
        let mut homeo = world
            .get_mut::<HomeostaticState>(*e)
            .expect("agent reserve");
        let target = homeo.energy_target;
        // A parent is full (above the 0.65 prey threshold and the 0.30 safe reserve); the rest are
        // far below it, so they cannot reproduce this tick.
        homeo.energy = if i < parents { target } else { 0.1 * target };
    }
    let parent_ids = &agents[..parents];
    let reserve =
        |world: &World, e: Entity| world.get::<HomeostaticState>(e).unwrap().energy as f64;
    let parents_before: f64 = parent_ids.iter().map(|e| reserve(&world, *e)).sum();

    if refresh_mirror {
        // Refresh the `animals` mirror so it matches the reserves just set, exactly as the end of a
        // normal tick would.
        let mut census = Schedule::default();
        census.add_systems(ecosystem_census_system);
        census.run(&mut world);
    }

    let mirror_before = world.resource::<EcosystemBiomass>().animals;
    let closed_before = closed_total(&mut world);
    let ledger_before = *world.resource::<EnergyLedger>();

    let mut schedule = Schedule::default();
    schedule.add_systems(autonomous_reproduction_system);
    schedule.run(&mut world);

    let parents_after: f64 = parent_ids.iter().map(|e| reserve(&world, *e)).sum();
    let newborns: Vec<Entity> = {
        let mut q = world.query_filtered::<Entity, With<Agent>>();
        q.iter(&world).filter(|e| !agents.contains(e)).collect()
    };
    assert_eq!(
        newborns.len(),
        parents,
        "exactly {parents} parents were above the reproduction threshold"
    );
    let endowment: f64 = newborns.iter().map(|e| reserve(&world, *e)).sum();

    let closed_after = closed_total(&mut world);
    let ledger_after = *world.resource::<EnergyLedger>();
    Birth {
        births: parents,
        closed_before,
        closed_after,
        mirror_before,
        invested: parents_before - parents_after,
        endowment,
        granted_delta: ledger_after.granted() - ledger_before.granted(),
        settled_delta: ledger_after.settled_count() - ledger_before.settled_count(),
        refused_delta: ledger_after.refused() - ledger_before.refused(),
    }
}

fn one_birth() -> Birth {
    births(1, true)
}

#[test]
fn a_birth_conserves_closed_eu() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // The arithmetic half of the claim: parent loses `invested`, the child gets `endowment`, and the
    // remainder reaches detritus. This is expected to hold today; it is here so the fix to the
    // *accounting* below cannot trade a visible ledger entry for a leak.
    let b = one_birth();
    let overhead = b.invested - b.endowment;
    assert!(
        overhead > 0.0,
        "a birth must have a Lindeman overhead, got {overhead}"
    );
    assert!(
        (b.closed_after - b.closed_before).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "one birth moved the closed total by {:.9} EU (before {:.6}, after {:.6})",
        b.closed_after - b.closed_before,
        b.closed_before,
        b.closed_after
    );
}

#[test]
fn the_reproduction_overhead_is_visible_to_the_energy_ledger() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // The accounting half. The overhead is a flow of EU from the animals compartment to detritus,
    // so it must be a transaction the ledger settled: `granted` grows by exactly that amount and
    // `settled_count` by one. Written straight into `pool.detritus`, neither moves, and the ledger
    // has no record that the flow ever happened.
    let b = one_birth();
    let overhead = b.invested - b.endowment;
    assert!(
        overhead > 0.0,
        "a birth must have a Lindeman overhead, got {overhead}"
    );
    assert_eq!(
        b.settled_delta, 1,
        "the reproduction overhead ({overhead:.6} EU) is not a settled ledger transaction: \
         settled_count moved by {} (granted moved by {:.6}, refused by {:.6})",
        b.settled_delta, b.granted_delta, b.refused_delta
    );
    assert!(
        (b.granted_delta - overhead).abs() <= 1e-9,
        "the ledger granted {:.9} EU for a birth whose overhead was {overhead:.9} EU",
        b.granted_delta
    );
    assert_eq!(
        b.refused_delta, 0.0,
        "the overhead must be fully funded, not partly refused"
    );
}

#[test]
fn every_birth_of_the_first_tick_is_visible_to_the_ledger_although_the_mirror_is_still_zero() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // The first tick is where most births of a short run happen: genesis fills every founder's
    // reserve, the census that snapshots `pool.animals` has not run yet, and the mirror is zero. A
    // ledger transfer clamps to the mirror, so a routing that only works when the mirror is fresh
    // hands every one of those births to the raw detritus write, and the ledger never sees them
    // (measured on a 20-founder run: 14 of the first 19 births, 280 EU, at a5b89f3). The previous
    // tests run the census first and could not see that.
    let b = births(3, false);
    let overhead = b.invested - b.endowment;
    assert!(
        overhead > 0.0,
        "births must have a Lindeman overhead, got {overhead}"
    );
    assert!(
        b.mirror_before < overhead / b.births as f64,
        "the premise of this test is a stale mirror, but pool.animals was {} EU",
        b.mirror_before
    );
    assert!(
        (b.closed_after - b.closed_before).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "{} births with a stale mirror moved the closed total by {:.9} EU",
        b.births,
        b.closed_after - b.closed_before
    );
    assert_eq!(
        b.settled_delta, b.births as u64,
        "{} births produced {overhead:.6} EU of overhead but only {} settled ledger transactions \
         (granted moved by {:.6}, refused by {:.6}): the rest was written straight into detritus",
        b.births, b.settled_delta, b.granted_delta, b.refused_delta
    );
    assert!(
        (b.granted_delta - overhead).abs() <= 1e-9,
        "the ledger granted {:.9} EU for births whose overhead was {overhead:.9} EU",
        b.granted_delta
    );
    assert_eq!(
        b.refused_delta, 0.0,
        "a stale mirror is not a shortage: the reserves held the EU, so nothing may be refused"
    );
}
