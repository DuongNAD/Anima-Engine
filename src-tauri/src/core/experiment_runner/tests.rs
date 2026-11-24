use super::*;
use crate::core::experiment::{InitialConditionSet, WorldLawSet, MANIFEST_SCHEMA_VERSION};
use crate::core::reference_world::ReferenceEvolutionWorld;
use crate::core::scenario::{run_scenario, ReferenceEcosystem, Scenario};
use crate::core::world_artifact::WorldIdentity;
use std::sync::atomic::{AtomicUsize, Ordering};

fn ref_init() -> InitialConditionSet {
    // Exactly the ReferenceEcosystem defaults, so a baseline manifest run is byte-identical to a
    // legacy scenario run (AE-S01).
    InitialConditionSet::new(vec![
        ("precip".into(), 1.0),
        ("temperature".into(), 0.5),
        ("npp".into(), 1.0),
        ("plants".into(), 100.0),
        ("herbivores".into(), 40.0),
        ("predators".into(), 8.0),
        ("detritus".into(), 0.0),
    ])
}

fn baseline_manifest(seeds: Vec<u64>) -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "ref-baseline".into(),
        name: "ref-baseline".into(),
        observer: crate::core::observer::ObserverPolicy::default(),
        island: None,
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: ref_init(),
        interventions: vec![],
        seeds,
        duration_ticks: 6000,
        sample_period: 600,
        observable_ids: vec!["plants".into(), "herbivores".into(), "predators".into()],
        exotic_interventions: Vec::new(),
    }
}

static PREFLIGHT_MODEL_CONSTRUCTIONS: AtomicUsize = AtomicUsize::new(0);

/// A minimal model used only to prove invalid experiment inputs are rejected before the model
/// factory (and therefore before the runner seeds an RNG).
struct PreflightCountingModel;

impl ExperimentModel for PreflightCountingModel {
    type Snapshot = ();

    fn model_version() -> &'static str {
        "test-preflight-counting/1"
    }

    fn from_manifest(
        _laws: &WorldLawSet,
        _initial: &InitialConditionSet,
        _forcings: &[ExoticIntervention],
        _seed: u64,
        _grid: (usize, usize),
        _run_ticks: u64,
    ) -> Result<Self, ExperimentError> {
        PREFLIGHT_MODEL_CONSTRUCTIONS.fetch_add(1, Ordering::SeqCst);
        Ok(Self)
    }

    fn snapshot(&self) -> Self::Snapshot {}

    fn from_snapshot(_snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
        PREFLIGHT_MODEL_CONSTRUCTIONS.fetch_add(1, Ordering::SeqCst);
        Ok(Self)
    }

    fn step(
        &mut self,
        _clock: &SimClock,
        _active: &[&InterventionCommand],
        _ledger: &mut CausalLedger,
        _rng: &mut StdRng,
    ) {
    }

    fn checksum(&self) -> u32 {
        0
    }

    fn observables(&self) -> Vec<(String, f64)> {
        Vec::new()
    }
}

// ---- AE-S02: replay determinism ---------------------------------------------------------

#[test]
fn build_identity_is_more_specific_than_the_package_version() {
    let id = build_id();
    let version = env!("CARGO_PKG_VERSION");

    assert_ne!(
        id, version,
        "a package version alone aliases binaries built from different source revisions"
    );
    assert!(
        id.starts_with(&format!("{version}+")),
        "build identity must retain the human-readable package version: {id}"
    );
}

#[test]
fn ae_s02_same_manifest_and_seed_give_same_checksum() {
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![42]);
    let a = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 42, None, None);
    let b = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 42, None, None);
    assert_eq!(a.final_checksum, b.final_checksum);
    assert_eq!(a.final_observables, b.final_observables);
    assert_eq!(a.provenance.run_id, b.provenance.run_id);
    assert!(a.status.is_completed());
}

#[test]
fn run_identity_distinguishes_numerical_models() {
    let registry = ObservableRegistry::reference_default();
    let manifest = baseline_manifest(vec![42]);
    let reference =
        RunProvenance::derive::<ReferenceEvolutionWorld>(&manifest, &registry, 42, None, None);
    let other =
        RunProvenance::derive::<DuplicateObservableModel>(&manifest, &registry, 42, None, None);

    assert_ne!(reference.model_version, other.model_version);
    assert_ne!(
        reference.run_id, other.run_id,
        "two numerical models must not publish the same address for different runs"
    );
}

#[test]
fn run_identity_distinguishes_observable_registries() {
    let registry = ObservableRegistry::reference_default();
    let mut alternate_json =
        serde_json::to_value(&registry).expect("the default registry serializes");
    alternate_json["specs"][0]["display_name"] =
        serde_json::Value::String("alternate display contract".into());
    let alternate: ObservableRegistry =
        serde_json::from_value(alternate_json).expect("the alternate registry parses");
    alternate
        .validate()
        .expect("the alternate registry is valid");

    let manifest = baseline_manifest(vec![42]);
    let original =
        RunProvenance::derive::<ReferenceEvolutionWorld>(&manifest, &registry, 42, None, None);
    let changed =
        RunProvenance::derive::<ReferenceEvolutionWorld>(&manifest, &alternate, 42, None, None);

    assert_ne!(
        original.registry_fingerprint, changed.registry_fingerprint,
        "the control must actually change the registry contract"
    );
    assert_ne!(
        original.run_id, changed.run_id,
        "different measurement contracts must not share a run address"
    );
}

#[test]
fn run_identity_distinguishes_builds() {
    let first = run_identity_base("experiment", 1, 2, "model/1", "build-a", 42);
    let second = run_identity_base("experiment", 1, 2, "model/1", "build-b", 42);

    assert_ne!(
        first, second,
        "different binaries must not publish the same run address"
    );
}

// ---- AE-S01: baseline parity with the legacy scenario -----------------------------------

#[test]
fn ae_s01_baseline_manifest_matches_legacy_scenario_checksum() {
    let reg = ObservableRegistry::reference_default();
    let seed = 7u64;
    let manifest = baseline_manifest(vec![seed]);
    let via_manifest =
        run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &reg, seed, None, None);

    // The equivalent legacy scenario over the untouched ReferenceEcosystem.
    let scn = Scenario {
        name: "legacy".into(),
        seed,
        duration_ticks: 6000,
        sample_period: 600,
        interventions: vec![],
    };
    let legacy = run_scenario::<ReferenceEcosystem>(&scn);

    // exotic_energy = None must reproduce the legacy world byte-for-byte (checksum), with no MU
    // budget and no hidden field.
    assert_eq!(
        via_manifest.final_checksum, legacy.final_checksum,
        "baseline manifest must match the legacy scenario checksum"
    );
    assert!(via_manifest.exotic_budget.is_none());
    // The EU observables agree too.
    for name in ["plants", "herbivores", "predators", "detritus"] {
        let mv = via_manifest.observable(name).unwrap();
        let lv = legacy
            .final_observables
            .iter()
            .find(|(k, _)| k == name)
            .unwrap()
            .1;
        assert!((mv - lv).abs() < 1e-12, "{name}: {mv} vs {lv}");
    }
}

// ---- AE-108 / AE-S14: ensemble preserves failures + order -------------------------------

#[test]
fn ae_s14_ensemble_preserves_seed_order_and_summarizes() {
    let reg = ObservableRegistry::reference_default();
    let seeds = vec![10u64, 11, 12, 13, 14];
    let m = baseline_manifest(seeds.clone());
    let summary = run_ensemble::<ReferenceEvolutionWorld>(&m, &reg).expect("valid ensemble");
    assert_eq!(summary.seed_order, seeds);
    assert_eq!(summary.requested_runs, 5);
    assert_eq!(summary.completed_runs, 5);
    assert!(summary.failed.is_empty());
    // Each seed's run kept its own identity, in order.
    for (run, seed) in summary.runs.iter().zip(&seeds) {
        assert_eq!(run.provenance.seed, *seed);
    }
    // A summary exists for the requested observables with N and a CI.
    let plants = summary
        .metrics
        .iter()
        .find(|m| m.observable == "plants")
        .expect("plants metric");
    assert_eq!(plants.n, 5);
    let low = plants.ci95_low.expect("n=5 estimates a lower bound");
    let high = plants.ci95_high.expect("n=5 estimates an upper bound");
    assert!(low <= plants.mean && plants.mean <= high);
}

#[test]
fn ensemble_metrics_include_observables_first_emitted_by_later_seeds() {
    let registry = ObservableRegistry::reference_default();
    let manifest = baseline_manifest(vec![10, 11]);
    let mut first =
        run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 10, None, None);
    let mut second =
        run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 11, None, None);
    first.final_observables = vec![("shared".into(), 1.0), ("first-only".into(), 10.0)];
    second.final_observables = vec![("shared".into(), 3.0), ("later-only".into(), 20.0)];

    let metrics = summarize_metrics(&[&first, &second]).expect("finite metric summaries");
    let names: Vec<&str> = metrics
        .iter()
        .map(|metric| metric.observable.as_str())
        .collect();

    assert_eq!(names, ["shared", "first-only", "later-only"]);
    assert_eq!(metrics[0].n, 2);
    assert_eq!(metrics[0].mean, 2.0);
    assert_eq!(metrics[1].n, 1);
    assert_eq!(metrics[2].n, 1);
}

struct DuplicateObservableModel;

impl ExperimentModel for DuplicateObservableModel {
    type Snapshot = ();

    fn model_version() -> &'static str {
        "test-duplicate-observable/1"
    }

    fn from_manifest(
        _laws: &WorldLawSet,
        _initial: &InitialConditionSet,
        _forcings: &[ExoticIntervention],
        _seed: u64,
        _grid: (usize, usize),
        _run_ticks: u64,
    ) -> Result<Self, ExperimentError> {
        Ok(Self)
    }

    fn snapshot(&self) -> Self::Snapshot {}

    fn from_snapshot(_snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
        Ok(Self)
    }

    fn step(
        &mut self,
        _clock: &SimClock,
        _active: &[&InterventionCommand],
        _ledger: &mut CausalLedger,
        _rng: &mut StdRng,
    ) {
    }

    fn checksum(&self) -> u32 {
        1
    }

    fn observables(&self) -> Vec<(String, f64)> {
        vec![("ambiguous".into(), 1.0), ("ambiguous".into(), 2.0)]
    }
}

#[test]
fn a_duplicate_observable_name_fails_the_run_instead_of_choosing_one_value() {
    let registry = ObservableRegistry::reference_default();
    let mut manifest = baseline_manifest(vec![10]);
    manifest.duration_ticks = 1;
    manifest.sample_period = 1;

    let result =
        run_manifest_seed::<DuplicateObservableModel>(&manifest, &registry, 10, None, None);

    assert!(matches!(
        result.status,
        RunStatus::Failed { tick: 1, ref reason, .. }
            if reason.contains("observable 'ambiguous' was emitted more than once")
    ));
}

struct InvalidBudgetModel {
    seed: u64,
}

impl ExperimentModel for InvalidBudgetModel {
    type Snapshot = u64;

