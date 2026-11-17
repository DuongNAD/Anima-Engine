//! Card M1 — a probe, not a gate: print world checksums so runs on different operating systems can
//! be compared.
//!
//! Every determinism gate in this repository compares runs on **one** machine: two runs in one
//! process, or the same binary in several processes (`live_cross_process_probe.rs`). None of them
//! can say whether the same seed and manifest give the same world on Windows, Linux and macOS, and
//! the project has so far only *assumed* they do not. This probe makes that a measurement. The
//! workflow `.github/workflows/determinism-matrix.yml` runs it on all three and
//! `scripts/paper/compare_checksums.mjs` builds the matrix and names the first point that differs.
//!
//! What it prints, one JSON object per line after the `M1 ` marker:
//!
//! - `model`: `live.probe` (the cross-process probe's world), `live.p1` (the declared predator/prey
//!   configuration of `predator_prey_coexistence_tests`, which adds steering, mortality and
//!   reproduction), or `reference` (`ReferenceEvolutionWorld` on the baseline manifest);
//! - `seed` and `tick`;
//! - `checksum`, the model's own `ExperimentModel::checksum`;
//! - `observables`, every observable except the withheld ones as `[name, value]`, the value
//!   formatted with `{:?}`. That is the shortest string that round-trips the `f64`, so two lines that
//!   agree as text agree bit for bit.
//!
//! **Withheld: every body-mass observable** (`live.trait_mass_*`). The project keeps the body-mass
//! line of experiments blind until a preregistration is frozen (E-SEL1 closed without an experiment;
//! QD-SEL1b is open), and this probe's output lands in CI logs and a committed table. The checksum
//! still covers that state, so a platform difference there is still caught, only not shown.
//!
//! By default it runs a quick subset (seed 1, ticks 0 and 180) so that the ordinary `cargo test`
//! stays cheap. `ANIMA_M1_FULL=1` runs the full matrix: seeds 1, 2, 3 and ticks 0, 180, 1800 and
//! 18000. Tick 0 is the world straight after genesis, before any schedule run, so a difference there
//! is in world generation rather than in the tick dynamics. The reference world has no tick-0 row:
//! the runner refuses a zero-tick manifest.
//! Run it in release, as the workflow does:
//!
//! ```text
//! ANIMA_M1_FULL=1 cargo test --release --test cross_platform_checksum_probe -- --nocapture
//! ```
//!
//! The test passes whatever the values are. It asserts only that it printed every row it promised
//! and that each row covers every observable the model emits, less the withheld ones.

use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{run_manifest_seed, ExperimentModel, RunStatus};
use anima_engine_lib::core::live_experiment::{
    all_live_observable_ids, live_observables, LiveExperimentAdapter, LIVE_KEY_DRIVE_FLOOR,
    LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS, LIVE_KEY_KILL_EFFICIENCY, LIVE_KEY_KILL_HYDRATION,
    LIVE_KEY_LAKES, LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY, LIVE_KEY_PREDATOR_FRACTION,
    LIVE_KEY_REPRODUCTION, LIVE_KEY_TARGET_MIN_ENERGY, LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
};
use anima_engine_lib::core::observer::ObserverPolicy;
use anima_engine_lib::core::reference_world::ReferenceEvolutionWorld;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use serde_json::json;

const FULL_ENV: &str = "ANIMA_M1_FULL";
const FULL_SEEDS: [u64; 3] = [1, 2, 3];
const FULL_TICKS: [u64; 4] = [0, 180, 1_800, 18_000];
const QUICK_SEEDS: [u64; 1] = [1];
const QUICK_TICKS: [u64; 2] = [0, 180];
/// The field grid both live configurations are built with, as in the other live probes.
const GRID: (usize, usize) = (16, 16);
/// Observables never printed (see the module docs). Matched as a substring of the name.
const WITHHELD: &str = "mass";

fn withheld(name: &str) -> bool {
    name.contains(WITHHELD)
}

fn initial(pairs: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect())
}

/// The world of `live_cross_process_probe.rs`: default rules, no mortality or reproduction.
fn live_probe_world() -> InitialConditionSet {
    initial(&[
        (LIVE_KEY_FOUNDERS, 10.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.3),
        (LIVE_KEY_TREES, 8.0),
        (LIVE_KEY_LAKES, 2.0),
        (LIVE_KEY_FOOD_CAP, 50.0),
    ])
}

