//! Observable gates — feature `ecological-selection`, branch O.
//!
//! - **Wiring** — all ids (legacy 12 + extended 10) emit in stable order, all finite.
//! - **OB-S01** — `live.shannon` / `live.simpson` match the panel's own index math on the same
//!   abundances.
//! - **OB-S02** — variance separates populations an identical mean cannot.
//! - **OB-S03** — turnover counts are WINDOWED: they drain on sample instead of accumulating.
//! - **OB-S04** — `live.archive_coverage` mirrors `archive.grid.len()`.
//! - **O-5** — death observables on a run that declares no death rule FAIL instead of
//!   fabricating zeros.
//!
//! CA-S03's strict form ("the legacy twelve provably cannot distinguish") is documented against
//! the turnover/frozen pair in `turnover_is_visible_to_new_observables_at_a_matched_count`.

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    all_live_observable_ids, live_observables, mass_statistics, LiveExperimentAdapter,
    LIVE_EXTENDED_OBSERVABLE_IDS, LIVE_KEY_DEHYDRATION_TICKS, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS,
    LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_STARVE_TICKS, LIVE_KEY_TREES,
};
use bevy_ecs::prelude::*;

const SEED: u64 = 0x5EED;

fn base_initials() -> InitialConditionSet {
    InitialConditionSet::new(vec![
        (LIVE_KEY_FOUNDERS.to_string(), 6.0),
        (LIVE_KEY_PREDATOR_FRACTION.to_string(), 0.0),
        (LIVE_KEY_TREES.to_string(), 4.0),
        ("live.lakes".to_string(), 1.0),
        (LIVE_KEY_FOOD_CAP.to_string(), 50.0),
    ])
}

fn adapter(initials: &InitialConditionSet) -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(&WorldLawSet::baseline(), initials, &[], SEED, (16, 16), 0)
        .expect("the live world builds")
}

fn value_of(obs: &[(String, f64)], id: &str) -> f64 {
    obs.iter()
        .find(|(k, _)| k == id)
        .map(|(_, v)| *v)
        .unwrap_or_else(|| panic!("{id} missing from emitted observables"))
}

#[test]
fn every_registered_id_emits_exactly_once_in_stable_order() {
    let mut a = adapter(&base_initials());
    a.run_schedule_once();
    let obs = {
        let mut world = a.world();
        live_observables(&mut world)
    };
    let expected = all_live_observable_ids();
    assert_eq!(obs.len(), expected.len());
    for ((k, v), id) in obs.iter().zip(expected.iter()) {
        assert_eq!(k, id, "emission order drifted at {id}");
        assert!(v.is_finite(), "{id} must be finite");
    }
    // Every extended spec is registered with a unit and a real cadence (same property the frozen
    // preregistration enforces for the legacy twelve).
    let reg = anima_engine_lib::core::experiment::ObservableRegistry::live_default();
    for id in LIVE_EXTENDED_OBSERVABLE_IDS {
        let spec = reg.get(id).unwrap_or_else(|| panic!("{id} unregistered"));
        assert!(!spec.unit.is_empty());
        assert!(spec.cadence_period > 0);
    }
}

#[test]
fn shannon_and_simpson_match_the_index_math_on_the_same_abundances() {
    // OB-S01: the observable must be the panel's number, not merely a number.
    let mut a = adapter(&base_initials());
    a.run_schedule_once();
    use anima_engine_lib::core::ecs::{Agent, Predator, Prey};
    let (prey, predators) = {
        let mut world = a.world();
        let mut qp = world.query_filtered::<(), (With<Agent>, With<Prey>)>();
        let mut qd = world.query_filtered::<(), (With<Agent>, With<Predator>)>();
        (
            qp.iter(&world).count() as u32,
            qd.iter(&world).count() as u32,
        )
    };
    let obs = {
        let mut world = a.world();
        live_observables(&mut world)
    };
    let expected_shannon =
        anima_engine_lib::core::ecology::shannon_index(&[prey, predators]) as f64;
    let expected_simpson =
        anima_engine_lib::core::ecology::simpson_index(&[prey, predators]) as f64;
    assert_eq!(
        value_of(&obs, "live.shannon").to_bits(),
        expected_shannon.to_bits()
    );
    assert_eq!(
        value_of(&obs, "live.simpson").to_bits(),
        expected_simpson.to_bits()
    );
}

