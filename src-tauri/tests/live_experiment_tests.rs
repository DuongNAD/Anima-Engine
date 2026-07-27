//! §3.3 gate: the **live** Bevy world under the experiment contract.
//!
//! Every test here drives `LiveExperimentAdapter` through the *shared* runner —
//! `run_manifest_seed`, `checkpoint_fork` — the same functions the headless reference world goes
//! through, with the same `ExperimentManifest`, `SimClock`, `InterventionCommand` semantics,
//! `CausalLedger` and `ObservableRegistry` type. And the adapter runs
//! `simulation_schedule::build_tick_schedule`, which is literally the function
//! `SimulationEngine::start` calls, so what is exercised here is the app's schedule rather than a
//! hand-declared subset of it.
//!
//! What is deliberately **not** claimed: numerical agreement with the reference world. Seven
//! relaxing scalars and a 256×256 field with a physics solver are not two implementations of one
//! equation. `reference_and_live_agree_on_the_direction_of_a_shared_law` states the claim that can
//! honestly be made.

use anima_engine_lib::core::experiment::{
    ExperimentError, ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{
    checkpoint_fork, run_manifest_seed, ExperimentModel, RunStatus,
};
use anima_engine_lib::core::intervention::{Curve, InterventionCommand, InterventionKind, Region};
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveSnapshot, LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::snapshot::{self, SnapshotEnvelope};
use anima_engine_lib::core::world_artifact::WorldIdentity;

/// Short enough that a debug-build suite finishes, long enough that the ecology band (60 ticks)
/// fires three times so a band-gated forcing has a schedule to be exact about.
const RUN_TICKS: u64 = 180;
const SEED: u64 = 4242;

fn live_registry() -> ObservableRegistry {
    ObservableRegistry::live_default()
}

fn base_initial() -> InitialConditionSet {
    InitialConditionSet::new(vec![
        ("live.founders".to_string(), 6.0),
        ("live.predator_fraction".to_string(), 0.5),
        ("live.trees".to_string(), 4.0),
        ("live.lakes".to_string(), 2.0),
    ])
}

fn manifest(name: &str, interventions: Vec<InterventionCommand>) -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: name.to_string(),
        name: name.to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: base_initial(),
        interventions,
        seeds: vec![SEED],
        duration_ticks: RUN_TICKS,
        sample_period: 60,
        observable_ids: LIVE_OBSERVABLE_IDS.iter().map(|s| s.to_string()).collect(),
        exotic_interventions: Vec::new(),
        observer: Default::default(),
    }
}

fn deforest(start_tick: u64) -> InterventionCommand {
    InterventionCommand {
        id: 1,
        cause_id: 7,
        kind: InterventionKind::Deforest,
        region: Region::Global,
        start_tick,
        duration_ticks: RUN_TICKS * 2,
        intensity: 0.5,
        signed_negative: true,
        curve: Curve::Step,
        reversible: false,
    }
}

fn cull_predators(start_tick: u64) -> InterventionCommand {
    InterventionCommand {
        id: 2,
        cause_id: 9,
        kind: InterventionKind::RemovePredators,
        region: Region::Global,
        start_tick,
        duration_ticks: RUN_TICKS * 2,
        intensity: 1.0,
        signed_negative: false,
        curve: Curve::Step,
        reversible: false,
    }
}

// ---- Determinism -------------------------------------------------------------------------------

#[test]
fn the_same_seed_and_manifest_give_the_same_live_checksum() {
    let m = manifest("live-determinism", vec![]);
    let reg = live_registry();
    let a = run_manifest_seed::<LiveExperimentAdapter>(&m, &reg, SEED, None, None);
    let b = run_manifest_seed::<LiveExperimentAdapter>(&m, &reg, SEED, None, None);

    assert_eq!(a.status, RunStatus::Completed, "run a: {:?}", a.status);
    assert_eq!(b.status, RunStatus::Completed, "run b: {:?}", b.status);
    assert_eq!(
        a.final_checksum, b.final_checksum,
        "the live world must replay to the same checksum from the same manifest and seed"
    );
    assert_eq!(a.final_observables, b.final_observables);
    assert_eq!(a.series, b.series);
    assert_eq!(
        a.provenance.manifest_fingerprint, b.provenance.manifest_fingerprint,
        "both runs must be attributed to the same manifest"
    );
}