    fn model_version() -> &'static str {
        "test-non-finite-budget/1"
    }

    fn from_manifest(
        _laws: &WorldLawSet,
        _initial: &InitialConditionSet,
        _forcings: &[ExoticIntervention],
        seed: u64,
        _grid: (usize, usize),
        _run_ticks: u64,
    ) -> Result<Self, ExperimentError> {
        Ok(Self { seed })
    }

    fn snapshot(&self) -> Self::Snapshot {
        self.seed
    }

    fn from_snapshot(snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
        Ok(Self { seed: *snapshot })
    }

    fn step(
        &mut self,
        _clock: &SimClock,
        _active: &[&InterventionCommand],
        _ledger: &mut CausalLedger,
        _rng: &mut StdRng,
    ) {
    }

    fn checksum(&self) -> u32 {
        7
    }

    fn observables(&self) -> Vec<(String, f64)> {
        vec![
            ("plants".into(), 1.0),
            ("herbivores".into(), 1.0),
            ("predators".into(), 1.0),
        ]
    }

    fn exotic_budget(&self) -> Option<ExoticEnergyBudget> {
        let mut budget = ExoticEnergyBudget {
            unit: crate::core::exotic_energy::UnitId::new(crate::core::exotic_energy::MU_UNIT),
            initial: 1.0,
            sourced: 0.0,
            field: 1.0,
            organism_storage: 0.0,
            dissipated: 0.0,
            exported: 0.0,
        };
        match self.seed {
            10 => budget.sourced = f64::NAN,
            11 => budget.field = -1.0,
            12 => budget.unit = crate::core::exotic_energy::UnitId::new("undeclared-budget-unit"),
            13 => {
                budget.initial = f64::MAX;
                budget.sourced = f64::MAX;
            }
            _ => {}
        }
        Some(budget)
    }
}

#[test]
fn a_non_finite_exotic_budget_fails_and_remains_json_round_trippable() {
    let registry = ObservableRegistry::reference_default();
    let mut manifest = baseline_manifest(vec![10]);
    manifest.duration_ticks = 1;
    manifest.sample_period = 1;
    manifest.laws.exotic_energy = Some(crate::core::exotic_energy::ExoticEnergyLaw::mana_uniform(
        1.0,
    ));

    let result = run_manifest_seed::<InvalidBudgetModel>(&manifest, &registry, 10, None, None);

    assert!(matches!(
        result.status,
        RunStatus::Failed { tick: 1, ref reason, .. }
            if reason.contains("exotic budget")
                && reason.contains("sourced")
                && reason.contains("non-finite")
    ));
    assert!(
        result.exotic_budget.is_none(),
        "an invalid budget must not survive into a JSON artifact"
    );
    let encoded = serde_json::to_vec(&result).expect("failed result serializes");
    assert!(
        !encoded
            .windows(b"\"sourced\":null".len())
            .any(|window| window == b"\"sourced\":null"),
        "invalid numeric fields must not be erased into JSON null: {}",
        String::from_utf8_lossy(&encoded)
    );
    serde_json::from_slice::<RunResult>(&encoded).expect("public result schema round-trips");
}

#[test]
fn a_negative_exotic_budget_compartment_fails_the_run() {
    let registry = ObservableRegistry::reference_default();
    let mut manifest = baseline_manifest(vec![11]);
    manifest.duration_ticks = 1;
    manifest.sample_period = 1;
    manifest.laws.exotic_energy = Some(crate::core::exotic_energy::ExoticEnergyLaw::mana_uniform(
        1.0,
    ));

    let result = run_manifest_seed::<InvalidBudgetModel>(&manifest, &registry, 11, None, None);

    assert!(matches!(
        result.status,
        RunStatus::Failed { tick: 1, ref reason, .. }
            if reason.contains("exotic budget")
                && reason.contains("field")
                && reason.contains("negative")
    ));
    assert!(result.exotic_budget.is_none());
}

#[test]
fn an_exotic_budget_with_an_undeclared_unit_fails_the_run() {
    let registry = ObservableRegistry::reference_default();
    let mut manifest = baseline_manifest(vec![12]);
    manifest.duration_ticks = 1;
    manifest.sample_period = 1;
    manifest.laws.exotic_energy = Some(crate::core::exotic_energy::ExoticEnergyLaw::mana_uniform(
        1.0,
    ));

    let result = run_manifest_seed::<InvalidBudgetModel>(&manifest, &registry, 12, None, None);

    assert!(matches!(
        result.status,
        RunStatus::Failed { tick: 1, ref reason, .. }
            if reason.contains("exotic budget unit")
                && reason.contains("undeclared-budget-unit")
                && reason.contains(crate::core::exotic_energy::MU_UNIT)
    ));
    assert!(result.exotic_budget.is_none());
}

#[test]
fn a_budget_whose_finite_compartments_overflow_its_balance_fails_the_run() {
    let registry = ObservableRegistry::reference_default();
    let mut manifest = baseline_manifest(vec![13]);
    manifest.duration_ticks = 1;
    manifest.sample_period = 1;
    manifest.laws.exotic_energy = Some(crate::core::exotic_energy::ExoticEnergyLaw::mana_uniform(
        1.0,
    ));

    let result = run_manifest_seed::<InvalidBudgetModel>(&manifest, &registry, 13, None, None);

    assert!(matches!(
        result.status,
        RunStatus::Failed { tick: 1, ref reason, .. }
            if reason.contains("exotic budget balance error")
                && reason.contains("non-finite")
    ));
    assert!(result.exotic_budget.is_none());
}

#[test]
fn ensemble_metrics_keep_finite_extreme_samples_json_safe() {
    let registry = ObservableRegistry::reference_default();
    let manifest = baseline_manifest(vec![10, 11]);
    let mut first =
        run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 10, None, None);
    let mut second =
        run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 11, None, None);
    first.final_observables = vec![("extreme".into(), f64::MAX)];
    second.final_observables = vec![("extreme".into(), f64::MAX)];

    let metrics = summarize_metrics(&[&first, &second]).expect("finite extreme summary");
    let extreme = metrics.first().expect("extreme metric");

    assert_eq!(extreme.mean, f64::MAX);
    assert_eq!(extreme.std, Some(0.0));
    assert!(extreme.ci95_low.is_some_and(f64::is_finite));
    assert!(extreme.ci95_high.is_some_and(f64::is_finite));
    let encoded = serde_json::to_string(&metrics).expect("metric summary must serialize");
    assert!(
        !encoded.contains("null"),
        "finite samples must not become JSON null: {encoded}"
    );
}

#[test]
fn paired_effect_keeps_finite_extreme_samples_json_safe() {
    let effect = paired_effect(
        "extreme",
        2,
        &[f64::MAX, f64::MAX],
        &[f64::MAX, f64::MAX],
        &[0.0, 0.0],
    )
    .expect("finite paired effect");

    assert_eq!(effect.control_mean, Some(f64::MAX));
    assert_eq!(effect.treatment_mean, Some(f64::MAX));
    assert_eq!(effect.paired_mean_delta, Some(0.0));
    assert_eq!(effect.paired_sd, Some(0.0));
    assert!(
        [
            effect.control_mean,
            effect.treatment_mean,
            effect.paired_mean_delta,
            effect.paired_sd,
            effect.paired_se,
            effect.ci95_low,
            effect.ci95_high,
            effect.paired_dz,
        ]
        .into_iter()
        .flatten()
        .all(f64::is_finite),
        "every defined paired statistic must be finite"
    );
    let encoded = serde_json::to_value(&effect).expect("paired effect must serialize");
    for field in [
        "control_mean",
        "treatment_mean",
        "paired_mean_delta",
        "paired_sd",
        "paired_se",
        "ci95_low",
        "ci95_high",
    ] {
        assert!(
            encoded[field].as_f64().is_some(),
            "defined field {field} must remain numeric JSON: {encoded}"
        );
    }
    assert!(
        encoded["paired_dz"].is_null(),
        "zero paired variance makes dz undefined"
    );
}

#[test]
fn independent_effect_keeps_large_finite_variance_and_interval() {
    let effect = effect_size("large", &[1.0e200, 3.0e200], &[2.0e200, 4.0e200])
        .expect("finite independent effect");

    assert_eq!(effect.control_mean, 2.0e200);
    assert_eq!(effect.treatment_mean, 3.0e200);
    assert!(effect
        .pooled_sd
        .is_some_and(|value| value.is_finite() && value > 0.0));
    assert!(effect.hedges_g.is_some_and(f64::is_finite));
    assert!(effect.ci95_low.is_some_and(f64::is_finite));
    assert!(effect.ci95_high.is_some_and(f64::is_finite));
    let encoded = serde_json::to_string(&effect).expect("effect size must serialize");
    assert!(
        !encoded.contains("null"),
        "finite effect statistics must not become JSON null: {encoded}"
    );
}

#[test]
fn aggregate_reports_unrepresentable_finite_result_as_structured_error() {
    let error = finite_sample_stats("opposite-extremes", &[-f64::MAX, f64::MAX])
        .expect_err("sample SD exceeds finite f64");

    assert_eq!(
        error,
        ExperimentError::AggregateNotFinite {
            observable: "opposite-extremes".into(),
            statistic: "sample standard deviation".into(),
        }
    );
    let encoded = serde_json::to_value(&error).expect("structured error must serialize");
    assert_eq!(
        encoded["AggregateNotFinite"]["observable"],
        "opposite-extremes"
    );
    assert_eq!(
        encoded["AggregateNotFinite"]["statistic"],
        "sample standard deviation"
    );
}

#[test]
fn singleton_metric_does_not_fabricate_spread_or_confidence() {
    let registry = ObservableRegistry::reference_default();
    let manifest = baseline_manifest(vec![10]);
    let mut run =
        run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 10, None, None);
    run.final_observables = vec![("pilot".into(), 42.0)];

    let metrics = summarize_metrics(&[&run]).expect("singleton descriptive summary");
    let encoded = serde_json::to_value(&metrics[0]).expect("metric must serialize");

    assert_eq!(encoded["mean"], 42.0);
    assert_eq!(encoded["min"], 42.0);
    assert_eq!(encoded["max"], 42.0);
    assert!(encoded["std"].is_null(), "one sample has no sample SD");
    assert!(
        encoded["ci95_low"].is_null() && encoded["ci95_high"].is_null(),
        "one sample cannot estimate a confidence interval: {encoded}"
    );
}

#[test]
fn singleton_independent_groups_do_not_fabricate_inference() {
    let effect =
        effect_size("pilot", &[1.0], &[3.0]).expect("raw singleton difference is descriptive");
    let encoded = serde_json::to_value(&effect).expect("effect must serialize");

    assert_eq!(encoded["control_mean"], 1.0);
    assert_eq!(encoded["treatment_mean"], 3.0);
    assert_eq!(encoded["mean_difference"], 2.0);
    for field in ["pooled_sd", "hedges_g", "ci95_low", "ci95_high"] {
        assert!(
            encoded[field].is_null(),
            "{field} is not estimable from singleton groups: {encoded}"
        );
    }

    let asymmetric = effect_size("pilot", &[1.0], &[2.0, 4.0]).expect("raw asymmetric difference");
    let asymmetric_json =
        serde_json::to_value(&asymmetric).expect("asymmetric effect must serialize");
    assert_eq!(asymmetric_json["mean_difference"], 2.0);
    for field in ["pooled_sd", "hedges_g", "ci95_low", "ci95_high"] {
        assert!(
            asymmetric_json[field].is_null(),
            "{field} needs variance estimates from both groups: {asymmetric_json}"
        );
    }
}

