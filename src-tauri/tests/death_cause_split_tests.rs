//! Card S9: deaths in the live world are counted by cause, and the split is pure bookkeeping.
//!
//! `mortality_system` kills an agent when it is *starved* (energy at the bottom for
//! `starve_ticks`) or *dehydrated* (hydration at the bottom for `dehydration_ticks`). Before S9
//! `VitalStatistics` recorded one undifferentiated death, so no measurement could say whether
//! hunger or thirst was killing a scarce population. S9 files every death under exactly one
//! cause (`core::mortality::lethal_cause`) and reports the window's split as
//! `live.deaths_starvation` and `live.deaths_dehydration`.
//!
//! What these tests hold:
//! - the attribution rule, including a tick on which both conditions are lethal;
//! - the two cause counts add up to `live.deaths` at every sample, in a world where both causes
//!   occur, read the way the shared runner reads (peek between samples, drain on sample ticks);
//! - the same at every sample of a run driven by the runner itself;
//! - requesting the two new observables does not move the trajectory (two runs compared, never an
//!   absolute checksum: checksums differ by platform);
//! - a run without a death rule may not ask for them.

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::ecs::{Agent, EpochManager};
use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{run_manifest_seed, ExperimentModel, RunStatus};
use anima_engine_lib::core::live_experiment::{
    live_observables, live_observables_peek, LiveExperimentAdapter, LiveWorldConfig,
    LIVE_EXTENDED_OBSERVABLE_IDS, LIVE_KEY_DEHYDRATION_TICKS, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS,
    LIVE_KEY_LAKES, LIVE_KEY_MORTALITY, LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_STARVE_TICKS,
    LIVE_KEY_TREES, LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::mortality::{lethal_cause, DeathCause, DeathVerdict};
use anima_engine_lib::core::resources::{MortalityPolicy, VitalStatistics};
use anima_engine_lib::core::world_artifact::WorldIdentity;
use bevy_ecs::prelude::*;

const STARVATION: &str = "live.deaths_starvation";
const DEHYDRATION: &str = "live.deaths_dehydration";

fn observable(sample: &[(String, f64)], id: &str) -> f64 {
    sample
        .iter()
        .find_map(|(name, value)| (name == id).then_some(*value))
        .unwrap_or_else(|| panic!("the sample has no `{id}`"))
}

// ---- The attribution rule ------------------------------------------------------------------------

fn policy(starve_ticks: u32, dehydration_ticks: u32) -> MortalityPolicy {
    MortalityPolicy {
        enabled: true,
        starve_ticks,
        dehydration_ticks,
    }
}

fn homeo(energy: f32, starving: u32, hydration: f32, thirst: Option<u32>) -> HomeostaticState {
    HomeostaticState {
        energy,
        hydration,
        starving_ticks: starving,
        dehydrating_ticks: thirst,
        ..HomeostaticState::default()
    }
}

#[test]
fn one_lethal_condition_is_the_cause() {
    let p = policy(30, 20);
    assert_eq!(lethal_cause(&homeo(0.0, 29, 0.0, Some(19)), &p), None);
    assert_eq!(
        lethal_cause(&homeo(0.0, 30, 50.0, Some(0)), &p),
        Some(DeathVerdict {
            cause: DeathCause::Starvation,
            both_lethal: false
        })
    );
    assert_eq!(
        lethal_cause(&homeo(50.0, 0, 0.0, Some(20)), &p),
        Some(DeathVerdict {
            cause: DeathCause::Dehydration,
            both_lethal: false
        })
    );
    // A streak at the grace with the resource above the bottom is not lethal.
    assert_eq!(lethal_cause(&homeo(0.5, 40, 0.5, Some(40)), &p), None);
}

#[test]
fn when_both_are_lethal_the_one_lethal_first_is_the_cause_and_a_tie_is_starvation() {
    let p = policy(30, 20);
    // Thirst 5 ticks past its grace, hunger 2 past its own: thirst became lethal first.
    assert_eq!(
        lethal_cause(&homeo(0.0, 32, 0.0, Some(25)), &p),
        Some(DeathVerdict {
            cause: DeathCause::Dehydration,
            both_lethal: true
        })
    );
    // Hunger further past its grace.
    assert_eq!(
        lethal_cause(&homeo(0.0, 36, 0.0, Some(21)), &p),
        Some(DeathVerdict {
            cause: DeathCause::Starvation,
            both_lethal: true
        })
    );
    // Both crossed on this very tick: the tie goes to starvation.
    assert_eq!(
        lethal_cause(&homeo(0.0, 30, 0.0, Some(20)), &p),
        Some(DeathVerdict {
            cause: DeathCause::Starvation,
            both_lethal: true
        })
    );
}

#[test]
fn a_shared_era_thirst_streak_is_read_from_the_old_counter() {
    // `dehydrating_ticks: None` is a save from before F-044: the shared counter is the thirst
    // streak, exactly as the old rule read it.
    let p = policy(1_000, 20);
    assert_eq!(
        lethal_cause(&homeo(10.0, 20, 0.0, None), &p),
        Some(DeathVerdict {
            cause: DeathCause::Dehydration,
            both_lethal: false
        })
    );
}

#[test]
fn vital_statistics_partition_deaths_by_cause_and_close_the_window_together() {
    let mut v = VitalStatistics::default();
    v.record_death_by(DeathCause::Starvation, 10, false);
    v.record_death_by(DeathCause::Dehydration, 20, true);
    v.record_death_by(DeathCause::Dehydration, 30, false);
    assert_eq!(v.peek_cause_window(), (1, 2));
    assert_eq!(v.peek_window(), (0, 3, Some(20.0)));
    assert_eq!(v.total_deaths_both_lethal, 1);
    // Peeking is not sampling.
    assert_eq!(v.peek_cause_window(), (1, 2));
    assert_eq!(v.sample_window(), (0, 3, Some(20.0)));
    assert_eq!(
        v.peek_cause_window(),
        (0, 0),
        "sampling closes the cause window too"
    );
    v.record_death_by(DeathCause::Starvation, 5, false);
    assert_eq!(v.peek_cause_window(), (1, 0));
    assert_eq!(
        (v.total_deaths_starvation, v.total_deaths_dehydration),
        (2, 2),
        "the totals are cumulative"
    );
}

// ---- A world where both causes occur ------------------------------------------------------------

const GRACE: u32 = 30;
const FOUNDERS: usize = 7;
const MAX_TICKS: u32 = 2_000;

/// Seven founders, no predators, food, trees or lakes, and an emptied resource field, so nobody can
/// eat or drink. Founders alternate between "out of water, fed" and "out of food, watered"; the
/// first is out of both, so its death is decided by the tie-break.
fn two_cause_world() -> LiveExperimentAdapter {
    let initials = InitialConditionSet::new(vec![
        (LIVE_KEY_FOUNDERS.to_string(), FOUNDERS as f64),
        (LIVE_KEY_PREDATOR_FRACTION.to_string(), 0.0),
        (LIVE_KEY_TREES.to_string(), 0.0),
        (LIVE_KEY_LAKES.to_string(), 0.0),
        (LIVE_KEY_FOOD_CAP.to_string(), 0.0),
        (LIVE_KEY_MORTALITY.to_string(), 1.0),
        (LIVE_KEY_STARVE_TICKS.to_string(), GRACE as f64),
        (LIVE_KEY_DEHYDRATION_TICKS.to_string(), GRACE as f64),
    ]);
    let adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initials,
        &[],
        0x59,
        (16, 16),
        0,
    )
    .expect("a live world with a declared death rule builds");
    {
        let mut world = adapter.world();
        {
            let mut field = world.resource_mut::<ResourceField>();
            field.r.fill(0.0);
            field.r_max.fill(0.0);
        }
        if let Some(mut epochs) = world.get_resource_mut::<EpochManager>() {
            epochs.ticks_per_epoch = 1_000_000;
        }
        let mut animals_eu = 0.0f64;
        let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
        for (i, mut homeo) in q.iter_mut(&mut world).enumerate() {
            let (energy, hydration) = match i {
                0 => (0.0, 0.0),
                i if i % 2 == 1 => (homeo.energy_target, 0.0),
                _ => (0.0, homeo.hydration_target),
            };
            homeo.energy = energy;
            homeo.hydration = hydration;
            homeo.starving_ticks = 0;
            homeo.dehydrating_ticks = Some(0);
            animals_eu += energy as f64;
        }
        // Keep the closed-EU books consistent with the energy just set.
        world.resource_mut::<EcosystemBiomass>().animals = animals_eu;
    }
    adapter
}