#[test]
fn the_live_run_actually_moved_and_is_not_a_frozen_world() {
    // A checksum that reproduces would also reproduce if the schedule did nothing at all. This is
    // the control: 180 ticks of the real schedule must change the world.
    let m = manifest("live-motion", vec![]);
    let reg = live_registry();
    let result = run_manifest_seed::<LiveExperimentAdapter>(&m, &reg, SEED, None, None);
    assert_eq!(result.status, RunStatus::Completed);
    assert_eq!(result.series.len(), (RUN_TICKS / 60) as usize);

    let first = &result.series[0];
    let last = result.series.last().expect("a sampled series");
    assert_ne!(
        first.observables, last.observables,
        "the live world did not change over {RUN_TICKS} ticks of its own schedule"
    );
    let agents = result
        .observable("live.agent_count")
        .expect("agent count is emitted");
    assert!(agents > 0.0, "the founding population vanished");
    assert!(result.final_observables.iter().all(|(_, v)| v.is_finite()));
}

// ---- Intervention timing -------------------------------------------------------------------------

#[test]
fn a_forcing_fires_on_the_exact_declared_tick_and_never_before_it() {
    let reg = live_registry();

    // Active from tick 1: the ecology band's first firing inside the window is tick 60.
    let early = run_manifest_seed::<LiveExperimentAdapter>(
        &manifest("live-forcing-early", vec![deforest(1)]),
        &reg,
        SEED,
        None,
        None,
    );
    assert_eq!(early.status, RunStatus::Completed);
    let early_ticks: Vec<u64> = early.ledger.all().iter().map(|r| r.tick).collect();
    assert_eq!(
        early_ticks,
        vec![60, 120, 180],
        "a global deforestation active from tick 1 applies on every ecology-band firing"
    );

    // Active from tick 61: tick 60 is outside the window, so the first application is 120.
    let late = run_manifest_seed::<LiveExperimentAdapter>(
        &manifest("live-forcing-late", vec![deforest(61)]),
        &reg,
        SEED,
        None,
        None,
    );
    assert_eq!(late.status, RunStatus::Completed);
    let late_ticks: Vec<u64> = late.ledger.all().iter().map(|r| r.tick).collect();
    assert_eq!(
        late_ticks,
        vec![120, 180],
        "a forcing whose window opens at 61 must not act on tick 60"
    );

    // Every effect is attributed to the intervention that caused it, not to background dynamics.
    for rec in late.ledger.all() {
        assert_eq!(late.ledger.root_cause(rec.effect_id), Some(7));
        assert_eq!(rec.target, "plants");
        assert!(rec.delta < 0.0, "deforestation must remove standing crop");
    }
}

#[test]
fn a_manifest_without_interventions_records_nothing_and_differs_from_one_with_them() {
    let reg = live_registry();
    let control = run_manifest_seed::<LiveExperimentAdapter>(
        &manifest("live-control", vec![]),
        &reg,
        SEED,
        None,
        None,
    );
    let treatment = run_manifest_seed::<LiveExperimentAdapter>(
        &manifest("live-treatment", vec![deforest(1)]),
        &reg,
        SEED,
        None,
        None,
    );
    assert!(
        control.ledger.is_empty(),
        "an undisturbed run has nothing to explain"
    );
    assert!(!treatment.ledger.is_empty());
    assert_ne!(
        control.final_checksum, treatment.final_checksum,
        "the declared factor must actually change the world"
    );
    let control_plants = control.observable("plants").expect("plants");
    let treatment_plants = treatment.observable("plants").expect("plants");
    assert!(
        treatment_plants < control_plants,
        "deforestation should leave less standing crop: control {control_plants}, treatment {treatment_plants}"
    );
}