#[test]
fn zero_pooled_sd_does_not_claim_zero_standardized_effect() {
    let effect = effect_size("constant", &[1.0, 1.0], &[3.0, 3.0]).expect("finite raw difference");
    let encoded = serde_json::to_value(&effect).expect("effect must serialize");

    assert_eq!(encoded["pooled_sd"], 0.0);
    assert!(
        encoded["hedges_g"].is_null(),
        "a nonzero difference divided by zero spread is undefined, not zero"
    );
    assert_eq!(encoded["ci95_low"], 2.0);
    assert_eq!(encoded["ci95_high"], 2.0);
}

#[test]
fn student_t_critical_values_follow_the_nist_95_percent_table() {
    assert!((conservative_student_t_critical_95(1.0) - 12.706_204_736_432_095).abs() < 1e-12);
    assert!((conservative_student_t_critical_95(2.0) - 4.302_652_729_696_142).abs() < 1e-12);
    assert!((conservative_student_t_critical_95(30.0) - 2.042_272_456_301_238).abs() < 1e-12);

    let mut previous = f64::INFINITY;
    for df in 1..=30 {
        let current = conservative_student_t_critical_95(df as f64);
        assert!(current < previous, "critical values must fall as df grows");
        previous = current;
    }

    assert_eq!(
        conservative_student_t_critical_95(1.99),
        conservative_student_t_critical_95(1.0),
        "fractional Welch df uses the conservative lower integer"
    );
    assert!(
        conservative_student_t_critical_95(1_000_000.0) > 1.96,
        "large-df fallback remains conservative relative to the normal limit"
    );
    assert!(conservative_student_t_critical_95(0.0).is_nan());
    assert!(conservative_student_t_critical_95(f64::NAN).is_nan());
}

#[test]
fn small_sample_metric_ci_uses_student_t_not_normal_cutoff() {
    let registry = ObservableRegistry::reference_default();
    let manifest = baseline_manifest(vec![10, 11]);
    let mut first =
        run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 10, None, None);
    let mut second =
        run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 11, None, None);
    first.final_observables = vec![("pilot".into(), 0.0)];
    second.final_observables = vec![("pilot".into(), 2.0)];

    let metrics = summarize_metrics(&[&first, &second]).expect("two-sample summary");
    let pilot = metrics.first().expect("pilot summary");
    // For [0, 2], mean=1, sample SD=sqrt(2), SE=1 and df=1. NIST t(0.975, 1).
    let critical = 12.706_204_736_432_095;
    assert!((pilot.ci95_low.expect("lower bound") - (1.0 - critical)).abs() < 1e-12);
    assert!((pilot.ci95_high.expect("upper bound") - (1.0 + critical)).abs() < 1e-12);
    let encoded = serde_json::to_value(pilot).expect("summary JSON");
    assert_eq!(encoded["ci95_method"], "student_t_conservative_table");
}

#[test]
fn small_sample_paired_ci_uses_student_t_not_normal_cutoff() {
    let effect =
        paired_effect("pilot", 2, &[0.0, 0.0], &[0.0, 2.0], &[0.0, 2.0]).expect("paired effect");

    // The paired deltas [0, 2] have mean=1, SE=1 and df=1.
    let critical = 12.706_204_736_432_095;
    assert!((effect.ci95_low.expect("lower bound") - (1.0 - critical)).abs() < 1e-12);
    assert!((effect.ci95_high.expect("upper bound") - (1.0 + critical)).abs() < 1e-12);
    let encoded = serde_json::to_value(&effect).expect("paired JSON");
    assert_eq!(
        encoded["ci95_method"],
        "paired_student_t_conservative_table"
    );
}

#[test]
fn independent_ci_uses_welch_degrees_of_freedom() {
    let effect = effect_size("pilot", &[0.0, 2.0], &[2.0, 6.0]).expect("independent effect");

    // SE² = 1 + 4 = 5 and Welch df = 25/17 ≈ 1.47. The deterministic table uses the
    // conservative lower integer df=1, rather than the pooled df=2 or a normal cutoff.
    let half = 12.706_204_736_432_095 * 5.0_f64.sqrt();
    assert!((effect.ci95_low.expect("lower bound") - (3.0 - half)).abs() < 1e-12);
    assert!((effect.ci95_high.expect("upper bound") - (3.0 + half)).abs() < 1e-12);
    let encoded = serde_json::to_value(&effect).expect("independent JSON");
    assert_eq!(encoded["ci95_method"], "welch_student_t_conservative_table");
}

#[test]
fn optional_inference_fields_read_existing_numeric_artifacts() {
    let metric: MetricSummary = serde_json::from_value(serde_json::json!({
        "observable": "legacy",
        "n": 5,
        "mean": 10.0,
        "std": 2.0,
        "min": 7.0,
        "max": 13.0,
        "ci95_low": 8.0,
        "ci95_high": 12.0
    }))
    .expect("existing numeric metric artifact");
    assert_eq!(metric.std, Some(2.0));
    assert_eq!(metric.ci95_low, Some(8.0));
    assert_eq!(metric.ci95_high, Some(12.0));
    assert_eq!(metric.ci95_method, None);

    let effect: EffectSize = serde_json::from_value(serde_json::json!({
        "observable": "legacy",
        "n_control": 5,
        "n_treatment": 5,
        "control_mean": 10.0,
        "treatment_mean": 12.0,
        "mean_difference": 2.0,
        "pooled_sd": 1.5,
        "hedges_g": 1.2,
        "ci95_low": 0.5,
        "ci95_high": 3.5
    }))
    .expect("existing numeric effect artifact");
    assert_eq!(effect.pooled_sd, Some(1.5));
    assert_eq!(effect.hedges_g, Some(1.2));
    assert_eq!(effect.ci95_low, Some(0.5));
    assert_eq!(effect.ci95_high, Some(3.5));
    assert_eq!(effect.ci95_method, None);
}

// A test-double model that fails to construct for one poisoned seed, proving the ensemble keeps
// failed runs (rather than dropping them) with a deterministic order — AE-108 failure-preservation.
struct FlakyModel(ReferenceEvolutionWorld);
impl ExperimentModel for FlakyModel {
    type Snapshot = <ReferenceEvolutionWorld as ExperimentModel>::Snapshot;

    fn model_version() -> &'static str {
        "test-flaky-reference/1"
    }

    fn from_manifest(
        laws: &WorldLawSet,
        initial: &InitialConditionSet,
        forcings: &[ExoticIntervention],
        seed: u64,
        grid: (usize, usize),
        run_ticks: u64,
    ) -> Result<Self, ExperimentError> {
        if seed == 12 {
            return Err(ExperimentError::EmptyField {
                field: "poisoned-seed-12".into(),
            });
        }
        Ok(FlakyModel(ReferenceEvolutionWorld::from_manifest(
            laws, initial, forcings, seed, grid, run_ticks,
        )?))
    }
    fn snapshot(&self) -> Self::Snapshot {
        self.0.snapshot()
    }
    fn from_snapshot(snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
        Ok(FlakyModel(ReferenceEvolutionWorld::from_snapshot(
            snapshot,
        )?))
    }
    fn step(
        &mut self,
        clock: &SimClock,
        active: &[&InterventionCommand],
        ledger: &mut CausalLedger,
        rng: &mut StdRng,
    ) {
        self.0.step(clock, active, ledger, rng)
    }
    fn checksum(&self) -> u32 {
        self.0.checksum()
    }
    fn observables(&self) -> Vec<(String, f64)> {
        self.0.observables()
    }
}

#[test]
fn ae_s14_ensemble_keeps_failed_runs() {
    let reg = ObservableRegistry::reference_default();
    let seeds = vec![10u64, 11, 12, 13, 14];
    let m = baseline_manifest(seeds.clone());
    let summary = run_ensemble::<FlakyModel>(&m, &reg).expect("valid ensemble");
    assert_eq!(summary.requested_runs, 5);
    assert_eq!(summary.completed_runs, 4);
    assert_eq!(summary.failed.len(), 1);
    assert_eq!(summary.failed[0].0, 12);
    // The failed run is still present in `runs`, in order, not dropped.
    assert_eq!(summary.runs.len(), 5);
    assert!(matches!(summary.runs[2].status, RunStatus::Failed { .. }));
}

struct PanickingModel {
    inner: ReferenceEvolutionWorld,
    panic_during_step: bool,
}

impl ExperimentModel for PanickingModel {
    type Snapshot = <ReferenceEvolutionWorld as ExperimentModel>::Snapshot;

    fn model_version() -> &'static str {
        "test-panicking-reference/1"
    }

    fn from_manifest(
        laws: &WorldLawSet,
        initial: &InitialConditionSet,
        forcings: &[ExoticIntervention],
        seed: u64,
        grid: (usize, usize),
        run_ticks: u64,
    ) -> Result<Self, ExperimentError> {
        Ok(Self {
            inner: ReferenceEvolutionWorld::from_manifest(
                laws, initial, forcings, seed, grid, run_ticks,
            )?,
            panic_during_step: seed == 12,
        })
    }

    fn snapshot(&self) -> Self::Snapshot {
        self.inner.snapshot()
    }

    fn from_snapshot(snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
        Ok(Self {
            inner: ReferenceEvolutionWorld::from_snapshot(snapshot)?,
            panic_during_step: false,
        })
    }

    fn step(
        &mut self,
        clock: &SimClock,
        active: &[&InterventionCommand],
        ledger: &mut CausalLedger,
        rng: &mut StdRng,
    ) {
        assert!(
            !(self.panic_during_step && clock.tick == 120),
            "poisoned seed panicked deliberately"
        );
        self.inner.step(clock, active, ledger, rng);
    }

    fn checksum(&self) -> u32 {
        self.inner.checksum()
    }

    fn observables(&self) -> Vec<(String, f64)> {
        self.inner.observables()
    }
}

#[test]
fn ae_s14_a_panicking_seed_is_preserved_without_destroying_the_ensemble() {
    let registry = ObservableRegistry::reference_default();
    let seeds = vec![10, 11, 12, 13];
    let mut manifest = baseline_manifest(seeds.clone());
    manifest.sample_period = 10;
    let summary = run_ensemble::<PanickingModel>(&manifest, &registry).expect("manifest is valid");

    assert_eq!(summary.seed_order, seeds);
    assert_eq!(summary.completed_runs, 3);
    assert_eq!(summary.failed.len(), 1);
    assert_eq!(summary.failed[0].0, 12);
    assert!(matches!(
        &summary.runs[2].status,
        RunStatus::Failed { tick: 120, reason, checksum: 0 } if reason.contains("panicked")
    ));
    assert_eq!(
        summary.runs[2].series.len(),
        11,
        "samples through tick 110 must survive the panic at tick 120"
    );
}