#[test]
fn cause_counts_add_up_to_live_deaths_at_every_sample_when_both_causes_occur() {
    const SAMPLE_PERIOD: u32 = 7;
    let mut adapter = two_cause_world();
    let (mut starvation, mut dehydration, mut deaths) = (0.0, 0.0, 0.0);
    let mut samples = 0;
    let mut extinct_at = None;
    for tick in 1..=MAX_TICKS {
        adapter.run_schedule_once();
        let mut world = adapter.world();
        // The runner reads every tick and keeps only sample ticks; a peek must not steal events.
        let sample = if tick % SAMPLE_PERIOD == 0 {
            live_observables(&mut world)
        } else {
            let peek = live_observables_peek(&mut world);
            assert_eq!(
                observable(&peek, STARVATION) + observable(&peek, DEHYDRATION),
                observable(&peek, "live.deaths"),
                "a peek at tick {tick} splits the open window too"
            );
            continue;
        };
        samples += 1;
        let (s, d, all) = (
            observable(&sample, STARVATION),
            observable(&sample, DEHYDRATION),
            observable(&sample, "live.deaths"),
        );
        assert_eq!(s + d, all, "sample at tick {tick}: {s} + {d} != {all}");
        starvation += s;
        dehydration += d;
        deaths += all;
        if observable(&sample, "live.agent_count") == 0.0 {
            extinct_at = Some(tick);
            break;
        }
    }
    let extinct_at = extinct_at.expect("nobody can eat or drink, so everyone must die");
    let both = adapter
        .world()
        .resource::<VitalStatistics>()
        .total_deaths_both_lethal;
    println!(
        "S9 two-cause world: extinct at tick {extinct_at} after {samples} samples; \
         starvation {starvation}, dehydration {dehydration}, both lethal {both}"
    );
    assert_eq!(
        deaths, FOUNDERS as f64,
        "every founder's death is counted once"
    );
    assert!(
        starvation >= 1.0 && dehydration >= 1.0,
        "both causes must occur (starvation {starvation}, dehydration {dehydration})"
    );
    // Founders 2, 4 and 6 are out of food only, 1, 3 and 5 out of water only, founder 0 out of both
    // from the first tick, so its two streaks tie and the tie goes to starvation. The counts are
    // deliberately unequal (4 against 3): an adapter that swapped the two ids would fail here, which
    // the sum check alone cannot see.
    assert_eq!(starvation, 4.0, "{STARVATION}");
    assert_eq!(dehydration, 3.0, "{DEHYDRATION}");
    assert_eq!(both, 1, "exactly one death was decided by the tie-break");
}