#[test]
fn removing_predators_removes_predators_and_conserves_their_energy() {
    let reg = live_registry();
    let control = run_manifest_seed::<LiveExperimentAdapter>(
        &manifest("live-cull-control", vec![]),
        &reg,
        SEED,
        None,
        None,
    );
    let treatment = run_manifest_seed::<LiveExperimentAdapter>(
        &manifest("live-cull", vec![cull_predators(1)]),
        &reg,
        SEED,
        None,
        None,
    );
    let control_preds = control
        .observable("live.predator_count")
        .expect("predators");
    let treatment_preds = treatment
        .observable("live.predator_count")
        .expect("predators");
    assert!(control_preds > 0.0, "the control run must have predators");
    assert_eq!(
        treatment_preds, 0.0,
        "a full-intensity cull must leave no predators"
    );

    // The culled bodies' reserves went into detritus rather than out of the world: the closed-EU
    // total is the invariant a declared forcing may not break.
    let control_eu = control.observable("live.closed_eu_total").expect("eu");
    let treatment_eu = treatment.observable("live.closed_eu_total").expect("eu");
    let drift = (treatment_eu - control_eu).abs();
    assert!(
        drift / control_eu.abs().max(1.0) < 1e-6,
        "culling predators moved {drift} EU into or out of a closed system \
         (control {control_eu}, treatment {treatment_eu})"
    );
}

fn rate_forcing(
    id: u32,
    cause_id: u32,
    kind: InterventionKind,
    negative: bool,
) -> InterventionCommand {
    InterventionCommand {
        id,
        cause_id,
        kind,
        region: Region::Global,
        start_tick: 1,
        duration_ticks: RUN_TICKS * 2,
        intensity: 0.8,
        signed_negative: negative,
        curve: Curve::Step,
        reversible: true,
    }
}

#[test]
fn a_rainfall_forcing_scales_regrowth_and_is_sampled_on_the_ecology_band() {
    let reg = live_registry();
    let control = run_manifest_seed::<LiveExperimentAdapter>(
        &manifest("live-rain-control", vec![]),
        &reg,
        SEED,
        None,
        None,
    );
    let drought = run_manifest_seed::<LiveExperimentAdapter>(
        &manifest(
            "live-drought",
            vec![rate_forcing(3, 11, InterventionKind::RainfallDelta, true)],
        ),
        &reg,
        SEED,
        None,
        None,
    );
    assert_eq!(drought.status, RunStatus::Completed);

    let control_crop = control.observable("live.standing_crop").expect("crop");
    let drought_crop = drought.observable("live.standing_crop").expect("crop");
    assert!(
        drought_crop < control_crop,
        "a −80% rainfall forcing must leave less standing crop: control {control_crop}, \
         drought {drought_crop}"
    );

    // The forcing acts every tick, but its provenance is sampled on the ecology band — one record
    // per firing, not sixty a second.
    let ticks: Vec<u64> = drought.ledger.all().iter().map(|r| r.tick).collect();
    assert_eq!(ticks, vec![60, 120, 180]);
    for rec in drought.ledger.all() {
        assert_eq!(rec.target, "live.fertility_multiplier");
        assert_eq!(drought.ledger.root_cause(rec.effect_id), Some(11));
        assert!(rec.quantity < 1.0, "a drought multiplier must be below 1");
    }
}

