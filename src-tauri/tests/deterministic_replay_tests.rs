//! Integration tests for Deterministic Replay and State Digest Parity (Feature 8 & Feature 7).

use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, ObserverPolicy, WorldIdentity,
    WorldLawSet, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{run_ensemble, run_manifest_seed};
use anima_engine_lib::core::intervention::{
    Curve, InterventionCommand, InterventionKind, Region, TraitMod,
};
use anima_engine_lib::core::reference_world::ReferenceEvolutionWorld;
use anima_engine_lib::core::sim_clock::ECOLOGY_PERIOD;

fn create_test_manifest(interventions: Vec<InterventionCommand>) -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "deterministic_replay_test".into(),
        name: "Determinism & Replay Test".into(),
        observer: ObserverPolicy::default(),
        world_identity: WorldIdentity {
            seed: 1337,
            generator_version: 20,
            width: 128,
            height: 128,
            checksum: 0xDEAD_BEEF,
        },
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(vec![
            ("precip".into(), 1.0),
            ("temperature".into(), 0.5),
            ("plants".into(), 100.0),
            ("herbivores".into(), 40.0),
            ("predators".into(), 8.0),
            ("detritus".into(), 0.0),
        ]),
        interventions,
        seeds: vec![12345, 67890],
        duration_ticks: 600,
        sample_period: ECOLOGY_PERIOD,
        observable_ids: vec!["plants".into(), "herbivores".into(), "predators".into()],
        exotic_interventions: Vec::new(),
    }
}

#[test]
fn bit_exact_replay_parity_across_repeated_runs() {
    let registry = ObservableRegistry::reference_default();
    let manifest = create_test_manifest(vec![
        InterventionCommand {
            id: 1,
            cause_id: 10,
            kind: InterventionKind::ToxicityPlume {
                center_x: 8.0,
                center_y: 8.0,
                radius: 4.0,
                toxicity_level: 0.8,
                decay_rate: 0.05,
            },
            region: Region::Global,
            start_tick: 120,
            duration_ticks: 180,
            intensity: 1.0,
            signed_negative: true,
            curve: Curve::Step,
            reversible: false,
        },
        InterventionCommand {
            id: 2,
            cause_id: 20,
            kind: InterventionKind::GeneticInjection {
                target_species: Some(1),
                trait_modifications: vec![
                    TraitMod::ScaleMorphology(1.2),
                    TraitMod::MetabolicEfficiencyDelta(0.15),
                ],
                count: 30,
            },
            region: Region::Global,
            start_tick: 300,
            duration_ticks: 120,
            intensity: 1.0,
            signed_negative: false,
            curve: Curve::Step,
            reversible: true,
        },
    ]);

    let seed = 12345;

    let run1 = run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, seed, None, None);
    let run2 = run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, seed, None, None);

    assert_eq!(run1.status, run2.status);
    assert_eq!(run1.final_checksum, run2.final_checksum);
    assert_eq!(run1.final_observables, run2.final_observables);
    assert_eq!(run1.series.len(), run2.series.len());

    for (s1, s2) in run1.series.iter().zip(run2.series.iter()) {
        assert_eq!(s1.tick, s2.tick);
        assert_eq!(s1.observables, s2.observables);
    }
}

#[test]
fn altering_extended_interventions_causes_deterministic_checksum_divergence() {
    let registry = ObservableRegistry::reference_default();

    let manifest_a = create_test_manifest(vec![
        InterventionCommand {
            id: 1,
            cause_id: 10,
            kind: InterventionKind::PathogenTransmission {
                pathogen_id: 1,
                transmission_rate: 0.5,
                mortality_multiplier: 1.0,
            },
            region: Region::Global,
            start_tick: 60,
            duration_ticks: 180,
            intensity: 1.0,
            signed_negative: true,
            curve: Curve::Step,
            reversible: false,
        },
    ]);

    let manifest_b = create_test_manifest(vec![
        InterventionCommand {
            id: 1,
            cause_id: 10,
            kind: InterventionKind::PathogenTransmission {
                pathogen_id: 1,
                transmission_rate: 0.9, // Altered transmission rate
                mortality_multiplier: 1.5,
            },
            region: Region::Global,
            start_tick: 60,
            duration_ticks: 180,
            intensity: 1.0,
            signed_negative: true,
            curve: Curve::Step,
            reversible: false,
        },
    ]);

    let seed = 12345;
    let run_a = run_manifest_seed::<ReferenceEvolutionWorld>(&manifest_a, &registry, seed, None, None);
    let run_b = run_manifest_seed::<ReferenceEvolutionWorld>(&manifest_b, &registry, seed, None, None);

    assert_eq!(run_a.status, anima_engine_lib::core::experiment_runner::RunStatus::Completed);
    assert_eq!(run_b.status, anima_engine_lib::core::experiment_runner::RunStatus::Completed);
    assert_ne!(run_a.final_checksum, run_b.final_checksum);
}

#[test]
fn ensemble_execution_isolates_random_streams_per_seed() {
    let registry = ObservableRegistry::reference_default();
    let manifest = create_test_manifest(vec![]);

    let ensemble = run_ensemble::<ReferenceEvolutionWorld>(&manifest, &registry)
        .expect("ensemble execution must succeed");

    assert_eq!(ensemble.completed_runs, 2);
    assert_eq!(ensemble.runs.len(), 2);

    let run_seed1 = &ensemble.runs[0];
    let run_seed2 = &ensemble.runs[1];

    assert_eq!(run_seed1.provenance.seed, 12345);
    assert_eq!(run_seed2.provenance.seed, 67890);

    // Runs under different seeds have distinct trajectories
    assert_ne!(run_seed1.final_checksum, run_seed2.final_checksum);
}