// ---- AUDIT: AE-209 checkpoint exotic channel ---------------------------------------------

fn exotic_removal(id: u32, start: u64, dur: u64) -> ExoticIntervention {
    use crate::core::exotic_energy::ExoticInterventionKind;
    use crate::core::intervention::Region;
    ExoticIntervention {
        id,
        cause_id: 900 + id,
        kind: ExoticInterventionKind::RemoveSource,
        region: Region::Global,
        start_tick: start,
        duration_ticks: dur,
        amount: 0.5,
        curve: crate::core::intervention::Curve::Step,
    }
}

fn mana_manifest_for_fork(seeds: Vec<u64>) -> ExperimentManifest {
    use crate::core::exotic_energy::ExoticEnergyLaw;
    let mut m = baseline_manifest(seeds);
    m.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(150.0, 4));
    m
}

#[test]
fn ck_exotic_channel_diverges_only_after_the_fork_and_keeps_identities() {
    use crate::core::exotic_energy::ExoticInterventionKind;
    use crate::core::intervention::Curve;

    let reg = ObservableRegistry::reference_default();
    let mut m = mana_manifest_for_fork(vec![7]);
    // Observe both the last ecology tick before the treatment first fires and the firing tick.
    m.sample_period = 60;
    let fork_tick = 3000u64;
    let extra = exotic_removal(1, 3120, 60);

    let rep = checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
        &m,
        &reg,
        7,
        fork_tick,
        &[],
        std::slice::from_ref(&extra),
    )
    .expect("fork ok");

    // The report carries the STRUCTURED exotic extras, not display strings.
    assert_eq!(rep.treatment_extra_exotic, vec![extra.clone()]);

    // Effective-treatment provenance: treatment fingerprints the base + exotic extras manifest;
    // prefix and control keep the base identity.
    let mut effective = m.clone();
    effective.exotic_interventions.push(extra);
    assert_eq!(
        rep.treatment.provenance.manifest_fingerprint,
        effective.fingerprint()
    );
    assert_eq!(rep.control.provenance.manifest_fingerprint, m.fingerprint());
    assert_eq!(rep.prefix.provenance.manifest_fingerprint, m.fingerprint());
    assert_ne!(
        rep.treatment.provenance.manifest_fingerprint,
        rep.control.provenance.manifest_fingerprint
    );

    // A runtime forcing NEVER changes the law (ER01).
    assert_eq!(
        rep.treatment.provenance.law_fingerprint,
        rep.control.provenance.law_fingerprint
    );

    // Control continuation is bit-for-bit the uninterrupted control run (no prefix re-sim).
    let uninterrupted = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 7, None, None);
    assert_eq!(rep.control.final_checksum, uninterrupted.final_checksum);
    // The treatment diverges because of the post-fork suppression.
    assert_ne!(rep.treatment.final_checksum, rep.control.final_checksum);
    assert!(
        rep.treatment.observable("exotic.density_total").unwrap()
            < rep.control.observable("exotic.density_total").unwrap(),
        "suppressing the source after the fork must leave less MU"
    );

    let sampled = |run: &RunResult, tick: u64, name: &str| {
        run.series
            .iter()
            .find(|sample| sample.tick == tick)
            .and_then(|sample| {
                sample
                    .observables
                    .iter()
                    .find(|(observable, _)| observable == name)
                    .map(|(_, value)| *value)
            })
            .unwrap_or_else(|| panic!("missing {name} sample at tick {tick}"))
    };
    assert_eq!(
        sampled(&rep.control, 3060, "exotic.density_total"),
        sampled(&rep.treatment, 3060, "exotic.density_total"),
        "branches must remain identical through the last ecology firing before the extra forcing"
    );
    assert!(
        sampled(&rep.treatment, 3120, "exotic.density_total")
            < sampled(&rep.control, 3120, "exotic.density_total"),
        "RemoveSource must first diverge on its applicable post-fork ecology firing"
    );

    // MU still closes; EU is untouched by the exotic treatment.
    let b = rep.treatment.exotic_budget.clone().expect("budget");
    assert!(b.balance_error().abs() / b.throughput() < 1e-4);
    for name in ["plants", "herbivores", "predators", "detritus"] {
        assert_eq!(
            rep.control.observable(name).unwrap(),
            rep.treatment.observable(name).unwrap(),
            "{name} must be unaffected by an exotic forcing"
        );
    }

    // Attribution under the forcing's own cause id.
    assert!(rep
        .treatment
        .ledger
        .all()
        .iter()
        .any(|e| e.target == "exotic.source_suppressed" && e.cause_id == 901));
    // ...and no such record exists before/at the fork in the shared prefix.
    assert!(!rep
        .prefix
        .ledger
        .all()
        .iter()
        .any(|e| e.target == "exotic.source_suppressed"));

    // The other two checkpoint treatment kinds inject (rather than suppress) MU, keep the same
    // immutable law, preserve the EU state, and attribute movement to their own cause ids.
    for (id, kind, curve) in [
        (2, ExoticInterventionKind::AddSource, Curve::Step),
        (3, ExoticInterventionKind::Pulse, Curve::RampDown),
    ] {
        let mut injection = exotic_removal(id, 3120, 60);
        injection.kind = kind;
        injection.curve = curve;
        injection.amount = 0.25;
        let injected = checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            fork_tick,
            &[],
            &[injection],
        )
        .expect("injection fork");
        assert_eq!(
            sampled(&injected.control, 3060, "exotic.density_total"),
            sampled(&injected.treatment, 3060, "exotic.density_total")
        );
        assert!(
            sampled(&injected.treatment, 3120, "exotic.density_total")
                > sampled(&injected.control, 3120, "exotic.density_total"),
            "{kind:?} must inject MU on its first applicable post-fork ecology firing"
        );
        assert_eq!(
            injected.treatment.provenance.law_fingerprint,
            injected.control.provenance.law_fingerprint
        );
        assert!(injected
            .treatment
            .ledger
            .all()
            .iter()
            .any(|entry| entry.target == "exotic.forcing" && entry.cause_id == 900 + id));
        for name in ["plants", "herbivores", "predators", "detritus"] {
            assert_eq!(
                injected.control.observable(name),
                injected.treatment.observable(name),
                "{kind:?} must not alter the EU model"
            );
        }
    }
}

#[test]
fn ck_exotic_channel_replays_deterministically_and_round_trips() {
    let reg = ObservableRegistry::reference_default();
    let m = mana_manifest_for_fork(vec![7]);
    let extra = exotic_removal(1, 3060, 600);
    let a = checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
        &m,
        &reg,
        7,
        3000,
        &[],
        std::slice::from_ref(&extra),
    )
    .expect("ok");
    let b =
        checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(&m, &reg, 7, 3000, &[], &[extra])
            .expect("ok");
    assert_eq!(a.treatment.final_checksum, b.treatment.final_checksum);
    assert_eq!(a.control.final_checksum, b.control.final_checksum);

    let json = serde_json::to_string(&a).expect("serializes");
    let back: CheckpointForkReport = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(back.treatment_extra_exotic, a.treatment_extra_exotic);
    assert_eq!(
        back.treatment.provenance.manifest_fingerprint,
        a.treatment.provenance.manifest_fingerprint
    );
}

#[test]
fn ck_exotic_channel_rejects_invalid_and_non_post_fork_extras() {
    use crate::core::intervention::Region;

    let reg = ObservableRegistry::reference_default();
    let m = mana_manifest_for_fork(vec![7]);
    let fork_tick = 3000u64;

    // Starts at/before the fork ⇒ belongs to the shared prefix.
    assert!(checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
        &m,
        &reg,
        7,
        fork_tick,
        &[],
        &[exotic_removal(1, 2940, 60)]
    )
    .is_err());

    // Has no ecology firing STRICTLY after the fork: window [3001, 3060) contains no multiple
    // of 60 greater than fork_tick.
    assert!(checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
        &m,
        &reg,
        7,
        fork_tick,
        &[],
        &[exotic_removal(2, 3001, 59)]
    )
    .is_err());

    // Structurally invalid (NaN amount).
    let mut nan = exotic_removal(3, 3060, 60);
    nan.amount = f32::NAN;
    assert!(checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
        &m,
        &reg,
        7,
        fork_tick,
        &[],
        &[nan]
    )
    .is_err());

    // Duplicate id within the exotic extras.
    assert!(checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
        &m,
        &reg,
        7,
        fork_tick,
        &[],
        &[exotic_removal(4, 3060, 60), exotic_removal(4, 3120, 60)]
    )
    .is_err());

    // Collision with a BASE exotic forcing id.
    let mut with_base = mana_manifest_for_fork(vec![7]);
    with_base.exotic_interventions = vec![exotic_removal(9, 600, 60)];
    assert!(checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
        &with_base,
        &reg,
        7,
        fork_tick,
        &[],
        &[exotic_removal(9, 3060, 60)]
    )
    .is_err());

    // A partially overflowing rectangle is not a valid 16×16 checkpoint treatment. This pure
    // grid preflight must reject it before a generic model factory (and therefore RNG) runs.
    let mut outside_grid = exotic_removal(6, 3060, 60);
    outside_grid.region = Region::Rect {
        min_x: 15,
        min_y: 15,
        max_x: 16,
        max_y: 16,
    };
    PREFLIGHT_MODEL_CONSTRUCTIONS.store(0, Ordering::SeqCst);
    assert!(checkpoint_fork_with_exotic::<PreflightCountingModel>(
        &m,
        &reg,
        7,
        fork_tick,
        &[],
        &[outside_grid]
    )
    .is_err());
    assert_eq!(
        PREFLIGHT_MODEL_CONSTRUCTIONS.load(Ordering::SeqCst),
        0,
        "invalid checkpoint geometry must be rejected before model construction"
    );

    // Exotic extras on a world with no exotic field.
    let baseline = baseline_manifest(vec![7]);
    assert!(checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
        &baseline,
        &reg,
        7,
        fork_tick,
        &[],
        &[exotic_removal(5, 3060, 60)]
    )
    .is_err());
}

#[test]
fn ck_legacy_checkpoint_fork_still_works_unchanged() {
    // The pre-existing entry point keeps its signature and behaviour (no exotic extras).
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![99]);
    let uninterrupted = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 99, None, None);
    let fork =
        checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 99, 2500, &[]).expect("fork ok");
    assert_eq!(fork.control.final_checksum, uninterrupted.final_checksum);
    assert!(fork.treatment_extra_exotic.is_empty());
}

// ---- AUDIT D1: PAIRED same-seed control/treatment (the real AE-S14 gate) -----------------

fn mana_treatment(seeds: Vec<u64>) -> ExperimentManifest {
    use crate::core::exotic_energy::ExoticEnergyLaw;
    let mut m = baseline_manifest(seeds);
    m.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(150.0, 4));
    m
}