#[test]
fn a_temperature_forcing_changes_the_world_and_records_why() {
    let reg = live_registry();
    let control = run_manifest_seed::<LiveExperimentAdapter>(
        &manifest("live-temp-control", vec![]),
        &reg,
        SEED,
        None,
        None,
    );
    let warmed = run_manifest_seed::<LiveExperimentAdapter>(
        &manifest(
            "live-warming",
            vec![rate_forcing(
                4,
                13,
                InterventionKind::TemperatureDelta,
                false,
            )],
        ),
        &reg,
        SEED,
        None,
        None,
    );
    assert_eq!(warmed.status, RunStatus::Completed);
    assert_ne!(
        control.final_checksum, warmed.final_checksum,
        "shifting the homeostatic temperature target must change the world"
    );
    assert!(warmed.final_observables.iter().all(|(_, v)| v.is_finite()));

    let records = warmed.ledger.all();
    assert!(!records.is_empty());
    for rec in records {
        assert_eq!(rec.target, "live.temp_target_shift_c");
        assert_eq!(warmed.ledger.root_cause(rec.effect_id), Some(13));
        assert!(
            rec.quantity > 0.0,
            "a warming forcing must record a positive shift"
        );
    }
    // No direction is claimed for the *consequences* of a target shift: the engine's sweat and
    // metabolism terms both read it, and asserting a sign here would be a guess dressed as a gate.
}

// ---- Fork from a known tick ----------------------------------------------------------------------

#[test]
fn a_fork_from_a_known_tick_shares_its_prefix_and_diverges_only_afterwards() {
    let reg = live_registry();
    let m = manifest("live-fork", vec![]);
    let report = checkpoint_fork::<LiveExperimentAdapter>(&m, &reg, SEED, 60, &[deforest(61)])
        .expect("the fork must be accepted");

    assert_eq!(report.fork_tick, 60);
    assert_eq!(report.prefix.status, RunStatus::Completed);
    assert_eq!(report.control.status, RunStatus::Completed);
    assert_eq!(report.treatment.status, RunStatus::Completed);

    // The control branch continued from the checkpoint with the manifest's own inputs, so it must
    // land where an uninterrupted run lands.
    let uninterrupted = run_manifest_seed::<LiveExperimentAdapter>(&m, &reg, SEED, None, None);
    assert_eq!(
        report.control.final_checksum, uninterrupted.final_checksum,
        "a control branch forked at tick 60 diverged from an uninterrupted run"
    );
    assert_ne!(
        report.control.final_checksum, report.treatment.final_checksum,
        "the treatment factor must change the branch"
    );
    assert!(
        report.delta_of("plants").expect("plants delta") < 0.0,
        "post-fork deforestation must lower standing crop relative to the control"
    );
    // Provenance names the parent and the tick, so the branch can be replayed.
    assert_eq!(
        report.control.provenance.parent_run_id,
        Some(report.prefix.provenance.run_id.clone())
    );
    assert_eq!(report.treatment.provenance.fork_tick, Some(60));
}

// ---- Save / load / resume ------------------------------------------------------------------------

/// Round-trip a live snapshot through the real on-disk format — envelope, checksum, atomic write,
/// schema migration — rather than copying a struct in memory.
fn through_disk(snapshot: &LiveSnapshot, label: &str) -> LiveSnapshot {
    let dir = std::env::temp_dir().join(format!("anima_live_{}_{}", std::process::id(), label));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("checkpoint.json");
    let envelope = SnapshotEnvelope::seal(snapshot.saved.clone()).expect("seal");
    snapshot::write_atomic(&path, &envelope).expect("write");
    let back = snapshot::read(&path).expect("read");
    assert_eq!(
        back.loaded_from_schema,
        snapshot::SCHEMA_VERSION,
        "a snapshot this build wrote must read back at the current schema"
    );
    let _ = std::fs::remove_dir_all(&dir);
    LiveSnapshot {
        saved: back,
        config: snapshot.config,
        seed: snapshot.seed,
    }
}