// ---- Through the shared runner -------------------------------------------------------------------

const RUN_SEED: u64 = 1;
const RUN_TICKS: u64 = 3_000;
const RUN_SAMPLE_PERIOD: u64 = 60;

/// The default live world with a death rule and births declared (the same world as
/// `live_windowed_observable_sampling_tests.rs`). Without any hand edits it kills by both causes
/// within the run; the test asserts that rather than assuming it.
fn runner_initials() -> InitialConditionSet {
    InitialConditionSet::new(vec![
        ("live.mortality".to_string(), 1.0),
        ("live.reproduction".to_string(), 1.0),
    ])
}

fn manifest(with_cause_split: bool) -> ExperimentManifest {
    let observable_ids: Vec<String> = LIVE_OBSERVABLE_IDS
        .iter()
        .chain(LIVE_EXTENDED_OBSERVABLE_IDS.iter())
        .filter(|id| with_cause_split || !matches!(**id, STARVATION | DEHYDRATION))
        .map(|s| s.to_string())
        .collect();
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "s9-death-causes".to_string(),
        name: "s9-death-causes".to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: runner_initials(),
        interventions: vec![],
        seeds: vec![RUN_SEED],
        duration_ticks: RUN_TICKS,
        sample_period: RUN_SAMPLE_PERIOD,
        observable_ids,
        exotic_interventions: Vec::new(),
        observer: Default::default(),
    }
}