#[test]
fn genesis_fork_does_not_fabricate_zero_for_a_treatment_only_observable() {
    let reg = ObservableRegistry::reference_default();
    let treatment = mana_treatment(vec![7]);
    let report =
        genesis_fork::<ReferenceEvolutionWorld>(&treatment, &reg, &FactorDiff::genesis_exotic())
            .expect("fork runs");

    assert!(report.control.observable("exotic.density_total").is_none());
    assert!(report
        .treatment
        .observable("exotic.density_total")
        .is_some());
    assert!(
        report.delta_of("exotic.density_total").is_none(),
        "a missing control observation is not a measured zero"
    );
}

#[test]
fn d1_paired_runner_uses_same_ordered_seeds_and_reports_paired_stats() {
    let reg = ObservableRegistry::reference_default();
    let seeds = vec![1u64, 2, 3, 4, 5];
    let treatment = mana_treatment(seeds.clone());
    let rep = run_paired_ensemble::<ReferenceEvolutionWorld>(
        &treatment,
        &reg,
        &FactorDiff::genesis_exotic(),
    )
    .expect("paired run");

    // Same ordered seed set by construction, one pair per seed.
    assert_eq!(rep.seed_order, seeds);
    assert_eq!(rep.pairs.len(), seeds.len());
    for (p, s) in rep.pairs.iter().zip(&seeds) {
        assert_eq!(p.seed, *s);
        assert_eq!(p.control.provenance.seed, *s);
        assert_eq!(p.treatment.provenance.seed, *s);
        assert!(p.is_complete());
    }

    // Provenance/fingerprints of BOTH sides are carried.
    assert_eq!(rep.treatment_manifest_fingerprint, treatment.fingerprint());
    assert_eq!(
        rep.control_manifest_fingerprint,
        treatment.control_variant().fingerprint()
    );
    assert_ne!(
        rep.control_manifest_fingerprint,
        rep.treatment_manifest_fingerprint
    );
    assert_ne!(rep.control_law_fingerprint, rep.treatment_law_fingerprint);
    assert_eq!(rep.registry_fingerprint, reg.fingerprint());

    // EU observables: the exotic field never touches biomass, so every paired delta is exactly 0
    // and the paired SD is 0 — but the statistics are DEFINED (n = 5).
    let plants = rep.effect_of("plants").expect("plants effect");
    assert_eq!(plants.n_requested, 5);
    assert_eq!(plants.n_complete_pairs, 5);
    assert_eq!(plants.paired_mean_delta, Some(0.0));
    assert_eq!(plants.paired_sd, Some(0.0));
    assert_eq!(plants.paired_se, Some(0.0));
    assert_eq!(plants.ci95_low, Some(0.0));
    assert_eq!(plants.ci95_high, Some(0.0));
    // Zero paired variance ⇒ standardized dz is UNDEFINED, not infinite.
    assert_eq!(plants.paired_dz, None);
}

#[test]
fn d1_paired_effects_have_known_sign_and_magnitude() {
    // A drought treatment, same seeds: every pair must move plants down by the same mechanism,
    // so the paired mean delta is negative and the paired CI excludes zero.
    let reg = ObservableRegistry::reference_default();
    let seeds = vec![21u64, 22, 23, 24, 25, 26];
    let control = baseline_manifest(seeds.clone());
    let mut treatment = baseline_manifest(seeds.clone());
    treatment.interventions = vec![drought(1, 600)];

    // `control_variant` only strips the EXOTIC regime, so a non-exotic factor needs an explicit
    // control manifest — that is what `run_paired_ensemble_with_control` is for.
    let allowed = FactorDiff {
        allowed_paths: vec!["interventions".to_string()],
    };
    let rep = run_paired_ensemble_with_control::<ReferenceEvolutionWorld>(
        &control, &treatment, &reg, &allowed,
    )
    .expect("paired run");

    let plants = rep.effect_of("plants").expect("plants effect");
    assert_eq!(plants.n_complete_pairs, 6);
    let delta = plants.paired_mean_delta.expect("defined");
    assert!(delta < 0.0, "drought must lower plants, got {delta}");
    assert!(plants.ci95_high.expect("defined") < 0.0, "CI excludes 0");
    let dz = plants
        .paired_dz
        .expect("paired dz must be defined for non-zero variance");
    assert!(
        dz.is_finite() && dz < 0.0,
        "drought dz must be finite and negative"
    );
    // Control/treatment means bracket the delta consistently.
    assert!((plants.treatment_mean.unwrap() - plants.control_mean.unwrap() - delta).abs() < 1e-9);
}

#[test]
fn d1_paired_single_pair_defines_means_but_not_spread() {
    let reg = ObservableRegistry::reference_default();
    let treatment = mana_treatment(vec![7]);
    let rep = run_paired_ensemble::<ReferenceEvolutionWorld>(
        &treatment,
        &reg,
        &FactorDiff::genesis_exotic(),
    )
    .expect("paired run");
    let plants = rep.effect_of("plants").expect("plants effect");
    assert_eq!(plants.n_complete_pairs, 1);
    assert!(plants.control_mean.is_some());
    assert!(plants.treatment_mean.is_some());
    assert!(plants.paired_mean_delta.is_some());
    // n = 1 ⇒ no spread is estimable.
    assert_eq!(plants.paired_sd, None);
    assert_eq!(plants.paired_se, None);
    assert_eq!(plants.ci95_low, None);
    assert_eq!(plants.ci95_high, None);
    assert_eq!(plants.paired_dz, None);
}

// A model that fails at runtime for ONE seed, to prove one-sided failures never silently drop a
// half-pair and never contaminate the effect statistics.
struct OneSidedFailModel {
    inner: ReferenceEvolutionWorld,
    fail: bool,
    ticks: u64,
}
impl ExperimentModel for OneSidedFailModel {
    type Snapshot = (<ReferenceEvolutionWorld as ExperimentModel>::Snapshot, u64);

    fn model_version() -> &'static str {
        "test-one-sided-failure/1"
    }

    fn from_manifest(
        laws: &WorldLawSet,
        initial: &InitialConditionSet,
        forcings: &[ExoticIntervention],
        seed: u64,
        grid: (usize, usize),
        run_ticks: u64,
    ) -> Result<Self, ExperimentError> {
        Ok(OneSidedFailModel {
            inner: ReferenceEvolutionWorld::from_manifest(
                laws, initial, forcings, seed, grid, run_ticks,
            )?,
            // Only the TREATMENT side (which has the exotic law) fails, and only for seed 3.
            fail: seed == 3 && laws.exotic_energy.is_some(),
            ticks: 0,
        })
    }
    fn snapshot(&self) -> Self::Snapshot {
        (self.inner.snapshot(), self.ticks)
    }
    fn from_snapshot(s: &Self::Snapshot) -> Result<Self, ExperimentError> {
        Ok(OneSidedFailModel {
            inner: ReferenceEvolutionWorld::from_snapshot(&s.0)?,
            fail: false,
            ticks: s.1,
        })
    }
    fn step(
        &mut self,
        clock: &SimClock,
        active: &[&InterventionCommand],
        ledger: &mut CausalLedger,
        rng: &mut StdRng,
    ) {
        self.inner.step(clock, active, ledger, rng);
        self.ticks += 1;
    }
    fn checksum(&self) -> u32 {
        self.inner.checksum()
    }
    fn observables(&self) -> Vec<(String, f64)> {
        let mut o = self.inner.observables();
        if self.fail && self.ticks >= 120 {
            o.push(("diverged".into(), f64::NAN));
        }
        o
    }
}

#[test]
fn d1_one_sided_failure_is_preserved_and_excluded_from_effects() {
    let reg = ObservableRegistry::reference_default();
    let seeds = vec![1u64, 2, 3, 4, 5];
    let treatment = mana_treatment(seeds.clone());
    let rep =
        run_paired_ensemble::<OneSidedFailModel>(&treatment, &reg, &FactorDiff::genesis_exotic())
            .expect("paired run");

    // The half-pair is PRESERVED, not dropped: 5 pairs recorded, 1 incomplete.
    assert_eq!(rep.pairs.len(), 5);
    let broken = rep.pairs.iter().find(|p| p.seed == 3).expect("seed 3 pair");
    assert!(!broken.is_complete());
    assert!(broken.control.status.is_completed());
    assert!(!broken.treatment.status.is_completed());
    assert_eq!(rep.incomplete_pairs(), 1);

    // Effects use only the 4 complete pairs, and n_requested still reports all 5.
    let plants = rep.effect_of("plants").expect("plants effect");
    assert_eq!(plants.n_requested, 5);
    assert_eq!(plants.n_complete_pairs, 4);
}

#[test]
fn d1_zero_complete_pairs_yields_all_none_statistics() {
    // Force EVERY treatment run to fail, so no pair is complete.
    struct AlwaysFail(ReferenceEvolutionWorld, u64, bool);
    impl ExperimentModel for AlwaysFail {
        type Snapshot = <ReferenceEvolutionWorld as ExperimentModel>::Snapshot;

        fn model_version() -> &'static str {
            "test-always-fail/1"
        }

        fn from_manifest(
            laws: &WorldLawSet,
            initial: &InitialConditionSet,
            forcings: &[ExoticIntervention],
            seed: u64,
            grid: (usize, usize),
            run_ticks: u64,
        ) -> Result<Self, ExperimentError> {
            Ok(AlwaysFail(
                ReferenceEvolutionWorld::from_manifest(
                    laws, initial, forcings, seed, grid, run_ticks,
                )?,
                0,
                laws.exotic_energy.is_some(),
            ))
        }
        fn snapshot(&self) -> Self::Snapshot {
            self.0.snapshot()
        }
        fn from_snapshot(s: &Self::Snapshot) -> Result<Self, ExperimentError> {
            Ok(AlwaysFail(
                ReferenceEvolutionWorld::from_snapshot(s)?,
                0,
                false,
            ))
        }
        fn step(
            &mut self,
            c: &SimClock,
            a: &[&InterventionCommand],
            l: &mut CausalLedger,
            r: &mut StdRng,
        ) {
            self.0.step(c, a, l, r);
            self.1 += 1;
        }
        fn checksum(&self) -> u32 {
            self.0.checksum()
        }
        fn observables(&self) -> Vec<(String, f64)> {
            let mut o = self.0.observables();
            if self.2 && self.1 >= 120 {
                o.push(("diverged".into(), f64::NAN));
            }
            o
        }
    }

    let reg = ObservableRegistry::reference_default();
    let treatment = mana_treatment(vec![1, 2, 3]);
    let rep = run_paired_ensemble::<AlwaysFail>(&treatment, &reg, &FactorDiff::genesis_exotic())
        .expect("paired run");
    assert_eq!(rep.incomplete_pairs(), 3);
    let plants = rep.effect_of("plants").expect("plants is still listed");
    assert_eq!(plants.n_requested, 3);
    assert_eq!(plants.n_complete_pairs, 0);
    // n = 0 ⇒ EVERY statistic is None; nothing is fabricated.
    assert_eq!(plants.control_mean, None);
    assert_eq!(plants.treatment_mean, None);
    assert_eq!(plants.paired_mean_delta, None);
    assert_eq!(plants.paired_sd, None);
    assert_eq!(plants.paired_se, None);
    assert_eq!(plants.ci95_low, None);
    assert_eq!(plants.ci95_high, None);
    assert_eq!(plants.paired_dz, None);
}