#[test]
fn run_k_then_save_load_then_run_the_rest_matches_an_uninterrupted_run() {
    use anima_engine_lib::core::causal::CausalLedger;
    use anima_engine_lib::core::intervention::InterventionQueue;
    use anima_engine_lib::core::sim_clock::SimClock;
    use rand::SeedableRng;

    const K: u64 = 60;

    let laws = WorldLawSet::baseline();
    let initial = base_initial();
    let queue = InterventionQueue::new(vec![]);

    // Uninterrupted.
    let mut reference =
        LiveExperimentAdapter::from_manifest(&laws, &initial, &[], SEED, (16, 16), RUN_TICKS)
            .expect("build");
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);
    let mut clock = SimClock::new();
    let mut ledger = CausalLedger::new();
    for _ in 0..RUN_TICKS {
        let tick = clock.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        reference.step(&clock, &active, &mut ledger, &mut rng);
    }
    let reference_checksum = reference.checksum();
    let reference_observables = reference.observables();

    // Interrupted at K, through a real file, then resumed.
    let mut interrupted =
        LiveExperimentAdapter::from_manifest(&laws, &initial, &[], SEED, (16, 16), RUN_TICKS)
            .expect("build");
    let mut rng_b = rand::rngs::StdRng::seed_from_u64(SEED);
    let mut clock_b = SimClock::new();
    let mut ledger_b = CausalLedger::new();
    for _ in 0..K {
        let tick = clock_b.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        interrupted.step(&clock_b, &active, &mut ledger_b, &mut rng_b);
    }
    let snapshot = through_disk(&interrupted.snapshot(), "resume");
    assert_eq!(
        snapshot
            .saved
            .experiment
            .as_ref()
            .expect("the snapshot carries its experiment state")
            .clock_tick,
        K,
        "the multi-rate clock must survive the round trip, or a resumed run applies band-gated \
         forcings on the wrong ticks"
    );

    let mut resumed = LiveExperimentAdapter::from_snapshot(&snapshot).expect("resume");
    let mut clock_c = SimClock { tick: K };
    for _ in 0..(RUN_TICKS - K) {
        let tick = clock_c.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        resumed.step(&clock_c, &active, &mut ledger_b, &mut rng_b);
    }

    assert_eq!(
        reference_checksum,
        resumed.checksum(),
        "a live run resumed from a checkpoint diverged from an uninterrupted one; some piece of \
         trajectory-relevant state is missing from the snapshot"
    );
    assert_eq!(reference_observables, resumed.observables());
}