#[test]
fn mass_statistics_separates_populations_an_identical_mean_cannot() {
    // OB-S02: two samples, both mean 3.0 — flat vs split. The mean is blind to the difference;
    // the variance and the tails are not. This is exactly why a mean-only view cannot see a
    // population mid-divergence.
    let flat = [3.0f32, 3.0, 3.0, 3.0];
    let split = [0.0f32, 0.0, 6.0, 6.0];
    let f = mass_statistics(&flat);
    let s = mass_statistics(&split);
    assert!((f[0] - s[0]).abs() < 1e-12, "means agree by construction");
    assert!(s[1] > f[1], "variance must expose the split");
    assert!(
        f[3] - f[2] < s[3] - s[2],
        "the split population has wider tails"
    );

    // Percentile math on a hand-checkable sample: nearest-rank p10/p90 of 1..=10 are 1 and 9.
    let ramp: Vec<f32> = (1..=10).map(|i| i as f32).collect();
    let r = mass_statistics(&ramp);
    assert_eq!(r[2], 1.0);
    assert_eq!(r[3], 9.0);
    assert!(mass_statistics(&[])[1] == 0.0);
}

// ---------------------------------------------------------------------------------------
// OB-S03 / O-5 — turnover windows and the preflight refusal
// ---------------------------------------------------------------------------------------

fn mortality_manifest_initials(with_mortality: bool) -> InitialConditionSet {
    let mut v = vec![
        (LIVE_KEY_FOUNDERS.to_string(), 6.0),
        (LIVE_KEY_PREDATOR_FRACTION.to_string(), 0.0),
        (LIVE_KEY_TREES.to_string(), 0.0),
        ("live.lakes".to_string(), 0.0),
        (LIVE_KEY_FOOD_CAP.to_string(), 0.0),
    ];
    if with_mortality {
        v.push(("live.mortality".to_string(), 1.0));
        v.push((LIVE_KEY_STARVE_TICKS.to_string(), 2.0));
        v.push((LIVE_KEY_DEHYDRATION_TICKS.to_string(), 2.0));
    }
    InitialConditionSet::new(v)
}

/// Warm-up reserves: lets agents bank real `survival_ticks` before the sentence below.
fn endow_healthy(a: &mut LiveExperimentAdapter, energy: f32, hydration: f32) {
    use anima_engine_lib::ai::hrrl::HomeostaticState;
    use anima_engine_lib::core::ecs::Agent;
    let mut world = a.world();
    let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
    for mut homeo in q.iter_mut(&mut world) {
        homeo.energy = energy;
        homeo.hydration = hydration;
        homeo.starving_ticks = 0;
    }
}

/// EU-neutral death sentence: hydration is not an EU store; the counter is pre-advanced past the
/// declared threshold so the very next tick's mortality removes them through the sanctioned
/// primitive.
fn sentence_to_death(a: &mut LiveExperimentAdapter, threshold: u32) {
    use anima_engine_lib::ai::hrrl::HomeostaticState;
    use anima_engine_lib::core::ecs::Agent;
    let mut world = a.world();
    let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
    for mut homeo in q.iter_mut(&mut world) {
        homeo.hydration = 0.0;
        homeo.starving_ticks = threshold;
    }
}

#[test]
fn turnover_counts_are_windowed_not_cumulative() {
    let mut a = adapter(&mortality_manifest_initials(true));
    // Ten living ticks first: the dead must have LIVED, or a lifespan is meaningless.
    endow_healthy(&mut a, 500.0, 500.0);
    for _ in 0..10u32 {
        a.run_schedule_once();
    }
    sentence_to_death(&mut a, 2);
    a.run_schedule_once();

    let first = {
        let mut world = a.world();
        live_observables(&mut world)
    };
    assert_eq!(
        value_of(&first, "live.deaths"),
        6.0,
        "all six sentenced agents die in this tick"
    );
    let lifespan = value_of(&first, "live.mean_lifespan");
    assert!(
        lifespan >= 9.0,
        "the window's deaths carry their lifespans (got {lifespan})"
    );

    // Sampling DRAINS the window: the next sample reports no new events instead of replaying
    // the cumulative total. A cumulative count would answer "has death ever happened", which is
    // not what a turnover observable is for.
    let second = {
        let mut world = a.world();
        live_observables(&mut world)
    };
    assert_eq!(value_of(&second, "live.deaths"), 0.0);
    assert_eq!(value_of(&second, "live.mean_lifespan"), 0.0);
}