#[test]
fn d1_observable_missing_on_one_side_is_listed_not_compared() {
    let reg = ObservableRegistry::reference_default();
    let treatment = mana_treatment(vec![1, 2, 3]);
    let rep = run_paired_ensemble::<ReferenceEvolutionWorld>(
        &treatment,
        &reg,
        &FactorDiff::genesis_exotic(),
    )
    .expect("paired run");
    // The MU observables exist only on the treatment side.
    assert!(rep
        .treatment_only
        .iter()
        .any(|o| o == "exotic.density_total"));
    assert!(rep.effect_of("exotic.density_total").is_none());
    assert!(rep.control_only.is_empty());
}

#[test]
fn d1_paired_report_json_round_trips_without_nan() {
    let reg = ObservableRegistry::reference_default();
    let treatment = mana_treatment(vec![1, 2]);
    let rep = run_paired_ensemble::<ReferenceEvolutionWorld>(
        &treatment,
        &reg,
        &FactorDiff::genesis_exotic(),
    )
    .expect("paired run");
    let json = serde_json::to_string(&rep).expect("serializes");
    let back: PairedEnsembleReport =
        serde_json::from_str(&json).expect("deserializes (no NaN/inf leaked)");
    assert_eq!(back.seed_order, rep.seed_order);
    assert_eq!(
        back.treatment_manifest_fingerprint,
        rep.treatment_manifest_fingerprint
    );
    assert_eq!(back.effects.len(), rep.effects.len());
}

#[test]
fn d1_paired_runner_rejects_bad_inputs_before_any_model_or_rng() {
    let reg = ObservableRegistry::reference_default();
    let treatment = mana_treatment(vec![1, 2]);
    PREFLIGHT_MODEL_CONSTRUCTIONS.store(0, Ordering::SeqCst);

    // An allowlist that does not cover the exotic regime must be rejected up front.
    let too_strict = FactorDiff {
        allowed_paths: vec!["seeds".to_string()],
    };
    assert!(matches!(
        run_paired_ensemble::<PreflightCountingModel>(&treatment, &reg, &too_strict),
        Err(ExperimentError::UndeclaredFactorDifference { .. })
    ));

    // A malformed registry is rejected before anything runs.
    let mut bad_reg = ObservableRegistry::reference_default();
    bad_reg.version = 999;
    assert!(run_paired_ensemble::<PreflightCountingModel>(
        &treatment,
        &bad_reg,
        &FactorDiff::genesis_exotic()
    )
    .is_err());

    // An invalid manifest (empty seeds) is rejected before anything runs.
    let mut empty = mana_treatment(vec![]);
    empty.seeds = vec![];
    assert!(matches!(
        run_paired_ensemble::<PreflightCountingModel>(&empty, &reg, &FactorDiff::genesis_exotic()),
        Err(ExperimentError::EmptySeeds)
    ));
    assert_eq!(
        PREFLIGHT_MODEL_CONSTRUCTIONS.load(Ordering::SeqCst),
        0,
        "all malformed inputs must fail before model construction or RNG seeding"
    );
}

// ---- Independent (NOT paired) descriptive helper — see `compare_ensembles` docs ----------

#[test]
fn ae_s14_m4_effect_size_reports_difference_g_and_interval() {
    use crate::core::exotic_energy::ExoticEnergyLaw;
    let reg = ObservableRegistry::reference_default();
    let seeds = vec![1u64, 2, 3, 4, 5];

    // Control: baseline. Treatment: same everything but a live exotic law. The EU pools are
    // deliberately unaffected by the exotic field (AE-S05), so the EU effect size must be ~0
    // while the MU observable exists only in the treatment.
    let control_m = baseline_manifest(seeds.clone());
    let mut treatment_m = baseline_manifest(seeds.clone());
    treatment_m.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(150.0, 4));
    treatment_m
        .observable_ids
        .push("exotic.density_total".into());

    let control = run_ensemble::<ReferenceEvolutionWorld>(&control_m, &reg).expect("control");
    let treatment = run_ensemble::<ReferenceEvolutionWorld>(&treatment_m, &reg).expect("treatment");

    let cmp = compare_ensembles(&control, &treatment).expect("finite comparison");

    // N is reported per side and failures are preserved (none here).
    assert_eq!(cmp.control_n, 5);
    assert_eq!(cmp.treatment_n, 5);
    assert!(cmp.control_failed.is_empty() && cmp.treatment_failed.is_empty());

    // A shared observable is compared; the EU pools show ~zero effect (exotic never touches EU).
    let plants = cmp.effect_of("plants").expect("plants compared");
    assert_eq!(plants.n_control, 5);
    assert_eq!(plants.n_treatment, 5);
    assert!(
        plants.mean_difference.abs() < 1e-9,
        "EU effect must be ~0, got {}",
        plants.mean_difference
    );
    // The interval brackets the mean difference.
    assert!(plants.ci95_low.expect("n=5 lower bound") <= plants.mean_difference);
    assert!(plants.mean_difference <= plants.ci95_high.expect("n=5 upper bound"));

    // An observable present only in the treatment is reported as treatment-only, not silently
    // dropped and not compared against a fabricated zero.
    assert!(
        cmp.treatment_only
            .iter()
            .any(|o| o == "exotic.density_total"),
        "treatment-only observables must be listed: {:?}",
        cmp.treatment_only
    );
    assert!(cmp.effect_of("exotic.density_total").is_none());
}

#[test]
fn ae_s14_m4_effect_size_detects_a_real_difference_with_correct_sign() {
    // A drought treatment must produce a NEGATIVE mean difference on plants with a large
    // standardized effect size, versus an undroughted control.
    let reg = ObservableRegistry::reference_default();
    let seeds = vec![11u64, 12, 13, 14, 15, 16];
    let control_m = baseline_manifest(seeds.clone());
    let mut treatment_m = baseline_manifest(seeds.clone());
    treatment_m.interventions = vec![drought(1, 600)];

    let control = run_ensemble::<ReferenceEvolutionWorld>(&control_m, &reg).expect("control");
    let treatment = run_ensemble::<ReferenceEvolutionWorld>(&treatment_m, &reg).expect("treatment");
    let cmp = compare_ensembles(&control, &treatment).expect("finite comparison");

    let plants = cmp.effect_of("plants").expect("plants compared");
    assert!(
        plants.mean_difference < 0.0,
        "drought must lower plants, got {}",
        plants.mean_difference
    );
    let hedges_g = plants.hedges_g.expect("both groups estimate spread");
    assert!(
        hedges_g.abs() > 1.0,
        "a drought should be a large standardized effect, got g={}",
        hedges_g
    );
    // The 95% interval excludes zero for a real effect this large.
    assert!(
        plants.ci95_high.expect("n=6 upper bound") < 0.0,
        "interval should exclude 0"
    );
}

#[test]
fn ae_s14_m4_effect_size_preserves_failed_runs_and_degenerate_variance() {
    let reg = ObservableRegistry::reference_default();
    let seeds = vec![10u64, 11, 12, 13, 14]; // FlakyModel poisons seed 12
    let m = baseline_manifest(seeds.clone());
    let control = run_ensemble::<FlakyModel>(&m, &reg).expect("control");
    let treatment = run_ensemble::<FlakyModel>(&m, &reg).expect("treatment");
    let cmp = compare_ensembles(&control, &treatment).expect("finite comparison");

    // Failures are surfaced, never dropped; N counts only completed runs.
    assert_eq!(cmp.control_failed.len(), 1);
    assert_eq!(cmp.treatment_failed.len(), 1);
    assert_eq!(cmp.control_n, 4);
    assert_eq!(cmp.treatment_n, 4);

    // Control == treatment here, so the raw difference is exactly 0. If between-seed spread is
    // nonzero, g is the defined value 0; if spread is zero, g is explicitly undefined.
    let plants = cmp.effect_of("plants").expect("plants compared");
    assert_eq!(plants.mean_difference, 0.0);
    match plants.pooled_sd.expect("n=4 estimates pooled spread") {
        0.0 => assert_eq!(plants.hedges_g, None),
        _ => assert_eq!(plants.hedges_g, Some(0.0)),
    }
}

// ---- Task-5: hardened entry points ------------------------------------------------------

#[test]
fn run_manifest_seed_fails_on_unknown_observable_not_silently() {
    let reg = ObservableRegistry::reference_default();
    let mut m = baseline_manifest(vec![1]);
    m.observable_ids = vec!["not_a_real_observable".into()];
    let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 1, None, None);
    // Never silently executed: a structured Failed result mentioning the invalid manifest.
    assert!(matches!(res.status, RunStatus::Failed { .. }));
    if let RunStatus::Failed { reason, .. } = &res.status {
        assert!(reason.contains("manifest") && reason.contains("not_a_real_observable"));
    }
}

#[test]
fn run_manifest_seed_fails_on_invalid_manifest_and_registry() {
    let reg = ObservableRegistry::reference_default();

    // Duplicate seed → invalid manifest → Failed (not executed).
    let mut m = baseline_manifest(vec![1]);
    m.seeds = vec![5, 5];
    let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 5, None, None);
    assert!(matches!(res.status, RunStatus::Failed { .. }));

    // Invalid registry (unknown schema version) → Failed.
    let mut bad_reg = ObservableRegistry::reference_default();
    bad_reg.version = 999;
    let good_m = baseline_manifest(vec![1]);
    let res = run_manifest_seed::<ReferenceEvolutionWorld>(&good_m, &bad_reg, 1, None, None);
    assert!(matches!(res.status, RunStatus::Failed { .. }));
    if let RunStatus::Failed { reason, .. } = &res.status {
        assert!(reason.contains("registry"));
    }
}

#[test]
fn ensemble_rejects_invalid_and_empty_inputs_at_ensemble_level() {
    // ISSUE-2: an invalid manifest/registry (or an EMPTY seed set) is a structured ensemble-level
    // error — NOT a misleading zero-run or all-failed "summary". No fabricated seed is introduced.
    let reg = ObservableRegistry::reference_default();

    // Unknown observable → Err.
    let mut m = baseline_manifest(vec![1, 2, 3]);
    m.observable_ids = vec!["nope".into()];
    assert!(run_ensemble::<ReferenceEvolutionWorld>(&m, &reg).is_err());

    // Empty seed set → Err (previously a misleading zero-run summary).
    let mut m = baseline_manifest(vec![]);
    m.seeds = vec![];
    assert!(matches!(
        run_ensemble::<ReferenceEvolutionWorld>(&m, &reg),
        Err(ExperimentError::EmptySeeds)
    ));

    // Invalid registry → Err.
    let mut bad_reg = ObservableRegistry::reference_default();
    bad_reg.version = 999;
    let good_m = baseline_manifest(vec![1]);
    assert!(run_ensemble::<ReferenceEvolutionWorld>(&good_m, &bad_reg).is_err());
}