#[test]
fn requesting_the_cause_split_does_not_move_the_trajectory_and_it_adds_up_at_every_sample() {
    let registry = ObservableRegistry::live_default();
    let with = run_manifest_seed::<LiveExperimentAdapter>(
        &manifest(true),
        &registry,
        RUN_SEED,
        None,
        None,
    );
    let without = run_manifest_seed::<LiveExperimentAdapter>(
        &manifest(false),
        &registry,
        RUN_SEED,
        None,
        None,
    );
    assert_eq!(with.status, RunStatus::Completed, "{:?}", with.warnings);
    assert_eq!(
        without.status,
        RunStatus::Completed,
        "{:?}",
        without.warnings
    );
    assert_eq!(
        with.final_checksum, without.final_checksum,
        "requesting two bookkeeping observables must not change the world"
    );
    assert_eq!(with.series.len(), without.series.len());
    for (a, b) in with.series.iter().zip(without.series.iter()) {
        assert_eq!(a.tick, b.tick);
        for id in LIVE_OBSERVABLE_IDS {
            assert_eq!(
                observable(&a.observables, id).to_bits(),
                observable(&b.observables, id).to_bits(),
                "{id} at tick {} differs between the two runs",
                a.tick
            );
        }
    }

    let (mut starvation, mut dehydration) = (0.0, 0.0);
    for s in &with.series {
        let (st, de, all) = (
            observable(&s.observables, STARVATION),
            observable(&s.observables, DEHYDRATION),
            observable(&s.observables, "live.deaths"),
        );
        assert_eq!(
            st + de,
            all,
            "sample at tick {}: {st} + {de} != {all}",
            s.tick
        );
        starvation += st;
        dehydration += de;
    }
    println!("S9 runner world: starvation {starvation}, dehydration {dehydration}");
    assert!(
        starvation > 0.0 && dehydration > 0.0,
        "the runner world must kill by both causes (starvation {starvation}, dehydration {dehydration})"
    );
}

// ---- Gating --------------------------------------------------------------------------------------

#[test]
fn a_run_without_a_death_rule_may_not_ask_for_the_cause_split() {
    let no_death = LiveWorldConfig::from_initial_conditions(&InitialConditionSet::new(vec![]))
        .expect("the empty declaration is valid");
    for id in [STARVATION, DEHYDRATION] {
        assert!(
            no_death
                .ensure_observables_supported(&[id.to_string()])
                .is_err(),
            "{id} without a death rule would be a fabricated zero"
        );
    }
    let with_death = LiveWorldConfig::from_initial_conditions(&InitialConditionSet::new(vec![(
        LIVE_KEY_MORTALITY.to_string(),
        1.0,
    )]))
    .expect("a declared death rule is valid");
    with_death
        .ensure_observables_supported(&[STARVATION.to_string(), DEHYDRATION.to_string()])
        .expect("a run with a death rule can report its causes");
}

#[test]
fn the_cause_split_is_appended_after_the_existing_extended_ids() {
    let n = LIVE_EXTENDED_OBSERVABLE_IDS.len();
    assert_eq!(
        &LIVE_EXTENDED_OBSERVABLE_IDS[n - 2..],
        &[STARVATION, DEHYDRATION],
        "new ids go at the end; the order before them is part of the emission contract"
    );
}