#[test]
fn archive_coverage_mirrors_the_grid_len() {
    let mut a = adapter(&base_initials());
    a.run_schedule_once();
    let grid_len = {
        let world = a.world();
        world
            .get_resource::<anima_engine_lib::core::agent_systems::BevyMapElitesArchive>()
            .map(|b| b.archive.grid.len() as f64)
            .expect("the adapter always inserts the MAP-Elites archive")
    };
    let obs = {
        let mut world = a.world();
        live_observables(&mut world)
    };
    assert_eq!(
        value_of(&obs, "live.archive_coverage").to_bits(),
        grid_len.to_bits(),
        "OB-S04: the observable must be exactly the archive's filled-cell count"
    );
}

#[test]
fn death_observables_are_refused_when_the_run_declares_no_death_rule() {
    // O-5 / OB-S05: a zero would look like a measurement. The refusal happens at the declared
    // config, before any tick runs.
    let off = mortality_manifest_initials(false);
    let cfg_off =
        anima_engine_lib::core::live_experiment::LiveWorldConfig::from_initial_conditions(&off)
            .expect("valid");
    for id in ["live.deaths", "live.mean_lifespan"] {
        let err = cfg_off
            .ensure_observables_supported(&[id.to_string()])
            .expect_err("death observables without a death rule must fail preflight");
        assert!(matches!(
            err,
            anima_engine_lib::core::experiment::ExperimentError::InvalidObservableRequest { .. }
        ));
    }

    let on = mortality_manifest_initials(true);
    let cfg_on =
        anima_engine_lib::core::live_experiment::LiveWorldConfig::from_initial_conditions(&on)
            .expect("valid");
    cfg_on
        .ensure_observables_supported(&[
            "live.deaths".to_string(),
            "live.mean_lifespan".to_string(),
            "live.births".to_string(),
            "live.shannon".to_string(),
        ])
        .expect("with a declared death rule every turnover observable is legal");

    // Unrelated ids are never refused by this check.
    cfg_off
        .ensure_observables_supported(&["live.agent_count".to_string()])
        .expect("legacy observables need nothing new");
}

// ---------------------------------------------------------------------------------------
// CA-S03 — the causal channel, honestly scoped
//
// A turnover run vs a frozen run, sampled at the SAME instant: the new observables name and
// count the deaths (windowed) while the legacy twelve — all instantaneous state aggregates —
// carry no event information at all. The strict historical form ("the legacy twelve provably
// cannot distinguish") holds only for quantities that are genuinely identical here; agent_count,
// for instance, DOES differ because deaths shrink the population. What the legacy set cannot do
// is separate "the population changed" from "selection acted", or say how long the dead lived —
// that is exactly what live.deaths / live.mean_lifespan add.
// ---------------------------------------------------------------------------------------

#[test]
fn turnover_is_visible_to_the_new_observables_and_invisible_to_the_legacy_semantics() {
    // Turnover world: mortality declared, everyone sentenced after ten living ticks.
    let mut a = adapter(&mortality_manifest_initials(true));
    endow_healthy(&mut a, 500.0, 500.0);
    for _ in 0..10u32 {
        a.run_schedule_once();
    }
    sentence_to_death(&mut a, 2);
    a.run_schedule_once();

    // Frozen world: identical ecology and identical sentence — but no death rule, so nobody
    // CAN die.
    let mut b = adapter(&mortality_manifest_initials(false));
    endow_healthy(&mut b, 500.0, 500.0);
    for _ in 0..10u32 {
        b.run_schedule_once();
    }
    sentence_to_death(&mut b, 2);
    b.run_schedule_once();

    let (obs_a, obs_b) = {
        let mut wa = a.world();
        let mut wb = b.world();
        (live_observables(&mut wa), live_observables(&mut wb))
    };

    assert_eq!(
        value_of(&obs_a, "live.deaths"),
        6.0,
        "the turnover run reports its six deaths"
    );
    assert_eq!(
        value_of(&obs_b, "live.deaths"),
        0.0,
        "the frozen run reports none"
    );
    assert!(
        value_of(&obs_a, "live.mean_lifespan") >= 9.0
            && value_of(&obs_b, "live.mean_lifespan") == 0.0,
        "lifespans exist only where deaths happened"
    );

    // And the legacy semantics really are silent about the EVENT: no legacy id encodes death,
    // birth or lifespan — grep the id list itself as the receipt.
    for id in anima_engine_lib::core::live_experiment::LIVE_OBSERVABLE_IDS {
        assert!(
            !id.contains("death") && !id.contains("birth") && !id.contains("lifespan"),
            "{id} claims turnover semantics it does not have"
        );
    }
}