#[test]
fn result_is_self_describing_for_every_emitted_observable() {
    // The manifest requests only a subset, but the model also emits detritus/npp/etc. Every
    // EMITTED observable must carry a registry spec (not just the requested ones).
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![1]);
    let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 1, None, None);
    assert!(res.status.is_completed());
    let spec_ids: std::collections::BTreeSet<&str> =
        res.observable_specs.iter().map(|s| s.id.as_str()).collect();
    for (name, _) in &res.final_observables {
        assert!(
            spec_ids.contains(name.as_str()),
            "emitted observable '{name}' has no spec in the self-describing result"
        );
    }
}

// ---- AE-107 / AE-S09: headless checkpoint fork ------------------------------------------

fn drought(id: u32, start: u64) -> InterventionCommand {
    use crate::core::intervention::{Curve, InterventionKind, Region};
    InterventionCommand {
        id,
        cause_id: id,
        kind: InterventionKind::RainfallDelta,
        region: Region::Global,
        start_tick: start,
        duration_ticks: 6000,
        intensity: 0.5,
        signed_negative: true,
        curve: Curve::Step,
        reversible: true,
    }
}

#[test]
fn ae_s09_checkpoint_continuation_equals_uninterrupted_run() {
    // The snapshot (model state + cloned RNG) is COMPLETE: a control branch continued from the
    // fork with identical inputs reproduces an uninterrupted run bit-for-bit — proving the fork
    // is not a re-simulation and captures the full state including the RNG stream.
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![99]);
    let uninterrupted = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 99, None, None);

    let fork =
        checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 99, 2500, &[]).expect("fork ok");
    assert_eq!(
        fork.control.final_checksum, uninterrupted.final_checksum,
        "checkpoint continuation must equal the uninterrupted run"
    );
    // With no treatment interventions, both branches are identical.
    assert_eq!(fork.control.final_checksum, fork.treatment.final_checksum);
    // Provenance records the fork lineage.
    assert_eq!(fork.control.provenance.fork_tick, Some(2500));
    assert_eq!(
        fork.control.provenance.parent_run_id.as_deref(),
        Some(fork.prefix.provenance.run_id.as_str())
    );
    assert!(fork.prefix.status.is_completed());
}

#[test]
fn ae_s09_post_fork_treatment_diverges_only_after_the_fork() {
    // A treatment intervention applied AFTER the fork tick makes the branches diverge, but they
    // share the identical pre-fork snapshot, so the prefix is common history.
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![7]);
    let fork_tick = 3000;
    // Drought starting after the fork only affects the treatment branch.
    let extra = vec![drought(1, fork_tick + 60)];
    let fork = checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 7, fork_tick, &extra)
        .expect("fork ok");

    // Both branches restored from the SAME snapshot → identical pre-fork state (checksum of the
    // prefix is shared). The treatment diverges by the end.
    assert_ne!(
        fork.control.final_checksum, fork.treatment.final_checksum,
        "a post-fork intervention must change the treatment branch"
    );
    // Plants fall in the treatment (drought), a real declared effect.
    assert!(fork.delta_of("plants").unwrap() < 0.0);
    assert!(!fork.declared_factors.is_empty());
    // Both children point at the same parent prefix and fork tick.
    assert_eq!(
        fork.control.provenance.parent_run_id,
        fork.treatment.provenance.parent_run_id
    );
    assert_eq!(fork.treatment.provenance.fork_tick, Some(fork_tick));
}

#[test]
fn checkpoint_fork_rejects_bad_fork_tick() {
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![1]);
    assert!(checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 1, 0, &[]).is_err());
    assert!(
        checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 1, m.duration_ticks, &[]).is_err()
    );
}

// ---- ISSUE-1: a seed absent from the manifest is rejected before any model/RNG --------------

#[test]
fn run_manifest_seed_rejects_seed_not_in_manifest() {
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![1, 2, 3]);
    let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 999, None, None);
    assert!(matches!(res.status, RunStatus::Failed { .. }));
    if let RunStatus::Failed { reason, tick, .. } = &res.status {
        assert_eq!(*tick, 0, "rejected before any tick was run");
        assert!(reason.contains("seed 999") && reason.contains("seed set"));
    }
    // A declared seed is accepted.
    assert!(
        run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 2, None, None)
            .status
            .is_completed()
    );
}

#[test]
fn checkpoint_fork_rejects_seed_not_in_manifest() {
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![1, 2, 3]);
    assert!(matches!(
        checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 42, 100, &[]),
        Err(ExperimentError::SeedNotInManifest { seed: 42 })
    ));
}

// ---- ISSUE-3: a transient (series-only) observable is described + warned --------------------

// Emits an extra observable ONLY on the first two ticks, so it appears in the series but never in
// `final_observables`. It is not in the registry, so the union must surface a missing-spec warning.
struct TransientModel {
    inner: ReferenceEvolutionWorld,
    ticks: u64,
}
impl ExperimentModel for TransientModel {
    type Snapshot = (<ReferenceEvolutionWorld as ExperimentModel>::Snapshot, u64);

    fn model_version() -> &'static str {
        "test-transient-observable/1"
    }

    fn from_manifest(
        laws: &WorldLawSet,
        initial: &InitialConditionSet,
        forcings: &[ExoticIntervention],
        seed: u64,
        grid: (usize, usize),
        run_ticks: u64,
    ) -> Result<Self, ExperimentError> {
        Ok(TransientModel {
            inner: ReferenceEvolutionWorld::from_manifest(
                laws, initial, forcings, seed, grid, run_ticks,
            )?,
            ticks: 0,
        })
    }
    fn snapshot(&self) -> Self::Snapshot {
        (self.inner.snapshot(), self.ticks)
    }
    fn from_snapshot(snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
        Ok(TransientModel {
            inner: ReferenceEvolutionWorld::from_snapshot(&snapshot.0)?,
            ticks: snapshot.1,
        })
    }
    fn step(
        &mut self,
        clock: &SimClock,
        active: &[&InterventionCommand],
        ledger: &mut CausalLedger,
        rng: &mut StdRng,
    ) {
        self.inner.step(clock, active, ledger, rng);
        self.ticks += 1;
    }
    fn checksum(&self) -> u32 {
        self.inner.checksum()
    }
    fn observables(&self) -> Vec<(String, f64)> {
        let mut o = self.inner.observables();
        if self.ticks <= 2 {
            o.push(("transient.only_in_series".into(), 1.0));
        }
        o
    }
}

#[test]
fn result_describes_transient_series_only_observable_with_missing_spec_warning() {
    let reg = ObservableRegistry::reference_default();
    let m = ExperimentManifest {
        duration_ticks: 10,
        sample_period: 1, // sample every tick, so the transient lands in the series
        ..baseline_manifest(vec![1])
    };
    let res = run_manifest_seed::<TransientModel>(&m, &reg, 1, None, None);
    assert!(res.status.is_completed());
    // The transient is present in the series but NOT in final_observables.
    assert!(res.series.iter().any(|s| s
        .observables
        .iter()
        .any(|(n, _)| n == "transient.only_in_series")));
    assert!(!res
        .final_observables
        .iter()
        .any(|(n, _)| n == "transient.only_in_series"));
    // The union-based metadata surfaces the missing-spec warning for the series-only observable.
    assert!(
        res.warnings
            .iter()
            .any(|w| w.contains("transient.only_in_series") && w.contains("no registry spec")),
        "series-only observable must be reported as a missing-spec warning; warnings: {:?}",
        res.warnings
    );
}

// ---- ISSUE-4: checkpoint window semantics + failing prefix ----------------------------------

#[test]
fn checkpoint_rejects_treatment_extra_outside_post_fork_window() {
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![7]); // duration 6000
    let fork_tick = 3000u64;

    // start_tick == fork_tick → belongs to the shared prefix → rejected.
    assert!(matches!(
        checkpoint_fork::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            fork_tick,
            &[drought(1, fork_tick)]
        ),
        Err(ExperimentError::InapplicableIntervention { id: 1, .. })
    ));

    // start_tick after the run ends → rejected.
    assert!(matches!(
        checkpoint_fork::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            fork_tick,
            &[drought(2, m.duration_ticks + 1)]
        ),
        Err(ExperimentError::InapplicableIntervention { id: 2, .. })
    ));

    // Boundary OK cases: first post-fork tick, and the very last tick.
    assert!(
        checkpoint_fork::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            fork_tick,
            &[drought(3, fork_tick + 1)]
        )
        .is_ok(),
        "start_tick == fork_tick+1 must be accepted"
    );
    assert!(
        checkpoint_fork::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            fork_tick,
            &[drought(4, m.duration_ticks)]
        )
        .is_ok(),
        "start_tick == duration_ticks must be accepted (it fires on the last processed tick)"
    );
}

// A model whose observables go non-finite partway through the prefix, to exercise the
// failing-prefix path of checkpoint_fork.
struct PrefixFailModel {
    inner: ReferenceEvolutionWorld,
    ticks: u64,
}
impl ExperimentModel for PrefixFailModel {
    type Snapshot = (<ReferenceEvolutionWorld as ExperimentModel>::Snapshot, u64);

    fn model_version() -> &'static str {
        "test-prefix-failure/1"
    }

    fn from_manifest(
        laws: &WorldLawSet,
        initial: &InitialConditionSet,
        forcings: &[ExoticIntervention],
        seed: u64,
        grid: (usize, usize),
        run_ticks: u64,
    ) -> Result<Self, ExperimentError> {
        Ok(PrefixFailModel {
            inner: ReferenceEvolutionWorld::from_manifest(
                laws, initial, forcings, seed, grid, run_ticks,
            )?,
            ticks: 0,
        })
    }
    fn snapshot(&self) -> Self::Snapshot {
        (self.inner.snapshot(), self.ticks)
    }
    fn from_snapshot(snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
        Ok(PrefixFailModel {
            inner: ReferenceEvolutionWorld::from_snapshot(&snapshot.0)?,
            ticks: snapshot.1,
        })
    }
    fn step(
        &mut self,
        clock: &SimClock,
        active: &[&InterventionCommand],
        ledger: &mut CausalLedger,
        rng: &mut StdRng,
    ) {
        self.inner.step(clock, active, ledger, rng);
        self.ticks += 1;
    }
    fn checksum(&self) -> u32 {
        self.inner.checksum()
    }
    fn observables(&self) -> Vec<(String, f64)> {
        let mut o = self.inner.observables();
        if self.ticks >= 100 {
            o.push(("diverged".into(), f64::NAN));
        }
        o
    }
}

// ---- DEFECT A: a completed RunResult must survive a real JSON round-trip -----------------