/// The declared configuration of `predator_prey_coexistence_tests` (STATE §3.00 P1). It adds the
/// steering drive, mortality and reproduction, so it exercises far more floating-point code.
fn live_p1_world() -> InitialConditionSet {
    initial(&[
        (LIVE_KEY_FOUNDERS, 10.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.3),
        (LIVE_KEY_TREES, 8.0),
        (LIVE_KEY_LAKES, 2.0),
        (LIVE_KEY_MORTALITY, 1.0),
        (LIVE_KEY_REPRODUCTION, 1.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_THIRST_STEERING, 1.0),
        (LIVE_KEY_KILL_HYDRATION, 1.0),
        (LIVE_KEY_KILL_EFFICIENCY, 0.6),
        (LIVE_KEY_TARGET_MIN_ENERGY, 10.0),
    ])
}

/// `ReferenceEcosystem`'s defaults, exactly as the AE-S01 parity test builds them.
fn reference_initial() -> InitialConditionSet {
    initial(&[
        ("precip", 1.0),
        ("temperature", 0.5),
        ("npp", 1.0),
        ("plants", 100.0),
        ("herbivores", 40.0),
        ("predators", 8.0),
        ("detritus", 0.0),
    ])
}

fn reference_manifest(seed: u64, ticks: u64) -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "m1-reference".into(),
        name: "m1-reference".into(),
        observer: ObserverPolicy::default(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: reference_initial(),
        interventions: vec![],
        seeds: vec![seed],
        duration_ticks: ticks,
        sample_period: ticks,
        // The legacy three: every other id in the default registry needs an AE3 population or an
        // exotic law, which the baseline manifest does not declare.
        observable_ids: vec!["plants".into(), "herbivores".into(), "predators".into()],
        exotic_interventions: Vec::new(),
    }
}

fn emit(model: &str, seed: u64, tick: u64, checksum: u32, observables: &[(String, f64)]) {
    let obs: Vec<[String; 2]> = observables
        .iter()
        .filter(|(k, _)| !withheld(k))
        .map(|(k, v)| [k.clone(), format!("{v:?}")])
        .collect();
    let row = json!({
        "model": model,
        "seed": seed,
        "tick": tick,
        "checksum": checksum,
        "observables": obs,
    });
    println!("M1 {row}");
}

/// Run one live configuration to each checkpoint in turn and print a row at each.
fn live_rows(model: &str, world: &InitialConditionSet, seed: u64, ticks: &[u64]) -> usize {
    let last = *ticks.last().expect("at least one checkpoint");
    let mut adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        world,
        &[],
        seed,
        GRID,
        last,
    )
    .unwrap_or_else(|e| panic!("{model} seed {seed} builds: {e:?}"));
    let mut now = 0u64;
    let mut rows = 0;
    for &tick in ticks {
        while now < tick {
            adapter.run_schedule_once();
            now += 1;
        }
        let observables = {
            let mut w = adapter.world();
            live_observables(&mut w)
        };
        assert_eq!(
            observables.len(),
            all_live_observable_ids().len(),
            "a row must cover every observable the live adapter emits"
        );
        emit(model, seed, tick, adapter.checksum(), &observables);
        rows += 1;
    }
    rows
}

/// The reference world through the shared runner: one run per checkpoint, since a run to N ticks
/// is the state at N.
fn reference_rows(seed: u64, ticks: &[u64]) -> usize {
    let registry = ObservableRegistry::reference_default();
    let mut rows = 0;
    for &tick in ticks.iter().filter(|&&t| t > 0) {
        let manifest = reference_manifest(seed, tick);
        let result =
            run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, seed, None, None);
        assert_eq!(
            result.status,
            RunStatus::Completed,
            "reference seed {seed} tick {tick} must complete: {:?}",
            result.warnings
        );
        emit(
            "reference",
            seed,
            tick,
            result.final_checksum,
            &result.final_observables,
        );
        rows += 1;
    }
    rows
}

#[test]
fn m1_print_world_checksums_for_cross_platform_comparison() {
    let full = std::env::var(FULL_ENV).is_ok_and(|v| v == "1");
    let (seeds, ticks): (&[u64], &[u64]) = if full {
        (&FULL_SEEDS, &FULL_TICKS)
    } else {
        (&QUICK_SEEDS, &QUICK_TICKS)
    };
    let meta = json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "family": std::env::consts::FAMILY,
        "mode": if full { "full" } else { "quick" },
        "seeds": seeds,
        "ticks": ticks,
        "withheld": all_live_observable_ids()
            .into_iter()
            .filter(|n| withheld(n))
            .collect::<Vec<_>>(),
    });
    println!("M1_META {meta}");

    let mut rows = 0;
    for &seed in seeds {
        rows += live_rows("live.probe", &live_probe_world(), seed, ticks);
        rows += live_rows("live.p1", &live_p1_world(), seed, ticks);
        rows += reference_rows(seed, ticks);
    }
    let reference_ticks = ticks.iter().filter(|&&t| t > 0).count();
    assert_eq!(
        rows,
        seeds.len() * (2 * ticks.len() + reference_ticks),
        "the probe must print one row per model, seed and checkpoint"
    );
}
