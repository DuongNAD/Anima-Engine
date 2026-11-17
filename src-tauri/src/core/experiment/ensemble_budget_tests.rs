use super::tests::base_manifest;
use super::*;
use crate::core::intervention::{Curve, InterventionCommand, InterventionKind, Region};

/// G2 gate #3. A manifest at the documented maxima is individually legal on every axis, and
/// before this check it was accepted — then the runner tried to hold the result set in RAM.
/// It must now be refused up front, naming the estimate and the limit.
#[test]
fn a_manifest_at_the_documented_maxima_is_refused_not_attempted() {
    let seeds = MAX_SEEDS as u64;
    let samples = MAX_DURATION_TICKS; // sample_period = 1
    let observables = MAX_OBSERVABLES as u64;
    let estimate = seeds
        .saturating_mul(samples)
        .saturating_mul(observables)
        .saturating_mul(BYTES_PER_SAMPLED_OBSERVABLE);

    assert!(
        estimate > MAX_ENSEMBLE_RESULT_BYTES,
        "the documented maxima must exceed the declared ceiling, or this gate is vacuous: \
             {estimate} vs {MAX_ENSEMBLE_RESULT_BYTES}"
    );
    // ~2.7e16 bytes — about 27 petabytes. The point of the ceiling in one number.
    assert!(estimate > 1e16 as u64, "estimate was {estimate}");
}

/// The estimate saturates instead of wrapping. A wrapped product would come out small and sail
/// through the very check it should fail — the failure mode a budget must not have.
#[test]
fn the_estimate_saturates_rather_than_wrapping() {
    let huge = u64::MAX;
    let product = huge
        .saturating_mul(huge)
        .saturating_mul(huge)
        .saturating_mul(BYTES_PER_SAMPLED_OBSERVABLE);
    assert_eq!(product, u64::MAX);
    assert!(product > MAX_ENSEMBLE_RESULT_BYTES);
}

/// `sample_period == 0` means "never sample", so the series stays empty and the budget is not
/// charged for it. A long run that records only final observables is legitimate.
#[test]
fn never_sampling_costs_nothing_in_series_memory() {
    assert_eq!(samples_for(0, MAX_DURATION_TICKS), 0);
    assert_eq!(samples_for(1000, 100_000), 100);
}

#[test]
fn never_sampling_still_budgets_the_causal_ledger() {
    let registry = ObservableRegistry::reference_default();
    let mut manifest = base_manifest();
    manifest.seeds = (0..MAX_SEEDS as u64).collect();
    manifest.duration_ticks = MAX_DURATION_TICKS;
    manifest.sample_period = 0;
    manifest.interventions = vec![InterventionCommand {
        id: 1,
        cause_id: 7,
        kind: InterventionKind::RainfallDelta,
        region: Region::Global,
        start_tick: 0,
        duration_ticks: MAX_DURATION_TICKS,
        intensity: 0.1,
        signed_negative: false,
        curve: Curve::Step,
        reversible: true,
    }];

    assert!(matches!(
        manifest.validate(&registry),
        Err(ExperimentError::EnsembleTooLarge { .. })
    ));
}

fn samples_for(sample_period: u64, duration_ticks: u64) -> u64 {
    duration_ticks.checked_div(sample_period).unwrap_or(0)
}