#[test]
fn defect_a_run_result_json_round_trips_for_baseline_and_treatment() {
    use crate::core::exotic_energy::ExoticEnergyLaw;
    let reg = ObservableRegistry::reference_default();

    // Demonstrate the exact failure mode this guards against: an infinite bound serializes to
    // `null` and can no longer be read back as an f64. This is why the registry uses finite
    // bounds and why the round-trip below is a real regression guard.
    let mut infinite_spec = reg.specs()[0].clone();
    infinite_spec.valid_max = f64::INFINITY;
    let bad_json = serde_json::to_string(&infinite_spec).expect("serializes");
    assert!(bad_json.contains("\"valid_max\":null"));
    assert!(
        serde_json::from_str::<ObservableSpec>(&bad_json).is_err(),
        "a null bound must fail to deserialize — proving infinities lose data"
    );

    // Baseline (exotic = None) and treatment (live MU field) both round-trip losslessly.
    let baseline = baseline_manifest(vec![5]);
    let mut treatment = baseline_manifest(vec![5]);
    treatment.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(150.0, 4));
    treatment.observable_ids.push("exotic.density_total".into());

    for m in [baseline, treatment] {
        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 5, None, None);
        assert!(res.status.is_completed());
        let json = serde_json::to_string(&res).expect("RunResult serializes");
        // Deserialization is the rigorous proof: `serde_json` renders a non-finite float as
        // `null`, and `null` cannot be parsed back into an `f64` — so a run carrying an infinite
        // spec bound (or observable) fails HERE. (Legitimate `Option::None` fields such as
        // `parent_run_id` also serialize as null, which is why a naive substring check would be
        // wrong; a successful round-trip plus the equality assertions below is what matters.)
        let back: RunResult = serde_json::from_str(&json).expect("RunResult deserializes");

        // Structural / integer / string content is EXACT across the round-trip.
        assert_eq!(back.final_checksum, res.final_checksum);
        assert_eq!(back.provenance, res.provenance);
        assert_eq!(back.observable_specs, res.observable_specs);
        assert_eq!(back.series.len(), res.series.len());
        assert_eq!(
            back.final_observables.len(),
            res.final_observables.len(),
            "no observable may be dropped by the round-trip"
        );

        // Float PAYLOAD values survive to serde_json's documented ±1 ULP accuracy (the
        // `float_roundtrip` feature is not enabled — the same caveat `dynamic_fields`' save/load
        // test records). This is an honest property of JSON export, not bit-exactness.
        for ((kb, vb), (kr, vr)) in back.final_observables.iter().zip(&res.final_observables) {
            assert_eq!(kb, kr);
            let tol = 1e-12 * vr.abs().max(1.0);
            assert!(
                (vb - vr).abs() <= tol,
                "observable '{kb}' drifted beyond 1 ULP: {vb} vs {vr}"
            );
        }

        // The spec ranges survived intact and finite (the point of the fix).
        for s in &back.observable_specs {
            assert!(s.valid_min.is_finite() && s.valid_max.is_finite());
        }
    }
}

// ---- DEFECT B: checkpoint_fork validates treatment_extra with the shared helper ----------

#[test]
fn defect_b_checkpoint_validates_treatment_extra_values_and_ids() {
    let reg = ObservableRegistry::reference_default();
    let mut m = baseline_manifest(vec![7]);
    m.interventions = vec![drought(1, 100)]; // a base intervention with id 1
    let fork_tick = 3000u64;

    // Invalid VALUE inside an otherwise well-placed extra (NaN intensity) → rejected.
    let mut bad = drought(2, fork_tick + 10);
    bad.intensity = f32::NAN;
    assert!(
        checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 7, fork_tick, &[bad]).is_err(),
        "a NaN-intensity extra must be rejected"
    );

    // Invalid geometry inside an extra → rejected.
    let mut bad = drought(3, fork_tick + 10);
    bad.region = crate::core::intervention::Region::Rect {
        min_x: 9,
        min_y: 0,
        max_x: 1,
        max_y: 1,
    };
    assert!(
        checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 7, fork_tick, &[bad]).is_err(),
        "an inverted-Rect extra must be rejected"
    );

    // Duplicate id WITHIN the extras → rejected.
    let dup = vec![drought(9, fork_tick + 10), drought(9, fork_tick + 20)];
    assert!(matches!(
        checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 7, fork_tick, &dup),
        Err(ExperimentError::DuplicateId { .. })
    ));

    // Duplicate id AGAINST the base interventions (base uses id 1) → rejected.
    let clash = vec![drought(1, fork_tick + 10)];
    assert!(matches!(
        checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 7, fork_tick, &clash),
        Err(ExperimentError::DuplicateId { .. })
    ));

    // A clean, uniquely-identified, post-fork extra is accepted.
    assert!(checkpoint_fork::<ReferenceEvolutionWorld>(
        &m,
        &reg,
        7,
        fork_tick,
        &[drought(42, fork_tick + 10)]
    )
    .is_ok());
}

#[test]
fn defect_b_checkpoint_enforces_combined_intervention_limit() {
    use crate::core::experiment::MAX_INTERVENTIONS;
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![7]);
    // More extras than the combined ceiling → structured ResourceLimit, before any model/RNG.
    let extras: Vec<InterventionCommand> = (0..(MAX_INTERVENTIONS as u32 + 1))
        .map(|i| drought(i, 3001))
        .collect();
    assert!(matches!(
        checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 7, 3000, &extras),
        Err(ExperimentError::ResourceLimit { .. })
    ));
}

// ---- DEFECT P1: effective-treatment provenance + structured report ----------------------

#[test]
fn p1_treatment_provenance_uses_effective_manifest_fingerprint() {
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![7]);
    let fork_tick = 3000u64;
    let extra = drought(42, fork_tick + 10);

    let fork = checkpoint_fork::<ReferenceEvolutionWorld>(
        &m,
        &reg,
        7,
        fork_tick,
        std::slice::from_ref(&extra),
    )
    .expect("fork ok");

    // Independently reconstruct the effective treatment manifest: base + appended extras.
    let mut effective = m.clone();
    effective.interventions.push(extra.clone());
    let effective_fp = effective.fingerprint();
    let base_fp = m.fingerprint();
    assert_ne!(
        effective_fp, base_fp,
        "appending an extra must change the manifest fingerprint"
    );

    // Control keeps BASE provenance; treatment carries the EFFECTIVE fingerprint.
    assert_eq!(fork.control.provenance.manifest_fingerprint, base_fp);
    assert_eq!(
        fork.treatment.provenance.manifest_fingerprint, effective_fp,
        "treatment provenance must fingerprint the effective treatment input"
    );
    assert_ne!(
        fork.treatment.provenance.manifest_fingerprint,
        fork.control.provenance.manifest_fingerprint
    );
    // Run ids must differ too (they embed the fingerprint), so the two branches are
    // independently addressable for replay.
    assert_ne!(
        fork.treatment.provenance.run_id,
        fork.control.provenance.run_id
    );
    assert!(fork
        .treatment
        .provenance
        .run_id
        .contains(&format!("{effective_fp:016x}")));

    // The prefix is shared history under the BASE manifest.
    assert_eq!(fork.prefix.provenance.manifest_fingerprint, base_fp);
    assert!(fork.prefix.provenance.parent_run_id.is_none());
}

#[test]
fn p1_no_extras_keeps_control_and_treatment_fingerprints_equal() {
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![7]);
    let fork = checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 7, 3000, &[]).expect("ok");
    let base_fp = m.fingerprint();
    assert_eq!(fork.control.provenance.manifest_fingerprint, base_fp);
    assert_eq!(
        fork.treatment.provenance.manifest_fingerprint, base_fp,
        "with no extras the effective treatment input IS the base manifest"
    );
    assert_eq!(fork.prefix.provenance.manifest_fingerprint, base_fp);
}

#[test]
fn p1_report_carries_structured_extras_that_survive_json_roundtrip() {
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![7]);
    let fork_tick = 3000u64;
    let extra = drought(42, fork_tick + 10);
    let fork = checkpoint_fork::<ReferenceEvolutionWorld>(
        &m,
        &reg,
        7,
        fork_tick,
        std::slice::from_ref(&extra),
    )
    .expect("fork ok");

    // The report carries the FULL structured command, not a lossy "kind@start" string.
    assert_eq!(fork.treatment_extra, vec![extra.clone()]);
    let json = serde_json::to_string(&fork).expect("report serializes");
    let back: CheckpointForkReport = serde_json::from_str(&json).expect("report deserializes");
    assert_eq!(back.treatment_extra, vec![extra]);
    assert_eq!(
        back.treatment.provenance.manifest_fingerprint,
        fork.treatment.provenance.manifest_fingerprint
    );
    assert_eq!(back.fork_tick, fork_tick);
}

#[test]
fn p1_fork_remains_deterministic_and_tick_exact() {
    // The provenance change must not perturb the simulation itself.
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![99]);
    let uninterrupted = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 99, None, None);
    let a = checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 99, 2500, &[]).expect("ok");
    let b = checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 99, 2500, &[]).expect("ok");
    assert_eq!(a.control.final_checksum, uninterrupted.final_checksum);
    assert_eq!(a.control.final_checksum, b.control.final_checksum);
    assert_eq!(a.treatment.final_checksum, b.treatment.final_checksum);
    assert_eq!(a.control.provenance.fork_tick, Some(2500));
}

// ---- DEFECT P2: genesis_fork preflight-validates the registry ---------------------------

#[test]
fn p2_genesis_fork_validates_registry_before_any_model_work() {
    use crate::core::exotic_energy::ExoticEnergyLaw;
    let mut treatment = baseline_manifest(vec![7]);
    treatment.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(150.0, 4));

    // A malformed registry (unsupported version) must fail preflight with a structured error.
    let mut bad_reg = ObservableRegistry::reference_default();
    bad_reg.version = 999;
    assert!(matches!(
        genesis_fork::<ReferenceEvolutionWorld>(
            &treatment,
            &bad_reg,
            &FactorDiff::genesis_exotic()
        ),
        Err(ExperimentError::UnsupportedSchemaVersion { .. })
    ));

    // A duplicate-id registry likewise fails preflight, never stepping a model.
    let mut dup_reg = ObservableRegistry::reference_default();
    dup_reg.push_duplicate_for_test();
    assert!(matches!(
        genesis_fork::<ReferenceEvolutionWorld>(
            &treatment,
            &dup_reg,
            &FactorDiff::genesis_exotic()
        ),
        Err(ExperimentError::DuplicateId { .. })
    ));

    // The valid registry still runs the fork.
    let good = ObservableRegistry::reference_default();
    assert!(genesis_fork::<ReferenceEvolutionWorld>(
        &treatment,
        &good,
        &FactorDiff::genesis_exotic()
    )
    .is_ok());
}

#[test]
fn checkpoint_fork_fails_structurally_when_prefix_diverges() {
    let reg = ObservableRegistry::reference_default();
    let m = baseline_manifest(vec![1]); // duration 6000
                                        // The prefix diverges at tick 100, well before fork_tick 2500.
    let err = checkpoint_fork::<PrefixFailModel>(&m, &reg, 1, 2500, &[]).unwrap_err();
    assert!(matches!(
        err,
        ExperimentError::CheckpointPrefixFailed { tick: 100, .. }
    ));
}