#[test]
fn a_snapshot_without_experiment_state_cannot_be_resumed_as_an_experiment() {
    let mut saved = anima_engine_lib::core::simulation_state::empty_saved_state_for_tests();
    saved.experiment = None;
    let snapshot = LiveSnapshot {
        saved,
        config: Default::default(),
        seed: SEED,
    };
    match LiveExperimentAdapter::from_snapshot(&snapshot) {
        Err(ExperimentError::InvalidLaw { reason }) => {
            assert!(reason.contains("no live experiment state"), "{reason}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_snapshot_whose_laws_were_edited_is_refused_because_a_law_is_immutable_in_a_run() {
    let laws = WorldLawSet::baseline();
    let initial = base_initial();
    let adapter =
        LiveExperimentAdapter::from_manifest(&laws, &initial, &[], SEED, (16, 16), RUN_TICKS)
            .expect("build");
    let mut snapshot = adapter.snapshot();

    // Rewrite the recorded law fingerprint, the way a hand-edited or corrupted file would.
    let state = snapshot
        .saved
        .experiment
        .as_mut()
        .expect("experiment state");
    state.law_fingerprint ^= 0xDEAD_BEEF;

    match LiveExperimentAdapter::from_snapshot(&snapshot) {
        Err(ExperimentError::InvalidLaw { reason }) => {
            assert!(reason.contains("ER01"), "{reason}");
        }
        other => panic!("expected a law-immutability refusal, got {other:?}"),
    }
}

#[test]
fn a_snapshot_from_an_unknown_live_schema_is_refused_rather_than_guessed_at() {
    let laws = WorldLawSet::baseline();
    let adapter = LiveExperimentAdapter::from_manifest(
        &laws,
        &base_initial(),
        &[],
        SEED,
        (16, 16),
        RUN_TICKS,
    )
    .expect("build");
    let mut snapshot = adapter.snapshot();
    snapshot
        .saved
        .experiment
        .as_mut()
        .expect("experiment state")
        .schema_version = 99;
    match LiveExperimentAdapter::from_snapshot(&snapshot) {
        Err(ExperimentError::UnsupportedSchemaVersion { component, .. }) => {
            assert_eq!(component, "live_experiment");
        }
        other => panic!("expected an unsupported-schema refusal, got {other:?}"),
    }
}

// ---- Refusals, rather than silent wrong answers --------------------------------------------------

#[test]
fn an_exotic_law_is_refused_rather_than_run_as_a_baseline() {
    use anima_engine_lib::core::exotic_energy::{
        BoundaryMode, EnergySourceId, ExoticEnergyLaw, ExoticSourceModel, SourceTopology, UnitId,
    };

    let law = ExoticEnergyLaw {
        id: EnergySourceId::new("mana"),
        display_name: "Mana".to_string(),
        unit: UnitId::new("MU"),
        source_model: ExoticSourceModel::Renewable,
        topology: SourceTopology::Uniform,
        initial_amount: 1.0,
        source_rate: 0.1,
        diffusion_rate: 0.1,
        decay_rate: 0.01,
        max_density: 10.0,
        boundary: BoundaryMode::Closed,
    };
    let laws = WorldLawSet::with_exotic(law);
    match LiveExperimentAdapter::from_manifest(
        &laws,
        &base_initial(),
        &[],
        SEED,
        (16, 16),
        RUN_TICKS,
    ) {
        Err(ExperimentError::InvalidLaw { reason }) => {
            assert!(reason.contains("exotic-energy field"), "{reason}");
        }
        other => panic!("expected the live world to refuse an exotic law, got {other:?}"),
    }
}

#[test]
fn an_initial_condition_the_live_world_cannot_honour_is_refused() {
    let initial = InitialConditionSet::new(vec![
        ("live.founders".to_string(), 4.0),
        // A reference-world key. Silently ignoring it would run a different experiment and report
        // it as the declared one.
        ("plants".to_string(), 100.0),
    ]);
    match LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        SEED,
        (16, 16),
        RUN_TICKS,
    ) {
        Err(ExperimentError::InvalidLaw { reason }) => {
            assert!(reason.contains("'plants'"), "{reason}");
        }
        other => panic!("expected an unknown-key refusal, got {other:?}"),
    }

    // A key it does honour, but out of range.
    let bad = InitialConditionSet::new(vec![("live.predator_fraction".to_string(), 1.7)]);
    match LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &bad,
        &[],
        SEED,
        (16, 16),
        RUN_TICKS,
    ) {
        Err(ExperimentError::OutOfRange { field, .. }) => {
            assert_eq!(field, "live.predator_fraction");
        }
        other => panic!("expected an out-of-range refusal, got {other:?}"),
    }
}

// ---- Observable identity ---------------------------------------------------------------------------

#[test]
fn the_live_registry_is_valid_and_describes_exactly_what_the_world_emits() {
    let reg = live_registry();
    reg.validate().expect("the live registry must be valid");

    let m = manifest("live-observables", vec![]);
    let result = run_manifest_seed::<LiveExperimentAdapter>(&m, &reg, SEED, None, None);
    assert_eq!(result.status, RunStatus::Completed);
    assert!(
        result.warnings.is_empty(),
        "every emitted observable must have a registry spec: {:?}",
        result.warnings
    );

    let emitted: Vec<&str> = result
        .final_observables
        .iter()
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(
        emitted,
        LIVE_OBSERVABLE_IDS.to_vec(),
        "the emission order is part of the contract"
    );
    for id in LIVE_OBSERVABLE_IDS {
        let spec = reg.get(id).unwrap_or_else(|| panic!("no spec for {id}"));
        assert!(!spec.unit.is_empty(), "{id} has no unit");
        assert_eq!(spec.source, "core::live_experiment::LiveExperimentAdapter");
    }
}

#[test]
fn live_and_reference_agree_on_shared_ids() {
    // Two registries may share an id only when it means the same thing. Anything else is two units
    // wearing one name, which is exactly what `ObservableSpec` exists to prevent.
    let live = ObservableRegistry::live_default();
    let reference = ObservableRegistry::reference_default();
    let mut shared = 0;
    for spec in live.specs() {
        let Some(other) = reference.get(&spec.id) else {
            continue;
        };
        shared += 1;
        assert_eq!(
            spec.unit, other.unit,
            "'{}' has unit '{}' live and '{}' in the reference registry",
            spec.id, spec.unit, other.unit
        );
        assert_eq!(
            spec.conservation, other.conservation,
            "'{}' has a different conservation role in the two registries",
            spec.id
        );
        assert_eq!(
            spec.scope, other.scope,
            "'{}' has a different scope",
            spec.id
        );
    }
    assert_eq!(
        shared, 2,
        "exactly `plants` and `detritus` are meant to be shared; a new shared id needs a decision, \
         not a silent addition"
    );
}

#[test]
fn reference_and_live_agree_on_the_direction_of_a_shared_law() {
    use anima_engine_lib::core::reference_world::ReferenceEvolutionWorld;

    // The reference path, through the same runner and the same intervention type.
    let reference_registry = ObservableRegistry::reference_default();
    let mut reference_manifest = ExperimentManifest {
        initial_conditions: InitialConditionSet::new(vec![]),
        observable_ids: vec!["plants".to_string(), "detritus".to_string()],
        ..manifest("reference-deforest", vec![deforest(1)])
    };
    reference_manifest.duration_ticks = 1_200;
    let reference_treatment = run_manifest_seed::<ReferenceEvolutionWorld>(
        &reference_manifest,
        &reference_registry,
        SEED,
        None,
        None,
    );
    let mut reference_control_manifest = reference_manifest.clone();
    reference_control_manifest.interventions.clear();
    reference_control_manifest.experiment_id = "reference-control".to_string();
    reference_control_manifest.name = "reference-control".to_string();
    let reference_control = run_manifest_seed::<ReferenceEvolutionWorld>(
        &reference_control_manifest,
        &reference_registry,
        SEED,
        None,
        None,
    );

    let live_registry = ObservableRegistry::live_default();
    let live_treatment = run_manifest_seed::<LiveExperimentAdapter>(
        &manifest("live-deforest", vec![deforest(1)]),
        &live_registry,
        SEED,
        None,
        None,
    );
    let live_control = run_manifest_seed::<LiveExperimentAdapter>(
        &manifest("live-deforest-control", vec![]),
        &live_registry,
        SEED,
        None,
        None,
    );

    for (label, control, treatment) in [
        ("reference", &reference_control, &reference_treatment),
        ("live", &live_control, &live_treatment),
    ] {
        assert_eq!(
            control.status,
            RunStatus::Completed,
            "{label} control: {:?}",
            control.status
        );
        assert_eq!(
            treatment.status,
            RunStatus::Completed,
            "{label} treatment: {:?}",
            treatment.status
        );
        let c = control.observable("plants").expect("plants");
        let t = treatment.observable("plants").expect("plants");
        assert!(
            t < c,
            "{label}: deforestation must lower `plants`, got control {c} and treatment {t}"
        );
    }
    // The numbers are not comparable and this test never pretends otherwise — only the sign is.
}
