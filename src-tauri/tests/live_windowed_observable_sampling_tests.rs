//! The runner's sampled `live.births` / `live.deaths` must count every event of the sample period.
//!
//! `live.births`, `live.deaths` and `live.mean_lifespan` are *window* observables: each read of the
//! live world's observables drains `VitalStatistics`, so a value counts the events since the previous
//! read. The documented meaning is "since the previous sample". The shared runner, though, reads a
//! model's observables on **every** tick, to check that they are finite and unique, and keeps only the
//! reads that fall on a sample tick. Every read in between drains the window and is thrown away, so a
//! sampled value counts the events of its own tick only.
//!
//! The ground truth here needs no runner: the same seed and manifest, stepped the same way, with the
//! observables read on every tick and summed. The two runs are the same trajectory (reading
//! observables does not feed back into the world), so a correct runner's sampled counts add up to the
//! same totals.

use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{run_manifest_seed, ExperimentModel, RunStatus};
use anima_engine_lib::core::intervention::{InterventionCommand, InterventionQueue};
use anima_engine_lib::core::live_experiment::{LiveExperimentAdapter, LIVE_OBSERVABLE_IDS};
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use rand::SeedableRng;

const SEED: u64 = 1;
const TICKS: u64 = 3_000;
const SAMPLE_PERIOD: u64 = 60;

fn initial() -> InitialConditionSet {
    InitialConditionSet::new(vec![
        ("live.mortality".to_string(), 1.0),
        ("live.reproduction".to_string(), 1.0),
    ])
}

fn manifest() -> ExperimentManifest {
    let mut observable_ids: Vec<String> =
        LIVE_OBSERVABLE_IDS.iter().map(|s| s.to_string()).collect();
    observable_ids.extend(["live.births", "live.deaths"].map(String::from));
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "windowed-sampling".to_string(),
        name: "windowed-sampling".to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: initial(),
        interventions: vec![],
        seeds: vec![SEED],
        duration_ticks: TICKS,
        sample_period: SAMPLE_PERIOD,
        observable_ids,
        exotic_interventions: Vec::new(),
        observer: Default::default(),
        island: None,
    }
}

fn obs(o: &[(String, f64)], id: &str) -> f64 {
    o.iter()
        .find(|(k, _)| k == id)
        .unwrap_or_else(|| panic!("{id} missing"))
        .1
}

/// Births and deaths over the whole run, every tick read and summed, and the final checksum.
fn ground_truth() -> (f64, f64, u32) {
    let mut adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial(),
        &[],
        SEED,
        (16, 16),
        TICKS,
    )
    .expect("the live world builds");
    let mut clock = SimClock { tick: 0 };
    let mut ledger = CausalLedger::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);
    let queue = InterventionQueue::new(vec![]);
    let (mut births, mut deaths) = (0.0, 0.0);
    for _ in 0..TICKS {
        let tick = clock.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        adapter.step(&clock, &active, &mut ledger, &mut rng);
        let o = adapter.observables();
        births += obs(&o, "live.births");
        deaths += obs(&o, "live.deaths");
    }
    (births, deaths, adapter.checksum())
}

#[test]
fn sampled_births_and_deaths_add_up_to_every_event_of_the_run() {
    let (true_births, true_deaths, true_checksum) = ground_truth();
    assert!(
        true_births > 0.0 && true_deaths > 0.0,
        "the configuration must produce both events (births {true_births}, deaths {true_deaths})"
    );

    let registry = ObservableRegistry::live_default();
    let result =
        run_manifest_seed::<LiveExperimentAdapter>(&manifest(), &registry, SEED, None, None);
    assert_eq!(result.status, RunStatus::Completed, "{:?}", result.warnings);
    assert_eq!(
        result.final_checksum, true_checksum,
        "the runner and the hand-driven loop must be the same trajectory"
    );
    let sampled_births: f64 = result
        .series
        .iter()
        .map(|s| obs(&s.observables, "live.births"))
        .sum();
    let sampled_deaths: f64 = result
        .series
        .iter()
        .map(|s| obs(&s.observables, "live.deaths"))
        .sum();
    println!(
        "WINDOW true_births={true_births} sampled_births={sampled_births} true_deaths={true_deaths} \
         sampled_deaths={sampled_deaths} samples={}",
        result.series.len()
    );
    assert_eq!(
        (sampled_births, sampled_deaths),
        (true_births, true_deaths),
        "the samples must count every birth and death of their period, not only those of the sample tick"
    );
}

#[test]
fn a_peek_leaves_the_window_open_and_a_sample_closes_it() {
    use anima_engine_lib::core::resources::VitalStatistics;
    let mut v = VitalStatistics::default();
    v.record_birth();
    v.record_birth();
    v.record_death(30);
    assert_eq!(v.peek_window(), (2, 1, Some(30.0)));
    assert_eq!(
        v.peek_window(),
        (2, 1, Some(30.0)),
        "a peek must not close the window"
    );
    v.record_death(10);
    assert_eq!(
        v.sample_window(),
        (2, 2, Some(20.0)),
        "the sample counts everything since the last sample"
    );
    assert_eq!(v.peek_window(), (0, 0, None), "and closes the window");
}
